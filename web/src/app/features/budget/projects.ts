import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ProjectView } from '../../api/models/project-view';
import { BudgetsService } from '../../api/services/budgets.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, toIsoDate, todayIso } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { parseMoney, sum, toEditable } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { openAndRefresh } from './budget-dialogs';
import { BudgetGauge } from './gauge';
import { DocumentsService } from '../../api/services/documents.service';
import { confirmDeletion } from '../documents/delete-confirmation';
import { DocumentSection } from '../documents/document-section';
import { LineCategories } from './line-categories';
import { CategoriesStore } from '../../shared/stores';

/** Poste en cours de saisie (R-BUD-26) ; `key` identifie la ligne du formulaire. */
interface LineDraft {
  key: number;
  id: string | null;
  name: string;
  amount: string;
  categories: string[];
}

/** Création ou modification d'un projet (BUD-06, R-BUD-20, R-BUD-21), et de ses postes (BUD-11, R-BUD-26). */
@Component({
  selector: 'app-project-dialog',
  imports: [
    FormsModule,
    MatIconModule,
    MoneyPipe,
    LineCategories,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ project ? 'Modifier le projet' : 'Nouveau projet' }}</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Nom</mat-label>
        <input matInput maxlength="80" [ngModel]="name()" (ngModelChange)="name.set($event)" />
      </mat-form-field>
      <div class="dates">
        <mat-form-field>
          <mat-label>Début</mat-label>
          <input
            matInput
            [matDatepicker]="startPicker"
            [ngModel]="start()"
            (ngModelChange)="start.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="startPicker" />
          <mat-datepicker #startPicker />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Fin (facultative)</mat-label>
          <input
            matInput
            [matDatepicker]="endPicker"
            [ngModel]="end()"
            (ngModelChange)="end.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="endPicker" />
          <mat-datepicker #endPicker />
        </mat-form-field>
      </div>
      @if (lines().length === 0) {
        <mat-form-field>
          <mat-label>Montant prévu</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="amount()"
            (ngModelChange)="amount.set($event)"
          />
          <span matTextSuffix>€</span>
        </mat-form-field>
      }
      <section class="lines" aria-label="Postes">
        <div class="lines-head">
          <strong>Postes</strong>
          @if (lines().length > 0) {
            <span class="muted">
              Total
              @if (linesTotal(); as total) {
                {{ total | money }}
              } @else {
                —
              }
            </span>
          }
        </div>
        @if (lines().length === 0) {
          <p class="muted small">
            Détaillez le prévu (train, logement, restaurants…) : le montant du projet devient la
            somme de ses postes.
          </p>
        }
        @for (line of lines(); track line.key; let i = $index) {
          <div class="line">
            <div class="line-fields">
              <mat-form-field class="line-name" subscriptSizing="dynamic">
                <mat-label>Poste</mat-label>
                <input
                  matInput
                  maxlength="80"
                  [ngModel]="line.name"
                  (ngModelChange)="patch(i, { name: $event })"
                />
              </mat-form-field>
              <mat-form-field class="line-amount" subscriptSizing="dynamic">
                <mat-label>Montant</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="line.amount"
                  (ngModelChange)="patch(i, { amount: $event })"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
              <button
                mat-icon-button
                type="button"
                (click)="removeLine(i)"
                [attr.aria-label]="'Retirer le poste ' + (line.name || i + 1)"
              >
                <mat-icon>delete</mat-icon>
              </button>
            </div>
            <app-line-categories
              kind="EXPENSE"
              [value]="line.categories"
              (valueChange)="patch(i, { categories: $event })"
            />
          </div>
        }
        <button mat-button type="button" class="add" (click)="addLine()">
          <mat-icon>add</mat-icon> Ajouter un poste
        </button>
        @if (lines().length > 0) {
          <p class="muted small">
            Les catégories d'un poste y rangent les transactions rattachées au projet ; les autres
            vont « hors postes », et se rangent depuis le détail de la transaction.
          </p>
        }
      </section>
      <mat-checkbox [ngModel]="exclude()" (ngModelChange)="exclude.set($event)">
        Retirer ses dépenses des lignes du mois
      </mat-checkbox>
      <p class="muted small">
        Elles restent comptées dans les analyses par catégorie et dans le solde du mois.
      </p>
      @if (project) {
        <mat-checkbox [ngModel]="closed()" (ngModelChange)="closed.set($event)">
          Projet terminé (clôturé)
        </mat-checkbox>
      }
      <mat-form-field>
        <mat-label>Note</mat-label>
        <textarea
          matInput
          rows="2"
          [ngModel]="note()"
          (ngModelChange)="note.set($event)"
        ></textarea>
      </mat-form-field>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid() || saving()" (click)="save()">
        {{ project ? 'Enregistrer' : 'Créer' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(34rem, 80vw);
    }
    .dates {
      display: flex;
      gap: var(--field-gap);
    }
    .lines {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }
    .lines-head {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
    }
    .line {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
      padding: 0.75rem 0;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .line-fields {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .line-name {
      flex: 2;
      min-width: 0;
    }
    .line-amount {
      flex: 1;
      min-width: 6rem;
    }
    .add {
      align-self: flex-start;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class ProjectDialog {
  protected readonly project = inject<ProjectView | null>(MAT_DIALOG_DATA);
  private readonly api = inject(BudgetsService);
  private readonly ref = inject(MatDialogRef<ProjectDialog>);

  protected readonly name = signal(this.project?.name ?? '');
  protected readonly start = signal<Date | null>(
    fromIsoDate(this.project?.start_date ?? todayIso()),
  );
  protected readonly end = signal<Date | null>(
    this.project?.end_date ? fromIsoDate(this.project.end_date) : null,
  );
  protected readonly amount = signal(
    this.project && this.project.lines.length === 0 ? toEditable(this.project.amount_minor) : '',
  );
  private nextKey = 0;
  protected readonly lines = signal<LineDraft[]>(
    (this.project?.lines ?? []).map((l) => ({
      key: this.nextKey++,
      id: l.id,
      name: l.name,
      amount: toEditable(l.amount_minor),
      categories: l.category_ids,
    })),
  );
  /** Prévu du projet détaillé : somme des postes, `null` tant qu'un montant est illisible (R-BUD-26). */
  protected readonly linesTotal = computed(() => {
    const amounts = this.lines().map((l) => parseMoney(l.amount));
    return amounts.every((a) => a !== null) ? sum(amounts as number[]) : null;
  });
  protected readonly exclude = signal(this.project?.exclude_from_period_budgets ?? true);
  protected readonly closed = signal(this.project ? !this.project.active : false);
  protected readonly note = signal(this.project?.note ?? '');
  protected readonly error = signal<string | null>(null);
  protected readonly saving = signal(false);

  protected readonly valid = computed(() => {
    if (!this.name().trim() || !this.start()) return false;
    const lines = this.lines();
    if (lines.length === 0) {
      const amount = parseMoney(this.amount());
      return amount !== null && amount > 0;
    }
    return lines.every((l) => {
      const amount = parseMoney(l.amount);
      return !!l.name.trim() && amount !== null && amount > 0;
    });
  });

  /** Le premier poste reprend le montant global déjà saisi. */
  protected addLine(): void {
    const first = this.lines().length === 0;
    this.lines.update((lines) => [
      ...lines,
      {
        key: this.nextKey++,
        id: null,
        name: '',
        amount: first ? this.amount() : '',
        categories: [],
      },
    ]);
  }

  /** Retirer le dernier poste rend au projet un montant global : la somme qu'il avait. */
  protected removeLine(index: number): void {
    if (this.lines().length === 1) this.amount.set(this.lines()[0].amount);
    this.lines.update((lines) => lines.filter((_, i) => i !== index));
  }

  protected patch(index: number, change: Partial<LineDraft>): void {
    this.lines.update((lines) => lines.map((l, i) => (i === index ? { ...l, ...change } : l)));
  }

  protected async save(): Promise<void> {
    const start = this.start();
    if (!start) return;
    const end = this.end();
    const body = {
      name: this.name(),
      start_date: toIsoDate(start),
      end_date: end ? toIsoDate(end) : null,
      amount_minor: this.lines().length === 0 ? (parseMoney(this.amount()) ?? 0) : null,
      lines: this.lines().map((l) => ({
        id: l.id,
        name: l.name,
        amount_minor: parseMoney(l.amount) ?? 0,
        category_ids: l.categories,
      })),
      exclude_from_period_budgets: this.exclude(),
      active: !this.closed(),
      note: this.note() || null,
      alert_thresholds: this.project?.alert_thresholds ?? null,
    };
    this.saving.set(true);
    this.error.set(null);
    try {
      const saved = this.project
        ? await firstValueFrom(this.api.updateProject({ id: this.project.id, body }))
        : await firstValueFrom(this.api.createProject({ body }));
      this.ref.close(saved.id);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.saving.set(false);
    }
  }
}

/** E20 — Projets (BUD-06). */
@Component({
  selector: 'app-projects',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatProgressBarModule,
    MoneyPipe,
    FrDatePipe,
    BudgetGauge,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/budget" aria-label="Retour au budget">
          <mat-icon>arrow_back</mat-icon>
        </a>
        <h1>Projets</h1>
        <button mat-flat-button (click)="create()"><mat-icon>add</mat-icon> Nouveau projet</button>
      </div>
      @if (projects.isLoading() && !projects.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @for (group of groups(); track group.title) {
        <h2>{{ group.title }}</h2>
        @for (p of group.projects; track p.id) {
          <a class="card project" [routerLink]="['/budget/projets', p.id]">
            <div class="head">
              <strong>{{ p.name }}</strong>
              <span class="amount"
                >{{ p.actual_minor | money }} / {{ p.amount_minor | money }}</span
              >
            </div>
            <app-budget-gauge
              [actual]="p.actual_minor"
              [available]="p.amount_minor"
              [alert]="p.alert"
              [label]="'Consommation de ' + p.name"
            />
            <span class="muted small">
              {{ p.start_date | frDate }}
              @if (p.end_date) {
                → {{ p.end_date | frDate }}
              }
            </span>
          </a>
        }
      } @empty {
        @if (projects.hasValue()) {
          <div class="empty card">
            <p>Aucun projet.</p>
            <p class="muted">
              Un projet suit un budget ponctuel (vacances, travaux) sur plusieurs catégories et
              plusieurs mois.
            </p>
          </div>
        }
      }
    </div>
  `,
  styles: `
    h2 {
      font: var(--mat-sys-title-medium);
    }
    .project {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
      margin-bottom: 0.75rem;
      color: inherit;
      text-decoration: none;
    }
    .head {
      display: flex;
      justify-content: space-between;
      gap: 1rem;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class ProjectsPage {
  private readonly api = inject(BudgetsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);

  protected readonly projects = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listProjects(),
  });

  protected readonly groups = computed(() => {
    const all = this.projects.value() ?? [];
    return [
      { title: 'En cours', projects: all.filter((p) => p.active) },
      { title: 'Terminés', projects: all.filter((p) => !p.active) },
    ].filter((g) => g.projects.length > 0);
  });

  protected async create(): Promise<void> {
    const ref = this.dialog.open(ProjectDialog, { data: null });
    const id = await firstValueFrom(ref.afterClosed());
    if (typeof id === 'string') {
      this.version.bump();
      await this.router.navigate(['/budget/projets', id]);
    }
  }
}

/** E20 — Détail d'un projet : réel par catégorie, ventilations rattachées (R-BUD-22, R-BUD-23). */
@Component({
  selector: 'app-project',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MoneyPipe,
    FrDatePipe,
    BudgetGauge,
    DocumentSection,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      @if (project.value(); as p) {
        <div class="page-header">
          <a mat-icon-button routerLink="/budget/projets" aria-label="Retour aux projets">
            <mat-icon>arrow_back</mat-icon>
          </a>
          <h1>{{ p.name }}</h1>
          @if (!p.active) {
            <span class="tag">terminé</span>
          }
          <button mat-button (click)="edit(p)"><mat-icon>edit</mat-icon> Modifier</button>
          @if (p.splits.length === 0) {
            <button mat-button class="danger" (click)="remove(p)">
              <mat-icon>delete</mat-icon> Supprimer
            </button>
          }
        </div>
        <section class="card">
          <div class="head">
            <span class="amount big">{{ p.actual_minor | money }}</span>
            <span class="muted">sur {{ p.amount_minor | money }} prévus</span>
          </div>
          <app-budget-gauge
            [actual]="p.actual_minor"
            [available]="p.amount_minor"
            [alert]="p.alert"
          />
          <p class="muted small">
            Du {{ p.start_date | frDate }}
            @if (p.end_date) {
              au {{ p.end_date | frDate }}
            }
            ·
            {{
              p.exclude_from_period_budgets
                ? 'retiré des lignes du mois'
                : 'compté aussi dans les lignes du mois'
            }}
          </p>
          @if (p.note) {
            <p>{{ p.note }}</p>
          }
        </section>

        @if (p.lines.length > 0) {
          <section class="card">
            <h2>Postes</h2>
            @for (l of p.lines; track l.id) {
              <div class="post">
                <div class="post-head">
                  <span>{{ l.name }}</span>
                  <span class="amount"
                    >{{ l.actual_minor | money }} / {{ l.amount_minor | money }}</span
                  >
                </div>
                <app-budget-gauge
                  [actual]="l.actual_minor"
                  [available]="l.amount_minor"
                  [alert]="l.alert"
                  [label]="'Consommation du poste ' + l.name"
                />
                @if (l.category_ids.length > 0) {
                  <span class="muted small">{{ categoryNames(l.category_ids) }}</span>
                }
              </div>
            }
            @if (p.unassigned_minor !== 0) {
              <div class="row">
                <span>Hors postes</span>
                <span class="amount">{{ p.unassigned_minor | money }}</span>
              </div>
            }
          </section>
        }

        @if (p.by_category.length > 0) {
          <section class="card">
            <h2>Par catégorie</h2>
            @for (c of p.by_category; track c.category_id ?? 'none') {
              <div class="row">
                <span>{{ c.name }}</span>
                <span class="amount">{{ c.amount_minor | money }}</span>
              </div>
            }
          </section>
        }

        <section class="card">
          <h2>Transactions rattachées</h2>
          @for (s of p.splits; track $index) {
            <a class="row link" [routerLink]="['/transactions', s.transaction_id]">
              <span>
                <span class="muted small"
                  >{{ s.date | frDate: 'short' }} · {{ s.account_name }}</span
                >
                <br />
                {{ s.label }}
                @if (p.lines.length > 0) {
                  <span class="muted small">· {{ lineName(p, s.project_line_id) }}</span>
                }
                @if (s.out_of_range) {
                  <mat-icon class="warn" aria-label="Hors des dates du projet">event_busy</mat-icon>
                }
              </span>
              <span class="amount" [class.negative]="s.amount_minor < 0">{{
                s.amount_minor | money
              }}</span>
            </a>
          } @empty {
            <p class="muted">
              Aucune transaction. Rattache-en depuis la liste des transactions (sélection, puis «
              Projet »), depuis le détail d'une transaction, ou par une règle.
            </p>
          }
        </section>
        <app-document-section [target]="{ kind: 'PROJECT', id: p.id }" />
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    h2 {
      font: var(--mat-sys-title-medium);
      margin: 0 0 0.5rem;
    }
    .head {
      display: flex;
      align-items: baseline;
      gap: 0.5rem;
      margin-bottom: 0.5rem;
    }
    .big {
      font: var(--mat-sys-headline-small);
    }
    .row {
      display: flex;
      justify-content: space-between;
      gap: 1rem;
      padding: 0.5rem 0;
      border-top: 1px solid var(--mat-sys-outline-variant);
      color: inherit;
      text-decoration: none;
    }
    .post {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      padding: 0.5rem 0;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .post-head {
      display: flex;
      justify-content: space-between;
      gap: 1rem;
    }
    .link:hover {
      background: var(--mat-sys-surface-container);
    }
    .warn {
      font-size: 1rem;
      width: 1rem;
      height: 1rem;
      vertical-align: middle;
      color: var(--mat-sys-tertiary);
    }
    .tag {
      font: var(--mat-sys-label-small);
      background: var(--mat-sys-secondary-container);
      border-radius: 0.5rem;
      padding: 0.125rem 0.5rem;
    }
    .danger {
      color: var(--mat-sys-error);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class ProjectPage {
  private readonly api = inject(BudgetsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly documents = inject(DocumentsService);
  private readonly router = inject(Router);
  private readonly categories = inject(CategoriesStore);

  readonly id = input.required<string>();

  protected categoryNames(ids: string[]): string {
    return ids.map((id) => this.categories.label(id)).join(', ');
  }

  /** Poste d'une ventilation du projet, ou « hors postes » (R-BUD-27). */
  protected lineName(project: ProjectView, id: string | null | undefined): string {
    return project.lines.find((l) => l.id === id)?.name ?? 'hors postes';
  }

  protected readonly project = rxResource({
    params: () => ({ id: this.id(), version: this.version.value() }),
    stream: ({ params }) => this.api.getProject({ id: params.id }),
  });

  protected edit(project: ProjectView): void {
    void openAndRefresh(this.dialog, this.version, ProjectDialog, project);
  }

  protected async remove(project: ProjectView): Promise<void> {
    const deletion = await confirmDeletion(this.dialog, this.documents, project.id, {
      title: `Supprimer « ${project.name} » ?`,
      message: 'Le projet n’a aucune transaction rattachée.',
      action: 'Supprimer',
      destructive: true,
      objectName: 'ce projet',
    });
    if (deletion) {
      await firstValueFrom(
        this.api.deleteProject({ id: project.id, delete_documents: deletion.deleteDocuments }),
      );
      this.version.bump();
      await this.router.navigate(['/budget/projets']);
    }
  }
}
