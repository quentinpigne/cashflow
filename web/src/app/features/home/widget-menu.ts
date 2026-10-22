import { ChangeDetectionStrategy, Component, input } from '@angular/core';
import { MatIconModule } from '@angular/material/icon';
import { MatMenu, MatMenuModule } from '@angular/material/menu';
import { MatTooltipModule } from '@angular/material/tooltip';

/**
 * Réglages d'un encadré de l'accueil (ecrans.md E1) : un bouton discret dans l'en-tête de l'encadré, qui
 * ouvre le menu (`mat-menu`) de l'encadré.
 */
@Component({
  selector: 'app-widget-menu',
  imports: [MatIconModule, MatMenuModule, MatTooltipModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <button
      class="trigger"
      type="button"
      [matMenuTriggerFor]="menu()"
      [attr.aria-label]="'Réglages de l’encadré ' + label()"
      matTooltip="Réglages"
    >
      <mat-icon>tune</mat-icon>
    </button>
  `,
  styles: `
    :host {
      display: flex;
      flex: none;
    }
    // Même allure que la poignée de déplacement voisine.
    .trigger {
      display: flex;
      padding: 0.25rem;
      border: none;
      border-radius: 0.5rem;
      background: transparent;
      color: var(--mat-sys-on-surface-variant);
      cursor: pointer;
      &:hover,
      &[aria-expanded='true'] {
        background: var(--mat-sys-surface-container-highest);
      }
    }
  `,
})
export class WidgetMenu {
  /** Nom de l'encadré, pour les lecteurs d'écran. */
  readonly label = input.required<string>();
  readonly menu = input.required<MatMenu>();
}
