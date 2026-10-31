import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { of } from 'rxjs';

import { CreditService } from '../../api/services/credit.service';
import { DataVersion } from '../../core/data-version';
import { FeaturesStore } from '../../shared/stores';
import { WidgetMenu } from '../home/widget-menu';

/** Demandes de crédit qui ont des démarches à relancer (TDB-12, R-TDB-25). */
@Component({
  selector: 'app-home-follow-ups',
  imports: [RouterLink, WidgetMenu],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (due().length > 0) {
      <section class="card">
        <h2 class="widget-title"><a routerLink="/prets/demandes">Démarches à relancer</a></h2>
        <app-widget-menu widget="follow-ups" />
        @for (a of due(); track a.id) {
          <a class="row" [routerLink]="['/prets/demandes', a.id]">
            <span class="label">{{ a.name }}</span>
            <span class="badge">{{ a.follow_ups_due }} à relancer</span>
          </a>
        }
      </section>
    }
  `,
  styleUrl: '../home/widget.scss',
})
export class HomeFollowUps {
  private readonly api = inject(CreditService);
  private readonly version = inject(DataVersion);

  private readonly features = inject(FeaturesStore);

  /** Les demandes de crédit font partie de la rubrique Prêts (spec 13). */
  private readonly applications = rxResource({
    params: () => ({ enabled: this.features.loans(), version: this.version.value() }),
    stream: ({ params }) => (params.enabled ? this.api.listLoanApplications() : of([])),
  });

  protected readonly due = computed(() =>
    (this.applications.value() ?? []).filter((a) => a.follow_ups_due > 0),
  );
}
