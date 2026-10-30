import { ChangeDetectionStrategy, Component, signal } from '@angular/core';

import { PurchaseCapacity } from '../../api/models/purchase-capacity';
import { LoansTabs } from './credit-shared';
import { PurchaseCapacityPage } from './purchase-capacity';
import { SimulationsPage } from './simulations';

/** E36 — Simuler : la capacité d'achat, puis le simulateur qui en part, puis les simulations (CRE-08). */
@Component({
  selector: 'app-simulate',
  imports: [LoansTabs, PurchaseCapacityPage, SimulationsPage],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header"><h1>Prêts</h1></div>
      <app-loans-tabs />
      <h2 class="part-title">Capacité d'achat</h2>
      <app-purchase-capacity (changed)="capacity.set($event)" />
      <h2 class="part-title">Simulateur</h2>
      <app-simulations [start]="capacity()" />
    </div>
  `,
  styles: `
    .part-title {
      font: var(--mat-sys-headline-small);
      margin: 1.5rem 0 0.75rem;
    }
  `,
})
export class SimulatePage {
  protected readonly capacity = signal<PurchaseCapacity | null>(null);
}
