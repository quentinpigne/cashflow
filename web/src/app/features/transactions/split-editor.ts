import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';

import { CategoryKind } from '../../api/models/category-kind';
import { allocate, formatMoney, parseMoney, parsePercent, sum, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';

/** Une ventilation en cours d'édition (montant signé, comme la transaction). */
export interface SplitDraft {
  amount_minor: number;
  category_id: string | null;
  note: string | null;
  /** Dépense que la ventilation rembourse (R-TRX-50) : gardée telle quelle. */
  refunds_transaction_id?: string | null;
}

export interface SplitEditorData {
  totalMinor: number;
  currency: string;
  kind: CategoryKind;
  splits: SplitDraft[];
}

interface Line {
  category: string | null;
  note: string;
  refunds: string | null;
  /** Montant saisi en valeur absolue, ou pourcentage, selon le mode. */
  input: string;
}

/**
 * E6 — Ventilation (TRX-03) : par montants ou par pourcentages, reste à ventiler, arrondi sur la
 * dernière ligne (R-TRX-14). « Valider » n'est actif que si le reste vaut 0 (R-TRX-10).
 */
@Component({
  selector: 'app-split-editor',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatFormFieldModule,
    MatInputModule,
    MatIconModule,
    CategoryPicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './split-editor.html',
  styleUrl: './split-editor.scss',
})
export class SplitEditor {
  protected readonly data = inject<SplitEditorData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<SplitEditor, SplitDraft[]>);

  /** Signe de la transaction : les lignes sont saisies en valeur absolue. */
  private readonly sign = this.data.totalMinor < 0 ? -1 : 1;
  protected readonly mode = signal<'amount' | 'percent'>('amount');
  protected readonly lines = signal<Line[]>(
    this.data.splits.length > 0
      ? this.data.splits.map((s) => ({
          category: s.category_id,
          note: s.note ?? '',
          refunds: s.refunds_transaction_id ?? null,
          input: toEditable(s.amount_minor * this.sign, this.data.currency),
        }))
      : [
          {
            category: null,
            note: '',
            refunds: null,
            input: toEditable(Math.abs(this.data.totalMinor), this.data.currency),
          },
        ],
  );

  protected readonly total = formatMoney(this.data.totalMinor, { currency: this.data.currency });

  /** Montants signés calculés pour chaque ligne, ou `null` si une saisie est invalide. */
  protected readonly amounts = computed<(number | null)[]>(() => {
    const lines = this.lines();
    if (this.mode() === 'amount') {
      return lines.map((l) => {
        const value = parseMoney(l.input, this.data.currency);
        return value === null ? null : value * this.sign;
      });
    }
    const weights = lines.map((l) => parsePercent(l.input));
    if (weights.some((w) => w === null)) {
      return weights.map(() => null);
    }
    return allocate(this.data.totalMinor, weights as number[]) ?? weights.map(() => null);
  });

  protected readonly remaining = computed(() => {
    const amounts = this.amounts();
    return amounts.some((a) => a === null) ? null : this.data.totalMinor - sum(amounts as number[]);
  });

  protected readonly percentTotal = computed(() =>
    sum(this.lines().map((l) => parsePercent(l.input) ?? 0)),
  );

  protected readonly valid = computed(() => {
    const amounts = this.amounts();
    return this.remaining() === 0 && amounts.every((a) => a !== null && a !== 0);
  });

  protected format(minor: number | null): string {
    return minor === null ? '—' : formatMoney(minor, { currency: this.data.currency });
  }

  /** Une ligne de signe opposé à la transaction est un remboursement (R-TRX-11). */
  protected isOpposite(minor: number | null): boolean {
    return minor !== null && minor !== 0 && Math.sign(minor) !== Math.sign(this.data.totalMinor);
  }

  protected update(index: number, patch: Partial<Line>): void {
    this.lines.update((lines) =>
      lines.map((line, i) => (i === index ? { ...line, ...patch } : line)),
    );
  }

  protected add(): void {
    const remaining = this.remaining();
    const input =
      this.mode() === 'amount' && remaining
        ? toEditable(Math.abs(remaining), this.data.currency)
        : '';
    this.lines.update((lines) => [...lines, { category: null, note: '', refunds: null, input }]);
  }

  protected remove(index: number): void {
    this.lines.update((lines) => lines.filter((_, i) => i !== index));
  }

  /** « Répartir le reste » : la ligne reçoit le reste à ventiler. */
  protected fill(index: number): void {
    const remaining = this.remaining();
    const current = this.amounts()[index];
    if (remaining === null || current === null) {
      return;
    }
    this.update(index, {
      input: toEditable((current + remaining) * this.sign, this.data.currency),
    });
  }

  protected switchMode(mode: 'amount' | 'percent'): void {
    if (mode === this.mode()) {
      return;
    }
    if (mode === 'amount') {
      // Les montants calculés deviennent la saisie.
      const amounts = this.amounts();
      this.lines.update((lines) =>
        lines.map((l, i) => ({
          ...l,
          input:
            amounts[i] === null
              ? ''
              : toEditable((amounts[i] as number) * this.sign, this.data.currency),
        })),
      );
    } else {
      this.lines.update((lines) => lines.map((l) => ({ ...l, input: '' })));
    }
    this.mode.set(mode);
  }

  protected save(): void {
    const amounts = this.amounts() as number[];
    this.ref.close(
      this.lines().map((line, i) => ({
        amount_minor: amounts[i],
        category_id: line.category,
        note: line.note.trim() || null,
        refunds_transaction_id: line.refunds,
      })),
    );
  }
}
