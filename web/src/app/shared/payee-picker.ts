import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  model,
  signal,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import {
  MatAutocompleteModule,
  MatAutocompleteSelectedEvent,
} from '@angular/material/autocomplete';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';

import { errorMessage } from '../core/http-errors';
import { PayeesStore } from './stores';

interface Group {
  name: string;
  options: { id: string; text: string }[];
}

/** Préfixe de l'option « Créer le tiers « … » » (R-TIE-15). */
const CREATE = 'new:';

/**
 * Choix d'un tiers par recherche, regroupés par groupe de tiers (ecrans.md E5). Les tiers archivés ne sont pas
 * proposés (R-TIE-6). Avec `creatable`, un nom inconnu propose de créer le tiers, sans groupe (R-TIE-15).
 */
@Component({
  selector: 'app-payee-picker',
  imports: [
    FormsModule,
    MatAutocompleteModule,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <mat-form-field [appearance]="appearance()" [subscriptSizing]="subscript()" class="picker">
      @if (label()) {
        <mat-label>{{ label() }}</mat-label>
      }
      <input
        matInput
        [placeholder]="placeholder()"
        [ngModel]="text()"
        (ngModelChange)="text.set($event)"
        [matAutocomplete]="auto"
        [disabled]="disabled()"
        (blur)="restore()"
      />
      @if (value() && !disabled()) {
        <button
          matIconSuffix
          mat-icon-button
          type="button"
          aria-label="Retirer le tiers"
          (click)="clear()"
        >
          <mat-icon>close</mat-icon>
        </button>
      }
      @if (error()) {
        <mat-hint class="error">{{ error() }}</mat-hint>
      }
      <mat-autocomplete #auto (optionSelected)="select($event)" [displayWith]="display">
        @if (creation(); as name) {
          <mat-option [value]="create + name">
            <mat-icon>add</mat-icon>Créer le tiers « {{ name }} »
          </mat-option>
        }
        @for (group of groups(); track group.name) {
          <mat-optgroup [label]="group.name">
            @for (option of group.options; track option.id) {
              <mat-option [value]="option.id">{{ option.text }}</mat-option>
            }
          </mat-optgroup>
        } @empty {
          @if (!creation()) {
            <mat-option disabled>Aucun tiers</mat-option>
          }
        }
      </mat-autocomplete>
    </mat-form-field>
  `,
  styles: `
    .picker {
      width: 100%;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class PayeePicker {
  private readonly store = inject(PayeesStore);

  readonly value = model<string | null>(null);
  readonly label = input('Tiers');
  readonly placeholder = input('Magasin, personne, organisme…');
  readonly appearance = input<'fill' | 'outline'>('outline');
  readonly subscript = input<'fixed' | 'dynamic'>('dynamic');
  readonly creatable = input(true);
  readonly disabled = input(false);

  protected readonly create = CREATE;
  protected readonly text = signal('');
  protected readonly error = signal<string | null>(null);

  constructor() {
    // Le texte suit le tiers choisi.
    effect(() => this.text.set(this.store.name(this.value())));
  }

  protected readonly display = (value: string | null): string => {
    if (!value) return '';
    if (value.startsWith(CREATE)) return value.slice(CREATE.length);
    return this.store.byId().has(value) ? this.store.name(value) : value;
  };

  private readonly query = computed(() => {
    const query = this.text().trim().toLowerCase();
    return query === this.store.name(this.value()).toLowerCase() ? '' : query;
  });

  protected readonly groups = computed<Group[]>(() => {
    const query = this.query();
    const groups = new Map<string, Group['options']>();
    const named = (id: string | null | undefined) => this.store.groupName(id) || 'Sans groupe';
    const options = this.store
      .list()
      .filter((p) => !p.archived)
      .filter((p) => !query || p.name.toLowerCase().includes(query))
      .sort((a, b) => named(a.group_id).localeCompare(named(b.group_id)));
    for (const p of options) {
      const name = named(p.group_id);
      groups.set(name, [...(groups.get(name) ?? []), { id: p.id, text: p.name }]);
    }
    // « Sans groupe » en dernier.
    return [...groups.entries()]
      .map(([name, list]) => ({ name, options: list }))
      .sort((a, b) => Number(a.name === 'Sans groupe') - Number(b.name === 'Sans groupe'));
  });

  /** Nom à créer : saisi, et différent de tout tiers existant (R-TIE-1). */
  protected readonly creation = computed(() => {
    const name = this.text().trim();
    if (!this.creatable() || !this.query() || name.length > 80) return null;
    const lower = name.toLowerCase();
    return this.store.list().some((p) => p.name.toLowerCase() === lower) ? null : name;
  });

  protected async select(event: MatAutocompleteSelectedEvent): Promise<void> {
    const value = event.option.value as string;
    this.error.set(null);
    if (!value.startsWith(CREATE)) {
      this.value.set(value);
      return;
    }
    try {
      this.value.set(await this.store.create(value.slice(CREATE.length)));
    } catch (error) {
      this.error.set(errorMessage(error));
      this.text.set('');
    }
  }

  protected clear(): void {
    this.value.set(null);
    this.text.set('');
  }

  protected restore(): void {
    // Un texte libre qui ne correspond à rien reprend le tiers choisi.
    setTimeout(() => this.text.set(this.store.name(this.value())), 150);
  }
}
