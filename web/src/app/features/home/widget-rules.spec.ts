import { Account } from '../../api/models/account';
import { BatchSummary } from '../../api/models/batch-summary';
import { LoanSummary } from '../../api/models/loan-summary';
import { MonthFlows } from '../../api/models/month-flows';
import { OccurrenceView } from '../../api/models/occurrence-view';
import { ProjectView } from '../../api/models/project-view';
import {
  cashAccount,
  freshness,
  nextPlannedSpend,
  provisionOf,
  upcomingItems,
  yearTotals,
} from './widget-rules';

const account = (id: string, kind: Account['kind'], order: number, closed?: string) =>
  ({ id, name: id, kind, display_order: order, closed_on: closed ?? null }) as Account;

describe('encadrés de l’accueil', () => {
  it('R-TDB-15 : la trésorerie suit le premier compte courant ouvert', () => {
    const accounts = [
      account('livret', 'LIVRET_A', 0),
      account('ancien', 'CHECKING', 0, '2026-01-31'),
      account('joint', 'CHECKING', 2),
      account('perso', 'CHECKING', 1),
    ];
    expect(cashAccount(accounts)?.id).toBe('perso');
    expect(cashAccount([account('livret', 'LIVRET_A', 0)])).toBeNull();
  });

  it('R-TDB-16 : les échéances des 15 prochains jours, récurrences et prêts, par date', () => {
    const occurrence = (date: string, status: OccurrenceView['status']) =>
      ({
        date,
        status,
        label: `Loyer ${date}`,
        account_id: 'perso',
        amount_minor: -89_000,
      }) as OccurrenceView;
    const loan = {
      id: 'pret',
      name: 'Prêt immo',
      next_installment: { date: '2026-11-05', number: 12, total_minor: 95_000 },
    } as LoanSummary;
    const items = upcomingItems(
      [
        occurrence('2026-11-03', 'UPCOMING'),
        occurrence('2026-10-30', 'MISSED'),
        occurrence('2026-11-20', 'UPCOMING'),
      ],
      [loan],
      (id) => id,
      '2026-10-31',
    );
    expect(items.map((i) => [i.date, i.amount_minor])).toEqual([
      ['2026-11-03', -89_000],
      ['2026-11-05', -95_000],
    ]);
    expect(items[1].label).toBe('Échéance n° 12');
  });

  it('R-TDB-17 : le réel d’une année civile complète, sinon rien', () => {
    const month = (m: string, income: number, expenses: number) =>
      ({
        month: m,
        income_minor: income,
        expenses_minor: expenses,
        savings_minor: income - expenses,
      }) as MonthFlows;
    const months = Array.from({ length: 12 }, (_, i) =>
      month(`2025-${String(i + 1).padStart(2, '0')}`, 300_000, 200_000),
    );
    expect(yearTotals([...months, month('2026-01', 1, 1)], 2025)).toEqual({
      income_minor: 3_600_000,
      expenses_minor: 2_400_000,
      savings_minor: 1_200_000,
    });
    expect(yearTotals(months.slice(1), 2025)).toBeNull();
  });

  it('R-TDB-18 : prochaine dépense prévue et mise de côté du mois', () => {
    const project = {
      schedule: [
        { month: '2026-09', planned_minor: 10_000, actual_minor: 0 },
        { month: '2027-02', planned_minor: 50_000, actual_minor: 0 },
      ],
      provision: [{ month: '2026-10', amount_minor: 18_402 }],
    } as unknown as ProjectView;
    expect(nextPlannedSpend(project, '2026-10')).toEqual({
      month: '2027-02',
      amount_minor: 50_000,
    });
    expect(nextPlannedSpend(project, '2027-03')).toBeNull();
    expect(provisionOf(project, '2026-10')).toBe(18_402);
    expect(provisionOf(project, '2026-11')).toBeNull();
  });

  it('R-TDB-23 : un compte dont le dernier relevé a plus de 31 jours est à mettre à jour', () => {
    const batch = (account_id: string, period_end: string | null, status: BatchSummary['status']) =>
      ({
        account_id,
        period_end,
        status,
        committed_at: '2026-10-02T08:00:00Z',
        created_at: '2026-10-01T08:00:00Z',
      }) as BatchSummary;
    const rows = freshness(
      [
        batch('perso', '2026-10-15', 'COMMITTED'),
        batch('livret', '2026-09-20', 'COMMITTED'),
        batch('livret', '2026-10-25', 'CANCELLED'),
        batch('joint', null, 'COMMITTED'),
        batch('ancien', '2026-01-31', 'COMMITTED'),
      ],
      [
        {
          account_id: 'perso',
          account_name: 'perso',
          old_pending: 0,
          last_reconciled: '2026-10-15',
        },
      ],
      [
        account('perso', 'CHECKING', 0),
        account('livret', 'LIVRET_A', 0),
        account('joint', 'CHECKING', 1),
        account('ancien', 'CHECKING', 2, '2026-02-01'),
      ],
      '2026-10-31',
    );
    expect(rows.map((r) => [r.account_id, r.last_statement, r.stale])).toEqual([
      ['livret', '2026-09-20', true],
      ['joint', '2026-10-02', false],
      ['perso', '2026-10-15', false],
    ]);
    expect(rows[2].last_reconciled).toBe('2026-10-15');
  });
});
