import { ChangeDetectionStrategy, Component, inject, viewChild } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule } from '@angular/material/dialog';

import { AddressInput } from '../../api/models/address-input';
import { AddressView } from '../../api/models/address-view';
import { AddressFields } from '../../shared/address-fields';

export interface AddressDialogData {
  title: string;
  address: AddressView | null;
}

/** Résultat : l'adresse saisie, ou `null` pour la retirer. */
export interface AddressDialogResult {
  address: AddressInput | null;
}

/** Adresse d'une transaction ou d'une sélection (TRX-15, R-TRX-59) : la section Adresse d'un tiers. */
@Component({
  selector: 'app-transaction-address-dialog',
  imports: [AddressFields, MatButtonModule, MatDialogModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ data.title }}</h2>
    <mat-dialog-content class="form">
      <p class="muted">Elle l'emporte sur l'adresse du tiers dans les analyses par lieu.</p>
      <app-address-fields [initial]="data.address" />
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button
        mat-flat-button
        [mat-dialog-close]="{ address: fields().value() }"
        [disabled]="!!fields().problem() || !fields().value()"
      >
        Enregistrer
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      min-width: min(22rem, 80vw);
    }

    p {
      margin-top: 0;
    }
  `,
})
export class TransactionAddressDialog {
  protected readonly data = inject<AddressDialogData>(MAT_DIALOG_DATA);
  protected readonly fields = viewChild.required(AddressFields);
}
