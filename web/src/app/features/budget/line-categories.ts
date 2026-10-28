import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  model,
  signal,
} from '@angular/core';
import { MatChipsModule } from '@angular/material/chips';
import { MatIconModule } from '@angular/material/icon';

import { CategoryKind } from '../../api/models/category-kind';
import { CategoryPicker } from '../../shared/category-picker';
import { CategoriesStore } from '../../shared/stores';

/** Catégories d'une ligne de budget : une ou plusieurs, de la nature de la ligne (R-BUD-10). */
@Component({
  selector: 'app-line-categories',
  imports: [MatChipsModule, MatIconModule, CategoryPicker],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (value().length > 0) {
      <mat-chip-set aria-label="Catégories de la ligne">
        @for (id of value(); track id) {
          <mat-chip (removed)="remove(id)">
            {{ store.label(id) }}
            <button matChipRemove [attr.aria-label]="'Retirer ' + store.label(id)">
              <mat-icon>cancel</mat-icon>
            </button>
          </mat-chip>
        }
      </mat-chip-set>
    }
    <!-- Un sélecteur neuf après chaque choix : le champ se vide pour en ajouter une autre. -->
    @for (key of [added()]; track key) {
      <app-category-picker [kind]="kind()" [label]="label()" (valueChange)="add($event)" />
    }
  `,
  styles: `
    :host {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }
  `,
})
export class LineCategories {
  protected readonly store = inject(CategoriesStore);

  readonly value = model<string[]>([]);
  readonly kind = input<CategoryKind>('EXPENSE');

  /** Nombre de choix faits : change à chaque ajout, et recrée le sélecteur. */
  protected readonly added = signal(0);
  protected readonly label = computed(() =>
    this.value().length === 0 ? 'Catégorie' : 'Ajouter une catégorie',
  );

  protected add(id: string | null): void {
    if (id && !this.value().includes(id)) {
      this.value.update((ids) => [...ids, id]);
    }
    this.added.update((n) => n + 1);
  }

  protected remove(id: string): void {
    this.value.update((ids) => ids.filter((c) => c !== id));
  }
}
