import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { RouterLink } from '@angular/router';

import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';
import { groupByFamily } from './account-groups';
import { AccountForm } from './account-form';

/** E2 — Comptes (CPT-01, CPT-03) : groupés par famille, clôturés à part. */
@Component({
  selector: 'app-accounts',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatExpansionModule,
    MatProgressBarModule,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './accounts.html',
  styleUrl: './accounts.scss',
})
export class AccountsPage {
  protected readonly store = inject(AccountsStore);
  private readonly form = inject(AccountForm);

  protected readonly groups = computed(() => groupByFamily(this.store.open()));
  protected readonly closed = computed(() => this.store.list().filter((a) => !!a.closed_on));

  protected create(): void {
    this.form.open();
  }
}
