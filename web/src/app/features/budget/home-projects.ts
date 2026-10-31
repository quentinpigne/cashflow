import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { RouterLink } from '@angular/router';

import { currentMonth, formatMonthKey } from '../../core/dates';
import { PlannedProjects } from '../../core/planned-projects';
import { MoneyPipe } from '../../shared/money.pipe';
import { ProjectsStore } from '../../shared/stores';
import { nextPlannedSpend, provisionOf } from '../home/widget-rules';
import { WidgetMenu, WidgetMenuItem } from '../home/widget-menu';
import { BudgetGauge } from './gauge';

/** Projets en cours et planifiés : réel sur prévu, mise de côté, prochaine dépense (TDB-10, R-TDB-18). */
@Component({
  selector: 'app-home-projects',
  imports: [RouterLink, WidgetMenu, MoneyPipe, BudgetGauge],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (rows().length > 0) {
      <section class="card">
        <h2 class="widget-title"><a routerLink="/budget/projets">Projets</a></h2>
        <app-widget-menu widget="projects" [items]="menuItems()" />
        @for (r of rows(); track r.project.id) {
          <a class="item" [routerLink]="['/budget/projets', r.project.id]">
            <span class="row">
              <span class="label"
                ><span
                  >{{ r.project.name }}
                  @if (r.project.status === 'PLANNED') {
                    <span class="tag">planifié</span>
                  }
                </span>
                @if (r.provision !== null) {
                  <span class="muted small">Mise de côté ce mois : {{ r.provision | money }}</span>
                }
                @if (r.next; as n) {
                  <span class="muted small"
                    >Prochaine dépense : {{ monthLabel(n.month).toLowerCase() }} ·
                    {{ n.amount_minor | money }}</span
                  >
                }
              </span>
              <span class="amount small"
                >{{ r.project.actual_minor | money }} / {{ r.project.amount_minor | money }}</span
              >
            </span>
            <app-budget-gauge
              [actual]="r.project.actual_minor"
              [available]="r.project.amount_minor"
              [alert]="r.project.alert"
              [label]="'Consommation de ' + r.project.name"
            />
          </a>
        }
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
  styles: `
    .item {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      padding: 0.375rem 0;
      color: inherit;
      text-decoration: none;
    }
  `,
})
export class HomeProjects {
  private readonly projects = inject(ProjectsStore);
  protected readonly plannedProjects = inject(PlannedProjects);
  protected readonly monthLabel = formatMonthKey;

  protected readonly rows = computed(() => {
    const month = currentMonth();
    const planned = this.plannedProjects.value();
    return this.projects
      .list()
      .filter((p) => p.status === 'ACTIVE' || (planned && p.status === 'PLANNED'))
      .map((project) => ({
        project,
        provision: provisionOf(project, month),
        next: nextPlannedSpend(project, month),
      }));
  });

  /** L'interrupteur « Projets planifiés », partagé avec le budget et l'évolution (R-BUD-72). */
  protected readonly menuItems = computed<WidgetMenuItem[]>(() => [
    {
      label: 'Projets planifiés',
      checked: this.plannedProjects.value(),
      select: () => this.plannedProjects.set(!this.plannedProjects.value()),
    },
  ]);
}
