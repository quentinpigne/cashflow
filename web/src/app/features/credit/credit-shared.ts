import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTabsModule } from '@angular/material/tabs';
import { RouterLink, RouterLinkActive } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ApplicationStatus } from '../../api/models/application-status';
import { Insurance } from '../../api/models/insurance';
import { LenderRequestStatus } from '../../api/models/lender-request-status';
import { LoanKind } from '../../api/models/loan-kind';
import { PieceStatus } from '../../api/models/piece-status';
import { ProjectKind } from '../../api/models/project-kind';
import { SimulationInput } from '../../api/models/simulation-input';
import { SimulationView } from '../../api/models/simulation-view';
import { CreditService } from '../../api/services/credit.service';
import { errorMessage } from '../../core/http-errors';
import { LOAN_KIND_LABELS } from '../../core/labels';
import { formatRate, parseMoney, parseRatePpm, toEditable } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { InstitutionsStore } from '../../shared/stores';

export const PROJECT_KIND_LABELS: Record<ProjectKind, string> = {
  EXISTING_HOME: "Achat dans l'ancien",
  NEW_HOME: 'Achat dans le neuf',
  WORKS: 'Travaux',
  VEHICLE: 'Véhicule',
  OTHER: 'Autre projet',
};
export const PROJECT_KINDS = Object.keys(PROJECT_KIND_LABELS) as ProjectKind[];

export const APPLICATION_STATUS_LABELS: Record<ApplicationStatus, string> = {
  PREPARING: 'En préparation',
  SUBMITTED: 'Envoyée',
  ACCEPTED: 'Acceptée',
  ABANDONED: 'Abandonnée',
};
export const APPLICATION_STATUSES = Object.keys(APPLICATION_STATUS_LABELS) as ApplicationStatus[];

export const LENDER_STATUS_LABELS: Record<LenderRequestStatus, string> = {
  SENT: 'Envoyée',
  OFFER: 'Offre reçue',
  REFUSED: 'Refusée',
  ACCEPTED: 'Acceptée',
  WITHDRAWN: 'Retirée',
};
export const LENDER_STATUSES = Object.keys(LENDER_STATUS_LABELS) as LenderRequestStatus[];

export const PIECE_STATUS_LABELS: Record<PieceStatus, string> = {
  PROVIDED: 'Fournie',
  INCOMPLETE: 'Incomplète',
  MISSING: 'Manquante',
  NOT_REQUIRED: 'Non requise',
};

export const INSURANCE_LABELS: Record<Insurance['mode'], string> = {
  NONE: 'Aucune',
  FIXED: 'Montant mensuel',
  RATE_ON_PRINCIPAL: 'Taux annuel sur le capital',
};

/** Taux en millionièmes → saisie en pour cent (« 3,25 »). */
export function rateEditable(ppm: number): string {
  return formatRate(ppm).replace(/\s*%$/, '');
}

/** « 25 ans », « 20 ans et 6 mois », « 18 mois ». */
export function formatDuration(months: number): string {
  const years = Math.floor(months / 12);
  const rest = months % 12;
  if (years === 0) return `${rest} mois`;
  const y = `${years} an${years > 1 ? 's' : ''}`;
  return rest ? `${y} et ${rest} mois` : y;
}

/** Saisie de l'assurance emprunteur, partagée par le simulateur et le formulaire de simulation. */
export function insuranceOf(
  mode: Insurance['mode'],
  amount: string,
  rate: string,
): Insurance | null {
  switch (mode) {
    case 'NONE':
      return { mode: 'NONE' };
    case 'FIXED': {
      const minor = parseMoney(amount);
      return minor === null || minor < 0 ? null : { mode: 'FIXED', amount_minor: minor };
    }
    case 'RATE_ON_PRINCIPAL': {
      const ppm = parseRatePpm(rate);
      return ppm === null ? null : { mode: 'RATE_ON_PRINCIPAL', rate_ppm: ppm };
    }
  }
}

