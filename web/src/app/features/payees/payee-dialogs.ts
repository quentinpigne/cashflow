import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';

import { PayeeView } from '../../api/models/payee-view';
import { PayeesStore } from '../../shared/stores';

/** Champs d'un tiers ou d'un groupe saisis dans un dialogue. */
export interface PayeeFormValue {
  name: string;
  /** Tiers seulement : groupe choisi, ou nom d'un nouveau groupe à créer. */
  groupId: string | null;
  newGroup: string | null;
  note: string | null;
}

export interface PayeeFormData {
  title: string;
  action: string;
  /** Tiers (avec un choix de groupe) ou groupe. */
  kind: 'payee' | 'group';
  value: Partial<PayeeFormValue>;
}

/** Valeur de l'option « Nouveau groupe… ». */
const NEW_GROUP = '__new__';

/** Création ou modification d'un tiers ou d'un groupe de tiers (E31). */
@Component({
  selector: 'app-payee-form',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ data.title }}</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Nom</mat-label>
        <input
          matInput
          [ngModel]="name()"
          (ngModelChange)="name.set($event)"
          maxlength="80"
          cdkFocusInitial
        />
      </mat-form-field>
      @if (data.kind === 'payee') {
        <mat-form-field>
          <mat-label>Groupe</mat-label>
          <mat-select [ngModel]="group()" (ngModelChange)="group.set($event)">
            <mat-option [value]="null">Sans groupe</mat-option>
            @for (g of store.groups(); track g.id) {
              <mat-option [value]="g.id">{{ g.name }}</mat-option>
            }
            <mat-option [value]="newGroup">Nouveau groupe…</mat-option>
          </mat-select>
        </mat-form-field>
        @if (group() === newGroup) {
          <mat-form-field>
            <mat-label>Nom du nouveau groupe</mat-label>
            <input
              matInput
              [ngModel]="groupName()"
              (ngModelChange)="groupName.set($event)"
              maxlength="80"
            />
          </mat-form-field>
        }
      }
      <mat-form-field>
        <mat-label>Note</mat-label>
        <textarea
          matInput
          rows="2"
          [ngModel]="note()"
          (ngModelChange)="note.set($event)"
        ></textarea>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="result()" [disabled]="!valid()">
        {{ data.action }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      min-width: min(22rem, 80vw);
    }
  `,
})
export class PayeeFormDialog {
  protected readonly data = inject<PayeeFormData>(MAT_DIALOG_DATA);
  protected readonly store = inject(PayeesStore);
  protected readonly newGroup = NEW_GROUP;
  protected readonly name = signal(this.data.value.name ?? '');
  protected readonly group = signal<string | null>(this.data.value.groupId ?? null);
  protected readonly groupName = signal('');
  protected readonly note = signal(this.data.value.note ?? '');

  protected readonly valid = computed(
    () => !!this.name().trim() && (this.group() !== NEW_GROUP || !!this.groupName().trim()),
  );

  protected readonly result = computed<PayeeFormValue>(() => {
    const creating = this.group() === NEW_GROUP;
    return {
      name: this.name().trim(),
      groupId: creating ? null : this.group(),
      newGroup: creating ? this.groupName().trim() : null,
      note: this.note().trim() || null,
    };
  });
}

/** « Fusionner dans… » (R-TIE-7) : un autre tiers. */
@Component({
  selector: 'app-payee-merge',
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatSelectModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Fusionner « {{ source.name }} »</h2>
    <mat-dialog-content>
      <p>
        Ses {{ source.transaction_count }} transaction(s), ses récurrences et les règles qui
        l'affectent passeront au tiers choisi, puis « {{ source.name }} » sera supprimé.
      </p>
      <mat-form-field class="full-width">
        <mat-label>Fusionner dans</mat-label>
        <mat-select [ngModel]="target()" (ngModelChange)="target.set($event)">
          @for (p of options(); track p.id) {
            <mat-option [value]="p.id">{{ p.name }}</mat-option>
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
export class PayeeMergeDialog {
  protected readonly source = inject<PayeeView>(MAT_DIALOG_DATA);
  private readonly store = inject(PayeesStore);
  protected readonly target = signal<string | null>(null);
  protected readonly options = computed(() =>
    this.store.list().filter((p) => p.id !== this.source.id && !p.archived),
  );
}

/** Choix d'un groupe pour plusieurs tiers (E31). `null` : sans groupe. */
@Component({
  selector: 'app-payee-regroup',
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatSelectModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Changer de groupe ({{ count }} tiers)</h2>
    <mat-dialog-content>
      <mat-form-field class="full-width">
        <mat-label>Groupe</mat-label>
        <mat-select [ngModel]="group()" (ngModelChange)="group.set($event)">
          <mat-option [value]="none">Sans groupe</mat-option>
          @for (g of store.groups(); track g.id) {
            <mat-option [value]="g.id">{{ g.name }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="group()" [disabled]="!group()">Déplacer</button>
    </mat-dialog-actions>
  `,
})
export class PayeeRegroupDialog {
  protected readonly count = inject<number>(MAT_DIALOG_DATA);
  protected readonly store = inject(PayeesStore);
  /** Valeur de « Sans groupe », distincte de l'absence de choix. */
  protected readonly none = 'none';
  protected readonly group = signal<string | null>(null);
}
