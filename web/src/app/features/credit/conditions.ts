import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  input,
  output,
  signal,
  untracked,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';

import { CreditConditions } from '../../api/models/credit-conditions';
import { Insurance } from '../../api/models/insurance';
import { LoanKind } from '../../api/models/loan-kind';
import { ProjectKind } from '../../api/models/project-kind';
import { LOAN_KIND_LABELS } from '../../core/labels';
import { formatMoney, formatRate, parseRatePpm } from '../../core/money';
import { editable, moneyOrZero } from './applications';
import { INSURANCE_LABELS, formatDuration, insuranceOf, rateEditable } from './credit-shared';

/** Libellés courts des types de projet, pour les puces de l'onglet Simuler (E36). */
export const PROJECT_SHORT_LABELS: Record<ProjectKind, string> = {
  EXISTING_HOME: 'Ancien',
  NEW_HOME: 'Neuf',
  WORKS: 'Travaux',
  VEHICLE: 'Véhicule',
  OTHER: 'Autre',
};

/** « 5,00 % sur 5 ans, sans assurance, sans frais » : les conditions en une ligne. */
export function conditionsSummary(c: CreditConditions, withDuration = true): string {
  const insurance =
    c.insurance.mode === 'NONE'
      ? 'sans assurance'
      : c.insurance.mode === 'FIXED'
        ? `assurance ${formatMoney(c.insurance.amount_minor)}/mois`
        : `assurance ${formatRate(c.insurance.rate_ppm)}`;
  const fees = c.fees_minor + c.guarantee_fees_minor;
  const duration = withDuration ? ` sur ${formatDuration(c.duration_months)}` : '';
  return `${formatRate(c.annual_rate_ppm)}${duration}, ${insurance}, ${fees ? `${formatMoney(fees)} de frais` : 'sans frais'}`;
}

function same(a: CreditConditions, b: CreditConditions): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Saisie des conditions d'un crédit (R-CRE-38, R-CRE-39) : type de prêt, taux, durée, assurance, frais. */
@Component({
  selector: 'app-conditions-fields',
  imports: [FormsModule, MatFormFieldModule, MatInputModule, MatSelectModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="fields">
      <mat-form-field>
        <mat-label>Type de prêt</mat-label>
        <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
          @for (k of kinds; track k) {
            <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <mat-form-field>
        <mat-label>Taux nominal</mat-label>
        <input matInput inputmode="decimal" [ngModel]="rate()" (ngModelChange)="rate.set($event)" />
        <span matTextSuffix>%</span>
      </mat-form-field>
      @if (!durationComputed()) {
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
        <input matInput inputmode="decimal" [ngModel]="fees()" (ngModelChange)="fees.set($event)" />
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
    @if (invalid()) {
      <p class="error" role="alert">Conditions incomplètes ou invalides.</p>
    }
  `,
  styles: `
    .fields {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
      gap: calc(var(--field-gap) * 1.5) var(--field-gap);
      align-items: start;
    }
    .error {
      margin: 0;
      color: var(--mat-sys-error);
    }
  `,
})
export class ConditionsFields {
  /** Les conditions affichées ; reprises dans la saisie quand elles changent d'ailleurs (projet, simulation). */
  readonly value = input.required<CreditConditions>();
  /** La durée est cherchée (mensualité fixée) : son champ disparaît. */
  readonly durationComputed = input(false);
  readonly changed = output<CreditConditions>();

  protected readonly kinds: LoanKind[] = ['MORTGAGE', 'CONSUMER', 'OTHER'];
  protected readonly kindLabels = LOAN_KIND_LABELS;
  protected readonly insuranceModes: Insurance['mode'][] = ['NONE', 'FIXED', 'RATE_ON_PRINCIPAL'];
  protected readonly insuranceLabels = INSURANCE_LABELS;

  protected readonly kind = signal<LoanKind>('MORTGAGE');
  protected readonly rate = signal('');
  protected readonly duration = signal('');
  protected readonly insuranceMode = signal<Insurance['mode']>('NONE');
  protected readonly insuranceAmount = signal('');
  protected readonly insuranceRate = signal('');
  protected readonly fees = signal('');
  protected readonly guarantee = signal('');

  protected readonly durationHint = computed(() => {
    const months = Number(this.duration());
    return Number.isInteger(months) && months > 0 ? formatDuration(months) : '';
  });

  /** Les conditions saisies, ou `null` tant qu'une saisie est invalide. */
  private readonly parsed = computed<CreditConditions | null>(() => {
    const rate = parseRatePpm(this.rate());
    const duration = /^\d{1,3}$/.test(this.duration().trim()) ? Number(this.duration()) : null;
    const insurance = insuranceOf(
      this.insuranceMode(),
      this.insuranceAmount(),
      this.insuranceRate(),
    );
    const fees = moneyOrZero(this.fees());
    const guarantee = moneyOrZero(this.guarantee());
    if (rate === null || rate < 0 || !duration || !insurance) return null;
    if (fees === null || fees < 0 || guarantee === null || guarantee < 0) return null;
    return {
      kind: this.kind(),
      annual_rate_ppm: rate,
      duration_months: duration,
      insurance,
      fees_minor: fees,
      guarantee_fees_minor: guarantee,
    };
  });
  protected readonly invalid = computed(() => this.parsed() === null);

  constructor() {
    effect(() => {
      const value = this.value();
      untracked(() => {
        const current = this.parsed();
        if (!current || !same(current, value)) this.fill(value);
      });
    });
    effect(() => {
      const current = this.parsed();
      if (current && !same(current, untracked(this.value))) this.changed.emit(current);
    });
  }

  private fill(c: CreditConditions): void {
    this.kind.set(c.kind);
    this.rate.set(rateEditable(c.annual_rate_ppm));
    this.duration.set(String(c.duration_months));
    this.insuranceMode.set(c.insurance.mode);
    this.insuranceAmount.set(
      c.insurance.mode === 'FIXED' ? editable(c.insurance.amount_minor) : '',
    );
    this.insuranceRate.set(
      c.insurance.mode === 'RATE_ON_PRINCIPAL' ? rateEditable(c.insurance.rate_ppm) : '',
    );
    this.fees.set(editable(c.fees_minor));
    this.guarantee.set(editable(c.guarantee_fees_minor));
  }
}
