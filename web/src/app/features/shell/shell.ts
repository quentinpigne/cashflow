import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatListModule } from '@angular/material/list';
import { MatSidenavModule } from '@angular/material/sidenav';
import { MatToolbarModule } from '@angular/material/toolbar';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

import { Session } from '../../core/auth';
import { Layout } from '../../core/layout';
import { QuickEntry } from '../transactions/quick-entry';

interface NavItem {
  path: string;
  label: string;
  icon: string;
  exact: boolean;
}

/** Navigation (ecrans.md) : barre du bas et « + » sur mobile, menu latéral permanent sur ordinateur. */
@Component({
  selector: 'app-shell',
  imports: [
    RouterOutlet,
    RouterLink,
    RouterLinkActive,
    MatSidenavModule,
    MatListModule,
    MatToolbarModule,
    MatIconModule,
    MatButtonModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './shell.html',
  styleUrl: './shell.scss',
})
export class Shell {
  protected readonly layout = inject(Layout);
  private readonly quickEntry = inject(QuickEntry);
  private readonly session = inject(Session);
  private readonly router = inject(Router);

  /** Menu latéral (ordinateur). */
  protected readonly items: NavItem[] = [
    { path: '/', label: 'Accueil', icon: 'home', exact: true },
    { path: '/comptes', label: 'Comptes', icon: 'account_balance', exact: false },
    { path: '/transactions', label: 'Transactions', icon: 'receipt_long', exact: true },
    { path: '/import', label: 'Import', icon: 'upload_file', exact: false },
    { path: '/analyses', label: 'Analyses', icon: 'insights', exact: false },
    { path: '/reglages', label: 'Réglages', icon: 'settings', exact: false },
  ];

  /** Barre du bas (mobile) : l'import s'ouvre depuis un compte ou les réglages (ecrans.md). */
  protected readonly mobileItems = this.items.filter((i) => i.path !== '/import');

  protected newTransaction(): void {
    // Sur la fiche d'un compte, ce compte est présélectionné (ecrans.md E3).
    const match = /^\/comptes\/([0-9a-f-]{36})/.exec(this.router.url);
    this.quickEntry.open({ accountId: match?.[1] });
  }

  protected async logout(): Promise<void> {
    await this.session.logout();
    await this.router.navigate(['/connexion']);
  }
}
