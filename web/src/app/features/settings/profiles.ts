import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { Profile } from '../../api/models/profile';
import { ImportsService } from '../../api/services/imports.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { AccountsStore } from '../../shared/stores';
import { CategoryNameDialog, NameData } from './category-dialogs';

/** E17 — Profils d'import (IMP-02). */
@Component({
  selector: 'app-profiles',
  imports: [RouterLink, MatButtonModule, MatIconModule, MatMenuModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Profils d'import</h1>
      </div>
      <p class="muted">
        Un profil décrit comment lire les fichiers d'une banque. Il se crée à la première
        importation.
      </p>
      <div class="card">
        @for (p of profiles.value() ?? []; track p.id) {
          <div class="row">
            <span class="main">
              <span>{{ p.name }}</span>
              <span class="muted small"
                >{{ p.format
                }}{{ p.default_account_id ? ' · ' + accountName(p.default_account_id) : '' }}</span
              >
            </span>
            <button
              mat-icon-button
              [matMenuTriggerFor]="menu"
              [attr.aria-label]="'Actions sur ' + p.name"
            >
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              <button mat-menu-item (click)="rename(p)"><mat-icon>edit</mat-icon>Renommer</button>
              @if (p.format !== 'OFX') {
                <a mat-menu-item routerLink="/import/nouveau" [queryParams]="{ profil: p.id }"
                  ><mat-icon>tune</mat-icon>Modifier les réglages</a
                >
              }
              @for (a of accounts.open(); track a.id) {
                @if (a.id !== p.default_account_id) {
                  <button mat-menu-item (click)="setAccount(p, a.id)">
                    <mat-icon>account_balance</mat-icon>Compte par défaut : {{ a.name }}
                  </button>
                }
              }
              <button mat-menu-item (click)="remove(p)">
                <mat-icon>delete</mat-icon>Supprimer
              </button>
            </mat-menu>
          </div>
        } @empty {
          <p class="empty">Aucun profil enregistré.</p>
        }
      </div>
    </div>
  `,
  styles: `
    .row {
      display: flex;
      align-items: center;
      border-top: 1px solid var(--mat-sys-outline-variant);
      .main {
        flex: 1;
        display: flex;
        flex-direction: column;
        padding: 0.5rem 0;
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
  `,
})
export class ProfilesPage {
  private readonly api = inject(ImportsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  protected readonly accounts = inject(AccountsStore);

  protected readonly profiles = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listImportProfiles(),
  });

  protected accountName(id: string): string {
    return this.accounts.byId().get(id)?.name ?? '—';
  }

  private async save(p: Profile, change: Partial<Profile>): Promise<void> {
    try {
      const next = { ...p, ...change };
      await firstValueFrom(
        this.api.updateImportProfile({
          id: p.id,
          body: {
            name: next.name,
            format: next.format,
            settings: next.settings ?? null,
            default_account_id: next.default_account_id ?? null,
          },
        }),
      );
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  protected async rename(p: Profile): Promise<void> {
    const ref = this.dialog.open<CategoryNameDialog, NameData, string>(CategoryNameDialog, {
      data: { title: 'Renommer le profil', name: p.name, action: 'Renommer' },
      width: '24rem',
    });
    const name = await firstValueFrom(ref.afterClosed());
    if (name) {
      await this.save(p, { name });
    }
  }

  protected setAccount(p: Profile, account: string): Promise<void> {
    return this.save(p, { default_account_id: account });
  }

  protected async remove(p: Profile): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${p.name} »`,
      message: 'Les imports déjà faits ne changent pas.',
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) {
      try {
        await firstValueFrom(this.api.deleteImportProfile({ id: p.id }));
        this.version.bump();
      } catch (error) {
        this.snack.open(errorMessage(error), undefined, { duration: 5000 });
      }
    }
  }
}
