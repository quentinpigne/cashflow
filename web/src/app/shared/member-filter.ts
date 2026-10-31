import {
  ChangeDetectionStrategy,
  Component,
  Injectable,
  computed,
  inject,
  input,
  signal,
} from '@angular/core';
import { MatCheckboxModule } from '@angular/material/checkbox';

import { MembersStore } from './stores';

/**
 * Choix de membres mémorisés à part (R-MBR-8) : un par écran pour les analyses et les projections, un par
 * encadré sur l'accueil (`home.<encadré>`).
 */
export type MemberScreen = 'analyses' | 'projections' | `home.${string}`;

/** Case « Foyer » : ce qui n'est affecté à aucun membre (R-MBR-9). */
export const HOUSEHOLD = 'household';

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

  constructor(screen: MemberScreen, members: MembersStore) {
    this.screen = screen;
    this.members = members;
    this.unchecked = signal<string[]>(stored(screen));
  }

  /** Les cases, « Foyer » d'abord, et les membres dans leur ordre. */
  readonly options = computed(() =>
    this.members.active()
      ? [
          { token: HOUSEHOLD, name: 'Foyer', color: null as string | null },
          ...this.members
            .list()
            .map((m) => ({ token: m.id, name: m.name, color: m.color ?? null })),
        ]
      : [],
  );

  readonly checked = computed(() => {
    const unchecked = new Set(this.unchecked());
    return this.options()
      .map((o) => o.token)
      .filter((t) => !unchecked.has(t));
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
  private readonly screens = new Map<MemberScreen, MemberSelection>();

  of(screen: MemberScreen): MemberSelection {
    let selection = this.screens.get(screen);
    if (!selection) {
      selection = new MemberSelection(screen, this.members);
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
  /** Ce qui n'est pas réparti entre membres, rappelé quand toutes les cases ne sont pas cochées (R-MBR-9). */
  readonly note = input<string>('');
  protected readonly selection = computed(() => this.selections.of(this.screen()));
}
