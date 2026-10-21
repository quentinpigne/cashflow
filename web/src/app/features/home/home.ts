import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { FlowsView } from '../../api/models/flows-view';
import { UncategorizedItem } from '../../api/models/uncategorized-item';
import { DashboardService } from '../../api/services/dashboard.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { formatMonth } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { FAMILY_LABELS } from '../../core/labels';
import { formatRatePpm } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { HomeBudget } from '../budget/home-budget';
import { HomeSavings } from '../savings/home-savings';

/** Définitions affichées dans les info-bulles « comment est-ce calculé ? » (spec 08). */
const DEFINITIONS = {
  netWorth:
    'Somme des soldes des comptes de liquidités et d’épargne à taux fixe, et des valeurs des comptes d’épargne variable et des prêts (négatifs), pour les comptes comptés dans le patrimoine.',
  variation: 'Patrimoine aujourd’hui moins patrimoine à la fin du mois précédent.',
  income:
    'Somme des ventilations sur des catégories de revenus. Les virements internes sont exclus.',
  expenses:
    'Somme des ventilations sur des catégories de dépenses ; les remboursements viennent en déduction.',
  savings: 'Revenus moins dépenses du mois.',
  rate: 'Épargne du mois divisée par les revenus. Non défini si les revenus sont nuls.',
};

/** E1 — Accueil (TDB-01). */
@Component({
  selector: 'app-home',
  imports: [
    HomeBudget,
    HomeSavings,
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatProgressBarModule,
    MatTooltipModule,
    CategoryPicker,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './home.html',
  styleUrl: './home.scss',
})
export class HomePage {
  private readonly api = inject(DashboardService);
  private readonly transactions = inject(TransactionsService);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);

  protected readonly definitions = DEFINITIONS;
  protected readonly familyLabels = FAMILY_LABELS;
  protected readonly dashboard = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.getDashboard(),
  });
  protected readonly showDetail = signal(false);

  protected readonly monthLabel = computed(() =>
    this.dashboard.hasValue() ? formatMonth(this.dashboard.value().month.from) : '',
  );

  protected rate(flows: FlowsView): string {
    return flows.savings_rate_ppm === null || flows.savings_rate_ppm === undefined
      ? '—'
      : formatRatePpm(flows.savings_rate_ppm);
  }

  /** Catégorisation sur place, sans ouvrir la transaction (ecrans.md E1). */
  protected async categorize(item: UncategorizedItem, categoryId: string | null): Promise<void> {
    if (!categoryId) {
      return;
    }
    try {
      await firstValueFrom(
        this.transactions.categorizeSplit({ id: item.split_id, body: { category_id: categoryId } }),
      );
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 4000 });
    }
  }
}
