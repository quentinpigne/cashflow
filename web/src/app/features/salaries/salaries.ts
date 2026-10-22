import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatDialog } from '@angular/material/dialog';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Router, RouterLink } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';
import { firstValueFrom } from 'rxjs';

import { SalariesService } from '../../api/services/salaries.service';
import { DataVersion } from '../../core/data-version';
import { Layout } from '../../core/layout';
import { formatChartValue, formatRatePpm, toChartValue } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { EmploymentDialog } from './salary-dialogs';

const axisMoney = (v: number) => formatChartValue(v, 'EUR', { bare: true }).replace(/,00$/, '');

/** Variation annuelle signée : « +5,9 % » (R-SAL-8). */
export function formatChange(ppm: number | null | undefined): string {
  if (ppm === null || ppm === undefined) return '';
  return (ppm > 0 ? '+' : '') + formatRatePpm(ppm);
}

/** E29 — Salaires : évolution annuelle et emplois (SAL-01, SAL-02). */
@Component({
  selector: 'app-salaries-page',
  imports: [
    RouterLink,
    MatButtonModule,
    MatButtonToggleModule,
    MatExpansionModule,
    MatIconModule,
    MatProgressBarModule,
    NgxEchartsDirective,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Salaires</h1>
        <button mat-flat-button (click)="create()"><mat-icon>add</mat-icon>Nouvel emploi</button>
      </div>
      @if (overview.isLoading() && !overview.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      } @else if (overview.value(); as o) {
        @if (o.employments.length === 0) {
          <div class="empty">
            <p>Aucun emploi.</p>
            <button mat-stroked-button (click)="create()">Créer un emploi</button>
          </div>
        } @else {
          <section class="card group">
            <header>
              <h2>Évolution</h2>
              @if (layout.mobile()) {
                <mat-button-toggle-group
                  hideSingleSelectionIndicator
                  [value]="measure()"
                  (change)="measure.set($event.value)"
                  aria-label="Montant affiché"
                >
                  <mat-button-toggle value="net">Net</mat-button-toggle>
                  <mat-button-toggle value="gross">Brut</mat-button-toggle>
                </mat-button-toggle-group>
              }
            </header>
            @if (chart(); as options) {
              <div class="chart" echarts [options]="options"></div>
            }
            <table>
              <thead>
                <tr>
                  <th>Année</th>
                  <th class="right">Net</th>
                  <th class="right wide">Brut</th>
                  <th class="right">Variation</th>
                </tr>
              </thead>
              <tbody>
                @for (y of yearsDesc(); track y.year) {
                  <tr>
                    <td>
                      {{ y.year }}
                      @if (y.provisional) {
                        <span class="muted small">prévisionnel</span>
                      } @else if (y.incomplete) {
                        <span class="muted small">incomplet</span>
                      }
                    </td>
                    <td class="right amount">{{ y.net_minor | money }}</td>
                    <td class="right amount wide">
                      {{ y.gross_minor | money }}{{ y.gross_incomplete ? ' *' : '' }}
                    </td>
                    <td class="right">{{ change(y.net_change_ppm) }}</td>
                  </tr>
                }
              </tbody>
            </table>
            @if (hasIncompleteGross()) {
              <p class="muted small">* Brut incomplet : mois non renseigné ou prime sans brut.</p>
            }
          </section>

          <section class="card group">
            <header><h2>Emplois</h2></header>
            @for (e of current(); track e.id) {
              <a class="row" [routerLink]="['/salaires', e.id]">
                <span class="name">
                  <span>{{ e.employer }}</span>
                  <span class="muted small"
                    >{{ e.job_title ? e.job_title + ' · ' : '' }}depuis le {{ e.start_date | frDate
                    }}{{ e.end_date ? ' · fin le ' + (e.end_date | frDate) : '' }}</span
                  >
                </span>
                <span class="amount">
                  @if (e.current_net_minor !== null && e.current_net_minor !== undefined) {
                    {{ e.current_net_minor | money }}
                    <span class="muted small">/ mois</span>
                  } @else {
                    <span class="muted small">non renseigné</span>
                  }
                </span>
              </a>
            }
            @if (ended().length > 0) {
              <mat-accordion>
                <mat-expansion-panel>
                  <mat-expansion-panel-header>
                    <mat-panel-title>Emplois terminés ({{ ended().length }})</mat-panel-title>
                  </mat-expansion-panel-header>
                  @for (e of ended(); track e.id) {
                    <a class="row" [routerLink]="['/salaires', e.id]">
                      <span class="name">{{ e.employer }}</span>
                      <span class="muted small"
                        >{{ e.start_date | frDate: 'short' }} –
                        {{ e.end_date! | frDate: 'short' }}</span
                      >
                    </a>
                  }
                </mat-expansion-panel>
              </mat-accordion>
            }
          </section>
        }
      }
    </div>
  `,
  styleUrl: '../accounts/accounts.scss',
  styles: `
    .chart {
      height: 18rem;
      margin-bottom: 0.5rem;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      th,
      td {
        padding: 0.35rem 0.25rem;
        border-top: 1px solid var(--mat-sys-outline-variant);
        text-align: left;
      }
      th {
        font: var(--mat-sys-label-medium);
        color: var(--mat-sys-on-surface-variant);
      }
      .right {
        text-align: right;
      }
      @media (max-width: 599.98px) {
        .wide {
          display: none;
        }
      }
    }
  `,
})
export class SalariesPage {
  private readonly api = inject(SalariesService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);
  protected readonly layout = inject(Layout);

  protected readonly overview = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.salariesOverview(),
  });
  protected readonly measure = signal<'net' | 'gross'>('net');
  protected readonly current = computed(() =>
    (this.overview.value()?.employments ?? []).filter((e) => !e.ended).reverse(),
  );
  protected readonly ended = computed(() =>
    (this.overview.value()?.employments ?? []).filter((e) => e.ended).reverse(),
  );
  protected readonly yearsDesc = computed(() =>
    [...(this.overview.value()?.years ?? [])].reverse(),
  );
  protected readonly hasIncompleteGross = computed(() =>
    (this.overview.value()?.years ?? []).some((y) => y.gross_incomplete),
  );
  protected readonly change = formatChange;

  /** Barres du net annuel par emploi, courbe du brut ; bascule net / brut sur mobile (E29). */
  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const o = this.overview.value();
    if (!o || o.years.length === 0) return null;
    const years = o.years.map((y) => String(y.year));
    const mobile = this.layout.mobile();
    const showNet = !mobile || this.measure() === 'net';
    const showGross = !mobile || this.measure() === 'gross';
    const bars = showNet
      ? o.employments.map((e) => ({
          name: e.employer,
          type: 'bar',
          stack: 'net',
          data: o.years.map((y) => {
            const part = y.employments.find((p) => p.employment_id === e.id);
            return part ? toChartValue(part.net_minor) : 0;
          }),
          itemStyle: { opacity: 1 },
        }))
      : [];
    const gross = showGross
      ? [
          {
            name: 'Brut',
            type: 'line',
            data: o.years.map((y) => toChartValue(y.gross_minor)),
            lineStyle: { width: 2 },
          },
        ]
      : [];
    return {
      grid: { left: 8, right: 16, top: 40, bottom: 8, containLabel: true },
      legend: { top: 0 },
      tooltip: { trigger: 'axis', valueFormatter: (v: number) => formatChartValue(v) },
      xAxis: { type: 'category', data: years },
      yAxis: { type: 'value', axisLabel: { formatter: axisMoney } },
      series: [...bars, ...gross],
    };
  });

  protected async create(): Promise<void> {
    const ref = this.dialog.open(EmploymentDialog, { data: null, maxWidth: '100vw' });
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.version.bump();
      await this.router.navigate(['/salaires', id]);
    }
  }
}
