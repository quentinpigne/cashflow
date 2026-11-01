import { TransactionListItem } from '../api/models/transaction-list-item';
import { RefundBadge } from '../api/models/refund-badge';
import { refundMark } from './refund-mark';

function item(amount: number, refund: RefundBadge | null): TransactionListItem {
  return {
    id: 't',
    account_id: 'a',
    account_name: 'Courant',
    amount_minor: amount,
    balance_date: '2026-03-28',
    clearing_status: 'CLEARED',
    date: '2026-03-28',
    has_documents: false,
    label: 'Note de frais',
    origin: 'MANUAL',
    splits: [],
    refund,
  } as TransactionListItem;
}

const none = { refunds: [], refunded_minor: 0, pending_minor: 0, fully_refunded: false };

describe('refundMark (TRX-12)', () => {
  it('cite les dépenses que rembourse une entrée (R-TRX-50)', () => {
    const refunds = [{ transaction_id: 'd', date: '2026-03-02', label: 'Restaurant' }];
    expect(refundMark(item(4_000, { ...none, refunds }))?.text).toBe(
      'Rembourse : Restaurant du 2 mars',
    );
  });

  it('dit ce qu’une dépense a reçu et ce qu’elle attend (R-TRX-52, R-TRX-53)', () => {
    expect(
      refundMark(item(-5_200, { ...none, refunded_minor: 5_200, fully_refunded: true }))?.text,
    ).toBe('Remboursée en totalité');
    const waiting = refundMark(
      item(-5_200, { ...none, refunded_minor: 4_000, expected_minor: 5_200, pending_minor: 1_200 }),
    );
    expect(waiting?.waiting).toBe(true);
    expect(waiting?.text).toBe('Remboursement attendu : reste 12,00 € à recevoir (40,00 € reçus)');
    expect(refundMark(item(-5_200, { ...none, refunded_minor: 1_000 }))?.text).toBe(
      'Remboursée : 10,00 € sur 52,00 €',
    );
    expect(refundMark(item(-5_200, null))).toBeNull();
  });
});
