import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';

import { BudgetsService } from '../../api/services/budgets.service';
import { DataVersion } from '../../core/data-version';
import { MoneyPipe } from '../../shared/money.pipe';
import { HomeSettings } from '../home/home-settings';
import { WidgetMenu, WidgetMenuItem, optionItem } from '../home/widget-menu';
import { BudgetGauge } from './gauge';

/**
 * Budgets sur l'accueil (TDB-06) : reste à budgéter, lignes en alerte, objectifs d'épargne du mois et
 * projets actifs. Rien ne s'affiche tant qu'aucun budget n'existe.
 */
@Component({
  selector: 'app-home-budget',
  imports: [RouterLink, WidgetMenu, MoneyPipe, BudgetGauge],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (budget.value(); as b) {
      @if (b.has_lines || (settings.option('budget.projects') && projects().length > 0)) {
        <section class="card">
          <h2 class="widget-title"><a routerLink="/budget">Budget du mois</a></h2>
          <app-widget-menu widget="budget" [items]="menuItems()" />
          @if (b.has_lines) {
            <div class="row">
              <span>Reste à budgéter</span>
              <span class="amount" [class.negative]="b.to_budget_minor < 0">{{
                b.to_budget_minor | money
              }}</span>
            </div>
          }
          @for (line of alerts(); track line.id) {
            <a class="item" routerLink="/budget">
              <span class="row">
                <span>
                  @if (line.parent_name) {
                    <span class="muted">{{ line.parent_name }} ›</span>
                  }
                  {{ line.name }}
                </span>
                <span class="amount" [class.negative]="line.remaining_minor < 0">
                  {{ line.actual_minor | money }} / {{ line.available_minor | money }}
                </span>
              </span>
              <app-budget-gauge
                [actual]="line.actual_minor"
                [available]="line.available_minor"
                [alert]="line.alert"
                [label]="'Consommation de ' + line.name"
              />
            </a>
          }
          @if (settings.option('budget.goals') && goals().length > 0) {
            <h3>Épargne</h3>
            @for (line of goals(); track line.id) {
              <a class="item" routerLink="/budget">
                <span class="row">
                  <span>→ {{ line.name }}</span>
                  <span class="amount">
                    {{ line.actual_minor | money }} / {{ line.available_minor | money }}
                  </span>
                </span>
                <app-budget-gauge
                  [actual]="line.actual_minor"
                  [available]="line.available_minor"
                  [alert]="line.alert"
                  [label]="'Objectif ' + line.name"
                />
              </a>
            }
          }
          @if (settings.option('budget.projects') && projects().length > 0) {
            <h3>Projets</h3>
            @for (p of projects(); track p.id) {
              <a class="item" [routerLink]="['/budget/projets', p.id]">
                <span class="row">
                  <span>{{ p.name }}</span>
                  <span class="amount">
                    {{ p.actual_minor | money }} / {{ p.amount_minor | money }}
                  </span>
                </span>
                <app-budget-gauge
                  [actual]="p.actual_minor"
                  [available]="p.amount_minor"
                  [alert]="p.alert"
                  [label]="'Consommation de ' + p.name"
                />
              </a>
            }
          }
        </section>
      }
    }
  `,
  styles: `
    :host {
      display: contents;
    }
    h3 {
      font: var(--mat-sys-title-small);
      margin: 1rem 0 0.25rem;
    }
    .row {
      display: flex;
      justify-content: space-between;
      gap: 1rem;
    }
    .item {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      padding: 0.5rem 0;
      color: inherit;
      text-decoration: none;
    }
  `,
})
export class HomeBudget {
  private readonly api = inject(BudgetsService);
  private readonly version = inject(DataVersion);
  protected readonly settings = inject(HomeSettings);

  protected readonly budget = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.monthBudget(),
  });
  private readonly allProjects = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listProjects(),
  });

  protected readonly alerts = computed(() =>
    (this.budget.value()?.lines ?? []).filter(
      (l) => l.kind === 'EXPENSE' && (l.alert === 'WARNING' || l.alert === 'OVER'),
    ),
  );
  protected readonly goals = computed(() =>
    (this.budget.value()?.lines ?? []).filter((l) => l.kind === 'SAVINGS'),
  );
  protected readonly projects = computed(() =>
    (this.allProjects.value() ?? []).filter((p) => p.active),
  );
  protected readonly menuItems = computed<WidgetMenuItem[]>(() => [
    optionItem(this.settings, 'budget.goals', 'Objectifs d’épargne'),
    optionItem(this.settings, 'budget.projects', 'Projets'),
  ]);
}
