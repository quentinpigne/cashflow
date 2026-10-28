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
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { InterestMethod } from '../../api/models/interest-method';
import { TermsView } from '../../api/models/terms-view';
import { SavingsService } from '../../api/services/savings.service';
import { DataVersion } from '../../core/data-version';
import { keptValue } from '../../core/resources';
import { todayIso } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { formatRate, parseMoney, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';

/** Dernière année consultée, par compte. */
const CHOSEN_YEAR = new Map<string, number>();

const METHOD_LABELS: Record<InterestMethod, string> = {
  FORTNIGHTLY: 'Par quinzaines (livrets réglementés)',
  DAILY: 'Au jour le jour (à venir)',
  MONTHLY: 'Au mois (à venir)',
};

/** Conditions d'un compte à taux déterminé (CPT-05). */
@Component({
  selector: 'app-terms-dialog',
  imports: [
    FormsModule,
    RouterLink,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    CategoryPicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Conditions d'épargne</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Barème de taux</mat-label>
        <mat-select [ngModel]="schedule()" (ngModelChange)="schedule.set($event)">
          @for (s of schedules.value() ?? []; track s.id) {
            <mat-option [value]="s.id">{{ s.name }}</mat-option>
          }
        </mat-select>
        <mat-hint
          ><a routerLink="/reglages/baremes" mat-dialog-close
            >Gérer les barèmes et les taux</a
          ></mat-hint
        >
      </mat-form-field>
      <mat-form-field>
        <mat-label>Calcul des intérêts</mat-label>
        <mat-select [ngModel]="method()" (ngModelChange)="method.set($event)">
          @for (m of methods; track m) {
            <mat-option [value]="m">{{ methodLabels[m] }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <mat-form-field>
        <mat-label>Plafond des versements (facultatif)</mat-label>
        <input matInput inputmode="decimal" [ngModel]="cap()" (ngModelChange)="cap.set($event)" />
        <span matTextSuffix>€</span>
      </mat-form-field>
      <app-category-picker
        label="Catégorie des intérêts"
        kind="INCOME"
        [value]="category()"
        (valueChange)="category.set($event)"
      />
      <mat-checkbox [ngModel]="roundEach()" (ngModelChange)="roundEach.set($event)">
        Arrondir l'intérêt de chaque quinzaine (si la banque procède ainsi)
      </mat-checkbox>
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
      min-width: min(26rem, 80vw);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class TermsDialog {
  private readonly data = inject<{ accountId: string; terms: TermsView }>(MAT_DIALOG_DATA);
  private readonly api = inject(SavingsService);
  private readonly ref = inject(MatDialogRef<TermsDialog>);

  protected readonly methods: InterestMethod[] = ['FORTNIGHTLY', 'DAILY', 'MONTHLY'];
  protected readonly methodLabels = METHOD_LABELS;
  protected readonly schedules = rxResource({ stream: () => this.api.listRateSchedules() });
  protected readonly schedule = signal(this.data.terms.rate_schedule_id);
  protected readonly method = signal(this.data.terms.interest_method);
  protected readonly cap = signal(
    this.data.terms.deposit_cap_minor ? toEditable(this.data.terms.deposit_cap_minor) : '',
  );
  protected readonly category = signal<string | null>(this.data.terms.interest_category_id ?? null);
  protected readonly roundEach = signal(this.data.terms.round_each_fortnight);
  protected readonly error = signal<string | null>(null);

  protected readonly valid = computed(() => {
    const cap = this.cap().trim();
    return cap === '' || (parseMoney(cap) ?? 0) > 0;
  });

  protected async save(): Promise<void> {
    const cap = this.cap().trim();
    try {
      await firstValueFrom(
        this.api.updateSavingsTerms({
          id: this.data.accountId,
          body: {
            rate_schedule_id: this.schedule(),
            interest_method: this.method(),
            capitalization: this.data.terms.capitalization,
            deposit_cap_minor: cap ? parseMoney(cap) : null,
            maturity_date: this.data.terms.maturity_date ?? null,
            interest_category_id: this.category(),
            round_each_fortnight: this.roundEach(),
          },
        }),
      );
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Intérêts d'un compte à taux déterminé (EPA-02 à EPA-04), sur la fiche du compte (E3). */
@Component({
  selector: 'app-interest-panel',
  imports: [
    MatButtonModule,
    MatExpansionModule,
    MatIconModule,
    MatTooltipModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card panel">
      <header>
        <h2>Intérêts</h2>
        <span class="year">
          <button mat-icon-button aria-label="Année précédente" (click)="showYear(year() - 1)">
            <mat-icon>chevron_left</mat-icon>
          </button>
          <strong>{{ year() }}</strong>
          <button
            mat-icon-button
            aria-label="Année suivante"
            (click)="showYear(year() + 1)"
            [disabled]="year() >= currentYear"
          >
            <mat-icon>chevron_right</mat-icon>
          </button>
        </span>
        <button mat-button (click)="editTerms()"><mat-icon>tune</mat-icon>Conditions</button>
      </header>
      @if (currentTerms(); as t) {
        <p class="muted small">
          {{ t.schedule_name }} · taux actuel <strong>{{ rate(t.current_rate_ppm) }}</strong>
        </p>
      }
      @if (currentInterest(); as i) {
        @if (!i.supported) {
          <p class="muted">
            Le calcul de cette méthode arrive dans une version ultérieure : seules les quinzaines
            sont prises en charge.
          </p>
        } @else {
          <dl class="figures">
            @if (i.year === currentYear) {
              <dt>Courus depuis le 1er janvier</dt>
              <dd class="amount">{{ i.accrued_minor | money }}</dd>
              <dt>Attendus au 31 décembre</dt>
              <dd class="amount">
                {{ i.expected_minor | money }}
                <mat-icon
                  class="info"
                  matTooltip="Si aucun mouvement n'intervient d'ici là, au taux connu aujourd'hui."
                  >info</mat-icon
                >
              </dd>
            } @else {
              <dt>Calculés pour {{ i.year }}</dt>
              <dd class="amount">{{ i.expected_minor | money }}</dd>
              <dt>Inscrits</dt>
              <dd class="amount">
                {{
                  i.booked_minor === null || i.booked_minor === undefined
                    ? '—'
                    : (i.booked_minor | money)
                }}
                @if (gap(i); as g) {
                  <span class="gap">écart {{ g | money: 'EUR' : { signed: true } }}</span>
                }
              </dd>
            }
            @if (i.deposit_cap_minor) {
              <dt>Capital versé / plafond</dt>
              <dd class="amount" [class.negative]="!!i.cap_exceeded_on">
                {{ i.capital_minor | money }} / {{ i.deposit_cap_minor | money }}
              </dd>
            }
          </dl>
          @if (i.cap_exceeded_on) {
            <p class="warning">
              <mat-icon>warning</mat-icon> Le versement du {{ i.cap_exceeded_on | frDate }} dépasse
              le plafond : la banque peut le refuser.
            </p>
          }
          @if (i.proposal === 'AVAILABLE' && i.expected_minor > 0) {
            <p>
              <button mat-flat-button (click)="record(i.year)">
                Inscrire les intérêts {{ i.year }} ({{ i.expected_minor | money }})
              </button>
            </p>
          }
          @if (i.fortnights.length > 0) {
            <mat-expansion-panel class="detail">
              <mat-expansion-panel-header>Détail par quinzaine</mat-expansion-panel-header>
              <table>
                <thead>
                  <tr>
                    <th>Quinzaine</th>
                    <th class="right">Solde en valeur</th>
                    <th class="right">Taux</th>
                    <th class="right">Intérêt</th>
                  </tr>
                </thead>
                <tbody>
                  @for (f of i.fortnights; track f.start) {
                    <tr [class.future]="f.end > today">
                      <td>{{ f.start | frDate: 'short' }} – {{ f.end | frDate: 'short' }}</td>
                      <td class="right">{{ f.balance_minor | money }}</td>
                      <td class="right">{{ rate(f.rate_ppm) }}</td>
                      <td class="right">{{ cents(f.interest) }}</td>
                    </tr>
                  }
                </tbody>
              </table>
            </mat-expansion-panel>
          }
        }
      }
    </section>
  `,
  styles: `
    .panel {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
        flex: 1;
      }
    }
    .year {
      display: flex;
      align-items: center;
    }
    .small {
      font: var(--mat-sys-body-small);
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
    .info {
      font-size: 1rem;
      width: 1rem;
      height: 1rem;
      vertical-align: middle;
      color: var(--mat-sys-on-surface-variant);
    }
    .gap {
      margin-left: 0.5rem;
      color: var(--mat-sys-error);
    }
    .warning {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      color: var(--mat-sys-error);
    }
    .detail {
      margin-top: 0.5rem;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font: var(--mat-sys-body-small);
      th,
      td {
        padding: 0.25rem;
        border-top: 1px solid var(--mat-sys-outline-variant);
      }
    }
    .right {
      text-align: right;
    }
    .future {
      color: var(--mat-sys-on-surface-variant);
      font-style: italic;
    }
  `,
})
export class InterestPanel {
  private readonly api = inject(SavingsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  readonly accountId = input.required<string>();

  protected readonly today = todayIso();
  protected readonly currentYear = Number(this.today.slice(0, 4));
  /** Année affichée, gardée par compte : la fiche recrée le panneau à chaque rechargement. */
  protected readonly year = linkedSignal(
    () => CHOSEN_YEAR.get(this.accountId()) ?? this.currentYear,
  );

  protected showYear(year: number): void {
    this.year.set(year);
    CHOSEN_YEAR.set(this.accountId(), year);
  }

  protected readonly terms = rxResource({
    params: () => ({ id: this.accountId(), version: this.version.value() }),
    stream: ({ params }) => this.api.getSavingsTerms({ id: params.id }),
  });
  protected readonly interest = rxResource({
    params: () => ({ id: this.accountId(), year: this.year(), version: this.version.value() }),
    stream: ({ params }) => this.api.getInterest({ id: params.id, year: params.year }),
  });
  protected readonly currentTerms = keptValue(this.terms, this.accountId);
  protected readonly currentInterest = keptValue(
    this.interest,
    () => `${this.accountId()}/${this.year()}`,
  );

  protected rate(ppm: number): string {
    return formatRate(ppm);
  }

  /** Intérêt d'une quinzaine en euros, deux décimales (le calcul garde la précision, R-EPA-6). */
  protected cents(interest: string): string {
    const euros = Number(interest) / 100;
    return (
      euros.toLocaleString('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }) + ' €'
    );
  }

  protected gap(i: { booked_minor?: number | null; expected_minor: number }): number | null {
    if (i.booked_minor === null || i.booked_minor === undefined) return null;
    const gap = i.booked_minor - i.expected_minor;
    return gap === 0 ? null : gap;
  }

  protected async editTerms(): Promise<void> {
    const terms = this.currentTerms();
    if (!terms) return;
    const ref = this.dialog.open(TermsDialog, { data: { accountId: this.accountId(), terms } });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async record(year: number): Promise<void> {
    try {
      await firstValueFrom(this.api.recordInterest({ id: this.accountId(), year }));
      this.version.bump();
      this.snack.open(
        `Intérêts ${year} inscrits, en attente : l'import du relevé les pointera.`,
        undefined,
        { duration: 5000 },
      );
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }
}
