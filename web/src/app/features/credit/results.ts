import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatIconModule } from '@angular/material/icon';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';

import { AffordabilityView } from '../../api/models/affordability-view';
import { ComputeResult } from '../../api/models/compute-result';
import { CreditConditions } from '../../api/models/credit-conditions';
import { PurchaseCapacity } from '../../api/models/purchase-capacity';
import { formatChartValue, formatMoney, formatRate, toChartValue } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { formatDuration } from './credit-shared';

const axisMoney = (v: number) => formatChartValue(v, 'EUR', { bare: true }).replace(/,00$/, '');

/** « 2,5 mois » : des dixièmes de mois. */
export function tenthsOfMonths(tenths: number): string {
  return `${(tenths / 10).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} mois`;
}

/** Mensualité, coût total et TAEG d'un crédit calculé (R-CRE-2 à R-CRE-4). */
@Component({
  selector: 'app-loan-figures',
  imports: [MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @let r = loan();
    <div class="figures">
      <div class="figure" [class.main]="main() === 'PAYMENT'">
        <span class="muted">Échéance, assurance comprise</span>
        <strong>{{ r.result.total_payment_minor | money }}</strong>
        <span class="small muted"
          >{{ r.result.payment_minor | money }} +
          {{ r.result.insurance_minor | money }} d'assurance, sur
          {{ duration(r.duration_months) }}</span
        >
      </div>
      <div class="figure">
        <span class="muted">Coût total du crédit</span>
        <strong>{{ r.result.total_cost_minor | money }}</strong>
        <span class="small muted"
          >intérêts {{ r.result.total_interest_minor | money }} · assurance
          {{ r.result.total_insurance_minor | money }}</span
        >
      </div>
      <div class="figure">
        <span class="muted">TAEG</span>
        <strong>{{ rate(r.result.apr_ppm) }}</strong>
      </div>
    </div>
  `,
  styles: `
    .figures {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
      gap: 1rem;
    }
    .figure {
      display: flex;
      flex-direction: column;
      gap: 0.125rem;
      strong {
        font: var(--mat-sys-title-large);
      }
      &.main strong {
        color: var(--mat-sys-primary);
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class LoanFigures {
  readonly loan = input.required<ComputeResult>();
  /** Le chiffre cherché, mis en avant. */
  readonly main = input<'PAYMENT' | 'NONE'>('PAYMENT');
  protected readonly rate = formatRate;
  protected readonly duration = formatDuration;
}

/** Endettement, reste à vivre et capacité d'épargne d'une échéance (R-CRE-15, R-CRE-32, R-CRE-40). */
@Component({
  selector: 'app-affordability',
  imports: [MatIconModule, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @let a = value();
    <div class="figures">
      <div class="figure">
        <span class="muted">Taux d'endettement</span>
        @if (a.debt_ratio_ppm !== null && a.debt_ratio_ppm !== undefined) {
          <strong
            class="ratio"
            [class.over]="a.debt_ratio_exceeded"
            [class.ok]="!a.debt_ratio_exceeded"
          >
            {{ rate(a.debt_ratio_ppm) }}
            <mat-icon>{{ a.debt_ratio_exceeded ? 'warning' : 'check_circle' }}</mat-icon>
          </strong>
          <span class="small muted">visé : {{ rate(a.max_debt_ratio_ppm) }}</span>
        } @else {
          <strong>—</strong>
          <span class="small muted">sans revenus</span>
        }
      </div>
      <div class="figure">
        <span class="muted">Reste à vivre</span>
        <strong [class.negative]="a.remaining_income_minor < 0">{{
          a.remaining_income_minor | money
        }}</strong>
        <span class="small muted">par mois</span>
      </div>
      @if (
        a.savings_capacity_before_minor !== null && a.savings_capacity_before_minor !== undefined
      ) {
        <div class="figure">
          <span class="muted">Capacité d'épargne</span>
          <strong>
            {{ a.savings_capacity_before_minor | money }} →
            <span [class.negative]="(a.savings_capacity_after_minor ?? 0) < 0">{{
              a.savings_capacity_after_minor | money
            }}</span>
          </strong>
          <span class="small muted">par mois, avant → après</span>
        </div>
      }
    </div>
    @if (a.duration_exceeded) {
      <p class="warn">
        <mat-icon>warning</mat-icon>Durée au-delà de la norme du HCSF pour un logement (25 ans, 27
        dans le neuf) : indicatif, rien n'est bloqué.
      </p>
    }
  `,
  styles: `
    .figures {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
      gap: 1rem;
    }
    .figure {
      display: flex;
      flex-direction: column;
      gap: 0.125rem;
      strong {
        font: var(--mat-sys-title-large);
      }
    }
    .ratio {
      display: flex;
      align-items: center;
      gap: 0.25rem;
      mat-icon {
        font-size: 1.25rem;
        width: 1.25rem;
        height: 1.25rem;
      }
      &.ok mat-icon {
        color: var(--cf-success, #2e7d32);
      }
      &.over {
        color: var(--cf-warning, #b26a00);
      }
    }
    .negative {
      color: var(--mat-sys-error);
    }
    .warn {
      display: flex;
      gap: 0.5rem;
      align-items: flex-start;
      margin: 0.75rem 0 0;
      color: var(--cf-warning, #b26a00);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class Affordability {
  readonly value = input.required<AffordabilityView>();
  protected readonly rate = formatRate;
}

/** Que puis-je acheter ? — les détails repliés : années à venir, sensibilité, patrimoine (R-CRE-33 à R-CRE-35). */
@Component({
  selector: 'app-capacity-details',
  imports: [MatExpansionModule, NgxEchartsDirective, MoneyPipe, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @let r = result();
    <mat-accordion multi>
      @if (r.years.length > 1) {
        <mat-expansion-panel>
          <mat-expansion-panel-header>
            <mat-panel-title>Dans les années à venir</mat-panel-title>
          </mat-expansion-panel-header>
          <div class="years">
            <span class="head">Date</span><span class="head r wide">Revenus</span
            ><span class="head r">Mensualité max.</span
            ><span class="head r wide">Apport possible</span
            ><span class="head r">Prix maximal</span>
            @for (y of r.years; track y.date) {
              <span>{{ $first ? "Aujourd'hui" : (y.date | frDate: 'month') }}</span>
              <span class="r wide">{{ y.income_minor | money }}</span>
              <span class="r">{{ y.max_payment_minor | money }}</span>
              <span class="r wide">{{ y.down_payment_minor | money }}</span>
              <strong class="r">{{ y.max_price_minor | money }}</strong>
            }
          </div>
          <p class="muted small">
            Salaires, dépenses et capacité d'épargne projetés du scénario par défaut : des
            hypothèses, pas des prévisions.
          </p>
        </mat-expansion-panel>
      }
      <mat-expansion-panel>
        <mat-expansion-panel-header>
          <mat-panel-title>Sensibilité</mat-panel-title>
        </mat-expansion-panel-header>
        <div class="grid" [style.--columns]="rates().length">
          <span></span>
          @for (rate of rates(); track rate) {
            <span class="head r">{{ rateLabel(rate) }}</span>
          }
          @for (d of durations(); track d) {
            <span class="head">{{ durationText(d) }}</span>
            @for (rate of rates(); track rate) {
              <span class="r" [class.current]="isCurrent(d, rate)">{{ cell(d, rate) }}</span>
            }
          }
        </div>
      </mat-expansion-panel>
      @if (chart(); as options) {
        <mat-expansion-panel>
          <mat-expansion-panel-header>
            <mat-panel-title>Patrimoine projeté sur 10 ans</mat-panel-title>
          </mat-expansion-panel-header>
          <ng-template matExpansionPanelContent>
            <div echarts [options]="options" class="chart"></div>
          </ng-template>
          <p class="muted small">
            Avec l'achat au prix maximal : le bien garde son prix, seuls les frais, les intérêts et
            l'assurance appauvrissent. Approximation : l'apport ne rapporte plus d'intérêts, ce que
            la courbe ne retire pas.
          </p>
        </mat-expansion-panel>
      }
    </mat-accordion>
  `,
  styles: `
    .years {
      display: grid;
      grid-template-columns: auto repeat(4, minmax(6rem, 1fr));
      gap: 0.25rem 1rem;
      @media (max-width: 599.98px) {
        grid-template-columns: auto repeat(2, minmax(0, 1fr));
        .wide {
          display: none;
        }
      }
    }
    .grid {
      display: grid;
      grid-template-columns: auto repeat(var(--columns), minmax(5rem, 1fr));
      gap: 0.25rem 1rem;
      .current {
        font-weight: 600;
        color: var(--mat-sys-primary);
      }
    }
    .head {
      color: var(--mat-sys-on-surface-variant);
    }
    .r {
      text-align: right;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .chart {
      width: 100%;
      height: 300px;
    }
  `,
})
export class CapacityDetails {
  readonly result = input.required<PurchaseCapacity>();
  readonly conditions = input.required<CreditConditions>();

  protected readonly durationText = formatDuration;
  protected readonly rateLabel = formatRate;

  protected readonly durations = computed(() => [
    ...new Set(this.result().sensitivity.map((c) => c.duration_months)),
  ]);
  protected readonly rates = computed(() => [
    ...new Set(this.result().sensitivity.map((c) => c.annual_rate_ppm)),
  ]);

  protected cell(duration: number, rate: number): string {
    const c = this.result().sensitivity.find(
      (s) => s.duration_months === duration && s.annual_rate_ppm === rate,
    );
    return c ? formatMoney(c.max_price_minor) : '—';
  }

  protected isCurrent(duration: number, rate: number): boolean {
    const c = this.conditions();
    return c.duration_months === duration && c.annual_rate_ppm === rate;
  }

  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const points = this.result().net_worth;
    if (points.length === 0) return null;
    return {
      grid: { left: 8, right: 16, top: 40, bottom: 8, containLabel: true },
      legend: { top: 0 },
      tooltip: { trigger: 'axis', valueFormatter: (v: number) => formatChartValue(v) },
      xAxis: { type: 'time' },
      yAxis: { type: 'value', axisLabel: { formatter: axisMoney } },
      series: [
        {
          name: 'Sans achat',
          type: 'line',
          showSymbol: false,
          lineStyle: { type: 'dashed', width: 2 },
          data: points.map((p) => [p.date, toChartValue(p.without_minor)]),
        },
        {
          name: "Avec l'achat",
          type: 'line',
          showSymbol: false,
          lineStyle: { width: 3 },
          data: points.map((p) => [p.date, toChartValue(p.with_minor)]),
        },
      ],
    };
  });
}
