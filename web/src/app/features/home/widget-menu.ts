import { ChangeDetectionStrategy, Component, inject, input } from '@angular/core';
import { MatDividerModule } from '@angular/material/divider';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatTooltipModule } from '@angular/material/tooltip';

import { WIDGET_LABELS, Widget } from './home-layout';
import { HomeSettings, WidgetOption } from './home-settings';

/** Une entrée cochable du menu de réglages : option à cocher, ou choix exclusif (`radio`). */
export interface WidgetMenuItem {
  label: string;
  checked: boolean;
  select: () => void;
  radio?: boolean;
}

/** Entrée qui bascule une option d'affichage de l'accueil. */
export function optionItem(
  settings: HomeSettings,
  key: WidgetOption,
  label: string,
): WidgetMenuItem {
  return { label, checked: settings.option(key), select: () => settings.toggle(key) };
}

/**
 * Réglages d'un encadré de l'accueil (ecrans.md E1) : un bouton discret à côté de la poignée de
 * l'encadré, qui ouvre un menu avec les options de l'encadré, puis « Masquer l'encadré ».
 */
@Component({
  selector: 'app-widget-menu',
  imports: [MatDividerModule, MatIconModule, MatMenuModule, MatTooltipModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <button
      class="trigger"
      type="button"
      [matMenuTriggerFor]="menu"
      [attr.aria-label]="'Réglages de l’encadré ' + labels[widget()]"
      matTooltip="Réglages"
    >
      <mat-icon>tune</mat-icon>
    </button>
    <mat-menu #menu xPosition="before">
      @for (item of items(); track item.label) {
        <button
          mat-menu-item
          [role]="item.radio ? 'menuitemradio' : 'menuitemcheckbox'"
          [attr.aria-checked]="item.checked"
          (click)="item.select()"
        >
          <mat-icon>{{ item.checked ? 'check' : '' }}</mat-icon>
          <span>{{ item.label }}</span>
        </button>
      }
      @if (items().length > 0) {
        <mat-divider />
      }
      <button mat-menu-item (click)="settings.hide(widget())">
        <mat-icon>visibility_off</mat-icon>
        <span>Masquer l’encadré</span>
      </button>
    </mat-menu>
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
  protected readonly settings = inject(HomeSettings);
  protected readonly labels = WIDGET_LABELS;

  readonly widget = input.required<Widget>();
  /** Options propres à l'encadré, avant « Masquer l'encadré ». */
  readonly items = input<WidgetMenuItem[]>([]);
}
