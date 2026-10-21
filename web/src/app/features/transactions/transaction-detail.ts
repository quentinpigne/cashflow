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
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, todayIso, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { ReconciliationService } from '../../api/services/reconciliation.service';
import { ORIGIN_LABELS, STATUS_LABELS } from '../../core/labels';
import { abs, negate, parseMoney, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { CategoriesStore, ProjectsStore } from '../../shared/stores';
import { MarkTransferDialog } from './mark-transfer';
import { SplitDraft, SplitEditor, SplitEditorData } from './split-editor';

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
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './transaction-detail.html',
  styleUrl: './transaction-detail.scss',
})
export class TransactionDetailView {
  private readonly api = inject(TransactionsService);
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

  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly direction = signal<Direction>('out');
  protected readonly category = signal<string | null>(null);
  /** Projet de la transaction (BUD-06) : l'interface le porte sur toutes ses ventilations. */
  protected readonly project = signal<string | null>(null);
  protected readonly projects = inject(ProjectsStore);
  /** Projets proposés : ceux en cours, plus le projet actuel même clôturé (R-BUD-24). */
  protected readonly projectOptions = computed(() => {
    const current = this.project();
    return this.projects.list().filter((p) => p.active || p.id === current);
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
      this.category.set(t.splits.length === 1 ? (t.splits[0].category_id ?? null) : null);
      this.project.set(t.splits.find((s) => s.project_id)?.project_id ?? null);
      this.pendingSplits.set(null);
      // Rapprochée : montant et date de solde verrouillés (R-TRX-3).
      if (t.locked) {
        this.form.controls.amount.disable();
        if (!t.bank_date) {
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
      category_id: s.category_id ?? null,
      note: s.note ?? null,
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
        { amount_minor: amount, category_id: this.category(), note: t.splits[0]?.note ?? null },
      ];
    }
    // Une ventilation multiple inchangée garde ses ventilations, mais le projet peut avoir changé.
    if (
      !splits &&
      this.isSplit() &&
      this.project() !== (t.splits.find((s) => s.project_id)?.project_id ?? null)
    ) {
      splits = t.splits.map((s) => ({
        amount_minor: s.amount_minor,
        category_id: s.category_id ?? null,
        note: s.note ?? null,
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
            splits: splits?.map((s) => ({ ...s, project_id: this.project() })) ?? null,
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

  protected async remove(t: TransactionDetail): Promise<void> {
    try {
      await firstValueFrom(this.api.deleteTransaction({ id: t.id }));
    } catch (error) {
      this.error.set(errorMessage(error));
      return;
    }
    this.version.bump();
    this.leave(t);
    // Annuler plutôt que confirmer (ecrans.md, principes) : la transaction est recréée à l'identique.
    const undo = this.snack.open('Transaction supprimée', 'Annuler', { duration: 5000 });
    undo.onAction().subscribe(async () => {
      await firstValueFrom(
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
              note: s.note ?? null,
              project_id: this.keptProject(s.project_id),
            })),
          },
        }),
      );
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
          splits: t.transfer
            ? null
            : t.splits.map((s) => ({
                amount_minor: s.amount_minor,
                category_id: s.category_id ?? null,
                note: s.note ?? null,
                project_id: this.keptProject(s.project_id),
              })),
        },
      }),
    );
    this.version.bump();
    await this.router.navigate(['/transactions', created.id]);
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
