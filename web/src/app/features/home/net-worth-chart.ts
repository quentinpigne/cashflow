import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';

import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { formatShortDate } from '../../core/dates';
import { formatChartValue, toChartValue } from '../../core/money';

/** Patrimoine à chaque fin de mois des 12 derniers mois et aujourd'hui (TDB-11, R-TDB-22). */
@Component({
  selector: 'app-net-worth-chart',
  imports: [NgxEchartsDirective],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (chart(); as options) {
      <div
        echarts
        [options]="options"
        class="chart"
        role="img"
        aria-label="Patrimoine des 12 derniers mois"
      ></div>
    }
  `,
  styles: `
    .chart {
      height: 6rem;
      margin-top: 0.5rem;
    }
  `,
})
export class NetWorthChart {
  private readonly api = inject(DashboardService);
  private readonly version = inject(DataVersion);

  private readonly history = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.netWorthHistory({ months: 12 }),
  });

  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const points = this.history.value() ?? [];
    if (points.length < 2) return null;
    return {
      grid: { left: 0, right: 0, top: 4, bottom: 0 },
      tooltip: {
        trigger: 'axis',
        valueFormatter: (v: number) => formatChartValue(v),
      },
      xAxis: { type: 'category', show: false, data: points.map((p) => formatShortDate(p.date)) },
      yAxis: { type: 'value', show: false, scale: true },
      series: [
        {
          type: 'line',
          smooth: true,
          showSymbol: false,
          areaStyle: { opacity: 0.15 },
          data: points.map((p) => toChartValue(p.total_minor)),
        },
      ],
    };
  });
}
