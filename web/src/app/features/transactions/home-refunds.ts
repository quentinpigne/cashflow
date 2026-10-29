import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatIconModule } from '@angular/material/icon';
import { RouterLink } from '@angular/router';

import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { WidgetMenu } from '../home/widget-menu';

/** Remboursements attendus, pas encore entièrement reçus, sur l'accueil (TRX-12, R-TRX-53). */
@Component({
  selector: 'app-home-refunds',
  imports: [RouterLink, MatIconModule, WidgetMenu, FrDatePipe, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (pending.value()?.length) {
      <section class="card refunds">
        <h2 class="widget-title">Remboursements attendus</h2>
        <app-widget-menu widget="refunds" />
        @for (p of pending.value(); track p.transaction_id) {
          <a class="row" [routerLink]="['/transactions', p.transaction_id]">
            <mat-icon>hourglass_top</mat-icon>
            <span
              >{{ p.label }}
              <span class="muted small"
                >{{ p.date | frDate: 'short' }} · {{ p.amount_minor | money }}</span
              ></span
            >
            <span class="amount"
              >{{ p.pending_minor | money }}
              @if (p.refunded_minor > 0) {
                <span class="muted small">sur {{ p.expected_minor | money }}</span>
              }
            </span>
          </a>
        }
      </section>
    }
  `,
  styles: `
    :host {
      display: contents;
    }
    .row {
      display: grid;
      grid-template-columns: auto 1fr auto;
      align-items: center;
      gap: 0.5rem;
      padding: 0.25rem 0;
      color: inherit;
      text-decoration: none;
      mat-icon {
        color: var(--mat-sys-tertiary);
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class HomeRefunds {
  private readonly api = inject(TransactionsService);
  private readonly version = inject(DataVersion);

  protected readonly pending = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.pendingRefunds(),
  });
}
