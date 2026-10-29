import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatAutocompleteModule } from '@angular/material/autocomplete';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { debounceTime, firstValueFrom, of, switchMap } from 'rxjs';

import { DocumentView } from '../../api/models/document-view';
import { LinkKind } from '../../api/models/link-kind';
import { LinkTarget } from '../../api/models/link-target';
import { AssetsService } from '../../api/services/assets.service';
import { DocumentsService } from '../../api/services/documents.service';
import { LoansService } from '../../api/services/loans.service';
import { SalariesService } from '../../api/services/salaries.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { formatMoney } from '../../core/money';
import { errorMessage } from '../../core/http-errors';
import { FrDatePipe } from '../../shared/date.pipe';
import { AccountsStore, InstitutionsStore, PayeesStore, ProjectsStore } from '../../shared/stores';
import { KIND_ICONS, KIND_LABELS, LINK_KINDS, LINK_LABELS } from './document-labels';

interface Candidate {
  id: string;
  label: string;
  /** Détail affiché sous le nom : date et montant d'une transaction. */
  detail: string | null;
}

/** Choix d'un objet à rattacher : sa nature, puis l'objet (DOC-02, E35 « Rattacher à… »). */
@Component({
  selector: 'app-link-target-dialog',
  imports: [
    FormsModule,
    MatAutocompleteModule,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Rattacher à…</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Nature</mat-label>
        <mat-select [ngModel]="kind()" (ngModelChange)="choose($event)">
          @for (k of kinds; track k) {
            <mat-option [value]="k">{{ labels[k] }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <mat-form-field>
        <mat-label>{{ labels[kind()] }}</mat-label>
        <input
          matInput
          [matAutocomplete]="auto"
          [ngModel]="query()"
          (ngModelChange)="type($event)"
        />
        <mat-autocomplete
          #auto
          [displayWith]="display"
          (optionSelected)="selected.set($event.option.value)"
        >
          @for (c of shown(); track c.id) {
            <mat-option [value]="c">
              {{ c.label }}
              @if (c.detail) {
                <span class="muted small"> · {{ c.detail }}</span>
              }
            </mat-option>
          }
        </mat-autocomplete>
        @if (kind() === 'TRANSACTION') {
          <mat-hint>Cherchez par libellé.</mat-hint>
        }
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!selected()" (click)="done()">Rattacher</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(28rem, 80vw);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class LinkTargetDialog {
  private readonly ref = inject(MatDialogRef<LinkTargetDialog, LinkTarget>);
  private readonly accounts = inject(AccountsStore);
  private readonly institutions = inject(InstitutionsStore);
  private readonly payees = inject(PayeesStore);
  private readonly projects = inject(ProjectsStore);
  private readonly salaries = inject(SalariesService);
  private readonly assets = inject(AssetsService);
  private readonly loans = inject(LoansService);
  private readonly transactions = inject(TransactionsService);

  protected readonly kinds = LINK_KINDS;
  protected readonly labels = LINK_LABELS;
  protected readonly kind = signal<LinkKind>('ACCOUNT');
  protected readonly query = signal<string | Candidate>('');
  protected readonly selected = signal<Candidate | null>(null);
  protected readonly display = (c: Candidate | string | null): string =>
    typeof c === 'string' ? c : (c?.label ?? '');

  private readonly remote = rxResource({
    params: () => this.kind(),
    stream: ({ params }) => {
      switch (params) {
        case 'EMPLOYMENT':
          return this.salaries
            .salariesOverview()
            .pipe(
              switchMap((o) =>
                of(o.employments.map((e) => ({ id: e.id, label: e.employer, detail: null }))),
              ),
            );
        case 'ASSET':
          return this.assets
            .listAssets()
            .pipe(switchMap((l) => of(l.map((a) => ({ id: a.id, label: a.name, detail: null })))));
        case 'LOAN':
          return this.loans
            .listLoans()
            .pipe(switchMap((l) => of(l.map((a) => ({ id: a.id, label: a.name, detail: null })))));
        default:
          return of([] as Candidate[]);
      }
    },
  });

  /** Transactions cherchées par libellé, les plus récentes d'abord. */
  private readonly found = toSignal(
    toObservable(
      computed(() => {
        const query = this.query();
        return this.kind() === 'TRANSACTION' && typeof query === 'string' ? query : null;
      }),
    ).pipe(
      debounceTime(250),
      switchMap((text) =>
        text && text.trim().length >= 2
          ? this.transactions.listAllTransactions({ text: text.trim(), limit: 20 })
          : of(null),
      ),
    ),
    { initialValue: null },
  );

  private readonly candidates = computed<Candidate[]>(() => {
    switch (this.kind()) {
      case 'ACCOUNT':
        return this.accounts.list().map((a) => ({ id: a.id, label: a.name, detail: null }));
      case 'INSTITUTION':
        return this.institutions.list().map((i) => ({ id: i.id, label: i.name, detail: null }));
      case 'PAYEE':
        return this.payees.list().map((p) => ({ id: p.id, label: p.name, detail: null }));
      case 'PROJECT':
        return this.projects.list().map((p) => ({ id: p.id, label: p.name, detail: null }));
      case 'TRANSACTION':
        return (this.found()?.items ?? []).map((t) => ({
          id: t.id,
          label: t.label,
          detail: `${t.date.split('-').reverse().join('/')} · ${formatMoney(t.amount_minor)}`,
        }));
      default:
        return this.remote.value() ?? [];
    }
  });

  protected readonly shown = computed(() => {
    const query = this.query();
    if (typeof query !== 'string' || this.kind() === 'TRANSACTION') return this.candidates();
    const text = query.trim().toLowerCase();
    return this.candidates()
      .filter((c) => c.label.toLowerCase().includes(text))
      .slice(0, 50);
  });

  /** Le texte saisi annule le choix ; une option choisie arrive comme objet. */
  protected type(value: string | Candidate): void {
    this.query.set(value);
    if (typeof value === 'string') this.selected.set(null);
  }

  protected choose(kind: LinkKind): void {
    this.kind.set(kind);
    this.query.set('');
    this.selected.set(null);
  }

  protected done(): void {
    const selected = this.selected();
    if (selected) this.ref.close({ kind: this.kind(), id: selected.id });
  }
}

/** Ouvre le choix d'un objet à rattacher ; résout à `undefined` si l'utilisateur renonce. */
export async function pickLinkTarget(dialog: MatDialog): Promise<LinkTarget | undefined> {
  const ref = dialog.open<LinkTargetDialog, void, LinkTarget>(LinkTargetDialog, {
    maxWidth: '100vw',
  });
  return firstValueFrom(ref.afterClosed());
}

/** Rattachement de documents existants à un objet, choisis par la recherche (R-DOC-16). */
@Component({
  selector: 'app-attach-documents-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDialogModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ pick ? 'Choisir des documents' : 'Rattacher des documents' }}</h2>
    <mat-dialog-content class="form">
      <mat-form-field subscriptSizing="dynamic">
        <mat-icon matPrefix>search</mat-icon>
        <mat-label>Rechercher</mat-label>
        <input matInput [ngModel]="query()" (ngModelChange)="query.set($event)" />
      </mat-form-field>
      <div class="list">
        @for (d of candidates(); track d.id) {
          <mat-checkbox [checked]="chosen().has(d.id)" (change)="toggle(d.id)">
            <span class="line">
              <mat-icon>{{ d.kind ? icons[d.kind] : 'inbox' }}</mat-icon>
              <span>
                {{ d.title }}
                <span class="muted small">
                  {{ d.kind ? kindLabels[d.kind] : 'À classer' }} ·
                  {{ d.reference_date | frDate }}
                </span>
              </span>
            </span>
          </mat-checkbox>
        } @empty {
          <p class="muted">Aucun document ne correspond.</p>
        }
      </div>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="chosen().size === 0 || busy()" (click)="attach()">
        {{ pick ? 'Choisir' : 'Rattacher' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(32rem, 80vw);
    }
    .list {
      display: flex;
      flex-direction: column;
      max-height: 50vh;
      overflow-y: auto;
    }
    .line {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class AttachDocumentsDialog {
  /** Rattacher à un objet, ou seulement choisir des documents (`null`) : leurs identifiants sont renvoyés. */
  private readonly target = inject<LinkTarget | null>(MAT_DIALOG_DATA);
  protected readonly pick = this.target === null;
  private readonly api = inject(DocumentsService);
  private readonly ref = inject(MatDialogRef<AttachDocumentsDialog, boolean | string[]>);

  protected readonly icons = KIND_ICONS;
  protected readonly kindLabels = KIND_LABELS;
  protected readonly query = signal('');
  protected readonly chosen = signal<ReadonlySet<string>>(new Set());
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  private readonly results = toSignal(
    toObservable(this.query).pipe(
      debounceTime(250),
      switchMap((q) => this.api.listDocuments({ q: q.trim() || undefined })),
    ),
    { initialValue: [] as DocumentView[] },
  );

  /** Les documents pas encore rattachés à l'objet. */
  protected readonly candidates = computed(() =>
    this.results()
      .filter((d) => !d.links.some((l) => l.id === this.target?.id))
      .slice(0, 100),
  );

  protected toggle(id: string): void {
    this.chosen.update((chosen) => {
      const next = new Set(chosen);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  protected async attach(): Promise<void> {
    if (!this.target) {
      this.ref.close([...this.chosen()]);
      return;
    }
    this.busy.set(true);
    try {
      for (const id of this.chosen()) {
        await firstValueFrom(this.api.linkDocument({ id, body: this.target }));
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.busy.set(false);
    }
  }
}

/** Ouvre le rattachement de documents existants ; résout à `true` si des documents ont été rattachés. */
export async function attachDocuments(dialog: MatDialog, target: LinkTarget): Promise<boolean> {
  const ref = dialog.open<AttachDocumentsDialog, LinkTarget | null, boolean | string[]>(
    AttachDocumentsDialog,
    {
      data: target,
      maxWidth: '100vw',
    },
  );
  return (await firstValueFrom(ref.afterClosed())) === true;
}

/** Choix de documents de la bibliothèque ; résout à leurs identifiants, vide si l'utilisateur renonce. */
export async function pickDocuments(dialog: MatDialog): Promise<string[]> {
  const ref = dialog.open<AttachDocumentsDialog, LinkTarget | null, boolean | string[]>(
    AttachDocumentsDialog,
    {
      data: null,
      maxWidth: '100vw',
    },
  );
  const result = await firstValueFrom(ref.afterClosed());
  return Array.isArray(result) ? result : [];
}
