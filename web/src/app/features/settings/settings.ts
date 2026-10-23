import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatListModule } from '@angular/material/list';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';

import { FeaturesInput } from '../../api/models/features-input';
import { SystemService } from '../../api/services/system.service';
import { firstValueFrom } from 'rxjs';

import { Session } from '../../core/auth';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { formatRate, parseRatePpm } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';

/** E9 — Réglages. */
@Component({
  selector: 'app-settings',
  imports: [
    RouterLink,
    MatListModule,
    MatIconModule,
    MatButtonModule,
    MatFormFieldModule,
    MatInputModule,
    MatSlideToggleModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header"><h1>Réglages</h1></div>
      <mat-nav-list class="card">
        <a mat-list-item routerLink="/reglages/categories">
          <mat-icon matListItemIcon>category</mat-icon>
          <span matListItemTitle>Catégories</span>
        </a>
        <a mat-list-item routerLink="/reglages/tiers">
          <mat-icon matListItemIcon>storefront</mat-icon>
          <span matListItemTitle>Tiers</span>
        </a>
        <a mat-list-item routerLink="/reglages/etablissements">
          <mat-icon matListItemIcon>account_balance</mat-icon>
          <span matListItemTitle>Établissements</span>
        </a>
        <a mat-list-item routerLink="/recurrences">
          <mat-icon matListItemIcon>event_repeat</mat-icon>
          <span matListItemTitle>Récurrences</span>
        </a>
        <a mat-list-item routerLink="/reglages/baremes">
          <mat-icon matListItemIcon>percent</mat-icon>
          <span matListItemTitle>Barèmes de taux</span>
        </a>
        <a mat-list-item routerLink="/reglages/regles">
          <mat-icon matListItemIcon>rule</mat-icon>
          <span matListItemTitle>Règles de catégorisation</span>
        </a>
        <a mat-list-item routerLink="/import">
          <mat-icon matListItemIcon>upload_file</mat-icon>
          <span matListItemTitle>Import de relevés</span>
        </a>
        <a mat-list-item routerLink="/reglages/profils">
          <mat-icon matListItemIcon>tune</mat-icon>
          <span matListItemTitle>Profils d'import</span>
        </a>
      </mat-nav-list>

      <section class="card section">
        <h2>Rubriques</h2>
        <p class="muted">
          Les biens (logement, véhicule), les prêts et les salaires ont leurs propres rubriques, à
          activer si vous en avez.
        </p>
        @if (features.value(); as f) {
          <mat-slide-toggle
            [checked]="f.assets"
            [disabled]="f.assets && f.asset_count > 0"
            (change)="toggle({ assets: $event.checked })"
            >Biens</mat-slide-toggle
          >
          @if (f.assets && f.asset_count > 0) {
            <p class="muted small">
              {{ f.asset_count }} bien(s) : les supprimer pour désactiver la rubrique.
            </p>
          }
          <mat-slide-toggle
            [checked]="f.loans"
            [disabled]="f.loans && f.loan_count > 0"
            (change)="toggle({ loans: $event.checked })"
            >Prêts</mat-slide-toggle
          >
          @if (f.loans && f.loan_count > 0) {
            <p class="muted small">
              {{ f.loan_count }} prêt(s) : les supprimer pour désactiver la rubrique.
            </p>
          }
          <mat-slide-toggle
            [checked]="f.salaries"
            [disabled]="f.salaries && f.employment_count > 0"
            (change)="toggle({ salaries: $event.checked })"
            >Salaires</mat-slide-toggle
          >
          @if (f.salaries && f.employment_count > 0) {
            <p class="muted small">
              {{ f.employment_count }} emploi(s) : les supprimer pour désactiver la rubrique.
            </p>
          }
          @if (f.salaries) {
            <mat-form-field class="tolerance">
              <mat-label>Tolérance de l'écart de salaire</mat-label>
              <input
                matInput
                inputmode="decimal"
                [value]="percent(f.salary_gap_tolerance_ppm)"
                (change)="saveTolerance($any($event.target).value)"
              />
              <span matTextSuffix>%</span>
              <mat-hint>Écart signalé entre le net attendu et le reçu (R-SAL-9).</mat-hint>
            </mat-form-field>
          }
        }
      </section>

      <section class="card section">
        <h2>Export</h2>
        <p class="muted">Toutes les données, au format JSON ouvert (ADR-0008).</p>
        <a mat-stroked-button href="/api/v1/export" download>
          <mat-icon>download</mat-icon>
          Télécharger l'export complet
        </a>
      </section>

      <section class="card section">
        <h2>Sécurité</h2>
        <button mat-stroked-button (click)="logoutEverywhere()" [disabled]="busy()">
          <mat-icon>logout</mat-icon>
          Se déconnecter de tous les appareils
        </button>
      </section>

      <section class="card section">
        <h2>À propos</h2>
        @if (info.value(); as i) {
          <p>Cashflow {{ i.version }} · schéma {{ i.schema_version }}</p>
          <p class="muted">
            @if (i.last_backup; as b) {
              Dernière sauvegarde automatique : {{ formatTimestamp(b.at) }}
            } @else {
              Aucune sauvegarde automatique pour l'instant.
            }
          </p>
        }
      </section>
    </div>
  `,
  styles: `
    .section {
      margin-top: 1rem;
      h2 {
        margin: 0 0 0.5rem;
        font: var(--mat-sys-title-medium);
      }
      mat-slide-toggle {
        display: block;
        margin: 0.5rem 0;
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .tolerance {
      margin-top: 0.75rem;
      width: min(20rem, 100%);
    }
  `,
})
export class SettingsPage {
  private readonly system = inject(SystemService);
  private readonly session = inject(Session);
  private readonly dialog = inject(MatDialog);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);

  protected readonly info = rxResource({ stream: () => this.system.systemInfo() });
  protected readonly features = rxResource({
    params: () => this.version.value(),
    stream: () => this.system.getFeatures(),
  });
  protected readonly busy = signal(false);

  protected formatTimestamp(at: string): string {
    return new Intl.DateTimeFormat('fr-FR', { dateStyle: 'long', timeStyle: 'short' }).format(
      new Date(at),
    );
  }

  protected percent(ppm: number): string {
    return formatRate(ppm).replace(/\u00a0%$/, '');
  }

  /** Tolérance de l'écart entre salaire attendu et reçu (R-SAL-9). */
  protected async saveTolerance(value: string): Promise<void> {
    const ppm = parseRatePpm(value);
    if (ppm === null) {
      this.snack.open('Pourcentage invalide', undefined, { duration: 4000 });
      return;
    }
    await this.toggle({ salary_gap_tolerance_ppm: ppm });
  }

  /** Active ou désactive une rubrique (R-BIE-7, R-PRE-12, R-SAL-14). */
  protected async toggle(body: FeaturesInput): Promise<void> {
    try {
      await firstValueFrom(this.system.updateFeatures({ body }));
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 4000 });
    }
    this.version.bump();
  }

  protected async logoutEverywhere(): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Se déconnecter de tous les appareils',
      message: 'Toutes les sessions seront fermées, y compris celle-ci.',
      action: 'Se déconnecter partout',
    });
    if (!ok) {
      return;
    }
    this.busy.set(true);
    try {
      await this.session.logout(true);
      await this.router.navigate(['/connexion']);
    } finally {
      this.busy.set(false);
    }
  }
}
