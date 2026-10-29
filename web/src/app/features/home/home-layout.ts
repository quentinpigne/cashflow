import { Injectable, signal } from '@angular/core';

/** Les encadrés de l'accueil (ecrans.md E1). */
export const WIDGETS = [
  'net-worth',
  'month',
  'recent',
  'alerts',
  'missed',
  'refunds',
  'uncategorized',
  'budget',
  'savings',
  'accounts',
  'assets',
  'loans',
] as const;

export type Widget = (typeof WIDGETS)[number];

/** Nom de chaque encadré (lecteurs d'écran, liste des encadrés masqués). */
export const WIDGET_LABELS: Record<Widget, string> = {
  'net-worth': 'Patrimoine',
  month: 'Mois en cours',
  recent: 'Dernières saisies',
  alerts: 'Alertes',
  missed: 'Échéances non vues',
  refunds: 'Remboursements attendus',
  uncategorized: 'À catégoriser',
  budget: 'Budget du mois',
  savings: 'Épargne',
  accounts: 'Comptes',
  assets: 'Biens',
  loans: 'Prêts',
};

/** Nombre de colonnes, d'après la largeur disponible : une sur mobile, jusqu'à quatre sur grand écran. */
export const MAX_COLUMNS = 4;

/** Largeur minimale d'une colonne et espacement entre colonnes, en pixels (22rem et 1rem). */
const MIN_COLUMN_WIDTH = 352;
const COLUMN_GAP = 16;

export function columnCount(width: number): number {
  const count = Math.floor((width + COLUMN_GAP) / (MIN_COLUMN_WIDTH + COLUMN_GAP));
  return Math.min(MAX_COLUMNS, Math.max(1, count));
}

/** Disposition propre à chaque nombre de colonnes (clé : nombre de colonnes). */
export type HomeLayoutState = Record<number, Widget[][]>;

const STORAGE_KEY = 'cashflow.home-layout.v2';
/** Version précédente : deux colonnes sur ordinateur, une liste sur mobile. */
const LEGACY_KEY = 'cashflow.home-layout.v1';

const DEFAULTS: Record<number, Widget[][]> = {
  1: [
    [
      'net-worth',
      'month',
      'recent',
      'alerts',
      'missed',
      'refunds',
      'uncategorized',
      'budget',
      'savings',
      'accounts',
      'assets',
      'loans',
    ],
  ],
  2: [
    ['net-worth', 'month', 'recent', 'alerts', 'missed', 'refunds', 'uncategorized'],
    ['budget', 'savings', 'accounts', 'assets', 'loans'],
  ],
  3: [
    ['net-worth', 'month', 'alerts', 'missed'],
    ['recent', 'uncategorized', 'refunds', 'savings'],
    ['budget', 'accounts', 'assets', 'loans'],
  ],
  4: [
    ['net-worth', 'month', 'alerts', 'missed'],
    ['recent', 'uncategorized', 'refunds'],
    ['budget', 'savings'],
    ['accounts', 'assets', 'loans'],
  ],
};

export function defaultColumns(count: number): Widget[][] {
  return DEFAULTS[count].map((c) => [...c]);
}

/**
 * Garde chaque encadré connu exactement une fois dans `count` colonnes : les inconnus (version
 * précédente) disparaissent, les colonnes en trop se replient sur la dernière, les nouveaux encadrés
 * s'ajoutent à la fin de la première colonne.
 */
export function normalize(columns: unknown, count: number): Widget[][] {
  if (!Array.isArray(columns)) return defaultColumns(count);
  const known = new Set<string>(WIDGETS);
  const seen = new Set<Widget>();
  const keep = (list: unknown): Widget[] =>
    (Array.isArray(list) ? list : []).filter((w): w is Widget => {
      if (typeof w !== 'string' || !known.has(w) || seen.has(w as Widget)) return false;
      seen.add(w as Widget);
      return true;
    });
  const result = Array.from({ length: count }, (_, i) => keep(columns[i]));
  result[count - 1].push(...columns.slice(count).flatMap(keep));
  result[0].push(...WIDGETS.filter((w) => !seen.has(w)));
  return result;
}

/** Relit une disposition sauvegardée, y compris celle de la version précédente. */
export function parse(saved: unknown, legacy: unknown): HomeLayoutState {
  const state: HomeLayoutState = {};
  if (saved && typeof saved === 'object') {
    for (const [key, columns] of Object.entries(saved)) {
      const count = Number(key);
      if (count in DEFAULTS) state[count] = normalize(columns, count);
    }
  } else if (legacy && typeof legacy === 'object') {
    const { desktop, mobile } = legacy as { desktop?: unknown; mobile?: unknown };
    if (desktop) state[2] = normalize(desktop, 2);
    if (mobile) state[1] = normalize([mobile], 1);
  }
  return state;
}

/**
 * Disposition de l'accueil, propre à ce navigateur (`localStorage`). Le stockage peut être indisponible
 * (navigation privée) : la disposition par défaut s'applique alors, sans erreur.
 */
@Injectable({ providedIn: 'root' })
export class HomeLayout {
  /** Dispositions modifiées seulement : un nombre de colonnes absent suit la disposition par défaut. */
  readonly state = signal<HomeLayoutState>(this.load());

  private load(): HomeLayoutState {
    try {
      const read = (key: string): unknown => {
        const raw = localStorage.getItem(key);
        return raw ? JSON.parse(raw) : null;
      };
      return parse(read(STORAGE_KEY), read(LEGACY_KEY));
    } catch {
      return {};
    }
  }

  columns(count: number): Widget[][] {
    return this.state()[count] ?? defaultColumns(count);
  }

  save(count: number, columns: Widget[][]): void {
    this.state.update((state) => ({ ...state, [count]: normalize(columns, count) }));
    this.persist();
  }

  reset(count: number): void {
    this.state.update((state) => {
      const next = { ...state };
      delete next[count];
      return next;
    });
    this.persist();
  }

  isDefault(count: number): boolean {
    return JSON.stringify(this.columns(count)) === JSON.stringify(DEFAULTS[count]);
  }

  private persist(): void {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.state()));
      localStorage.removeItem(LEGACY_KEY);
    } catch {
      // Stockage indisponible : la disposition vaut pour la session seulement.
    }
  }
}
