import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  output,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { TransactionDetail } from '../../api/models/transaction-detail';
import { LoanPaymentProposal } from '../../api/models/loan-payment-proposal';
import { LoansService } from '../../api/services/loans.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, todayIso, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { ReconciliationService } from '../../api/services/reconciliation.service';
import { ORIGIN_LABELS, STATUS_LABELS } from '../../core/labels';
import { abs, negate, parseMoney, toEditable } from '../../core/money';
import { decodeAttachment, encodeAttachment } from '../../core/attachments';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { PayeePicker } from '../../shared/payee-picker';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AttachmentsStore, CategoriesStore, PayeesStore, ProjectsStore } from '../../shared/stores';
import { MarkTransferDialog } from './mark-transfer';
import { RefundPanel } from './refund-panel';
import { SplitDraft, SplitEditor, SplitEditorData } from './split-editor';
import { DocumentSection } from '../documents/document-section';
import { confirmDeletion } from '../documents/delete-confirmation';
import { DocumentsService } from '../../api/services/documents.service';
import { LinkTarget } from '../../api/models/link-target';
import { proposedLine } from '../budget/project-lines';

type Direction = 'out' | 'in';

/** E7 — Détail d'une transaction (TRX-01, TRX-03, TRX-05). */
@Component({
  selector: 'app-transaction-detail',
  imports: [
    MatSelectModule,
    ReactiveFormsModule,
    RouterLink,
    MatButtonModule,
    MatButtonToggleModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatProgressBarModule,
    CategoryPicker,
    PayeePicker,
    RefundPanel,
    DocumentSection,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './transaction-detail.html',
  styleUrl: './transaction-detail.scss',
})
export class TransactionDetailView {
  private readonly api = inject(TransactionsService);
  private readonly documents = inject(DocumentsService);
  private readonly payeesStore = inject(PayeesStore);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  protected readonly categories = inject(CategoriesStore);

  readonly transactionId = input.required<string>();
  /** Affiché dans un panneau : bouton de fermeture au lieu du retour. */
  readonly embedded = input(false);
  readonly closed = output<void>();

  protected readonly originLabels = ORIGIN_LABELS;
  protected readonly statusLabels = STATUS_LABELS;
  private readonly reconciliation = inject(ReconciliationService);
  protected readonly detail = rxResource({
    params: () => ({ id: this.transactionId(), version: this.version.value() }),
    stream: ({ params }) => this.api.getTransaction({ id: params.id }),
  });

  private readonly loans = inject(LoansService);
  /** Échéance de prêt à laquelle correspond un débit pas encore ventilé (R-CPT-17). */
  protected readonly loanProposal = rxResource({
    params: () => {
      const t = this.value();
      return t && !t.transfer && !t.loan_payment && t.amount_minor < 0
        ? { id: t.id, version: this.version.value() }
        : undefined;
    },
    stream: ({ params }) => this.loans.transactionLoanProposal({ id: params.id }),
  });

  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly direction = signal<Direction>('out');
  protected readonly category = signal<string | null>(null);
  /** Tiers, modifiable même sur une transaction rapprochée (R-TIE-12). */
  protected readonly payee = signal<string | null>(null);
  /** Projet de la transaction (BUD-06) : l'interface le porte sur toutes ses ventilations. */
  protected readonly project = signal<string | null>(null);
  protected readonly projects = inject(ProjectsStore);
  /** Projets proposés : ceux en cours, plus le projet actuel même clôturé (R-BUD-24). */
  protected readonly projectOptions = computed(() => {
    const current = this.project();
    return this.projects.list().filter((p) => p.active || p.id === current);
  });
  /** Poste de la transaction dans son projet (BUD-11), le même pour toutes ses ventilations, ou `MIXED`. */
  protected readonly line = signal<string | null>(null);
  protected readonly MIXED = 'mixed';
  private initialLine: string | null = null;
  protected readonly projectLines = computed(() => {
    const id = this.project();
    return id ? (this.projects.byId().get(id)?.lines ?? []) : [];
  });
  /** Ventilation modifiée, pas encore enregistrée. */
  protected readonly pendingSplits = signal<SplitDraft[] | null>(null);

  private readonly fb = inject(FormBuilder).nonNullable;
  protected readonly form = this.fb.group({
    amount: [
      '',
      [
        Validators.required,
        (c: { value: string }) => {
          const value = parseMoney(c.value);
          return value === null || value <= 0 ? { amount: true } : null;
        },
      ],
    ],
    date: [new Date(), Validators.required],
    label: ['', [Validators.required, Validators.maxLength(200)]],
    note: [''],
  });

  protected readonly value = computed(() => (this.detail.hasValue() ? this.detail.value() : null));
  protected readonly splits = computed(() => this.pendingSplits() ?? this.value()?.splits ?? []);
  protected readonly isSplit = computed(() => this.splits().length > 1);
  private readonly attachments = inject(AttachmentsStore);

  /** Libellé d'une ventilation : catégorie, remboursement d'un prêt ou acquisition d'un bien (R-TRX-37). */
  protected splitLabel(split: {
    category_id?: string | null;
    loan_id?: string | null;
    asset_id?: string | null;
  }): string {
    const value =
      'loan_id' in split || 'asset_id' in split ? encodeAttachment(split) : split.category_id;
    const attachment = value ? this.attachments.byValue().get(value) : undefined;
    if (attachment) return attachment.label;
    if (value?.startsWith('loan:')) return 'Remboursement de prêt';
    if (value?.startsWith('asset:')) return 'Acquisition de bien';
    return this.categories.label(value);
  }

  constructor() {
    // Le formulaire reprend la transaction à chaque chargement.
    effect(() => {
      const t = this.value();
      if (!t) {
        return;
      }
      this.form.reset({
        amount: toEditable(abs(t.amount_minor), t.currency),
        date: fromIsoDate(t.date),
        label: t.label,
        note: t.note ?? '',
      });
      this.direction.set(t.amount_minor < 0 ? 'out' : 'in');
      this.category.set(t.splits.length === 1 ? encodeAttachment(t.splits[0]) : null);
      this.payee.set(t.payee_id ?? null);
      this.project.set(t.splits.find((s) => s.project_id)?.project_id ?? null);
      const lines = new Set(t.splits.map((s) => s.project_line_id ?? null));
      this.initialLine = lines.size > 1 ? this.MIXED : (t.splits[0]?.project_line_id ?? null);
      this.line.set(this.initialLine);
      this.pendingSplits.set(null);
      // Rapprochée : montant et date de solde verrouillés (R-TRX-3).
      // Mensualité ventilée selon son échéance : montant verrouillé (R-CPT-17).
      if (t.locked || t.loan_payment) {
        this.form.controls.amount.disable();
        if (t.locked && !t.bank_date) {
          this.form.controls.date.disable();
        }
      } else {
        this.form.controls.amount.enable();
        this.form.controls.date.enable();
      }
    });
  }

  private signedAmount(): number | null {
    const t = this.value();
    const value = parseMoney(this.form.controls.amount.value, t?.currency);
    if (value === null) {
      return null;
    }
    return this.direction() === 'out' ? negate(value) : value;
  }

  protected async editSplits(t: TransactionDetail): Promise<void> {
    const total = this.signedAmount();
    if (total === null) {
      return;
    }
    const current = this.splits().map((s) => ({
      amount_minor: s.amount_minor,
      category_id: encodeAttachment(s),
      note: s.note ?? null,
      refunds_transaction_id: s.refunds_transaction_id ?? null,
    }));
    // Une ventilation unique suit le montant saisi (R-TRX-4).
    const splits =
      current.length === 1
        ? [{ ...current[0], amount_minor: total, category_id: this.category() }]
        : current;
    const data: SplitEditorData = {
      totalMinor: total,
      currency: t.currency,
      kind: total < 0 ? 'EXPENSE' : 'INCOME',
      splits,
    };
    const ref = this.dialog.open<SplitEditor, SplitEditorData, SplitDraft[]>(SplitEditor, {
      data,
      width: '36rem',
      maxWidth: '100vw',
    });
    const result = await firstValueFrom(ref.afterClosed());
    if (result) {
      this.pendingSplits.set(result);
      if (result.length === 1) {
        this.category.set(result[0].category_id);
      }
    }
  }

  protected async save(t: TransactionDetail): Promise<void> {
    this.form.markAllAsTouched();
    const amount = this.signedAmount();
    if (this.form.invalid || amount === null) {
      return;
    }
    const value = this.form.getRawValue();
    let splits: SplitDraft[] | null = this.pendingSplits();
    if (!splits && !this.isSplit() && !t.transfer) {
      splits = [
        {
          amount_minor: amount,
          category_id: this.category(),
          note: t.splits[0]?.note ?? null,
          refunds_transaction_id: t.splits[0]?.refunds_transaction_id ?? null,
        },
      ];
    }
    // Une ventilation multiple inchangée garde ses ventilations, mais le projet ou le poste peut avoir changé.
    const unchanged = !splits && this.isSplit();
    if (
      unchanged &&
      (this.project() !== (t.splits.find((s) => s.project_id)?.project_id ?? null) ||
        this.line() !== this.initialLine)
    ) {
      splits = t.splits.map((s) => ({
        amount_minor: s.amount_minor,
        category_id: encodeAttachment(s),
        note: s.note ?? null,
        refunds_transaction_id: s.refunds_transaction_id ?? null,
      }));
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(
        this.api.updateTransaction({
          id: t.id,
          body: {
            account_id: t.account_id,
            date: toIsoDate(value.date),
            amount_minor: amount,
            label: value.label,
            note: value.note || null,
            payee_id: t.transfer ? null : this.payee(),
            splits:
              splits?.map((s, i) => ({
                ...s,
                ...decodeAttachment(s.category_id),
                project_id: this.project(),
                project_line_id: this.splitLine(s.category_id, unchanged ? t.splits[i] : undefined),
              })) ?? null,
          },
        }),
      );
      this.version.bump();
      this.snack.open('Transaction enregistrée', undefined, { duration: 3000 });
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }

  /** Pointage manuel (RAP-07). */
  protected async setCleared(t: TransactionDetail, cleared: boolean): Promise<void> {
    try {
      await firstValueFrom(this.reconciliation.setCleared({ id: t.id, body: { cleared } }));
      this.version.bump();
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  /** Ventile la mensualité selon son échéance : capital, intérêts, assurance (CPT-10). */
  protected async payInstallment(p: LoanPaymentProposal): Promise<void> {
    try {
      await firstValueFrom(
        this.loans.payLoanInstallments({
          id: p.loan_id,
          body: [{ transaction_id: p.transaction_id, installment_number: p.installment_number }],
        }),
      );
      this.version.bump();
      this.snack.open(`Échéance n° ${p.installment_number} ventilée`, undefined, {
        duration: 3000,
      });
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  protected async unpayInstallment(t: TransactionDetail): Promise<void> {
    const payment = t.loan_payment;
    if (!payment) return;
    const ok = await confirm(this.dialog, {
      title: 'Annuler la ventilation ?',
      message:
        'La part de capital est retirée du prêt et la mensualité redevient une dépense non catégorisée.',
      action: 'Annuler la ventilation',
    });
    if (!ok) return;
    try {
      await firstValueFrom(
        this.loans.unpayLoanInstallment({
          id: payment.loan_id,
          number: payment.installment_number,
        }),
      );
      this.version.bump();
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  protected async markAsTransfer(t: TransactionDetail): Promise<void> {
    const ref = this.dialog.open(MarkTransferDialog, {
      data: t,
      width: '32rem',
      maxWidth: '100vw',
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async unlink(t: TransactionDetail): Promise<void> {
    if (!t.transfer) {
      return;
    }
    const ok = await confirm(this.dialog, {
      title: 'Délier le virement',
      message: 'Les deux transactions sont conservées, sans catégorie.',
      action: 'Délier',
    });
    if (ok) {
      await firstValueFrom(this.api.unlinkTransfer({ id: t.transfer.transfer_id }));
      this.version.bump();
    }
  }

  /** Le tiers de la transaction, proposé comme second rattachement d'un justificatif (DOC-04). */
  protected payeeTarget(t: TransactionDetail): { target: LinkTarget; label: string } | null {
    if (!t.payee_id) return null;
    const name = this.payeesStore.byId().get(t.payee_id)?.name ?? 'son tiers';
    return { target: { kind: 'PAYEE', id: t.payee_id }, label: name };
  }

  protected async remove(t: TransactionDetail): Promise<void> {
    // Sans justificatif, pas de confirmation : « Annuler » suffit (ecrans.md, principes) ; avec, la confirmation
    // dit qu'ils restent dans la bibliothèque (R-DOC-12).
    const deletion = await confirmDeletion(this.dialog, this.documents, t.id, {
      title: 'Supprimer la transaction ?',
      message: '',
      action: 'Supprimer',
      destructive: true,
      objectName: 'cette transaction',
      confirmWithoutDocuments: false,
    });
    if (!deletion) return;
    const kept = deletion.deleteDocuments
      ? []
      : await firstValueFrom(this.documents.listDocuments({ link_id: t.id }));
    try {
      await firstValueFrom(
        this.api.deleteTransaction({ id: t.id, delete_documents: deletion.deleteDocuments }),
      );
    } catch (error) {
      this.error.set(errorMessage(error));
      return;
    }
    this.version.bump();
    this.leave(t);
    // Annuler plutôt que confirmer : la transaction est recréée à l'identique, avec ses justificatifs restés dans
    // la bibliothèque.
    const undo = this.snack.open('Transaction supprimée', 'Annuler', { duration: 5000 });
    undo.onAction().subscribe(async () => {
      const { id } = await firstValueFrom(
        this.api.createTransaction({
          body: {
            account_id: t.account_id,
            date: t.date,
            amount_minor: t.amount_minor,
            label: t.label,
            note: t.note ?? null,
            splits: t.splits.map((s) => ({
              amount_minor: s.amount_minor,
              category_id: s.category_id ?? null,
              loan_id: s.loan_id ?? null,
              asset_id: s.asset_id ?? null,
              note: s.note ?? null,
              project_id: this.keptProject(s.project_id),
              project_line_id: this.keptProject(s.project_id) ? (s.project_line_id ?? null) : null,
              refunds_transaction_id: s.refunds_transaction_id ?? null,
            })),
          },
        }),
      );
      for (const d of kept) {
        await firstValueFrom(
          this.documents.linkDocument({ id: d.id, body: { kind: 'TRANSACTION', id } }),
        );
      }
      this.version.bump();
    });
  }

  protected async duplicate(t: TransactionDetail): Promise<void> {
    const created = await firstValueFrom(
      this.api.createTransaction({
        body: {
          account_id: t.account_id,
          date: todayIso(),
          amount_minor: t.amount_minor,
          label: t.label,
          note: t.note ?? null,
          payee_id: t.payee_id ?? null,
          splits: t.transfer
            ? null
            : t.splits.map((s) => ({
                amount_minor: s.amount_minor,
                category_id: s.category_id ?? null,
                loan_id: s.loan_id ?? null,
                asset_id: s.asset_id ?? null,
                note: s.note ?? null,
                project_id: this.keptProject(s.project_id),
                project_line_id: this.keptProject(s.project_id)
                  ? (s.project_line_id ?? null)
                  : null,
              })),
        },
      }),
    );
    this.version.bump();
    await this.router.navigate(['/transactions', created.id]);
  }

  /** Changer de projet présélectionne le poste proposé par les catégories (R-BUD-27). */
  protected changeProject(t: TransactionDetail, id: string | null): void {
    this.project.set(id);
    const attachments =
      this.pendingSplits()?.map((s) => s.category_id) ??
      (this.isSplit() ? t.splits.map((s) => s.category_id ?? null) : [this.category()]);
    const proposals = new Set(attachments.map((a) => this.proposal(a)));
    this.line.set(proposals.size > 1 ? this.MIXED : ([...proposals][0] ?? null));
  }

  /** Une nouvelle catégorie déplace la transaction dans son poste, si le poste était celui proposé. */
  protected changeCategory(value: string | null): void {
    if (this.line() === this.proposal(this.category())) {
      this.line.set(this.proposal(value));
    }
    this.category.set(value);
  }

  private proposal(attachment: string | null | undefined): string | null {
    const category = decodeAttachment(attachment).category_id;
    const parent = category ? this.categories.byId().get(category)?.parentId : null;
    return proposedLine(this.projectLines(), category, parent);
  }

  /** Poste d'une ventilation enregistrée : celui choisi, ou, pour « plusieurs postes », le sien ou le proposé. */
  private splitLine(
    attachment: string | null,
    existing: { project_id?: string | null; project_line_id?: string | null } | undefined,
  ): string | null {
    if (!this.project()) return null;
    if (this.line() !== this.MIXED) return this.line();
    return existing && existing.project_id === this.project()
      ? (existing.project_line_id ?? null)
      : this.proposal(attachment);
  }

  /** Projet repris par une copie : seulement s'il est encore en cours (R-BUD-24). */
  private keptProject(id: string | null | undefined): string | null {
    return id && this.projects.byId().get(id)?.active ? id : null;
  }

  protected leave(t: TransactionDetail): void {
    if (this.embedded()) {
      this.closed.emit();
    } else {
      void this.router.navigate(['/comptes', t.account_id]);
    }
  }
}
