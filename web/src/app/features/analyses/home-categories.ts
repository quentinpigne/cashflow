import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';

import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { currentMonth } from '../../core/dates';
import { percentOf } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';
import { WidgetMenu } from '../home/widget-menu';
import { MemberSelections } from '../../shared/member-filter';
import { keepValue } from '../../shared/keep-value';

/** Les 5 catégories aux plus fortes dépenses du mois, avec leur moyenne sur 12 mois (TDB-11, R-TDB-19). */
@Component({
  selector: 'app-home-categories',
  imports: [RouterLink, WidgetMenu, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (rows().length > 0 || members.partial()) {
      <section class="card">
        <h2 class="widget-title">
          <a routerLink="/analyses">Dépenses par catégorie</a>{{ members.titleSuffix() }}
        </h2>
        <app-widget-menu widget="categories" [items]="members.menuItems()" />
        @if (rows().length === 0) {
          <p class="muted">Rien pour ce choix de membres.</p>
        }
        @for (r of rows(); track r.line.category_id ?? 'none') {
          <a class="item" routerLink="/transactions" [queryParams]="r.query">
            <span class="row">
              <span class="label">{{ r.line.name }}</span>
              <span class="amount">{{ r.line.amount_minor | money }}</span>
            </span>
            <span class="track"><span class="bar" [style.width.%]="r.width"></span></span>
            <span class="muted small">Moyenne : {{ r.line.average_minor | money }}</span>
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
      gap: 0.125rem;
      padding: 0.25rem 0;
      color: inherit;
      text-decoration: none;
    }
    .track {
      display: block;
    }
    .bar {
      display: block;
    }
  `,
})
export class HomeCategories {
  private readonly api = inject(DashboardService);
  private readonly version = inject(DataVersion);

  /** Membres cochés dans cet encadré (R-MBR-8, R-MBR-9). */
  protected readonly members = inject(MemberSelections).of('home.categories');
  private readonly report = rxResource({
    params: () => ({ version: this.version.value(), members: this.members.param() }),
    stream: ({ params }) => this.api.categoryReport({ kind: 'EXPENSE', members: params.members }),
  });
  protected readonly reportValue = keepValue(this.report);

  protected readonly rows = computed(() => {
    const month = currentMonth();
    const lines = [...(this.reportValue()?.lines ?? [])]
      // Le non catégorisé a son propre encadré (« À catégoriser »).
      .filter((l) => l.category_id && l.amount_minor > 0)
      .sort((a, b) => b.amount_minor - a.amount_minor)
      .slice(0, 5);
    const top = Math.max(0, ...lines.map((l) => l.amount_minor));
    return lines.map((line) => ({
      line,
      width: percentOf(line.amount_minor, top),
      query: { mois: month, categorie: line.category_id },
    }));
  });
}
