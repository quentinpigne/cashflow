import {
  ChangeDetectionStrategy,
  Component,
  Injectable,
  computed,
  inject,
  signal,
} from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import {
  MAT_BOTTOM_SHEET_DATA,
  MatBottomSheet,
  MatBottomSheetModule,
  MatBottomSheetRef,
} from '@angular/material/bottom-sheet';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatDatepickerModule } from '@angular/material/datepicker';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { CategoryKind } from '../../api/models/category-kind';
import { SplitInput } from '../../api/models/split-input';
import { TransactionsService } from '../../api/services/transactions.service';
import { decodeAttachment, isAttachment } from '../../core/attachments';
import { DataVersion } from '../../core/data-version';
import { toIsoDate } from '../../core/dates';
import { errorMessage, problemOf } from '../../core/http-errors';
import { Layout } from '../../core/layout';
import { formatMoney, negate, parseMoney, sum } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { PayeePicker } from '../../shared/payee-picker';
import { AccountsStore, CategoriesStore, PayeesStore } from '../../shared/stores';
import { SplitDraft, SplitEditor, SplitEditorData } from './split-editor';

export interface QuickEntryData {
  accountId?: string;
}

type Mode = 'expense' | 'income' | 'transfer';

const LAST_ACCOUNT_KEY = 'cashflow.lastAccount';

function rememberedAccount(): string | null {
  try {
    return localStorage.getItem(LAST_ACCOUNT_KEY);
  } catch {
    return null;
  }
}

function rememberAccount(id: string): void {
  try {
    localStorage.setItem(LAST_ACCOUNT_KEY, id);
  } catch {
    // Préférence de confort seulement.
  }
}

/** Ouvre la saisie rapide : panneau du bas sur mobile, dialogue sur ordinateur (ecrans.md E5). */
@Injectable({ providedIn: 'root' })
export class QuickEntry {
  private readonly sheet = inject(MatBottomSheet);
  private readonly dialog = inject(MatDialog);
  private readonly layout = inject(Layout);

  open(data: QuickEntryData = {}): void {
    if (this.layout.mobile()) {
      this.sheet.open(QuickEntryForm, { data, panelClass: 'quick-entry-sheet' });
    } else {
      this.dialog.open(QuickEntryForm, { data, width: '32rem', autoFocus: 'first-tabbable' });
    }
  }
}

