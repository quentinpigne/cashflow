import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';

import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { formatMonth } from '../../core/dates';
import { formatChartValue, formatRatePpm, toChartValue } from '../../core/money';
import { WidgetMenu } from '../home/widget-menu';

/** Épargne des 12 derniers mois et taux d'épargne glissant (TDB-11, R-TDB-20). */
@Component({
  selector: 'app-home-evolution',
  imports: [RouterLink, NgxEchartsDirective, WidgetMenu],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (chart(); as options) {
      <section class="card">
        <h2 class="widget-title">
          <a routerLink="/analyses" [queryParams]="{ onglet: 'evolution' }">Évolution</a>
        </h2>
        <app-widget-menu widget="evolution" />
        @if (rate(); as r) {
          <p class="muted small">Taux d'épargne sur 12 mois : {{ r }}</p>
        }
        <div
          echarts
          [options]="options"
          class="chart tall"
          role="img"
          aria-label="Épargne des 12 derniers mois"
        ></div>
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
  styles: `
    .tall {
      height: 10rem;
    }
  `,
})
export class HomeEvolution {
  private readonly api = inject(DashboardService);
  private readonly version = inject(DataVersion);

  private readonly evolution = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.evolutionReport({ months: 12 }),
  });

  protected readonly rate = computed(() => {
    const months = this.evolution.value()?.months ?? [];
    const ppm = months[months.length - 1]?.rolling_rate_ppm;
    return ppm === null || ppm === undefined ? null : formatRatePpm(ppm);
  });

  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const months = this.evolution.value()?.months ?? [];
    if (months.length === 0) return null;
    return {
      grid: { left: 4, right: 4, top: 8, bottom: 4, containLabel: true },
      tooltip: { trigger: 'axis' },
      xAxis: {
        type: 'category',
        data: months.map((m) => formatMonth(`${m.month}-01`).slice(0, 4)),
        axisLabel: { fontSize: 10 },
      },
      yAxis: [
        { type: 'value', show: false },
        { type: 'value', show: false },
      ],
      series: [
        {
          name: 'Épargne',
          type: 'bar',
          data: months.map((m) => toChartValue(m.savings_minor)),
          tooltip: { valueFormatter: (v: number) => formatChartValue(v) },
        },
        {
          name: 'Taux glissant',
          type: 'line',
          yAxisIndex: 1,
          smooth: true,
          showSymbol: false,
          data: months.map((m) =>
            (m.rolling_rate_ppm ?? null) === null ? null : (m.rolling_rate_ppm as number) / 10_000,
          ),
          tooltip: { valueFormatter: (v: number) => `${v.toFixed(1).replace('.', ',')} %` },
        },
      ],
    };
  });
}
