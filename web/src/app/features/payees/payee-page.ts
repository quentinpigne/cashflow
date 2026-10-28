import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Router, RouterLink } from '@angular/router';
import type { EChartsCoreOption } from 'echarts/core';
import { NgxEchartsDirective } from 'ngx-echarts';

import { PayeesService } from '../../api/services/payees.service';
import { TransactionsService } from '../../api/services/transactions.service';
import { DataVersion } from '../../core/data-version';
import { formatChartValue, toChartValue } from '../../core/money';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { CategoriesStore, PayeesStore } from '../../shared/stores';
import { formatAddress, mapLink } from './address';
import { PayeeActions } from './payee-actions';

/** E32 — Fiche d'un tiers ou d'un groupe de tiers (TIE-04). */
@Component({
  selector: 'app-payee-page',
  imports: [
    RouterLink,
    MatButtonModule,
    MatButtonToggleModule,
    MatIconModule,
    MatMenuModule,
    MatProgressBarModule,
    NgxEchartsDirective,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      @if (summary.value(); as s) {
        <div class="page-header">
          <a mat-icon-button routerLink="/reglages/tiers" aria-label="Retour aux tiers"
            ><mat-icon>arrow_back</mat-icon></a
          >
          <h1>{{ s.name }}</h1>
          @if (s.archived) {
            <span class="tag">archivé</span>
          }
          <button mat-icon-button [matMenuTriggerFor]="menu" aria-label="Plus d'actions">
            <mat-icon>more_vert</mat-icon>
          </button>
          <mat-menu #menu>
            @if (payee(); as p) {
              <button mat-menu-item (click)="actions.editPayee(p)">
                <mat-icon>edit</mat-icon>Modifier (nom, groupe, adresse)
              </button>
              <button mat-menu-item (click)="actions.setArchived(p, !p.archived)">
                <mat-icon>{{ p.archived ? 'unarchive' : 'archive' }}</mat-icon
                >{{ p.archived ? 'Désarchiver' : 'Archiver' }}
              </button>
              <button mat-menu-item (click)="merge()">
                <mat-icon>merge</mat-icon>Fusionner dans…
              </button>
              <button mat-menu-item (click)="remove()" [disabled]="p.in_use">
                <mat-icon>delete</mat-icon>Supprimer
              </button>
            }
            @if (group(); as g) {
              <button mat-menu-item (click)="actions.createPayee(g.id)">
                <mat-icon>add</mat-icon>Ajouter un tiers
              </button>
              <button mat-menu-item (click)="actions.editGroup(g)">
                <mat-icon>edit</mat-icon>Renommer
              </button>
              <button mat-menu-item (click)="removeGroup()">
                <mat-icon>delete</mat-icon>Supprimer
              </button>
            }
          </mat-menu>
        </div>
        <p class="muted subtitle">
          @if (s.group_id && s.group_name) {
            Groupe
            <a [routerLink]="['/tiers/groupe', s.group_id]">{{ s.group_name }}</a>
          } @else if (kind() === 'group') {
            Groupe de {{ s.members.length }} tiers :
            @for (m of s.members; track m.id; let last = $last) {
              <a [routerLink]="['/tiers', m.id]">{{ m.name }}</a
              >{{ last ? '' : ', ' }}
            }
          } @else {
            Sans groupe
          }
          @if (s.note) {
            · {{ s.note }}
          }
        </p>
        @if (s.address; as a) {
          <p class="muted subtitle address">
            <mat-icon inline>{{ mapLink(a) ? 'location_on' : 'location_off' }}</mat-icon>
            {{ formatAddress(a) }}
            @if (a.department_name) {
              · {{ a.department_name }} ({{ a.department_code }})
            }
            @if (mapLink(a); as link) {
              · <a [href]="link" target="_blank" rel="noopener noreferrer">Voir sur une carte</a>
            }
          </p>
        }

        <div class="figures">
          @for (t of [s.last_12_months, s.year]; track $index) {
            <section class="card figure">
              <h2>{{ $index === 0 ? '12 derniers mois' : 'Depuis le 1er janvier' }}</h2>
              <p class="big">{{ t.expenses_minor | money }}</p>
              <p class="muted">de dépenses, sur {{ t.transactions }} transaction(s)</p>
              @if (t.income_minor) {
                <p>{{ t.income_minor | money }} de revenus</p>
              }
              @if (t.uncategorized_out_minor || t.uncategorized_in_minor) {
                <p class="warn">
                  À catégoriser :
                  @if (t.uncategorized_out_minor) {
                    {{ t.uncategorized_out_minor | money }} de sorties
                  }
                  @if (t.uncategorized_in_minor) {
                    {{ t.uncategorized_in_minor | money }} d'entrées
                  }
                </p>
              }
              @if (t.average_expense_minor !== null && t.average_expense_minor !== undefined) {
                <p class="muted">Dépense moyenne : {{ t.average_expense_minor | money }}</p>
              }
            </section>
          }
        </div>

        <section class="card">
          <div class="section-head">
            <h2>Évolution des dépenses</h2>
            <mat-button-toggle-group
              [value]="months()"
              (change)="months.set($event.value)"
              aria-label="Durée"
            >
              <mat-button-toggle [value]="12">12 mois</mat-button-toggle>
              <mat-button-toggle [value]="24">24 mois</mat-button-toggle>
            </mat-button-toggle-group>
          </div>
          @if (chart(); as options) {
            <div echarts [options]="options" class="chart"></div>
          } @else {
            <p class="muted">Aucune dépense sur la période.</p>
          }
        </section>

        <section class="card">
          <h2>Par catégorie, sur 12 mois</h2>
          @for (c of s.categories; track c.category_id ?? 'none') {
            <div class="line">
              <span [class.muted]="!c.category_id">{{ c.name }}</span>
              <span class="amount">{{ c.amount_minor | money }}</span>
            </div>
          } @empty {
            <p class="muted">Aucune dépense sur 12 mois.</p>
          }
        </section>

        <section class="card">
          <div class="section-head">
            <h2>Dernières transactions</h2>
            <a mat-button routerLink="/transactions" [queryParams]="filter()">Tout voir</a>
          </div>
          @for (t of transactions.value()?.items ?? []; track t.id) {
            <button class="line link" (click)="open(t.id)">
              <span class="text">
                <span>{{ t.label }}</span>
                <span class="muted small"
                  >{{ t.date | frDate: 'short' }} · {{ t.account_name }}
                  @if (kind() === 'group') {
                    · {{ store.name(t.payee_id) }}
                  }
                  · {{ categoryOf(t.splits) }}</span
                >
              </span>
              <span class="amount" [class.negative]="t.amount_minor < 0">{{
                t.amount_minor | money
              }}</span>
            </button>
          } @empty {
            <p class="muted">Aucune transaction.</p>
          }
        </section>
      } @else if (summary.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      } @else if (summary.error()) {
        <p class="empty">
          Ce tiers n'existe plus. <a routerLink="/reglages/tiers">Revenir à la liste</a>
        </p>
      }
    </div>
  `,
  styles: `
    .subtitle a {
      color: var(--mat-sys-primary);
    }

    .figures {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
      gap: 1rem;
      margin-bottom: 1rem;
    }

    .figure p {
      margin: 0.25rem 0;
    }

    .big {
      font: var(--mat-sys-headline-small);
    }

    .warn {
      color: var(--mat-sys-error);
    }

    section.card {
      margin-bottom: 1rem;
    }

    h2 {
      font: var(--mat-sys-title-medium);
      margin: 0 0 0.5rem;
    }

    .section-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem;
      flex-wrap: wrap;
    }

    .chart {
      width: 100%;
      height: 280px;
    }

    .line {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 0.5rem;
      width: 100%;
      padding: 0.375rem 0;
      border-bottom: 1px solid var(--mat-sys-outline-variant);
    }

    .link {
      border: none;
      border-bottom: 1px solid var(--mat-sys-outline-variant);
      background: none;
      font: inherit;
      color: inherit;
      text-align: left;
      cursor: pointer;
    }

    .text {
      display: flex;
      flex-direction: column;
    }

    .small {
      font: var(--mat-sys-body-small);
    }

    .tag {
      font: var(--mat-sys-label-small);
      color: var(--mat-sys-on-secondary-container);
      background: var(--mat-sys-secondary-container);
      border-radius: 0.5rem;
      padding: 0.125rem 0.5rem;
    }
  `,
})
export class PayeePage {
  private readonly api = inject(PayeesService);
  private readonly transactionsApi = inject(TransactionsService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);
  private readonly categories = inject(CategoriesStore);
  protected readonly store = inject(PayeesStore);
  protected readonly formatAddress = formatAddress;
  protected readonly mapLink = mapLink;
  protected readonly actions = inject(PayeeActions);

  readonly id = input.required<string>();
  /** Donnée de la route : fiche d'un tiers ou d'un groupe. */
  readonly kind = input<'payee' | 'group'>('payee');

  protected readonly months = signal<12 | 24>(12);

  protected readonly summary = rxResource({
    params: () => ({ id: this.id(), kind: this.kind(), version: this.version.value() }),
    stream: ({ params }) =>
      params.kind === 'group'
        ? this.api.payeeGroupSummary({ id: params.id })
        : this.api.payeeSummary({ id: params.id }),
  });

  /** Filtre de la liste des transactions (E14), par tiers ou par groupe. */
  protected readonly filter = computed<{ tiers?: string; groupe?: string }>(() =>
    this.kind() === 'group' ? { groupe: this.id() } : { tiers: this.id() },
  );

  protected readonly transactions = rxResource({
    params: () => ({ ...this.filter(), version: this.version.value() }),
    stream: ({ params }) =>
      this.transactionsApi.listAllTransactions({
        payee_id: params.tiers,
        payee_group_id: params.groupe,
        limit: 10,
      }),
  });

  protected readonly payee = computed(() =>
    this.kind() === 'payee' ? this.store.byId().get(this.id()) : undefined,
  );
  protected readonly group = computed(() =>
    this.kind() === 'group' ? this.store.groupById().get(this.id()) : undefined,
  );

  /** Barres mensuelles des dépenses ; empilées par tiers pour un groupe. */
  protected readonly chart = computed<EChartsCoreOption | null>(() => {
    const all = this.summary.value()?.months ?? [];
    const months = all.slice(-this.months());
    if (months.every((m) => m.expenses_minor === 0)) return null;
    // « 11/25 » : court et lisible sur 24 mois.
    const labels = months.map((m) => `${m.month.slice(5)}/${m.month.slice(2, 4)}`);
    const series =
      this.kind() === 'group'
        ? [...new Set(months.flatMap((m) => m.by_payee.map((p) => p.payee_id)))].map((id) => ({
            name: months.flatMap((m) => m.by_payee).find((p) => p.payee_id === id)?.name ?? '',
            type: 'bar',
            stack: 'total',
            data: months.map((m) =>
              toChartValue(m.by_payee.find((p) => p.payee_id === id)?.expenses_minor ?? 0),
            ),
          }))
        : [
            {
              name: 'Dépenses',
              type: 'bar',
              data: months.map((m) => toChartValue(m.expenses_minor)),
            },
          ];
    return {
      grid: {
        left: 8,
        right: 8,
        top: this.kind() === 'group' ? 32 : 8,
        bottom: 8,
        containLabel: true,
      },
      legend: this.kind() === 'group' ? { top: 0, type: 'scroll' } : undefined,
      tooltip: {
        trigger: 'axis',
        axisPointer: { type: 'shadow' },
        valueFormatter: (v: number) => formatChartValue(v),
      },
      xAxis: { type: 'category', data: labels },
      yAxis: {
        type: 'value',
        axisLabel: {
          formatter: (v: number) => formatChartValue(v, 'EUR', { bare: true }).replace(/,00$/, ''),
        },
      },
      series,
    };
  });

  protected categoryOf(splits: { category_id?: string | null }[]): string {
    return splits.length > 1 ? 'Ventilée' : this.categories.label(splits[0]?.category_id);
  }

  protected open(id: string): void {
    void this.router.navigate(['/transactions', id]);
  }

  protected async merge(): Promise<void> {
    const p = this.payee();
    const target = p ? await this.actions.merge(p) : null;
    if (target) await this.router.navigate(['/tiers', target]);
  }

  protected async remove(): Promise<void> {
    const p = this.payee();
    if (p && (await this.actions.remove(p))) await this.router.navigate(['/reglages/tiers']);
  }

  protected async removeGroup(): Promise<void> {
    const g = this.group();
    if (g && (await this.actions.removeGroup(g))) await this.router.navigate(['/reglages/tiers']);
  }
}
