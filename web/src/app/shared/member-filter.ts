import {
  ChangeDetectionStrategy,
  Component,
  Injectable,
  Signal,
  computed,
  inject,
  input,
  signal,
} from '@angular/core';
import { MatCheckboxModule } from '@angular/material/checkbox';

import { AccountsStore, FeaturesStore, MembersStore } from './stores';

/**
 * Choix de membres mémorisés à part (R-MBR-8) : un par écran pour les analyses et les projections, un par
 * encadré sur l'accueil (`home.<encadré>`).
 */
export type MemberScreen = 'analyses' | 'projections' | `home.${string}`;

/** Case « Foyer » : ce qui n'est affecté à aucun membre (R-MBR-9). */
export const HOUSEHOLD = 'household';

/**
 * Ce que compte un encadré ou un écran, pour ne proposer que les cases qui y comptent quelque chose (R-MBR-8) :
 * patrimoine, comptes ouverts, compte courant, épargne, flux (dépenses et revenus) ou salaire.
 */
export type MemberScope = 'net-worth' | 'accounts' | 'cash' | 'savings' | 'flows' | 'salary';

/** Pour chaque périmètre, les cases qui comptent quelque chose : `household` et des identifiants de membres. */
@Injectable({ providedIn: 'root' })
export class MemberRelevance {
  private readonly accounts = inject(AccountsStore);
  private readonly members = inject(MembersStore);
  private readonly features = inject(FeaturesStore);

  /** Titulaires des comptes retenus, et « Foyer » si l'un d'eux n'en a pas. */
  private holders(keep: (a: ReturnType<AccountsStore['list']>[number]) => boolean): Set<string> {
    return new Set(
      this.accounts
        .list()
        .filter(keep)
        .map((a) => a.member_id ?? HOUSEHOLD),
    );
  }

  private readonly scopes: Record<MemberScope, Signal<Set<string>>> = {
    'net-worth': computed(() => {
      const tokens = this.holders((a) => a.include_in_net_worth);
      const f = this.features.features.value();
      // Les biens et les prêts n'ont pas de membre : ils comptent avec « Foyer » (R-MBR-9).
      if ((f?.assets && f.asset_count > 0) || (f?.loans && f.loan_count > 0)) tokens.add(HOUSEHOLD);
      return tokens;
    }),
    accounts: computed(() => this.holders((a) => !a.closed_on)),
    cash: computed(() => this.holders((a) => !a.closed_on && a.kind === 'CHECKING')),
    savings: computed(() =>
      this.holders(
        (a) =>
          !a.closed_on &&
          (a.family === 'FIXED_SAVINGS' ||
            a.family === 'VARIABLE_SAVINGS' ||
            a.include_in_emergency_fund),
      ),
    ),
    // Une ventilation compte pour son membre, à défaut pour le titulaire de son compte (R-MBR-6).
    flows: computed(() => {
      const tokens = this.holders(() => true);
      for (const m of this.members.list()) if (m.split_count > 0) tokens.add(m.id);
      return tokens;
    }),
    salary: computed(() => {
      const tokens = new Set<string>();
      let assigned = 0;
      for (const m of this.members.list()) {
        if (m.employment_count > 0) tokens.add(m.id);
        assigned += m.employment_count;
      }
      if ((this.features.features.value()?.employment_count ?? 0) > assigned) tokens.add(HOUSEHOLD);
      return tokens;
    }),
  };

  of(scope: MemberScope): Signal<Set<string>> {
    return this.scopes[scope];
  }
}

const key = (screen: MemberScreen) => `cashflow.members.${screen}`;

/** Cases décochées, mémorisées : un nouveau membre apparaît coché. */
function stored(screen: MemberScreen): string[] {
  try {
    const raw = localStorage.getItem(key(screen));
    const value: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
  } catch {
    return [];
  }
}

/** Le choix des membres d'un écran (R-MBR-8 à R-MBR-10). */
export class MemberSelection {
  private readonly screen: MemberScreen;
  private readonly members: MembersStore;
  private readonly unchecked: ReturnType<typeof signal<string[]>>;
  private readonly relevant: Signal<Set<string>>;

  constructor(screen: MemberScreen, members: MembersStore, relevant: Signal<Set<string>>) {
    this.screen = screen;
    this.members = members;
    this.relevant = relevant;
    this.unchecked = signal<string[]>(stored(screen));
  }

  /**
   * Les cases qui comptent quelque chose ici, « Foyer » d'abord, puis les membres dans leur ordre ; aucune s'il
   * en reste moins de deux : il n'y a rien à choisir (R-MBR-8).
   */
  readonly options = computed(() => {
    if (!this.members.active()) return [];
    const relevant = this.relevant();
    const all = [
      { token: HOUSEHOLD, name: 'Foyer', color: null as string | null },
      ...this.members.list().map((m) => ({ token: m.id, name: m.name, color: m.color ?? null })),
    ].filter((o) => relevant.has(o.token));
    return all.length > 1 ? all : [];
  });

