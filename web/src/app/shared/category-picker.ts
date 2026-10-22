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
import { AttachmentsStore, CategoriesStore } from './stores';

interface Option {
  /** Identifiant de catégorie, ou `loan:<id>` / `asset:<id>` (core/attachments.ts). */
  id: string;
  text: string;
}

interface Group {
  name: string;
  options: Option[];
}

/**
 * Choix d'une catégorie par recherche dans « Parent › Sous-catégorie » (ecrans.md E5).
 * Les catégories de la nature préférée viennent d'abord ; les archivées ne sont pas proposées. Avec
 * `attachments`, un remboursement de prêt ou une acquisition de bien se choisit de la même façon (R-TRX-37).
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
              <mat-option [value]="option.id">{{ option.text }}</mat-option>
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
  private readonly attachmentsStore = inject(AttachmentsStore);

  readonly value = model<string | null>(null);
  readonly kind = input<CategoryKind | null>(null);
  readonly label = input('Catégorie');
  readonly placeholder = input('Rechercher…');
  readonly appearance = input<'fill' | 'outline'>('outline');
  readonly subscript = input<'fixed' | 'dynamic'>('dynamic');
  /** Propose aussi « Remboursement › prêt » et « Acquisition › bien » (ventilation, R-TRX-37). */
  readonly attachments = input(false);

  /** Libellé d'une valeur : catégorie ou rattachement. */
  private label_(value: string | null): string {
    const attachment = value ? this.attachmentsStore.byValue().get(value) : undefined;
    return attachment ? attachment.label : this.store.label(value);
  }

  protected readonly text = signal('');

  constructor() {
    // Le texte suit la valeur choisie (affichage « Parent › Enfant »).
    effect(() => this.text.set(this.value() ? this.label_(this.value()) : ''));
  }

  /**
   * Texte du champ. Le déclencheur d'autocomplétion est aussi l'accesseur de `ngModel` : il reçoit
   * tantôt l'identifiant de l'option choisie, tantôt le texte du champ (libellé ou saisie en cours).
   */
  protected readonly display = (value: string | null): string => {
    if (!value) return '';
    return this.store.byId().has(value) || this.attachmentsStore.byValue().has(value)
      ? this.label_(value)
      : value;
  };

  protected readonly groups = computed<Group[]>(() => {
    const query = this.text().trim().toLowerCase();
    const selectedLabel = this.label_(this.value()).toLowerCase();
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
    const groups = new Map<string, Option[]>();
    for (const option of sorted) {
      const name = option.parentName ?? option.name;
      const text = option.parentId ? option.name : option.name + ' (général)';
      groups.set(name, [...(groups.get(name) ?? []), { id: option.id, text }]);
    }
    if (this.attachments()) {
      for (const a of this.attachmentsStore.options()) {
        if (searching && !a.label.toLowerCase().includes(query)) continue;
        groups.set(a.group, [...(groups.get(a.group) ?? []), { id: a.value, text: a.name }]);
      }
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
      this.text.set(this.value() ? this.label_(this.value()) : '');
    }, 150);
  }
}
