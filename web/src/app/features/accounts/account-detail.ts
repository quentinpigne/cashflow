import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  linkedSignal,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatChipsModule } from '@angular/material/chips';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatDialog } from '@angular/material/dialog';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSortModule, Sort } from '@angular/material/sort';
import { MatTableModule } from '@angular/material/table';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatSelectModule } from '@angular/material/select';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { TransactionListItem } from '../../api/models/transaction-list-item';
import { AccountsService } from '../../api/services/accounts.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { formatDate, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { ClearingStatus } from '../../api/models/clearing-status';
import { ReconciliationService } from '../../api/services/reconciliation.service';
import { KIND_LABELS, STATUS_LABELS, hasComputedBalance } from '../../core/labels';
import { Layout } from '../../core/layout';
import { formatMoney, parseMoney } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { CategoriesStore } from '../../shared/stores';
import { TransactionDetailView } from '../transactions/transaction-detail';
import { AccountForm } from './account-form';
import { CloseAccountDialog } from './close-dialog';
import { InterestPanel } from '../savings/interest-panel';
import { ValuationPanel } from '../savings/valuation-panel';
import { ReconciliationPanel } from './reconciliation-panel';
import { AccountStatements } from './statements';

interface Filters {
  from: Date | null;
  to: Date | null;
  category: string | null;
  status: ClearingStatus | null;
  min: string;
  max: string;
  text: string;
}

const NO_FILTERS: Filters = {
  from: null,
  to: null,
  category: null,
  status: null,
  min: '',
  max: '',
  text: '',
};
const PAGE = 200;

