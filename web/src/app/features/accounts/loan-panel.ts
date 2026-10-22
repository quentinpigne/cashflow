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
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { RouterLink } from '@angular/router';
import { catchError, firstValueFrom, of } from 'rxjs';

import { Insurance } from '../../api/models/insurance';
import { LoanPaymentProposal } from '../../api/models/loan-payment-proposal';
import { LoanTermsView } from '../../api/models/loan-terms-view';
import { LoansService } from '../../api/services/loans.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, toIsoDate } from '../../core/dates';
import { errorMessage, problemOf } from '../../core/http-errors';
import { formatRate, parseMoney, parseRatePpm, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore, CategoriesStore } from '../../shared/stores';

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

/** Conditions d'un prêt (E3, R-CPT-15) : la mensualité calculée s'affiche pendant la saisie. */
@Component({
  selector: 'app-loan-terms-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    CategoryPicker,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Conditions du prêt</h2>
    <mat-dialog-content class="form">
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
          <mat-label>Taux nominal annuel</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="rate()"
            (ngModelChange)="rate.set($event)"
          />
          <span matTextSuffix>%</span>
        </mat-form-field>
      </div>
      <div class="pair">
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
        <mat-form-field>
          <mat-label>Première échéance</mat-label>
          <input
            matInput
            [matDatepicker]="picker"
            [ngModel]="first()"
            (ngModelChange)="first.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="picker" />
          <mat-datepicker #picker />
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
      <mat-form-field>
        <mat-label>Compte qui paie les mensualités</mat-label>
        <mat-select [ngModel]="payer()" (ngModelChange)="payer.set($event)">
          @for (a of payers(); track a.id) {
            <mat-option [value]="a.id">{{ a.name }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
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
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!body()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(32rem, 80vw);
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
  `,
})
export class LoanTermsDialog {
  private readonly data = inject<{ accountId: string; terms: LoanTermsView | null }>(
    MAT_DIALOG_DATA,
  );
  private readonly api = inject(LoansService);
  private readonly ref = inject(MatDialogRef<LoanTermsDialog>);
  private readonly accounts = inject(AccountsStore);
  private readonly categories = inject(CategoriesStore);

  protected readonly insuranceModes: InsuranceMode[] = ['NONE', 'FIXED', 'RATE_ON_PRINCIPAL'];
  protected readonly insuranceLabels = INSURANCE_LABELS;
  protected readonly payers = computed(() =>
    this.accounts.open().filter((a) => a.family === 'CASH'),
  );

  private readonly terms = this.data.terms;
  protected readonly principal = signal(this.terms ? toEditable(this.terms.principal_minor) : '');
  protected readonly rate = signal(this.terms ? rateEditable(this.terms.annual_rate_ppm) : '');
  protected readonly duration = signal(this.terms ? String(this.terms.duration_months) : '240');
  protected readonly first = signal<Date | null>(
    this.terms ? fromIsoDate(this.terms.first_installment_date) : null,
  );
  protected readonly insuranceMode = signal<InsuranceMode>(this.terms?.insurance.mode ?? 'NONE');
  protected readonly insuranceAmount = signal(
    this.terms?.insurance.mode === 'FIXED' ? toEditable(this.terms.insurance.amount_minor) : '',
  );
  protected readonly insuranceRate = signal(
    this.terms?.insurance.mode === 'RATE_ON_PRINCIPAL'
      ? rateEditable(this.terms.insurance.rate_ppm)
      : '',
  );
  protected readonly payer = linkedSignal(
    () => this.terms?.payer_account_id ?? this.payers()[0]?.id ?? null,
  );
  /** Par défaut « Prêts › Intérêts » et « Prêts › Assurance emprunteur » (ecrans.md E4). */
  protected readonly interestCategory = linkedSignal<string | null>(
    () => this.terms?.interest_category_id ?? this.byLabel('Prêts › Intérêts'),
  );
  protected readonly insuranceCategory = linkedSignal<string | null>(
    () => this.terms?.insurance_category_id ?? this.byLabel('Prêts › Assurance emprunteur'),
  );
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
    params: () => this.figures() ?? undefined,
    stream: ({ params }) => this.api.previewLoan({ body: params }).pipe(catchError(() => of(null))),
  });

  protected readonly body = computed(() => {
    const figures = this.figures();
    const first = this.first();
    const payer = this.payer();
    const interest = this.interestCategory();
    const insurance = this.insuranceMode() === 'NONE' ? null : this.insuranceCategory();
    if (!figures || !first || !payer || !interest) return null;
    if (this.insuranceMode() !== 'NONE' && !insurance) return null;
    return {
      ...figures,
      first_installment_date: toIsoDate(first),
      payer_account_id: payer,
      interest_category_id: interest,
      insurance_category_id: insurance,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    try {
      await firstValueFrom(this.api.saveLoanTerms({ id: this.data.accountId, body }));
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Mensualités déjà enregistrées à ventiler en une fois selon leurs échéances (E3, CPT-10). */
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
  protected readonly data = inject<{ accountId: string; proposals: LoanPaymentProposal[] }>(
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
      await firstValueFrom(this.api.payLoanInstallments({ id: this.data.accountId, body }));
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Fiche d'un prêt (E3, CPT-09) : résumé et tableau d'amortissement, une année à la fois. */
@Component({
  selector: 'app-loan-panel',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatMenuModule,
    MatTooltipModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card panel">
      <header>
        <h2>Tableau d'amortissement</h2>
        @if (loan.value(); as l) {
          <span class="actions">
            <button mat-stroked-button (click)="edit()"><mat-icon>edit</mat-icon>Conditions</button>
            <button mat-icon-button aria-label="Retirer les conditions du prêt" (click)="remove()">
              <mat-icon>delete</mat-icon>
            </button>
          </span>
        }
      </header>
      @if (loan.value(); as l) {
        <dl class="figures">
          <dt>Mensualité</dt>
          <dd class="amount">
            {{ l.terms.monthly_payment_minor | money }}
            @if (l.terms.monthly_insurance_minor > 0) {
              <span class="muted small"
                >+ {{ l.terms.monthly_insurance_minor | money }} d'assurance</span
              >
            }
          </dd>
          <dt>Capital restant dû</dt>
          <dd class="amount">{{ l.outstanding_minor | money }}</dd>
          <dt>Selon le tableau</dt>
          <dd class="amount">
            {{ l.theoretical_outstanding_minor | money }}
            @if (l.gap_minor !== 0) {
              <span
                class="gap"
                matTooltip="Écart entre la valeur du compte et le tableau : saisir une valorisation ou ventiler les mensualités."
                >écart {{ l.gap_minor | money: 'EUR' : { signed: true } }}</span
              >
            }
          </dd>
          <dt>Coût du crédit</dt>
          <dd class="amount">
            {{ l.total_cost_minor | money }}
            <span class="muted small">dont {{ l.total_insurance_minor | money }} d'assurance</span>
          </dd>
          <dt>Fin du prêt</dt>
          <dd>{{ l.end_date | frDate }}</dd>
        </dl>
        @if (proposals.value()?.length) {
          <p class="pending">
            <mat-icon>info</mat-icon>
            <span
              >{{ proposals.value()?.length }} mensualité(s) déjà enregistrée(s) correspondent à des
              échéances non ventilées.</span
            >
            <button mat-stroked-button (click)="payPast()">Ventiler les mensualités passées</button>
          </p>
        }
        <div class="year">
          <button
            mat-icon-button
            aria-label="Année précédente"
            [disabled]="year() <= firstYear()"
            (click)="year.set(year() - 1)"
          >
            <mat-icon>chevron_left</mat-icon>
          </button>
          <span>{{ year() }}</span>
          <button
            mat-icon-button
            aria-label="Année suivante"
            [disabled]="year() >= lastYear()"
            (click)="year.set(year() + 1)"
          >
            <mat-icon>chevron_right</mat-icon>
          </button>
        </div>
        <table>
          <thead>
            <tr>
              <th><span class="cdk-visually-hidden">Payée</span></th>
              <th>N°</th>
              <th>Date</th>
              <th class="right">Mensualité</th>
              <th class="right wide">Capital</th>
              <th class="right wide">Intérêts</th>
              <th class="right wide">Assurance</th>
              <th class="right">Restant dû</th>
            </tr>
          </thead>
          <tbody>
            @for (i of rows(); track i.number) {
              <tr [class.next]="i.number === l.next_installment">
                <td class="state">
                  @if (i.paid_transaction_id; as paid) {
                    <button
                      mat-icon-button
                      [matMenuTriggerFor]="paidMenu"
                      [attr.aria-label]="'Échéance ' + i.number + ' payée'"
                      matTooltip="Mensualité ventilée"
                    >
                      <mat-icon>check_circle</mat-icon>
                    </button>
                    <mat-menu #paidMenu>
                      <a mat-menu-item [routerLink]="['/transactions', paid]">
                        <mat-icon>open_in_new</mat-icon>Voir la mensualité
                      </a>
                      <button mat-menu-item (click)="unpay(i.number)">
                        <mat-icon>undo</mat-icon>Annuler la ventilation
                      </button>
                    </mat-menu>
                  }
                </td>
                <td>{{ i.number }}</td>
                <td>{{ i.date | frDate: 'short' }}</td>
                <td class="right amount">{{ i.total_minor | money }}</td>
                <td class="right amount wide">{{ i.principal_minor | money }}</td>
                <td class="right amount wide">{{ i.interest_minor | money }}</td>
                <td class="right amount wide">{{ i.insurance_minor | money }}</td>
                <td class="right amount">{{ i.remaining_minor | money }}</td>
              </tr>
            }
          </tbody>
        </table>
      } @else if (missing()) {
        <p class="muted">
          Sans conditions, le capital restant dû se suit par valorisations. Avec elles,
          l'application calcule le tableau d'amortissement.
        </p>
        <button mat-stroked-button (click)="edit()">
          <mat-icon>add</mat-icon>Saisir les conditions du prêt
        </button>
      }
    </section>
  `,
  styles: `
    .panel {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      flex-wrap: wrap;
      justify-content: space-between;
      align-items: center;
      gap: 0.5rem;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .figures {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 0.25rem 1rem;
      margin: 0.5rem 0;
      dd {
        margin: 0;
        text-align: right;
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .gap {
      margin-left: 0.25rem;
      font: var(--mat-sys-label-small);
      color: var(--mat-sys-on-error-container);
      background: var(--mat-sys-error-container);
      border-radius: 0.5rem;
      padding: 0 0.375rem;
    }
    .pending {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem;
      padding: 0.5rem 0.75rem;
      border-radius: var(--mat-sys-corner-medium);
      background: var(--mat-sys-surface-container);
      span {
        flex: 1;
      }
    }
    .state {
      width: 2.5rem;
      padding: 0;
      mat-icon {
        color: var(--mat-sys-primary);
      }
    }
    .year {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 0.5rem;
      font: var(--mat-sys-title-small);
    }
    table {
      width: 100%;
      border-collapse: collapse;
      th,
      td {
        padding: 0.25rem;
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
      tr.next td {
        background: var(--mat-sys-secondary-container);
        font-weight: 500;
      }
      @media (max-width: 599.98px) {
        .wide {
          display: none;
        }
      }
    }
  `,
})
export class LoanPanel {
  private readonly api = inject(LoansService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  readonly accountId = input.required<string>();

  protected readonly loan = rxResource({
    params: () => ({ id: this.accountId(), version: this.version.value() }),
    stream: ({ params }) => this.api.getLoan({ id: params.id }),
  });
  protected readonly proposals = rxResource({
    params: () =>
      this.loan.hasValue() ? { id: this.accountId(), version: this.version.value() } : undefined,
    stream: ({ params }) => this.api.loanPaymentProposals({ id: params.id }),
  });
  /** Pas de conditions : l'API répond NO_LOAN_TERMS. */
  protected readonly missing = computed(
    () => problemOf(this.loan.error())?.code === 'NO_LOAN_TERMS',
  );

  private readonly years = computed(() =>
    (this.loan.value()?.installments ?? []).map((i) => fromIsoDate(i.date).getFullYear()),
  );
  protected readonly firstYear = computed(() => this.years()[0] ?? new Date().getFullYear());
  protected readonly lastYear = computed(() => this.years().at(-1) ?? new Date().getFullYear());
  /** Année affichée : celle de la prochaine échéance, par défaut. */
  protected readonly year = linkedSignal(() => {
    const l = this.loan.value();
    const next = l?.installments.find((i) => i.number === l.next_installment);
    return next ? fromIsoDate(next.date).getFullYear() : this.lastYear();
  });
  protected readonly rows = computed(() =>
    (this.loan.value()?.installments ?? []).filter(
      (i) => fromIsoDate(i.date).getFullYear() === this.year(),
    ),
  );

  protected async edit(): Promise<void> {
    const ref = this.dialog.open(LoanTermsDialog, {
      data: { accountId: this.accountId(), terms: this.loan.value()?.terms ?? null },
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async payPast(): Promise<void> {
    const ref = this.dialog.open(LoanPaymentsDialog, {
      data: { accountId: this.accountId(), proposals: this.proposals.value() ?? [] },
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async unpay(number: number): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Annuler la ventilation ?',
      message:
        'La part de capital est retirée du prêt et la mensualité redevient une dépense non catégorisée.',
      action: 'Annuler la ventilation',
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.unpayLoanInstallment({ id: this.accountId(), number }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  protected async remove(): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Retirer les conditions du prêt ?',
      message: 'Le capital restant dû se suivra de nouveau par valorisations.',
      action: 'Retirer',
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteLoanTerms({ id: this.accountId() }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }
}
