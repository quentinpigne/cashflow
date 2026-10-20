import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { BatchSummary } from '../../api/models/batch-summary';
import { ImportsService } from '../../api/services/imports.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { BATCH_STATUS_LABELS } from '../../core/labels';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';

/** E12 — Historique des imports (IMP-06, IMP-07, R-IMP-21). */
@Component({
  selector: 'app-import-history',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatMenuModule,
    MatProgressBarModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Import</h1>
        <a mat-flat-button routerLink="/import/nouveau"
          ><mat-icon>upload_file</mat-icon>Importer un relevé</a
        >
      </div>
      @if (batches.isLoading() && !batches.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      }
      <div class="card">
        @for (b of batches.value() ?? []; track b.id) {
          <div class="row">
            <span class="main">
              <span>
                <strong>{{ b.account_name }}</strong> · {{ b.file_name }}
              </span>
              <span class="muted small">
                {{ statusLabels[b.status] }}
                @if (b.period_start && b.period_end) {
                  · du {{ b.period_start | frDate: 'short' }} au {{ b.period_end | frDate }}
                }
                · {{ b.line_count }} lignes
                @if (b.status === 'COMMITTED' || b.status === 'REVERTED') {
                  · {{ b.to_create }} créées, {{ b.to_match }} pointées
                }
                @if (b.error_count) {
                  · {{ b.error_count }} en erreur
                }
              </span>
            </span>
            @if (b.status === 'REVIEWING') {
              <a mat-stroked-button [routerLink]="['/import', b.id]">Reprendre la revue</a>
            } @else {
              <a mat-button [routerLink]="['/import', b.id]">Voir</a>
            }
            <button
              mat-icon-button
              [matMenuTriggerFor]="menu"
              [attr.aria-label]="'Actions sur ' + b.file_name"
            >
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              @if (b.has_file) {
                <a mat-menu-item [href]="'/api/v1/imports/' + b.id + '/file'" download>
                  <mat-icon>download</mat-icon>Télécharger le fichier source
                </a>
                <button mat-menu-item (click)="deleteFile(b)">
                  <mat-icon>delete_sweep</mat-icon>Supprimer le fichier source
                </button>
              }
              @if (b.status === 'COMMITTED') {
                <button mat-menu-item (click)="revert(b)">
                  <mat-icon>undo</mat-icon>Annuler ce lot
                </button>
              }
            </mat-menu>
          </div>
        } @empty {
          <p class="empty">Aucun import pour l'instant.</p>
        }
      </div>
    </div>
  `,
  styles: `
    .row {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.5rem 0;
      border-top: 1px solid var(--mat-sys-outline-variant);
      .main {
        flex: 1;
        display: flex;
        flex-direction: column;
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class ImportHistoryPage {
  private readonly api = inject(ImportsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  protected readonly statusLabels = BATCH_STATUS_LABELS;
  protected readonly batches = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listImports(),
  });

  protected async revert(b: BatchSummary): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Annuler ce lot',
      message:
        'Les transactions créées par cet import seront supprimées, les transactions pointées reviendront à leur état précédent, et les points de contrôle du relevé seront retirés.',
      action: 'Annuler le lot',
      destructive: true,
    });
    if (ok) {
      await this.run(() => firstValueFrom(this.api.revertImport({ id: b.id })), 'Lot annulé');
    }
  }

  protected async deleteFile(b: BatchSummary): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le fichier source',
      message:
        'Le lot et ses lignes restent ; seul le fichier conservé sur le serveur est supprimé.',
      action: 'Supprimer',
    });
    if (ok) {
      await this.run(
        () => firstValueFrom(this.api.deleteImportFile({ id: b.id })),
        'Fichier supprimé',
      );
    }
  }

  private async run(action: () => Promise<unknown>, message: string): Promise<void> {
    try {
      await action();
      this.version.bump();
      this.snack.open(message, undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }
}
