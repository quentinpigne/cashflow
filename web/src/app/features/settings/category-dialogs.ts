import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';

import { Category } from '../../api/models/category';
import { CategoriesStore } from '../../shared/stores';

export interface NameData {
  title: string;
  name: string;
  action: string;
}

/** Saisie d'un nom de catégorie (création, renommage). */
@Component({
  selector: 'app-category-name',
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ data.title }}</h2>
    <mat-dialog-content>
      <mat-form-field class="full-width">
        <mat-label>Nom</mat-label>
        <input
          matInput
          [ngModel]="name()"
          (ngModelChange)="name.set($event)"
          maxlength="60"
          cdkFocusInitial
        />
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="name().trim()" [disabled]="!name().trim()">
        {{ data.action }}
      </button>
    </mat-dialog-actions>
  `,
})
export class CategoryNameDialog {
  protected readonly data = inject<NameData>(MAT_DIALOG_DATA);
  protected readonly name = signal(this.data.name);
}

/** « Fusionner dans… » (R-TRX-22) : cible de même nature, ni elle-même ni une de ses sous-catégories. */
@Component({
  selector: 'app-category-merge',
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatSelectModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Fusionner « {{ source.name }} »</h2>
    <mat-dialog-content>
      <p>
        Les {{ usage }} ventilations de cette catégorie{{
          source.children.length ? ' et de ses sous-catégories' : ''
        }}
        seront réaffectées à la catégorie choisie, puis « {{ source.name }} » sera supprimée.
      </p>
      <mat-form-field class="full-width">
        <mat-label>Fusionner dans</mat-label>
        <mat-select [ngModel]="target()" (ngModelChange)="target.set($event)">
          @for (option of options(); track option.id) {
            <mat-option [value]="option.id">{{ option.label }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="target()" [disabled]="!target()">
        Fusionner
      </button>
    </mat-dialog-actions>
  `,
})
export class CategoryMergeDialog {
  protected readonly source = inject<Category>(MAT_DIALOG_DATA);
  private readonly store = inject(CategoriesStore);
  protected readonly target = signal<string | null>(null);
  protected readonly usage =
    this.source.usage + this.source.children.reduce((total, c) => total + c.usage, 0);

  protected readonly options = computed(() =>
    this.store
      .flat()
      .filter(
        (c) =>
          c.kind === this.source.kind &&
          !c.archived &&
          c.id !== this.source.id &&
          c.parentId !== this.source.id,
      ),
  );
}
