import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { StatementView } from '../../api/models/statement-view';
import { AccountsService } from '../../api/services/accounts.service';
import { DocumentsService } from '../../api/services/documents.service';
import { DataVersion } from '../../core/data-version';
import { keptValue } from '../../core/resources';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { documentUrl, formatSize } from '../documents/document-labels';
import { uploadDocument } from '../documents/upload-dialog';

/** Une ligne de la liste des relevés : un relevé, ou un mois sans relevé (R-CPT-21). */
interface StatementEntry {
  month: string;
  statement: StatementView | null;
}

interface StatementYear {
  year: string;
  count: number;
  entries: StatementEntry[];
}

/** E3 — Relevés d'un compte (CPT-11). */
@Component({
  selector: 'app-account-statements',
  imports: [MatButtonModule, MatIconModule, MatMenuModule, RouterLink, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section
      class="card panel statements"
      [class.dragging]="dragging()"
      (dragover)="dragOver($event)"
      (dragleave)="dragging.set(false)"
      (drop)="drop($event)"
    >
      <header>
        <h2>Relevés</h2>
        <button mat-stroked-button (click)="add()"><mat-icon>add</mat-icon>Ajouter</button>
      </header>
      @for (y of years(); track y.year) {
        <details [open]="expanded().has(y.year)" (toggle)="toggle(y.year, $event)">
          <summary>
            {{ y.year }}
            <span class="muted small">{{ y.count }} relevé{{ y.count > 1 ? 's' : '' }}</span>
          </summary>
          @for (e of y.entries; track $index) {
            @if (e.statement; as s) {
              <div class="row">
                <span>{{ e.month | frDate: 'month' }}</span>
                <span class="name">
                  {{ s.file_name }}
                  <span class="muted small">
                    @if (s.period_start && s.period_end) {
                      du {{ s.period_start | frDate }} au {{ s.period_end | frDate }} ·
                    }
                    {{ size(s.size_bytes) }}{{ s.note ? ' · ' + s.note : '' }}
                  </span>
                </span>
                <span class="actions">
                  <a
                    mat-icon-button
                    [href]="fileUrl(s, false)"
                    target="_blank"
                    rel="noopener"
                    [attr.aria-label]="'Ouvrir le relevé ' + s.file_name"
                    ><mat-icon>open_in_new</mat-icon></a
                  >
                  <a
                    mat-icon-button
                    class="wide"
                    [href]="fileUrl(s, true)"
                    download
                    [attr.aria-label]="'Télécharger le relevé ' + s.file_name"
                    ><mat-icon>download</mat-icon></a
                  >
                  <button
                    mat-icon-button
                    [matMenuTriggerFor]="menu"
                    [attr.aria-label]="'Actions sur le relevé ' + s.file_name"
                  >
                    <mat-icon>more_vert</mat-icon>
                  </button>
                </span>
                <mat-menu #menu>
                  <a mat-menu-item [href]="fileUrl(s, true)" download>
                    <mat-icon>download</mat-icon>Télécharger
                  </a>
                  <a mat-menu-item [routerLink]="['/documents', s.id]">
                    <mat-icon>edit</mat-icon>Modifier
                  </a>
                  <button mat-menu-item (click)="remove(s)">
                    <mat-icon>delete</mat-icon>Supprimer
                  </button>
                </mat-menu>
              </div>
            } @else {
              <div class="row missing">
                <span>{{ e.month | frDate: 'month' }}</span>
                <span class="small">sans relevé</span>
              </div>
            }
          }
        </details>
      } @empty {
        <p class="muted">
          Aucun relevé archivé : déposez un PDF ici, utilisez « Ajouter », ou importez un relevé
          PDF.
        </p>
      }
    </section>
  `,
  styles: `
    .panel {
      margin-top: 1rem;
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .statements {
      outline: 2px dashed transparent;
      outline-offset: -2px;
      &.dragging {
        outline-color: var(--mat-sys-primary);
      }
    }
    summary {
      cursor: pointer;
      padding: 0.5rem 0;
      font: var(--mat-sys-title-small);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .row {
      display: grid;
      grid-template-columns: 9rem 1fr auto;
      align-items: center;
      gap: 0.75rem;
      min-height: 2.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      .name {
        overflow-wrap: anywhere;
      }
      &.missing {
        color: var(--mat-sys-on-surface-variant);
        opacity: 0.7;
      }
      @media (max-width: 599.98px) {
        grid-template-columns: 6.5rem 1fr auto;
        .wide {
          display: none;
        }
      }
    }
  `,
})
export class AccountStatements {
  private readonly api = inject(AccountsService);
  private readonly documents = inject(DocumentsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  readonly accountId = input.required<string>();

  private readonly list = rxResource({
    params: () => ({ id: this.accountId(), version: this.version.value() }),
    stream: ({ params }) => this.api.listStatements({ id: params.id }),
  });
  private readonly currentList = keptValue(this.list, this.accountId);

  protected readonly size = formatSize;
  protected readonly dragging = signal(false);
  /** Années dépliées : toutes repliées par défaut. */
  protected readonly expanded = signal<ReadonlySet<string>>(new Set());

  /** Relevés et mois sans relevé, par année, du plus récent au plus ancien (R-CPT-21). */
  protected readonly years = computed<StatementYear[]>(() => {
    const list = this.currentList();
    if (!list) return [];
    const entries: StatementEntry[] = [
      ...list.statements.map((s) => ({ month: s.statement_month, statement: s })),
      ...list.missing_months.map((month) => ({ month, statement: null })),
    ].sort((a, b) => b.month.localeCompare(a.month));
    const years: StatementYear[] = [];
    for (const entry of entries) {
      const year = entry.month.slice(0, 4);
      let group = years.at(-1);
      if (group?.year !== year) {
        group = { year, count: 0, entries: [] };
        years.push(group);
      }
      group.entries.push(entry);
      if (entry.statement) group.count++;
    }
    return years;
  });

  protected toggle(year: string, event: Event): void {
    const open = (event.target as HTMLDetailsElement).open;
    this.expanded.update((years) => {
      const next = new Set(years);
      if (open) next.add(year);
      else next.delete(year);
      return next;
    });
  }

  protected fileUrl(statement: StatementView, download: boolean): string {
    return documentUrl(statement.id, download);
  }

  /** Nouveau relevé, éventuellement avec un fichier déposé (R-CPT-19, R-DOC-15). La modification se fait sur la
   * fiche du document (E35). */
  protected async add(file: File | null = null): Promise<void> {
    const created = await uploadDocument(this.dialog, {
      file,
      kind: 'BANK_STATEMENT',
      fixedKind: true,
      target: { kind: 'ACCOUNT', id: this.accountId() },
    });
    if (created) this.version.bump();
  }

  protected dragOver(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes('Files')) return;
    event.preventDefault();
    this.dragging.set(true);
  }

  protected async drop(event: DragEvent): Promise<void> {
    event.preventDefault();
    this.dragging.set(false);
    const file = event.dataTransfer?.files[0];
    if (file) await this.add(file);
  }

  protected async remove(statement: StatementView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le relevé ?',
      message: `« ${statement.file_name} » ne sera plus consultable.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.documents.deleteDocument({ id: statement.id }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }
}