/** E27 — onglets de la rubrique Prêts : prêts, simulations, demandes de crédit. */
@Component({
  selector: 'app-loans-tabs',
  imports: [MatTabsModule, RouterLink, RouterLinkActive],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <nav mat-tab-nav-bar [tabPanel]="panel" class="tabs" aria-label="Rubrique Prêts">
      @for (t of tabs; track t.path) {
        <a
          mat-tab-link
          [routerLink]="t.path"
          routerLinkActive
          #rla="routerLinkActive"
          [routerLinkActiveOptions]="{ exact: t.exact }"
          [active]="rla.isActive"
          >{{ t.label }}</a
        >
      }
    </nav>
    <mat-tab-nav-panel #panel />
  `,
  styles: `
    .tabs {
      margin-bottom: 1rem;
    }
  `,
})
export class LoansTabs {
  protected readonly tabs = [
    { path: '/prets', label: 'Prêts', exact: true },
    { path: '/prets/simulations', label: 'Simulations', exact: false },
    { path: '/prets/demandes', label: 'Demandes', exact: false },
  ];
}

export interface SimulationDialogData {
  /** Simulation modifiée, ou valeurs de départ d'une nouvelle simulation. */
  simulation: SimulationView | null;
  initial?: Partial<SimulationInput>;
  title?: string;
  /** Enregistrement à la place de la création ou de la modification : l'offre d'une démarche. */
  submit?: (body: SimulationInput) => Promise<unknown>;
}

/** Formulaire d'une simulation : nouvelle, modifiée, ou offre reçue (R-CRE-1, R-CRE-21). */
@Component({
  selector: 'app-simulation-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ title }}</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Nom</mat-label>
        <input matInput maxlength="80" [ngModel]="name()" (ngModelChange)="name.set($event)" />
      </mat-form-field>
      <div class="pair">
        <mat-form-field>
          <mat-label>Type de prêt</mat-label>
          <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
            @for (k of kinds; track k) {
              <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Établissement</mat-label>
          <mat-select [ngModel]="institution()" (ngModelChange)="institution.set($event)">
            <mat-option [value]="null">Aucun</mat-option>
            @for (i of institutions.active(); track i.id) {
              <mat-option [value]="i.id">{{ i.name }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
      </div>
      <div class="pair">
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
        <mat-form-field>
          <mat-label>Durée</mat-label>
          <input
            matInput
            inputmode="numeric"
            [ngModel]="duration()"
            (ngModelChange)="duration.set($event)"
          />
          <span matTextSuffix>mois</span>
        </mat-form-field>
      </div>
      <div class="pair">
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
      </div>
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
          <mat-label>Taux d'assurance annuel</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="insuranceRate()"
            (ngModelChange)="insuranceRate.set($event)"
          />
          <span matTextSuffix>%</span>
        </mat-form-field>
      }
      <div class="pair">
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
      <mat-form-field>
        <mat-label>Note</mat-label>
        <input matInput [ngModel]="note()" (ngModelChange)="note.set($event)" />
      </mat-form-field>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!body() || busy()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(30rem, 80vw);
    }
    .pair {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--field-gap);
    }
    .error {
      color: var(--mat-sys-error);
      margin: 0;
    }
  `,
})
export class SimulationDialog {
  private readonly data = inject<SimulationDialogData>(MAT_DIALOG_DATA);
  private readonly api = inject(CreditService);
  private readonly ref = inject(MatDialogRef<SimulationDialog, boolean>);
  protected readonly institutions = inject(InstitutionsStore);

  protected readonly kinds: LoanKind[] = ['MORTGAGE', 'CONSUMER', 'OTHER'];
  protected readonly kindLabels = LOAN_KIND_LABELS;
  protected readonly insuranceModes: Insurance['mode'][] = ['NONE', 'FIXED', 'RATE_ON_PRINCIPAL'];
  protected readonly insuranceLabels = INSURANCE_LABELS;

