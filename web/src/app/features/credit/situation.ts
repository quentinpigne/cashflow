import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  computed,
  inject,
  input,
  output,
  signal,
} from '@angular/core';
import { takeUntilDestroyed, toObservable } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { catchError, debounceTime, distinctUntilChanged, filter, of, skip, switchMap } from 'rxjs';

import { BorrowerSituation } from '../../api/models/borrower-situation';
import { SituationView } from '../../api/models/situation-view';
import { CreditService } from '../../api/services/credit.service';
import { errorMessage } from '../../core/http-errors';
import { Layout } from '../../core/layout';
import { formatRate, parseRatePpm } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';
import { editable, moneyOrZero } from './applications';
import { rateEditable } from './credit-shared';

/** E36 — Ma situation : le résumé d'une ligne et « Ajuster » (R-CRE-30). */
@Component({
  selector: 'app-situation-bar',
  imports: [MatButtonModule, MatIconModule, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card situation" aria-label="Ma situation">
      @if (view(); as v) {
        <div class="summary">
          <strong class="title">Ma situation</strong>
          <span><span class="muted">Revenus</span> {{ v.income_minor | money }}/mois</span>
          <span><span class="muted">Charges</span> {{ v.charges_minor | money }}</span>
          <span class="wide"
            ><span class="muted">Épargne mobilisable</span> {{ v.savings_minor | money }}</span
          >
          <span
            ><span class="muted">Apport possible</span>
            {{ v.possible_down_payment_minor | money }}</span
          >
          <span
            ><span class="muted">Endettement visé</span>
            {{ rate(v.situation.max_debt_ratio_ppm) }}</span
          >
        </div>
        <button mat-stroked-button (click)="adjust(v)"><mat-icon>tune</mat-icon>Ajuster</button>
      } @else {
        <span class="muted">Chargement de votre situation…</span>
      }
    </section>
  `,
  styles: `
    .situation {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem 1rem;
      margin-bottom: 1rem;
    }
    .summary {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem 1.25rem;
      align-items: baseline;
    }
    .title {
      font: var(--mat-sys-title-small);
    }
    @media (max-width: 599.98px) {
      .wide {
        display: none;
      }
    }
  `,
})
export class SituationBar {
  private readonly dialog = inject(MatDialog);
  private readonly layout = inject(Layout);

  readonly view = input<SituationView | null>(null);
  /** La situation, après chaque changement gardé. */
  readonly changed = output<SituationView>();

  protected readonly rate = formatRate;

  protected adjust(view: SituationView): void {
    const data: SituationPanelData = { view, changed: (v) => this.changed.emit(v) };
    const mobile = this.layout.mobile();
    this.dialog.open(SituationPanel, {
      data,
      autoFocus: false,
      ...(mobile
        ? {
            width: '100vw',
            maxWidth: '100vw',
            position: { bottom: '0' },
            panelClass: 'bottom-panel',
          }
        : { width: '28rem', position: { right: '0', top: '0' }, panelClass: 'side-panel' }),
    });
  }
}

interface SituationPanelData {
  view: SituationView;
  changed: (view: SituationView) => void;
}

/** E36 — Ajuster ma situation : panneau latéral, ou feuille du bas sur mobile (R-CRE-30). */
@Component({
  selector: 'app-situation-panel',
  imports: [
    FormsModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDialogModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Ma situation</h2>
    <mat-dialog-content>
      @let v = view();
      <p class="muted small">
        Commune à tous les projets ; chaque changement est gardé et recalcule le résultat.
      </p>
      <div class="fields">
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
          <mat-hint>de dépenses ({{ v.expenses_minor | money }} par mois)</mat-hint>
        </mat-form-field>
      </div>

      <h3>Épargne mobilisable pour l'apport</h3>
      @for (a of v.accounts; track a.id) {
        <div class="check-line">
          <mat-checkbox [checked]="accounts().has(a.id)" (change)="toggleAccount(a.id)">{{
            a.name
          }}</mat-checkbox>
          <span class="amount">{{ a.balance_minor | money }}</span>
        </div>
      }

      @if (v.loans.length > 0) {
        <h3>Prêts en cours remboursés par ce crédit</h3>
        @for (l of v.loans; track l.loan_id) {
          <div class="check-line">
            <mat-checkbox [checked]="excluded().has(l.loan_id)" (change)="toggleLoan(l.loan_id)">{{
              l.name
            }}</mat-checkbox>
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
      </div>
      <h3>Co-emprunteur</h3>
      <div class="fields">
        <mat-form-field>
          <mat-label>Ses revenus</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="coIncome()"
            (ngModelChange)="coIncome.set($event)"
          />
          <span matTextSuffix>€</span>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Ses charges</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="coCharges()"
            (ngModelChange)="coCharges.set($event)"
          />
          <span matTextSuffix>€</span>
        </mat-form-field>
      </div>

      <div class="totals">
        <span>Revenus</span><span class="amount">{{ v.income_minor | money }}</span>
        <span>Charges</span><span class="amount">{{ v.charges_minor | money }}</span>
        <span>Épargne mobilisable</span><span class="amount">{{ v.savings_minor | money }}</span>
        <span>Réserve</span><span class="amount">− {{ v.reserve_minor | money }}</span>
        <strong>Apport possible</strong
        ><strong class="amount">{{ v.possible_down_payment_minor | money }}</strong>
      </div>
      @if (busy()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (error() || invalid()) {
        <p class="error" role="alert">{{ error() ?? 'Saisie incomplète ou invalide.' }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-flat-button mat-dialog-close>Fermer</button>
    </mat-dialog-actions>
  `,
  styles: `
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
    .totals {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 0.25rem 1rem;
      margin-top: 1rem;
      padding-top: 0.75rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class SituationPanel {
  private readonly api = inject(CreditService);
  private readonly data = inject<SituationPanelData>(MAT_DIALOG_DATA);

  protected readonly view = signal(this.data.view);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  private readonly s = this.data.view.situation;
  protected readonly debtRatio = signal(rateEditable(this.s.max_debt_ratio_ppm));
  protected readonly reserveMonths = signal(String(this.s.reserve_months));
  protected readonly otherIncome = signal(editable(this.s.other_income_minor));
  protected readonly otherCharges = signal(editable(this.s.other_charges_minor));
  protected readonly coIncome = signal(editable(this.s.co_borrower_income_minor));
  protected readonly coCharges = signal(editable(this.s.co_borrower_charges_minor));
  protected readonly accounts = signal<ReadonlySet<string>>(
    new Set(this.data.view.accounts.filter((a) => a.selected).map((a) => a.id)),
  );
  protected readonly excluded = signal<ReadonlySet<string>>(new Set(this.s.excluded_loan_ids));

  /** La situation saisie, ou `null` tant qu'une saisie est invalide. */
  private readonly draft = computed<BorrowerSituation | null>(() => {
    const debt = parseRatePpm(this.debtRatio());
    const reserve = /^\d{1,3}$/.test(this.reserveMonths().trim())
      ? Number(this.reserveMonths())
      : null;
    const amounts = [
      this.otherIncome(),
      this.otherCharges(),
      this.coIncome(),
      this.coCharges(),
    ].map(moneyOrZero);
    if (debt === null || debt <= 0 || reserve === null) return null;
    if (amounts.some((a) => a === null || a < 0)) return null;
    const [otherIncome, otherCharges, coIncome, coCharges] = amounts as number[];
    return {
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
  protected readonly invalid = computed(() => this.draft() === null);

  constructor() {
    toObservable(this.draft)
      .pipe(
        filter((d): d is BorrowerSituation => d !== null),
        distinctUntilChanged((a, b) => JSON.stringify(a) === JSON.stringify(b)),
        // La situation à l'ouverture n'est pas un changement.
        skip(1),
        debounceTime(400),
        switchMap((body) => {
          this.busy.set(true);
          return this.api.saveBorrowerSituation({ body }).pipe(
            catchError((e) => {
              this.error.set(errorMessage(e));
              return of(null);
            }),
          );
        }),
        takeUntilDestroyed(inject(DestroyRef)),
      )
      .subscribe((view) => {
        this.busy.set(false);
        if (!view) return;
        this.error.set(null);
        this.view.set(view);
        this.data.changed(view);
      });
  }

  protected toggleAccount(id: string): void {
    this.accounts.update((set) => toggled(set, id));
  }

  protected toggleLoan(id: string): void {
    this.excluded.update((set) => toggled(set, id));
  }
}

function toggled(set: ReadonlySet<string>, id: string): ReadonlySet<string> {
  const next = new Set(set);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}
