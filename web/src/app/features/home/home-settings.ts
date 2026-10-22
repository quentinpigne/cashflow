import { Injectable, computed, signal } from '@angular/core';

import { WIDGETS, Widget } from './home-layout';

/** Options d'affichage des encadrés de l'accueil (ecrans.md E1), avec leur valeur par défaut. */
export const OPTION_DEFAULTS = {
  'net-worth.detail': false,
  'month.previous': true,
  'accounts.detail': true,
  'budget.goals': true,
  'budget.projects': true,
  'savings.emergency': true,
  'savings.interest': true,
  'savings.variable': true,
} satisfies Record<string, boolean>;

export type WidgetOption = keyof typeof OPTION_DEFAULTS;

export interface HomeSettingsState {
  hidden: Widget[];
  /** Options différentes de leur valeur par défaut seulement. */
  options: Partial<Record<WidgetOption, boolean>>;
}

const STORAGE_KEY = 'cashflow.home-settings.v1';

/** Ne garde que les encadrés et options connus (une version précédente a pu en avoir d'autres). */
export function normalizeSettings(saved: unknown): HomeSettingsState {
  const state = (saved && typeof saved === 'object' ? saved : {}) as Partial<
    Record<keyof HomeSettingsState, unknown>
  >;
  const hidden = WIDGETS.filter((w) => Array.isArray(state.hidden) && state.hidden.includes(w));
  const options: HomeSettingsState['options'] = {};
  if (state.options && typeof state.options === 'object') {
    for (const [key, value] of Object.entries(state.options)) {
      if (key in OPTION_DEFAULTS && typeof value === 'boolean') {
        options[key as WidgetOption] = value;
      }
    }
  }
  return { hidden, options };
}

/**
 * Réglages des encadrés de l'accueil, propres à ce navigateur (`localStorage`) : encadrés masqués et
 * options d'affichage. Le stockage peut être indisponible (navigation privée) : les réglages valent
 * alors pour la session.
 */
@Injectable({ providedIn: 'root' })
export class HomeSettings {
  private readonly state = signal<HomeSettingsState>(this.load());

  readonly hiddenWidgets = computed(() => this.state().hidden);

  isHidden(widget: Widget): boolean {
    return this.state().hidden.includes(widget);
  }

  hide(widget: Widget): void {
    this.save({ ...this.state(), hidden: [...this.state().hidden, widget] });
  }

  show(widget: Widget): void {
    this.save({ ...this.state(), hidden: this.state().hidden.filter((w) => w !== widget) });
  }

  option(key: WidgetOption): boolean {
    return this.state().options[key] ?? OPTION_DEFAULTS[key];
  }

  toggle(key: WidgetOption): void {
    this.save({ ...this.state(), options: { ...this.state().options, [key]: !this.option(key) } });
  }

  private load(): HomeSettingsState {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      return normalizeSettings(raw ? JSON.parse(raw) : null);
    } catch {
      return normalizeSettings(null);
    }
  }

  private save(state: HomeSettingsState): void {
    this.state.set(normalizeSettings(state));
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.state()));
    } catch {
      // Stockage indisponible : les réglages valent pour la session seulement.
    }
  }
}
