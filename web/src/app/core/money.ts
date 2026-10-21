/**
 * Montants en unités mineures (ADR-0001) : analyse des saisies, formatage et répartition.
 *
 * Seul module autorisé à faire de l'arithmétique sur des montants (CLAUDE.md). Aucun calcul ne passe
 * par un flottant : les montants restent des entiers de centimes, manipulés comme tels.
 */

const SPACES = /[\s\u00a0\u202f]/g;

/** Nombre de décimales d'une devise (ISO 4217), d'après Intl. */
export function minorDigits(currency = 'EUR'): number {
  return (
    new Intl.NumberFormat('fr-FR', { style: 'currency', currency }).resolvedOptions()
      .maximumFractionDigits ?? 2
  );
}

const groupFormatter = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 0 });

/** Partie entière groupée par milliers à la française (« 1 234 »), à partir d'un entier. */
function groupInteger(value: number): string {
  return groupFormatter.format(value);
}

function currencySymbol(currency: string): string {
  const part = new Intl.NumberFormat('fr-FR', { style: 'currency', currency })
    .formatToParts(0)
    .find((p) => p.type === 'currency');
  return part?.value ?? currency;
}

export interface FormatOptions {
  currency?: string;
  /** Affiche « + » devant un montant positif (par défaut : non, cf. ecrans.md). */
  signed?: boolean;
  /** Sans symbole de devise. */
  bare?: boolean;
}

/** « 1 234,56 € » à partir de 123456 centimes. */
export function formatMoney(minor: number, options: FormatOptions = {}): string {
  const currency = options.currency ?? 'EUR';
  const digits = minorDigits(currency);
  const negative = minor < 0;
  const absolute = Math.abs(minor);
  const scale = 10 ** digits;
  const integer = Math.trunc(absolute / scale);
  const fraction = absolute % scale;
  let text = groupInteger(integer);
  if (digits > 0) {
    text += ',' + String(fraction).padStart(digits, '0');
  }
  const sign = negative ? '−' : options.signed && minor > 0 ? '+' : '';
  return options.bare ? `${sign}${text}` : `${sign}${text}\u00a0${currencySymbol(currency)}`;
}

/**
 * Analyse une saisie (« 1 234,56 », « 12.5 », « -3 », « 1 234,56 € ») en unités mineures.
 * Renvoie `null` si la saisie n'est pas un montant valide ou a trop de décimales.
 */
export function parseMoney(input: string, currency = 'EUR'): number | null {
  const digits = minorDigits(currency);
  let text = input.replace(SPACES, '').replace(/€|EUR/gi, '').replace('−', '-');
  if (text === '') {
    return null;
  }
  let negative = false;
  if (text.startsWith('-') || text.startsWith('+')) {
    negative = text.startsWith('-');
    text = text.slice(1);
  }
  const match = /^(\d+)(?:[.,](\d*))?$/.exec(text);
  if (!match) {
    return null;
  }
  const [, integer, decimals = ''] = match;
  if (decimals.length > digits) {
    return null;
  }
  const minor = Number(integer + decimals.padEnd(digits, '0'));
  if (!Number.isSafeInteger(minor)) {
    return null;
  }
  return negative ? -minor : minor;
}

/** Montant en texte éditable (« 1234,56 »), sans séparateur de milliers ni symbole. */
export function toEditable(minor: number, currency = 'EUR'): string {
  const digits = minorDigits(currency);
  const scale = 10 ** digits;
  const absolute = Math.abs(minor);
  const integer = Math.trunc(absolute / scale);
  const fraction = absolute % scale;
  const text =
    digits > 0 ? `${integer},${String(fraction).padStart(digits, '0')}` : String(integer);
  return minor < 0 ? `-${text}` : text;
}

/**
 * Pourcentage saisi (« 33,33 ») en centièmes de point (3333), pour la répartition.
 * Renvoie `null` s'il est invalide, négatif ou a plus de deux décimales.
 */
