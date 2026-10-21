import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatRadioModule } from '@angular/material/radio';
import { MatSelectModule } from '@angular/material/select';
import { ComponentType } from '@angular/cdk/portal';
import { firstValueFrom } from 'rxjs';

import { AmountScope } from '../../api/models/amount-scope';
import { BudgetKind } from '../../api/models/budget-kind';
import { BudgetLineMonth } from '../../api/models/budget-line-month';
import { Periodicity } from '../../api/models/periodicity';
import { BudgetsService } from '../../api/services/budgets.service';
import { DataVersion } from '../../core/data-version';
import { currentMonth, formatMonthKey } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { parseMoney, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';

const MONTH_NAMES = [
  'janvier',
  'février',
  'mars',
  'avril',
  'mai',
  'juin',
  'juillet',
  'août',
  'septembre',
  'octobre',
  'novembre',
  'décembre',
];

/** Seuils d'alerte saisis : un ou deux pourcentages (R-BUD-60). */
function parseThresholds(first: string, second: string): number[] | null {
  const values = [first, second]
    .map((v) => v.trim())
    .filter((v) => v !== '')
    .map(Number);
  if (values.length === 0 || values.some((v) => !Number.isInteger(v) || v < 1 || v > 500)) {
    return null;
  }
  return values;
}

export interface LineCreateData {
  month: string;
  kind?: BudgetKind;
}

/** Nouvelle ligne de budget, à partir du mois affiché (BUD-02, BUD-05, BUD-07). */
@Component({
  selector: 'app-budget-line-create',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    CategoryPicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Nouvelle ligne</h2>
    <mat-dialog-content class="form">
      <mat-button-toggle-group
        [value]="kind()"
        (change)="kind.set($event.value)"
        aria-label="Nature de la ligne"
      >
        <mat-button-toggle value="EXPENSE">Dépense</mat-button-toggle>
        <mat-button-toggle value="INCOME">Revenu</mat-button-toggle>
        <mat-button-toggle value="SAVINGS">Épargne</mat-button-toggle>
      </mat-button-toggle-group>

      @if (kind() === 'SAVINGS') {
        <mat-form-field>
          <mat-label>Compte d'épargne</mat-label>
          <mat-select [ngModel]="account()" (ngModelChange)="account.set($event)">
            @for (a of savingsAccounts(); track a.id) {
              <mat-option [value]="a.id">{{ a.name }}</mat-option>
            } @empty {
              <mat-option disabled>Aucun compte d'épargne</mat-option>
            }
          </mat-select>
        </mat-form-field>
      } @else {
        <app-category-picker
          [kind]="kind() === 'INCOME' ? 'INCOME' : 'EXPENSE'"
          [value]="category()"
          (valueChange)="category.set($event)"
        />
        <mat-button-toggle-group
          [value]="periodicity()"
          (change)="periodicity.set($event.value)"
          aria-label="Périodicité"
        >
          <mat-button-toggle value="MONTHLY">Par mois</mat-button-toggle>
          <mat-button-toggle value="YEARLY">Par an</mat-button-toggle>
        </mat-button-toggle-group>
        @if (periodicity() === 'YEARLY') {
          <mat-form-field>
            <mat-label>L'année commence en</mat-label>
            <mat-select [ngModel]="yearStart()" (ngModelChange)="yearStart.set($event)">
              @for (name of monthNames; track $index) {
                <mat-option [value]="$index + 1">{{ name }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        }
      }

      <mat-form-field>
        <mat-label>{{
          periodicity() === 'YEARLY' && kind() !== 'SAVINGS' ? 'Montant par an' : 'Montant par mois'
        }}</mat-label>
        <input
          matInput
          inputmode="decimal"
          [ngModel]="amount()"
          (ngModelChange)="amount.set($event)"
        />
        <span matTextSuffix>€</span>
      </mat-form-field>

      @if (kind() !== 'INCOME') {
        <mat-checkbox [ngModel]="rollover()" (ngModelChange)="rollover.set($event)">
          Reporter le reliquat d'un mois sur l'autre
        </mat-checkbox>
      }
      <p class="muted small">À partir de {{ monthLabel }}.</p>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid() || saving()" (click)="save()">Ajouter</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
      min-width: min(24rem, 80vw);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class LineCreateDialog {
  protected readonly data = inject<LineCreateData>(MAT_DIALOG_DATA);
  private readonly api = inject(BudgetsService);
  private readonly accounts = inject(AccountsStore);
  private readonly ref = inject(MatDialogRef<LineCreateDialog>);

  protected readonly monthNames = MONTH_NAMES;
  protected readonly monthLabel = formatMonthKey(this.data.month).toLowerCase();
  protected readonly kind = signal<BudgetKind>(this.data.kind ?? 'EXPENSE');
  protected readonly category = signal<string | null>(null);
  protected readonly account = signal<string | null>(null);
  protected readonly periodicity = signal<Periodicity>('MONTHLY');
  protected readonly yearStart = signal(1);
  protected readonly amount = signal('');
  protected readonly rollover = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly saving = signal(false);

  protected readonly savingsAccounts = computed(() =>
    this.accounts
      .open()
      .filter((a) => a.family === 'FIXED_SAVINGS' || a.family === 'VARIABLE_SAVINGS'),
  );

  protected readonly valid = computed(() => {
    const amount = parseMoney(this.amount());
    const target = this.kind() === 'SAVINGS' ? this.account() : this.category();
    return amount !== null && amount > 0 && !!target;
  });

  protected async save(): Promise<void> {
    const savings = this.kind() === 'SAVINGS';
    this.saving.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(
        this.api.createBudgetLine({
          body: {
            kind: this.kind(),
            category_id: savings ? null : this.category(),
            target_account_id: savings ? this.account() : null,
            periodicity: savings ? 'MONTHLY' : this.periodicity(),
            year_start_month: this.yearStart(),
            amount_minor: parseMoney(this.amount()) ?? 0,
            rollover: this.kind() !== 'INCOME' && this.rollover(),
            from: this.data.month,
          },
        }),
      );
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.saving.set(false);
    }
  }
}

export interface LineEditData {
  line: BudgetLineMonth;
  month: string;
  thresholds: number[];
}

/** Modifier une ligne : montant (à partir de ce mois ou ce mois seulement), report, seuils (R-BUD-51). */
@Component({
  selector: 'app-budget-line-edit',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatInputModule,
    MatRadioModule,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ line.name }}</h2>
    <mat-dialog-content class="form">
      <p class="muted small">
        {{ line.periodicity === 'YEARLY' ? 'Ligne annuelle' : 'Ligne mensuelle' }}, en vigueur
        depuis {{ fromLabel }}.
        @if (line.override_minor !== null && line.override_minor !== undefined) {
          Montant habituel : {{ line.amount_minor | money }}.
        }
      </p>
      <mat-form-field>
        <mat-label>{{ line.periodicity === 'YEARLY' ? 'Montant par an' : 'Montant' }}</mat-label>
        <input
          matInput
          inputmode="decimal"
          [ngModel]="amount()"
          (ngModelChange)="amount.set($event)"
        />
        <span matTextSuffix>€</span>
      </mat-form-field>
      @if (amountChanged()) {
        <mat-radio-group
          class="scope"
          [ngModel]="scope()"
          (ngModelChange)="scope.set($event)"
          aria-label="Portée du changement"
        >
          <mat-radio-button value="FROM_MONTH">À partir de {{ monthLabel }}</mat-radio-button>
          <mat-radio-button value="THIS_MONTH">{{
            line.periodicity === 'YEARLY'
              ? 'Pour cette année budgétaire seulement'
              : 'Pour ' + monthLabel + ' seulement'
          }}</mat-radio-button>
        </mat-radio-group>
        @if (past) {
          <p class="warning">Ce mois est passé : le changement modifie les reports cumulés.</p>
        }
      }

      @if (line.kind !== 'INCOME') {
        <mat-checkbox [ngModel]="rollover()" (ngModelChange)="rollover.set($event)">
          Reporter le reliquat d'un mois sur l'autre
        </mat-checkbox>
      }
      @if (line.kind === 'EXPENSE') {
        <mat-checkbox [ngModel]="customThresholds()" (ngModelChange)="customThresholds.set($event)">
          Seuils d'alerte propres à cette ligne
        </mat-checkbox>
        @if (customThresholds()) {
          <div class="thresholds">
            <mat-form-field>
              <mat-label>Premier seuil</mat-label>
              <input
                matInput
                inputmode="numeric"
                [ngModel]="first()"
                (ngModelChange)="first.set($event)"
              />
              <span matTextSuffix>%</span>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Second seuil</mat-label>
              <input
                matInput
                inputmode="numeric"
                [ngModel]="second()"
                (ngModelChange)="second.set($event)"
              />
              <span matTextSuffix>%</span>
            </mat-form-field>
          </div>
        }
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
      <div class="secondary">
        @if (line.rollover && line.carried_minor !== 0) {
          <button mat-button (click)="resetRollover()">Remettre le report à zéro</button>
        }
        @if (line.override_minor !== null && line.override_minor !== undefined) {
          <button mat-button (click)="removeOverride()">Revenir au montant habituel</button>
        }
        <button mat-button class="danger" (click)="end()">
          Supprimer à partir de {{ monthLabel }}
        </button>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid() || saving()" (click)="save()">
        Enregistrer
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
      min-width: min(26rem, 80vw);
    }
    .scope {
      display: flex;
      flex-direction: column;
    }
    .thresholds {
      display: flex;
      gap: 0.5rem;
    }
    .secondary {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem;
    }
    .danger {
      color: var(--mat-sys-error);
    }
    .warning {
      color: var(--mat-sys-tertiary);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class LineEditDialog {
  private readonly data = inject<LineEditData>(MAT_DIALOG_DATA);
  private readonly api = inject(BudgetsService);
  private readonly dialog = inject(MatDialog);
  private readonly ref = inject(MatDialogRef<LineEditDialog>);

  protected readonly line = this.data.line;
  protected readonly monthLabel = formatMonthKey(this.data.month).toLowerCase();
  protected readonly fromLabel = formatMonthKey(this.line.effective_from).toLowerCase();
  protected readonly past = this.data.month < currentMonth();
  private readonly initialAmount = this.line.override_minor ?? this.line.amount_minor;

  protected readonly amount = signal(toEditable(this.initialAmount));
  protected readonly scope = signal<AmountScope>('FROM_MONTH');
  protected readonly rollover = signal(this.line.rollover);
  protected readonly customThresholds = signal(!!this.line.alert_thresholds);
  protected readonly first = signal(
    String((this.line.alert_thresholds ?? this.data.thresholds)[0] ?? ''),
  );
  protected readonly second = signal(
    String((this.line.alert_thresholds ?? this.data.thresholds)[1] ?? ''),
  );
  protected readonly error = signal<string | null>(null);
  protected readonly saving = signal(false);

  protected readonly amountChanged = computed(
    () => parseMoney(this.amount()) !== this.initialAmount,
  );
  protected readonly valid = computed(() => {
    const amount = parseMoney(this.amount());
    if (amount === null || amount < 0) return false;
    return !this.customThresholds() || parseThresholds(this.first(), this.second()) !== null;
  });

  private async run(action: () => Promise<unknown>): Promise<void> {
    this.saving.set(true);
    this.error.set(null);
    try {
      await action();
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.saving.set(false);
    }
  }

  protected save(): Promise<void> {
    return this.run(async () => {
      const thresholds = this.customThresholds()
        ? parseThresholds(this.first(), this.second())
        : null;
      const settingsChanged =
        this.rollover() !== this.line.rollover ||
        JSON.stringify(thresholds) !== JSON.stringify(this.line.alert_thresholds ?? null);
      // Les réglages d'abord : une nouvelle version les reprend.
      if (settingsChanged) {
        await firstValueFrom(
          this.api.updateBudgetLine({
            id: this.line.id,
            body: { rollover: this.rollover(), alert_thresholds: thresholds },
          }),
        );
      }
      if (this.amountChanged()) {
        await firstValueFrom(
          this.api.changeBudgetAmount({
            id: this.line.id,
            body: {
              month: this.data.month,
              amount_minor: parseMoney(this.amount()) ?? 0,
              scope: this.scope(),
            },
          }),
        );
      }
    });
  }

  protected resetRollover(): Promise<void> {
    return this.run(() =>
      firstValueFrom(
        this.api.resetBudgetRollover({ id: this.line.id, body: { month: this.data.month } }),
      ),
    );
  }

  protected removeOverride(): Promise<void> {
    const period = this.line.periodicity === 'YEARLY' ? this.yearStart() : this.data.month;
    return this.run(() =>
      firstValueFrom(this.api.deleteBudgetOverride({ id: this.line.id, period })),
    );
  }

  /** Premier mois de l'année budgétaire qui contient le mois affiché (R-BUD-15). */
  private yearStart(): string {
    const [y, m] = this.data.month.split('-').map(Number);
    const start = this.line.year_start_month;
    const year = m >= start ? y : y - 1;
    return `${year}-${String(start).padStart(2, '0')}`;
  }

  protected async end(): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${this.line.name} » ?`,
      message: `La ligne disparaît à partir de ${this.monthLabel} ; les mois précédents la gardent.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) {
      await this.run(() =>
        firstValueFrom(
          this.api.endBudgetLine({ id: this.line.id, body: { month: this.data.month } }),
        ),
      );
    }
  }
}

/** Seuils d'alerte globaux, en pour cent du disponible (R-BUD-60). */
@Component({
  selector: 'app-budget-thresholds',
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Seuils d'alerte</h2>
    <mat-dialog-content>
      <p class="muted">
        Une notification est émise la première fois que le réel d'une ligne ou d'un projet atteint
        ce pourcentage du disponible. Chaque ligne peut avoir ses propres seuils.
      </p>
      <div class="thresholds">
        <mat-form-field>
          <mat-label>Premier seuil</mat-label>
          <input
            matInput
            inputmode="numeric"
            [ngModel]="first()"
            (ngModelChange)="first.set($event)"
          />
          <span matTextSuffix>%</span>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Second seuil</mat-label>
          <input
            matInput
            inputmode="numeric"
            [ngModel]="second()"
            (ngModelChange)="second.set($event)"
          />
          <span matTextSuffix>%</span>
        </mat-form-field>
      </div>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!thresholds()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .thresholds {
      display: flex;
      gap: 0.5rem;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class ThresholdsDialog {
  private readonly data = inject<{ thresholds: number[] }>(MAT_DIALOG_DATA);
  private readonly api = inject(BudgetsService);
  private readonly ref = inject(MatDialogRef<ThresholdsDialog>);

  protected readonly first = signal(String(this.data.thresholds[0] ?? ''));
  protected readonly second = signal(String(this.data.thresholds[1] ?? ''));
  protected readonly thresholds = computed(() => parseThresholds(this.first(), this.second()));
  protected readonly error = signal<string | null>(null);

  protected async save(): Promise<void> {
    try {
      await firstValueFrom(
        this.api.setBudgetThresholds({ body: { thresholds: this.thresholds() ?? [] } }),
      );
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Ouvre un dialogue de budget et signale un changement des données s'il aboutit. */
export async function openAndRefresh<T, D>(
  dialog: MatDialog,
  version: DataVersion,
  component: ComponentType<T>,
  data: D,
): Promise<boolean> {
  const ref = dialog.open(component, { data, autoFocus: 'first-tabbable' });
  // Un dialogue abouti se ferme avec `true` ou l'identifiant enregistré ; « Annuler » ne renvoie rien.
  const done = !!(await firstValueFrom(ref.afterClosed()));
  if (done) {
    version.bump();
  }
  return done;
}
