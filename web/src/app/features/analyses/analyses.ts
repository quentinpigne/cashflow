import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTabsModule } from '@angular/material/tabs';
import { Router } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';

import { CategoryKind } from '../../api/models/category-kind';
import { CategoryLine } from '../../api/models/category-line';
import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { formatMonth, shiftMonth, toIsoDate } from '../../core/dates';
import { formatChartValue, formatRatePpm, toChartValue } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';
import { PayeeAnalysis } from '../payees/payee-analysis';

/** E15 — Analyses (TDB-02, TDB-03, TIE-04). */
@Component({
  selector: 'app-analyses',
  imports: [
    FormsModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatIconModule,
    MatProgressBarModule,
    MatTabsModule,
    NgxEchartsDirective,
    MoneyPipe,
    PayeeAnalysis,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './analyses.html',
  styleUrl: './analyses.scss',
})
export class AnalysesPage {
  private readonly api = inject(DashboardService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);

  protected readonly month = signal(toIsoDate(new Date()).slice(0, 7));
  protected readonly kind = signal<CategoryKind>('EXPENSE');
  protected readonly months = signal<12 | 24>(12);
  /** Inclure les dépenses rattachées à des projets (TDB-07). */
  protected readonly withProjects = signal(true);
  protected readonly expanded = signal<Set<string>>(new Set());

  protected readonly report = rxResource({
    params: () => ({
      month: this.month(),
      kind: this.kind(),
      withProjects: this.withProjects(),
      version: this.version.value(),
    }),
    stream: ({ params }) =>
      this.api.categoryReport({
        month: params.month,
        kind: params.kind,
        exclude_projects: !params.withProjects,
      }),
  });

  protected readonly evolution = rxResource({
    params: () => ({ months: this.months(), version: this.version.value() }),
    stream: ({ params }) => this.api.evolutionReport({ months: params.months }),
  });

  protected readonly chartHeight = computed(() =>
    Math.max(200, (this.report.value()?.lines.length ?? 0) * 36),
  );
  protected readonly monthLabel = computed(() => formatMonth(`${this.month()}-01`));
  protected readonly isCurrent = computed(() => this.month() === toIsoDate(new Date()).slice(0, 7));

  protected readonly categoryChart = computed<EChartsCoreOption | null>(() => {
    const report = this.report.value();
    if (!report || report.lines.length === 0) {
      return null;
    }
    const lines = [...report.lines].filter((l) => l.amount_minor > 0).reverse();
    return {
      grid: { left: 8, right: 24, top: 8, bottom: 8, containLabel: true },
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        valueFormatter: (v: number) => formatChartValue(v),
      },
      xAxis: {
        type: 'value',
        axisLabel: {
          formatter: (v: number) => formatChartValue(v, 'EUR', { bare: true }).replace(/,00$/, ''),
        },
      },
      yAxis: { type: 'category', data: lines.map((l) => l.name) },
      series: [
        {
          name: this.isCurrent() ? 'Ce mois-ci' : this.monthLabel(),
          type: 'bar',
          data: lines.map((l) => toChartValue(l.amount_minor)),
        },
        {
          name: 'Moyenne 12 mois',
          type: 'bar',
          data: lines.map((l) => toChartValue(l.average_minor)),
          itemStyle: { opacity: 0.35 },
        },
      ],
    };
  });

  protected readonly evolutionChart = computed<EChartsCoreOption | null>(() => {
    const months = this.evolution.value()?.months ?? [];
    if (months.length === 0) {
      return null;
    }
    return {
      grid: { left: 8, right: 8, top: 32, bottom: 8, containLabel: true },
      legend: { top: 0 },
      tooltip: { trigger: 'axis', valueFormatter: (v: number) => formatChartValue(v) },
      xAxis: {
        type: 'category',
        data: months.map((m) => formatMonth(`${m.month}-01`).replace(/ \d{2}(\d{2})$/, ' $1')),
      },
      yAxis: {
        type: 'value',
        axisLabel: {
          formatter: (v: number) => formatChartValue(v, 'EUR', { bare: true }).replace(/,00$/, ''),
        },
      },
      series: [
        { name: 'Revenus', type: 'bar', data: months.map((m) => toChartValue(m.income_minor)) },
        { name: 'Dépenses', type: 'bar', data: months.map((m) => toChartValue(m.expenses_minor)) },
        {
          name: 'Épargne',
          type: 'line',
          data: months.map((m) => toChartValue(m.savings_minor)),
          smooth: true,
        },
      ],
    };
  });

  protected rate(ppm: number | null | undefined): string {
    return ppm === null || ppm === undefined ? '—' : formatRatePpm(ppm);
  }

  protected previousMonth(): void {
    this.month.set(shiftMonth(this.month(), -1));
  }

  protected nextMonth(): void {
    this.month.set(shiftMonth(this.month(), 1));
  }

  protected toggle(line: CategoryLine): void {
    const key = line.category_id ?? 'none';
    this.expanded.update((s) => {
      const next = new Set(s);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  protected isExpanded(line: CategoryLine): boolean {
    return this.expanded().has(line.category_id ?? 'none');
  }

  /** Clic sur une catégorie : les transactions correspondantes (E14). */
  protected drill(line: CategoryLine): void {
    const report = this.report.value();
    if (!report) {
      return;
    }
    const queryParams: Record<string, string> = { du: report.from, au: report.to };
    if (line.category_id) {
      queryParams['categorie'] = line.category_id;
    } else {
      queryParams['a_categoriser'] = '1';
    }
    void this.router.navigate(['/transactions'], { queryParams });
  }
}
