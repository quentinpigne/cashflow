import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { debounceTime, firstValueFrom } from 'rxjs';

import { DocumentKind } from '../../api/models/document-kind';
import { DocumentView } from '../../api/models/document-view';
import { LinkKind } from '../../api/models/link-kind';
import { DocumentsService } from '../../api/services/documents.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage, problemOf } from '../../core/http-errors';
import { Layout } from '../../core/layout';
import { FrDatePipe } from '../../shared/date.pipe';
import {
  DOCUMENT_KINDS,
  DOCUMENT_MAX_BYTES,
  KIND_ICONS,
  KIND_LABELS,
  LINK_KINDS,
  LINK_LABELS,
  documentUrl,
  formatSize,
  linkLabel,
  snippetParts,
} from './document-labels';

/** Valeur du filtre de type : un type, les documents à classer, ou tous. */
type KindFilter = DocumentKind | 'UNSORTED' | null;

interface YearGroup {
  year: string;
  documents: DocumentView[];
}

/** Fichier refusé à l'envoi, avec le document qui l'a déjà s'il y en a un (R-DOC-3). */
interface Refusal {
  name: string;
  reason: string;
  existing: string | null;
}

/** E34 — Documents : la bibliothèque, cherchable et filtrable (DOC-01, DOC-03). */
@Component({
  selector: 'app-documents',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatButtonToggleModule,
    MatDialogModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div
      class="page"
      [class.dragging]="dragging()"
      (dragover)="dragOver($event)"
      (dragleave)="dragging.set(false)"
      (drop)="drop($event)"
    >
      <div class="page-header">
        <h1>Documents</h1>
        <input
          #picker
          class="hidden"
          type="file"
          multiple
          accept=".pdf,.jpg,.jpeg,.png,application/pdf,image/jpeg,image/png"
          aria-label="Fichiers à envoyer"
          (change)="choose($event)"
        />
        <button mat-flat-button (click)="picker.click()">
          <mat-icon>upload_file</mat-icon
          >{{ layout.mobile() ? 'Envoyer' : 'Envoyer des documents' }}
        </button>
      </div>

      <mat-form-field class="search" subscriptSizing="dynamic">
        <mat-icon matPrefix>search</mat-icon>
        <mat-label>Rechercher dans les titres, les rattachements et le contenu</mat-label>
        <input matInput [ngModel]="query()" (ngModelChange)="query.set($event)" />
        @if (query()) {
          <button matSuffix mat-icon-button aria-label="Effacer" (click)="query.set('')">
            <mat-icon>close</mat-icon>
          </button>
        }
      </mat-form-field>

      <div class="filters">
        @if (unsortedCount() > 0 && kind() !== 'UNSORTED') {
          <button mat-stroked-button class="unsorted" (click)="kind.set('UNSORTED')">
            <mat-icon>inbox</mat-icon>À classer ({{ unsortedCount() }})
          </button>
        }
        <mat-form-field subscriptSizing="dynamic">
          <mat-label>Type</mat-label>
          <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
            <mat-option [value]="null">Tous</mat-option>
            <mat-option value="UNSORTED">À classer</mat-option>
            @for (k of kinds; track k) {
              <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field subscriptSizing="dynamic">
          <mat-label>Année</mat-label>
          <mat-select [ngModel]="year()" (ngModelChange)="year.set($event)">
            <mat-option [value]="null">Toutes</mat-option>
            @for (y of years(); track y) {
              <mat-option [value]="y">{{ y }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field subscriptSizing="dynamic">
          <mat-label>Rattaché à</mat-label>
          <mat-select [ngModel]="linkKind()" (ngModelChange)="linkKind.set($event)">
            <mat-option [value]="null">Tout</mat-option>
            <mat-option value="NONE">Sans rattachement</mat-option>
            @for (k of linkKinds; track k) {
              <mat-option [value]="k">{{ linkLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        @if (filtered()) {
          <button mat-button (click)="clear()">Effacer les filtres</button>
        }
      </div>

      @if (uploading(); as u) {
        <div class="uploading">
          <span>Envoi de {{ u.name }} ({{ u.index }}/{{ u.total }})</span>
          <mat-progress-bar mode="determinate" [value]="(100 * (u.index - 1)) / u.total" />
        </div>
      }

      @if (list.isLoading() && !list.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      }

      @for (g of groups(); track g.year) {
        @if (g.year) {
          <h2 class="year">{{ g.year }}</h2>
        }
        <div class="card list">
          @for (d of g.documents; track d.id) {
            <div class="row">
              <mat-icon class="kind" [class.unsorted-icon]="!d.kind">{{
                d.kind ? icons[d.kind] : 'inbox'
              }}</mat-icon>
              <a class="main" [routerLink]="['/documents', d.id]">
                <span class="title">{{ d.title }}</span>
                <span class="muted small">
                  {{ d.kind ? kindLabels[d.kind] : 'À classer' }} ·
                  {{ d.reference_date | frDate }} · {{ size(d.size_bytes) }}
                  @if (!d.content_indexed) {
                    · contenu non indexé
                  }
                </span>
                @if (d.snippet) {
                  <span class="snippet small">
                    @for (p of parts(d.snippet); track $index) {
                      @if (p.hit) {
                        <strong>{{ p.text }}</strong>
                      } @else {
                        {{ p.text }}
                      }
                    }
                  </span>
                }
                @if (d.links.length > 0) {
                  <span class="chips">
                    @for (l of d.links; track l.id) {
                      <span class="chip">{{ label(l) }}</span>
                    }
                  </span>
                }
              </a>
              <a
                mat-icon-button
                [href]="url(d.id)"
                target="_blank"
                rel="noopener"
                [attr.aria-label]="'Ouvrir ' + d.title"
                ><mat-icon>open_in_new</mat-icon></a
              >
              @if (!layout.mobile()) {
                <a
                  mat-icon-button
                  [href]="url(d.id, true)"
                  download
                  [attr.aria-label]="'Télécharger ' + d.title"
                  ><mat-icon>download</mat-icon></a
                >
              }
            </div>
          }
        </div>
      } @empty {
        @if (list.hasValue()) {
          @if (filtered()) {
            <p class="empty">
              Aucun document ne correspond.
              <button mat-button (click)="clear()">Effacer les filtres</button>
            </p>
          } @else {
            <p class="empty">
              Aucun document. Envoyez vos contrats, factures et tickets, ou déposez-les ici.
            </p>
          }
        }
      }
    </div>
  `,
  styles: `
    .page {
      outline: 2px dashed transparent;
      outline-offset: -4px;
      min-height: 60vh;
      &.dragging {
        outline-color: var(--mat-sys-primary);
      }
    }
    .hidden {
      display: none;
    }
    .search {
      width: 100%;
    }
    .filters {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem;
      margin: 0.75rem 0;
      mat-form-field {
        width: 11rem;
      }
      @media (max-width: 599.98px) {
        mat-form-field {
          width: calc(50% - 0.25rem);
        }
      }
    }
    .unsorted {
      --mat-button-outlined-label-text-color: var(--cf-warning, #b26a00);
    }
    .uploading {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      margin-bottom: 0.75rem;
    }
    .year {
      font: var(--mat-sys-title-small);
      margin: 1rem 0 0.5rem;
    }
    .list {
      padding-top: 0;
      padding-bottom: 0;
    }
    .row {
      display: grid;
      grid-template-columns: auto 1fr auto auto;
      align-items: center;
      gap: 0.75rem;
      padding: 0.5rem 0;
      & + .row {
        border-top: 1px solid var(--mat-sys-outline-variant);
      }
      @media (max-width: 599.98px) {
        grid-template-columns: auto 1fr auto;
      }
    }
    .kind {
      color: var(--mat-sys-on-surface-variant);
      &.unsorted-icon {
        color: var(--cf-warning, #b26a00);
      }
    }
    .main {
      display: flex;
      flex-direction: column;
      gap: 0.125rem;
      color: inherit;
      text-decoration: none;
      min-width: 0;
      overflow-wrap: anywhere;
    }
    .title {
      font: var(--mat-sys-body-large);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .snippet {
      color: var(--mat-sys-on-surface-variant);
      strong {
        color: var(--mat-sys-on-surface);
      }
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem;
      margin-top: 0.25rem;
    }
    .chip {
      font: var(--mat-sys-label-small);
      padding: 0.125rem 0.5rem;
      border-radius: 1rem;
      background: var(--mat-sys-surface-container-high);
    }
  `,
})
export class DocumentsPage {
  private readonly api = inject(DocumentsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  protected readonly layout = inject(Layout);

  protected readonly kinds = DOCUMENT_KINDS;
  protected readonly kindLabels = KIND_LABELS;
  protected readonly icons = KIND_ICONS;
  protected readonly linkKinds = LINK_KINDS;
  protected readonly linkLabels = LINK_LABELS;
  protected readonly size = formatSize;
  protected readonly label = linkLabel;
  protected readonly parts = snippetParts;

  protected readonly query = signal('');
  protected readonly kind = signal<KindFilter>(null);
  protected readonly year = signal<number | null>(null);
  protected readonly linkKind = signal<LinkKind | 'NONE' | null>(null);
  protected readonly dragging = signal(false);
  protected readonly uploading = signal<{ name: string; index: number; total: number } | null>(
    null,
  );

  private readonly debounced = toSignal(toObservable(this.query).pipe(debounceTime(250)), {
    initialValue: '',
  });

  protected readonly list = rxResource({
    params: () => ({
      q: this.debounced().trim() || undefined,
      kind: this.kind(),
      year: this.year(),
      linkKind: this.linkKind(),
      version: this.version.value(),
    }),
    stream: ({ params }) =>
      this.api.listDocuments({
        q: params.q,
        kind: params.kind && params.kind !== 'UNSORTED' ? params.kind : undefined,
        unsorted: params.kind === 'UNSORTED' || undefined,
        year: params.year ?? undefined,
        link_kind: params.linkKind && params.linkKind !== 'NONE' ? params.linkKind : undefined,
        unlinked: params.linkKind === 'NONE' || undefined,
      }),
  });

  /** Tous les documents, pour les années proposées et le nombre de documents à classer. */
  private readonly all = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listDocuments(),
  });

  protected readonly unsortedCount = computed(
    () => (this.all.value() ?? []).filter((d) => !d.kind).length,
  );
  protected readonly years = computed(() =>
    [...new Set((this.all.value() ?? []).map((d) => Number(d.reference_date.slice(0, 4))))].sort(
      (a, b) => b - a,
    ),
  );

  protected readonly filtered = computed(
    () => !!this.query().trim() || !!this.kind() || !!this.year() || !!this.linkKind(),
  );

  /** Sans recherche, par année de la date de référence ; avec une recherche, par pertinence (R-DOC-20). */
  protected readonly groups = computed<YearGroup[]>(() => {
    const documents = this.list.value() ?? [];
    if (documents.length === 0) return [];
    if (this.debounced().trim()) return [{ year: '', documents }];
    const groups: YearGroup[] = [];
    for (const d of documents) {
      const year = d.reference_date.slice(0, 4);
      let group = groups.at(-1);
      if (group?.year !== year) {
        group = { year, documents: [] };
        groups.push(group);
      }
      group.documents.push(d);
    }
    return groups;
  });

  protected url(id: string, download = false): string {
    return documentUrl(id, download);
  }

  protected clear(): void {
    this.query.set('');
    this.kind.set(null);
    this.year.set(null);
    this.linkKind.set(null);
  }

  protected choose(event: Event): void {
    const input = event.target as HTMLInputElement;
    const files = [...(input.files ?? [])];
    input.value = '';
    void this.send(files);
  }

  protected dragOver(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes('Files')) return;
    event.preventDefault();
    this.dragging.set(true);
  }

  protected drop(event: DragEvent): void {
    event.preventDefault();
    this.dragging.set(false);
    void this.send([...(event.dataTransfer?.files ?? [])]);
  }

  /** Envoie des fichiers : chacun devient un document à classer ; un refus n'arrête pas les autres (R-DOC-14). */
  private async send(files: File[]): Promise<void> {
    if (files.length === 0) return;
    const refusals: Refusal[] = [];
    let archived = 0;
    let last: string | null = null;
    for (const [i, file] of files.entries()) {
      this.uploading.set({ name: file.name, index: i + 1, total: files.length });
      if (file.size > DOCUMENT_MAX_BYTES) {
        refusals.push({ name: file.name, reason: 'Le fichier dépasse 10 Mo.', existing: null });
        continue;
      }
      try {
        const created = await firstValueFrom(this.api.createDocument({ body: { file } }));
        archived++;
        last = created.id;
      } catch (e) {
        const problem = problemOf(e);
        refusals.push({
          name: file.name,
          reason: errorMessage(e),
          existing:
            problem?.code === 'DOCUMENT_ALREADY_ARCHIVED' ? (problem.document_id ?? null) : null,
        });
      }
    }
    this.uploading.set(null);
    this.version.bump();
    const summary =
      `${archived} document${archived > 1 ? 's archivés' : ' archivé'}` +
      (refusals.length ? `, ${refusals.length} refusé${refusals.length > 1 ? 's' : ''}` : '');
    if (refusals.length > 0) {
      const ref = this.dialog.open(RefusalsDialog, { data: refusals, maxWidth: '32rem' });
      const open = await firstValueFrom(ref.afterClosed());
      // « Fermer » rend une chaîne vide ; « Ouvrir le document existant », son identifiant.
      if (typeof open === 'string' && open) await this.router.navigate(['/documents', open]);
    } else if (archived === 1 && last) {
      const ref = this.snack.open(summary, 'Décrire', { duration: 5000 });
      ref.onAction().subscribe(() => void this.router.navigate(['/documents', last]));
    } else {
      this.snack.open(summary, undefined, { duration: 4000 });
    }
  }
}

/** Fichiers refusés à l'envoi, et leur raison (R-DOC-14). */
@Component({
  selector: 'app-refusals-dialog',
  imports: [MatButtonModule, MatDialogModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>
      {{ data.length > 1 ? data.length + ' fichiers refusés' : 'Un fichier refusé' }}
    </h2>
    <mat-dialog-content>
      @for (r of data; track $index) {
        <p>
          <strong>{{ r.name }}</strong> : {{ r.reason }}
          @if (r.existing) {
            <button mat-button [mat-dialog-close]="r.existing">Ouvrir le document existant</button>
          }
        </p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-flat-button mat-dialog-close>Fermer</button>
    </mat-dialog-actions>
  `,
})
export class RefusalsDialog {
  protected readonly data = inject<Refusal[]>(MAT_DIALOG_DATA);
}
