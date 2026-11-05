import { HttpClient } from '@angular/common/http';
import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Router } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';
import { from } from 'rxjs';

import { CategoryKind } from '../../api/models/category-kind';
import { LocationGrouping } from '../../api/models/location-grouping';
import { LocationLine } from '../../api/models/location-line';
import { ReportPeriod } from '../../api/models/report-period';
import { PayeesService } from '../../api/services/payees.service';
import { DataVersion } from '../../core/data-version';
import { currentMonth } from '../../core/dates';
import { formatChartValue, toChartValue } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { countryName } from '../../shared/address';
import { MemberSelections } from '../../shared/member-filter';
import { PeriodPicker } from '../../shared/period-picker';

/** Nom du fond de carte enregistré auprès d'ECharts. */
const MAP = 'departements';

interface DepartmentsGeoJson {
  features: { properties: { code: string; nom: string } }[];
}

/** Fond de carte embarqué (R-TIE-29) : chargé une fois, avec l'onglet ; renvoie ses départements. */
let mapReady: Promise<{ code: string; name: string }[]> | null = null;

function loadMap(http: HttpClient): Promise<{ code: string; name: string }[]> {
  mapReady ??= Promise.all([
    import('echarts'),
    new Promise<DepartmentsGeoJson>((resolve, reject) =>
      http
        .get<DepartmentsGeoJson>('maps/departements.json')
        .subscribe({ next: resolve, error: reject }),
    ),
  ]).then(([echarts, geoJson]) => {
    echarts.registerMap(MAP, geoJson as unknown as Parameters<typeof echarts.registerMap>[1]);
    return geoJson.features.map((f) => ({ code: f.properties.code, name: f.properties.nom }));
  });
  mapReady.catch(() => (mapReady = null));
  return mapReady;
}

