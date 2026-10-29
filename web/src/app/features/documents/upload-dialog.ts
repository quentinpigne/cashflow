import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { DocumentKind } from '../../api/models/document-kind';
import { DocumentView } from '../../api/models/document-view';
import { LinkTarget } from '../../api/models/link-target';
import { DocumentsService } from '../../api/services/documents.service';
import { currentMonth, shiftMonth, toIsoDate } from '../../core/dates';
import { errorMessage, problemOf } from '../../core/http-errors';
import { MONTH_NAMES } from '../salaries/salary-dialogs';
import {
  DOCUMENT_ACCEPT,
  DOCUMENT_KINDS,
  DOCUMENT_MAX_BYTES,
  KIND_LABELS,
  PDF_ACCEPT,
  hasMonth,
} from './document-labels';

export interface UploadDialogData {
  /** Fichier déjà choisi, par exemple déposé sur une section. */
  file?: File | null;
  /** Type proposé (R-DOC-15). */
  kind?: DocumentKind | null;
  /** Objet depuis lequel le document est envoyé : il y est rattaché d'office (R-DOC-15). */
  target?: LinkTarget | null;
  /** Second rattachement proposé, coché : le tiers d'une transaction (DOC-04). */
  extra?: { target: LinkTarget; label: string } | null;
  /** Le type ne se change pas : relevé envoyé depuis la section Relevés, fiche depuis Fiches de paie. */
  fixedKind?: boolean;
}

