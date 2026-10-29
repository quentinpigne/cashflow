import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { DocumentKind } from '../../api/models/document-kind';
import { DocumentView } from '../../api/models/document-view';
import { LinkTarget } from '../../api/models/link-target';
import { DocumentsService } from '../../api/services/documents.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { Layout } from '../../core/layout';
import { FrDatePipe } from '../../shared/date.pipe';
import { KIND_ICONS, KIND_LABELS, documentUrl, linkLabel } from './document-labels';
import { attachDocuments } from './link-dialogs';
import { uploadDocument } from './upload-dialog';

/**
 * Section Documents de la fiche d'un objet (DOC-02), ou encadré Justificatifs d'une transaction (DOC-04) : les
 * documents rattachés à l'objet, l'envoi d'un document rattaché d'office, le rattachement d'un document existant.
 */
@Component({
  selector: 'app-document-section',
  imports: [MatButtonModule, MatIconModule, MatMenuModule, RouterLink, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section
      class="card panel"
      [class.dragging]="dragging()"
      (dragover)="dragOver($event)"
      (dragleave)="dragging.set(false)"
      (drop)="drop($event)"
    >
      <header>
        <h2>{{ heading() }}</h2>
        <span class="actions">
          @if (layout.mobile() && thumbnails()) {
            <input
              #camera
              class="hidden"
              type="file"
              accept="image/*"
              capture="environment"
              aria-label="Prendre une photo"
              (change)="choose($event)"
            />
            <button mat-stroked-button (click)="camera.click()">
              <mat-icon>photo_camera</mat-icon>Photo
            </button>
          }
          <button mat-stroked-button (click)="add(null)"><mat-icon>add</mat-icon>Ajouter</button>
          <button
            mat-icon-button
            [matMenuTriggerFor]="more"
            [attr.aria-label]="'Autres actions sur les ' + heading().toLowerCase()"
          >
            <mat-icon>more_vert</mat-icon>
          </button>
          <mat-menu #more>
            <button mat-menu-item (click)="attachExisting()">
              <mat-icon>link</mat-icon>Rattacher un document existant
            </button>
          </mat-menu>
        </span>
      </header>
      @if (thumbnails()) {
        <div class="tiles">
          @for (d of documents(); track d.id) {
            <div class="tile">
              <a [routerLink]="['/documents', d.id]" class="preview" [attr.aria-label]="d.title">
                @if (d.media_type === 'application/pdf') {
                  <mat-icon>picture_as_pdf</mat-icon>
                } @else {
                  <img [src]="url(d.id)" alt="" loading="lazy" />
                }
              </a>
              <span class="tile-title">{{ d.title }}</span>
              <button
                mat-icon-button
                class="detach"
                [attr.aria-label]="'Détacher ' + d.title"
                (click)="detach(d)"
              >
                <mat-icon>link_off</mat-icon>
              </button>
            </div>
          } @empty {
            <p class="muted">{{ empty() }}</p>
          }
        </div>
      } @else {
        @for (d of documents(); track d.id) {
          <div class="row">
            <mat-icon class="kind">{{ d.kind ? icons[d.kind] : 'inbox' }}</mat-icon>
            <a class="name" [routerLink]="['/documents', d.id]">
              {{ d.title }}
              <span class="muted small">
                {{ d.kind ? kindLabels[d.kind] : 'À classer' }} · {{ d.reference_date | frDate }}
              </span>
              @if (others(d).length > 0) {
                <span class="chips">
                  @for (l of others(d); track l.id) {
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
            <button
              mat-icon-button
              [matMenuTriggerFor]="menu"
              [attr.aria-label]="'Actions sur ' + d.title"
            >
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              <a mat-menu-item [href]="url(d.id, true)" download>
                <mat-icon>download</mat-icon>Télécharger
              </a>
              <button mat-menu-item (click)="detach(d)">
                <mat-icon>link_off</mat-icon>Détacher
              </button>
            </mat-menu>
          </div>
        } @empty {
          <p class="muted">{{ empty() }}</p>
        }
      }
    </section>
  `,
  styles: `
    .panel {
      margin-top: 1rem;
      outline: 2px dashed transparent;
      outline-offset: -2px;
      &.dragging {
        outline-color: var(--mat-sys-primary);
      }
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 0.5rem;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
      .actions {
        display: flex;
        align-items: center;
        gap: 0.25rem;
      }
    }
    .hidden {
      display: none;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .row {
      display: grid;
      grid-template-columns: auto 1fr auto auto;
      align-items: center;
      gap: 0.5rem;
      min-height: 2.75rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      .kind {
        color: var(--mat-sys-on-surface-variant);
      }
      .name {
        color: inherit;
        text-decoration: none;
        overflow-wrap: anywhere;
        display: flex;
        flex-direction: column;
      }
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem;
      margin: 0.25rem 0;
    }
    .chip {
      font: var(--mat-sys-label-small);
      padding: 0.125rem 0.5rem;
      border-radius: 1rem;
      background: var(--mat-sys-surface-container-high);
    }
    .tiles {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(7rem, 1fr));
      gap: 0.75rem;
      margin-top: 0.75rem;
    }
    .tile {
      position: relative;
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      .preview {
        display: flex;
        align-items: center;
        justify-content: center;
        height: 6rem;
        border-radius: var(--mat-sys-corner-small);
        background: var(--mat-sys-surface-container-high);
        overflow: hidden;
        color: var(--mat-sys-on-surface-variant);
        img {
          width: 100%;
          height: 100%;
          object-fit: cover;
        }
        mat-icon {
          font-size: 2.5rem;
          width: 2.5rem;
          height: 2.5rem;
        }
      }
      .tile-title {
        font: var(--mat-sys-body-small);
        overflow-wrap: anywhere;
      }
      .detach {
        position: absolute;
        top: 0;
        right: 0;
      }
    }
  `,
})
export class DocumentSection {
  private readonly api = inject(DocumentsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  protected readonly layout = inject(Layout);

  /** Objet de la fiche : les documents qui lui sont rattachés. */
  readonly target = input.required<LinkTarget>();
  readonly heading = input('Documents');
  /** Type proposé à l'envoi (R-DOC-15). */
  readonly defaultKind = input<DocumentKind | null>(null);
  /** Types montrés ailleurs sur la fiche : les relevés d'un compte, les fiches de paie d'un emploi. */
  readonly exclude = input<readonly DocumentKind[]>([]);
  /** Second rattachement proposé à l'envoi : le tiers d'une transaction (DOC-04). */
  readonly extra = input<{ target: LinkTarget; label: string } | null>(null);
  /** Vignettes plutôt que liste : les justificatifs d'une transaction (E7). */
  readonly thumbnails = input(false);
  readonly empty = input('Aucun document. Ajouter un contrat, une facture…');

  private readonly list = rxResource({
    params: () => ({ id: this.target().id, version: this.version.value() }),
    stream: ({ params }) => this.api.listDocuments({ link_id: params.id }),
  });

  protected readonly documents = computed(() =>
    (this.list.value() ?? []).filter((d) => !d.kind || !this.exclude().includes(d.kind)),
  );

  protected readonly icons = KIND_ICONS;
  protected readonly kindLabels = KIND_LABELS;
  protected readonly label = linkLabel;
  protected readonly dragging = signal(false);

  protected url(id: string, download = false): string {
    return documentUrl(id, download);
  }

  /** Les autres rattachements du document, en puces. */
  protected others(d: DocumentView) {
    return d.links.filter((l) => l.id !== this.target().id);
  }

  protected async add(file: File | null): Promise<void> {
    const created = await uploadDocument(this.dialog, {
      file,
      kind: this.defaultKind(),
      target: this.target(),
      extra: this.extra(),
    });
    if (created) this.version.bump();
  }

  protected choose(event: Event): void {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (file) void this.add(file);
  }

  protected async attachExisting(): Promise<void> {
    if (await attachDocuments(this.dialog, this.target())) this.version.bump();
  }

  protected async detach(d: DocumentView): Promise<void> {
    const target = this.target();
    try {
      await firstValueFrom(
        this.api.unlinkDocument({ id: d.id, kind: target.kind, target_id: target.id }),
      );
      this.version.bump();
      const undo = this.snack.open('Document détaché', 'Annuler', { duration: 5000 });
      undo.onAction().subscribe(async () => {
        await firstValueFrom(this.api.linkDocument({ id: d.id, body: target }));
        this.version.bump();
      });
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
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
}
