import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatIconModule } from '@angular/material/icon';
import { RouterLink } from '@angular/router';

import { ProjectionsService } from '../../api/services/projections.service';
import { DataVersion } from '../../core/data-version';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { WidgetMenu } from '../home/widget-menu';

/** Échéances passées non honorées, signalées sur l'accueil (R-TRX-42). */
@Component({
  selector: 'app-home-missed',
  imports: [RouterLink, MatIconModule, WidgetMenu, FrDatePipe, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (missed().length > 0) {
      <section class="card">
        <h2 class="widget-title"><a routerLink="/recurrences">Échéances non vues</a></h2>
        <app-widget-menu widget="missed" />
        @for (o of missed(); track o.rule_id + o.date) {
          <a class="row" routerLink="/recurrences">
            <mat-icon>event_busy</mat-icon>
            <span
              >{{ o.label }}
              <span class="muted small">attendue le {{ o.date | frDate: 'short' }}</span></span
            >
            <span class="amount" [class.negative]="o.amount_minor < 0">{{
              o.amount_minor | money
            }}</span>
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
        color: var(--mat-sys-error);
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class HomeMissed {
  private readonly api = inject(ProjectionsService);
  private readonly version = inject(DataVersion);

  private readonly occurrences = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listOccurrences({ days: 0 }),
  });
  protected readonly missed = computed(() =>
    (this.occurrences.value() ?? []).filter((o) => o.status === 'MISSED'),
  );
}
