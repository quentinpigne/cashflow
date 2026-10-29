import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  linkedSignal,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
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
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { RefundCandidate } from '../../api/models/refund-candidate';
import { Split } from '../../api/models/split';
import { SplitInput } from '../../api/models/split-input';
import { TransactionDetail } from '../../api/models/transaction-detail';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { abs, parseMoney, subtract, sum, takeFrom, toEditable } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';

/** Une dépense choisie dans le dialogue, et le montant qu'elle reçoit. */
export interface RefundChoice {
  transaction_id: string;
  amount_minor: number;
  category_id: string;
}

interface RefundDialogData {
  transactionId: string;
  currency: string;
  /** Ce que l'entrée n'affecte encore à aucune dépense. */
  available: number;
}

interface Row {
  candidate: RefundCandidate;
  selected: boolean;
  amount: string;
}

/** Ventilation renvoyée telle quelle à l'API, avec son lien de remboursement. */
function kept(s: Split, amount = s.amount_minor): SplitInput {
  return {
    amount_minor: amount,
    category_id: s.category_id ?? null,
    note: s.note ?? null,
    project_id: s.project_id ?? null,
    loan_id: s.loan_id ?? null,
    asset_id: s.asset_id ?? null,
    refunds_transaction_id: s.refunds_transaction_id ?? null,
  };
}

