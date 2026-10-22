import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { Institution } from '../../api/models/institution';
import { InstitutionsService } from '../../api/services/institutions.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { InstitutionsStore } from '../../shared/stores';

interface Draft {
  id: string | null;
  name: string;
  bank_code: string;
  bic: string;
}

/** Établissements (CPT-02) : référentiel prérempli, modifiable, archivable (R-CPT-10). */
@Component({
  selector: 'app-institutions',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Établissements</h1>
        <button mat-flat-button (click)="edit(null)"><mat-icon>add</mat-icon>Ajouter</button>
      </div>

      @if (draft(); as d) {
        <form class="card editor" (ngSubmit)="save()">
          <mat-form-field>
            <mat-label>Nom</mat-label>
            <input
              matInput
              name="name"
              [ngModel]="d.name"
              (ngModelChange)="patch({ name: $event })"
              required
              maxlength="80"
            />
          </mat-form-field>
          <mat-form-field>
            <mat-label>Code banque</mat-label>
            <input
              matInput
              name="bank_code"
              [ngModel]="d.bank_code"
              (ngModelChange)="patch({ bank_code: $event })"
              maxlength="5"
              inputmode="numeric"
            />
          </mat-form-field>
          <mat-form-field>
            <mat-label>BIC</mat-label>
            <input
              matInput
              name="bic"
              [ngModel]="d.bic"
              (ngModelChange)="patch({ bic: $event })"
              maxlength="11"
            />
          </mat-form-field>
          @if (error()) {
            <p class="error" role="alert">{{ error() }}</p>
          }
          <div class="actions">
            <button mat-button type="button" (click)="draft.set(null)">Annuler</button>
            <button mat-flat-button type="submit">Enregistrer</button>
          </div>
        </form>
      }

      @if (store.all.isLoading() && !store.all.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      }
      <div class="card">
        @for (institution of store.list(); track institution.id) {
          <div class="row" [class.archived]="institution.archived">
            <span class="name">
              {{ institution.name }}
              @if (institution.bank_code) {
                <span class="muted"> · {{ institution.bank_code }}</span>
              }
              @if (institution.archived) {
                <span class="muted"> · archivé</span>
              }
            </span>
            <button
              mat-icon-button
              (click)="edit(institution)"
              [attr.aria-label]="'Modifier ' + institution.name"
            >
              <mat-icon>edit</mat-icon>
            </button>
            <button
              mat-icon-button
              (click)="toggleArchive(institution)"
              [attr.aria-label]="
                (institution.archived ? 'Désarchiver ' : 'Archiver ') + institution.name
              "
            >
              <mat-icon>{{ institution.archived ? 'unarchive' : 'archive' }}</mat-icon>
            </button>
          </div>
        } @empty {
          <p class="empty">Aucun établissement.</p>
        }
      </div>
    </div>
  `,
  styles: `
    .editor {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
      gap: var(--field-gap);
      margin-bottom: 1rem;
      .actions {
        grid-column: 1 / -1;
        display: flex;
        justify-content: flex-end;
        gap: 0.5rem;
      }
    }
    .row {
      display: flex;
      align-items: center;
      border-top: 1px solid var(--mat-sys-outline-variant);
      .name {
        flex: 1;
      }
      &.archived .name {
        color: var(--mat-sys-on-surface-variant);
      }
    }
    .error {
      grid-column: 1 / -1;
      color: var(--mat-sys-error);
    }
  `,
})
export class InstitutionsPage {
  protected readonly store = inject(InstitutionsStore);
  private readonly api = inject(InstitutionsService);
  private readonly version = inject(DataVersion);

  protected readonly draft = signal<Draft | null>(null);
  protected readonly error = signal<string | null>(null);

  protected edit(institution: Institution | null): void {
    this.error.set(null);
    this.draft.set({
      id: institution?.id ?? null,
      name: institution?.name ?? '',
      bank_code: institution?.bank_code ?? '',
      bic: institution?.bic ?? '',
    });
  }

  protected patch(change: Partial<Draft>): void {
    this.draft.update((d) => (d ? { ...d, ...change } : d));
  }

  protected async save(): Promise<void> {
    const d = this.draft();
    if (!d) {
      return;
    }
    const current = d.id ? this.store.list().find((i) => i.id === d.id) : undefined;
    const body = {
      name: d.name,
      bank_code: d.bank_code || null,
      bic: d.bic || null,
      archived: current?.archived ?? false,
    };
    try {
      await firstValueFrom(
        d.id
          ? this.api.updateInstitution({ id: d.id, body })
          : this.api.createInstitution({ body }),
      );
      this.draft.set(null);
      this.version.bump();
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  protected async toggleArchive(institution: Institution): Promise<void> {
    const body = {
      name: institution.name,
      bank_code: institution.bank_code ?? null,
      bic: institution.bic ?? null,
      archived: !institution.archived,
    };
    await firstValueFrom(this.api.updateInstitution({ id: institution.id, body }));
    this.version.bump();
  }
}
