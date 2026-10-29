import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { DomSanitizer } from '@angular/platform-browser';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { DocumentKind } from '../../api/models/document-kind';
import { DocumentView } from '../../api/models/document-view';
import { LinkView } from '../../api/models/link-view';
import { DocumentsService } from '../../api/services/documents.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MONTH_NAMES } from '../salaries/salary-dialogs';
import {
  DOCUMENT_KINDS,
  KIND_LABELS,
  LINK_ICONS,
  LINK_LABELS,
  documentUrl,
  formatSize,
  hasMonth,
  linkLabel,
  linkRoute,
} from './document-labels';
import { pickLinkTarget } from './link-dialogs';

/** E35 — Fiche d'un document (DOC-01, DOC-02). */
@Component({
  selector: 'app-document-page',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatSelectModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/documents" aria-label="Retour aux documents"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>{{ doc()?.title ?? 'Document' }}</h1>
        @if (doc(); as d) {
          <a mat-stroked-button [href]="url(d.id)" target="_blank" rel="noopener">
            <mat-icon>open_in_new</mat-icon>Ouvrir
          </a>
          <button mat-icon-button [matMenuTriggerFor]="menu" aria-label="Actions sur le document">
            <mat-icon>more_vert</mat-icon>
          </button>
          <mat-menu #menu>
            <a mat-menu-item [href]="url(d.id, true)" download>
              <mat-icon>download</mat-icon>Télécharger
            </a>
            <button mat-menu-item (click)="remove(d)"><mat-icon>delete</mat-icon>Supprimer</button>
          </mat-menu>
        }
      </div>

      @if (doc(); as d) {
        <div class="layout">
          <section class="card preview">
            @if (d.media_type === 'application/pdf') {
              <iframe [src]="frame()" [title]="d.title"></iframe>
            } @else {
              <img [src]="url(d.id)" [alt]="d.title" />
            }
          </section>

          <div class="side">
            <section class="card form">
              <h2>Description</h2>
              <mat-form-field>
                <mat-label>Type</mat-label>
                <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
                  <mat-option [value]="null">À classer</mat-option>
                  @for (k of kinds; track k) {
                    <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Titre</mat-label>
                <input
                  matInput
                  maxlength="120"
                  [ngModel]="title()"
                  (ngModelChange)="title.set($event)"
                />
              </mat-form-field>
              @if (withMonth()) {
                <div class="pair">
                  <mat-form-field>
                    <mat-label>{{
                      kind() === 'PAYSLIP' ? 'Mois de paie' : 'Mois du relevé'
                    }}</mat-label>
                    <mat-select [ngModel]="month()" (ngModelChange)="month.set($event)">
                      @for (m of months; track $index) {
                        <mat-option [value]="$index + 1">{{ m }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                  <mat-form-field>
                    <mat-label>Année</mat-label>
                    <input
                      matInput
                      type="number"
                      min="1950"
                      max="2100"
                      [ngModel]="year()"
                      (ngModelChange)="year.set($event)"
                    />
                  </mat-form-field>
                </div>
              }
              <mat-form-field>
                <mat-label>Date du document</mat-label>
                <input
                  matInput
                  [matDatepicker]="datePicker"
                  [ngModel]="date()"
                  (ngModelChange)="date.set($event)"
                />
                <mat-datepicker-toggle matIconSuffix [for]="datePicker" />
                <mat-datepicker #datePicker />
              </mat-form-field>
              <div class="pair">
                <mat-form-field>
                  <mat-label>Début de la période</mat-label>
                  <input
                    matInput
                    [matDatepicker]="startPicker"
                    [ngModel]="start()"
                    (ngModelChange)="start.set($event)"
                  />
                  <mat-datepicker-toggle matIconSuffix [for]="startPicker" />
                  <mat-datepicker #startPicker />
                </mat-form-field>
                <mat-form-field>
                  <mat-label>Fin</mat-label>
                  <input
                    matInput
                    [matDatepicker]="endPicker"
                    [ngModel]="end()"
                    (ngModelChange)="end.set($event)"
                  />
                  <mat-datepicker-toggle matIconSuffix [for]="endPicker" />
                  <mat-datepicker #endPicker />
                  <mat-hint>Durée d'un contrat, validité d'une garantie…</mat-hint>
                </mat-form-field>
              </div>
              <mat-form-field>
                <mat-label>Note</mat-label>
                <textarea
                  matInput
                  rows="2"
                  [ngModel]="note()"
                  (ngModelChange)="note.set($event)"
                ></textarea>
              </mat-form-field>
              @if (error()) {
                <p class="error" role="alert">{{ error() }}</p>
              }
              <div class="save">
                <button mat-flat-button [disabled]="!dirty() || busy()" (click)="save(d)">
                  Enregistrer
                </button>
              </div>
              <p class="muted small">
                {{ d.file_name }} · {{ size(d.size_bytes) }} · envoyé le {{ d.sent_on | frDate }} ·
                {{ d.content_indexed ? 'contenu indexé' : 'contenu non indexé' }}
              </p>
            </section>

            <section class="card">
              <header>
                <h2>Rattachements</h2>
                <button mat-stroked-button (click)="link(d)">
                  <mat-icon>add_link</mat-icon>Rattacher à…
                </button>
              </header>
              <div class="chips">
                @for (l of d.links; track l.id) {
                  <span class="chip">
                    <mat-icon>{{ linkIcons[l.kind] }}</mat-icon>
                    <span class="muted">{{ linkLabels[l.kind] }}</span>
                    @if (route(l); as r) {
                      <a [routerLink]="r">{{ label(l) }}</a>
                    } @else {
                      {{ label(l) }}
                    }
                    <button
                      mat-icon-button
                      class="remove"
                      [attr.aria-label]="'Détacher de ' + l.name"
                      (click)="unlink(d, l)"
                    >
                      <mat-icon>close</mat-icon>
                    </button>
                  </span>
                } @empty {
                  <p class="muted">
                    Sans rattachement. Rattachez-le à un compte, une transaction, un tiers…
                  </p>
                }
              </div>
            </section>
          </div>
        </div>
      } @else if (resource.error()) {
        <p class="error">{{ message(resource.error()) }}</p>
      }
    </div>
  `,
  styles: `
    .layout {
      display: grid;
      grid-template-columns: minmax(0, 3fr) minmax(18rem, 2fr);
      gap: 1rem;
      align-items: start;
      @media (max-width: 899.98px) {
        grid-template-columns: minmax(0, 1fr);
      }
    }
    .preview {
      padding: 0;
      overflow: hidden;
      iframe {
        display: block;
        width: 100%;
        height: 75vh;
        border: 0;
      }
      img {
        display: block;
        max-width: 100%;
        margin: auto;
      }
    }
    .side {
      display: flex;
      flex-direction: column;
      gap: 1rem;
    }
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
    }
    h2 {
      margin: 0;
      font: var(--mat-sys-title-medium);
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .pair {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--field-gap);
    }
    .save {
      display: flex;
      justify-content: flex-end;
    }
    .small {
      font: var(--mat-sys-body-small);
      margin: 0;
    }
    .error {
      color: var(--mat-sys-error);
      margin: 0;
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
      margin-top: 0.75rem;
    }
    .chip {
      display: inline-flex;
      align-items: center;
      gap: 0.375rem;
      padding: 0 0 0 0.75rem;
      border-radius: 1rem;
      background: var(--mat-sys-surface-container-high);
      mat-icon {
        font-size: 1.125rem;
        width: 1.125rem;
        height: 1.125rem;
      }
      .remove {
        --mat-icon-button-state-layer-size: 2rem;
        padding: 0.25rem;
      }
    }
  `,
})
export class DocumentPage {
  private readonly api = inject(DocumentsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  private readonly sanitizer = inject(DomSanitizer);

  /** Identifiant du document, depuis la route. */
  readonly id = input.required<string>();

  protected readonly resource = rxResource({
    params: () => ({ id: this.id(), version: this.version.value() }),
    stream: ({ params }) => this.api.getDocument({ id: params.id }),
  });
  protected readonly doc = computed(() =>
    this.resource.hasValue() ? this.resource.value() : undefined,
  );

  protected readonly kinds = DOCUMENT_KINDS;
  protected readonly kindLabels = KIND_LABELS;
  protected readonly linkIcons = LINK_ICONS;
  protected readonly linkLabels = LINK_LABELS;
  protected readonly months = MONTH_NAMES;
  protected readonly size = formatSize;
  protected readonly label = linkLabel;
  protected readonly route = linkRoute;
  protected readonly message = errorMessage;

  protected readonly kind = signal<DocumentKind | null>(null);
  protected readonly title = signal('');
  protected readonly month = signal(1);
  protected readonly year = signal(2000);
  protected readonly date = signal<Date | null>(null);
  protected readonly start = signal<Date | null>(null);
  protected readonly end = signal<Date | null>(null);
  protected readonly note = signal('');
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);

  protected readonly withMonth = computed(() => hasMonth(this.kind()));

  /** L'aperçu PDF, servi par l'application sous `sandbox` (ADR-0024). */
  protected readonly frame = computed(() =>
    this.sanitizer.bypassSecurityTrustResourceUrl(documentUrl(this.id())),
  );

  constructor() {
    // Le formulaire reprend le document à chaque chargement.
    effect(() => {
      const d = this.doc();
      if (!d) return;
      this.kind.set(d.kind ?? null);
      this.title.set(d.title);
      const month = d.month ?? d.reference_date;
      this.month.set(Number(month.slice(5, 7)));
      this.year.set(Number(month.slice(0, 4)));
      this.date.set(d.document_date ? fromIsoDate(d.document_date) : null);
      this.start.set(d.period_start ? fromIsoDate(d.period_start) : null);
      this.end.set(d.period_end ? fromIsoDate(d.period_end) : null);
      this.note.set(d.note ?? '');
      this.error.set(null);
    });
  }

  private readonly input = computed(() => {
    const year = Number(this.year());
    const month =
      this.withMonth() && Number.isInteger(year)
        ? `${year}-${String(this.month()).padStart(2, '0')}-01`
        : null;
    const iso = (d: Date | null) => (d ? toIsoDate(d) : null);
    return {
      kind: this.kind(),
      title: this.title().trim(),
      month,
      document_date: iso(this.date()),
      period_start: iso(this.start()),
      period_end: iso(this.end()),
      note: this.note().trim() || null,
    };
  });

  protected readonly dirty = computed(() => {
    const d = this.doc();
    if (!d) return false;
    const i = this.input();
    return (
      i.kind !== (d.kind ?? null) ||
      i.title !== d.title ||
      i.month !== (d.month ?? null) ||
      i.document_date !== (d.document_date ?? null) ||
      i.period_start !== (d.period_start ?? null) ||
      i.period_end !== (d.period_end ?? null) ||
      i.note !== (d.note ?? null)
    );
  });

  protected url(id: string, download = false): string {
    return documentUrl(id, download);
  }

  protected async save(d: DocumentView): Promise<void> {
    this.busy.set(true);
    try {
      await firstValueFrom(this.api.updateDocument({ id: d.id, body: this.input() }));
      this.version.bump();
      this.snack.open('Document enregistré', undefined, { duration: 3000 });
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.busy.set(false);
    }
  }

  protected async link(d: DocumentView): Promise<void> {
    const target = await pickLinkTarget(this.dialog);
    if (!target) return;
    try {
      await firstValueFrom(this.api.linkDocument({ id: d.id, body: target }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  protected async unlink(d: DocumentView, l: LinkView): Promise<void> {
    try {
      await firstValueFrom(this.api.unlinkDocument({ id: d.id, kind: l.kind, target_id: l.id }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  /** Supprime le document après une confirmation qui liste ses rattachements (R-DOC-22). */
  protected async remove(d: DocumentView): Promise<void> {
    const links = d.links.map((l) => linkLabel(l)).join(', ');
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${d.title} » ?`,
      message: links
        ? `Il sera détaché de : ${links}, et ne sera plus consultable.`
        : 'Il ne sera plus consultable.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteDocument({ id: d.id }));
      this.version.bump();
      await this.router.navigate(['/documents']);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }
}
