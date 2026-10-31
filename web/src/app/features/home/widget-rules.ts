import { Account } from '../../api/models/account';
import { AccountHealth } from '../../api/models/account-health';
import { BatchSummary } from '../../api/models/batch-summary';
import { FlowsView } from '../../api/models/flows-view';
import { LoanSummary } from '../../api/models/loan-summary';
import { MonthFlows } from '../../api/models/month-flows';
import { OccurrenceView } from '../../api/models/occurrence-view';
import { ProjectView } from '../../api/models/project-view';
import { sum } from '../../core/money';

/** Compte de la trésorerie à venir : le premier compte courant ouvert, dans l'ordre des comptes (R-TDB-15). */
export function cashAccount(accounts: Account[]): Account | null {
  return (
    accounts
      .filter((a) => a.kind === 'CHECKING' && !a.closed_on)
      .sort((a, b) => a.display_order - b.display_order)[0] ?? null
  );
}

/** Une échéance des prochains jours : récurrence ou mensualité de prêt (R-TDB-16). */
export interface UpcomingItem {
  date: string;
  label: string;
  /** Compte de la récurrence, ou prêt de la mensualité. */
  source: string;
  amount_minor: number;
  link: string[];
}

/** Échéances des `days` prochains jours, tous comptes confondus, par date, `max` au plus (R-TDB-16). */
export function upcomingItems(
  occurrences: OccurrenceView[],
  loans: LoanSummary[],
  accountName: (id: string) => string,
  today: string,
  days = 15,
  max = 8,
): UpcomingItem[] {
  const last = addDays(today, days);
  const inRange = (date: string) => date >= today && date <= last;
  const items: UpcomingItem[] = [
    ...occurrences
      .filter((o) => o.status === 'UPCOMING' && inRange(o.date))
      .map((o) => ({
        date: o.date,
        label: o.label,
        source: accountName(o.account_id),
        amount_minor: o.amount_minor,
        link: ['/recurrences'],
      })),
    ...loans
      .filter((l) => !l.closed_on && l.next_installment && inRange(l.next_installment.date))
      .map((l) => ({
        date: l.next_installment!.date,
        label: `Échéance n° ${l.next_installment!.number}`,
        source: l.name,
        amount_minor: -l.next_installment!.total_minor,
        link: ['/prets', l.id],
      })),
  ];
  return items.sort((a, b) => a.date.localeCompare(b.date)).slice(0, max);
}

function addDays(iso: string, days: number): string {
  const [y, m, d] = iso.split('-').map(Number);
  const date = new Date(Date.UTC(y, m - 1, d + days));
  return date.toISOString().slice(0, 10);
}

/** Revenus, dépenses et épargne d'une année civile, d'après les mois de l'évolution (R-TDB-17). */
export function yearTotals(
  months: MonthFlows[],
  year: number,
): Pick<FlowsView, 'income_minor' | 'expenses_minor' | 'savings_minor'> | null {
  const of = months.filter((m) => m.month.startsWith(`${year}-`));
  if (of.length < 12) return null;
  return {
    income_minor: sum(of.map((m) => m.income_minor)),
    expenses_minor: sum(of.map((m) => m.expenses_minor)),
    savings_minor: sum(of.map((m) => m.savings_minor)),
  };
}

/** Prochaine dépense prévue d'un projet : le premier mois du calendrier prévu à partir de `month` (R-TDB-18). */
export function nextPlannedSpend(
  project: ProjectView,
  month: string,
): { month: string; amount_minor: number } | null {
  const next = project.schedule.find((m) => m.month >= month && m.planned_minor > 0);
  return next ? { month: next.month, amount_minor: next.planned_minor } : null;
}

/** Mise de côté d'un projet pour `month`, s'il en a une (R-TDB-18). */
export function provisionOf(project: ProjectView, month: string): number | null {
  return project.provision.find((p) => p.month === month)?.amount_minor ?? null;
}

/** Fraîcheur d'un compte importé (R-TDB-23). */
export interface Freshness {
  account_id: string;
  account_name: string;
  /** Date du dernier relevé importé : fin de période du lot, à défaut sa date de validation. */
  last_statement: string;
  last_reconciled: string | null;
  stale: boolean;
}

/** Délai au-delà duquel un compte importé est à mettre à jour (R-TDB-23). */
export const STALE_DAYS = 31;

/**
 * Comptes ouverts qui ont au moins un import validé, les comptes à mettre à jour d'abord, puis par
 * dernier relevé le plus ancien (R-TDB-23).
 */
export function freshness(
  batches: BatchSummary[],
  health: AccountHealth[],
  accounts: Account[],
  today: string,
): Freshness[] {
  const open = new Map(accounts.filter((a) => !a.closed_on).map((a) => [a.id, a.name]));
  const last = new Map<string, string>();
  for (const b of batches.filter((b) => b.status === 'COMMITTED' && open.has(b.account_id))) {
    const date = b.period_end ?? (b.committed_at ?? b.created_at).slice(0, 10);
    if (date > (last.get(b.account_id) ?? '')) last.set(b.account_id, date);
  }
  const limit = addDays(today, -STALE_DAYS);
  return [...last.entries()]
    .map(([id, date]) => ({
      account_id: id,
      account_name: open.get(id) ?? '',
      last_statement: date,
      last_reconciled: health.find((h) => h.account_id === id)?.last_reconciled ?? null,
      stale: date < limit,
    }))
    .sort(
      (a, b) =>
        Number(b.stale) - Number(a.stale) || a.last_statement.localeCompare(b.last_statement),
    );
}
