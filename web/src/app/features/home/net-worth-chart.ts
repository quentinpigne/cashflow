import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';

import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { formatShortDate } from '../../core/dates';
import { formatChartValue, toChartValue } from '../../core/money';
import { MemberSelections } from '../../shared/member-filter';
import { keepValue } from '../../shared/keep-value';

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

  /** Membres cochés sur l'accueil (R-MBR-9). */
  private readonly members = inject(MemberSelections).of('home.net-worth', 'net-worth');
  private readonly history = rxResource({
    params: () => ({ version: this.version.value(), members: this.members.param() }),
    stream: ({ params }) => this.api.netWorthHistory({ months: 12, members: params.members }),
  });
  protected readonly historyValue = keepValue(this.history);

  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const points = this.historyValue() ?? [];
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
