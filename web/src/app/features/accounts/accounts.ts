import { CdkDragDrop, DragDropModule, moveItemInArray } from '@angular/cdk/drag-drop';
import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { Account } from '../../api/models/account';
import { AccountFamily } from '../../api/models/account-family';
import { AccountsService } from '../../api/services/accounts.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';
import { AccountGroup, groupByFamily } from './account-groups';
import { AccountForm } from './account-form';

/** E2 — Comptes (CPT-01, CPT-03, CPT-07) : groupés par famille, ordonnables, clôturés à part. */
@Component({
  selector: 'app-accounts',
  imports: [
    RouterLink,
    DragDropModule,
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
  private readonly api = inject(AccountsService);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);

  /** Mode « Ordonner » : les lignes se glissent au lieu d'ouvrir le compte. */
  protected readonly ordering = signal(false);
  /** Ordre déposé, affiché sans attendre le rechargement de la liste. */
  private readonly dropped = signal(new Map<AccountFamily, string[]>());

  protected readonly groups = computed(() =>
    groupByFamily(this.store.open()).map((group) => {
      const ids = this.dropped().get(group.family);
      return ids ? { ...group, accounts: inOrder(group.accounts, ids) } : group;
    }),
  );
  protected readonly closed = computed(() => this.store.list().filter((a) => !!a.closed_on));

  protected create(): void {
    this.form.open();
  }

  /** Glisser-déposer : nouvel ordre des comptes d'une famille (CPT-07). */
  protected async drop(group: AccountGroup, event: CdkDragDrop<Account[]>): Promise<void> {
    if (event.previousIndex === event.currentIndex) {
      return;
    }
    const ids = group.accounts.map((a) => a.id);
    moveItemInArray(ids, event.previousIndex, event.currentIndex);
    this.dropped.update((map) => new Map(map).set(group.family, ids));
    try {
      await firstValueFrom(this.api.reorderAccounts({ body: { ids } }));
    } catch (error) {
      this.dropped.update((map) => {
        const next = new Map(map);
        next.delete(group.family);
        return next;
      });
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
    this.version.bump();
  }
}

/** Comptes dans l'ordre des identifiants donnés ; les autres à la suite. */
function inOrder(accounts: Account[], ids: string[]): Account[] {
  const rank = (a: Account) => {
    const index = ids.indexOf(a.id);
    return index === -1 ? ids.length : index;
  };
  return [...accounts].sort((a, b) => rank(a) - rank(b));
}
