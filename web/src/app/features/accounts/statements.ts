import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  model,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';

import { StatementView } from '../../api/models/statement-view';
import { AccountsService } from '../../api/services/accounts.service';
import { DataVersion } from '../../core/data-version';
import { currentMonth, shiftMonth } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MONTH_NAMES, formatSize } from '../salaries/salary-dialogs';

/** Un relevé de compte est un PDF de 10 Mo au plus (R-CPT-18). */
const STATEMENT_ACCEPT = '.pdf,application/pdf';
const STATEMENT_MAX_BYTES = 10 * 1024 * 1024;

/** Une ligne de la liste des relevés : un relevé, ou un mois sans relevé (R-CPT-21). */
interface StatementEntry {
  month: string;
  statement: StatementView | null;
}

interface StatementYear {
  year: string;
  count: number;
  entries: StatementEntry[];
}

/** E3 — Relevés d'un compte (CPT-11). */
@Component({
  selector: 'app-account-statements',
  imports: [MatButtonModule, MatIconModule, MatMenuModule, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section
      class="card panel statements"
      [class.dragging]="dragging()"
      (dragover)="dragOver($event)"
      (dragleave)="dragging.set(false)"
      (drop)="drop($event)"
    >
      <header>
        <h2>Relevés</h2>
        <button mat-stroked-button (click)="add(null)"><mat-icon>add</mat-icon>Ajouter</button>
      </header>
      @for (y of years(); track y.year) {
        <details [open]="expanded().has(y.year)" (toggle)="toggle(y.year, $event)">
          <summary>
            {{ y.year }}
            <span class="muted small">{{ y.count }} relevé{{ y.count > 1 ? 's' : '' }}</span>
          </summary>
          @for (e of y.entries; track $index) {
            @if (e.statement; as s) {
              <div class="row">
                <span>{{ e.month | frDate: 'month' }}</span>
                <span class="name">
                  {{ s.file_name }}
                  <span class="muted small">
                    @if (s.period_start && s.period_end) {
                      du {{ s.period_start | frDate }} au {{ s.period_end | frDate }} ·
                    }
                    {{ size(s.size_bytes) }}{{ s.note ? ' · ' + s.note : '' }}
                  </span>
                </span>
                <span class="actions">
                  <a
                    mat-icon-button
                    [href]="fileUrl(s, false)"
                    target="_blank"
                    rel="noopener"
                    [attr.aria-label]="'Ouvrir le relevé ' + s.file_name"
                    ><mat-icon>open_in_new</mat-icon></a
                  >
                  <a
                    mat-icon-button
                    class="wide"
                    [href]="fileUrl(s, true)"
                    download
                    [attr.aria-label]="'Télécharger le relevé ' + s.file_name"
                    ><mat-icon>download</mat-icon></a
                  >
                  <button
                    mat-icon-button
                    [matMenuTriggerFor]="menu"
                    [attr.aria-label]="'Actions sur le relevé ' + s.file_name"
                  >
                    <mat-icon>more_vert</mat-icon>
                  </button>
                </span>
                <mat-menu #menu>
                  <a mat-menu-item [href]="fileUrl(s, true)" download>
                    <mat-icon>download</mat-icon>Télécharger
                  </a>
                  <button mat-menu-item (click)="add(s)"><mat-icon>edit</mat-icon>Modifier</button>
                  <button mat-menu-item (click)="remove(s)">
                    <mat-icon>delete</mat-icon>Supprimer
                  </button>
                </mat-menu>
              </div>
            } @else {
              <div class="row missing">
                <span>{{ e.month | frDate: 'month' }}</span>
                <span class="small">sans relevé</span>
              </div>
            }
          }
        </details>
      } @empty {
        <p class="muted">
          Aucun relevé archivé : déposez un PDF ici, utilisez « Ajouter », ou importez un relevé
          PDF.
        </p>
      }
    </section>
  `,
  styles: `
    .panel {
      margin-top: 1rem;
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .statements {
      outline: 2px dashed transparent;
      outline-offset: -2px;
      &.dragging {
        outline-color: var(--mat-sys-primary);
      }
    }
    summary {
      cursor: pointer;
      padding: 0.5rem 0;
      font: var(--mat-sys-title-small);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .row {
      display: grid;
      grid-template-columns: 9rem 1fr auto;
      align-items: center;
      gap: 0.75rem;
      min-height: 2.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      .name {
        overflow-wrap: anywhere;
      }
      &.missing {
        color: var(--mat-sys-on-surface-variant);
        opacity: 0.7;
      }
      @media (max-width: 599.98px) {
        grid-template-columns: 6.5rem 1fr auto;
        .wide {
          display: none;
        }
      }
    }
  `,
})
export class AccountStatements {
  private readonly api = inject(AccountsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  readonly accountId = input.required<string>();

  private readonly list = rxResource({
    params: () => ({ id: this.accountId(), version: this.version.value() }),
    stream: ({ params }) => this.api.listStatements({ id: params.id }),
  });

  protected readonly size = formatSize;
  protected readonly dragging = signal(false);
  /** Années dépliées : toutes repliées par défaut. */
  readonly expanded = model<ReadonlySet<string>>(new Set());

  /** Relevés et mois sans relevé, par année, du plus récent au plus ancien (R-CPT-21). */
  protected readonly years = computed<StatementYear[]>(() => {
    const list = this.list.value();
    if (!list) return [];
    const entries: StatementEntry[] = [
      ...list.statements.map((s) => ({ month: s.statement_month, statement: s })),
      ...list.missing_months.map((month) => ({ month, statement: null })),
    ].sort((a, b) => b.month.localeCompare(a.month));
    const years: StatementYear[] = [];
    for (const entry of entries) {
      const year = entry.month.slice(0, 4);
      let group = years.at(-1);
      if (group?.year !== year) {
        group = { year, count: 0, entries: [] };
        years.push(group);
      }
      group.entries.push(entry);
      if (entry.statement) group.count++;
    }
    return years;
  });

  protected toggle(year: string, event: Event): void {
    const open = (event.target as HTMLDetailsElement).open;
    this.expanded.update((years) => {
      const next = new Set(years);
      if (open) next.add(year);
      else next.delete(year);
      return next;
    });
  }

  protected fileUrl(statement: StatementView, download: boolean): string {
    const url = `/api/v1/accounts/${this.accountId()}/statements/${statement.id}/file`;
    return download ? `${url}?download=true` : url;
  }

  /** Nouveau relevé (`statement` absent), éventuellement avec un fichier déposé, ou modification (R-CPT-20). */
  protected async add(statement: StatementView | null, file: File | null = null): Promise<void> {
    const ref = this.dialog.open(StatementDialog, {
      data: { accountId: this.accountId(), statement, file },
      maxWidth: '100vw',
    });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected dragOver(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes('Files')) return;
    event.preventDefault();
    this.dragging.set(true);
  }

  protected async drop(event: DragEvent): Promise<void> {
    event.preventDefault();
    this.dragging.set(false);
    const file = event.dataTransfer?.files[0];
    if (file) await this.add(null, file);
  }

  protected async remove(statement: StatementView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le relevé ?',
      message: `« ${statement.file_name} » ne sera plus consultable.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(
        this.api.deleteStatement({ id: this.accountId(), statement_id: statement.id }),
      );
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }
}

export interface StatementDialogData {
  accountId: string;
  /** Relevé modifié : seuls le mois et la note changent (R-CPT-20). */
  statement: StatementView | null;
  /** Fichier déjà choisi, par exemple déposé sur la section. */
  file: File | null;
}

/** Envoi ou modification d'un relevé de compte (CPT-11). */
@Component({
  selector: 'app-statement-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ data.statement ? 'Modifier le relevé' : 'Nouveau relevé' }}</h2>
    <mat-dialog-content class="form">
      @if (data.statement; as s) {
        <p class="file"><mat-icon>description</mat-icon>{{ s.file_name }}</p>
      } @else {
        <label class="drop">
          <input
            type="file"
            [accept]="accept"
            aria-label="Fichier du relevé"
            (change)="choose($event)"
          />
          <mat-icon>upload_file</mat-icon>
          <span>{{ file()?.name ?? 'Choisir un PDF (10 Mo au plus)' }}</span>
        </label>
      }
      <div class="pair">
        <mat-form-field>
          <mat-label>Mois du relevé</mat-label>
          <mat-select [ngModel]="month()" (ngModelChange)="month.set($event)">
            @for (m of months; track $index) {
              <mat-option [value]="$index + 1">{{ m }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Année</mat-label>
          <input
            matInput
            type="number"
            min="1950"
            max="2100"
            [ngModel]="year()"
            (ngModelChange)="year.set($event)"
          />
        </mat-form-field>
      </div>
      <mat-form-field>
        <mat-label>Note</mat-label>
        <input matInput [ngModel]="note()" (ngModelChange)="note.set($event)" />
        <mat-hint>Relevé rectificatif, relevé de clôture…</mat-hint>
      </mat-form-field>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!statementMonth() || busy()" (click)="save()">
        Enregistrer
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(30rem, 80vw);
    }
    .pair {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--field-gap);
      @media (max-width: 599.98px) {
        grid-template-columns: 1fr;
      }
    }
    .error {
      color: var(--mat-sys-error);
      margin: 0;
    }
    .drop {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      padding: 1.25rem;
      border: 2px dashed var(--mat-sys-outline-variant);
      border-radius: var(--mat-sys-corner-medium);
      cursor: pointer;
      input {
        position: absolute;
        width: 1px;
        height: 1px;
        opacity: 0;
      }
    }
    .file {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      margin: 0;
    }
  `,
})
export class StatementDialog {
  protected readonly data = inject<StatementDialogData>(MAT_DIALOG_DATA);
  private readonly api = inject(AccountsService);
  private readonly ref = inject(MatDialogRef<StatementDialog>);

  protected readonly accept = STATEMENT_ACCEPT;
  protected readonly months = MONTH_NAMES;
  /** Le mois précédent par défaut : le relevé arrive après le mois qu'il couvre. */
  private readonly initial =
    this.data.statement?.statement_month ?? `${shiftMonth(currentMonth(), -1)}-01`;
  protected readonly month = signal(Number(this.initial.slice(5, 7)));
  protected readonly year = signal(Number(this.initial.slice(0, 4)));
  protected readonly note = signal(this.data.statement?.note ?? '');
  protected readonly file = signal<File | null>(null);
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  constructor() {
    if (this.data.file) this.setFile(this.data.file);
  }

  protected readonly statementMonth = computed(() => {
    const year = Number(this.year());
    if (!Number.isInteger(year) || year < 1950 || year > 2100) return null;
    if (!this.data.statement && !this.file()) return null;
    return `${year}-${String(this.month()).padStart(2, '0')}-01`;
  });

  protected choose(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file) this.setFile(file);
  }

  private setFile(file: File): void {
    if (file.size > STATEMENT_MAX_BYTES) {
      this.file.set(null);
      this.error.set('Le fichier dépasse 10 Mo.');
      return;
    }
    this.file.set(file);
    this.error.set(null);
  }

  protected async save(): Promise<void> {
    const statementMonth = this.statementMonth();
    if (!statementMonth) return;
    const id = this.data.accountId;
    const note = this.note().trim() || null;
    this.busy.set(true);
    try {
      const file = this.file();
      if (this.data.statement) {
        await firstValueFrom(
          this.api.updateStatement({
            id,
            statement_id: this.data.statement.id,
            body: { statement_month: statementMonth, note },
          }),
        );
      } else if (file) {
        await firstValueFrom(
          this.api.createStatement({
            id,
            body: { file, statement_month: statementMonth, note },
          }),
        );
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.busy.set(false);
    }
  }
}
