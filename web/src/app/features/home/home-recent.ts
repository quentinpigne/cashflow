import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { RouterLink } from '@angular/router';

import { RecentTransaction } from '../../api/models/recent-transaction';
import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { CategoriesStore } from '../../shared/stores';
import { WidgetMenu } from './widget-menu';

const COUNTS = [5, 10, 15] as const;
type Count = (typeof COUNTS)[number];
const STORAGE_KEY = 'cashflow.home-recent.v1';

function storedCount(): Count {
  try {
    const value = Number(localStorage.getItem(STORAGE_KEY));
    return (COUNTS as readonly number[]).includes(value) ? (value as Count) : 5;
  } catch {
    return 5;
  }
}

/**
 * Dernières saisies sur l'accueil (TDB-01) : les transactions les plus récemment créées, quelle que soit
 * leur date d'opération, pour revenir d'un clic sur celle qu'on vient d'enregistrer. Le nombre affiché
 * (5, 10 ou 15), choisi dans le menu de réglages de l'encadré, est gardé dans le navigateur.
 */
@Component({
  selector: 'app-home-recent',
  imports: [RouterLink, MatIconModule, MatMenuModule, WidgetMenu, FrDatePipe, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card">
      <header>
        <h2>Dernières saisies</h2>
        <app-widget-menu label="Dernières saisies" [menu]="settings" />
      </header>
      @for (t of recent.value() ?? []; track t.id) {
        <a
          class="row"
          [routerLink]="['/transactions', t.id]"
          [class.pending]="t.clearing_status === 'PENDING'"
        >
          <span class="main">
            <span class="muted small">{{ t.date | frDate: 'short' }} · {{ t.account_name }}</span>
            <span class="label">{{ t.label }}</span>
            <span class="muted small">{{ detail(t) }}</span>
          </span>
          <span class="amount" [class.negative]="t.amount_minor < 0">{{
            t.amount_minor | money: t.currency
          }}</span>
        </a>
      } @empty {
        @if (recent.hasValue()) {
          <p class="muted">Aucune transaction pour l'instant.</p>
        }
      }
      <a class="all" routerLink="/transactions">Toutes les transactions</a>
      <mat-menu #settings xPosition="before">
        @for (n of counts; track n) {
          <button
            mat-menu-item
            role="menuitemradio"
            [attr.aria-checked]="count() === n"
            (click)="setCount(n)"
          >
            <mat-icon>{{ count() === n ? 'check' : '' }}</mat-icon>
            <span>{{ n }} transactions</span>
          </button>
        }
      </mat-menu>
    </section>
  `,
  styles: `
    :host {
      display: contents;
    }
    header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem;
      // La poignée de déplacement de l'encadré est en haut à droite.
      margin-right: 2rem;
      margin-bottom: 0.5rem;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1rem;
      padding: 0.375rem 0.25rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      color: inherit;
      text-decoration: none;
      &:hover {
        background: var(--mat-sys-surface-container);
      }
    }
    .main {
      display: flex;
      flex-direction: column;
      min-width: 0;
    }
    .label {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .pending .label {
      font-style: italic;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .all {
      display: inline-block;
      margin-top: 0.5rem;
      font: var(--mat-sys-label-large);
      color: var(--mat-sys-primary);
    }
  `,
})
export class HomeRecent {
  private readonly api = inject(DashboardService);
  private readonly version = inject(DataVersion);
  private readonly categories = inject(CategoriesStore);

  protected readonly counts = COUNTS;
  protected readonly count = signal<Count>(storedCount());
  protected readonly recent = rxResource({
    params: () => ({ limit: this.count(), version: this.version.value() }),
    stream: ({ params }) => this.api.recentTransactions({ limit: params.limit }),
  });

  protected setCount(count: Count): void {
    this.count.set(count);
    try {
      localStorage.setItem(STORAGE_KEY, String(count));
    } catch {
      // Stockage indisponible : le choix vaut pour la session.
    }
  }

  /** Catégorie, ventilation ou virement, sous le libellé. */
  protected detail(t: RecentTransaction): string {
    if (t.counterpart_account_name) {
      return `${t.amount_minor < 0 ? 'Virement vers' : 'Virement depuis'} ${t.counterpart_account_name}`;
    }
    if (t.split_count > 1) {
      return `Ventilée en ${t.split_count}`;
    }
    return this.categories.label(t.category_id);
  }
}