/** « Lier une dépense » : les dépenses proposées pour une entrée d'argent (R-TRX-54). */
@Component({
  selector: 'app-refund-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatInputModule,
    MatProgressBarModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Rembourse une dépense</h2>
    <mat-dialog-content>
      <p class="muted">
        Cocher les dépenses remboursées et le montant de chacune : l'entrée est ventilée en
        conséquence. Reste à affecter :
        <strong [class.error]="left() < 0">{{ left() | money: data.currency }}</strong>
      </p>
      @if (candidates.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @for (row of rows(); track row.candidate.transaction_id; let i = $index) {
        <div class="row" [class.disabled]="!row.candidate.category_id">
          <mat-checkbox
            [checked]="row.selected"
            [disabled]="!row.candidate.category_id"
            (change)="update(i, { selected: $event.checked })"
            [aria-label]="'Rembourse ' + row.candidate.label"
          />
          <div class="what">
            <span>{{ row.candidate.label }}</span>
            <span class="muted small">
              {{ row.candidate.date | frDate: 'short' }} · {{ row.candidate.account_name }} ·
              {{ row.candidate.amount_minor | money: data.currency }}
              @if (row.candidate.pending_minor > 0) {
                · <strong>{{ row.candidate.pending_minor | money: data.currency }} attendus</strong>
              } @else if (row.candidate.refunded_minor > 0) {
                · {{ row.candidate.refunded_minor | money: data.currency }} déjà remboursés
              }
              @if (!row.candidate.category_id) {
                · à catégoriser d'abord
              }
            </span>
          </div>
          <mat-form-field class="amount-field" subscriptSizing="dynamic">
            <input
              matInput
              inputmode="decimal"
              [attr.aria-label]="'Montant remboursé de ' + row.candidate.label"
              [disabled]="!row.selected"
              [ngModel]="row.amount"
              (ngModelChange)="update(i, { amount: $event })"
            />
            <span matTextSuffix>€</span>
          </mat-form-field>
        </div>
      } @empty {
        @if (!candidates.isLoading()) {
          <p class="muted">Aucune dépense à rembourser dans les douze derniers mois.</p>
        }
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid()" (click)="save()">Ventiler</button>
    </mat-dialog-actions>
  `,
  styles: `
    .row {
      display: grid;
      grid-template-columns: auto 1fr 8rem;
      align-items: center;
      gap: 0.5rem;
      padding: 0.25rem 0;
      border-top: 1px solid var(--mat-sys-outline-variant);

      &.disabled {
        opacity: 0.6;
      }
    }
    .what {
      display: flex;
      flex-direction: column;
      min-width: 0;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class RefundDialog {
  protected readonly data = inject<RefundDialogData>(MAT_DIALOG_DATA);
  private readonly api = inject(TransactionsService);
  private readonly ref = inject(MatDialogRef<RefundDialog, RefundChoice[]>);

  protected readonly candidates = rxResource({
    stream: () => this.api.refundCandidates({ id: this.data.transactionId }),
  });
  protected readonly rows = linkedSignal<Row[]>(() =>
    (this.candidates.hasValue() ? this.candidates.value() : []).map((candidate) => ({
      candidate,
      selected: false,
      amount: toEditable(candidate.proposed_minor, this.data.currency),
    })),
  );

  private readonly chosen = computed(() =>
    this.rows()
      .filter((r) => r.selected)
      .map((r) => ({ row: r, amount: parseMoney(r.amount, this.data.currency) })),
  );
  protected readonly left = computed(() =>
    subtract(this.data.available, sum(this.chosen().map((c) => c.amount ?? 0))),
  );
  protected readonly valid = computed(
    () =>
      this.chosen().length > 0 &&
      this.left() >= 0 &&
      this.chosen().every(
        (c) => c.amount !== null && c.amount > 0 && c.amount <= c.row.candidate.refundable_minor,
      ),
  );

  protected update(index: number, patch: Partial<Row>): void {
    this.rows.update((rows) => rows.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  }

  protected save(): void {
    this.ref.close(
      this.chosen().map((c) => ({
        transaction_id: c.row.candidate.transaction_id,
        amount_minor: c.amount ?? 0,
        category_id: c.row.candidate.category_id ?? '',
      })),
    );
  }
}

/**
 * Remboursements de frais d'une transaction (TRX-12) : remboursement attendu et reçus pour une dépense ;
 * dépenses remboursées et « Lier une dépense » pour une entrée d'argent.
 */
@Component({
  selector: 'app-refund-panel',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatTooltipModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @let t = transaction();
    @if (t.amount_minor < 0) {
      <mat-form-field>
        <mat-label>Remboursement attendu</mat-label>
        <input
          matInput
          inputmode="decimal"
          [ngModel]="expected()"
          (ngModelChange)="expected.set($event)"
          (keydown.enter)="saveExpected(t)"
          (blur)="saveExpected(t)"
        />
        <span matTextSuffix>€</span>
        @if (t.refund.expected_minor) {
          <button
            matIconSuffix
            mat-icon-button
            type="button"
            aria-label="Ne viendra pas"
            matTooltip="Ne viendra pas"
            (click)="setExpected(t, null)"
          >
            <mat-icon>close</mat-icon>
          </button>
        }
        @if (t.refund.refunded_minor > 0 || t.refund.expected_minor) {
          <mat-hint>
            {{ t.refund.refunded_minor | money: t.currency }} remboursés sur
            {{ abs(t.amount_minor) | money: t.currency }}
            @if (t.refund.pending_minor > 0) {
              · <strong>{{ t.refund.pending_minor | money: t.currency }} à recevoir</strong>
            }
          </mat-hint>
        }
      </mat-form-field>
      @if (t.refund.received.length > 0) {
        <div class="card">
          <div class="head"><strong>Remboursée par</strong></div>
          @for (r of t.refund.received; track r.split_id) {
            <div class="row">
              <span
                ><a [routerLink]="['/transactions', r.transaction_id]">{{ r.label }}</a>
                <span class="muted"> · {{ r.date | frDate: 'short' }}</span></span
              >
              <span class="amount">{{ r.amount_minor | money: t.currency }}</span>
            </div>
          }
        </div>
      }
    } @else {
      <div class="card">
        <div class="head">
          <strong>Remboursements</strong>
          <button mat-button type="button" [disabled]="available() <= 0" (click)="link(t)">
            Lier une dépense
          </button>
        </div>
        @for (r of t.refund.refunds; track r.split_id) {
          <div class="row">
            <span
              ><a [routerLink]="['/transactions', r.transaction_id]">{{ r.label }}</a>
              <span class="muted">
                · {{ r.date | frDate: 'short' }}, {{ r.amount_minor | money: t.currency }}</span
              ></span
            >
            <span class="end">
              <span class="amount">{{ refundedAmount(r.split_id) | money: t.currency }}</span>
              <button
                mat-icon-button
                type="button"
                class="compact"
                [attr.aria-label]="'Retirer le lien avec ' + r.label"
                matTooltip="Retirer le lien"
                (click)="unlink(t, r.split_id)"
              >
                <mat-icon>link_off</mat-icon>
              </button>
            </span>
          </div>
        } @empty {
          <p class="muted">N'est liée à aucune dépense.</p>
        }
      </div>
    }
    @if (error()) {
      <p class="error" role="alert">{{ error() }}</p>
    }
  `,
  styles: `
    /* Les champs et encadrés prennent place dans le formulaire de la fiche, avec son espacement. */
    :host {
      display: contents;
    }
    /* Comme l'encadré « Ventilation » de la fiche. */
    .head {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 0.5rem;
      min-height: 2.5rem;

      button {
        white-space: nowrap;
      }
    }
    .row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1rem;
      min-height: 2.25rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .end {
      display: flex;
      align-items: center;
      gap: 0.25rem;
      white-space: nowrap;
    }
    .compact {
      --mat-icon-button-state-layer-size: 2rem;
      padding: 0.25rem;

      mat-icon {
        font-size: 1.25rem;
        width: 1.25rem;
        height: 1.25rem;
      }
    }
    p {
      margin: 0.375rem 0 0;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class RefundPanel {
  private readonly api = inject(TransactionsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);

  readonly transaction = input.required<TransactionDetail>();

  protected readonly error = signal<string | null>(null);
  protected readonly expected = linkedSignal(() => {
    const t = this.transaction();
    return t.refund.expected_minor ? toEditable(t.refund.expected_minor, t.currency) : '';
  });
  /** Ce que l'entrée n'affecte encore à aucune dépense. */
  protected readonly available = computed(() =>
    sum(
      this.transaction()
        .splits.filter((s) => !s.refunds_transaction_id)
        .map((s) => s.amount_minor),
    ),
  );

  protected refundedAmount(splitId: string): number {
    return this.transaction().splits.find((s) => s.id === splitId)?.amount_minor ?? 0;
  }

  private async run(action: () => Promise<unknown>): Promise<void> {
    this.error.set(null);
    try {
      await action();
      this.version.bump();
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }

  protected saveExpected(t: TransactionDetail): Promise<void> | void {
    const text = this.expected().trim();
    const value = text === '' ? null : parseMoney(text, t.currency);
    if (text !== '' && value === null) {
      this.error.set('Montant invalide');
      return;
    }
    if (value === (t.refund.expected_minor ?? null)) return;
    return this.setExpected(t, value);
  }

  protected setExpected(t: TransactionDetail, value: number | null): Promise<void> {
    return this.run(() =>
      firstValueFrom(this.api.setExpectedRefund({ id: t.id, body: { expected_minor: value } })),
    );
  }

  private save(t: TransactionDetail, splits: SplitInput[]): Promise<void> {
    return this.run(() =>
      firstValueFrom(
        this.api.updateTransaction({
          id: t.id,
          body: {
            account_id: t.account_id,
            date: t.date,
            amount_minor: t.amount_minor,
            label: t.label,
            note: t.note ?? null,
            payee_id: t.payee_id ?? null,
            splits,
          },
        }),
      ),
    );
  }

  /** Ventile l'entrée : une ventilation par dépense choisie, prise sur ce qui n'était affecté à aucune. */
  protected async link(t: TransactionDetail): Promise<void> {
    const ref = this.dialog.open<RefundDialog, RefundDialogData, RefundChoice[]>(RefundDialog, {
      data: { transactionId: t.id, currency: t.currency, available: this.available() },
      width: '40rem',
      maxWidth: '100vw',
    });
    const chosen = await firstValueFrom(ref.afterClosed());
    if (!chosen?.length) return;
    const linked = t.splits.filter((s) => s.refunds_transaction_id);
    const free = t.splits.filter((s) => !s.refunds_transaction_id);
    const rest = takeFrom(
      free.map((s) => s.amount_minor),
      sum(chosen.map((c) => c.amount_minor)),
    );
    if (!rest) {
      this.error.set(
        "Le montant choisi dépasse ce que l'entrée n'affecte encore à aucune dépense.",
      );
      return;
    }
    await this.save(t, [
      ...free.map((s, i) => kept(s, rest[i])).filter((s) => s.amount_minor !== 0),
      ...linked.map((s) => kept(s)),
      ...chosen.map((c) => ({
        amount_minor: c.amount_minor,
        category_id: c.category_id,
        note: null,
        refunds_transaction_id: c.transaction_id,
      })),
    ]);
  }

  /** Retire le lien : la ventilation reste, avec sa catégorie (R-TRX-55). */
  protected unlink(t: TransactionDetail, splitId: string): Promise<void> {
    return this.save(
      t,
      t.splits.map((s) =>
        s.id === splitId ? { ...kept(s), refunds_transaction_id: null } : kept(s),
      ),
    );
  }

  protected readonly abs = abs;
}
