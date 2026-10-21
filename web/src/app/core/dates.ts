/** Dates calendaires (ADR-0012) : l'API échange des chaînes `AAAA-MM-JJ`, sans heure ni fuseau. */

/** Date locale → `AAAA-MM-JJ`. */
export function toIsoDate(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, '0');
  const d = String(date.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

/** `AAAA-MM-JJ` → date locale à minuit. */
export function fromIsoDate(iso: string): Date {
  const [y, m, d] = iso.split('-').map(Number);
  return new Date(y, m - 1, d);
}

export function todayIso(): string {
  return toIsoDate(new Date());
}

const long = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' });
const short = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' });
const weekday = new Intl.DateTimeFormat('fr-FR', {
  weekday: 'long',
  day: 'numeric',
  month: 'long',
});
const month = new Intl.DateTimeFormat('fr-FR', { month: 'long', year: 'numeric' });

/** « 12 mars 2026 » (ecrans.md, principes). */
export function formatDate(iso: string): string {
  return long.format(fromIsoDate(iso));
}

/** « 12 mars ». */
export function formatShortDate(iso: string): string {
  return short.format(fromIsoDate(iso));
}

/** « Mercredi 12 mars » : en-tête de jour des listes sur mobile. */
export function formatDayHeader(iso: string): string {
  const text = weekday.format(fromIsoDate(iso));
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** « Mars 2026 ». */
export function formatMonth(iso: string): string {
  const text = month.format(fromIsoDate(iso));
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Mois en cours, `AAAA-MM`. */
export function currentMonth(): string {
  return todayIso().slice(0, 7);
}

/** Mois `AAAA-MM` décalé de `delta` mois. */
export function shiftMonth(value: string, delta: number): string {
  const [y, m] = value.split('-').map(Number);
  const date = new Date(y, m - 1 + delta, 1);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`;
}

/** `AAAA-MM` → « Mars 2026 ». */
export function formatMonthKey(value: string): string {
  return formatMonth(`${value}-01`);
}
