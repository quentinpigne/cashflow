import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { firstValueFrom } from 'rxjs';

import { AccountKind } from '../../api/models/account-kind';
import { Assumptions } from '../../api/models/assumptions';
import { ScenarioView } from '../../api/models/scenario-view';
import { ProjectionsService } from '../../api/services/projections.service';
import { SavingsService } from '../../api/services/savings.service';
import { errorMessage } from '../../core/http-errors';
import { KIND_LABELS } from '../../core/labels';
import { parseMoney, parseRatePpm, toEditable } from '../../core/money';
import { AccountsStore } from '../../shared/stores';

const VARIABLE_KINDS: AccountKind[] = [
  'LIFE_INSURANCE',
  'PEA',
  'PEA_PME',
  'SECURITIES',
  'PER',
  'EMPLOYEE_SAVINGS',
  'CRYPTO',
  'VARIABLE_OTHER',
];

/** Un taux saisi en pour cent, vide si absent. */
function percent(ppm: number | null | undefined): string {
  if (ppm === null || ppm === undefined) return '';
  return (ppm / 10_000).toLocaleString('fr-FR', { maximumFractionDigits: 4 });
}

interface Row {
  key: string;
  label: string;
  rate: string;
  fees: string;
}

interface AccountRow extends Row {
  contribution: string;
}

interface PathRow {
  key: string;
  label: string;
  from: string;
  rate: string;
}

/**
 * Hypothèses d'un scénario (PRJ-03, R-PRJ-31) : rendement et frais par nature de compte, réglages propres
 * à un compte (y compris le versement mensuel), trajectoire de taux des barèmes (R-PRJ-21), capacité
 * d'épargne et hausse des salaires (R-PRJ-42 à R-PRJ-47, R-SAL-12).
 */
