import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatRadioModule } from '@angular/material/radio';
import { MatSelectModule } from '@angular/material/select';
import { firstValueFrom, of } from 'rxjs';

import { TransactionDetail } from '../../api/models/transaction-detail';
import { TransactionsService } from '../../api/services/transactions.service';
import { errorMessage, problemOf } from '../../core/http-errors';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';

const CREATE = '__create__';

/**
 * « Marquer comme virement » (R-TRX-35, R-TRX-36) : choix du compte de contrepartie, puis d'une
 * transaction candidate, ou création de la jambe opposée.
 */
@Component({
  selector: 'app-mark-transfer',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatFormFieldModule,
    MatSelectModule,
    MatRadioModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Marquer comme virement</h2>
    <mat-dialog-content>
      <p class="muted">
        {{ transaction.label }} · {{ transaction.amount_minor | money: transaction.currency }}
        {{ transaction.amount_minor < 0 ? 'vers' : 'depuis' }} :
      </p>
      <mat-form-field class="full-width">
        <mat-label>Compte de contrepartie</mat-label>
        <mat-select [ngModel]="account()" (ngModelChange)="account.set($event); choice.set(null)">
          @for (a of counterparts(); track a.id) {
            <mat-option [value]="a.id">{{ a.name }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      @if (account()) {
        @if (candidates.isLoading()) {
          <p class="muted">Recherche des transactions correspondantes…</p>
        } @else {
          <mat-radio-group
            class="choices"
            [ngModel]="choice()"
            (ngModelChange)="choice.set($event)"
          >
            @for (c of candidates.value() ?? []; track c.id) {
              <mat-radio-button [value]="c.id">
                {{ c.date | frDate: 'short' }} · {{ c.label }} ·
                {{ c.amount_minor | money: transaction.currency }}
              </mat-radio-button>
            }
            <mat-radio-button [value]="create"
              >Créer la transaction opposée sur ce compte</mat-radio-button
            >
          </mat-radio-group>
        }
      }
      @if (needsConfirm()) {
        <p class="warning">
          La ventilation catégorisée sera remplacée par une ventilation sans catégorie : un virement
          n'est ni un revenu ni une dépense.
        </p>
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!choice() || busy()" (click)="save()">
        {{ needsConfirm() ? 'Confirmer' : 'Lier' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .choices {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
    }
    .warning {
      color: var(--mat-sys-tertiary);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class MarkTransferDialog {
  protected readonly transaction = inject<TransactionDetail>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<MarkTransferDialog, boolean>);
  private readonly api = inject(TransactionsService);
  private readonly accounts = inject(AccountsStore);

  protected readonly create = CREATE;
  protected readonly account = signal<string | null>(null);
  protected readonly choice = signal<string | null>(null);
  protected readonly needsConfirm = signal(false);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly counterparts = computed(() =>
    this.accounts
      .open()
      .filter(
        (a) => a.id !== this.transaction.account_id && a.currency === this.transaction.currency,
      ),
  );

  protected readonly candidates = rxResource({
    params: () => this.account(),
    stream: ({ params }) =>
      params
        ? this.api.transferCandidates({ id: this.transaction.id, account_id: params })
        : of([]),
  });

  protected async save(): Promise<void> {
    const account = this.account();
    const choice = this.choice();
    if (!account || !choice) {
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(
        this.api.markAsTransfer({
          id: this.transaction.id,
          body: {
            counterpart_account_id: account,
            counterpart_transaction_id: choice === CREATE ? null : choice,
            confirm: this.needsConfirm(),
          },
        }),
      );
      this.ref.close(true);
    } catch (error) {
      if (problemOf(error)?.code === 'TRANSFER_REPLACES_CATEGORIES') {
        this.needsConfirm.set(true);
      } else {
        this.error.set(errorMessage(error));
      }
    } finally {
      this.busy.set(false);
    }
  }
}
