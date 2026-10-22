import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { firstValueFrom } from 'rxjs';

import { Checkpoint } from '../../api/models/checkpoint';
import { ReconciliationService } from '../../api/services/reconciliation.service';
import { toIsoDate } from '../../core/dates';
import { errorMessage, problemOf } from '../../core/http-errors';
import { parseMoney } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';

/** Saisie d'un point de contrôle manuel (RAP-01). */
@Component({
  selector: 'app-checkpoint-form',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Point de contrôle</h2>
    <mat-dialog-content class="form">
      <p class="muted">Le solde lu sur la banque, en fin de journée.</p>
      <mat-form-field>
        <mat-label>Date</mat-label>
        <input
          matInput
          [matDatepicker]="picker"
          [ngModel]="date()"
          (ngModelChange)="date.set($event)"
        />
        <mat-datepicker-toggle matIconSuffix [for]="picker" />
        <mat-datepicker #picker />
      </mat-form-field>
      <mat-form-field>
        <mat-label>Solde</mat-label>
        <input
          matInput
          inputmode="decimal"
          [ngModel]="balance()"
          (ngModelChange)="balance.set($event)"
          class="amount"
        />
        <span matTextSuffix>&nbsp;€</span>
      </mat-form-field>
      @if (exists()) {
        <p class="warning">
          Un point de contrôle existe déjà à cette date avec un autre solde. Le remplacer ?
        </p>
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button (click)="save()" [disabled]="busy()">
        {{ exists() ? 'Remplacer' : 'Ajouter' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
    }
    .warning {
      color: var(--mat-sys-tertiary);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class CheckpointFormDialog {
  private readonly accountId = inject<string>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<CheckpointFormDialog, boolean>);
  private readonly api = inject(ReconciliationService);

  protected readonly date = signal<Date | null>(new Date());
  protected readonly balance = signal('');
  protected readonly exists = signal(false);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected async save(): Promise<void> {
    const date = this.date();
    const balance = parseMoney(this.balance());
    if (!date || balance === null) {
      this.error.set('Saisir une date et un solde, par exemple 1 234,56.');
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(
        this.api.createCheckpoint({
          id: this.accountId,
          body: { date: toIsoDate(date), balance_minor: balance, replace: this.exists() },
        }),
      );
      this.ref.close(true);
    } catch (error) {
      if (problemOf(error)?.code === 'CHECKPOINT_EXISTS') {
        this.exists.set(true);
      } else {
        this.error.set(errorMessage(error));
      }
    } finally {
      this.busy.set(false);
    }
  }
}

/** Rapprocher : ce qui sera rapproché et ce qui sera figé (R-RAP-4, R-RAP-5). */
@Component({
  selector: 'app-reconcile-dialog',
  imports: [MatDialogModule, MatButtonModule, MatProgressBarModule, MoneyPipe, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Rapprocher au {{ checkpoint.date | frDate }}</h2>
    <mat-dialog-content>
      @if (preview.value(); as p) {
        <p>
          Le solde calculé correspond au relevé ({{ checkpoint.balance_minor | money }}). Toutes les
          transactions jusqu'à cette date seront rapprochées et verrouillées.
        </p>
        @if (p.pending.length > 0) {
          <h3>{{ p.pending.length }} transaction(s) encore en attente, à vérifier</h3>
          <ul>
            @for (t of p.pending; track t.id) {
              <li>{{ t.date | frDate: 'short' }} · {{ t.label }} · {{ t.amount_minor | money }}</li>
            }
          </ul>
        }
        @if (p.frozen.length > 0) {
          <h3>Écarts qui seront figés</h3>
          <ul>
            @for (c of p.frozen; track c.id) {
              <li>{{ c.date | frDate }} : écart de {{ c.gap_minor | money }}</li>
            }
          </ul>
        }
      } @else if (preview.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button (click)="confirm()" [disabled]="busy() || !preview.hasValue()">
        Rapprocher
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    h3 {
      font: var(--mat-sys-title-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class ReconcileDialog {
  protected readonly checkpoint = inject<Checkpoint>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<ReconcileDialog, boolean>);
  private readonly api = inject(ReconciliationService);
  protected readonly preview = rxResource({
    stream: () => this.api.reconcilePreview({ id: this.checkpoint.id }),
  });
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected async confirm(): Promise<void> {
    this.busy.set(true);
    try {
      await firstValueFrom(this.api.reconcile({ id: this.checkpoint.id }));
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}

/** Chercher l'écart (R-RAP-9). */
@Component({
  selector: 'app-gap-hints',
  imports: [MatDialogModule, MatButtonModule, MatProgressBarModule, MoneyPipe, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>
      Écart de {{ checkpoint.gap_minor | money }} au {{ checkpoint.date | frDate }}
    </h2>
    <mat-dialog-content>
      @if (hints.value(); as h) {
        <p class="muted">
          Pistes parmi les transactions
          @if (h.from) {
            du {{ h.from | frDate: 'short' }}
          }
          jusqu'au {{ h.to | frDate: 'short' }}.
        </p>
        @if (h.pending.length > 0) {
          <h3>En attente (souvent des saisies jamais passées en banque)</h3>
          <ul>
            @for (t of h.pending; track t.id) {
              <li>{{ t.date | frDate: 'short' }} · {{ t.label }} · {{ t.amount_minor | money }}</li>
            }
          </ul>
        }
        @if (h.singles.length > 0) {
          <h3>Montant égal à l'écart</h3>
          <ul>
            @for (t of h.singles; track t.id) {
              <li>{{ t.date | frDate: 'short' }} · {{ t.label }} · {{ t.amount_minor | money }}</li>
            }
          </ul>
        }
        @if (h.sign_errors.length > 0) {
          <h3>Montant égal au double de l'écart : erreur de signe ?</h3>
          <ul>
            @for (t of h.sign_errors; track t.id) {
              <li>{{ t.date | frDate: 'short' }} · {{ t.label }} · {{ t.amount_minor | money }}</li>
            }
          </ul>
        }
        @if (h.pairs.length > 0) {
          <h3>Deux transactions dont la somme égale l'écart</h3>
          <ul>
            @for (pair of h.pairs; track $index) {
              <li>
                {{ pair[0].label }} ({{ pair[0].amount_minor | money }}) + {{ pair[1].label }} ({{
                  pair[1].amount_minor | money
                }})
              </li>
            }
          </ul>
        }
        @if (!h.pending.length && !h.singles.length && !h.sign_errors.length && !h.pairs.length) {
          <p>
            Aucune piste évidente : une transaction manque peut-être, ou un montant a été mal saisi.
          </p>
        }
      } @else if (hints.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Fermer</button>
    </mat-dialog-actions>
  `,
  styles: `
    h3 {
      font: var(--mat-sys-title-small);
      margin-bottom: 0.25rem;
    }
  `,
})
export class GapHintsDialog {
  protected readonly checkpoint = inject<Checkpoint>(MAT_DIALOG_DATA);
  private readonly api = inject(ReconciliationService);
  protected readonly hints = rxResource({
    stream: () => this.api.gapHints({ id: this.checkpoint.id }),
  });
}
