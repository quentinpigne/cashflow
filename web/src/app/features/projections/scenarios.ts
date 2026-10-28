import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ScenarioView } from '../../api/models/scenario-view';
import { ProjectionsService } from '../../api/services/projections.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { KIND_LABELS } from '../../core/labels';
import { formatRate } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { AccountsStore } from '../../shared/stores';
import { ScenarioDialog } from './scenario-dialog';

/** E33 — Scénarios de projection (PRJ-03), dans les réglages. */
@Component({
  selector: 'app-scenarios',
  imports: [RouterLink, MatButtonModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages">
          <mat-icon>arrow_back</mat-icon>
        </a>
        <h1>Scénarios de projection</h1>
        <button mat-flat-button (click)="edit(null)"><mat-icon>add</mat-icon>Nouveau</button>
      </div>
      <p class="muted">
        Hypothèses utilisées par les projections : ce ne sont pas des prévisions, les rendements
        futurs sont incertains.
      </p>
      @for (s of scenarios.value() ?? []; track s.id) {
        <section class="card">
          <header>
            <h2>
              {{ s.name }}
              @if (s.is_default) {
                <span class="tag">par défaut</span>
              }
            </h2>
            <button mat-button (click)="edit(s)"><mat-icon>edit</mat-icon>Modifier</button>
            @if (!s.is_default) {
              <button mat-icon-button [attr.aria-label]="'Supprimer ' + s.name" (click)="remove(s)">
                <mat-icon>delete</mat-icon>
              </button>
            }
          </header>
          <p class="muted small">
            Horizon {{ s.horizon_months / 12 }} ans
            @if (s.inflation_rate_ppm) {
              · inflation {{ rate(s.inflation_rate_ppm) }}
            }
            <br />{{ capacityLine(s) }}
          </p>
          <ul class="yields">
            @for (y of kindYields(s); track y.label) {
              <li>
                <span>{{ y.label }}</span
                ><span class="amount">{{ y.rate }}</span>
              </li>
            }
          </ul>
        </section>
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      h2 {
        flex: 1;
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .tag {
      font: var(--mat-sys-label-small);
      background: var(--mat-sys-secondary-container);
      border-radius: 0.5rem;
      padding: 0.125rem 0.5rem;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .yields {
      list-style: none;
      padding: 0;
      margin: 0;
      li {
        display: flex;
        justify-content: space-between;
        border-top: 1px solid var(--mat-sys-outline-variant);
        padding: 0.25rem 0;
      }
    }
  `,
})
export class ScenariosPage {
  private readonly api = inject(ProjectionsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly accounts = inject(AccountsStore);

  protected readonly scenarios = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listScenarios(),
  });

  protected kindYields(s: ScenarioView): { label: string; rate: string }[] {
    return Object.entries(s.assumptions.by_kind ?? {}).map(([kind, y]) => ({
      label: KIND_LABELS[kind as keyof typeof KIND_LABELS] ?? kind,
      rate:
        formatRate(y.annual_return_ppm) +
        (y.annual_fees_ppm ? ` (frais ${formatRate(y.annual_fees_ppm)})` : ''),
    }));
  }

  /** Hypothèses de capacité d'épargne d'un scénario, en une ligne (R-PRJ-3). */
  protected capacityLine(s: ScenarioView): string {
    const c = s.assumptions.capacity;
    if (c?.enabled === false) return "Capacité d'épargne désactivée";
    const surplus = c?.surplus_account_id
      ? ` · reste vers ${this.accounts.byId().get(c.surplus_account_id)?.name ?? '—'}`
      : '';
    return (
      `Salaires +${formatRate(c?.salary_growth_ppm ?? 20_000)}/an · ` +
      `dépenses +${formatRate(c?.expense_growth_ppm ?? 20_000)}/an${surplus}`
    );
  }

  protected rate(ppm: number): string {
    return formatRate(ppm);
  }

  protected async edit(scenario: ScenarioView | null): Promise<void> {
    const ref = this.dialog.open(ScenarioDialog, { data: scenario, maxWidth: '44rem' });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async remove(scenario: ScenarioView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${scenario.name} » ?`,
      message: 'Ses hypothèses seront perdues.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteScenario({ id: scenario.id }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }
}