  private readonly s = this.data.simulation;
  private readonly i = this.data.initial ?? {};
  protected readonly title =
    this.data.title ?? (this.s ? 'Modifier la simulation' : 'Enregistrer la simulation');
  protected readonly name = signal(this.s?.name ?? this.i.name ?? '');
  protected readonly kind = signal<LoanKind>(this.s?.kind ?? this.i.kind ?? 'MORTGAGE');
  protected readonly institution = signal<string | null>(
    this.s?.institution_id ?? this.i.institution_id ?? null,
  );
  private readonly insurance = this.s?.insurance ?? this.i.insurance ?? { mode: 'NONE' };
  protected readonly principal = signal(
    toEditableOr(this.s?.principal_minor ?? this.i.principal_minor),
  );
  protected readonly duration = signal(
    String(this.s?.duration_months ?? this.i.duration_months ?? 240),
  );
  protected readonly rate = signal(
    rateEditable(this.s?.annual_rate_ppm ?? this.i.annual_rate_ppm ?? 0),
  );
  protected readonly insuranceMode = signal<Insurance['mode']>(this.insurance.mode);
  protected readonly insuranceAmount = signal(
    this.insurance.mode === 'FIXED' ? toEditable(this.insurance.amount_minor) : '',
  );
  protected readonly insuranceRate = signal(
    this.insurance.mode === 'RATE_ON_PRINCIPAL' ? rateEditable(this.insurance.rate_ppm) : '',
  );
  protected readonly fees = signal(toEditableOr(this.s?.fees_minor ?? this.i.fees_minor ?? 0));
  protected readonly guarantee = signal(
    toEditableOr(this.s?.guarantee_fees_minor ?? this.i.guarantee_fees_minor ?? 0),
  );
  protected readonly note = signal(this.s?.note ?? '');
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  protected readonly body = computed<SimulationInput | null>(() => {
    const name = this.name().trim();
    const principal = parseMoney(this.principal());
    const rate = parseRatePpm(this.rate());
    const duration = /^\d{1,3}$/.test(this.duration().trim()) ? Number(this.duration()) : null;
    const insurance = insuranceOf(
      this.insuranceMode(),
      this.insuranceAmount(),
      this.insuranceRate(),
    );
    const fees = parseMoney(this.fees() || '0');
    const guarantee = parseMoney(this.guarantee() || '0');
    if (!name || !principal || principal <= 0 || rate === null || !duration || !insurance)
      return null;
    if (fees === null || guarantee === null) return null;
    return {
      name,
      kind: this.kind(),
      principal_minor: principal,
      annual_rate_ppm: rate,
      duration_months: duration,
      insurance,
      fees_minor: fees,
      guarantee_fees_minor: guarantee,
      institution_id: this.institution(),
      application_id: this.s?.application_id ?? this.i.application_id ?? null,
      note: this.note().trim() || null,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    this.busy.set(true);
    try {
      if (this.data.submit) {
        await this.data.submit(body);
      } else if (this.s) {
        await firstValueFrom(this.api.updateLoanSimulation({ id: this.s.id, body }));
      } else {
        await firstValueFrom(this.api.createLoanSimulation({ body }));
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.busy.set(false);
    }
  }
}

function toEditableOr(minor: number | null | undefined): string {
  return minor == null ? '' : toEditable(minor);
}

/** Ouvre le formulaire d'une simulation ; résout à `true` si elle a été enregistrée. */
export async function editSimulation(
  dialog: MatDialog,
  data: SimulationDialogData,
): Promise<boolean> {
  const ref = dialog.open<SimulationDialog, SimulationDialogData, boolean>(SimulationDialog, {
    data,
    maxWidth: '100vw',
  });
  return (await firstValueFrom(ref.afterClosed())) === true;
}

/** E37 — comparaison de simulations (R-CRE-6). */
@Component({
  selector: 'app-compare-dialog',
  imports: [MatButtonModule, MatDialogModule, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Comparer</h2>
    <mat-dialog-content>
      <div class="table" [style.--columns]="data.length">
        <span></span>
        @for (s of data; track s.id) {
          <strong class="head">{{ s.name }}</strong>
        }
        @for (r of rows; track r.label) {
          <span class="label">{{ r.label }}</span>
          @for (s of data; track s.id) {
            <span class="value" [class.best]="isBest(r, s)">{{ r.show(s) }}</span>
          }
        }
        <span class="label">Écart à la moins chère</span>
        @for (s of data; track s.id) {
          <span class="value">{{ s.result.total_cost_minor - cheapest() | money }}</span>
        }
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-flat-button mat-dialog-close>Fermer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .table {
      display: grid;
      grid-template-columns: minmax(9rem, auto) repeat(var(--columns), minmax(7rem, 1fr));
      gap: 0.25rem 1rem;
      overflow-x: auto;
    }
    .head,
    .value {
      text-align: right;
    }
    .label {
      color: var(--mat-sys-on-surface-variant);
    }
    .best {
      font-weight: 600;
      color: var(--mat-sys-primary);
    }
  `,
})
export class CompareDialog {
  protected readonly data = inject<SimulationView[]>(MAT_DIALOG_DATA);
  private readonly money = new MoneyPipe();

  /** Lignes comparées ; `value` sert à trouver la meilleure (la plus petite), si la ligne en a une. */
  protected readonly rows: {
    label: string;
    show: (s: SimulationView) => string;
    value?: (s: SimulationView) => number;
  }[] = [
    { label: 'Capital', show: (s) => this.money.transform(s.principal_minor) },
    { label: 'Durée', show: (s) => formatDuration(s.duration_months) },
    {
      label: 'Taux nominal',
      show: (s) => formatRate(s.annual_rate_ppm),
      value: (s) => s.annual_rate_ppm,
    },
    {
      label: 'Échéance totale',
      show: (s) => this.money.transform(s.result.total_payment_minor),
      value: (s) => s.result.total_payment_minor,
    },
    {
      label: 'Intérêts',
      show: (s) => this.money.transform(s.result.total_interest_minor),
      value: (s) => s.result.total_interest_minor,
    },
    {
      label: 'Assurance totale',
      show: (s) => this.money.transform(s.result.total_insurance_minor),
      value: (s) => s.result.total_insurance_minor,
    },
    {
      label: 'Frais',
      show: (s) => this.money.transform(s.fees_minor + s.guarantee_fees_minor),
      value: (s) => s.fees_minor + s.guarantee_fees_minor,
    },
    {
      label: 'Coût total',
      show: (s) => this.money.transform(s.result.total_cost_minor),
      value: (s) => s.result.total_cost_minor,
    },
    { label: 'TAEG', show: (s) => formatRate(s.result.apr_ppm), value: (s) => s.result.apr_ppm },
  ];

  protected readonly cheapest = computed(() =>
    Math.min(...this.data.map((s) => s.result.total_cost_minor)),
  );

  protected isBest(row: (typeof this.rows)[number], s: SimulationView): boolean {
    if (!row.value || this.data.length < 2) return false;
    return row.value(s) === Math.min(...this.data.map((d) => row.value!(d)));
  }
}

export function compareSimulations(dialog: MatDialog, simulations: SimulationView[]): void {
  dialog.open(CompareDialog, { data: simulations, maxWidth: '100vw' });
}

/** Tableau d'amortissement d'une simulation, une année à la fois (R-CRE-2). */
@Component({
  selector: 'app-simulation-schedule-dialog',
  imports: [
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatSelectModule,
    FormsModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Tableau d'amortissement — {{ data.name }}</h2>
    <mat-dialog-content>
      <mat-form-field subscriptSizing="dynamic">
        <mat-label>Année</mat-label>
        <mat-select [ngModel]="year()" (ngModelChange)="year.set($event)">
          @for (y of years(); track y) {
            <mat-option [value]="y">{{ y }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <div class="grid">
        <span>N°</span><span>Date</span><span class="r">Échéance</span><span class="r">Capital</span
        ><span class="r">Intérêts</span><span class="r">Restant dû</span>
        @for (i of shown(); track i.number) {
          <span>{{ i.number }}</span>
          <span>{{ i.date | frDate: 'short' }}</span>
          <span class="r">{{ i.total_minor | money }}</span>
          <span class="r">{{ i.principal_minor | money }}</span>
          <span class="r">{{ i.interest_minor | money }}</span>
          <span class="r">{{ i.remaining_minor | money }}</span>
        }
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-flat-button mat-dialog-close>Fermer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .grid {
      display: grid;
      grid-template-columns: auto auto repeat(4, minmax(6rem, 1fr));
      gap: 0.25rem 1rem;
      margin-top: 1rem;
      overflow-x: auto;
    }
    .r {
      text-align: right;
    }
  `,
})
export class ScheduleDialog {
  protected readonly data = inject<SimulationView>(MAT_DIALOG_DATA);
  private readonly api = inject(CreditService);
  private readonly schedule = rxResource({
    params: () => this.data.id,
    stream: ({ params }) => this.api.loanSimulationSchedule({ id: params }),
  });
  protected readonly years = computed(() => [
    ...new Set((this.schedule.value() ?? []).map((i) => Number(i.date.slice(0, 4)))),
  ]);
  protected readonly year = signal<number | null>(null);
  protected readonly shown = computed(() => {
    const year = this.year() ?? this.years()[0];
    return (this.schedule.value() ?? []).filter((i) => Number(i.date.slice(0, 4)) === year);
  });
}

export function showSchedule(dialog: MatDialog, simulation: SimulationView): void {
  dialog.open(ScheduleDialog, { data: simulation, maxWidth: '100vw' });
}