  /** Cases cochées parmi celles proposées ; toutes si aucune ne l'est plus (case cochée devenue sans objet). */
  readonly checked = computed(() => {
    const unchecked = new Set(this.unchecked());
    const tokens = this.options().map((o) => o.token);
    const checked = tokens.filter((t) => !unchecked.has(t));
    return checked.length > 0 ? checked : tokens;
  });

  /** Toutes les cases ne sont pas cochées : les chiffres ne sont plus ceux du foyer (R-MBR-10). */
  readonly partial = computed(() => this.checked().length < this.options().length);

  /** Paramètre `members` de l'API : absent quand tout est coché. */
  readonly param = computed(() => (this.partial() ? this.checked().join(',') : undefined));

  /** Suffixe du titre d'un encadré filtré : « · Claire », « · 2 membres » ; vide quand tout est coché. */
  readonly titleSuffix = computed(() => {
    if (!this.partial()) return '';
    const checked = this.options().filter((o) => this.isChecked(o.token));
    return checked.length === 1 ? ` · ${checked[0].name}` : ` · ${checked.length} membres`;
  });

  /** Entrées « Membres » du menu de réglages d'un encadré (E1), qui laissent le menu ouvert. */
  readonly menuItems = computed(() =>
    this.options().map((o, i) => ({
      label: o.name,
      checked: this.isChecked(o.token),
      select: () => this.toggle(o.token, !this.isChecked(o.token)),
      keepOpen: true,
      section: i === 0 ? 'Membres' : undefined,
    })),
  );

  /** « Quentin et Claire seulement ». */
  readonly summary = computed(() => {
    const names = this.options()
      .filter((o) => this.checked().includes(o.token))
      .map((o) => o.name);
    const list =
      names.length > 1
        ? `${names.slice(0, -1).join(', ')} et ${names[names.length - 1]}`
        : names[0];
    return `${list} seulement`;
  });

  isChecked(token: string): boolean {
    return this.checked().includes(token);
  }

  /** Compte ce qui est affecté à `member`, ou au foyer pour `null` (R-MBR-9). */
  includes(member: string | null | undefined): boolean {
    return !this.partial() || this.isChecked(member ?? HOUSEHOLD);
  }

  /** La dernière case cochée ne se décoche pas (R-MBR-8). */
  toggle(token: string, on: boolean): void {
    if (!on && this.checked().length <= 1) return;
    const next = on ? this.unchecked().filter((t) => t !== token) : [...this.unchecked(), token];
    this.unchecked.set(next);
    try {
      localStorage.setItem(key(this.screen), JSON.stringify(next));
    } catch {
      // Stockage indisponible : le choix vaut pour la session.
    }
  }
}

/** Choix des membres, un par écran, partagé par les encadrés de l'écran. */
@Injectable({ providedIn: 'root' })
export class MemberSelections {
  private readonly members = inject(MembersStore);
  private readonly relevance = inject(MemberRelevance);
  private readonly screens = new Map<MemberScreen, MemberSelection>();

  /** Le choix d'un écran ou d'un encadré, limité aux cases qui comptent quelque chose pour `scope`. */
  of(screen: MemberScreen, scope: MemberScope): MemberSelection {
    let selection = this.screens.get(screen);
    if (!selection) {
      selection = new MemberSelection(screen, this.members, this.relevance.of(scope));
      this.screens.set(screen, selection);
    }
    return selection;
  }
}

/** Cases à cocher « Foyer · Quentin · Claire » (R-MBR-8), seulement si le foyer a des membres. */
@Component({
  selector: 'app-member-filter',
  imports: [MatCheckboxModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (selection().options().length > 0) {
      <div class="members" role="group" aria-label="Membres affichés">
        @for (o of selection().options(); track o.token) {
          <mat-checkbox
            [checked]="selection().isChecked(o.token)"
            [disabled]="selection().isChecked(o.token) && selection().checked().length === 1"
            (change)="selection().toggle(o.token, $event.checked)"
          >
            @if (o.color) {
              <span class="dot" [style.background]="o.color" aria-hidden="true"></span>
            }
            {{ o.name }}
          </mat-checkbox>
        }
      </div>
      @if (selection().partial()) {
        <p class="muted small">{{ selection().summary() }}{{ note() ? ' : ' + note() : '' }}</p>
      }
    }
  `,
  styles: `
    .members {
      display: flex;
      flex-wrap: wrap;
      column-gap: 0.5rem;
    }
    .dot {
      display: inline-block;
      width: 0.6rem;
      height: 0.6rem;
      margin-right: 0.25rem;
      border-radius: 50%;
    }
    .small {
      margin: 0 0 0.5rem;
      font: var(--mat-sys-body-small);
    }
  `,
})
export class MemberFilter {
  private readonly selections = inject(MemberSelections);
  readonly screen = input.required<MemberScreen>();
  /** Ce que compte l'écran : seules les cases qui y comptent quelque chose sont proposées (R-MBR-8). */
  readonly scope = input.required<MemberScope>();
  /** Ce qui n'est pas réparti entre membres, rappelé quand toutes les cases ne sont pas cochées (R-MBR-9). */
  readonly note = input<string>('');
  protected readonly selection = computed(() => this.selections.of(this.screen(), this.scope()));
}