/** Envoi d'un document, depuis la bibliothèque ou la fiche d'un objet (DOC-01, R-DOC-14, R-DOC-15). */
@Component({
  selector: 'app-upload-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatDialogModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ heading }}</h2>
    <mat-dialog-content class="form">
      <label class="drop">
        <input type="file" [accept]="accept()" aria-label="Fichier" (change)="choose($event)" />
        <mat-icon>upload_file</mat-icon>
        <span>{{ file()?.name ?? 'Choisir un fichier (PDF, JPEG ou PNG, 10 Mo au plus)' }}</span>
      </label>
      @if (!data.fixedKind) {
        <mat-form-field>
          <mat-label>Type</mat-label>
          <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
            <mat-option [value]="null">À classer</mat-option>
            @for (k of kinds; track k) {
              <mat-option [value]="k">{{ labels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
      }
      @if (withMonth()) {
        <div class="pair">
          <mat-form-field>
            <mat-label>{{ kind() === 'PAYSLIP' ? 'Mois de paie' : 'Mois du relevé' }}</mat-label>
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
      } @else {
        <mat-form-field>
          <mat-label>Titre</mat-label>
          <input matInput maxlength="120" [ngModel]="title()" (ngModelChange)="title.set($event)" />
          <mat-hint>Par défaut, le nom du fichier.</mat-hint>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Date du document</mat-label>
          <input
            matInput
            [matDatepicker]="picker"
            [ngModel]="date()"
            (ngModelChange)="date.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="picker" />
          <mat-datepicker #picker />
          <mat-hint>Date du ticket, de la facture, de la signature…</mat-hint>
        </mat-form-field>
      }
      <mat-form-field>
        <mat-label>Note</mat-label>
        <input matInput [ngModel]="note()" (ngModelChange)="note.set($event)" />
      </mat-form-field>
      @if (data.extra; as extra) {
        <mat-checkbox [ngModel]="withExtra()" (ngModelChange)="withExtra.set($event)">
          Rattacher aussi à {{ extra.label }}
        </mat-checkbox>
      }
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
      @if (existing(); as id) {
        <div class="existing">
          <button mat-stroked-button (click)="open(id)">Ouvrir le document existant</button>
          @if (data.target) {
            <button mat-flat-button (click)="linkExisting(id)">Le rattacher ici</button>
          }
        </div>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!ready() || busy()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(30rem, 80vw);
    }
    .pair {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--field-gap);
    }
    .error {
      color: var(--mat-sys-error);
      margin: 0;
    }
    .existing {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
    }
    .drop {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      padding: 1.25rem;
      border: 2px dashed var(--mat-sys-outline-variant);
      border-radius: var(--mat-sys-corner-medium);
      cursor: pointer;
      overflow-wrap: anywhere;
      input {
        position: absolute;
        width: 1px;
        height: 1px;
        opacity: 0;
      }
    }
  `,
})
export class UploadDialog {
  protected readonly data = inject<UploadDialogData>(MAT_DIALOG_DATA);
  private readonly api = inject(DocumentsService);
  private readonly ref = inject(MatDialogRef<UploadDialog, DocumentView>);
  private readonly router = inject(Router);

  protected readonly kinds = DOCUMENT_KINDS;
  protected readonly labels = KIND_LABELS;
  protected readonly months = MONTH_NAMES;
  protected readonly heading =
    this.data.kind === 'BANK_STATEMENT'
      ? 'Nouveau relevé'
      : this.data.kind === 'PAYSLIP'
        ? 'Nouvelle fiche de paie'
        : 'Nouveau document';

  protected readonly kind = signal<DocumentKind | null>(this.data.kind ?? null);
  protected readonly withMonth = computed(() => hasMonth(this.kind()));
  protected readonly accept = computed(() =>
    this.kind() === 'BANK_STATEMENT' ? PDF_ACCEPT : DOCUMENT_ACCEPT,
  );
  /** Le mois précédent par défaut : un relevé ou une fiche arrive après le mois qu'il couvre. */
  private readonly initial = `${shiftMonth(currentMonth(), -1)}-01`;
  protected readonly month = signal(Number(this.initial.slice(5, 7)));
  protected readonly year = signal(Number(this.initial.slice(0, 4)));
  protected readonly title = signal('');
  protected readonly date = signal<Date | null>(null);
  protected readonly note = signal('');
  protected readonly withExtra = signal(true);
  protected readonly file = signal<File | null>(null);
  protected readonly error = signal<string | null>(null);
  /** Document qui a déjà ce fichier (R-DOC-3). */
  protected readonly existing = signal<string | null>(null);
  protected readonly busy = signal(false);

  constructor() {
    if (this.data.file) this.setFile(this.data.file);
  }

  private readonly monthIso = computed(() => {
    if (!this.withMonth()) return null;
    const year = Number(this.year());
    if (!Number.isInteger(year) || year < 1950 || year > 2100) return null;
    return `${year}-${String(this.month()).padStart(2, '0')}-01`;
  });

  protected readonly ready = computed(
    () => !!this.file() && (!this.withMonth() || !!this.monthIso()),
  );

  protected choose(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file) this.setFile(file);
  }

  private setFile(file: File): void {
    this.existing.set(null);
    if (file.size > DOCUMENT_MAX_BYTES) {
      this.file.set(null);
      this.error.set('Le fichier dépasse 10 Mo.');
      return;
    }
    this.file.set(file);
    this.error.set(null);
  }

  protected async save(): Promise<void> {
    const file = this.file();
    if (!file || !this.ready()) return;
    const target = this.data.target ?? null;
    const date = this.date();
    this.busy.set(true);
    this.existing.set(null);
    try {
      let created = await firstValueFrom(
        this.api.createDocument({
          body: {
            file,
            kind: this.kind(),
            title: this.withMonth() ? null : this.title().trim() || null,
            document_date: !this.withMonth() && date ? toIsoDate(date) : null,
            month: this.monthIso(),
            note: this.note().trim() || null,
            link_kind: target?.kind ?? null,
            link_id: target?.id ?? null,
          },
        }),
      );
      const extra = this.data.extra;
      if (extra && this.withExtra()) {
        created = await firstValueFrom(
          this.api.linkDocument({ id: created.id, body: extra.target }),
        );
      }
      this.ref.close(created);
    } catch (e) {
      this.error.set(errorMessage(e));
      const problem = problemOf(e);
      if (problem?.code === 'DOCUMENT_ALREADY_ARCHIVED' && problem.document_id) {
        this.existing.set(problem.document_id);
      }
    } finally {
      this.busy.set(false);
    }
  }

  protected open(id: string): void {
    this.ref.close();
    void this.router.navigate(['/documents', id]);
  }

  /** Rattache le document qui a déjà ce fichier à l'objet d'où l'on envoyait (R-DOC-3). */
  protected async linkExisting(id: string): Promise<void> {
    const target = this.data.target;
    if (!target) return;
    try {
      this.ref.close(await firstValueFrom(this.api.linkDocument({ id, body: target })));
    } catch (e) {
      this.existing.set(null);
      this.error.set(errorMessage(e));
    }
  }
}

/** Ouvre l'envoi d'un document ; résout au document archivé, ou à `undefined` si l'envoi est abandonné. */
export async function uploadDocument(
  dialog: MatDialog,
  data: UploadDialogData,
): Promise<DocumentView | undefined> {
  const ref = dialog.open<UploadDialog, UploadDialogData, DocumentView>(UploadDialog, {
    data,
    maxWidth: '100vw',
  });
  return firstValueFrom(ref.afterClosed());
}
