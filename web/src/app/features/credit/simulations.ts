import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { catchError, debounceTime, firstValueFrom, map, of, switchMap } from 'rxjs';

import { ComputeInput } from '../../api/models/compute-input';
import { Insurance } from '../../api/models/insurance';
import { LoanKind } from '../../api/models/loan-kind';
import { SimulationGoal } from '../../api/models/simulation-goal';
import { SimulationView } from '../../api/models/simulation-view';
import { CreditService } from '../../api/services/credit.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { LOAN_KIND_LABELS } from '../../core/labels';
import { formatRate, parseMoney, parseRatePpm, toEditable } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { MoneyPipe } from '../../shared/money.pipe';
import { LoanDialog } from '../loans/loans';
import {
  INSURANCE_LABELS,
  LoansTabs,
  compareSimulations,
  editSimulation,
  formatDuration,
  insuranceOf,
  rateEditable,
  showSchedule,
} from './credit-shared';

/** E36 — Simulations : le simulateur et les simulations enregistrées (CRE-01 à CRE-03). */
@Component({
  selector: 'app-simulations',
  imports: [
    FormsModule,
    RouterLink,
    LoansTabs,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatSelectModule,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header"><h1>Prêts</h1></div>
      <app-loans-tabs />

      <section class="card simulator">
        <header>
          <h2>{{ editing()?.name ?? 'Simulateur' }}</h2>
          @if (editing()) {
            <button mat-button (click)="reset()">
              <mat-icon>add</mat-icon>Nouvelle simulation
            </button>
          }
        </header>
        <mat-button-toggle-group
          [value]="goal()"
          (change)="goal.set($event.value)"
          aria-label="Ce que je cherche"
        >
          <mat-button-toggle value="PAYMENT">La mensualité</mat-button-toggle>
          <mat-button-toggle value="PRINCIPAL">Le capital</mat-button-toggle>
          <mat-button-toggle value="DURATION">La durée</mat-button-toggle>
        </mat-button-toggle-group>
        <div class="fields">
          <mat-form-field>
            <mat-label>Type de prêt</mat-label>
            <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
              @for (k of kinds; track k) {
                <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          @if (goal() !== 'PRINCIPAL') {
            <mat-form-field>
              <mat-label>Capital</mat-label>
              <input
                matInput
                inputmode="decimal"
                [ngModel]="principal()"
                (ngModelChange)="principal.set($event)"
              />
              <span matTextSuffix>€</span>
            </mat-form-field>
          }
          @if (goal() !== 'DURATION') {
            <mat-form-field>
              <mat-label>Durée</mat-label>
              <input
                matInput
                inputmode="numeric"
                [ngModel]="duration()"
                (ngModelChange)="duration.set($event)"
              />
              <span matTextSuffix>mois</span>
              <mat-hint>{{ durationHint() }}</mat-hint>
            </mat-form-field>
          }
          @if (goal() !== 'PAYMENT') {
            <mat-form-field>
              <mat-label>Mensualité visée</mat-label>
              <input
                matInput
                inputmode="decimal"
                [ngModel]="target()"
                (ngModelChange)="target.set($event)"
              />
              <span matTextSuffix>€</span>
              <mat-hint>Assurance comprise</mat-hint>
            </mat-form-field>
          }
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
        </div>

        @if (outcome(); as o) {
          @if (o.error) {
            <p class="error" role="alert">{{ o.error }}</p>
          } @else if (o.result; as r) {
            <div class="results" aria-live="polite">
              @if (goal() === 'PRINCIPAL') {
                <div class="figure main">
                  <span class="muted">Capital empruntable</span>
                  <strong>{{ r.principal_minor | money }}</strong>
                </div>
              }
              @if (goal() === 'DURATION') {
                <div class="figure main">
                  <span class="muted">Durée</span>
                  <strong>{{ duration_(r.duration_months) }}</strong>
                </div>
              }
              <div class="figure" [class.main]="goal() === 'PAYMENT'">
                <span class="muted">Échéance, assurance comprise</span>
                <strong>{{ r.result.total_payment_minor | money }}</strong>
                <span class="small muted"
                  >{{ r.result.payment_minor | money }} +
                  {{ r.result.insurance_minor | money }} d'assurance</span
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
                <strong>{{ rateText(r.result.apr_ppm) }}</strong>
              </div>
            </div>
            <div class="actions">
              @if (editing(); as e) {
                <button mat-flat-button (click)="update(e)">Enregistrer les modifications</button>
                <button mat-stroked-button (click)="saveAsNew()">Enregistrer comme nouvelle</button>
              } @else {
                <button mat-flat-button (click)="saveAsNew()">
                  <mat-icon>bookmark_add</mat-icon>Enregistrer
                </button>
              }
            </div>
          }
        }
      </section>

      <section class="card">
        <header>
          <h2>Simulations enregistrées</h2>
          <button mat-stroked-button [disabled]="checked().size < 2" (click)="compare()">
            <mat-icon>compare_arrows</mat-icon>Comparer
          </button>
        </header>
        @for (s of simulations(); track s.id) {
          <div class="row">
            <mat-checkbox
              [checked]="checked().has(s.id)"
              (change)="toggle(s.id)"
              [attr.aria-label]="'Comparer ' + s.name"
            />
            <button class="open" (click)="load(s)">
              <span>{{ s.name }}</span>
              <span class="muted small">
                {{ s.principal_minor | money }} · {{ duration_(s.duration_months) }} ·
                {{ rateText(s.annual_rate_ppm) }}
                @if (s.application_name) {
                  · {{ s.application_name }}
                }
                @if (s.institution_name) {
                  · {{ s.institution_name }}
                }
                @if (s.loan_id) {
                  · prêt créé
                }
              </span>
            </button>
            <span class="numbers">
              <span class="amount">{{ s.result.total_payment_minor | money }}</span>
              <span class="muted small"
                >TAEG {{ rateText(s.result.apr_ppm) }} · coût
                {{ s.result.total_cost_minor | money }}</span
              >
            </span>
            <button
              mat-icon-button
              [matMenuTriggerFor]="menu"
              [attr.aria-label]="'Actions sur ' + s.name"
            >
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              <button mat-menu-item (click)="edit(s)"><mat-icon>edit</mat-icon>Modifier</button>
              <button mat-menu-item (click)="duplicate(s)">
                <mat-icon>content_copy</mat-icon>Dupliquer
              </button>
              <button mat-menu-item (click)="schedule(s)">
                <mat-icon>table_chart</mat-icon>Tableau d'amortissement
              </button>
              @if (s.loan_id) {
                <a mat-menu-item [routerLink]="['/prets', s.loan_id]">
                  <mat-icon>real_estate_agent</mat-icon>Voir le prêt
                </a>
              } @else {
                <button mat-menu-item (click)="convert(s)">
                  <mat-icon>real_estate_agent</mat-icon>Transformer en prêt
                </button>
              }
              <button mat-menu-item (click)="remove(s)">
                <mat-icon>delete</mat-icon>Supprimer
              </button>
            </mat-menu>
          </div>
        } @empty {
          <p class="muted">
            Aucune simulation. Calculez un crédit ci-dessus, puis « Enregistrer ».
          </p>
        }
      </section>
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 0.5rem;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .simulator mat-button-toggle-group {
      margin: 0.75rem 0;
      flex-wrap: wrap;
    }
    .fields {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
      gap: calc(var(--field-gap) * 1.5) var(--field-gap);
    }
    .results {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
      gap: 1rem;
      margin: 1rem 0;
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
    .actions {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
    .row {
      display: grid;
      grid-template-columns: auto 1fr auto auto;
      align-items: center;
      gap: 0.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      padding: 0.25rem 0;
      @media (max-width: 599.98px) {
        grid-template-columns: auto 1fr auto;
        .numbers {
          grid-column: 2;
          text-align: left;
        }
      }
    }
    .open {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      background: none;
      border: 0;
      padding: 0.25rem 0;
      color: inherit;
      font: inherit;
      text-align: left;
      cursor: pointer;
    }
    .numbers {
      display: flex;
      flex-direction: column;
      text-align: right;
    }
  `,
})
export class SimulationsPage {
  private readonly api = inject(CreditService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);

  protected readonly kinds: LoanKind[] = ['MORTGAGE', 'CONSUMER', 'OTHER'];
  protected readonly kindLabels = LOAN_KIND_LABELS;
  protected readonly insuranceModes: Insurance['mode'][] = ['NONE', 'FIXED', 'RATE_ON_PRINCIPAL'];
  protected readonly insuranceLabels = INSURANCE_LABELS;
  protected readonly rateText = formatRate;
  protected readonly duration_ = formatDuration;

  protected readonly goal = signal<SimulationGoal>('PAYMENT');
  protected readonly kind = signal<LoanKind>('MORTGAGE');
  protected readonly principal = signal('200 000');
  protected readonly duration = signal('240');
  protected readonly target = signal('1 200');
  protected readonly rate = signal('3,2');
  protected readonly insuranceMode = signal<Insurance['mode']>('RATE_ON_PRINCIPAL');
  protected readonly insuranceAmount = signal('');
  protected readonly insuranceRate = signal('0,3');
  protected readonly fees = signal('1 000');
  protected readonly guarantee = signal('2 000');
  /** Simulation enregistrée chargée dans le simulateur. */
  protected readonly editing = signal<SimulationView | null>(null);
  protected readonly checked = signal<ReadonlySet<string>>(new Set());

  private readonly list = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listLoanSimulations(),
  });
  protected readonly simulations = computed(() => this.list.value() ?? []);

  protected readonly durationHint = computed(() => {
    const months = Number(this.duration());
    return Number.isInteger(months) && months > 0 ? formatDuration(months) : '';
  });

  /** Requête du simulateur, ou le message qui dit ce qui manque. */
  private readonly request = computed<ComputeInput | string>(() => {
    const goal = this.goal();
    const rate = parseRatePpm(this.rate());
    const insurance = insuranceOf(
      this.insuranceMode(),
      this.insuranceAmount(),
      this.insuranceRate(),
    );
    const principal = parseMoney(this.principal());
    const target = parseMoney(this.target());
    const duration = /^\d{1,3}$/.test(this.duration().trim()) ? Number(this.duration()) : null;
    const fees = parseMoney(this.fees() || '0');
    const guarantee = parseMoney(this.guarantee() || '0');
    if (rate === null) return 'Saisissez le taux.';
    if (!insurance) return "Saisissez l'assurance.";
    if (fees === null || guarantee === null) return 'Frais invalides.';
    if (goal !== 'PRINCIPAL' && (!principal || principal <= 0)) return 'Saisissez le capital.';
    if (goal !== 'DURATION' && !duration) return 'Saisissez la durée.';
    if (goal !== 'PAYMENT' && (!target || target <= 0)) return 'Saisissez la mensualité visée.';
    return {
      goal,
      kind: this.kind(),
      principal_minor: goal === 'PRINCIPAL' ? null : principal,
      duration_months: goal === 'DURATION' ? null : duration,
      target_payment_minor: goal === 'PAYMENT' ? null : target,
      annual_rate_ppm: rate,
      insurance,
      fees_minor: fees,
      guarantee_fees_minor: guarantee,
    };
  });

  /** Résultat recalculé à la saisie (E36). */
  protected readonly outcome = toSignal(
    toObservable(this.request).pipe(
      debounceTime(200),
      switchMap((request) =>
        typeof request === 'string'
          ? of({ error: request, result: null })
          : this.api.computeLoanSimulation({ body: request }).pipe(
              map((result) => ({ error: null, result })),
              catchError((e) => of({ error: errorMessage(e), result: null })),
            ),
      ),
    ),
  );

  protected reset(): void {
    this.editing.set(null);
  }

  /** Charge une simulation enregistrée dans le simulateur. */
  protected load(s: SimulationView): void {
    this.editing.set(s);
    this.goal.set('PAYMENT');
    this.kind.set(s.kind);
    this.principal.set(toEditable(s.principal_minor));
    this.duration.set(String(s.duration_months));
    this.rate.set(rateEditable(s.annual_rate_ppm));
    this.insuranceMode.set(s.insurance.mode);
    if (s.insurance.mode === 'FIXED')
      this.insuranceAmount.set(toEditable(s.insurance.amount_minor));
    if (s.insurance.mode === 'RATE_ON_PRINCIPAL')
      this.insuranceRate.set(rateEditable(s.insurance.rate_ppm));
    this.fees.set(toEditable(s.fees_minor));
    this.guarantee.set(toEditable(s.guarantee_fees_minor));
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  /** Les valeurs du simulateur, une fois le capital et la durée trouvés. */
  private current() {
    const o = this.outcome();
    const request = this.request();
    if (!o?.result || typeof request === 'string') return null;
    return {
      kind: request.kind,
      principal_minor: o.result.principal_minor,
      duration_months: o.result.duration_months,
      annual_rate_ppm: request.annual_rate_ppm,
      insurance: request.insurance,
      fees_minor: request.fees_minor ?? 0,
      guarantee_fees_minor: request.guarantee_fees_minor ?? 0,
    };
  }

  protected async saveAsNew(): Promise<void> {
    const values = this.current();
    if (!values) return;
    const name = `${formatDuration(values.duration_months)} à ${formatRate(values.annual_rate_ppm)}`;
    if (await editSimulation(this.dialog, { simulation: null, initial: { ...values, name } })) {
      this.version.bump();
    }
  }

  protected async update(s: SimulationView): Promise<void> {
    const values = this.current();
    if (!values) return;
    try {
      const saved = await firstValueFrom(
        this.api.updateLoanSimulation({
          id: s.id,
          body: {
            ...values,
            name: s.name,
            institution_id: s.institution_id ?? null,
            application_id: s.application_id ?? null,
            note: s.note ?? null,
          },
        }),
      );
      this.editing.set(saved);
      this.version.bump();
      this.snack.open('Simulation enregistrée', undefined, { duration: 3000 });
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  protected async edit(s: SimulationView): Promise<void> {
    if (await editSimulation(this.dialog, { simulation: s })) this.version.bump();
  }

  protected async duplicate(s: SimulationView): Promise<void> {
    const initial = {
      name: `${s.name} (copie)`,
      kind: s.kind,
      principal_minor: s.principal_minor,
      duration_months: s.duration_months,
      annual_rate_ppm: s.annual_rate_ppm,
      insurance: s.insurance,
      fees_minor: s.fees_minor,
      guarantee_fees_minor: s.guarantee_fees_minor,
      institution_id: s.institution_id ?? null,
      application_id: s.application_id ?? null,
    };
    if (await editSimulation(this.dialog, { simulation: null, initial })) this.version.bump();
  }

  protected schedule(s: SimulationView): void {
    showSchedule(this.dialog, s);
  }

  protected toggle(id: string): void {
    this.checked.update((set) => {
      const next = new Set(set);
      if (next.has(id)) next.delete(id);
      else if (next.size < 4) next.add(id);
      return next;
    });
  }

  protected compare(): void {
    compareSimulations(
      this.dialog,
      this.simulations().filter((s) => this.checked().has(s.id)),
    );
  }

  /** Transforme une simulation en prêt (R-CRE-7). */
  protected async convert(s: SimulationView): Promise<void> {
    const ref = this.dialog.open(LoanDialog, { data: { simulation: s }, maxWidth: '100vw' });
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.version.bump();
      await this.router.navigate(['/prets', id]);
    }
  }

  protected async remove(s: SimulationView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${s.name} » ?`,
      message: s.loan_id ? 'Le prêt créé à partir de cette simulation reste.' : '',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    await firstValueFrom(this.api.deleteLoanSimulation({ id: s.id }));
    if (this.editing()?.id === s.id) this.editing.set(null);
    this.version.bump();
  }
}
