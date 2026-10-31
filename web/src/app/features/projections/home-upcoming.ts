import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { of } from 'rxjs';

import { LoansService } from '../../api/services/loans.service';
import { ProjectionsService } from '../../api/services/projections.service';
import { DataVersion } from '../../core/data-version';
import { todayIso } from '../../core/dates';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore, FeaturesStore } from '../../shared/stores';
import { upcomingItems } from '../home/widget-rules';
import { WidgetMenu } from '../home/widget-menu';

/** Récurrences et mensualités de prêts des 15 prochains jours (TDB-10, R-TDB-16). */
@Component({
  selector: 'app-home-upcoming',
  imports: [RouterLink, WidgetMenu, FrDatePipe, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (items().length > 0) {
      <section class="card">
        <h2 class="widget-title"><a routerLink="/recurrences">Prochaines échéances</a></h2>
        <app-widget-menu widget="upcoming" />
        @for (item of items(); track item.date + item.label + item.source) {
          <a class="row" [routerLink]="item.link">
            <span class="label"
              >{{ item.label }}
              <span class="muted small"
                >{{ item.date | frDate: 'short' }} · {{ item.source }}</span
              ></span
            >
            <span class="amount" [class.negative]="item.amount_minor < 0">{{
              item.amount_minor | money
            }}</span>
          </a>
        }
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
})
export class HomeUpcoming {
  private readonly projections = inject(ProjectionsService);
  private readonly loansApi = inject(LoansService);
  private readonly version = inject(DataVersion);
  private readonly accounts = inject(AccountsStore);
  private readonly features = inject(FeaturesStore);

  private readonly occurrences = rxResource({
    params: () => this.version.value(),
    stream: () => this.projections.listOccurrences({ days: 15 }),
  });
  private readonly loans = rxResource({
    params: () => ({ enabled: this.features.loans(), version: this.version.value() }),
    stream: ({ params }) => (params.enabled ? this.loansApi.listLoans() : of([])),
  });

  protected readonly items = computed(() => {
    const names = new Map(this.accounts.list().map((a) => [a.id, a.name]));
    return upcomingItems(
      this.occurrences.value() ?? [],
      this.loans.value() ?? [],
      (id) => names.get(id) ?? '',
      todayIso(),
    );
  });
}