/** E15, onglet « Par lieu » : dépenses ou revenus par pays, département ou ville, et carte (TIE-07). */
@Component({
  selector: 'app-location-analysis',
  imports: [
    PeriodPicker,
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
          <app-period-picker
            [mode]="period() === 'YEAR' ? 'year' : 'month'"
            [value]="period() === 'YEAR' ? year() : month()"
            [max]="period() === 'YEAR' ? currentYear : currentMonth"
            (valueChange)="choose($event)"
          />
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
          <mat-button-toggle value="COUNTRY">Pays</mat-button-toggle>
          <mat-button-toggle value="DEPARTMENT">Départements</mat-button-toggle>
          <mat-button-toggle value="CITY">Villes</mat-button-toggle>
        </mat-button-toggle-group>
      </div>

      @if (report.value(); as r) {
        <p class="muted">
          Du {{ r.from | frDate }} au {{ r.to | frDate }}, comparé au
          {{ r.previous_from | frDate }} – {{ r.previous_to | frDate }}. Le lieu d'une transaction
          est son adresse, sinon celle de son tiers ; seules les transactions qui ont l'une ou
          l'autre comptent.
        </p>
        <div class="layout">
          <table class="table">
            <thead>
              <tr>
                <th>Lieu ou tiers</th>
                <th class="right">Montant</th>
                <th class="right secondary">À catégoriser</th>
                <th class="right secondary">Tiers</th>
                <th class="right">{{ r.current ? 'Période préc. à date' : 'Période préc.' }}</th>
              </tr>
            </thead>
            <tbody>
              @for (line of r.lines; track line.key) {
                <tr class="root">
                  <td>
                    <button
                      mat-icon-button
                      class="expand"
                      (click)="toggle(line)"
                      [attr.aria-label]="(isExpanded(line) ? 'Replier ' : 'Déplier ') + name(line)"
                    >
                      <mat-icon>{{ isExpanded(line) ? 'expand_less' : 'expand_more' }}</mat-icon>
                    </button>
                    <span [class.muted]="isSpecial(line)">{{ name(line) }}</span>
                  </td>
                  <td class="amount">{{ line.amount_minor | money }}</td>
                  <td class="amount muted secondary">
                    {{ line.uncategorized_minor ? (line.uncategorized_minor | money) : '' }}
                  </td>
                  <td class="amount muted secondary">{{ payeeCount(line) }}</td>
                  <td class="amount muted">{{ line.previous_minor | money }}</td>
                </tr>
                @if (isExpanded(line)) {
                  @for (child of line.payees; track child.payee_id) {
                    <tr class="child">
                      <td>
                        @if (child.payee_id) {
                          <button class="link" (click)="open(child.payee_id)">
                            {{ child.name }}
                          </button>
                        } @else {
                          <span class="muted">{{ child.name }}</span>
                        }
                      </td>
                      <td class="amount">{{ child.amount_minor | money }}</td>
                      <td class="amount muted secondary">
                        {{ child.uncategorized_minor ? (child.uncategorized_minor | money) : '' }}
                      </td>
                      <td class="secondary"></td>
                      <td class="amount muted">{{ child.previous_minor | money }}</td>
                    </tr>
                  }
                }
              } @empty {
                <tr>
                  <td colspan="5" class="empty">Aucune transaction localisée sur cette période.</td>
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

          <section class="map-panel" aria-label="Carte des dépenses">
            @if (mapChart(); as options) {
              <div echarts [options]="options" class="map" (chartClick)="onMapClick($event)"></div>
            } @else if (map.isLoading()) {
              <mat-progress-bar mode="indeterminate" />
            } @else if (map.error()) {
              <p class="muted">Fond de carte indisponible.</p>
            }
            @if (r.off_map) {
              <p class="muted">
                {{ r.off_map }} lieu(x) hors de la carte (sans localisation, outre-mer ou étranger).
              </p>
            }
          </section>
        </div>
      } @else if (report.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
    </div>
  `,
  styleUrl: '../analyses/analyses.scss',
  styles: `
    .layout {
      display: grid;
      grid-template-columns: minmax(0, 3fr) minmax(0, 2fr);
      gap: 1.5rem;
      align-items: start;
    }

    .map {
      width: 100%;
      height: 460px;
    }

    @media (max-width: 959px) {
      .layout {
        grid-template-columns: minmax(0, 1fr);
      }

      .map {
        height: 380px;
      }
    }

    @media (max-width: 599px) {
      .secondary {
        display: none;
      }
    }
  `,
})
export class LocationAnalysis {
  private readonly api = inject(PayeesService);
  private readonly http = inject(HttpClient);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);

  protected readonly period = signal<ReportPeriod>('LAST12_MONTHS');
  protected readonly month = signal(currentMonth());
  protected readonly year = signal(new Date().getFullYear());
  protected readonly kind = signal<CategoryKind>('EXPENSE');
  protected readonly groupBy = signal<LocationGrouping>('CITY');
  protected readonly expanded = signal<Set<string>>(new Set());

  private readonly members = inject(MemberSelections).of('analyses', 'flows');
  protected readonly report = rxResource({
    params: () => ({
      period: this.period(),
      month: this.month(),
      year: this.year(),
      kind: this.kind(),
      group_by: this.groupBy(),
      version: this.version.value(),
      members: this.members.param(),
    }),
    stream: ({ params }) =>
      this.api.payeeLocationReport({
        period: params.period,
        month: params.period === 'MONTH' ? params.month : undefined,
        year: params.period === 'YEAR' ? params.year : undefined,
        kind: params.kind,
        group_by: params.group_by,
        members: params.members,
      }),
  });

  protected readonly map = rxResource({
    stream: () => from(loadMap(this.http)),
  });

  /**
   * Départements en dégradé selon le montant, tiers et adresses de transaction localisés en cercles
   * proportionnels (R-TIE-29, R-TRX-60).
   */
  protected readonly mapChart = computed<EChartsCoreOption | null>(() => {
    const r = this.report.value();
    const all = this.map.value();
    if (!r || !all) return null;
    // Tous les départements du fond de carte : sans montant, ils prennent la teinte la plus claire.
    const amounts = new Map(r.departments.map((d) => [d.code, d]));
    const departments = all.map(({ code, name }) => {
      const d = amounts.get(code);
      return { name: code, label: name, value: d ? toChartValue(d.amount_minor) : 0 };
    });
    const max = Math.max(1, ...departments.map((d) => d.value));
    const points = r.points.map((p) => ({
      name: p.name,
      id: p.payee_id,
      value: [p.longitude_e6 / 1_000_000, p.latitude_e6 / 1_000_000, toChartValue(p.amount_minor)],
    }));
    const maxPoint = Math.max(1, ...points.map((p) => p.value[2]));
    return {
      tooltip: {
        trigger: 'item',
        formatter: (item: {
          seriesType: string;
          name: string;
          value: unknown;
          data?: { label?: string };
        }) => {
          if (item.seriesType === 'scatter') {
            const value = (item.value as number[])[2] ?? 0;
            return `${item.name}<br>${formatChartValue(value)}`;
          }
          const label = item.data?.label ?? item.name;
          return typeof item.value === 'number' && !Number.isNaN(item.value)
            ? `${label}<br>${formatChartValue(item.value)}`
            : label;
        },
      },
      visualMap: {
        min: 0,
        max,
        seriesIndex: 0,
        calculable: false,
        orient: 'horizontal',
        left: 'center',
        bottom: 0,
        itemHeight: 120,
        text: [formatChartValue(max, 'EUR', { bare: true }).replace(/,00$/, ''), '0'],
        inRange: { color: ['#e3f2fd', '#1565c0'] },
      },
      geo: {
        map: MAP,
        nameProperty: 'code',
        roam: true,
        top: 8,
        bottom: 40,
        itemStyle: { areaColor: '#f5f5f5', borderColor: '#9e9e9e', borderWidth: 0.5 },
        emphasis: { label: { show: false }, itemStyle: { areaColor: '#ffe082' } },
      },
      series: [
        { type: 'map', geoIndex: 0, data: departments },
        {
          type: 'scatter',
          coordinateSystem: 'geo',
          data: points,
          symbolSize: (value: number[]) => 6 + 24 * Math.sqrt((value[2] ?? 0) / maxPoint),
          itemStyle: { color: '#ef6c00', opacity: 0.8, borderColor: '#fff', borderWidth: 1 },
          emphasis: { scale: 1.2 },
        },
      ],
    };
  });

  /** Nom affiché d'un lieu : un pays se traduit à partir de son code. */
  protected name(line: LocationLine): string {
    if (line.kind === 'COUNTRY' && line.country_code) return countryName(line.country_code);
    if (line.kind === 'DEPARTMENT' && line.department_code) {
      return `${line.name} (${line.department_code})`;
    }
    if (line.kind === 'CITY' && line.country_code && line.country_code !== 'FR') {
      return `${line.name} (${countryName(line.country_code)})`;
    }
    return line.name;
  }

  protected isSpecial(line: LocationLine): boolean {
    return (
      line.kind === 'NO_ADDRESS' || line.kind === 'ABROAD' || line.kind === 'UNKNOWN_DEPARTMENT'
    );
  }

  protected readonly currentMonth = currentMonth();
  protected readonly currentYear = new Date().getFullYear();

  protected choose(value: string | number): void {
    if (this.period() === 'YEAR') this.year.set(Number(value));
    else this.month.set(String(value));
  }

  protected toggle(line: LocationLine): void {
    this.expanded.update((s) => {
      const next = new Set(s);
      if (next.has(line.key)) next.delete(line.key);
      else next.add(line.key);
      return next;
    });
  }

  /** Tiers d'un lieu, sans compter « Sans tiers » (transactions localisées par leur adresse). */
  protected payeeCount(line: LocationLine): number {
    return line.payees.filter((p) => p.payee_id).length;
  }

  protected isExpanded(line: LocationLine): boolean {
    return this.expanded().has(line.key);
  }

  protected open(payeeId: string | null | undefined): void {
    if (payeeId) void this.router.navigate(['/tiers', payeeId]);
  }

  /** Un clic sur un tiers de la carte ouvre sa fiche (E32) ; une adresse de transaction n'en a pas. */
  protected onMapClick(event: { seriesType?: string; data?: unknown }): void {
    if (event.seriesType !== 'scatter') return;
    this.open((event.data as { id?: string } | undefined)?.id);
  }
}
