import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
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
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { firstValueFrom } from 'rxjs';

import { AccountFamily } from '../../api/models/account-family';
import { ValuationView } from '../../api/models/valuation-view';
import { SavingsService } from '../../api/services/savings.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, toIsoDate, todayIso } from '../../core/dates';
import { errorMessage, problemOf } from '../../core/http-errors';
import { formatRatePpm, parseMoney } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';

/** Saisie d'une valorisation (CPT-06) ; pour un prêt, le capital restant dû (R-CPT-12). */
@Component({
  selector: 'app-valuation-dialog',
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
    <h2 mat-dialog-title>{{ data.liability ? 'Capital restant dû' : 'Nouvelle valorisation' }}</h2>
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
        <mat-label>{{ data.liability ? 'Capital restant dû' : 'Valeur du compte' }}</mat-label>
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
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(22rem, 80vw);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class ValuationDialog {
  protected readonly data = inject<{ accountId: string; liability: boolean }>(MAT_DIALOG_DATA);
  private readonly api = inject(SavingsService);
  private readonly dialog = inject(MatDialog);
  private readonly ref = inject(MatDialogRef<ValuationDialog>);

  protected readonly date = signal<Date | null>(fromIsoDate(todayIso()));
  protected readonly value = signal('');
  protected readonly note = signal('');
  protected readonly error = signal<string | null>(null);
  protected readonly valid = computed(() => !!this.date() && parseMoney(this.value()) !== null);

  protected async save(replace = false): Promise<void> {
    const date = this.date();
    const amount = parseMoney(this.value());
    if (!date || amount === null) return;
    try {
      await firstValueFrom(
        this.api.saveValuation({
          id: this.data.accountId,
          body: {
            date: toIsoDate(date),
            // Saisie toujours positive ; un capital restant dû s'enregistre en négatif (R-CPT-12).
            value_minor: this.data.liability ? -Math.abs(amount) : amount,
            note: this.note() || null,
            replace,
          },
        }),
      );
      this.ref.close(true);
    } catch (e) {
      if (problemOf(e)?.code === 'VALUATION_EXISTS' && !replace) {
        const ok = await confirm(this.dialog, {
          title: 'Remplacer la valorisation ?',
          message: 'Une valorisation existe déjà à cette date.',
          action: 'Remplacer',
        });
        if (ok) await this.save(true);
        return;
      }
      this.error.set(errorMessage(e));
    }
  }
}

/**
 * Valorisations d'un compte à rendement variable ou d'un prêt (CPT-06) ; pour l'épargne variable, valeur,
 * capital investi, plus-value et performance (EPA-05, EPA-06).
 */
@Component({
  selector: 'app-valuation-panel',
  imports: [MatButtonModule, MatIconModule, MatTooltipModule, MoneyPipe, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (variable() && performance.value(); as p) {
      <section class="card panel">
        <header><h2>Performance</h2></header>
        <dl class="figures">
          <dt>
            Valeur
            @if (p.estimated) {
              <span class="tag" matTooltip="Dernière valorisation de plus de 90 jours"
                >estimée</span
              >
            }
          </dt>
          <dd class="amount">{{ p.value_minor | money }}</dd>
          <dt>Capital investi</dt>
          <dd class="amount">{{ p.invested_minor | money }}</dd>
          <dt>Plus-value</dt>
          <dd class="amount" [class.negative]="p.gain_minor < 0">
            {{ p.gain_minor | money: 'EUR' : { signed: true } }}
            @if (p.gain_ppm !== null && p.gain_ppm !== undefined) {
              <span class="muted">({{ rate(p.gain_ppm) }})</span>
            }
          </dd>
          <dt>
            TRI depuis l'ouverture
            <mat-icon
              class="info"
              matTooltip="Taux de rendement interne annualisé, qui tient compte des dates des versements."
              >info</mat-icon
            >
          </dt>
          <dd class="amount">{{ rate(p.irr_since_opening_ppm) }}</dd>
          <dt>TRI depuis le 1er janvier</dt>
          <dd class="amount">{{ rate(p.irr_year_to_date_ppm) }}</dd>
          <dt>
            Performance pondérée par le temps
            <mat-icon
              class="info"
              matTooltip="Indépendante des versements (Dietz modifié chaîné entre valorisations) : une approximation."
              >info</mat-icon
            >
          </dt>
          <dd class="amount">{{ rate(p.twr_since_opening_ppm) }}</dd>
        </dl>
        @if (p.years.length > 0) {
          <h3>Par année</h3>
          <table>
            <thead>
              <tr>
                <th>Année</th>
                <th class="right">Plus-value</th>
                <th class="right">TRI</th>
              </tr>
            </thead>
            <tbody>
              @for (y of p.years; track y.year) {
                <tr>
                  <td>{{ y.year }}</td>
                  @if (y.gain_minor !== null && y.gain_minor !== undefined) {
                    <td class="right">{{ y.gain_minor | money: 'EUR' : { signed: true } }}</td>
                    <td class="right">{{ rate(y.irr_ppm) }}</td>
                  } @else {
                    <td colspan="2" class="right muted">données insuffisantes</td>
                  }
                </tr>
              }
            </tbody>
          </table>
        }
      </section>
    }
    <section class="card panel">
      <header>
        <h2>{{ variable() ? 'Valorisations' : 'Capital restant dû' }}</h2>
        <button mat-stroked-button (click)="add()"><mat-icon>add</mat-icon>Ajouter</button>
      </header>
      @for (v of valuations.value() ?? []; track v.id) {
        <div class="row">
          <span>{{ v.date | frDate }}</span>
          <span class="muted small">{{ v.note }}</span>
          <span class="amount" [class.negative]="v.value_minor < 0">{{
            v.value_minor | money
          }}</span>
          <button
            mat-icon-button
            [attr.aria-label]="'Supprimer la valorisation du ' + v.date"
            (click)="remove(v)"
          >
            <mat-icon>delete</mat-icon>
          </button>
        </div>
      } @empty {
        <p class="muted">
          Aucune valorisation : la valeur affichée est le solde d'ouverture plus les versements.
        </p>
      }
    </section>
  `,
  styles: `
    .panel {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    h3 {
      font: var(--mat-sys-title-small);
      margin: 1rem 0 0.25rem;
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
    .tag {
      font: var(--mat-sys-label-small);
      background: var(--mat-sys-secondary-container);
      border-radius: 0.5rem;
      padding: 0 0.375rem;
    }
    .row {
      display: grid;
      grid-template-columns: auto 1fr auto auto;
      align-items: center;
      gap: 0.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .small {
      font: var(--mat-sys-body-small);
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
    }
    th.right,
    td.right {
      text-align: right;
    }
  `,
})
export class ValuationPanel {
  private readonly api = inject(SavingsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  readonly accountId = input.required<string>();
  readonly family = input.required<AccountFamily>();
  protected readonly variable = computed(() => this.family() === 'VARIABLE_SAVINGS');

  protected readonly valuations = rxResource({
    params: () => ({ id: this.accountId(), version: this.version.value() }),
    stream: ({ params }) => this.api.listValuations({ id: params.id }),
  });
  protected readonly performance = rxResource({
    params: () =>
      this.variable() ? { id: this.accountId(), version: this.version.value() } : undefined,
    stream: ({ params }) => this.api.getPerformance({ id: params.id }),
  });

  protected rate(ppm: number | null | undefined): string {
    return ppm === null || ppm === undefined ? '—' : formatRatePpm(ppm);
  }

  protected async add(): Promise<void> {
    const ref = this.dialog.open(ValuationDialog, {
      data: { accountId: this.accountId(), liability: this.family() === 'LIABILITY' },
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async remove(valuation: ValuationView): Promise<void> {
    try {
      await firstValueFrom(
        this.api.deleteValuation({ id: this.accountId(), valuation_id: valuation.id }),
      );
      this.version.bump();
      this.snack.open('Valorisation supprimée', undefined, { duration: 3000 });
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }
}
