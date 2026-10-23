import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Router } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';

import { CategoryKind } from '../../api/models/category-kind';
import { PayeeLine } from '../../api/models/payee-line';
import { ReportGrouping } from '../../api/models/report-grouping';
import { ReportPeriod } from '../../api/models/report-period';
import { PayeesService } from '../../api/services/payees.service';
import { DataVersion } from '../../core/data-version';
import { currentMonth, formatMonthKey, shiftMonth } from '../../core/dates';
import { formatChartValue, toChartValue } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';

/** Barres du graphique : les premières lignes seulement (ecrans.md E15). */
const CHART_LINES = 15;

/** E15, onglet « Par tiers » : dépenses ou revenus par groupe de tiers ou par tiers (TIE-04). */
@Component({
  selector: 'app-payee-analysis',
  imports: [
    MatButtonModule,
    MatButtonToggleModule,
    MatIconModule,
    MatProgressBarModule,
    NgxEchartsDirective,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="tab">
      <div class="toolbar">
        <mat-button-toggle-group
          [value]="period()"
          (change)="period.set($event.value)"
          aria-label="Période"
        >
          <mat-button-toggle value="MONTH">Mois</mat-button-toggle>
          <mat-button-toggle value="LAST12_MONTHS">12 mois</mat-button-toggle>
          <mat-button-toggle value="YEAR">Année</mat-button-toggle>
        </mat-button-toggle-group>
        @if (period() !== 'LAST12_MONTHS') {
          <button mat-icon-button aria-label="Période précédente" (click)="shift(-1)">
            <mat-icon>chevron_left</mat-icon>
          </button>
          <strong class="month">{{ periodLabel() }}</strong>
          <button
            mat-icon-button
            aria-label="Période suivante"
            (click)="shift(1)"
            [disabled]="isCurrent()"
          >
            <mat-icon>chevron_right</mat-icon>
          </button>
        }
        <span class="spacer"></span>
        <mat-button-toggle-group
          [value]="kind()"
          (change)="kind.set($event.value)"
          aria-label="Nature"
        >
          <mat-button-toggle value="EXPENSE">Dépenses</mat-button-toggle>
          <mat-button-toggle value="INCOME">Revenus</mat-button-toggle>
        </mat-button-toggle-group>
        <mat-button-toggle-group
          [value]="groupBy()"
          (change)="groupBy.set($event.value)"
          aria-label="Regroupement"
        >
          <mat-button-toggle value="GROUP">Groupes</mat-button-toggle>
          <mat-button-toggle value="PAYEE">Tiers</mat-button-toggle>
        </mat-button-toggle-group>
      </div>

      @if (report.value(); as r) {
        <p class="muted">
          Du {{ r.from | frDate }} au {{ r.to | frDate }}, comparé au
          {{ r.previous_from | frDate }} – {{ r.previous_to | frDate }}. Seules les transactions
          rattachées à un tiers comptent.
        </p>
        @if (chart(); as options) {
          <div echarts [options]="options" class="chart" [style.height.px]="chartHeight()"></div>
        }
        <table class="table">
          <thead>
            <tr>
              <th>{{ groupBy() === 'GROUP' ? 'Groupe ou tiers' : 'Tiers' }}</th>
              <th class="right">Montant</th>
              <th class="right secondary">À catégoriser</th>
              <th class="right secondary">Transactions</th>
              <th class="right">{{ r.current ? 'Période préc. à date' : 'Période préc.' }}</th>
            </tr>
          </thead>
          <tbody>
            @for (line of r.lines; track key(line)) {
              <tr class="root">
                <td>
                  @if (line.children.length) {
                    <button
                      mat-icon-button
                      class="expand"
                      (click)="toggle(line)"
                      [attr.aria-label]="(isExpanded(line) ? 'Replier ' : 'Déplier ') + line.name"
                    >
                      <mat-icon>{{ isExpanded(line) ? 'expand_less' : 'expand_more' }}</mat-icon>
                    </button>
                  }
                  <button class="link" (click)="open(line)">{{ line.name }}</button>
                </td>
                <td class="amount">{{ line.amount_minor | money }}</td>
                <td class="amount muted secondary">
                  {{ line.uncategorized_minor ? (line.uncategorized_minor | money) : '' }}
                </td>
                <td class="amount muted secondary">{{ line.transactions }}</td>
                <td class="amount muted">{{ line.previous_minor | money }}</td>
              </tr>
              @if (isExpanded(line)) {
                @for (child of line.children; track key(child)) {
                  <tr class="child">
                    <td>
                      <button class="link" (click)="open(child)">{{ child.name }}</button>
                    </td>
                    <td class="amount">{{ child.amount_minor | money }}</td>
                    <td class="amount muted secondary">
                      {{ child.uncategorized_minor ? (child.uncategorized_minor | money) : '' }}
                    </td>
                    <td class="amount muted secondary">{{ child.transactions }}</td>
                    <td class="amount muted">{{ child.previous_minor | money }}</td>
                  </tr>
                }
              }
            } @empty {
              <tr>
                <td colspan="5" class="empty">
                  Aucune transaction rattachée à un tiers sur cette période.
                </td>
              </tr>
            }
          </tbody>
          <tfoot>
            <tr>
              <th>Total</th>
              <th class="amount">{{ r.total_minor | money }}</th>
              <th class="amount muted secondary">
                {{ r.uncategorized_minor ? (r.uncategorized_minor | money) : '' }}
              </th>
              <th class="secondary"></th>
              <th class="amount muted">{{ r.previous_total_minor | money }}</th>
            </tr>
          </tfoot>
        </table>
      } @else if (report.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
    </div>
  `,
  styleUrl: '../analyses/analyses.scss',
  // Sur téléphone, le tableau garde le nom, le montant et la période précédente.
  styles: `
    @media (max-width: 599px) {
      .secondary {
        display: none;
      }
    }
  `,
})
export class PayeeAnalysis {
  private readonly api = inject(PayeesService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);

  protected readonly period = signal<ReportPeriod>('LAST12_MONTHS');
  protected readonly month = signal(currentMonth());
  protected readonly year = signal(new Date().getFullYear());
  protected readonly kind = signal<CategoryKind>('EXPENSE');
  protected readonly groupBy = signal<ReportGrouping>('GROUP');
  protected readonly expanded = signal<Set<string>>(new Set());

  protected readonly report = rxResource({
    params: () => ({
      period: this.period(),
      month: this.month(),
      year: this.year(),
      kind: this.kind(),
      group_by: this.groupBy(),
      version: this.version.value(),
    }),
    stream: ({ params }) =>
      this.api.payeeReport({
        period: params.period,
        month: params.period === 'MONTH' ? params.month : undefined,
        year: params.period === 'YEAR' ? params.year : undefined,
        kind: params.kind,
        group_by: params.group_by,
      }),
  });

  protected readonly isCurrent = computed(() =>
    this.period() === 'YEAR'
      ? this.year() === new Date().getFullYear()
      : this.month() === currentMonth(),
  );

  protected readonly periodLabel = computed(() =>
    this.period() === 'YEAR' ? String(this.year()) : formatMonthKey(this.month()),
  );

  private readonly chartLines = computed(() =>
    (this.report.value()?.lines ?? [])
      .filter((l) => l.amount_minor > 0)
      .slice(0, CHART_LINES)
      .reverse(),
  );

  protected readonly chartHeight = computed(() => Math.max(200, this.chartLines().length * 36));

  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const lines = this.chartLines();
    if (lines.length === 0) return null;
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
        { name: 'Période', type: 'bar', data: lines.map((l) => toChartValue(l.amount_minor)) },
        {
          name: 'Période précédente',
          type: 'bar',
          data: lines.map((l) => toChartValue(l.previous_minor)),
          itemStyle: { opacity: 0.35 },
        },
      ],
    };
  });

  protected shift(delta: number): void {
    if (this.period() === 'YEAR') this.year.update((y) => y + delta);
    else this.month.set(shiftMonth(this.month(), delta));
  }

  protected key(line: PayeeLine): string {
    return line.payee_id ?? `group:${line.group_id}`;
  }

  protected toggle(line: PayeeLine): void {
    const key = this.key(line);
    this.expanded.update((s) => {
      const next = new Set(s);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  protected isExpanded(line: PayeeLine): boolean {
    return this.expanded().has(this.key(line));
  }

  /** Un clic ouvre la fiche du tiers ou du groupe (E32). */
  protected open(line: PayeeLine): void {
    const target = line.payee_id ? ['/tiers', line.payee_id] : ['/tiers/groupe', line.group_id];
    void this.router.navigate(target);
  }
}
