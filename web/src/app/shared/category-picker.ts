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

import { CategoryKind } from '../api/models/category-kind';
import { CategoriesStore, FlatCategory } from './stores';

interface Group {
  name: string;
  options: FlatCategory[];
}

/**
 * Choix d'une catégorie par recherche dans « Parent › Sous-catégorie » (ecrans.md E5).
 * Les catégories de la nature préférée viennent d'abord ; les archivées ne sont pas proposées.
 */
@Component({
  selector: 'app-category-picker',
  imports: [
    FormsModule,
    MatAutocompleteModule,
    MatFormFieldModule,
    MatInputModule,
    MatIconModule,
    MatButtonModule,
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
        (blur)="restore()"
      />
      @if (value()) {
        <button
          matIconSuffix
          mat-icon-button
          type="button"
          aria-label="Retirer la catégorie"
          (click)="clear()"
        >
          <mat-icon>close</mat-icon>
        </button>
      }
      <mat-autocomplete #auto (optionSelected)="select($event)" [displayWith]="display">
        @for (group of groups(); track group.name) {
          <mat-optgroup [label]="group.name">
            @for (option of group.options; track option.id) {
              <mat-option [value]="option.id">{{
                option.parentId ? option.name : option.name + ' (général)'
              }}</mat-option>
            }
          </mat-optgroup>
        } @empty {
          <mat-option disabled>Aucune catégorie</mat-option>
        }
      </mat-autocomplete>
    </mat-form-field>
  `,
  styles: `
    .picker {
      width: 100%;
    }
  `,
})
export class CategoryPicker {
  private readonly store = inject(CategoriesStore);

  readonly value = model<string | null>(null);
  readonly kind = input<CategoryKind | null>(null);
  readonly label = input('Catégorie');
  readonly placeholder = input('Rechercher…');
  readonly appearance = input<'fill' | 'outline'>('outline');
  readonly subscript = input<'fixed' | 'dynamic'>('dynamic');

  protected readonly text = signal('');

  constructor() {
    // Le texte suit la valeur choisie (affichage « Parent › Enfant »).
    effect(() =>
      this.text.set(
        this.store.label(this.value()) === 'Non catégorisé' ? '' : this.store.label(this.value()),
      ),
    );
  }

  protected readonly display = (id: string | null): string => (id ? this.store.label(id) : '');

  protected readonly groups = computed<Group[]>(() => {
    const query = this.text().trim().toLowerCase();
    const selectedLabel = this.store.label(this.value()).toLowerCase();
    const searching = query !== '' && query !== selectedLabel;
    const options = this.store
      .flat()
      .filter((c) => !c.archived)
      .filter((c) => !searching || c.label.toLowerCase().includes(query));
    const preferred = this.kind();
    const sorted = preferred
      ? [
          ...options.filter((c) => c.kind === preferred),
          ...options.filter((c) => c.kind !== preferred),
        ]
      : options;
    const groups = new Map<string, FlatCategory[]>();
    for (const option of sorted) {
      const name = option.parentName ?? option.name;
      groups.set(name, [...(groups.get(name) ?? []), option]);
    }
    return [...groups.entries()].map(([name, list]) => ({ name, options: list }));
  });

  protected select(event: MatAutocompleteSelectedEvent): void {
    this.value.set(event.option.value as string);
  }

  protected clear(): void {
    this.value.set(null);
    this.text.set('');
  }

  protected restore(): void {
    // Un texte libre qui ne correspond à rien reprend la valeur choisie.
    setTimeout(() => {
      const label = this.store.label(this.value());
      this.text.set(this.value() ? label : '');
    }, 150);
  }
}
