import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { firstValueFrom } from 'rxjs';

import { RuleInput } from '../../api/models/rule-input';
import { RuleMatch } from '../../api/models/rule-match';
import { RuleView } from '../../api/models/rule-view';
import { RulesService } from '../../api/services/rules.service';
import { errorMessage } from '../../core/http-errors';
import { parseMoney, parsePercent, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';

export interface RuleFormData {
  rule: RuleView | null;
  /** Préremplissage (R-IMP-20). */
  draft?: Partial<RuleInput>;
}

interface Share {
  category: string | null;
  /** Pourcentage saisi (« 50 »). */
  percent: string;
}

/** Formulaire d'une règle (E16), avec aperçu et application à la demande (R-IMP-19). */
@Component({
  selector: 'app-rule-form',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
    CategoryPicker,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './rule-form.html',
  styleUrl: './rule-form.scss',
})
export class RuleFormDialog {
  private readonly data = inject<RuleFormData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<RuleFormDialog, boolean>);
  private readonly api = inject(RulesService);
  protected readonly accounts = inject(AccountsStore);

  private readonly source: Partial<RuleInput> = this.data.rule ?? this.data.draft ?? {};
  protected readonly name = signal(this.source.name ?? '');
  protected readonly labelOp = signal(this.source.conditions?.label?.op ?? 'contains');
  protected readonly labelValue = signal(this.source.conditions?.label?.value ?? '');
  protected readonly labelField = signal(this.source.conditions?.label?.field ?? 'bank_label');
  protected readonly amountOp = signal<string>(this.source.conditions?.amount?.op ?? '');
  protected readonly amountValue = signal(
    this.source.conditions?.amount ? toEditable(this.source.conditions.amount.value) : '',
  );
  protected readonly amountMax = signal(
    this.source.conditions?.amount?.max != null
      ? toEditable(this.source.conditions.amount.max)
      : '',
  );
  protected readonly sign = signal<string>(this.source.conditions?.sign ?? '');
  protected readonly accountIds = signal<string[]>(this.source.conditions?.accounts ?? []);
  protected readonly setLabel = signal(this.source.actions?.set_label ?? '');
  protected readonly transferTo = signal<string>(this.source.actions?.mark_transfer_to ?? '');
  protected readonly note = signal(this.source.actions?.set_note ?? '');
  protected readonly shares = signal<Share[]>(this.initialShares());
  protected readonly stop = signal(this.source.stop_processing ?? true);
  protected readonly active = signal(this.source.active ?? true);
  protected readonly priority = signal(this.source.priority ?? 0);
  protected readonly matches = signal<RuleMatch[] | null>(null);
  protected readonly chosen = signal<Set<string>>(new Set());
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly editing = !!this.data.rule;

  private initialShares(): Share[] {
    const splits = this.source.actions?.set_splits ?? [];
    if (splits.length === 0) {
      return [{ category: null, percent: '100' }];
    }
    const total = splits.reduce((sum, s) => sum + s.weight, 0) || 1;
    return splits.map((s) => ({
      category: s.category_id ?? null,
      percent: String(Math.round((s.weight * 10000) / total) / 100).replace('.', ','),
    }));
  }

  protected addShare(): void {
    this.shares.update((list) => [...list, { category: null, percent: '' }]);
  }

  protected updateShare(index: number, change: Partial<Share>): void {
    this.shares.update((list) => list.map((s, i) => (i === index ? { ...s, ...change } : s)));
  }

  protected removeShare(index: number): void {
    this.shares.update((list) => list.filter((_, i) => i !== index));
  }

  private conditions(): RuleInput['conditions'] {
    const amount = parseMoney(this.amountValue());
    return {
      label: this.labelValue().trim()
        ? { op: this.labelOp(), value: this.labelValue().trim(), field: this.labelField() }
        : null,
      amount:
        this.amountOp() && amount !== null
          ? { op: this.amountOp() as 'eq', value: amount, max: parseMoney(this.amountMax()) }
          : null,
      sign: (this.sign() || null) as 'debit' | 'credit' | null,
      accounts: this.accountIds(),
    };
  }

  private input(): RuleInput {
    const shares = this.transferTo()
      ? []
      : this.shares().filter((s) => s.category && (parsePercent(s.percent) ?? 0) > 0);
    return {
      name: this.name(),
      priority: this.priority(),
      conditions: this.conditions(),
      actions: {
        set_label: this.setLabel().trim() || null,
        set_splits: shares.map((s) => ({
          category_id: s.category,
          weight: parsePercent(s.percent) ?? 0,
        })),
        mark_transfer_to: this.transferTo() || null,
        set_note: this.note().trim() || null,
      },
      stop_processing: this.stop(),
      active: this.active(),
    };
  }

  protected async preview(): Promise<void> {
    this.error.set(null);
    try {
      const matches = await firstValueFrom(this.api.previewRule({ body: this.conditions() }));
      this.matches.set(matches);
      this.chosen.set(new Set(matches.map((m) => m.transaction_id)));
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  protected toggle(id: string, checked: boolean): void {
    this.chosen.update((s) => {
      const next = new Set(s);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
  }

  /** Enregistre la règle, puis l'applique aux transactions cochées dans l'aperçu (R-IMP-19). */
  protected async save(apply: boolean): Promise<void> {
    this.busy.set(true);
    this.error.set(null);
    try {
      const rule = this.data.rule
        ? await firstValueFrom(this.api.updateRule({ id: this.data.rule.id, body: this.input() }))
        : await firstValueFrom(this.api.createRule({ body: this.input() }));
      if (apply && this.chosen().size > 0) {
        await firstValueFrom(
          this.api.applyRule({ id: rule.id, body: { transaction_ids: [...this.chosen()] } }),
        );
      }
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
