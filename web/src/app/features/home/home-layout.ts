import { Injectable, signal } from '@angular/core';

/** Les encadrés de l'accueil (ecrans.md E1). */
export const WIDGETS = [
  'net-worth',
  'month',
  'alerts',
  'missed',
  'uncategorized',
  'budget',
  'savings',
  'accounts',
] as const;

export type Widget = (typeof WIDGETS)[number];

/** Disposition : deux colonnes sur ordinateur, une seule liste sur mobile. */
export interface HomeLayoutState {
  desktop: Widget[][];
  mobile: Widget[];
}

const STORAGE_KEY = 'cashflow.home-layout.v1';

const DEFAULT_DESKTOP: Widget[][] = [
  ['net-worth', 'month', 'alerts', 'missed', 'uncategorized'],
  ['budget', 'savings', 'accounts'],
];

export function defaultLayout(): HomeLayoutState {
  return { desktop: DEFAULT_DESKTOP.map((c) => [...c]), mobile: DEFAULT_DESKTOP.flat() };
}

/**
 * Garde chaque encadré connu exactement une fois : les inconnus (version précédente) disparaissent, les
 * nouveaux s'ajoutent à la fin de la première colonne.
 */
export function normalize(state: Partial<HomeLayoutState> | null | undefined): HomeLayoutState {
  const known = new Set<string>(WIDGETS);
  const seen = new Set<Widget>();
  const keep = (list: unknown): Widget[] =>
    (Array.isArray(list) ? list : []).filter((w): w is Widget => {
      if (typeof w !== 'string' || !known.has(w) || seen.has(w as Widget)) return false;
      seen.add(w as Widget);
      return true;
    });
  const columns = Array.isArray(state?.desktop) ? state.desktop : DEFAULT_DESKTOP;
  const desktop = [keep(columns[0]), keep(columns[1])];
  desktop[0].push(...WIDGETS.filter((w) => !seen.has(w)));

  seen.clear();
  const mobile = keep(state?.mobile ?? desktop.flat());
  mobile.push(...WIDGETS.filter((w) => !seen.has(w)));
  return { desktop, mobile };
}

/**
 * Disposition de l'accueil, propre à ce navigateur (`localStorage`). Le stockage peut être indisponible
 * (navigation privée) : la disposition par défaut s'applique alors, sans erreur.
 */
@Injectable({ providedIn: 'root' })
export class HomeLayout {
  readonly state = signal<HomeLayoutState>(this.load());

  private load(): HomeLayoutState {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      return normalize(raw ? (JSON.parse(raw) as Partial<HomeLayoutState>) : null);
    } catch {
      return defaultLayout();
    }
  }

  save(state: HomeLayoutState): void {
    this.state.set(normalize(state));
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.state()));
    } catch {
      // Stockage indisponible : la disposition vaut pour la session seulement.
    }
  }

  reset(): void {
    this.state.set(defaultLayout());
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      // Rien à faire.
    }
  }

  isDefault(): boolean {
    return JSON.stringify(this.state()) === JSON.stringify(defaultLayout());
  }
}
