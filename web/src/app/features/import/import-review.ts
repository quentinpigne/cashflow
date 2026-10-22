import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatChipsModule } from '@angular/material/chips';
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

import { CommitResult } from '../../api/models/commit-result';
import { ImportDecision } from '../../api/models/import-decision';
import { ImportLineView } from '../../api/models/import-line-view';
import { LineReviewInput } from '../../api/models/line-review-input';
import { ImportsService } from '../../api/services/imports.service';
import { LoansService } from '../../api/services/loans.service';
import { ReconciliationService } from '../../api/services/reconciliation.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage, problemOf } from '../../core/http-errors';
import { BATCH_STATUS_LABELS, DECISION_LABELS } from '../../core/labels';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore, CategoriesStore } from '../../shared/stores';
import { SplitDraft, SplitEditor, SplitEditorData } from '../transactions/split-editor';

type View = 'all' | 'create' | 'match' | 'duplicate' | 'error' | 'uncategorized';

/** E11 — Import : revue (IMP-04, IMP-05, R-IMP-18). */
@Component({
  selector: 'app-import-review',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatCheckboxModule,
    MatChipsModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatProgressBarModule,
    MatSelectModule,
    CategoryPicker,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './import-review.html',
  styleUrl: './import-review.scss',
})
export class ImportReviewPage {
  private readonly api = inject(ImportsService);
  private readonly reconciliation = inject(ReconciliationService);
  private readonly loans = inject(LoansService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  protected readonly accounts = inject(AccountsStore);
  protected readonly categories = inject(CategoriesStore);

  readonly id = input.required<string>();

  protected readonly decisionLabels = DECISION_LABELS;
  protected readonly statusLabels = BATCH_STATUS_LABELS;
  protected readonly view = signal<View>('all');
  protected readonly selection = signal<Set<string>>(new Set());
  protected readonly bulkCategory = signal<string | null>(null);
  protected readonly busy = signal(false);
  protected readonly result = signal<CommitResult | null>(null);
  protected readonly error = signal<string | null>(null);

  protected readonly batch = rxResource({
    params: () => ({ id: this.id(), version: this.version.value() }),
    stream: ({ params }) => this.api.getImport({ id: params.id }),
  });

  protected readonly reviewing = computed(() => this.batch.value()?.summary.status === 'REVIEWING');

  protected readonly lines = computed<ImportLineView[]>(() => {
    const lines = this.batch.value()?.lines ?? [];
    const view = this.view();
    return lines.filter((l) => {
      switch (view) {
        case 'create':
          return l.errors.length === 0 && l.decision === 'CREATE';
        case 'match':
          return l.decision === 'MATCH';
        case 'duplicate':
          return l.match_kind === 'DUPLICATE';
        case 'error':
          return l.errors.length > 0;
        case 'uncategorized':
          return (
            l.decision === 'CREATE' &&
            l.errors.length === 0 &&
            !l.transfer_to &&
            !l.splits.some((s) => s.category_id)
          );
        default:
          return true;
      }
    });
  });

  protected readonly counts = computed(() => {
    const lines = this.batch.value()?.lines ?? [];
    return {
      all: lines.length,
      create: lines.filter((l) => l.errors.length === 0 && l.decision === 'CREATE').length,
      match: lines.filter((l) => l.decision === 'MATCH').length,
      duplicate: lines.filter((l) => l.match_kind === 'DUPLICATE').length,
      error: lines.filter((l) => l.errors.length > 0).length,
      uncategorized: lines.filter(
        (l) =>
          l.decision === 'CREATE' &&
          l.errors.length === 0 &&
          !l.transfer_to &&
          !l.splits.some((s) => s.category_id),
      ).length,
    };
  });

  protected accountName(id: string | null | undefined): string {
    return (id && this.accounts.byId().get(id)?.name) || '—';
  }

  private state(line: ImportLineView): LineReviewInput {
    return {
      decision: line.decision ?? 'CREATE',
      match_transaction_id: line.matched?.transaction_id ?? null,
      label: line.label ?? null,
      splits: line.splits,
      note: line.note ?? null,
      transfer_to: line.transfer_to ?? null,
      transfer_transaction_id: line.transfer_transaction_id ?? null,
    };
  }

  private async save(line: ImportLineView, change: Partial<LineReviewInput>): Promise<void> {
    try {
      await firstValueFrom(
        this.api.reviewImportLine({
          id: this.id(),
          line_id: line.id,
          body: { ...this.state(line), ...change },
        }),
      );
      this.batch.reload();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  protected setDecision(line: ImportLineView, decision: ImportDecision): Promise<void> {
    return this.save(line, { decision });
  }

  protected setCategory(line: ImportLineView, category: string | null): Promise<void> {
    const amount = line.amount_minor ?? 0;
    return this.save(line, {
      splits: category ? [{ category_id: category, amount_minor: amount, note: null }] : [],
    });
  }

  protected setLabel(line: ImportLineView, label: string): Promise<void> {
    return this.save(line, { label: label.trim() || null });
  }

  protected acceptProposal(line: ImportLineView): Promise<void> {
    const proposal = line.proposal;
    if (!proposal) {
      return Promise.resolve();
    }
    return this.save(line, {
      transfer_to: proposal.account_id,
      transfer_transaction_id: proposal.transaction_id ?? null,
      splits: [],
    });
  }

  protected setTransfer(line: ImportLineView, account: string | null): Promise<void> {
    return this.save(line, { transfer_to: account, transfer_transaction_id: null, splits: [] });
  }

  protected async editSplits(line: ImportLineView): Promise<void> {
    const total = line.amount_minor ?? 0;
    const current: SplitDraft[] = line.splits.length
      ? line.splits.map((s) => ({
          amount_minor: s.amount_minor,
          category_id: s.category_id ?? null,
          note: s.note ?? null,
        }))
      : [{ amount_minor: total, category_id: null, note: null }];
    const data: SplitEditorData = {
      totalMinor: total,
      currency: this.batch.value()?.currency ?? 'EUR',
      kind: total < 0 ? 'EXPENSE' : 'INCOME',
      splits: current,
    };
    const ref = this.dialog.open<SplitEditor, SplitEditorData, SplitDraft[]>(SplitEditor, {
      data,
      width: '36rem',
      maxWidth: '100vw',
    });
    const result = await firstValueFrom(ref.afterClosed());
    if (result) {
      await this.save(line, { splits: result });
    }
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
    this.selection.set(
      checked
        ? new Set(
            this.lines()
              .filter((l) => l.errors.length === 0)
              .map((l) => l.id),
          )
        : new Set(),
    );
  }

  protected async bulk(decision: ImportDecision | null): Promise<void> {
    if (this.selection().size === 0) {
      return;
    }
    try {
      await firstValueFrom(
        this.api.bulkReviewImport({
          id: this.id(),
          body: {
            line_ids: [...this.selection()],
            decision,
            category_id: decision ? null : this.bulkCategory(),
          },
        }),
      );
      this.selection.set(new Set());
      this.batch.reload();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  protected async cancel(): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Annuler l’import',
      message: 'Le lot et ses lignes seront supprimés. Aucune transaction n’a été modifiée.',
      action: 'Annuler l’import',
      destructive: true,
    });
    if (ok) {
      await firstValueFrom(this.api.cancelImport({ id: this.id() }));
      this.version.bump();
      await this.router.navigate(['/import']);
    }
  }

  protected async commit(replace = false): Promise<void> {
    this.busy.set(true);
    this.error.set(null);
    try {
      const result = await firstValueFrom(
        this.api.commitImport({ id: this.id(), body: { replace_checkpoints: replace } }),
      );
      this.result.set(result);
      this.version.bump();
    } catch (error) {
      if (problemOf(error)?.code === 'CHECKPOINT_EXISTS' && !replace) {
        const ok = await confirm(this.dialog, {
          title: 'Remplacer un point de contrôle',
          message: `${errorMessage(error)}. Le solde du relevé remplacera celui déjà saisi.`,
          action: 'Remplacer et valider',
        });
        if (ok) {
          this.busy.set(false);
          await this.commit(true);
          return;
        }
      } else {
        this.error.set(errorMessage(error));
      }
    } finally {
      this.busy.set(false);
    }
  }

  /** Ventile les mensualités de prêt reconnues dans le lot, chacune selon son échéance (R-IMP-22). */
  protected async payLoans(): Promise<void> {
    const proposals = this.result()?.loan_payments ?? [];
    const loans = [...new Set(proposals.map((p) => p.loan_account_id))];
    try {
      for (const loan of loans) {
        const body = proposals
          .filter((p) => p.loan_account_id === loan)
          .map((p) => ({
            transaction_id: p.transaction_id,
            installment_number: p.installment_number,
          }));
        await firstValueFrom(this.loans.payLoanInstallments({ id: loan, body }));
      }
      this.version.bump();
      this.result.update((r) => (r ? { ...r, loan_payments: [] } : r));
      this.snack.open('Mensualités ventilées', undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  /** « Rapprocher au <date> » en un clic après la validation (R-RAP-11). */
  protected async reconcile(checkpointId: string): Promise<void> {
    try {
      await firstValueFrom(this.reconciliation.reconcile({ id: checkpointId }));
      this.version.bump();
      this.result.update((r) =>
        r
          ? {
              ...r,
              checkpoints: r.checkpoints.map((c) =>
                c.id === checkpointId ? { ...c, reconciled: true } : c,
              ),
            }
          : r,
      );
      this.snack.open('Rapprochement validé', undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }
}
