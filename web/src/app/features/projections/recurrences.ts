import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
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
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTabsModule } from '@angular/material/tabs';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { Frequency } from '../../api/models/frequency';
import { OccurrenceView } from '../../api/models/occurrence-view';
import { RecurringRuleView } from '../../api/models/recurring-rule-view';
import { ProjectionsService } from '../../api/services/projections.service';
import { DataVersion } from '../../core/data-version';
import { formatDate, fromIsoDate, toIsoDate, todayIso } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { abs, parseMoney, toEditable } from '../../core/money';
import { CategoryPicker } from '../../shared/category-picker';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';

const FREQUENCY_LABELS: Record<Frequency, string> = {
  WEEKLY: 'semaine',
  MONTHLY: 'mois',
  QUARTERLY: 'trimestre',
  YEARLY: 'an',
};

type Direction = 'out' | 'in' | 'transfer';

/** Création ou modification d'une récurrence (TRX-07, R-TRX-44). */
@Component({
  selector: 'app-recurrence-dialog',
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
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ rule ? 'Modifier la récurrence' : 'Nouvelle récurrence' }}</h2>
    <mat-dialog-content class="form">
      <mat-button-toggle-group
        [value]="direction()"
        (change)="direction.set($event.value)"
        aria-label="Sens"
      >
        <mat-button-toggle value="out">Dépense</mat-button-toggle>
        <mat-button-toggle value="in">Revenu</mat-button-toggle>
        <mat-button-toggle value="transfer">Virement</mat-button-toggle>
      </mat-button-toggle-group>
      <mat-form-field>
        <mat-label>Libellé</mat-label>
        <input matInput [ngModel]="label()" (ngModelChange)="label.set($event)" maxlength="200" />
      </mat-form-field>
      <mat-form-field>
        <mat-label>{{ direction() === 'transfer' ? 'Depuis le compte' : 'Compte' }}</mat-label>
        <mat-select [ngModel]="account()" (ngModelChange)="account.set($event)">
          @for (a of accounts.open(); track a.id) {
            <mat-option [value]="a.id">{{ a.name }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      @if (direction() === 'transfer') {
        <mat-form-field>
          <mat-label>Vers le compte</mat-label>
          <mat-select [ngModel]="target()" (ngModelChange)="target.set($event)">
            @for (a of accounts.open(); track a.id) {
              @if (a.id !== account()) {
                <mat-option [value]="a.id">{{ a.name }}</mat-option>
              }
            }
          </mat-select>
        </mat-form-field>
      } @else {
        <app-category-picker
          [kind]="direction() === 'in' ? 'INCOME' : 'EXPENSE'"
          [value]="category()"
          (valueChange)="category.set($event)"
        />
      }
      <div class="row">
        <mat-form-field>
          <mat-label>Montant</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="amount()"
            (ngModelChange)="amount.set($event)"
          />
          <span matTextSuffix>€</span>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Écart toléré</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="tolerance()"
            (ngModelChange)="tolerance.set($event)"
          />
          <span matTextSuffix>€</span>
        </mat-form-field>
      </div>
      <div class="row">
        <mat-form-field class="interval">
          <mat-label>Tous les</mat-label>
          <input
            matInput
            type="number"
            min="1"
            [ngModel]="interval()"
            (ngModelChange)="interval.set($event)"
          />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Période</mat-label>
          <mat-select [ngModel]="frequency()" (ngModelChange)="frequency.set($event)">
            @for (f of frequencies; track f) {
              <mat-option [value]="f">{{ frequencyLabels[f] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
      </div>
      <div class="row">
        <mat-form-field>
          <mat-label>Première échéance</mat-label>
          <input
            matInput
            [matDatepicker]="anchorPicker"
            [ngModel]="anchor()"
            (ngModelChange)="anchor.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="anchorPicker" />
          <mat-datepicker #anchorPicker />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Fin (facultative)</mat-label>
          <input
            matInput
            [matDatepicker]="endPicker"
            [ngModel]="end()"
            (ngModelChange)="end.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="endPicker" />
          <mat-datepicker #endPicker />
        </mat-form-field>
      </div>
      <mat-checkbox [ngModel]="active()" (ngModelChange)="active.set($event)">Active</mat-checkbox>
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
      gap: 0.5rem;
      min-width: min(28rem, 80vw);
    }
    .row {
      display: flex;
      gap: 0.5rem;
      > * {
        flex: 1;
      }
    }
    .interval {
      max-width: 6rem;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class RecurrenceDialog {
  protected readonly rule = inject<RecurringRuleView | null>(MAT_DIALOG_DATA);
  protected readonly accounts = inject(AccountsStore);
  private readonly api = inject(ProjectionsService);
  private readonly ref = inject(MatDialogRef<RecurrenceDialog>);

  protected readonly frequencies: Frequency[] = ['WEEKLY', 'MONTHLY', 'QUARTERLY', 'YEARLY'];
  protected readonly frequencyLabels = FREQUENCY_LABELS;
  protected readonly direction = signal<Direction>(
    this.rule?.target_account_id ? 'transfer' : (this.rule?.amount_minor ?? -1) > 0 ? 'in' : 'out',
  );
  protected readonly label = signal(this.rule?.label ?? '');
  protected readonly account = signal<string | null>(this.rule?.account_id ?? null);
  protected readonly target = signal<string | null>(this.rule?.target_account_id ?? null);
  protected readonly category = signal<string | null>(
    this.rule?.split_template?.[0]?.category_id ?? null,
  );
  protected readonly amount = signal(this.rule ? toEditable(abs(this.rule.amount_minor)) : '');
  protected readonly tolerance = signal(
    this.rule?.amount_tolerance_minor ? toEditable(this.rule.amount_tolerance_minor) : '',
  );
  protected readonly frequency = signal<Frequency>(this.rule?.frequency ?? 'MONTHLY');
  protected readonly interval = signal(this.rule?.interval ?? 1);
  protected readonly anchor = signal<Date | null>(
    fromIsoDate(this.rule?.anchor_date ?? todayIso()),
  );
  protected readonly end = signal<Date | null>(
    this.rule?.end_date ? fromIsoDate(this.rule.end_date) : null,
  );
  protected readonly active = signal(this.rule?.active ?? true);
  protected readonly error = signal<string | null>(null);

  protected readonly valid = computed(() => {
    const amount = parseMoney(this.amount());
    return (
      !!this.label().trim() &&
      !!this.account() &&
      !!this.anchor() &&
      amount !== null &&
      amount > 0 &&
      (this.direction() !== 'transfer' || !!this.target())
    );
  });

  protected async save(): Promise<void> {
    const amount = parseMoney(this.amount()) ?? 0;
    const anchor = this.anchor();
    const account = this.account();
    if (!anchor || !account) return;
    const signed = this.direction() === 'in' ? amount : -amount;
    const transfer = this.direction() === 'transfer';
    const end = this.end();
    const body = {
      account_id: account,
      target_account_id: transfer ? this.target() : null,
      label: this.label(),
      amount_minor: signed,
      amount_tolerance_minor: parseMoney(this.tolerance() || '0') ?? 0,
      frequency: this.frequency(),
      interval: Number(this.interval()) || 1,
      anchor_date: toIsoDate(anchor),
      end_date: end ? toIsoDate(end) : null,
      split_template: transfer ? [] : [{ category_id: this.category(), amount_minor: signed }],
      active: this.active(),
    };
    try {
      if (this.rule) {
        await firstValueFrom(this.api.updateRecurringRule({ id: this.rule.id, body }));
      } else {
        await firstValueFrom(this.api.createRecurringRule({ body }));
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** E23 — Récurrences et échéancier (TRX-07, R-TRX-40 à R-TRX-43). */
@Component({
  selector: 'app-recurrences',
  imports: [RouterLink, MatButtonModule, MatIconModule, MatTabsModule, MoneyPipe, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Récurrences</h1>
        <span class="spacer"></span>
        <button mat-flat-button (click)="edit(null)"><mat-icon>add</mat-icon> Nouvelle</button>
      </div>
      <mat-tab-group>
        <mat-tab label="Échéancier">
          <div class="tab">
            @for (o of occurrences.value() ?? []; track o.rule_id + o.date) {
              <div class="row" [class.missed]="o.status === 'MISSED'">
                <span class="date">{{ o.date | frDate: 'short' }}</span>
                <span class="label">
                  {{ o.label }}
                  <span class="muted small"
                    >{{ accountName(o.account_id)
                    }}{{
                      o.target_account_id ? ' → ' + accountName(o.target_account_id) : ''
                    }}</span
                  >
                </span>
                <span class="amount" [class.negative]="o.amount_minor < 0">{{
                  o.amount_minor | money
                }}</span>
                <span class="status">
                  @switch (o.status) {
                    @case ('HONORED') {
                      <a
                        mat-icon-button
                        [routerLink]="['/transactions', o.transaction_id]"
                        aria-label="Transaction correspondante"
                        ><mat-icon class="ok">check_circle</mat-icon></a
                      >
                    }
                    @case ('MISSED') {
                      <span class="missed-label">non vue</span>
                      <button mat-button (click)="materialize(o)">Créer</button>
                    }
                    @default {
                      <button mat-button (click)="materialize(o)">Créer</button>
                    }
                  }
                </span>
              </div>
            } @empty {
              <p class="muted">
                Aucune échéance. Une récurrence décrit un prélèvement, un salaire ou un versement
                régulier.
              </p>
            }
          </div>
        </mat-tab>
        <mat-tab label="Récurrences">
          <div class="tab">
            @for (r of rules.value() ?? []; track r.id) {
              <button class="rule" (click)="edit(r)" [class.inactive]="!r.active">
                <span class="label">
                  <strong>{{ r.label }}</strong>
                  <span class="muted small">
                    {{ every(r) }} · {{ accountName(r.account_id)
                    }}{{ r.target_account_id ? ' → ' + accountName(r.target_account_id) : '' }}
                    @if (r.next_date) {
                      · prochaine le {{ r.next_date | frDate: 'short' }}
                    }
                  </span>
                </span>
                @if (r.missed.length > 0) {
                  <span class="badge">{{ r.missed.length }} non vue(s)</span>
                }
                <span class="amount" [class.negative]="r.amount_minor < 0">{{
                  r.amount_minor | money
                }}</span>
              </button>
            } @empty {
              <p class="muted">Aucune récurrence.</p>
            }
          </div>
        </mat-tab>
      </mat-tab-group>
    </div>
  `,
  styles: `
    .page-header {
      display: flex;
      align-items: center;
      .spacer {
        flex: 1;
      }
    }
    .tab {
      padding: 1rem 0;
    }
    .row,
    .rule {
      display: grid;
      grid-template-columns: 5rem 1fr auto auto;
      align-items: center;
      gap: 0.75rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      padding: 0.25rem 0;
    }
    .rule {
      grid-template-columns: 1fr auto auto;
      width: 100%;
      border-left: 0;
      border-right: 0;
      border-bottom: 0;
      background: none;
      color: inherit;
      font: inherit;
      text-align: left;
      padding: 0.5rem 0.25rem;
      cursor: pointer;
      &:hover {
        background: var(--mat-sys-surface-container);
      }
    }
    .label {
      display: flex;
      flex-direction: column;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .status {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      min-width: 6rem;
    }
    .ok {
      color: #2e7d32;
    }
    .missed .date,
    .missed-label {
      color: var(--mat-sys-error);
    }
    .badge {
      font: var(--mat-sys-label-small);
      color: var(--mat-sys-on-error-container);
      background: var(--mat-sys-error-container);
      border-radius: 0.5rem;
      padding: 0.125rem 0.5rem;
    }
    .inactive {
      opacity: 0.55;
    }
    @media (max-width: 599.98px) {
      .row {
        grid-template-columns: 4rem 1fr auto;
        .status {
          grid-column: 2 / -1;
        }
      }
    }
  `,
})
export class RecurrencesPage {
  private readonly api = inject(ProjectionsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly accounts = inject(AccountsStore);

  protected readonly rules = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listRecurringRules(),
  });
  protected readonly occurrences = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listOccurrences({ days: 60 }),
  });

  protected accountName(id: string): string {
    return this.accounts.byId().get(id)?.name ?? '—';
  }

  protected every(rule: RecurringRuleView): string {
    const unit = FREQUENCY_LABELS[rule.frequency];
    return rule.interval > 1
      ? `tous les ${rule.interval} ${unit === 'mois' ? 'mois' : unit + 's'}`
      : `chaque ${unit}`;
  }

  protected async edit(rule: RecurringRuleView | null): Promise<void> {
    const ref = this.dialog.open(RecurrenceDialog, { data: rule });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  /** Crée la transaction en attente de l'échéance (R-TRX-43). */
  protected async materialize(o: OccurrenceView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Créer la transaction ?',
      message: `« ${o.label} » du ${formatDate(o.date)}, en attente, avec la catégorie de la récurrence.`,
      action: 'Créer',
    });
    if (!ok) return;
    try {
      await firstValueFrom(
        this.api.materializeOccurrence({ id: o.rule_id, body: { date: o.date } }),
      );
      this.version.bump();
      this.snack.open('Transaction créée, en attente', undefined, { duration: 3000 });
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }
}
