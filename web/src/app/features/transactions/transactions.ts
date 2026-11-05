import {
  ChangeDetectionStrategy,
  Component,
  OnInit,
  computed,
  effect,
  ElementRef,
  inject,
  input,
  signal,
  viewChild,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { ActivatedRoute, Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ClearingStatus } from '../../api/models/clearing-status';
import { TransactionListItem } from '../../api/models/transaction-list-item';
import { BudgetsService } from '../../api/services/budgets.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import {
  currentMonth,
  formatMonthKey,
  fromIsoDate,
  monthsBetween,
  shiftMonth,
  toIsoDate,
} from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { STATUS_LABELS } from '../../core/labels';
import { Layout } from '../../core/layout';
import { parseMoney } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { PayeePicker } from '../../shared/payee-picker';
import {
  AccountsStore,
  CategoriesStore,
  MembersStore,
  PayeesStore,
  ProjectsStore,
} from '../../shared/stores';
import { MemberTag } from '../../shared/member-tag';
import { RefundMark } from '../../shared/refund-mark';
import { loadPageSize, PAGE_SIZES, savePageSize } from '../../core/page-size';

/** Taille de page par défaut de la liste transverse. */
const PAGE = 100;

/** E14 — Transactions, tous comptes confondus (TRX-08, TRX-06). */
@Component({
  selector: 'app-transactions',
  imports: [
    RefundMark,
    MatPaginatorModule,
    MatTooltipModule,
    MemberTag,
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
  protected readonly members = inject(MembersStore);
  private readonly api = inject(TransactionsService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);
  private readonly snack = inject(MatSnackBar);
  protected readonly layout = inject(Layout);
  protected readonly accounts = inject(AccountsStore);
  protected readonly categories = inject(CategoriesStore);
  protected readonly payees = inject(PayeesStore);

  // Filtres reçus dans l'URL (lien depuis les analyses, E15, et la fiche d'un tiers, E32). Les filtres
  // saisis y sont recopiés : le retour depuis une transaction retrouve la liste telle qu'on l'a laissée.
  readonly comptes = input<string>();
  readonly categorie = input<string>();
  readonly du = input<string>();
  readonly au = input<string>();
  readonly a_categoriser = input<string>();
  readonly tiers = input<string>();
  readonly groupe = input<string>();
  /** Mois d'analyse `AAAA-MM` : lien depuis le budget ou les analyses (R-TRX-58). */
  readonly mois = input<string>();
  readonly statut = input<string>();
  readonly min_montant = input<string>();
  readonly max_montant = input<string>();
  readonly texte = input<string>();
  readonly sans_tiers = input<string>();
  readonly justificatif = input<string>();
  /** Page affichée, à partir de 1. */
  readonly page = input<string>();
  private readonly route = inject(ActivatedRoute);
  private urlTimer: ReturnType<typeof setTimeout> | undefined;

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
  /** Transactions qui comptent dans ce mois, quelle que soit leur date (R-TRX-58). */
  protected readonly analysisMonth = signal<string | null>(null);
  /** Mois d'analyse choisi en masse : `DATE` pour le mois de la date (TRX-13). */
  protected readonly bulkMonth = signal<string | null>(null);
  protected readonly DATE_MONTH = 'date';
  protected readonly monthChoices = monthsBetween(
    shiftMonth(currentMonth(), -12),
    shiftMonth(currentMonth(), 12),
  );
  protected readonly monthLabel = formatMonthKey;
  protected readonly bulkPayee = signal<string | null>(null);
  protected readonly pageSizes = PAGE_SIZES;
  protected readonly pageSize = signal(loadPageSize('transactions', PAGE));
  /** Page affichée, à partir de 0 ; revient à la première à chaque changement de filtre. */
  protected readonly pageIndex = signal(0);
  private readonly listTop = viewChild<ElementRef<HTMLElement>>('listTop');
  protected readonly selection = signal<Set<string>>(new Set());
  protected readonly projects = inject(ProjectsStore);
  private readonly budgets = inject(BudgetsService);
  protected readonly NO_PROJECT = 'none';
  protected readonly bulkProject = signal<string | null>(null);
  protected readonly bulkCategory = signal<string | null>(null);
  /** Membre choisi en masse (R-MBR-5) ; `NO_MEMBER` : le foyer. */
  protected readonly bulkMember = signal<string | null>(null);
  protected readonly NO_MEMBER = 'none';
  protected readonly busy = signal(false);
  /** Sur mobile, les filtres sont repliés derrière un bouton (comme sur E3). */
  protected readonly showFilters = signal(false);

  protected readonly result = rxResource({
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
        analysis_month: this.analysisMonth() ?? undefined,
        page: this.pageIndex() + 1,
        limit: this.pageSize(),
        version: this.version.value(),
      };
    },
    // `version` ne sert qu'à relancer la requête ; le client généré l'ignore.
    stream: ({ params }) => this.api.listAllTransactions(params),
  });

  protected readonly items = computed<TransactionListItem[]>(() =>
    this.result.hasValue() ? this.result.value().items : [],
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

  constructor() {
    // Les filtres et la page sont recopiés dans l'URL, sans créer d'entrée d'historique ; l'écriture est
    // différée pour ne pas réécrire l'URL à chaque frappe dans le champ texte.
    effect((onCleanup) => {
      const params = this.queryParams();
      clearTimeout(this.urlTimer);
      this.urlTimer = setTimeout(() => {
        // Une navigation en cours (ouverture d'une transaction) ne doit pas être remplacée.
        if (this.router.currentNavigation()) return;
        const target = this.router.createUrlTree([], {
          relativeTo: this.route,
          queryParams: params,
        });
        if (this.router.serializeUrl(target) === this.router.url) return;
        void this.router.navigateByUrl(target, { replaceUrl: true });
      }, 300);
      onCleanup(() => clearTimeout(this.urlTimer));
    });
  }

  private readonly queryParams = computed(() => {
    const from = this.from();
    const to = this.to();
    const withDocuments = this.withDocuments();
    return {
      comptes: this.accountIds().join(',') || null,
      du: from ? toIsoDate(from) : null,
      au: to ? toIsoDate(to) : null,
      categorie: this.category(),
      statut: this.status(),
      min_montant: this.min().trim() || null,
      max_montant: this.max().trim() || null,
      texte: this.text() || null,
      a_categoriser: this.uncategorized() ? '1' : null,
      tiers: this.payee(),
      groupe: this.payeeGroup(),
      sans_tiers: this.withoutPayee() ? '1' : null,
      justificatif: withDocuments === null ? null : withDocuments ? 'avec' : 'sans',
      mois: this.analysisMonth(),
      page: this.pageIndex() > 0 ? String(this.pageIndex() + 1) : null,
    };
  });

  /** Les filtres de l'URL s'appliquent au chargement. */
  ngOnInit(): void {
    if (this.comptes()) this.accountIds.set((this.comptes() as string).split(','));
    if (this.statut() && this.statuses.includes(this.statut() as ClearingStatus)) {
      this.status.set(this.statut() as ClearingStatus);
    }
    if (this.min_montant()) this.min.set(this.min_montant() as string);
    if (this.max_montant()) this.max.set(this.max_montant() as string);
    if (this.texte()) this.text.set(this.texte() as string);
    if (this.sans_tiers()) this.withoutPayee.set(true);
    if (this.justificatif()) this.withDocuments.set(this.justificatif() === 'avec');
    const page = Number(this.page());
    if (Number.isInteger(page) && page > 1) this.pageIndex.set(page - 1);
    if (this.categorie()) this.category.set(this.categorie() ?? null);
    if (this.du()) this.from.set(fromIsoDate(this.du() as string));
    if (this.au()) this.to.set(fromIsoDate(this.au() as string));
    if (this.a_categoriser()) this.uncategorized.set(true);
    if (this.tiers()) this.payee.set(this.tiers() ?? null);
    if (this.groupe()) this.payeeGroup.set(this.groupe() ?? null);
    if (this.mois()) this.analysisMonth.set(this.mois() ?? null);
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

  /** Change de page, ou de taille de page (retenue dans ce navigateur), et remonte en tête de liste. */
  protected changePage(event: PageEvent): void {
    if (event.pageSize !== this.pageSize()) {
      savePageSize('transactions', event.pageSize);
      this.pageSize.set(event.pageSize);
      this.pageIndex.set(0);
    } else {
      this.pageIndex.set(event.pageIndex);
    }
    this.selection.set(new Set());
    this.listTop()?.nativeElement.scrollIntoView({ block: 'start' });
  }

  protected reset(): void {
    this.pageIndex.set(0);
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

  /** Membre affecté (ou retiré) en masse sur toutes les ventilations ; les virements sont ignorés (R-MBR-5). */
  protected async setMemberOfSelection(): Promise<void> {
    const choice = this.bulkMember();
    if (!choice || this.selection().size === 0) {
      return;
    }
    const member = choice === this.NO_MEMBER ? null : choice;
    // Le nom est lu avant l'appel : la liste des membres se recharge ensuite.
    const verb = `affectée(s) à ${member ? this.members.name(member) : 'Foyer'}`;
    this.busy.set(true);
    try {
      const result = await firstValueFrom(
        this.api.setTransactionsMember({
          body: { transaction_ids: [...this.selection()], member_id: member },
        }),
      );
      this.selection.set(new Set());
      this.version.bump();
      const skipped = result.skipped ? ` (${result.skipped} virement(s) ignoré(s))` : '';
      this.snack.open(`${result.updated} transaction(s) ${verb}${skipped}`, undefined, {
        duration: 4000,
      });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    } finally {
      this.busy.set(false);
    }
  }

  /** Mois d'analyse en masse (TRX-13, R-TRX-57). */
  protected async setMonthOfSelection(): Promise<void> {
    const choice = this.bulkMonth();
    if (!choice || this.selection().size === 0) {
      return;
    }
    this.busy.set(true);
    try {
      const month = choice === this.DATE_MONTH ? null : choice;
      const result = await firstValueFrom(
        this.api.setTransactionsAnalysisMonth({
          body: { transaction_ids: [...this.selection()], analysis_month: month },
        }),
      );
      this.selection.set(new Set());
      this.version.bump();
      const where = month
        ? `en ${formatMonthKey(month).toLowerCase()}`
        : 'sur le mois de leur date';
      this.snack.open(`${result.updated} transaction(s) comptent ${where}`, undefined, {
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