@Component({
  selector: 'app-scenario-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>
      {{ scenario ? 'Scénario « ' + scenario.name + ' »' : 'Nouveau scénario' }}
    </h2>
    <mat-dialog-content class="form">
      <p class="muted small">
        Ce sont des hypothèses, pas des prévisions : les rendements passés ne préjugent pas des
        rendements futurs.
      </p>
      <div class="line">
        <mat-form-field>
          <mat-label>Nom</mat-label>
          <input matInput [ngModel]="name()" (ngModelChange)="name.set($event)" maxlength="60" />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Horizon</mat-label>
          <input
            matInput
            type="number"
            min="1"
            max="50"
            [ngModel]="years()"
            (ngModelChange)="years.set($event)"
          />
          <span matTextSuffix>ans</span>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Inflation</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="inflation()"
            (ngModelChange)="inflation.set($event)"
          />
          <span matTextSuffix>%/an</span>
        </mat-form-field>
      </div>
      <mat-checkbox [ngModel]="isDefault()" (ngModelChange)="isDefault.set($event)">
        Scénario par défaut
      </mat-checkbox>

      <h3>Capacité d'épargne</h3>
      <mat-checkbox [ngModel]="capacity()" (ngModelChange)="capacity.set($event)">
        Ajouter chaque mois aux liquidités les revenus projetés moins les dépenses et les échéances
        de prêts
      </mat-checkbox>
      @if (capacity()) {
        <div class="line">
          <mat-form-field>
            <mat-label>Hausse des salaires</mat-label>
            <input
              matInput
              inputmode="decimal"
              [ngModel]="salaryGrowth()"
              (ngModelChange)="salaryGrowth.set($event)"
            />
            <span matTextSuffix>%/an</span>
          </mat-form-field>
          <mat-form-field>
            <mat-label>Hausse des dépenses</mat-label>
            <input
              matInput
              inputmode="decimal"
              [ngModel]="expenseGrowth()"
              (ngModelChange)="expenseGrowth.set($event)"
            />
            <span matTextSuffix>%/an</span>
          </mat-form-field>
        </div>
        <mat-form-field>
          <mat-label>Compte d'affectation du reste</mat-label>
          <mat-select [ngModel]="surplus()" (ngModelChange)="surplus.set($event)">
            <mat-option [value]="null">Aucun : le reste reste sur le compte courant</mat-option>
            @for (a of savingsAccounts(); track a.id) {
              <mat-option [value]="a.id">{{ a.name }}</mat-option>
            }
          </mat-select>
          <mat-hint>Après les versements programmés, dans la limite du plafond.</mat-hint>
        </mat-form-field>
      }

      <h3>Rendement et frais annuels par nature</h3>
      <table>
        <thead>
          <tr>
            <th>Nature</th>
            <th>Rendement</th>
            <th>Frais</th>
          </tr>
        </thead>
        <tbody>
          @for (row of kinds(); track row.key; let i = $index) {
            <tr>
              <td>{{ row.label }}</td>
              <td>
                <input
                  class="cell"
                  inputmode="decimal"
                  [attr.aria-label]="'Rendement ' + row.label"
                  [ngModel]="row.rate"
                  (ngModelChange)="setKind(i, { rate: $event })"
                />&nbsp;%
              </td>
              <td>
                <input
                  class="cell"
                  inputmode="decimal"
                  [attr.aria-label]="'Frais ' + row.label"
                  [ngModel]="row.fees"
                  (ngModelChange)="setKind(i, { fees: $event })"
                />&nbsp;%
              </td>
            </tr>
          }
        </tbody>
      </table>

      @if (accountRows().length > 0) {
        <h3>Par compte (facultatif)</h3>
        <p class="muted small">
          Laisser vide pour reprendre la nature. Le versement mensuel remplace les virements
          programmés.
        </p>
        <table>
          <thead>
            <tr>
              <th>Compte</th>
              <th>Rendement</th>
              <th>Frais</th>
              <th>Versement/mois</th>
            </tr>
          </thead>
          <tbody>
            @for (row of accountRows(); track row.key; let i = $index) {
              <tr>
                <td>{{ row.label }}</td>
                <td>
                  <input
                    class="cell"
                    inputmode="decimal"
                    [attr.aria-label]="'Rendement ' + row.label"
                    [ngModel]="row.rate"
                    (ngModelChange)="setAccount(i, { rate: $event })"
                  />&nbsp;%
                </td>
                <td>
                  <input
                    class="cell"
                    inputmode="decimal"
                    [attr.aria-label]="'Frais ' + row.label"
                    [ngModel]="row.fees"
                    (ngModelChange)="setAccount(i, { fees: $event })"
                  />&nbsp;%
                </td>
                <td>
                  <input
                    class="cell"
                    inputmode="decimal"
                    [attr.aria-label]="'Versement ' + row.label"
                    [ngModel]="row.contribution"
                    (ngModelChange)="setAccount(i, { contribution: $event })"
                  />&nbsp;€
                </td>
              </tr>
            }
          </tbody>
        </table>
      }

      @if (paths().length > 0) {
        <h3>Taux futurs des barèmes (facultatif)</h3>
        <p class="muted small">Sans saisie, le dernier taux connu reste constant.</p>
        <table>
          <tbody>
            @for (row of paths(); track row.key; let i = $index) {
              <tr>
                <td>{{ row.label }}</td>
                <td>
                  à partir du
                  <input
                    class="cell date"
                    type="date"
                    [attr.aria-label]="'Date ' + row.label"
                    [ngModel]="row.from"
                    (ngModelChange)="setPath(i, { from: $event })"
                  />
                </td>
                <td>
                  <input
                    class="cell"
                    inputmode="decimal"
                    [attr.aria-label]="'Taux ' + row.label"
                    [ngModel]="row.rate"
                    (ngModelChange)="setPath(i, { rate: $event })"
                  />&nbsp;%
                </td>
              </tr>
            }
          </tbody>
        </table>
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(36rem, 85vw);
    }
    .line {
      display: flex;
      flex-wrap: wrap;
      gap: var(--field-gap);
      > * {
        flex: 1;
        min-width: 8rem;
      }
    }
    h3 {
      font: var(--mat-sys-title-small);
      margin: 0.75rem 0 0;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      th,
      td {
        text-align: left;
        padding: 0.25rem;
        border-top: 1px solid var(--mat-sys-outline-variant);
      }
    }
    .cell {
      width: 4.5rem;
      text-align: right;
      border: 1px solid var(--mat-sys-outline-variant);
      border-radius: 0.25rem;
      padding: 0.25rem;
      background: transparent;
      color: inherit;
    }
    .date {
      width: 9rem;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class ScenarioDialog {
  protected readonly scenario = inject<ScenarioView | null>(MAT_DIALOG_DATA);
  private readonly api = inject(ProjectionsService);
  private readonly savings = inject(SavingsService);
  private readonly accounts = inject(AccountsStore);
  private readonly ref = inject(MatDialogRef<ScenarioDialog>);

  private readonly assumptions: Assumptions = this.scenario?.assumptions ?? {};
  protected readonly name = signal(this.scenario?.name ?? '');
  protected readonly years = signal(Math.round((this.scenario?.horizon_months ?? 240) / 12));
  protected readonly inflation = signal(percent(this.scenario?.inflation_rate_ppm));
  protected readonly isDefault = signal(this.scenario?.is_default ?? false);
  protected readonly error = signal<string | null>(null);

  // Capacité d'épargne : activée, 2 % par défaut (R-PRJ-44, R-PRJ-47, R-SAL-12).
  private readonly cap = this.assumptions.capacity;
  protected readonly capacity = signal(this.cap?.enabled ?? true);
  protected readonly salaryGrowth = signal(percent(this.cap?.salary_growth_ppm ?? 20_000));
  protected readonly expenseGrowth = signal(percent(this.cap?.expense_growth_ppm ?? 20_000));
  protected readonly surplus = signal<string | null>(this.cap?.surplus_account_id ?? null);
  protected readonly savingsAccounts = computed(() =>
    this.accounts.open().filter((a) => a.family !== 'CASH'),
  );

  protected readonly kinds = signal<Row[]>([
    ...VARIABLE_KINDS.map((kind) => ({
      key: kind,
      label: KIND_LABELS[kind],
      rate: percent(this.assumptions.by_kind?.[kind]?.annual_return_ppm),
      fees: percent(this.assumptions.by_kind?.[kind]?.annual_fees_ppm),
    })),
    {
      key: 'family',
      label: 'Autre épargne variable (défaut)',
      rate: percent(this.assumptions.by_family?.['VARIABLE_SAVINGS']?.annual_return_ppm),
      fees: percent(this.assumptions.by_family?.['VARIABLE_SAVINGS']?.annual_fees_ppm),
    },
  ]);

  private readonly accountEdits = signal<Record<string, Partial<AccountRow>>>({});
  protected readonly accountRows = computed<AccountRow[]>(() =>
    this.accounts
      .open()
      .filter((a) => a.family === 'VARIABLE_SAVINGS' || a.family === 'FIXED_SAVINGS')
      .map((a) => {
        const own = this.assumptions.by_account?.[a.id];
        const base: AccountRow = {
          key: a.id,
          label: a.name,
          rate: percent(own?.annual_return_ppm),
          fees: percent(own?.annual_fees_ppm),
          contribution: own?.monthly_contribution_minor
            ? toEditable(own.monthly_contribution_minor)
            : '',
        };
        return { ...base, ...this.accountEdits()[a.id] };
      }),
  );

  private readonly schedules = rxResource({ stream: () => this.savings.listRateSchedules() });
  private readonly pathEdits = signal<Record<string, Partial<PathRow>>>({});
  protected readonly paths = computed<PathRow[]>(() =>
    (this.schedules.value() ?? [])
      .filter((s) => s.accounts.length > 0)
      .map((s) => {
        const step = this.assumptions.rate_paths?.[s.id]?.[0];
        const base: PathRow = {
          key: s.id,
          label: s.name,
          from: step?.from ?? '',
          rate: percent(step?.rate_ppm),
        };
        return { ...base, ...this.pathEdits()[s.id] };
      }),
  );

  protected readonly valid = computed(() => {
    const years = Number(this.years());
    const numbers = [
      ...this.kinds().flatMap((r) => [r.rate, r.fees]),
      ...this.accountRows().flatMap((r) => [r.rate, r.fees]),
      ...this.paths().map((r) => r.rate),
      this.inflation(),
    ];
    const growth = [this.salaryGrowth(), this.expenseGrowth()];
    return (
      !!this.name().trim() &&
      Number.isInteger(years) &&
      years >= 1 &&
      years <= 50 &&
      numbers.every((n) => n.trim() === '' || parseRatePpm(n) !== null) &&
      (!this.capacity() || growth.every((n) => parseRatePpm(n) !== null)) &&
      this.accountRows().every(
        (r) => r.contribution.trim() === '' || parseMoney(r.contribution) !== null,
      )
    );
  });

  protected setKind(index: number, change: Partial<Row>): void {
    this.kinds.update((rows) => rows.map((r, i) => (i === index ? { ...r, ...change } : r)));
  }

  protected setAccount(index: number, change: Partial<AccountRow>): void {
    const key = this.accountRows()[index].key;
    this.accountEdits.update((e) => ({ ...e, [key]: { ...e[key], ...change } }));
  }

  protected setPath(index: number, change: Partial<PathRow>): void {
    const key = this.paths()[index].key;
    this.pathEdits.update((e) => ({ ...e, [key]: { ...e[key], ...change } }));
  }

  private build(): Assumptions {
    const rate = (text: string) => (text.trim() === '' ? null : parseRatePpm(text));
    const result: Assumptions = {
      by_kind: {},
      by_family: {},
      by_account: {},
      rate_paths: {},
      capacity: {
        enabled: this.capacity(),
        salary_growth_ppm: rate(this.salaryGrowth()) ?? this.cap?.salary_growth_ppm ?? 20_000,
        expense_growth_ppm: rate(this.expenseGrowth()) ?? this.cap?.expense_growth_ppm ?? 20_000,
        surplus_account_id: this.surplus(),
      },
    };
    for (const row of this.kinds()) {
      const value = rate(row.rate);
      if (value === null) continue;
      const entry = { annual_return_ppm: value, annual_fees_ppm: rate(row.fees) ?? 0 };
      if (row.key === 'family') result.by_family!['VARIABLE_SAVINGS'] = entry;
      else result.by_kind![row.key] = entry;
    }
    for (const row of this.accountRows()) {
      const entry = {
        annual_return_ppm: rate(row.rate),
        annual_fees_ppm: rate(row.fees),
        monthly_contribution_minor: row.contribution.trim() ? parseMoney(row.contribution) : null,
      };
      if (Object.values(entry).some((v) => v !== null)) result.by_account![row.key] = entry;
    }
    for (const row of this.paths()) {
      const value = rate(row.rate);
      if (row.from && value !== null)
        result.rate_paths![row.key] = [{ from: row.from, rate_ppm: value }];
    }
    return result;
  }

  protected async save(): Promise<void> {
    const body = {
      name: this.name(),
      horizon_months: Number(this.years()) * 12,
      inflation_rate_ppm: this.inflation().trim() ? parseRatePpm(this.inflation()) : null,
      assumptions: this.build(),
      is_default: this.isDefault(),
    };
    try {
      if (this.scenario) {
        await firstValueFrom(this.api.updateScenario({ id: this.scenario.id, body }));
      } else {
        await firstValueFrom(this.api.createScenario({ body }));
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}