/** E5 — Saisie rapide (TRX-01, TRX-05). */
@Component({
  selector: 'app-quick-entry',
  imports: [
    ReactiveFormsModule,
    MatBottomSheetModule,
    MatDialogModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
    CategoryPicker,
    PayeePicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './quick-entry.html',
  styleUrl: './quick-entry.scss',
})
export class QuickEntryForm {
  private readonly sheetData = inject<QuickEntryData | null>(MAT_BOTTOM_SHEET_DATA, {
    optional: true,
  });
  private readonly dialogData = inject<QuickEntryData | null>(MAT_DIALOG_DATA, { optional: true });
  private readonly sheetRef = inject(MatBottomSheetRef, { optional: true });
  private readonly dialogRef = inject(MatDialogRef, { optional: true });
  private readonly api = inject(TransactionsService);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  private readonly version = inject(DataVersion);
  protected readonly accounts = inject(AccountsStore);

  private readonly data = this.sheetData ?? this.dialogData ?? {};
  protected readonly mode = signal<Mode>('expense');
  protected readonly category = signal<string | null>(null);
  /** Tiers (R-TIE-10), sans objet pour un virement (R-TIE-11). */
  protected readonly payee = signal<string | null>(null);
  private readonly payees = inject(PayeesStore);
  private readonly categories = inject(CategoriesStore);
  protected readonly splits = signal<SplitDraft[] | null>(null);
  protected readonly showNote = signal(false);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

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
    accountId: [this.data.accountId ?? rememberedAccount() ?? '', Validators.required],
    targetAccountId: [''],
    label: ['', [Validators.required, Validators.maxLength(200)]],
    date: [new Date(), Validators.required],
    note: [''],
  });

  private readonly amountValue = toSignal(this.form.controls.amount.valueChanges, {
    initialValue: '',
  });
  private readonly accountValue = toSignal(this.form.controls.accountId.valueChanges, {
    initialValue: this.form.controls.accountId.value,
  });

  protected readonly categoryKind = computed<CategoryKind>(() =>
    this.mode() === 'income' ? 'INCOME' : 'EXPENSE',
  );
  protected readonly currency = computed(
    () => this.accounts.byId().get(this.accountValue())?.currency ?? 'EUR',
  );
  /** Montant signé : négatif pour une dépense (R-TRX-1). */
  protected readonly signedAmount = computed(() => {
    const value = parseMoney(this.amountValue(), this.currency());
    if (value === null) {
      return null;
    }
    return this.mode() === 'expense' ? negate(value) : value;
  });
  protected readonly targets = computed(() =>
    this.accounts.open().filter((a) => a.id !== this.accountValue()),
  );

  constructor() {
    if (!this.accounts.byId().has(this.form.controls.accountId.value)) {
      this.form.controls.accountId.setValue(this.accounts.open()[0]?.id ?? '');
    }
  }

  /**
   * Choisir un tiers alors que la catégorie est vide reprend celle de sa dernière transaction, si elle est de la
   * nature du mode (R-TIE-14).
   */
  protected choosePayee(id: string | null): void {
    this.payee.set(id);
    const suggested = id ? this.payees.byId().get(id)?.suggested_category_id : null;
    if (
      suggested &&
      !this.category() &&
      !this.splits() &&
      this.categories.byId().get(suggested)?.kind === this.categoryKind()
    ) {
      this.category.set(suggested);
    }
  }

  protected setMode(mode: Mode): void {
    this.mode.set(mode);
    this.splits.set(null);
    this.category.set(null);
    this.form.controls.targetAccountId.setValidators(
      mode === 'transfer' ? Validators.required : null,
    );
    this.form.controls.targetAccountId.updateValueAndValidity();
  }

  protected splitSummary(): string {
    const splits = this.splits();
    return splits ? `Ventilée en ${splits.length} lignes` : '';
  }

  protected async editSplits(): Promise<void> {
    const total = this.signedAmount();
    if (total === null) {
      this.form.controls.amount.markAsTouched();
      return;
    }
    const current = this.splits() ?? [
      { amount_minor: total, category_id: this.category(), note: null },
    ];
    const data: SplitEditorData = {
      totalMinor: total,
      currency: this.currency(),
      kind: this.categoryKind(),
      splits: current,
    };
    const ref = this.dialog.open<SplitEditor, SplitEditorData, SplitDraft[]>(SplitEditor, {
      data,
      width: '36rem',
      maxWidth: '100vw',
    });
    const result = await firstValueFrom(ref.afterClosed());
    if (result) {
      this.splits.set(result.length > 1 ? result : null);
      if (result.length === 1) {
        this.category.set(result[0].category_id);
      }
    }
  }

  protected close(): void {
    this.sheetRef?.dismiss();
    this.dialogRef?.close();
  }

  /**
   * Ventilations envoyées à l'API : un remboursement ou une acquisition choisi comme « catégorie » devient une
   * ventilation rattachée (R-TRX-37).
   */
  private apiSplits(splits: SplitDraft[] | null, amount: number): SplitInput[] | null {
    const lines = splits ?? [{ amount_minor: amount, category_id: this.category(), note: null }];
    if (!splits && !isAttachment(this.category())) return null;
    return lines.map((s) => ({ ...s, ...decodeAttachment(s.category_id), project_id: null }));
  }

  protected async save(): Promise<void> {
    this.form.markAllAsTouched();
    const amount = this.signedAmount();
    if (this.form.invalid || amount === null) {
      return;
    }
    const value = this.form.getRawValue();
    const date = toIsoDate(value.date);
    this.busy.set(true);
    this.error.set(null);
    try {
      let id: string;
      if (this.mode() === 'transfer') {
        const created = await firstValueFrom(
          this.api.createTransfer({
            body: {
              from_account_id: value.accountId,
              to_account_id: value.targetAccountId,
              amount_minor: Math.abs(amount),
              date,
              label: value.label,
              note: value.note || null,
            },
          }),
        );
        id = created.outflow_transaction_id;
      } else {
        const splits = this.splits();
        // Une ventilation saisie avant de changer le montant ne correspond plus : on la refuse.
        if (splits && sum(splits.map((s) => s.amount_minor)) !== amount) {
          this.error.set(
            `La ventilation ne correspond plus au montant (${formatMoney(amount)}) : la reprendre.`,
          );
          return;
        }
        const created = await firstValueFrom(
          this.api.createTransaction({
            body: {
              account_id: value.accountId,
              date,
              amount_minor: amount,
              label: value.label,
              note: value.note || null,
              category_id: splits || isAttachment(this.category()) ? null : this.category(),
              splits: this.apiSplits(splits, amount),
              payee_id: this.payee(),
            },
          }),
        );
        id = created.id;
      }
      rememberAccount(value.accountId);
      this.version.bump();
      this.close();
      const ref = this.snack.open(
        this.mode() === 'transfer' ? 'Virement enregistré' : 'Transaction enregistrée',
        'Voir',
        { duration: 5000 },
      );
      ref.onAction().subscribe(() => void this.router.navigate(['/transactions', id]));
    } catch (error) {
      const problem = problemOf(error);
      this.error.set(problem?.field === 'label' ? 'Libellé invalide.' : errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
