import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';

import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { PlannedProjects } from '../../core/planned-projects';
import { subtract } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';
import { yearTotals } from '../home/widget-rules';
import { WidgetMenu } from '../home/widget-menu';

/** Estimation de fin d'année comparée à l'année précédente (TDB-10, R-TDB-17). */
@Component({
  selector: 'app-home-year-end',
  imports: [RouterLink, WidgetMenu, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (evolution.value(); as e) {
      <section class="card">
        <h2 class="widget-title">
          <a routerLink="/analyses" [queryParams]="{ onglet: 'evolution' }"
            >Estimation fin {{ e.year_end.year }}</a
          >
        </h2>
        <app-widget-menu widget="year-end" />
        @for (f of figures(); track f.label) {
          <div class="row">
            <span>{{ f.label }}</span>
            <span class="label end">
              <strong class="amount" [class.negative]="f.value < 0">{{ f.value | money }}</strong>
              @if (f.gap !== null) {
                <span class="muted small amount"
                  >{{ f.gap | money: 'EUR' : { signed: true } }} par rapport à
                  {{ e.year_end.year - 1 }}</span
                >
              }
            </span>
          </div>
        }
        @if (!e.has_budget) {
          <p class="muted small">Sans budget, les mois à venir ne comptent que les projets.</p>
        }
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
  styles: `
    .end {
      align-items: flex-end;
    }
  `,
})
export class HomeYearEnd {
  private readonly api = inject(DashboardService);
  private readonly version = inject(DataVersion);
  private readonly plannedProjects = inject(PlannedProjects);
  private readonly accounts = inject(AccountsStore);

  /** Assez de mois pour couvrir toute l'année précédente. */
  protected readonly evolution = rxResource({
    params: () => ({ planned: this.plannedProjects.value(), version: this.version.value() }),
    stream: ({ params }) =>
      this.api.evolutionReport({
        months: new Date().getMonth() + 13,
        planned_projects: params.planned,
      }),
  });

  protected readonly figures = computed(() => {
    const e = this.evolution.value();
    if (!e) return [];
    // L'année précédente ne se compare que si l'historique la couvre en entier (R-TDB-17).
    const first = this.accounts
      .list()
      .map((a) => a.opening_date)
      .sort()[0];
    const covered = !!first && first <= `${e.year_end.year - 1}-01-01`;
    const last = covered ? yearTotals(e.months, e.year_end.year - 1) : null;
    const gap = (value: number, previous: number | undefined) =>
      previous === undefined ? null : subtract(value, previous);
    return [
      {
        label: 'Revenus',
        value: e.year_end.income_minor,
        gap: gap(e.year_end.income_minor, last?.income_minor),
      },
      {
        label: 'Dépenses',
        value: e.year_end.expenses_minor,
        gap: gap(e.year_end.expenses_minor, last?.expenses_minor),
      },
      {
        label: 'Épargne',
        value: e.year_end.savings_minor,
        gap: gap(e.year_end.savings_minor, last?.savings_minor),
      },
    ];
  });
}
