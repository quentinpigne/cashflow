import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { forkJoin, map, of, switchMap } from 'rxjs';

import { SalariesService } from '../../api/services/salaries.service';
import { DataVersion } from '../../core/data-version';
import { formatMonth } from '../../core/dates';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { FeaturesStore } from '../../shared/stores';
import { WidgetMenu } from '../home/widget-menu';
import { MemberSelections } from '../../shared/member-filter';

/** Dernier mois contrôlé de chaque emploi en cours, et mois sans fiche de paie (TDB-12, R-TDB-26). */
@Component({
  selector: 'app-home-salary',
  imports: [RouterLink, WidgetMenu, FrDatePipe, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (rows().length > 0) {
      <section class="card">
        <h2 class="widget-title"><a routerLink="/salaires">Salaire</a></h2>
        <app-widget-menu widget="salary" />
        @for (r of rows(); track r.id) {
          <a class="item" [routerLink]="['/salaires', r.id]">
            <span class="row">
              <strong>{{ r.employer }}</strong>
              @if (r.check?.flagged) {
                <span class="badge">écart</span>
              }
            </span>
            @if (r.check; as c) {
              <span class="row small">
                <span>{{ c.month | frDate: 'month' }}</span>
                <span class="amount"
                  >{{ c.received_minor | money }} reçus / {{ c.expected_minor | money }}</span
                >
              </span>
            }
            @if (r.missing.length > 0) {
              <span class="muted small">Sans fiche de paie : {{ r.missing.join(', ') }}</span>
            }
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
      padding: 0.25rem 0;
      color: inherit;
      text-decoration: none;
    }
  `,
})
export class HomeSalary {
  private readonly api = inject(SalariesService);
  private readonly version = inject(DataVersion);
  private readonly features = inject(FeaturesStore);

  private readonly employments = rxResource({
    params: () => ({ enabled: this.features.salaries(), version: this.version.value() }),
    stream: ({ params }) =>
      params.enabled
        ? this.api.salariesOverview().pipe(
            map((o) => o.employments.filter((e) => !e.ended)),
            switchMap((list) =>
              list.length === 0
                ? of([])
                : forkJoin(list.map((e) => this.api.getEmployment({ id: e.id }))),
            ),
          )
        : of([]),
  });

  /** Membres cochés sur l'accueil (R-MBR-9). */
  private readonly members = inject(MemberSelections).of('home');
  /** Emplois dont le membre est coché, ceux sans membre avec « Foyer » (R-MBR-9). */
  protected readonly rows = computed(() =>
    (this.employments.value() ?? [])
      .filter((e) => this.members.includes(e.summary.member_id))
      .map((e) => ({
        id: e.summary.id,
        employer: e.summary.employer,
        // Dernier mois contrôlé : ni le mois en cours, ni un mois partiel ou non renseigné (R-SAL-9).
        check:
          [...e.checks]
            .sort((a, b) => b.month.localeCompare(a.month))
            .find((c) => !c.in_progress && !c.partial && !c.unknown) ?? null,
        missing: e.missing_payslip_months.map((m) => formatMonth(m).toLowerCase()),
      })),
  );
}
