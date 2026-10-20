import { ChangeDetectionStrategy, Component, inject, input } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { firstValueFrom } from 'rxjs';

import { Checkpoint } from '../../api/models/checkpoint';
import { ReconciliationService } from '../../api/services/reconciliation.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { CheckpointFormDialog, GapHintsDialog, ReconcileDialog } from './reconciliation-dialogs';

/** E13 — Rapprochement d'un compte (RAP-01 à RAP-06). */
@Component({
  selector: 'app-reconciliation-panel',
  imports: [MatButtonModule, MatIconModule, MatMenuModule, MatTooltipModule, MoneyPipe, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card panel">
      <header>
        <h2>Rapprochement</h2>
        <button mat-stroked-button (click)="add()">
          <mat-icon>add</mat-icon>Point de contrôle
        </button>
      </header>
      @for (c of points.value() ?? []; track c.id) {
        <div class="row">
          <span
            class="state"
            [class.ok]="c.reconciled"
            [class.gap]="!c.reconciled && c.gap_minor !== 0"
          >
            <mat-icon [matTooltip]="stateLabel(c)">{{
              c.reconciled
                ? 'lock'
                : c.gap_minor === 0
                  ? 'check_circle'
                  : c.frozen
                    ? 'block'
                    : 'error'
            }}</mat-icon>
          </span>
          <span class="date">{{ c.date | frDate }}</span>
          <span class="amount">{{ c.balance_minor | money }}</span>
          <span class="amount" [class.negative]="c.gap_minor !== 0">{{
            c.gap_minor === 0
              ? 'écart nul'
              : 'écart ' + (c.gap_minor | money: 'EUR' : { signed: true })
          }}</span>
          <span class="actions">
            @if (!c.reconciled && c.gap_minor === 0 && !c.frozen) {
              <button mat-button (click)="reconcile(c)">Rapprocher</button>
            }
            @if (!c.reconciled && c.gap_minor !== 0) {
              <button mat-button (click)="hints(c)">Chercher l'écart</button>
            }
            <button
              mat-icon-button
              [matMenuTriggerFor]="menu"
              [attr.aria-label]="'Actions sur le point du ' + (c.date | frDate)"
            >
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              @if (c.reconciled) {
                <button mat-menu-item (click)="unlock(c)">
                  <mat-icon>lock_open</mat-icon>Déverrouiller
                </button>
              } @else {
                <button mat-menu-item (click)="remove(c)">
                  <mat-icon>delete</mat-icon>Supprimer
                </button>
              }
            </mat-menu>
          </span>
        </div>
      } @empty {
        <p class="muted">
          Aucun point de contrôle. Saisir le solde lu sur la banque, ou importer un relevé qui en
          contient.
        </p>
      }
    </section>
  `,
  styles: `
    .panel {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .row {
      display: grid;
      grid-template-columns: 2rem 1fr auto auto auto;
      align-items: center;
      gap: 0.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      padding: 0.25rem 0;
      @media (max-width: 599.98px) {
        grid-template-columns: 2rem 1fr auto;
        .actions {
          grid-column: 1 / -1;
          justify-self: end;
        }
      }
    }
    .state {
      color: var(--mat-sys-on-surface-variant);
      &.ok {
        color: var(--mat-sys-primary);
      }
      &.gap {
        color: var(--mat-sys-error);
      }
    }
  `,
})
export class ReconciliationPanel {
  private readonly api = inject(ReconciliationService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  readonly accountId = input.required<string>();

  protected readonly points = rxResource({
    params: () => ({ id: this.accountId(), version: this.version.value() }),
    stream: ({ params }) => this.api.listCheckpoints({ id: params.id }),
  });

  protected stateLabel(c: Checkpoint): string {
    if (c.reconciled) return 'Rapproché';
    if (c.frozen) return 'Écart figé par un rapprochement postérieur';
    return c.gap_minor === 0 ? 'Écart nul : prêt à rapprocher' : 'Écart à expliquer';
  }

  private async open<T>(component: T, data: unknown): Promise<void> {
    const ref = this.dialog.open(component as never, { data, width: '32rem', maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected add(): Promise<void> {
    return this.open(CheckpointFormDialog, this.accountId());
  }

  protected reconcile(c: Checkpoint): Promise<void> {
    return this.open(ReconcileDialog, c);
  }

  protected hints(c: Checkpoint): void {
    this.dialog.open(GapHintsDialog, { data: c, width: '36rem', maxWidth: '100vw' });
  }

  protected async unlock(c: Checkpoint): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Déverrouiller le rapprochement',
      message:
        'Ce point de contrôle et tous les suivants repassent à l’état non rapproché ; les transactions concernées redeviennent modifiables.',
      action: 'Déverrouiller',
    });
    if (ok) {
      await this.run(() => firstValueFrom(this.api.unlockCheckpoint({ id: c.id })));
    }
  }

  protected async remove(c: Checkpoint): Promise<void> {
    await this.run(() => firstValueFrom(this.api.deleteCheckpoint({ id: c.id })));
  }

  private async run(action: () => Promise<unknown>): Promise<void> {
    try {
      await action();
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }
}
