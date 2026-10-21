import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Router, RouterLink } from '@angular/router';

import { BudgetLineMonth } from '../../api/models/budget-line-month';
import { BudgetsService } from '../../api/services/budgets.service';
import { DataVersion } from '../../core/data-version';
import { currentMonth, formatMonthKey, shiftMonth } from '../../core/dates';
import { MoneyPipe } from '../../shared/money.pipe';
import {
  LineCreateDialog,
  LineEditDialog,
  ThresholdsDialog,
  openAndRefresh,
} from './budget-dialogs';
import { BudgetGauge } from './gauge';

interface Section {
  title: string;
  hint?: string;
  lines: BudgetLineMonth[];
}

/** E18 — Budget du mois (BUD-02 à BUD-05, BUD-07, BUD-08). */
@Component({
  selector: 'app-budget',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatMenuModule,
    MatProgressBarModule,
    MatTooltipModule,
    MoneyPipe,
    BudgetGauge,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './budget.html',
  styleUrl: './budget.scss',
})
export class BudgetPage {
  private readonly api = inject(BudgetsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);

  /** Mois demandé dans l'adresse (`?mois=AAAA-MM`). */
  readonly mois = input<string | undefined>();

  private readonly chosen = signal<string | null>(null);
  protected readonly month = computed(() => this.chosen() ?? this.mois() ?? currentMonth());
  protected readonly monthLabel = computed(() => formatMonthKey(this.month()));

  protected readonly budget = rxResource({
    params: () => ({ month: this.month(), version: this.version.value() }),
    stream: ({ params }) => this.api.monthBudget({ month: params.month }),
  });

  protected readonly sections = computed<Section[]>(() => {
    const lines = this.budget.value()?.lines ?? [];
    const of = (predicate: (l: BudgetLineMonth) => boolean) => lines.filter(predicate);
    return [
      { title: 'Revenus', lines: of((l) => l.kind === 'INCOME') },
      {
        title: 'Dépenses mensuelles',
        lines: of((l) => l.kind === 'EXPENSE' && l.periodicity === 'MONTHLY'),
      },
      {
        title: 'Dépenses annuelles',
        hint: 'quote-part du mois, consommé sur l’année',
        lines: of((l) => l.kind === 'EXPENSE' && l.periodicity === 'YEARLY'),
      },
      { title: 'Épargne', lines: of((l) => l.kind === 'SAVINGS') },
    ].filter((s) => s.lines.length > 0);
  });

  protected readonly definitions = {
    toBudget:
      'Revenus prévus − dépenses mensuelles prévues − quotes-parts annuelles − objectifs d’épargne (R-BUD-40).',
    balance:
      'Revenus réels − dépenses réelles (lignes, hors budget, non catégorisées, projets) − épargne versée (R-BUD-41).',
  };

  protected previousMonth(): void {
    this.go(shiftMonth(this.month(), -1));
  }

  protected nextMonth(): void {
    this.go(shiftMonth(this.month(), 1));
  }

  private go(month: string): void {
    this.chosen.set(month);
    void this.router.navigate([], { queryParams: { mois: month }, replaceUrl: true });
  }

  protected add(): void {
    void openAndRefresh(this.dialog, this.version, LineCreateDialog, { month: this.month() });
  }

  protected edit(line: BudgetLineMonth): void {
    void openAndRefresh(this.dialog, this.version, LineEditDialog, {
      line,
      month: this.month(),
      thresholds: this.budget.value()?.thresholds ?? [80, 100],
    });
  }

  protected editThresholds(): void {
    void openAndRefresh(this.dialog, this.version, ThresholdsDialog, {
      thresholds: this.budget.value()?.thresholds ?? [80, 100],
    });
  }

  /** Les transactions qui alimentent une ligne ou un groupe, pour ce mois (E14). */
  protected drill(categoryId: string | null | undefined, uncategorized = false): void {
    const [y, m] = this.month().split('-').map(Number);
    const last = new Date(y, m, 0).getDate();
    const queryParams: Record<string, string> = {
      du: `${this.month()}-01`,
      au: `${this.month()}-${String(last).padStart(2, '0')}`,
    };
    if (categoryId) queryParams['categorie'] = categoryId;
    if (uncategorized) queryParams['a_categoriser'] = '1';
    void this.router.navigate(['/transactions'], { queryParams });
  }
}
