import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  linkedSignal,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Router, RouterLink } from '@angular/router';
import { catchError, firstValueFrom, of } from 'rxjs';

import { Insurance } from '../../api/models/insurance';
import { LoanKind } from '../../api/models/loan-kind';
import { LoanPaymentProposal } from '../../api/models/loan-payment-proposal';
import { LoanView } from '../../api/models/loan-view';
import { AssetsService } from '../../api/services/assets.service';
import { LoansService } from '../../api/services/loans.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, todayIso, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { LOAN_KIND_LABELS } from '../../core/labels';
import { formatRate, negate, parseMoney, parseRatePpm, sum, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import {
  AccountsStore,
  CategoriesStore,
  FeaturesStore,
  InstitutionsStore,
} from '../../shared/stores';
import { DocumentsService } from '../../api/services/documents.service';
import { confirmDeletion } from '../documents/delete-confirmation';
import { DocumentSection } from '../documents/document-section';

type InsuranceMode = Insurance['mode'];

const INSURANCE_LABELS: Record<InsuranceMode, string> = {
  NONE: 'Aucune',
  FIXED: 'Montant mensuel',
  RATE_ON_PRINCIPAL: 'Taux annuel sur le capital emprunté',
};

/** Taux en millionièmes → saisie en pour cent (« 3,25 »). */
function rateEditable(ppm: number): string {
  return formatRate(ppm).replace(/\s*%$/, '');
}

const SHARED_STYLES = `
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--field-gap);
    min-width: min(34rem, 80vw);
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--field-gap);
    @media (max-width: 599.98px) {
      grid-template-columns: 1fr;
    }
  }
  .preview {
    margin: 0;
    padding: 0.5rem 0.75rem;
    border-radius: var(--mat-sys-corner-medium);
    background: var(--mat-sys-surface-container);
  }
  .error {
    color: var(--mat-sys-error);
    margin: 0;
  }
`;

/** Création ou modification d'un prêt, en suivi simple ou avec tableau (E28, R-PRE-1, R-PRE-2). */
@Component({
  selector: 'app-loan-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    CategoryPicker,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ loan ? 'Modifier le prêt' : 'Nouveau prêt' }}</h2>
    <mat-dialog-content class="form">
      <div class="pair">
        <mat-form-field>
          <mat-label>Nom</mat-label>
          <input matInput maxlength="80" [ngModel]="name()" (ngModelChange)="name.set($event)" />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Type</mat-label>
          <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
            @for (k of kinds; track k) {
              <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
      </div>
      <div class="pair">
        <mat-form-field>
          <mat-label>Organisme prêteur</mat-label>
          <mat-select [ngModel]="institution()" (ngModelChange)="institution.set($event)">
            <mat-option [value]="null">Aucun</mat-option>
            @for (i of institutions.active(); track i.id) {
              <mat-option [value]="i.id">{{ i.name }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        @if (features.assets()) {
          <mat-form-field>
            <mat-label>Bien financé</mat-label>
            <mat-select [ngModel]="asset()" (ngModelChange)="asset.set($event)">
              <mat-option [value]="null">Aucun</mat-option>
              @for (a of assets.value() ?? []; track a.id) {
                <mat-option [value]="a.id">{{ a.name }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        }
      </div>
      <div class="pair">
        <mat-form-field>
          <mat-label>Capital emprunté</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="principal()"
            (ngModelChange)="principal.set($event)"
          />
          <span matTextSuffix>€</span>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Date de déblocage</mat-label>
          <input
            matInput
            [matDatepicker]="startPicker"
            [ngModel]="start()"
            (ngModelChange)="start.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="startPicker" />
          <mat-datepicker #startPicker />
        </mat-form-field>
      </div>

      <mat-button-toggle-group
        [value]="scheduled() ? 'schedule' : 'manual'"
        (change)="scheduled.set($event.value === 'schedule')"
        aria-label="Mode de suivi"
      >
        <mat-button-toggle value="schedule">Avec tableau d'amortissement</mat-button-toggle>
        <mat-button-toggle value="manual">Suivi simple</mat-button-toggle>
      </mat-button-toggle-group>

      @if (scheduled()) {
        <div class="pair">
          <mat-form-field>
            <mat-label>Taux nominal annuel</mat-label>
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
          </mat-form-field>
        </div>
        <div class="pair">
          <mat-form-field>
            <mat-label>Première échéance</mat-label>
            <input
              matInput
              [matDatepicker]="firstPicker"
              [ngModel]="first()"
              (ngModelChange)="first.set($event)"
            />
            <mat-datepicker-toggle matIconSuffix [for]="firstPicker" />
            <mat-datepicker #firstPicker />
          </mat-form-field>
          <mat-form-field>
            <mat-label>Compte qui paie les mensualités</mat-label>
            <mat-select [ngModel]="payer()" (ngModelChange)="payer.set($event)">
              @for (a of payers(); track a.id) {
                <mat-option [value]="a.id">{{ a.name }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        </div>
        <div class="pair">
          <mat-form-field>
            <mat-label>Assurance emprunteur</mat-label>
            <mat-select [ngModel]="insuranceMode()" (ngModelChange)="insuranceMode.set($event)">
              @for (m of insuranceModes; track m) {
                <mat-option [value]="m">{{ insuranceLabels[m] }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          @if (insuranceMode() === 'FIXED') {
            <mat-form-field>
              <mat-label>Assurance par mois</mat-label>
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
        </div>
        <app-category-picker
          label="Catégorie des intérêts"
          kind="EXPENSE"
          [value]="interestCategory()"
          (valueChange)="interestCategory.set($event)"
        />
        @if (insuranceMode() !== 'NONE') {
          <app-category-picker
            label="Catégorie de l'assurance"
            kind="EXPENSE"
            [value]="insuranceCategory()"
            (valueChange)="insuranceCategory.set($event)"
          />
        }
        @if (preview.value(); as p) {
          <p class="preview">
            Mensualité : <strong class="amount">{{ p.monthly_payment_minor | money }}</strong>
            @if (p.monthly_insurance_minor > 0) {
              + {{ p.monthly_insurance_minor | money }} d'assurance =
              <strong class="amount">{{ p.monthly_total_minor | money }}</strong>
            }
          </p>
        }
      } @else {
        <p class="muted">
          Le capital restant dû se saisit ensuite sur la fiche du prêt, à partir de l'espace client
          ou des relevés du prêteur.
        </p>
      }

      @if (loan) {
        <mat-form-field>
          <mat-label>Soldé le</mat-label>
          <input
            matInput
            [matDatepicker]="closedPicker"
            [ngModel]="closed()"
            (ngModelChange)="closed.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="closedPicker" />
          <mat-datepicker #closedPicker />
        </mat-form-field>
      }
      <mat-checkbox [ngModel]="netWorth()" (ngModelChange)="netWorth.set($event)">
        Compter dans le patrimoine
      </mat-checkbox>
      <mat-form-field>
        <mat-label>Note</mat-label>
        <textarea
          matInput
          rows="2"
          [ngModel]="notes()"
          (ngModelChange)="notes.set($event)"
        ></textarea>
      </mat-form-field>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!body()" (click)="save()">
        {{ loan ? 'Enregistrer' : 'Créer le prêt' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: SHARED_STYLES,
})
export class LoanDialog {
  protected readonly loan = inject<LoanView | null>(MAT_DIALOG_DATA);
  private readonly api = inject(LoansService);
  private readonly ref = inject(MatDialogRef<LoanDialog>);
  private readonly accounts = inject(AccountsStore);
  private readonly categories = inject(CategoriesStore);
  protected readonly institutions = inject(InstitutionsStore);
  protected readonly features = inject(FeaturesStore);
  private readonly assetsApi = inject(AssetsService);
  protected readonly assets = rxResource({
    params: () => (this.features.assets() ? true : undefined),
    stream: () => this.assetsApi.listAssets(),
  });

  protected readonly kinds: LoanKind[] = ['MORTGAGE', 'CONSUMER', 'OTHER'];
  protected readonly kindLabels = LOAN_KIND_LABELS;
  protected readonly insuranceModes: InsuranceMode[] = ['NONE', 'FIXED', 'RATE_ON_PRINCIPAL'];
  protected readonly insuranceLabels = INSURANCE_LABELS;
  protected readonly payers = computed(() =>
    this.accounts.open().filter((a) => a.family === 'CASH' && a.currency === 'EUR'),
  );

  private readonly s = this.loan?.schedule ?? null;
  protected readonly name = signal(this.loan?.summary.name ?? '');
  protected readonly kind = signal<LoanKind>(this.loan?.summary.kind ?? 'MORTGAGE');
  protected readonly institution = signal<string | null>(this.loan?.institution_id ?? null);
  protected readonly asset = signal<string | null>(this.loan?.summary.asset_id ?? null);
  protected readonly principal = signal(
    this.loan ? toEditable(this.loan.summary.principal_minor) : '',
  );
  protected readonly start = signal<Date | null>(
    this.loan ? fromIsoDate(this.loan.start_date) : fromIsoDate(todayIso()),
  );
  protected readonly scheduled = signal(this.loan ? !!this.s : true);
  protected readonly rate = signal(this.s ? rateEditable(this.s.annual_rate_ppm) : '');
  protected readonly duration = signal(this.s ? String(this.s.duration_months) : '240');
  protected readonly first = signal<Date | null>(
    this.s ? fromIsoDate(this.s.first_installment_date) : null,
  );
  protected readonly insuranceMode = signal<InsuranceMode>(this.s?.insurance.mode ?? 'NONE');
  protected readonly insuranceAmount = signal(
    this.s?.insurance.mode === 'FIXED' ? toEditable(this.s.insurance.amount_minor) : '',
  );
  protected readonly insuranceRate = signal(
    this.s?.insurance.mode === 'RATE_ON_PRINCIPAL' ? rateEditable(this.s.insurance.rate_ppm) : '',
  );
  protected readonly payer = linkedSignal(
    () => this.s?.payer_account_id ?? this.payers()[0]?.id ?? null,
  );
  /** Par défaut « Prêts › Intérêts » et « Prêts › Assurance emprunteur » (R-PRE-2). */
  protected readonly interestCategory = linkedSignal<string | null>(
    () => this.s?.interest_category_id ?? this.byLabel('Prêts › Intérêts'),
  );
  protected readonly insuranceCategory = linkedSignal<string | null>(
    () => this.s?.insurance_category_id ?? this.byLabel('Prêts › Assurance emprunteur'),
  );
  protected readonly closed = signal<Date | null>(
    this.loan?.summary.closed_on ? fromIsoDate(this.loan.summary.closed_on) : null,
  );
  protected readonly netWorth = signal(this.loan?.summary.include_in_net_worth ?? true);
  protected readonly notes = signal(this.loan?.notes ?? '');
  protected readonly error = signal<string | null>(null);

  private byLabel(label: string): string | null {
    return this.categories.flat().find((c) => c.label === label)?.id ?? null;
  }

  private readonly insurance = computed<Insurance | null>(() => {
    switch (this.insuranceMode()) {
      case 'NONE':
        return { mode: 'NONE' };
      case 'FIXED': {
        const amount = parseMoney(this.insuranceAmount());
        return amount === null || amount < 0 ? null : { mode: 'FIXED', amount_minor: amount };
      }
      case 'RATE_ON_PRINCIPAL': {
        const rate = parseRatePpm(this.insuranceRate());
        return rate === null ? null : { mode: 'RATE_ON_PRINCIPAL', rate_ppm: rate };
      }
    }
  });

  /** Ce qui suffit à calculer la mensualité. */
  private readonly figures = computed(() => {
    const principal = parseMoney(this.principal());
    const rate = parseRatePpm(this.rate());
    const duration = /^\d{1,3}$/.test(this.duration().trim()) ? Number(this.duration()) : null;
    const insurance = this.insurance();
    if (principal === null || principal <= 0 || rate === null || !duration || !insurance) {
      return null;
    }
    return {
      principal_minor: principal,
      annual_rate_ppm: rate,
      duration_months: duration,
      insurance,
    };
  });

  protected readonly preview = rxResource({
    params: () => (this.scheduled() ? (this.figures() ?? undefined) : undefined),
    stream: ({ params }) => this.api.previewLoan({ body: params }).pipe(catchError(() => of(null))),
  });

  protected readonly body = computed(() => {
    const name = this.name().trim();
    const principal = parseMoney(this.principal());
    const start = this.start();
    if (!name || principal === null || principal <= 0 || !start) return null;
    let schedule = null;
    if (this.scheduled()) {
      const figures = this.figures();
      const first = this.first();
      const payer = this.payer();
      const interest = this.interestCategory();
      const insurance = this.insuranceMode() === 'NONE' ? null : this.insuranceCategory();
      if (!figures || !first || !payer || !interest) return null;
      if (this.insuranceMode() !== 'NONE' && !insurance) return null;
      schedule = {
        annual_rate_ppm: figures.annual_rate_ppm,
        duration_months: figures.duration_months,
        insurance: figures.insurance,
        first_installment_date: toIsoDate(first),
        payer_account_id: payer,
        interest_category_id: interest,
        insurance_category_id: insurance,
      };
    }
    const closed = this.closed();
    return {
      name,
      kind: this.kind(),
      institution_id: this.institution(),
      asset_id: this.asset(),
      principal_minor: principal,
      start_date: toIsoDate(start),
      schedule,
      closed_on: closed ? toIsoDate(closed) : null,
      include_in_net_worth: this.netWorth(),
      notes: this.notes().trim() || null,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    try {
      const saved = this.loan
        ? await firstValueFrom(this.api.updateLoan({ id: this.loan.summary.id, body }))
        : await firstValueFrom(this.api.createLoan({ body }));
      this.ref.close(saved.summary.id);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Capital restant dû saisi, pour un prêt en suivi simple (R-PRE-5). */
@Component({
  selector: 'app-loan-balance-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Capital restant dû</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Date</mat-label>
        <input
          matInput
          [matDatepicker]="picker"
          [ngModel]="date()"
          (ngModelChange)="date.set($event)"
        />
        <mat-datepicker-toggle matIconSuffix [for]="picker" />
        <mat-datepicker #picker />
      </mat-form-field>
      <mat-form-field>
        <mat-label>Capital restant dû</mat-label>
        <input
          matInput
          inputmode="decimal"
          [ngModel]="value()"
          (ngModelChange)="value.set($event)"
        />
        <span matTextSuffix>€</span>
      </mat-form-field>
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
      <button mat-flat-button [disabled]="!valid()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: SHARED_STYLES,
})
export class LoanBalanceDialog {
  private readonly loanId = inject<string>(MAT_DIALOG_DATA);
  private readonly api = inject(LoansService);
  private readonly ref = inject(MatDialogRef<LoanBalanceDialog>);
  protected readonly date = signal<Date | null>(fromIsoDate(todayIso()));
  protected readonly value = signal('');
  protected readonly note = signal('');
  protected readonly error = signal<string | null>(null);
  protected readonly valid = computed(() => {
    const amount = parseMoney(this.value());
    return !!this.date() && amount !== null && amount >= 0;
  });

  protected async save(): Promise<void> {
    const date = this.date();
    const amount = parseMoney(this.value());
    if (!date || amount === null) return;
    try {
      await firstValueFrom(
        this.api.saveLoanBalance({
          id: this.loanId,
          body: {
            date: toIsoDate(date),
            outstanding_minor: amount,
            note: this.note().trim() || null,
          },
        }),
      );
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Mensualités déjà enregistrées à ventiler en une fois selon leurs échéances (E28, PRE-03). */
@Component({
  selector: 'app-loan-payments-dialog',
  imports: [MatDialogModule, MatButtonModule, MatCheckboxModule, MoneyPipe, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Ventiler les mensualités passées</h2>
    <mat-dialog-content>
      <p class="muted">
        Chaque mensualité cochée est ventilée selon son échéance : le capital rembourse le prêt, les
        intérêts et l'assurance deviennent des dépenses.
      </p>
      @for (p of data.proposals; track p.transaction_id) {
        <mat-checkbox
          [checked]="selected().has(p.transaction_id)"
          (change)="toggle(p, $event.checked)"
        >
          <span class="line">
            <span>{{ p.date | frDate: 'short' }} · {{ p.label }}</span>
            <span class="muted small"
              >échéance n° {{ p.installment_number }} : {{ p.principal_minor | money }} de
              capital</span
            >
          </span>
        </mat-checkbox>
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="selected().size === 0" (click)="save()">
        Ventiler {{ selected().size }} mensualité(s)
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    mat-checkbox {
      display: block;
    }
    .line {
      display: flex;
      flex-direction: column;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class LoanPaymentsDialog {
  protected readonly data = inject<{ loanId: string; proposals: LoanPaymentProposal[] }>(
    MAT_DIALOG_DATA,
  );
  private readonly api = inject(LoansService);
  private readonly ref = inject(MatDialogRef<LoanPaymentsDialog>);
  protected readonly selected = signal(new Set(this.data.proposals.map((p) => p.transaction_id)));
  protected readonly error = signal<string | null>(null);

  protected toggle(p: LoanPaymentProposal, checked: boolean): void {
    const next = new Set(this.selected());
    if (checked) next.add(p.transaction_id);
    else next.delete(p.transaction_id);
    this.selected.set(next);
  }

  protected async save(): Promise<void> {
    const body = this.data.proposals
      .filter((p) => this.selected().has(p.transaction_id))
      .map((p) => ({ transaction_id: p.transaction_id, installment_number: p.installment_number }));
    try {
      await firstValueFrom(this.api.payLoanInstallments({ id: this.data.loanId, body }));
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** E27 — Prêts (PRE-01). */
@Component({
  selector: 'app-loans-page',
  imports: [
    RouterLink,
    MatButtonModule,
    MatExpansionModule,
    MatIconModule,
    MatProgressBarModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Prêts</h1>
        <button mat-flat-button (click)="create()"><mat-icon>add</mat-icon>Nouveau prêt</button>
      </div>
      @if (loans.isLoading() && !loans.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      } @else if (current().length === 0 && closed().length === 0) {
        <div class="empty">
          <p>Aucun prêt.</p>
          <button mat-stroked-button (click)="create()">Créer un prêt</button>
        </div>
      } @else {
        <section class="card group">
          <header>
            <h2>En cours</h2>
            <span class="amount negative">{{ total() | money }}</span>
          </header>
          @for (l of current(); track l.id) {
            <a class="row" [routerLink]="['/prets', l.id]">
              <span class="name">
                <span>{{ l.name }}</span>
                <span class="muted small">
                  {{ kindLabels[l.kind] }}{{ l.institution_name ? ' · ' + l.institution_name : '' }}
                  @if (l.next_installment; as n) {
                    · prochaine échéance le {{ n.date | frDate: 'short' }}
                  }
                </span>
              </span>
              <span class="amount negative">{{ l.outstanding_minor ?? 0 | money }}</span>
            </a>
          }
        </section>
        @if (closed().length > 0) {
          <mat-accordion>
            <mat-expansion-panel>
              <mat-expansion-panel-header>
                <mat-panel-title>Prêts soldés ({{ closed().length }})</mat-panel-title>
              </mat-expansion-panel-header>
              @for (l of closed(); track l.id) {
                <a class="row" [routerLink]="['/prets', l.id]">
                  <span class="name">{{ l.name }}</span>
                  <span class="muted small">Soldé le {{ l.closed_on! | frDate: 'short' }}</span>
                </a>
              }
            </mat-expansion-panel>
          </mat-accordion>
        }
      }
    </div>
  `,
  styleUrl: '../accounts/accounts.scss',
})
export class LoansPage {
  private readonly api = inject(LoansService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);
  protected readonly kindLabels = LOAN_KIND_LABELS;

  protected readonly loans = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listLoans(),
  });
  protected readonly current = computed(() =>
    (this.loans.value() ?? []).filter((l) => !l.closed_on),
  );
  protected readonly closed = computed(() => (this.loans.value() ?? []).filter((l) => l.closed_on));
  /** Capital restant dû total, en négatif (dette). */
  protected readonly total = computed(() =>
    negate(sum(this.current().map((l) => l.outstanding_minor ?? 0))),
  );

  protected async create(): Promise<void> {
    const ref = this.dialog.open(LoanDialog, { data: null, maxWidth: '100vw' });
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.version.bump();
      await this.router.navigate(['/prets', id]);
    }
  }
}

/** E28 — Fiche d'un prêt (PRE-01 à PRE-03). */
@Component({
  selector: 'app-loan-page',
  imports: [
    DocumentSection,
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatMenuModule,
    MatProgressBarModule,
    MatTooltipModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './loan-page.html',
  styleUrl: './loan-page.scss',
})
export class LoanPage {
  private readonly api = inject(LoansService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly documents = inject(DocumentsService);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  protected readonly kindLabels = LOAN_KIND_LABELS;

  readonly id = input.required<string>();

  protected readonly loan = rxResource({
    params: () => ({ id: this.id(), version: this.version.value() }),
    stream: ({ params }) => this.api.getLoan({ id: params.id }),
  });
  protected readonly proposals = rxResource({
    params: () =>
      this.loan.value()?.schedule ? { id: this.id(), version: this.version.value() } : undefined,
    stream: ({ params }) => this.api.loanPaymentProposals({ id: params.id }),
  });

  private readonly years = computed(() =>
    (this.loan.value()?.schedule?.installments ?? []).map((i) => fromIsoDate(i.date).getFullYear()),
  );
  protected readonly firstYear = computed(() => this.years()[0] ?? new Date().getFullYear());
  protected readonly lastYear = computed(() => this.years().at(-1) ?? new Date().getFullYear());
  /** Année affichée : celle de la prochaine échéance, par défaut. */
  protected readonly year = linkedSignal(() => {
    const l = this.loan.value();
    const next = l?.schedule?.installments.find(
      (i) => i.number === l.summary.next_installment?.number,
    );
    return next ? fromIsoDate(next.date).getFullYear() : this.lastYear();
  });
  protected readonly rows = computed(() =>
    (this.loan.value()?.schedule?.installments ?? []).filter(
      (i) => fromIsoDate(i.date).getFullYear() === this.year(),
    ),
  );

  protected async edit(l: LoanView): Promise<void> {
    const ref = this.dialog.open(LoanDialog, { data: l, maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async remove(l: LoanView): Promise<void> {
    const deletion = await confirmDeletion(this.dialog, this.documents, l.summary.id, {
      title: 'Supprimer le prêt ?',
      message:
        'Ses mensualités ventilées et ses remboursements redeviennent des dépenses non catégorisées.',
      action: 'Supprimer',
      objectName: 'ce prêt',
    });
    if (!deletion) return;
    try {
      await firstValueFrom(
        this.api.deleteLoan({ id: l.summary.id, delete_documents: deletion.deleteDocuments }),
      );
      this.version.bump();
      await this.router.navigate(['/prets']);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  protected async addBalance(): Promise<void> {
    const ref = this.dialog.open(LoanBalanceDialog, { data: this.id() });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async removeBalance(date: string): Promise<void> {
    try {
      await firstValueFrom(this.api.deleteLoanBalance({ id: this.id(), date }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  protected async payPast(): Promise<void> {
    const ref = this.dialog.open(LoanPaymentsDialog, {
      data: { loanId: this.id(), proposals: this.proposals.value() ?? [] },
    });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async unpay(number: number): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Annuler la ventilation ?',
      message: 'La mensualité redevient une dépense non catégorisée.',
      action: 'Annuler la ventilation',
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.unpayLoanInstallment({ id: this.id(), number }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }
}