export function parsePercent(input: string): number | null {
  const text = input.replace(SPACES, '').replace('%', '');
  const match = /^(\d+)(?:[.,](\d{0,2}))?$/.exec(text);
  if (!match) {
    return null;
  }
  return Number(match[1] + (match[2] ?? '').padEnd(2, '0'));
}

/**
 * Répartit `totalMinor` selon des poids entiers positifs, l'arrondi allant sur la dernière part
 * (R-TRX-14). Même algorithme que le domaine (`ledger::split::allocate`).
 */
export function allocate(totalMinor: number, weights: number[]): number[] | null {
  const weightSum = weights.reduce((sum, w) => sum + w, 0);
  if (
    weights.length === 0 ||
    weightSum <= 0 ||
    weights.some((w) => w < 0 || !Number.isInteger(w))
  ) {
    return null;
  }
  const magnitude = BigInt(Math.abs(totalMinor));
  const sign = totalMinor < 0 ? -1 : 1;
  const sum = BigInt(weightSum);
  const parts: number[] = [];
  let allocated = 0;
  weights.forEach((weight, index) => {
    let minor: number;
    if (index === weights.length - 1) {
      minor = totalMinor - allocated;
    } else {
      // Arrondi au plus proche, demi vers le haut en valeur absolue (ADR-0001).
      const share = (magnitude * BigInt(weight) * 2n + sum) / (sum * 2n);
      minor = sign * Number(share);
    }
    allocated += minor;
    parts.push(minor);
  });
  return parts;
}

/** Somme exacte de montants entiers. */
export function sum(amounts: number[]): number {
  return amounts.reduce((total, amount) => total + amount, 0);
}

/** Opposé d'un montant. */
export function negate(minor: number): number {
  return minor === 0 ? 0 : -minor;
}

/** Valeur absolue d'un montant. */
export function abs(minor: number): number {
  return Math.abs(minor);
}

/** Pourcentage en millionièmes (ppm) affiché avec une décimale : « 41,7 % ». */
export function formatRatePpm(ppm: number): string {
  // 1 % = 10 000 ppm : un dixième de point vaut 1 000 ppm.
  const tenths = Math.round(ppm / 1000);
  const negative = tenths < 0;
  const absolute = Math.abs(tenths);
  return `${negative ? '−' : ''}${Math.trunc(absolute / 10)},${absolute % 10}\u00a0%`;
}

/**
 * Valeur d'axe pour un graphique (« 1234.56 » pour 123456 centimes). Affichage seulement : ce nombre
 * n'est jamais réutilisé dans un calcul.
 */
export function toChartValue(minor: number, currency = 'EUR'): number {
  return minor / 10 ** minorDigits(currency);
}

/** Libellé d'une valeur d'axe ou d'info-bulle d'un graphique. */
export function formatChartValue(
  value: number,
  currency = 'EUR',
  options: Omit<FormatOptions, 'currency'> = {},
): string {
  return formatMoney(Math.round(value * 10 ** minorDigits(currency)), { ...options, currency });
}

/** Taux annuel en millionièmes affiché avec deux décimales : « 1,70 % » (taux réglementés). */
export function formatRate(ppm: number): string {
  const hundredths = Math.round(ppm / 100);
  const negative = hundredths < 0;
  const absolute = Math.abs(hundredths);
  const decimals = String(absolute % 100).padStart(2, '0');
  return `${negative ? '−' : ''}${Math.trunc(absolute / 100)},${decimals} %`;
}

/** Saisie d'un taux en pour cent (« 1,7 », « 2.40 % ») → millionièmes ; `null` si invalide. */
export function parseRatePpm(input: string): number | null {
  const text = input.replace(/\s|%/g, '').replace(',', '.');
  if (!/^\d+(\.\d{1,4})?$/.test(text)) {
    return null;
  }
  const [units, decimals = ''] = text.split('.');
  return Number(units) * 10_000 + Number(decimals.padEnd(4, '0'));
}
