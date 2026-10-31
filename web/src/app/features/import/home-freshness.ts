import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';

import { ImportsService } from '../../api/services/imports.service';
import { ReconciliationService } from '../../api/services/reconciliation.service';
import { DataVersion } from '../../core/data-version';
import { todayIso } from '../../core/dates';
import { FrDatePipe } from '../../shared/date.pipe';
import { AccountsStore } from '../../shared/stores';
import { freshness } from '../home/widget-rules';
import { WidgetMenu } from '../home/widget-menu';

/** Dernier relevé importé et dernier rapprochement de chaque compte importé (TDB-12, R-TDB-23). */
@Component({
  selector: 'app-home-freshness',
  imports: [RouterLink, WidgetMenu, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (rows().length > 0) {
      <section class="card">
        <h2 class="widget-title"><a routerLink="/import">Comptes à mettre à jour</a></h2>
        <app-widget-menu widget="freshness" />
        @for (r of rows(); track r.account_id) {
          <a class="row" [routerLink]="['/comptes', r.account_id]">
            <span class="label"
              >{{ r.account_name }}
              <span class="muted small"
                >Relevé au {{ r.last_statement | frDate: 'short' }}
                @if (r.last_reconciled) {
                  · rapproché au {{ r.last_reconciled | frDate: 'short' }}
                } @else {
                  · jamais rapproché
                }
              </span></span
            >
            @if (r.stale) {
              <span class="badge">à mettre à jour</span>
            }
          </a>
        }
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
})
export class HomeFreshness {
  private readonly imports = inject(ImportsService);
  private readonly reconciliation = inject(ReconciliationService);
  private readonly version = inject(DataVersion);
  private readonly accounts = inject(AccountsStore);

  private readonly batches = rxResource({
    params: () => this.version.value(),
    stream: () => this.imports.listImports(),
  });
  private readonly health = rxResource({
    params: () => this.version.value(),
    stream: () => this.reconciliation.accountsHealth(),
  });

  protected readonly rows = computed(() =>
    freshness(
      this.batches.value() ?? [],
      this.health.value() ?? [],
      this.accounts.list(),
      todayIso(),
    ),
  );
}
