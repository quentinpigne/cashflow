import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatIconModule } from '@angular/material/icon';
import { MatTooltipModule } from '@angular/material/tooltip';
import { RouterLink } from '@angular/router';

import { SavingsService } from '../../api/services/savings.service';
import { DataVersion } from '../../core/data-version';
import { formatRate, formatRatePpm } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';

/**
 * Épargne sur l'accueil (TDB-05) : épargne de précaution, intérêts de l'année des livrets, valeur et
 * performance de l'épargne variable.
 */
@Component({
  selector: 'app-home-savings',
  imports: [RouterLink, MatIconModule, MatTooltipModule, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (overview.value(); as o) {
      <section class="card">
        <h2>Épargne</h2>
        @if (o.emergency_fund_minor > 0) {
          <div class="row">
            <span>
              Épargne de précaution
              <mat-icon
                class="info"
                matTooltip="Comptes marqués « épargne de précaution », rapportés aux dépenses mensuelles moyennes des 12 derniers mois complets (R-TDB-9)."
                >info</mat-icon
              >
            </span>
            <strong>{{ months(o.emergency_months_tenths) }}</strong>
          </div>
          <p class="muted small">
            {{ o.emergency_fund_minor | money }} pour {{ o.average_expenses_minor | money }} de
            dépenses par mois
          </p>
        }
        @if (o.fixed.length > 0) {
          <h3>Intérêts {{ year }}</h3>
          <div class="row">
            <span>Courus depuis le 1er janvier</span>
            <span class="amount">{{ o.interest_accrued_minor | money }}</span>
          </div>
          <div class="row muted">
            <span>Attendus au 31 décembre</span>
            <span class="amount">{{ o.interest_expected_minor | money }}</span>
          </div>
          @for (f of o.fixed; track f.account_id) {
            <a class="row link" [routerLink]="['/comptes', f.account_id]">
              <span>
                {{ f.name }} <span class="muted small">{{ rate(f.rate_ppm) }}</span>
                @if (f.interest_to_record) {
                  <mat-icon class="flag" matTooltip="Intérêts de l'an dernier à inscrire"
                    >edit_note</mat-icon
                  >
                }
                @if (f.cap_exceeded) {
                  <mat-icon class="flag warn" matTooltip="Plafond dépassé">warning</mat-icon>
                }
              </span>
              <span class="amount">{{ f.accrued_minor | money }}</span>
            </a>
          }
        }
        @if (o.variable.length > 0) {
          <h3>Épargne variable</h3>
          <div class="row">
            <span>Valeur / capital investi</span>
            <span class="amount">
              {{ o.variable_value_minor | money }} / {{ o.variable_invested_minor | money }}
            </span>
          </div>
          <div class="row">
            <span>TRI global depuis l'ouverture</span>
            <span class="amount">{{ ratePpm(o.variable_irr_ppm) }}</span>
          </div>
          @for (v of o.variable; track v.account_id) {
            <a class="row link" [routerLink]="['/comptes', v.account_id]">
              <span>
                {{ v.name }}
                @if (v.estimated) {
                  <span class="muted small">(estimée)</span>
                }
              </span>
              <span class="amount" [class.negative]="v.gain_minor < 0">
                {{ v.gain_minor | money: 'EUR' : { signed: true } }}
                <span class="muted small">TRI {{ ratePpm(v.irr_since_opening_ppm) }}</span>
              </span>
            </a>
          }
        }
      </section>
    }
  `,
  styles: `
    :host {
      display: contents;
    }
    h2 {
      font: var(--mat-sys-title-medium);
      margin: 0 0 0.5rem;
    }
    h3 {
      font: var(--mat-sys-title-small);
      margin: 1rem 0 0.25rem;
    }
    .row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1rem;
      padding: 0.25rem 0;
    }
    .link {
      color: inherit;
      text-decoration: none;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .info,
    .flag {
      font-size: 1rem;
      width: 1rem;
      height: 1rem;
      vertical-align: middle;
      color: var(--mat-sys-on-surface-variant);
    }
    .warn {
      color: var(--mat-sys-error);
    }
  `,
})
export class HomeSavings {
  private readonly api = inject(SavingsService);
  private readonly version = inject(DataVersion);

  protected readonly year = new Date().getFullYear();
  protected readonly overview = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.savingsOverview(),
  });

  protected months(tenths: number | null | undefined): string {
    if (tenths === null || tenths === undefined) return '—';
    return `${(tenths / 10).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} mois`;
  }

  protected rate(ppm: number): string {
    return formatRate(ppm);
  }

  protected ratePpm(ppm: number | null | undefined): string {
    return ppm === null || ppm === undefined ? '—' : formatRatePpm(ppm);
  }
}
