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
import { MatChipsModule } from '@angular/material/chips';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatPaginatorModule } from '@angular/material/paginator';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { RouterLink } from '@angular/router';
import { firstValueFrom, switchMap, timer } from 'rxjs';

import { RefundCandidate } from '../../api/models/refund-candidate';
import { RefundCandidatePage } from '../../api/models/refund-candidate-page';
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

/** Une dépense cochée, et le montant saisi : gardée d'une page ou d'une recherche à l'autre. */
interface Chosen {
  candidate: RefundCandidate;
  amount: string;
}

/** Dépenses par page dans le dialogue. */
const PAGE_SIZE = 10;

/** Ventilation renvoyée telle quelle à l'API, avec son lien de remboursement. */
function kept(s: Split, amount = s.amount_minor): SplitInput {
  return {
    amount_minor: amount,
    category_id: s.category_id ?? null,
    note: s.note ?? null,
    project_id: s.project_id ?? null,
    project_line_id: s.project_line_id ?? null,
    loan_id: s.loan_id ?? null,
    asset_id: s.asset_id ?? null,
    refunds_transaction_id: s.refunds_transaction_id ?? null,
  };
}

/**
 * « Lier une dépense » : toutes les dépenses qui peuvent être remboursées, dans l'ordre de R-TRX-54, par pages de
 * dix, et une recherche par libellé, tiers ou montant. Les dépenses cochées le restent d'une page à l'autre.
 */
@Component({
  selector: 'app-refund-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatChipsModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatPaginatorModule,
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
      <mat-form-field class="search" subscriptSizing="dynamic">
        <mat-label>Rechercher une dépense</mat-label>
        <mat-icon matPrefix>search</mat-icon>
        <input
          matInput
          [ngModel]="query()"
          (ngModelChange)="query.set($event)"
          placeholder="Libellé, tiers ou montant"
          cdkFocusInitial
        />
      </mat-form-field>
      @if (chosenList().length > 0) {
        <mat-chip-set class="chosen" aria-label="Dépenses cochées">
          @for (c of chosenList(); track c.candidate.transaction_id) {
            <mat-chip (removed)="toggle(c.candidate, false)">
              {{ c.candidate.label }} · {{ c.amount }} €
              <button matChipRemove [attr.aria-label]="'Décocher ' + c.candidate.label">
                <mat-icon>cancel</mat-icon>
              </button>
            </mat-chip>
          }
        </mat-chip-set>
      }
      @if (candidates.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @for (candidate of items(); track candidate.transaction_id) {
        @let choice = chosen().get(candidate.transaction_id);
        <div class="row" [class.disabled]="!candidate.category_id">
          <mat-checkbox
            [checked]="!!choice"
            [disabled]="!candidate.category_id"
            (change)="toggle(candidate, $event.checked)"
            [aria-label]="'Rembourse ' + candidate.label"
          />
          <div class="what">
            <span>{{ candidate.label }}</span>
            <span class="muted small">
              {{ candidate.date | frDate: 'short' }} · {{ candidate.account_name }} ·
              {{ candidate.amount_minor | money: data.currency }}
              @if (candidate.pending_minor > 0) {
                · <strong>{{ candidate.pending_minor | money: data.currency }} attendus</strong>
              } @else if (candidate.refunded_minor > 0) {
                · {{ candidate.refunded_minor | money: data.currency }} déjà remboursés
              }
              @if (!candidate.category_id) {
                · à catégoriser d'abord
              }
            </span>
          </div>
          <mat-form-field class="amount-field" subscriptSizing="dynamic">
            <input
              matInput
              inputmode="decimal"
              [attr.aria-label]="'Montant remboursé de ' + candidate.label"
              [disabled]="!choice"
              [ngModel]="choice?.amount ?? proposed(candidate)"
              (ngModelChange)="setAmount(candidate, $event)"
            />
            <span matTextSuffix>€</span>
          </mat-form-field>
        </div>
      } @empty {
        @if (!candidates.isLoading()) {
          <p class="muted">
            {{ query().trim() ? 'Aucune dépense ne correspond.' : 'Aucune dépense à rembourser.' }}
          </p>
        }
      }
      @if (page(); as p) {
        @if (p.total > p.page_size) {
          <mat-paginator
            [length]="p.total"
            [pageIndex]="p.page - 1"
            [pageSize]="p.page_size"
            [hidePageSize]="true"
            (page)="pageIndex.set($event.pageIndex)"
          />
        }
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid()" (click)="save()">Ventiler</button>
    </mat-dialog-actions>
  `,
  styles: `
    .search {
      width: 100%;
      margin-bottom: 0.5rem;
    }
    .chosen {
      display: block;
      margin-bottom: 0.5rem;
    }
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

  protected readonly query = signal('');
  /** Page affichée, à partir de 0 ; revient à la première à chaque recherche. */
  protected readonly pageIndex = linkedSignal({ source: this.query, computation: () => 0 });

  /** Page de dépenses, demandée au serveur après une courte pause de saisie. */
  protected readonly candidates = rxResource({
    params: () => ({ q: this.query().trim(), page: this.pageIndex() + 1 }),
    stream: ({ params }) =>
      timer(params.q ? 300 : 0).pipe(
        switchMap(() =>
          this.api.refundCandidates({
            id: this.data.transactionId,
            q: params.q || undefined,
            page: params.page,
            limit: PAGE_SIZE,
          }),
        ),
      ),
  });
  /** La page affichée reste visible pendant le chargement de la suivante. */
  protected readonly page = linkedSignal<
    RefundCandidatePage | undefined,
    RefundCandidatePage | undefined
  >({
    source: () => (this.candidates.hasValue() ? this.candidates.value() : undefined),
    computation: (next, previous) => next ?? previous?.value,
  });
  protected readonly items = computed(() => this.page()?.items ?? []);

  protected readonly chosen = signal<Map<string, Chosen>>(new Map());
  protected readonly chosenList = computed(() => [...this.chosen().values()]);

  private readonly amounts = computed(() =>
    this.chosenList().map((c) => ({ c, amount: parseMoney(c.amount, this.data.currency) })),
  );
  protected readonly left = computed(() =>
    subtract(this.data.available, sum(this.amounts().map((a) => a.amount ?? 0))),
  );
  protected readonly valid = computed(
    () =>
      this.amounts().length > 0 &&
      this.left() >= 0 &&
      this.amounts().every(
        (a) => a.amount !== null && a.amount > 0 && a.amount <= a.c.candidate.refundable_minor,
      ),
  );

  protected proposed(candidate: RefundCandidate): string {
    return toEditable(candidate.proposed_minor, this.data.currency);
  }

  protected toggle(candidate: RefundCandidate, checked: boolean): void {
    this.chosen.update((map) => {
      const next = new Map(map);
      if (checked) {
        next.set(candidate.transaction_id, { candidate, amount: this.proposed(candidate) });
      } else {
        next.delete(candidate.transaction_id);
      }
      return next;
    });
  }

  protected setAmount(candidate: RefundCandidate, amount: string): void {
    this.chosen.update((map) => {
      const current = map.get(candidate.transaction_id);
      if (!current) return map;
      return new Map(map).set(candidate.transaction_id, { ...current, amount });
    });
  }

  protected save(): void {
    this.ref.close(
      this.amounts().map((a) => ({
        transaction_id: a.c.candidate.transaction_id,
        amount_minor: a.amount ?? 0,
        category_id: a.c.candidate.category_id ?? '',
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
