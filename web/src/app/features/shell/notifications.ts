import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatBadgeModule } from '@angular/material/badge';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { NotificationView } from '../../api/models/notification-view';
import { BudgetsService } from '../../api/services/budgets.service';
import { DataVersion } from '../../core/data-version';

/**
 * Centre de notifications de l'application (BUD-08, R-BUD-62) : une cloche, un badge pour les non lues.
 * La liste se recharge après chaque modification des données, puisque le serveur évalue les seuils à
 * chaque écriture (R-BUD-63).
 */
@Component({
  selector: 'app-notifications',
  imports: [MatBadgeModule, MatButtonModule, MatIconModule, MatMenuModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <button
      mat-icon-button
      [matMenuTriggerFor]="menu"
      [attr.aria-label]="label()"
      (menuOpened)="opened()"
    >
      <mat-icon
        [matBadge]="unread() || null"
        matBadgeColor="warn"
        matBadgeSize="small"
        aria-hidden="false"
        >notifications</mat-icon
      >
    </button>
    <mat-menu #menu class="notifications-menu" xPosition="before">
      <div class="head" (click)="$event.stopPropagation()" role="presentation">
        <strong>Notifications</strong>
        @if (unread() > 0) {
          <button mat-button (click)="markAll()">Tout marquer comme lu</button>
        }
      </div>
      @for (n of items(); track n.id) {
        <button mat-menu-item class="item" [class.unread]="!n.read" (click)="open(n)">
          <span class="title">{{ n.title }}</span>
          <span class="body">{{ n.body }}</span>
          <span class="date">{{ when(n.created_at) }}</span>
        </button>
      } @empty {
        <p class="none">Aucune notification.</p>
      }
    </mat-menu>
  `,
  styles: `
    :host {
      display: inline-block;
    }
    .head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 1rem;
      padding: 0.5rem 1rem;
    }
    .item {
      height: auto;
      min-height: 3rem;
      padding-top: 0.5rem;
      padding-bottom: 0.5rem;
      line-height: 1.3;
    }
    .item ::ng-deep .mat-mdc-menu-item-text {
      display: flex;
      flex-direction: column;
    }
    .unread .title {
      font-weight: 600;
    }
    .body {
      white-space: pre-line;
      font: var(--mat-sys-body-small);
    }
    .date {
      font: var(--mat-sys-label-small);
      color: var(--mat-sys-on-surface-variant);
    }
    .none {
      padding: 0 1rem 0.5rem;
      color: var(--mat-sys-on-surface-variant);
    }
  `,
})
export class NotificationsButton {
  private readonly api = inject(BudgetsService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);

  private readonly notifications = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listNotifications(),
  });

  protected readonly unread = () => this.notifications.value()?.unread ?? 0;
  protected readonly items = () => this.notifications.value()?.items ?? [];
  protected readonly label = () =>
    this.unread() > 0 ? `Notifications, ${this.unread()} non lue(s)` : 'Notifications';

  protected opened(): void {
    this.notifications.reload();
  }

  protected async markAll(): Promise<void> {
    await firstValueFrom(this.api.markNotificationsRead({ body: {} }));
    this.notifications.reload();
  }

  protected async open(notification: NotificationView): Promise<void> {
    if (!notification.read) {
      await firstValueFrom(this.api.markNotificationsRead({ body: { id: notification.id } }));
      this.notifications.reload();
    }
    if (notification.link) {
      await this.router.navigateByUrl(notification.link);
    }
  }

  protected when(timestamp: string): string {
    return new Date(timestamp).toLocaleString('fr-FR', {
      day: 'numeric',
      month: 'short',
      hour: '2-digit',
      minute: '2-digit',
    });
  }
}
