import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { MemberView } from '../../api/models/member-view';
import { HouseholdService } from '../../api/services/household.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { MemberChip } from '../../shared/member-chip';
import { MembersStore } from '../../shared/stores';

interface Draft {
  id: string | null;
  name: string;
  color: string;
}

/** Couleur proposée à un nouveau membre, selon son rang. */
const PALETTE = ['#1a73e8', '#d93025', '#188038', '#e37400', '#9334e6', '#007b83'];

/** Ce qui est affecté à un membre, en toutes lettres (R-MBR-2). */
export function assignments(m: MemberView): string {
  const parts = [
    [m.account_count, 'compte', 'comptes'],
    [m.split_count, 'ventilation', 'ventilations'],
    [m.employment_count, 'emploi', 'emplois'],
  ] as const;
  const text = parts
    .filter(([count]) => count > 0)
    .map(([count, one, many]) => `${count} ${count > 1 ? many : one}`);
  return text.length ? text.join(', ') : 'rien d’affecté';
}

/** E44 — Membres du foyer (MBR-01, R-MBR-1, R-MBR-2). */
@Component({
  selector: 'app-members',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MemberChip,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Membres du foyer</h1>
        <button mat-flat-button (click)="edit(null)"><mat-icon>add</mat-icon>Nouveau membre</button>
      </div>
      <p class="muted intro">
        Affectez à un membre ses comptes, ses transactions et son emploi. Ce qui n'est affecté à
        personne appartient au foyer.
      </p>

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
              maxlength="40"
              cdkFocusInitial
            />
          </mat-form-field>
          <label class="color">
            Couleur
            <input
              type="color"
              name="color"
              [ngModel]="d.color"
              (ngModelChange)="patch({ color: $event })"
            />
          </label>
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
        @for (member of store.list(); track member.id; let first = $first; let last = $last) {
          <div class="row">
            <app-member-chip [member]="member" />
            <span class="name">
              {{ member.name }}
              <span class="muted small">{{ assignments(member) }}</span>
            </span>
            <button
              mat-icon-button
              [disabled]="first"
              (click)="move(member, -1)"
              [attr.aria-label]="'Monter ' + member.name"
            >
              <mat-icon>arrow_upward</mat-icon>
            </button>
            <button
              mat-icon-button
              [disabled]="last"
              (click)="move(member, 1)"
              [attr.aria-label]="'Descendre ' + member.name"
            >
              <mat-icon>arrow_downward</mat-icon>
            </button>
            <button
              mat-icon-button
              (click)="edit(member)"
              [attr.aria-label]="'Modifier ' + member.name"
            >
              <mat-icon>edit</mat-icon>
            </button>
            <button
              mat-icon-button
              (click)="remove(member)"
              [attr.aria-label]="'Supprimer ' + member.name"
            >
              <mat-icon>delete</mat-icon>
            </button>
          </div>
        } @empty {
          @if (store.all.hasValue()) {
            <p class="empty">Aucun membre : tout appartient au foyer.</p>
          }
        }
      </div>
    </div>
  `,
  styles: `
    .intro {
      margin: 0 0 1rem;
    }
    .editor {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--field-gap);
      margin-bottom: 1rem;
      mat-form-field {
        flex: 1 1 14rem;
      }
      .color {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        input {
          width: 3rem;
          height: 2.5rem;
          border: none;
          background: none;
        }
      }
      .actions {
        flex-basis: 100%;
        display: flex;
        justify-content: flex-end;
        gap: 0.5rem;
      }
    }
    .row {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      min-height: 3rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      .name {
        flex: 1;
        display: flex;
        flex-direction: column;
        min-width: 0;
      }
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      flex-basis: 100%;
      color: var(--mat-sys-error);
    }
    .empty {
      padding: 0.5rem;
    }
  `,
})
export class MembersPage {
  protected readonly store = inject(MembersStore);
  private readonly api = inject(HouseholdService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  protected readonly draft = signal<Draft | null>(null);
  protected readonly error = signal<string | null>(null);
  protected readonly assignments = assignments;

  protected edit(member: MemberView | null): void {
    this.error.set(null);
    const color = PALETTE[this.store.list().length % PALETTE.length];
    this.draft.set({
      id: member?.id ?? null,
      name: member?.name ?? '',
      color: member?.color ?? color,
    });
  }

  protected patch(change: Partial<Draft>): void {
    this.draft.update((d) => (d ? { ...d, ...change } : d));
  }

  protected async save(): Promise<void> {
    const d = this.draft();
    if (!d) return;
    const body = { name: d.name, color: d.color || null };
    try {
      if (d.id) {
        await firstValueFrom(this.api.updateMember({ id: d.id, body }));
      } else {
        await firstValueFrom(this.api.createMember({ body }));
      }
      this.draft.set(null);
      this.version.bump();
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }

  protected async move(member: MemberView, offset: number): Promise<void> {
    const ids = this.store.list().map((m) => m.id);
    const from = ids.indexOf(member.id);
    const to = from + offset;
    if (from < 0 || to < 0 || to >= ids.length) return;
    [ids[from], ids[to]] = [ids[to], ids[from]];
    try {
      await firstValueFrom(this.api.reorderMembers({ body: { ids } }));
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }

  /** Supprime un membre : ce qui lui est affecté revient au foyer (R-MBR-2). */
  protected async remove(member: MemberView): Promise<void> {
    const assigned = assignments(member);
    const message =
      assigned === 'rien d’affecté'
        ? `${member.name} n'a rien d'affecté.`
        : `${assigned} reviendront au foyer. Aucun montant ne change.`;
    const ok = await confirm(this.dialog, {
      title: `Supprimer ${member.name}`,
      message,
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteMember({ id: member.id }));
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), undefined, { duration: 5000 });
    }
  }
}
