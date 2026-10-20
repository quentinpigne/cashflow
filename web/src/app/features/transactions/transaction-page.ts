import { ChangeDetectionStrategy, Component, input } from '@angular/core';

import { TransactionDetailView } from './transaction-detail';

/** E7 en plein écran (mobile, ou lien direct). */
@Component({
  selector: 'app-transaction-page',
  imports: [TransactionDetailView],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `<div class="page"><app-transaction-detail [transactionId]="id()" /></div>`,
})
export class TransactionPage {
  readonly id = input.required<string>();
}
