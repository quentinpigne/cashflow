import { ChangeDetectionStrategy, Component, input } from '@angular/core';
import { MatIconModule } from '@angular/material/icon';
import { MatMenu, MatMenuModule } from '@angular/material/menu';
import { MatTooltipModule } from '@angular/material/tooltip';

/**
 * Réglages d'un encadré de l'accueil (ecrans.md E1) : un bouton discret à côté de la poignée de l'encadré,
 * qui ouvre le menu (`mat-menu`) de l'encadré.
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
    // À gauche de la poignée de déplacement (home.scss, .handle), avec la même taille et la même
    // discrétion : placé par rapport à l'encadré, pas dans le flux de son en-tête.
    :host {
      position: absolute;
      top: 0.375rem;
      right: 2.5rem;
      z-index: 1;
      display: flex;
    }
    .trigger {
      display: flex;
      padding: 0.375rem;
      border: none;
      border-radius: 0.5rem;
      background: transparent;
      color: var(--mat-sys-on-surface-variant);
      cursor: pointer;
      opacity: 0.35;
      mat-icon {
        font-size: 1.25rem;
        width: 1.25rem;
        height: 1.25rem;
      }
      &:hover,
      &[aria-expanded='true'] {
        background: var(--mat-sys-surface-container-highest);
      }
    }
    :host-context(.widget:hover) .trigger,
    .trigger:focus-visible,
    .trigger[aria-expanded='true'] {
      opacity: 1;
    }
    // Sur écran tactile, le bouton reste bien visible.
    @media (hover: none) {
      .trigger {
        opacity: 0.7;
      }
    }
  `,
})
export class WidgetMenu {
  /** Nom de l'encadré, pour les lecteurs d'écran. */
  readonly label = input.required<string>();
  readonly menu = input.required<MatMenu>();
}
