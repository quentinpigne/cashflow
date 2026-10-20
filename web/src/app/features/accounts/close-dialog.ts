import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { firstValueFrom } from 'rxjs';

import { Account } from '../../api/models/account';
import { AccountsService } from '../../api/services/accounts.service';
import { toIsoDate } from '../../core/dates';
import { errorMessage, problemOf } from '../../core/http-errors';
import { formatMoney } from '../../core/money';

/** Clôture d'un compte (CPT-04) : solde non nul à confirmer explicitement (R-CPT-5). */
@Component({
  selector: 'app-close-account',
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
    <h2 mat-dialog-title>Clôturer « {{ account.name }} »</h2>
    <mat-dialog-content>
      <mat-form-field class="full-width">
        <mat-label>Date de clôture</mat-label>
        <input
          matInput
          [matDatepicker]="picker"
          [ngModel]="date()"
          (ngModelChange)="date.set($event); balance.set(null)"
        />
        <mat-datepicker-toggle matIconSuffix [for]="picker" />
        <mat-datepicker #picker />
      </mat-form-field>
      @if (balance(); as amount) {
        <p class="warning">
          Le solde à cette date vaut {{ amount }}. Clôturer quand même ? Le compte restera
          consultable.
        </p>
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="busy() || !date()" (click)="close()">
        {{ balance() ? 'Clôturer quand même' : 'Clôturer' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .warning {
      color: var(--mat-sys-tertiary);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class CloseAccountDialog {
  protected readonly account = inject<Account>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<CloseAccountDialog, boolean>);
  private readonly api = inject(AccountsService);

  protected readonly date = signal<Date | null>(new Date());
  /** Solde non nul renvoyé par le serveur : la clôture demande alors une confirmation. */
  protected readonly balance = signal<string | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected async close(): Promise<void> {
    const date = this.date();
    if (!date) {
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(
        this.api.closeAccount({
          id: this.account.id,
          body: { closed_on: toIsoDate(date), confirm: this.balance() !== null },
        }),
      );
      this.ref.close(true);
    } catch (error) {
      if (problemOf(error)?.code === 'CLOSING_BALANCE_NOT_ZERO') {
        const at = await firstValueFrom(
          this.api.getAccount({ id: this.account.id, at: toIsoDate(date) }),
        );
        this.balance.set(formatMoney(at.balance_minor, { currency: at.currency }));
      } else {
        this.error.set(errorMessage(error));
      }
    } finally {
      this.busy.set(false);
    }
  }
}
