import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { firstValueFrom } from 'rxjs';

import { AttachedDocuments } from '../../api/models/attached-documents';
import { DocumentsService } from '../../api/services/documents.service';
import { ConfirmData } from '../../shared/confirm-dialog';

interface DeletionData extends ConfirmData {
  /** Nom de l'objet, pour la case : « rattachés seulement à <objet> ». */
  objectName: string;
  attached: AttachedDocuments;
}

/** Confirmation de suppression d'un objet qui a des documents rattachés (R-DOC-12). */
@Component({
  selector: 'app-deletion-dialog',
  imports: [FormsModule, MatButtonModule, MatCheckboxModule, MatDialogModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ data.title }}</h2>
    <mat-dialog-content>
      @if (data.message) {
        <p>{{ data.message }}</p>
      }
      @if (data.attached.total > 0) {
        <p>
          {{ data.attached.total }}
          {{
            data.attached.total > 1
              ? 'documents sont rattachés : ils restent'
              : 'document est rattaché : il reste'
          }}
          dans la bibliothèque.
        </p>
      }
      @if (data.attached.exclusive > 0) {
        <mat-checkbox [ngModel]="withDocuments()" (ngModelChange)="withDocuments.set($event)">
          Supprimer aussi
          {{
            data.attached.exclusive > 1
              ? 'les ' + data.attached.exclusive + ' documents rattachés'
              : 'le document rattaché'
          }}
          seulement à {{ data.objectName }}
        </mat-checkbox>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button
        mat-flat-button
        [class.destructive]="data.destructive"
        [mat-dialog-close]="{ deleteDocuments: withDocuments() }"
      >
        {{ data.action }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .destructive {
      --mat-button-filled-container-color: var(--mat-sys-error);
      --mat-button-filled-label-text-color: var(--mat-sys-on-error);
    }
  `,
})
export class DeletionDialog {
  protected readonly data = inject<DeletionData>(MAT_DIALOG_DATA);
  protected readonly withDocuments = signal(false);
}

export interface Deletion {
  /** Supprimer aussi les documents rattachés seulement à l'objet (R-DOC-12). */
  deleteDocuments: boolean;
}

/**
 * Confirme la suppression d'un objet. S'il a des documents rattachés, la confirmation le dit et propose de
 * supprimer ceux qui ne sont rattachés qu'à lui (R-DOC-12). Sans document, la confirmation habituelle, ou aucune si
 * `confirmWithoutDocuments` est faux. Résout à `null` si l'utilisateur renonce.
 */
export async function confirmDeletion(
  dialog: MatDialog,
  documents: DocumentsService,
  targetId: string,
  data: ConfirmData & { objectName: string; confirmWithoutDocuments?: boolean },
): Promise<Deletion | null> {
  const attached = await firstValueFrom(documents.attachedDocuments({ target_id: targetId }));
  if (attached.total === 0 && data.confirmWithoutDocuments === false) {
    return { deleteDocuments: false };
  }
  const ref = dialog.open<DeletionDialog, DeletionData, Deletion>(DeletionDialog, {
    data: { ...data, attached },
    maxWidth: '28rem',
  });
  return (await firstValueFrom(ref.afterClosed())) ?? null;
}
