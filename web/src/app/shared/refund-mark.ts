import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { MatIconModule } from '@angular/material/icon';
import { MatTooltipModule } from '@angular/material/tooltip';

import { TransactionListItem } from '../api/models/transaction-list-item';
import { formatShortDate } from '../core/dates';
import { formatMoney, negate } from '../core/money';

interface RefundMarkView {
  icon: string;
  text: string;
  waiting: boolean;
}

/** Ce que l'icône d'une ligne dit des remboursements de frais (TRX-12) ; `null` s'il n'y en a aucun. */
export function refundMark(item: TransactionListItem): RefundMarkView | null {
  const r = item.refund;
  if (!r) return null;
  if (r.refunds.length > 0) {
    const expenses = r.refunds.map((e) => `${e.label} du ${formatShortDate(e.date)}`).join(', ');
    return { icon: 'reply', text: `Rembourse : ${expenses}`, waiting: false };
  }
  if (r.fully_refunded) {
    return { icon: 'currency_exchange', text: 'Remboursée en totalité', waiting: false };
  }
  const received = formatMoney(r.refunded_minor);
  if (r.pending_minor > 0) {
    const pending = `Remboursement attendu : reste ${formatMoney(r.pending_minor)} à recevoir`;
    const text = r.refunded_minor > 0 ? `${pending} (${received} reçus)` : pending;
    return { icon: 'currency_exchange', text, waiting: true };
  }
  const total = formatMoney(negate(item.amount_minor));
  return {
    icon: 'currency_exchange',
    text: `Remboursée : ${received} sur ${total}`,
    waiting: false,
  };
}

/** Icône des remboursements de frais d'une ligne de liste, détaillés dans son info-bulle (E3, E14, TRX-12). */
@Component({
  selector: 'app-refund-mark',
  imports: [MatIconModule, MatTooltipModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (view(); as v) {
      <mat-icon
        class="refund-mark"
        [class.waiting]="v.waiting"
        [matTooltip]="v.text"
        [attr.aria-label]="v.text"
        >{{ v.icon }}</mat-icon
      >
    }
  `,
  styles: `
    .refund-mark {
      margin-left: 0.25rem;
      font-size: 1rem;
      width: 1rem;
      height: 1rem;
      vertical-align: text-bottom;
      color: var(--mat-sys-on-surface-variant);
    }
    .refund-mark.waiting {
      color: var(--mat-sys-tertiary);
    }
  `,
})
export class RefundMark {
  readonly item = input.required<TransactionListItem>();
  protected readonly view = computed(() => refundMark(this.item()));
}
