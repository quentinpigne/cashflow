import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { RouterLink } from '@angular/router';

import { PayeeGroupView } from '../../api/models/payee-group-view';
import { PayeeView } from '../../api/models/payee-view';
import { PayeesStore } from '../../shared/stores';
import { PayeeActions } from './payee-actions';

interface Section {
  group: PayeeGroupView | null;
  payees: PayeeView[];
}

/** E31 — Tiers (TIE-01). */
@Component({
  selector: 'app-payees',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatCheckboxModule,
    MatExpansionModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatProgressBarModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Tiers</h1>
        <button mat-stroked-button (click)="actions.createGroup()">
          <mat-icon>create_new_folder</mat-icon>Nouveau groupe
        </button>
        <button mat-flat-button (click)="actions.createPayee(null)">
          <mat-icon>add</mat-icon>Nouveau tiers
        </button>
      </div>

      <mat-form-field class="search" subscriptSizing="dynamic">
        <mat-icon matPrefix>search</mat-icon>
        <mat-label>Rechercher</mat-label>
        <input matInput [ngModel]="query()" (ngModelChange)="query.set($event)" />
      </mat-form-field>

      @if (selection().size > 0) {
        <div class="bulk card">
          <span>{{ selection().size }} tiers sélectionné(s)</span>
          <button mat-flat-button (click)="regroup()">Changer de groupe</button>
          <button mat-button (click)="clearSelection()">Annuler</button>
        </div>
      }

      @if (store.all.isLoading() && !store.all.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      }

      @for (section of sections(); track section.group?.id ?? 'none') {
        <section class="section">
          <div class="row group">
            @if (section.group; as g) {
              <a class="name" [routerLink]="['/tiers/groupe', g.id]">{{ g.name }}</a>
              <span class="muted count">{{ g.payee_count }} tiers</span>
              <button
                mat-icon-button
                [matMenuTriggerFor]="groupMenu"
                [attr.aria-label]="'Actions sur ' + g.name"
              >
                <mat-icon>more_vert</mat-icon>
              </button>
              <mat-menu #groupMenu>
                <button mat-menu-item (click)="actions.createPayee(g.id)">
                  <mat-icon>add</mat-icon>Ajouter un tiers
                </button>
                <button mat-menu-item (click)="actions.editGroup(g)">
                  <mat-icon>edit</mat-icon>Renommer
                </button>
                <button mat-menu-item (click)="actions.removeGroup(g)">
                  <mat-icon>delete</mat-icon>Supprimer
                </button>
              </mat-menu>
            } @else {
              <span class="name">Sans groupe</span>
            }
          </div>
          @for (payee of section.payees; track payee.id) {
            <div class="row payee">
              <mat-checkbox
                [checked]="selection().has(payee.id)"
                (change)="toggle(payee.id, $event.checked)"
                [attr.aria-label]="'Sélectionner ' + payee.name"
              />
              <a class="name" [routerLink]="['/tiers', payee.id]">{{ payee.name }}</a>
              <span class="muted count">{{ payee.transaction_count }}</span>
              <button
                mat-icon-button
                [matMenuTriggerFor]="menu"
                [attr.aria-label]="'Actions sur ' + payee.name"
              >
                <mat-icon>more_vert</mat-icon>
              </button>
              <mat-menu #menu>
                <button mat-menu-item (click)="actions.editPayee(payee)">
                  <mat-icon>edit</mat-icon>Modifier (nom, groupe)
                </button>
                <button mat-menu-item (click)="actions.setArchived(payee, true)">
                  <mat-icon>archive</mat-icon>Archiver
                </button>
                <button mat-menu-item (click)="actions.merge(payee)">
                  <mat-icon>merge</mat-icon>Fusionner dans…
                </button>
                <button mat-menu-item (click)="actions.remove(payee)" [disabled]="payee.in_use">
                  <mat-icon>delete</mat-icon>Supprimer
                </button>
              </mat-menu>
            </div>
          } @empty {
            <p class="muted empty">Aucun tiers dans ce groupe.</p>
          }
        </section>
      } @empty {
        @if (store.all.hasValue()) {
          <p class="empty">
            Aucun tiers. Un tiers se crée ici, ou directement depuis la saisie d'une transaction.
          </p>
        }
      }

      @if (archived().length > 0) {
        <mat-accordion>
          <mat-expansion-panel class="archived">
            <mat-expansion-panel-header>
              <mat-panel-title>Archivés ({{ archived().length }})</mat-panel-title>
            </mat-expansion-panel-header>
            @for (payee of archived(); track payee.id) {
              <div class="row">
                <a class="name" [routerLink]="['/tiers', payee.id]">{{ payee.name }}</a>
                <button mat-button (click)="actions.setArchived(payee, false)">Désarchiver</button>
              </div>
            }
          </mat-expansion-panel>
        </mat-accordion>
      }
    </div>
  `,
  styles: `
    .search {
      width: 100%;
      margin-bottom: 0.5rem;
    }

    .bulk {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem;
      margin-bottom: 0.5rem;
    }

    .section {
      margin-bottom: 1rem;
    }

    .row {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      min-height: 2.75rem;
      padding: 0 0.25rem;
      border-bottom: 1px solid var(--mat-sys-outline-variant);

      .name {
        flex: 1;
        color: inherit;
        text-decoration: none;
      }

      a.name:hover {
        text-decoration: underline;
      }

      &.group .name {
        font: var(--mat-sys-title-small);
      }

      &.payee {
        padding-left: 0.5rem;
      }
    }

    .count {
      font: var(--mat-sys-body-small);
    }

    .empty {
      padding: 0.5rem;
    }

    .archived {
      margin-top: 1rem;
    }
  `,
})
export class PayeesPage {
  protected readonly store = inject(PayeesStore);
  protected readonly actions = inject(PayeeActions);

  protected readonly query = signal('');
  protected readonly selection = signal<Set<string>>(new Set());

  private readonly matching = computed(() => {
    const query = this.query().trim().toLowerCase();
    return this.store.list().filter((p) => !query || p.name.toLowerCase().includes(query));
  });

  /** Groupes par ordre alphabétique, puis « Sans groupe » (ecrans.md E31). */
  protected readonly sections = computed<Section[]>(() => {
    const active = this.matching().filter((p) => !p.archived);
    const searching = !!this.query().trim();
    const groups: Section[] = this.store
      .groups()
      .map((group) => ({ group, payees: active.filter((p) => p.group_id === group.id) }))
      .filter((s) => !searching || s.payees.length > 0);
    const ungrouped = active.filter((p) => !p.group_id);
    return ungrouped.length ? [...groups, { group: null, payees: ungrouped }] : groups;
  });

  protected readonly archived = computed(() => this.matching().filter((p) => p.archived));

  protected toggle(id: string, checked: boolean): void {
    this.selection.update((s) => {
      const next = new Set(s);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
  }

  protected async regroup(): Promise<void> {
    if (await this.actions.regroup([...this.selection()])) this.clearSelection();
  }

  protected clearSelection(): void {
    this.selection.set(new Set());
  }
}
