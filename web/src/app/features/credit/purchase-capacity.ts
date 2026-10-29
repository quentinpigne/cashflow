import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  computed,
  inject,
  signal,
} from '@angular/core';
import { takeUntilDestroyed, toObservable } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';
import {
  catchError,
  debounceTime,
  distinctUntilChanged,
  filter,
  firstValueFrom,
  of,
  switchMap,
} from 'rxjs';

import { Insurance } from '../../api/models/insurance';
import { ProjectKind } from '../../api/models/project-kind';
import { PurchaseCapacity } from '../../api/models/purchase-capacity';
import { PurchaseHypotheses } from '../../api/models/purchase-hypotheses';
import { CreditService } from '../../api/services/credit.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import {
  formatChartValue,
  formatMoney,
  formatRate,
  parseRatePpm,
  toChartValue,
} from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { editable, moneyOrZero } from './applications';
import {
  INSURANCE_LABELS,
  LoansTabs,
  PROJECT_KINDS,
  PROJECT_KIND_LABELS,
  formatDuration,
  insuranceOf,
  rateEditable,
} from './credit-shared';

const axisMoney = (v: number) => formatChartValue(v, 'EUR', { bare: true }).replace(/,00$/, '');

/** E40 — Capacité d'achat (CRE-07). */
@Component({
  selector: 'app-purchase-capacity',
  imports: [
    FormsModule,
    LoansTabs,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
    NgxEchartsDirective,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header"><h1>Prêts</h1></div>
      <app-loans-tabs />

      @if (result(); as r) {
        <div class="layout">
          <section class="card hypotheses">
            <h2>Hypothèses</h2>
            <div class="fields">
              <mat-form-field>
                <mat-label>Projet</mat-label>
                <mat-select [ngModel]="project()" (ngModelChange)="project.set($event)">
                  @for (k of projectKinds; track k) {
                    <mat-option [value]="k">{{ projectLabels[k] }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Taux nominal</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="rate()"
                  (ngModelChange)="rate.set($event)"
                />
                <span matTextSuffix>%</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Durée</mat-label>
                <input
                  matInput
                  inputmode="numeric"
                  [ngModel]="duration()"
                  (ngModelChange)="duration.set($event)"
                />
                <span matTextSuffix>mois</span>
                <mat-hint>{{ durationLabel() }}</mat-hint>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Assurance</mat-label>
                <mat-select [ngModel]="insuranceMode()" (ngModelChange)="insuranceMode.set($event)">
                  @for (m of insuranceModes; track m) {
                    <mat-option [value]="m">{{ insuranceLabels[m] }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              @if (insuranceMode() === 'FIXED') {
                <mat-form-field>
                  <mat-label>Assurance mensuelle</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="insuranceAmount()"
                    (ngModelChange)="insuranceAmount.set($event)"
                  />
                  <span matTextSuffix>€</span>
                </mat-form-field>
              } @else if (insuranceMode() === 'RATE_ON_PRINCIPAL') {
                <mat-form-field>
                  <mat-label>Taux d'assurance</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="insuranceRate()"
                    (ngModelChange)="insuranceRate.set($event)"
                  />
                  <span matTextSuffix>%</span>
                </mat-form-field>
              }
              <mat-form-field>
                <mat-label>Frais de dossier</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="fees()"
                  (ngModelChange)="fees.set($event)"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Frais de garantie</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="guarantee()"
                  (ngModelChange)="guarantee.set($event)"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Endettement visé</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="debtRatio()"
                  (ngModelChange)="debtRatio.set($event)"
                />
                <span matTextSuffix>%</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Réserve de sécurité</mat-label>
                <input
                  matInput
                  inputmode="numeric"
                  [ngModel]="reserveMonths()"
                  (ngModelChange)="reserveMonths.set($event)"
                />
                <span matTextSuffix>mois</span>
                <mat-hint>de dépenses ({{ r.expenses_minor | money }} par mois)</mat-hint>
              </mat-form-field>
            </div>

            <h3>Épargne mobilisable pour l'apport</h3>
            @for (a of r.accounts; track a.id) {
              <div class="check-line">
                <mat-checkbox [checked]="accounts().has(a.id)" (change)="toggleAccount(a.id)">{{
                  a.name
                }}</mat-checkbox>
                <span class="amount">{{ a.balance_minor | money }}</span>
              </div>
            }

            @if (r.loans.length > 0) {
              <h3>Prêts en cours remboursés par ce crédit</h3>
              @for (l of r.loans; track l.loan_id) {
                <div class="check-line">
                  <mat-checkbox
                    [checked]="excluded().has(l.loan_id)"
                    (change)="toggleLoan(l.loan_id)"
                    >{{ l.name }}</mat-checkbox
                  >
                  <span class="amount">{{ l.monthly_minor | money }}</span>
                </div>
              }
            }

            <h3>Autres revenus et charges mensuels</h3>
            <div class="fields">
              <mat-form-field>
                <mat-label>Autres revenus</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="otherIncome()"
                  (ngModelChange)="otherIncome.set($event)"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Autres charges</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="otherCharges()"
                  (ngModelChange)="otherCharges.set($event)"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Revenus du co-emprunteur</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="coIncome()"
                  (ngModelChange)="coIncome.set($event)"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Charges du co-emprunteur</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="coCharges()"
                  (ngModelChange)="coCharges.set($event)"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
            </div>
            @if (busy()) {
              <mat-progress-bar mode="indeterminate" />
            }
            @if (error()) {
              <p class="error" role="alert">{{ error() }}</p>
            }
          </section>

          <div class="results">
            <section class="card today">
              <span class="muted">Prix d'achat maximal aujourd'hui</span>
              <strong class="price">{{ r.today.max_price_minor | money }}</strong>
              <span class="muted small"
                >+ {{ r.today.notary_fees_minor | money }} de frais de notaire estimés</span
              >
              <div class="cascade">
                <span>Revenus mensuels</span
                ><span class="amount">{{ r.today.income_minor | money }}</span>
                <span>Charges mensuelles</span
                ><span class="amount">{{ r.today.charges_minor | money }}</span>
                <span>Mensualité maximale</span
                ><strong class="amount">{{ r.today.max_payment_minor | money }}</strong>
                <span>Capital empruntable</span
                ><strong class="amount">{{ r.today.capital_minor | money }}</strong>
                <span>Épargne mobilisable</span
                ><span class="amount">{{ r.today.savings_minor | money }}</span>
                <span>Réserve gardée</span
                ><span class="amount">− {{ r.today.reserve_minor | money }}</span>
                <span>Apport possible</span
                ><strong class="amount">{{ r.today.down_payment_minor | money }}</strong>
                <span>Budget, frais de dossier déduits</span
                ><strong class="amount">{{ r.today.budget_minor | money }}</strong>
              </div>
              @if (r.today.max_payment_minor <= 0) {
                <p class="hint">
                  <mat-icon>info</mat-icon>Vos charges atteignent déjà l'endettement visé : aucun
                  crédit n'est possible sans exclure un prêt remboursé par ce crédit ou réduire les
                  charges.
                </p>
              }
              @if (r.today.savings_minor <= r.today.reserve_minor) {
                <p class="hint">
                  <mat-icon>info</mat-icon>L'épargne mobilisable ne dépasse pas la réserve de
                  sécurité : pas d'apport possible.
                </p>
              }
              <button
                mat-flat-button
                [disabled]="r.today.max_price_minor <= 0"
                (click)="createApplication(r)"
              >
                <mat-icon>assignment</mat-icon>Créer une demande
              </button>
            </section>

            <section class="card">
              <h2>Effet de l'achat</h2>
              <div class="figures">
                <div>
                  <span class="muted">Épargne restante</span>
                  <strong>{{ r.savings_after_minor | money }}</strong>
                  @if (
                    r.savings_after_tenths_of_months !== null &&
                    r.savings_after_tenths_of_months !== undefined
                  ) {
                    <span class="small muted"
                      >{{ months(r.savings_after_tenths_of_months) }} de dépenses</span
                    >
                  }
                </div>
                <div>
                  <span class="muted">Échéance du crédit</span>
                  <strong>{{ r.today.payment_minor | money }}</strong>
                </div>
                <div>
                  <span class="muted">Reste à vivre</span>
                  <strong>{{ r.remaining_income_minor | money }}</strong>
                </div>
                @if (
                  r.savings_capacity_before_minor !== null &&
                  r.savings_capacity_before_minor !== undefined
                ) {
                  <div>
                    <span class="muted">Capacité d'épargne mensuelle</span>
                    <strong>
                      {{ r.savings_capacity_before_minor | money }} →
                      <span [class.negative]="(r.savings_capacity_after_minor ?? 0) < 0">{{
                        r.savings_capacity_after_minor | money
                      }}</span>
                    </strong>
                    <span class="small muted">avant → après l'achat</span>
                  </div>
                }
              </div>
            </section>

            @if (r.years.length > 1) {
              <section class="card">
                <h2>Dans les années à venir</h2>
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
              </section>
            }

            <section class="card">
              <h2>Sensibilité</h2>
              <div class="grid" [style.--columns]="rates().length">
                <span></span>
                @for (rate of rates(); track rate) {
                  <span class="head r">{{ rateLabel(rate) }}</span>
                }
                @for (d of durations(); track d) {
                  <span class="head">{{ durationText(d) }}</span>
                  @for (rate of rates(); track rate) {
                    <span class="r" [class.current]="isCurrent(r, d, rate)">{{
                      cell(r, d, rate)
                    }}</span>
                  }
                }
              </div>
            </section>

            @if (chart(); as options) {
              <section class="card">
                <h2>Patrimoine projeté sur 10 ans</h2>
                <div echarts [options]="options" class="chart"></div>
                <p class="muted small">
                  Avec l'achat au prix maximal : le bien garde son prix, seuls les frais, les
                  intérêts et l'assurance appauvrissent. Approximation : l'apport ne rapporte plus
                  d'intérêts, ce que la courbe ne retire pas.
                </p>
              </section>
            }
          </div>
        </div>
      } @else {
        <mat-progress-bar mode="indeterminate" />
      }
    </div>
  `,
  styles: `
    .layout {
      display: grid;
      grid-template-columns: minmax(18rem, 2fr) minmax(0, 3fr);
      gap: 1rem;
      align-items: start;
      @media (max-width: 899.98px) {
        grid-template-columns: minmax(0, 1fr);
      }
    }
    .card {
      margin-bottom: 1rem;
    }
    h2 {
      margin: 0 0 0.75rem;
      font: var(--mat-sys-title-medium);
    }
    h3 {
      margin: 1rem 0 0.25rem;
      font: var(--mat-sys-title-small);
    }
    .fields {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
      gap: calc(var(--field-gap) * 1.5) var(--field-gap);
      align-items: start;
    }
    .check-line {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .today {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      .price {
        font: var(--mat-sys-display-small);
        color: var(--mat-sys-primary);
      }
      button {
        align-self: flex-start;
        margin-top: 0.75rem;
      }
    }
    .hint {
      display: flex;
      gap: 0.5rem;
      align-items: flex-start;
      margin: 0.75rem 0 0;
      color: var(--mat-sys-on-surface-variant);
    }
    .cascade {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 0.25rem 1rem;
      margin-top: 0.75rem;
    }
    .figures {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
      gap: 1rem;
      div {
        display: flex;
        flex-direction: column;
        gap: 0.125rem;
      }
      strong {
        font: var(--mat-sys-title-large);
      }
    }
    .negative {
      color: var(--mat-sys-error);
    }
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
      grid-template-columns: auto repeat(var(--columns), minmax(6rem, 1fr));
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
    .error {
      color: var(--mat-sys-error);
    }
    .chart {
      width: 100%;
      height: 300px;
    }
  `,
})
export class PurchaseCapacityPage {
  private readonly api = inject(CreditService);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);

  protected readonly projectKinds = PROJECT_KINDS;
  protected readonly projectLabels = PROJECT_KIND_LABELS;
  protected readonly insuranceModes: Insurance['mode'][] = ['NONE', 'FIXED', 'RATE_ON_PRINCIPAL'];
  protected readonly insuranceLabels = INSURANCE_LABELS;
  protected readonly durationText = formatDuration;
  protected readonly rateLabel = formatRate;

  protected readonly result = signal<PurchaseCapacity | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  // Hypothèses saisies (R-CRE-30), reprises du réglage au chargement.
  protected readonly project = signal<ProjectKind>('EXISTING_HOME');
  protected readonly rate = signal('');
  protected readonly duration = signal('');
  protected readonly insuranceMode = signal<Insurance['mode']>('NONE');
  protected readonly insuranceAmount = signal('');
  protected readonly insuranceRate = signal('');
  protected readonly fees = signal('');
  protected readonly guarantee = signal('');
  protected readonly debtRatio = signal('');
  protected readonly reserveMonths = signal('');
  protected readonly otherIncome = signal('');
  protected readonly otherCharges = signal('');
  protected readonly coIncome = signal('');
  protected readonly coCharges = signal('');
  protected readonly accounts = signal<ReadonlySet<string>>(new Set());
  protected readonly excluded = signal<ReadonlySet<string>>(new Set());
  private readonly loaded = signal(false);

  protected readonly durationLabel = computed(() => {
    const months = Number(this.duration());
    return Number.isInteger(months) && months > 0 ? formatDuration(months) : '';
  });

  /** Hypothèses complètes, ou `null` tant qu'une saisie est invalide. */
  private readonly draft = computed<PurchaseHypotheses | null>(() => {
    if (!this.loaded()) return null;
    const rate = parseRatePpm(this.rate());
    const debt = parseRatePpm(this.debtRatio());
    const duration = /^\d{1,3}$/.test(this.duration().trim()) ? Number(this.duration()) : null;
    const reserve = /^\d{1,3}$/.test(this.reserveMonths().trim())
      ? Number(this.reserveMonths())
      : null;
    const insurance = insuranceOf(
      this.insuranceMode(),
      this.insuranceAmount(),
      this.insuranceRate(),
    );
    const amounts = [
      this.fees(),
      this.guarantee(),
      this.otherIncome(),
      this.otherCharges(),
      this.coIncome(),
      this.coCharges(),
    ].map(moneyOrZero);
    if (rate === null || debt === null || !duration || reserve === null || !insurance) return null;
    if (amounts.some((a) => a === null || a < 0)) return null;
    const [fees, guarantee, otherIncome, otherCharges, coIncome, coCharges] = amounts as number[];
    return {
      project_kind: this.project(),
      annual_rate_ppm: rate,
      duration_months: duration,
      insurance,
      fees_minor: fees,
      guarantee_fees_minor: guarantee,
      max_debt_ratio_ppm: debt,
      reserve_months: reserve,
      account_ids: [...this.accounts()],
      other_income_minor: otherIncome,
      other_charges_minor: otherCharges,
      co_borrower_income_minor: coIncome,
      co_borrower_charges_minor: coCharges,
      excluded_loan_ids: [...this.excluded()],
    };
  });

  constructor() {
    void this.load();
    // Chaque changement recalcule la capacité et garde les hypothèses (R-CRE-30).
    toObservable(this.draft)
      .pipe(
        filter((d): d is PurchaseHypotheses => d !== null),
        debounceTime(400),
        distinctUntilChanged((a, b) => JSON.stringify(a) === JSON.stringify(b)),
        switchMap((body) => {
          this.busy.set(true);
          return this.api.savePurchaseHypotheses({ body }).pipe(
            catchError((e) => {
              this.error.set(errorMessage(e));
              return of(null);
            }),
          );
        }),
        takeUntilDestroyed(inject(DestroyRef)),
      )
      .subscribe((result) => {
        this.busy.set(false);
        if (result) {
          this.error.set(null);
          this.result.set(result);
        }
      });
  }

  private async load(): Promise<void> {
    const r = await firstValueFrom(this.api.purchaseCapacity());
    const h = r.hypotheses;
    this.project.set(h.project_kind);
    this.rate.set(rateEditable(h.annual_rate_ppm));
    this.duration.set(String(h.duration_months));
    this.insuranceMode.set(h.insurance.mode);
    if (h.insurance.mode === 'FIXED') this.insuranceAmount.set(editable(h.insurance.amount_minor));
    if (h.insurance.mode === 'RATE_ON_PRINCIPAL')
      this.insuranceRate.set(rateEditable(h.insurance.rate_ppm));
    this.fees.set(editable(h.fees_minor));
    this.guarantee.set(editable(h.guarantee_fees_minor));
    this.debtRatio.set(rateEditable(h.max_debt_ratio_ppm));
    this.reserveMonths.set(String(h.reserve_months));
    this.otherIncome.set(editable(h.other_income_minor));
    this.otherCharges.set(editable(h.other_charges_minor));
    this.coIncome.set(editable(h.co_borrower_income_minor));
    this.coCharges.set(editable(h.co_borrower_charges_minor));
    this.accounts.set(new Set(r.accounts.filter((a) => a.selected).map((a) => a.id)));
    this.excluded.set(new Set(h.excluded_loan_ids));
    this.result.set(r);
    this.loaded.set(true);
  }

  protected toggleAccount(id: string): void {
    this.accounts.update((set) => toggled(set, id));
  }

  protected toggleLoan(id: string): void {
    this.excluded.update((set) => toggled(set, id));
  }

  protected months(tenths: number): string {
    return `${(tenths / 10).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} mois`;
  }

  protected readonly durations = computed(() => [
    ...new Set((this.result()?.sensitivity ?? []).map((c) => c.duration_months)),
  ]);
  protected readonly rates = computed(() => [
    ...new Set((this.result()?.sensitivity ?? []).map((c) => c.annual_rate_ppm)),
  ]);

  protected cell(r: PurchaseCapacity, duration: number, rate: number): string {
    const c = r.sensitivity.find(
      (s) => s.duration_months === duration && s.annual_rate_ppm === rate,
    );
    return c ? formatMoney(c.max_price_minor) : '—';
  }

  protected isCurrent(r: PurchaseCapacity, duration: number, rate: number): boolean {
    return r.hypotheses.duration_months === duration && r.hypotheses.annual_rate_ppm === rate;
  }

  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const points = this.result()?.net_worth ?? [];
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

  /** Crée une demande au prix maximal, avec une simulation retenue (R-CRE-36). */
  protected async createApplication(r: PurchaseCapacity): Promise<void> {
    try {
      const view = await firstValueFrom(
        this.api.applicationFromCapacity({
          body: { name: `Achat jusqu'à ${formatMoney(r.today.max_price_minor)}` },
        }),
      );
      this.version.bump();
      await this.router.navigate(['/prets/demandes', view.id]);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }
}

function toggled(set: ReadonlySet<string>, id: string): ReadonlySet<string> {
  const next = new Set(set);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}
