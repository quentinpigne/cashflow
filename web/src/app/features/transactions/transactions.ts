import {
  ChangeDetectionStrategy,
  Component,
  OnInit,
  computed,
  inject,
  input,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ClearingStatus } from '../../api/models/clearing-status';
import { TransactionListItem } from '../../api/models/transaction-list-item';
import { BudgetsService } from '../../api/services/budgets.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { STATUS_LABELS } from '../../core/labels';
import { Layout } from '../../core/layout';
import { parseMoney } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { PayeePicker } from '../../shared/payee-picker';
import { AccountsStore, CategoriesStore, PayeesStore, ProjectsStore } from '../../shared/stores';

const PAGE = 200;

/** E14 — Transactions, tous comptes confondus (TRX-08, TRX-06). */
@Component({
  selector: 'app-transactions',
  imports: [
    FormsModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
    MatSlideToggleModule,
    CategoryPicker,
    PayeePicker,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './transactions.html',
  styleUrl: './transactions.scss',
})
export class TransactionsPage implements OnInit {
  private readonly api = inject(TransactionsService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);
  private readonly snack = inject(MatSnackBar);
  protected readonly layout = inject(Layout);
  protected readonly accounts = inject(AccountsStore);
  protected readonly categories = inject(CategoriesStore);
  protected readonly payees = inject(PayeesStore);

  // Filtres reçus dans l'URL (lien depuis les analyses, E15, et la fiche d'un tiers, E32).
  readonly categorie = input<string>();
  readonly du = input<string>();
  readonly au = input<string>();
  readonly a_categoriser = input<string>();
  readonly tiers = input<string>();
  readonly groupe = input<string>();

  protected readonly statusLabels = STATUS_LABELS;
  protected readonly statuses: ClearingStatus[] = ['PENDING', 'CLEARED', 'RECONCILED'];
  protected readonly accountIds = signal<string[]>([]);
  protected readonly from = signal<Date | null>(null);
  protected readonly to = signal<Date | null>(null);
  protected readonly category = signal<string | null>(null);
  protected readonly status = signal<ClearingStatus | null>(null);
  protected readonly min = signal('');
  protected readonly max = signal('');
  protected readonly text = signal('');
  protected readonly uncategorized = signal(false);
  /** Filtres par tiers (TIE-04). */
  protected readonly payee = signal<string | null>(null);
  protected readonly payeeGroup = signal<string | null>(null);
  protected readonly withoutPayee = signal(false);
  /** Avec ou sans justificatif (DOC-04). */
  protected readonly withDocuments = signal<boolean | null>(null);
  protected readonly bulkPayee = signal<string | null>(null);
  protected readonly limit = signal(PAGE);
  protected readonly selection = signal<Set<string>>(new Set());
  protected readonly projects = inject(ProjectsStore);
  private readonly budgets = inject(BudgetsService);
  protected readonly NO_PROJECT = 'none';
  protected readonly bulkProject = signal<string | null>(null);
  protected readonly bulkCategory = signal<string | null>(null);
  protected readonly busy = signal(false);
  /** Sur mobile, les filtres sont repliés derrière un bouton (comme sur E3). */
  protected readonly showFilters = signal(false);

  protected readonly page = rxResource({
    params: () => {
      return {
        accounts: this.accountIds().join(',') || undefined,
        from: this.from() ? toIsoDate(this.from() as Date) : undefined,
        to: this.to() ? toIsoDate(this.to() as Date) : undefined,
        category_id: this.category() ?? undefined,
        status: this.status() ?? undefined,
        min_amount_minor: parseMoney(this.min()) ?? undefined,
        max_amount_minor: parseMoney(this.max()) ?? undefined,
        text: this.text().trim() || undefined,
        uncategorized: this.uncategorized() || undefined,
        payee_id: this.payee() ?? undefined,
        payee_group_id: this.payeeGroup() ?? undefined,
        without_payee: this.withoutPayee() || undefined,
        with_documents: this.withDocuments() ?? undefined,
        limit: this.limit(),
        version: this.version.value(),
      };
    },
    // `version` ne sert qu'à relancer la requête ; le client généré l'ignore.
    stream: ({ params }) => this.api.listAllTransactions(params),
  });

  protected readonly items = computed<TransactionListItem[]>(() =>
    this.page.hasValue() ? this.page.value().items : [],
  );
  /** Les virements ne prennent ni catégorie ni tiers (R-TRX-32, R-TIE-11). */
  protected readonly selectableIds = computed(() =>
    this.items()
      .filter((i) => !i.transfer)
      .map((i) => i.id),
  );
  protected readonly allSelected = computed(
    () =>
      this.selectableIds().length > 0 &&
      this.selectableIds().every((id) => this.selection().has(id)),
  );

  /** Les filtres de l'URL s'appliquent au chargement. */
  ngOnInit(): void {
    if (this.categorie()) this.category.set(this.categorie() ?? null);
    if (this.du()) this.from.set(fromIsoDate(this.du() as string));
    if (this.au()) this.to.set(fromIsoDate(this.au() as string));
    if (this.a_categoriser()) this.uncategorized.set(true);
    if (this.tiers()) this.payee.set(this.tiers() ?? null);
    if (this.groupe()) this.payeeGroup.set(this.groupe() ?? null);
  }

  protected categoryText(item: TransactionListItem): string {
    if (item.transfer) {
      return `Virement ${item.transfer.is_outflow ? 'vers' : 'depuis'} ${item.transfer.counterpart_account_name}`;
    }
    const category =
      item.splits.length > 1 ? 'Ventilée' : this.categories.label(item.splits[0]?.category_id);
    return item.payee_id ? `${this.payees.name(item.payee_id)} · ${category}` : category;
  }

  protected toggle(id: string, checked: boolean): void {
    this.selection.update((s) => {
      const next = new Set(s);
      if (checked) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  }

  protected toggleAll(checked: boolean): void {
    this.selection.set(checked ? new Set(this.selectableIds()) : new Set());
  }

  protected clearSelection(): void {
    this.selection.set(new Set());
  }

  protected reset(): void {
    this.limit.set(PAGE);
    this.selection.set(new Set());
  }

  protected open(item: TransactionListItem): void {
    void this.router.navigate(['/transactions', item.id]);
  }

  /** Rattachement en masse à un projet, ou détachement (BUD-06). */
  protected async attachSelection(): Promise<void> {
    const choice = this.bulkProject();
    if (!choice || this.selection().size === 0) {
      return;
    }
    this.busy.set(true);
    try {
      const result = await firstValueFrom(
        this.budgets.attachToProject({
          body: {
            transaction_ids: [...this.selection()],
            project_id: choice === this.NO_PROJECT ? null : choice,
          },
        }),
      );
      this.selection.set(new Set());
      this.version.bump();
      const skipped = result.skipped ? ` (${result.skipped} virement(s) ignoré(s))` : '';
      const verb = choice === this.NO_PROJECT ? 'détachée(s)' : 'rattachée(s)';
      this.snack.open(`${result.attached} transaction(s) ${verb}${skipped}`, undefined, {
        duration: 4000,
      });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    } finally {
      this.busy.set(false);
    }
  }

  /** Tiers affecté (ou retiré) en masse ; les virements sont ignorés (TIE-02, R-TIE-11). */
  protected async setPayeeOfSelection(remove = false): Promise<void> {
    const payee = remove ? null : this.bulkPayee();
    if ((!payee && !remove) || this.selection().size === 0) {
      return;
    }
    this.busy.set(true);
    try {
      const result = await firstValueFrom(
        this.api.setTransactionsPayee({
          body: { transaction_ids: [...this.selection()], payee_id: payee },
        }),
      );
      this.selection.set(new Set());
      this.version.bump();
      const skipped = result.skipped ? ` (${result.skipped} virement(s) ignoré(s))` : '';
      const verb = remove ? 'sans tiers' : `affectée(s) à ${this.payees.name(payee)}`;
      this.snack.open(`${result.updated} transaction(s) ${verb}${skipped}`, undefined, {
        duration: 4000,
      });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    } finally {
      this.busy.set(false);
    }
  }

  /** Catégorisation en masse (TRX-06). */
  protected async categorizeSelection(): Promise<void> {
    const category = this.bulkCategory();
    if (!category || this.selection().size === 0) {
      return;
    }
    this.busy.set(true);
    try {
      const result = await firstValueFrom(
        this.api.categorizeTransactions({
          body: { transaction_ids: [...this.selection()], category_id: category },
        }),
      );
      this.selection.set(new Set());
      this.version.bump();
      const skipped = result.skipped
        ? ` (${result.skipped} ignorée(s) : virements ou ventilées)`
        : '';
      this.snack.open(`${result.categorized} transaction(s) catégorisée(s)${skipped}`, undefined, {
        duration: 4000,
      });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    } finally {
      this.busy.set(false);
    }
  }
}
