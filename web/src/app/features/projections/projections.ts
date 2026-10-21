import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  linkedSignal,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTabsModule } from '@angular/material/tabs';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';
import { firstValueFrom, of } from 'rxjs';

import { ScenarioView } from '../../api/models/scenario-view';
import { DashboardService } from '../../api/services/dashboard.service';
import { ProjectionsService } from '../../api/services/projections.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { KIND_LABELS } from '../../core/labels';
import { formatChartValue, formatRate, toChartValue } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';
import { ScenarioDialog } from './scenario-dialog';

const axisMoney = (v: number) => formatChartValue(v, 'EUR', { bare: true }).replace(/,00$/, '');

/** E24 — Projections (PRJ-01 à PRJ-05) et historique du patrimoine (TDB-04). */
@Component({
  selector: 'app-projections',
  imports: [
    FormsModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatProgressBarModule,
    MatSelectModule,
    MatTabsModule,
    NgxEchartsDirective,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './projections.html',
  styleUrl: './projections.scss',
})
export class ProjectionsPage {
  private readonly api = inject(ProjectionsService);
  private readonly dashboard = inject(DashboardService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  protected readonly accounts = inject(AccountsStore);

  protected readonly disclaimer =
    'Hypothèses par défaut, non des prévisions : les rendements futurs sont incertains.';

  // ------------------------------------------------------------ Patrimoine (PRJ-05, TDB-04)

  protected readonly years = signal(10);
  protected readonly worth = rxResource({
    params: () => ({ months: this.years() * 12, version: this.version.value() }),
    stream: ({ params }) => this.api.netWorthProjection({ months: params.months }),
  });
  protected readonly history = rxResource({
    params: () => this.version.value(),
    stream: () => this.dashboard.netWorthHistory({ months: 24 }),
  });

  protected readonly worthChart = computed<EChartsCoreOption | null>(() => {
    const worth = this.worth.value();
    const history = this.history.value() ?? [];
    if (!worth) return null;
    const start: [string, number] = [worth.today, toChartValue(worth.start_minor)];
    return {
      grid: { left: 8, right: 16, top: 40, bottom: 8, containLabel: true },
      legend: { top: 0 },
      tooltip: { trigger: 'axis', valueFormatter: (v: number) => formatChartValue(v) },
      xAxis: { type: 'time' },
      yAxis: { type: 'value', axisLabel: { formatter: axisMoney } },
      series: [
        {
          name: 'Réel',
          type: 'line',
          showSymbol: false,
          data: history.map((p) => [p.date, toChartValue(p.total_minor)]),
          lineStyle: { width: 3 },
        },
        ...worth.scenarios.map((s) => ({
          name: s.name,
          type: 'line',
          showSymbol: false,
          lineStyle: { type: 'dashed', width: s.is_default ? 3 : 2 },
          data: [start, ...s.points.map((p) => [p.date, toChartValue(p.value_minor)])],
        })),
      ],
    };
  });

  protected readonly finals = computed(() =>
    (this.worth.value()?.scenarios ?? []).map((s) => ({
      name: s.name,
      value: s.points[s.points.length - 1]?.value_minor ?? 0,
      real: s.points[s.points.length - 1]?.real_value_minor ?? null,
    })),
  );

  // ------------------------------------------------------------ Trésorerie (PRJ-01)

  protected readonly cashAccounts = computed(() =>
    this.accounts.open().filter((a) => a.family === 'CASH'),
  );
  protected readonly cashAccount = linkedSignal(() => this.cashAccounts()[0]?.id ?? null);
  protected readonly days = signal(90);
  protected readonly withVariable = signal(true);
  protected readonly cash = rxResource({
    params: () => ({
      id: this.cashAccount(),
      days: this.days(),
      variable: this.withVariable(),
      version: this.version.value(),
    }),
    stream: ({ params }) =>
      params.id
        ? this.api.cashProjection({ id: params.id, days: params.days, variable: params.variable })
        : of(null),
  });

  protected readonly cashChart = computed<EChartsCoreOption | null>(() => {
    const p = this.cash.value();
    if (!p) return null;
    return {
      grid: { left: 8, right: 16, top: 16, bottom: 8, containLabel: true },
      tooltip: { trigger: 'axis', valueFormatter: (v: number) => formatChartValue(v) },
      xAxis: { type: 'time' },
      yAxis: { type: 'value', axisLabel: { formatter: axisMoney } },
      series: [
        {
          name: 'Solde projeté',
          type: 'line',
          step: 'end',
          showSymbol: false,
          data: [
            [p.today, toChartValue(p.start_minor)],
            ...p.days.map((d) => [d.date, toChartValue(d.balance_minor)]),
          ],
          markLine: {
            symbol: 'none',
            silent: true,
            lineStyle: { color: '#c62828', type: 'dashed' },
            label: { formatter: 'seuil' },
            data: [{ yAxis: toChartValue(p.threshold_minor) }],
          },
          markPoint: {
            symbolSize: 40,
            data: [
              {
                name: 'Minimum',
                coord: [p.minimum_date, toChartValue(p.minimum_minor)],
                value: formatChartValue(toChartValue(p.minimum_minor), 'EUR', { bare: true }),
              },
            ],
          },
        },
      ],
    };
  });

  // ------------------------------------------------------------ Épargne (PRJ-02, PRJ-04)

  protected readonly savingsAccounts = computed(() =>
    this.accounts
      .open()
      .filter((a) => a.family === 'FIXED_SAVINGS' || a.family === 'VARIABLE_SAVINGS'),
  );
  protected readonly savingsAccount = linkedSignal(() => this.savingsAccounts()[0]?.id ?? null);
  protected readonly savingsYears = signal(5);
  protected readonly scenarioId = linkedSignal(
    () => (this.scenarios.value() ?? []).find((s) => s.is_default)?.id ?? null,
  );
  protected readonly projection = rxResource({
    params: () => ({
      id: this.savingsAccount(),
      scenario: this.scenarioId(),
      months: this.savingsYears() * 12,
      version: this.version.value(),
    }),
    stream: ({ params }) =>
      params.id
        ? this.api.accountProjection({
            id: params.id,
            scenario_id: params.scenario ?? undefined,
            months: params.months,
          })
        : of(null),
  });

  protected readonly savingsChart = computed<EChartsCoreOption | null>(() => {
    const p = this.projection.value();
    if (!p) return null;
    const series = [
      {
        name: 'Valeur projetée',
        type: 'line',
        showSymbol: false,
        data: [
          [p.start_date, toChartValue(p.start_minor)],
          ...p.points.map((pt) => [pt.date, toChartValue(pt.value_minor)]),
        ],
      },
    ];
    if (p.points.some((pt) => pt.real_value_minor !== null && pt.real_value_minor !== undefined)) {
      series.push({
        name: 'En euros constants',
        type: 'line',
        showSymbol: false,
        data: [
          [p.start_date, toChartValue(p.start_minor)],
          ...p.points.map((pt) => [pt.date, toChartValue(pt.real_value_minor ?? 0)]),
        ],
      });
    }
    return {
      grid: { left: 8, right: 16, top: 32, bottom: 8, containLabel: true },
      legend: { top: 0 },
      tooltip: { trigger: 'axis', valueFormatter: (v: number) => formatChartValue(v) },
      xAxis: { type: 'time' },
      yAxis: { type: 'value', axisLabel: { formatter: axisMoney } },
      series,
    };
  });

  // ------------------------------------------------------------ Scénarios (PRJ-03)

  protected readonly scenarios = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listScenarios(),
  });

  protected kindYields(s: ScenarioView): { label: string; rate: string }[] {
    return Object.entries(s.assumptions.by_kind ?? {}).map(([kind, y]) => ({
      label: KIND_LABELS[kind as keyof typeof KIND_LABELS] ?? kind,
      rate:
        formatRate(y.annual_return_ppm) +
        (y.annual_fees_ppm ? ` (frais ${formatRate(y.annual_fees_ppm)})` : ''),
    }));
  }

  protected rate(ppm: number | null | undefined): string {
    return ppm === null || ppm === undefined ? '—' : formatRate(ppm);
  }

  protected async editScenario(scenario: ScenarioView | null): Promise<void> {
    const ref = this.dialog.open(ScenarioDialog, { data: scenario, maxWidth: '44rem' });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async removeScenario(scenario: ScenarioView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${scenario.name} » ?`,
      message: 'Ses hypothèses seront perdues.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteScenario({ id: scenario.id }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }
}
