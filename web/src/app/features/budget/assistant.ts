import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { BudgetSuggestion } from '../../api/models/budget-suggestion';
import { BudgetsService } from '../../api/services/budgets.service';
import { DataVersion } from '../../core/data-version';
import { currentMonth, formatMonthKey } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { parseMoney, toEditable } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';

interface Row {
  suggestion: BudgetSuggestion;
  selected: boolean;
  amount: string;
}

/**
 * E19 — Assistant de création du budget (BUD-10) : une proposition par catégorie, d'après la moyenne des
 * derniers mois complets ; on coche, on ajuste, on crée les lignes à partir du mois en cours.
 */
@Component({
  selector: 'app-budget-assistant',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatCheckboxModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/budget" aria-label="Retour au budget">
          <mat-icon>arrow_back</mat-icon>
        </a>
        <h1>Assistant de création</h1>
      </div>
      @if (suggestions.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (suggestions.value(); as s) {
        @if (s.months.length === 0) {
          <div class="empty card">
            <p>Pas encore de mois complet à analyser.</p>
            <p class="muted">
              Importe ou saisis au moins un mois de transactions, puis reviens ici.
            </p>
          </div>
        } @else {
          <p class="muted">
            Moyenne de {{ s.months.length }} mois complet{{ s.months.length > 1 ? 's' : '' }} ({{
              period()
            }}), arrondie à l'euro supérieur. Les lignes sont créées à partir de {{ startLabel }}.
          </p>
          @for (group of groups(); track group.title) {
            <section class="card">
              <h2>{{ group.title }}</h2>
              @for (row of group.rows; track row.suggestion.category_id) {
                <div class="row" [class.disabled]="row.suggestion.has_line">
                  <mat-checkbox
                    [checked]="row.selected"
                    [disabled]="row.suggestion.has_line"
                    (change)="toggle(row, $event.checked)"
                  >
                    @if (row.suggestion.parent_name) {
                      <span class="muted small">{{ row.suggestion.parent_name }} ›</span>
                    }
                    {{ row.suggestion.name }}
                    @if (row.suggestion.has_line) {
                      <span class="muted small">(déjà une ligne)</span>
                    }
                  </mat-checkbox>
                  <span class="history muted small">
                    @for (m of row.suggestion.monthly_minor; track $index) {
                      <span>{{ m | money }}</span>
                    }
                  </span>
                  <mat-form-field class="amount-field">
                    <input
                      matInput
                      inputmode="decimal"
                      autocomplete="off"
                      [attr.aria-label]="'Montant pour ' + row.suggestion.name"
                      [ngModel]="row.amount"
                      (ngModelChange)="setAmount(row, $event)"
                      [disabled]="!row.selected"
                    />
                    <span matTextSuffix>&nbsp;€</span>
                  </mat-form-field>
                </div>
              }
            </section>
          }
          <div class="actions">
            <span class="muted">{{ selectedCount() }} ligne(s) sélectionnée(s)</span>
            <button
              mat-flat-button
              [disabled]="selectedCount() === 0 || saving()"
              (click)="create()"
            >
              Créer le budget
            </button>
          </div>
        }
      }
    </div>
  `,
  styles: `
    .page-header {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .card {
      margin-bottom: 1rem;
    }
    h2 {
      font: var(--mat-sys-title-medium);
      margin: 0 0 0.5rem;
    }
    .row {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto 9rem;
      align-items: center;
      gap: 1rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      padding: 0.5rem 0;
    }
    .disabled {
      opacity: 0.6;
    }
    .history {
      display: flex;
      gap: 0.75rem;
    }
    // Champ de montant de la même taille que partout ailleurs ; seuls les chiffres sont à chasse fixe.
    .amount-field input {
      font-variant-numeric: tabular-nums;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .actions {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 1rem;
    }
    @media (max-width: 599.98px) {
      .row {
        grid-template-columns: minmax(0, 1fr) 8rem;
      }
      .history {
        display: none;
      }
    }
  `,
})
export class BudgetAssistantPage {
  private readonly api = inject(BudgetsService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);
  private readonly snack = inject(MatSnackBar);

  protected readonly startLabel = formatMonthKey(currentMonth()).toLowerCase();
  protected readonly suggestions = rxResource({ stream: () => this.api.budgetSuggestions() });
  protected readonly rows = signal<Row[]>([]);
  protected readonly saving = signal(false);

  constructor() {
    effect(() => {
      const s = this.suggestions.value();
      // Proposées cochées : les dépenses récurrentes et les revenus ; jamais une catégorie déjà budgétée.
      this.rows.set(
        (s?.categories ?? []).map((suggestion) => ({
          suggestion,
          selected: !suggestion.has_line,
          amount: toEditable(suggestion.suggested_minor),
        })),
      );
    });
  }

  protected readonly period = computed(() => {
    const months = this.suggestions.value()?.months ?? [];
    if (months.length === 0) return '';
    const first = formatMonthKey(months[0]).toLowerCase();
    const last = formatMonthKey(months[months.length - 1]).toLowerCase();
    return months.length === 1 ? first : `${first} à ${last}`;
  });

  protected readonly groups = computed(() =>
    [
      { title: 'Revenus', rows: this.rows().filter((r) => r.suggestion.kind === 'INCOME') },
      { title: 'Dépenses', rows: this.rows().filter((r) => r.suggestion.kind === 'EXPENSE') },
    ].filter((g) => g.rows.length > 0),
  );

  protected readonly selectedCount = computed(
    () => this.rows().filter((r) => r.selected && !r.suggestion.has_line).length,
  );

  protected toggle(row: Row, selected: boolean): void {
    this.rows.update((rows) => rows.map((r) => (r === row ? { ...r, selected } : r)));
  }

  protected setAmount(row: Row, amount: string): void {
    this.rows.update((rows) => rows.map((r) => (r === row ? { ...r, amount } : r)));
  }

  protected async create(): Promise<void> {
    const selected = this.rows().filter((r) => r.selected && !r.suggestion.has_line);
    this.saving.set(true);
    let created = 0;
    try {
      for (const row of selected) {
        const amount = parseMoney(row.amount);
        if (amount === null || amount <= 0) continue;
        await firstValueFrom(
          this.api.createBudgetLine({
            body: {
              kind: row.suggestion.kind,
              category_id: row.suggestion.category_id,
              amount_minor: amount,
              from: currentMonth(),
            },
          }),
        );
        created++;
      }
      this.version.bump();
      this.snack.open(`${created} ligne(s) créée(s)`, undefined, { duration: 3000 });
      await this.router.navigate(['/budget']);
    } catch (e) {
      this.version.bump();
      this.snack.open(errorMessage(e), 'Fermer');
    } finally {
      this.saving.set(false);
    }
  }
}
