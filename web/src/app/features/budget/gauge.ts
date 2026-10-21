import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';

import { AlertLevel } from '../../api/models/alert-level';

/**
 * Jauge d'une ligne (R-BUD-61) : neutre sous le premier seuil, orange entre les deux, rouge au-delà ;
 * verte pour un objectif d'épargne atteint, rouge pour un objectif manqué.
 */
@Component({
  selector: 'app-budget-gauge',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div
      class="track"
      role="progressbar"
      [attr.aria-valuenow]="percent()"
      aria-valuemin="0"
      aria-valuemax="100"
      [attr.aria-label]="label()"
    >
      <div class="fill" [class]="alert()" [style.width.%]="width()"></div>
    </div>
  `,
  styles: `
    :host {
      display: block;
    }
    .track {
      height: 0.375rem;
      border-radius: 0.25rem;
      background: var(--mat-sys-surface-container-highest);
      overflow: hidden;
    }
    .fill {
      height: 100%;
      border-radius: 0.25rem;
      background: var(--mat-sys-primary);
    }
    .WARNING {
      background: #e8a33d;
    }
    .OVER,
    .MISSED {
      background: var(--mat-sys-error);
    }
    .REACHED {
      background: #2e7d32;
    }
  `,
})
export class BudgetGauge {
  readonly actual = input.required<number>();
  readonly available = input.required<number>();
  readonly alert = input<AlertLevel>('NONE');
  readonly label = input('Consommation');

  protected readonly percent = computed(() => {
    const available = this.available();
    if (available <= 0) return this.actual() > 0 ? 100 : 0;
    return Math.round((Math.max(0, this.actual()) / available) * 100);
  });
  protected readonly width = computed(() => Math.min(100, this.percent()));
}
