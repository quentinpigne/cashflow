import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';

import { PayeesService } from '../../api/services/payees.service';
import { DataVersion } from '../../core/data-version';
import { percentOf, sum } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';
import { WidgetMenu } from '../home/widget-menu';

/** Les 5 tiers aux plus fortes dépenses du mois, et leur part (TDB-11, R-TDB-21). */
@Component({
  selector: 'app-home-payees',
  imports: [RouterLink, WidgetMenu, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (rows().length > 0) {
      <section class="card">
        <h2 class="widget-title"><a routerLink="/analyses">Tiers du mois</a></h2>
        <app-widget-menu widget="payees" />
        @for (r of rows(); track r.id) {
          <a class="row" [routerLink]="['/tiers', r.id]">
            <span class="label"
              >{{ r.name }}
              <span class="muted small">{{ r.share }} % des dépenses avec un tiers</span></span
            >
            <span class="amount">{{ r.amount_minor | money }}</span>
          </a>
        }
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
})
export class HomePayees {
  private readonly api = inject(PayeesService);
  private readonly version = inject(DataVersion);

  private readonly report = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.payeeReport({ period: 'MONTH', kind: 'EXPENSE', group_by: 'PAYEE' }),
  });

  protected readonly rows = computed(() => {
    const lines = (this.report.value()?.lines ?? []).filter(
      (l) => l.payee_id && l.amount_minor > 0,
    );
    const total = sum(lines.map((l) => l.amount_minor));
    return [...lines]
      .sort((a, b) => b.amount_minor - a.amount_minor)
      .slice(0, 5)
      .map((l) => ({
        id: l.payee_id as string,
        name: l.name,
        amount_minor: l.amount_minor,
        share: percentOf(l.amount_minor, total),
      }));
  });
}