/** E3 — Détail d'un compte (CPT-03, TRX-02). */
@Component({
  selector: 'app-account-detail',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatChipsModule,
    MatDatepickerModule,
    MatExpansionModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatProgressBarModule,
    MatSortModule,
    MatTableModule,
    MatCheckboxModule,
    MatSelectModule,
    MatTooltipModule,
    CategoryPicker,
    ReconciliationPanel,
    InterestPanel,
    ValuationPanel,
    AccountStatements,
    MoneyPipe,
    FrDatePipe,
    TransactionDetailView,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './account-detail.html',
  styleUrl: './account-detail.scss',
})
export class AccountDetailPage {
  private readonly accountsApi = inject(AccountsService);
  private readonly transactionsApi = inject(TransactionsService);
  private readonly reconciliation = inject(ReconciliationService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly form = inject(AccountForm);
  private readonly router = inject(Router);
  protected readonly layout = inject(Layout);
  protected readonly categories = inject(CategoriesStore);

  readonly id = input.required<string>();

  protected readonly kindLabels = KIND_LABELS;
  protected readonly columns = ['status', 'date', 'label', 'category', 'amount', 'balance'];
  protected readonly statuses: ClearingStatus[] = ['PENDING', 'CLEARED', 'RECONCILED'];
  protected readonly statusLabels = STATUS_LABELS;
  protected readonly computed = hasComputedBalance;
  protected readonly balanceDate = signal<Date | null>(null);
  protected readonly filters = signal<Filters>(NO_FILTERS);
  protected readonly showFilters = signal(false);
  protected readonly limit = signal(PAGE);
  protected readonly selected = signal<string | null>(null);
  /** Années de relevés dépliées : gardées ici, la section étant recréée à chaque rechargement. */
  protected readonly statementYears = linkedSignal<string, ReadonlySet<string>>({
    source: this.id,
    computation: () => new Set(),
  });
  protected readonly sort = signal<Sort>({ active: 'date', direction: 'desc' });
  protected readonly iban = signal<string | null>(null);
  protected readonly error = signal<string | null>(null);

  protected readonly account = rxResource({
    params: () => ({ id: this.id(), at: this.balanceDate(), version: this.version.value() }),
    stream: ({ params }) =>
      this.accountsApi.getAccount({
        id: params.id,
        at: params.at ? toIsoDate(params.at) : undefined,
      }),
  });

  protected readonly page = rxResource({
    params: () => ({
      id: this.id(),
      filters: this.filters(),
      limit: this.limit(),
      version: this.version.value(),
    }),
    stream: ({ params }) => {
      const f = params.filters;
      return this.transactionsApi.listTransactions({
        id: params.id,
        from: f.from ? toIsoDate(f.from) : undefined,
        to: f.to ? toIsoDate(f.to) : undefined,
        category_id: f.category ?? undefined,
        status: f.status ?? undefined,
        min_amount_minor: parseMoney(f.min) ?? undefined,
        max_amount_minor: parseMoney(f.max) ?? undefined,
        text: f.text.trim() || undefined,
        limit: params.limit,
      });
    },
  });

  protected readonly items = computed<TransactionListItem[]>(() => {
    const items = this.page.hasValue() ? this.page.value().items : [];
    const { active, direction } = this.sort();
    if (!direction || active === 'date') {
      return direction === 'asc' ? [...items].reverse() : items;
    }
    const factor = direction === 'asc' ? 1 : -1;
    const key = (t: TransactionListItem): string | number =>
      active === 'amount' ? t.amount_minor : active === 'label' ? t.label.toLowerCase() : '';
    return [...items].sort((a, b) => (key(a) > key(b) ? factor : key(a) < key(b) ? -factor : 0));
  });

  /** Liste mobile groupée par jour, dans l'ordre affiché. */
  protected readonly days = computed(() => {
    const groups: { date: string; items: TransactionListItem[] }[] = [];
    for (const item of this.items()) {
      const last = groups.at(-1);
      if (last?.date === item.date) {
        last.items.push(item);
      } else {
        groups.push({ date: item.date, items: [item] });
      }
    }
    return groups;
  });

  /** Filtres actifs, affichés en puces (ecrans.md E3). */
  protected readonly chips = computed(() => {
    const f = this.filters();
    const chips: { key: keyof Filters; label: string }[] = [];
    if (f.from) chips.push({ key: 'from', label: `Depuis le ${formatDate(toIsoDate(f.from))}` });
    if (f.to) chips.push({ key: 'to', label: `Jusqu'au ${formatDate(toIsoDate(f.to))}` });
    if (f.category) chips.push({ key: 'category', label: this.categories.label(f.category) });
    if (f.status) chips.push({ key: 'status', label: STATUS_LABELS[f.status] });
    if (parseMoney(f.min) !== null)
      chips.push({ key: 'min', label: `≥ ${formatMoney(parseMoney(f.min) ?? 0)}` });
    if (parseMoney(f.max) !== null)
      chips.push({ key: 'max', label: `≤ ${formatMoney(parseMoney(f.max) ?? 0)}` });
    if (f.text.trim()) chips.push({ key: 'text', label: `« ${f.text.trim()} »` });
    return chips;
  });

  protected setFilter<K extends keyof Filters>(key: K, value: Filters[K]): void {
    this.filters.update((f) => ({ ...f, [key]: value }));
    this.limit.set(PAGE);
  }

  protected clearFilter(key: keyof Filters): void {
    this.setFilter(key, NO_FILTERS[key]);
  }

  protected categoryText(item: TransactionListItem): string {
    if (item.transfer) {
      return `Virement ${item.transfer.is_outflow ? 'vers' : 'depuis'} ${item.transfer.counterpart_account_name}`;
    }
    if (item.splits.length > 1) {
      return 'Ventilée';
    }
    return this.categories.label(item.splits[0]?.category_id);
  }

  /** Pointage manuel (RAP-07). */
  protected async toggleCleared(item: TransactionListItem, cleared: boolean): Promise<void> {
    try {
      await firstValueFrom(this.reconciliation.setCleared({ id: item.id, body: { cleared } }));
      this.version.bump();
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  protected open(item: TransactionListItem): void {
    if (this.layout.mobile()) {
      void this.router.navigate(['/transactions', item.id]);
    } else {
      this.selected.set(item.id);
    }
  }

  protected edit(): void {
    if (this.account.hasValue()) {
      this.form.open(this.account.value());
    }
  }

  protected async close(): Promise<void> {
    if (!this.account.hasValue()) {
      return;
    }
    const ref = this.dialog.open(CloseAccountDialog, {
      data: this.account.value(),
      width: '28rem',
      maxWidth: '100vw',
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async reopen(): Promise<void> {
    await firstValueFrom(this.accountsApi.reopenAccount({ id: this.id() }));
    this.version.bump();
  }

  protected async remove(): Promise<void> {
    const name = this.account.hasValue() ? this.account.value().name : 'ce compte';
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${name} »`,
      message:
        'Le compte est vide : il sera supprimé définitivement, avec ses conditions et valorisations.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) {
      return;
    }
    try {
      await firstValueFrom(this.accountsApi.deleteAccount({ id: this.id() }));
      this.version.bump();
      await this.router.navigate(['/comptes']);
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  protected async revealIban(): Promise<void> {
    if (this.iban()) {
      this.iban.set(null);
      return;
    }
    const view = await firstValueFrom(this.accountsApi.revealIban({ id: this.id() }));
    this.iban.set(view.iban);
  }
}
