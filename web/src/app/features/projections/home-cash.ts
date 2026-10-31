import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatIconModule } from '@angular/material/icon';
import { RouterLink } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';
import { of } from 'rxjs';

import { ProjectionsService } from '../../api/services/projections.service';
import { DataVersion } from '../../core/data-version';
import { PlannedProjects } from '../../core/planned-projects';
import { formatChartValue, toChartValue } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';
import { cashAccount } from '../home/widget-rules';
import { WidgetMenu } from '../home/widget-menu';

/** Trésorerie à venir du compte courant sur 30 jours (TDB-10, R-TDB-15). */
@Component({
  selector: 'app-home-cash',
  imports: [RouterLink, MatIconModule, NgxEchartsDirective, WidgetMenu, FrDatePipe, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (account(); as a) {
      @if (projection.value(); as p) {
        <section class="card">
          <h2 class="widget-title">
            <a routerLink="/projections" [queryParams]="{ onglet: 'tresorerie', compte: a.id }"
              >Trésorerie à venir</a
            >
          </h2>
          <app-widget-menu widget="cash" />
          <p class="muted small">{{ a.name }} · 30 prochains jours</p>
          <div class="row">
            <span>Aujourd'hui</span>
            <span class="amount" [class.negative]="p.start_minor < 0">{{
              p.start_minor | money
            }}</span>
          </div>
          <div class="row">
            <span>Dans 30 jours</span>
            <span class="amount" [class.negative]="end() < 0">{{ end() | money }}</span>
          </div>
          <div class="row">
            <span>Au plus bas, le {{ p.minimum_date | frDate: 'short' }}</span>
            <strong class="amount" [class.negative]="p.minimum_minor < 0">{{
              p.minimum_minor | money
            }}</strong>
          </div>
          @if (p.minimum_minor < 0) {
            <p class="warning small" role="status">
              <mat-icon inline>warning</mat-icon> Le compte passe sous zéro le
              {{ p.minimum_date | frDate: 'short' }}.
            </p>
          }
          @if (chart(); as options) {
            <div echarts [options]="options" class="chart" aria-hidden="true"></div>
          }
        </section>
      }
    }
  `,
  styleUrl: '../home/widget.scss',
})
export class HomeCash {
  private readonly api = inject(ProjectionsService);
  private readonly version = inject(DataVersion);
  private readonly accounts = inject(AccountsStore);
  private readonly plannedProjects = inject(PlannedProjects);

  protected readonly account = computed(() => cashAccount(this.accounts.list()));

  protected readonly projection = rxResource({
    params: () => ({
      id: this.account()?.id,
      planned: this.plannedProjects.value(),
      version: this.version.value(),
    }),
    stream: ({ params }) =>
      params.id
        ? this.api.cashProjection({ id: params.id, days: 30, planned_projects: params.planned })
        : of(null),
  });

  protected readonly end = computed(() => {
    const days = this.projection.value()?.days ?? [];
    return days.length > 0 ? days[days.length - 1].balance_minor : 0;
  });

  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const days = this.projection.value()?.days ?? [];
    if (days.length < 2) return null;
    return {
      grid: { left: 0, right: 0, top: 4, bottom: 0 },
      tooltip: { trigger: 'axis', valueFormatter: (v: number) => formatChartValue(v) },
      xAxis: { type: 'category', show: false, data: days.map((d) => d.date) },
      yAxis: { type: 'value', show: false, scale: true },
      series: [
        {
          type: 'line',
          data: days.map((d) => toChartValue(d.balance_minor)),
          showSymbol: false,
          areaStyle: { opacity: 0.15 },
          markLine: { silent: true, symbol: 'none', data: [{ yAxis: 0 }], label: { show: false } },
        },
      ],
    };
  });
}
