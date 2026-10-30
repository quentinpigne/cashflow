import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
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
import { MatStepperModule } from '@angular/material/stepper';
import { firstValueFrom } from 'rxjs';

import { ApplicationView } from '../../api/models/application-view';
import { AssetKind } from '../../api/models/asset-kind';
import { DocumentKind } from '../../api/models/document-kind';
import { SimulationView } from '../../api/models/simulation-view';
import { AssetsService } from '../../api/services/assets.service';
import { CreditService } from '../../api/services/credit.service';
import { DocumentsService } from '../../api/services/documents.service';
import { SystemService } from '../../api/services/system.service';
import { DataVersion } from '../../core/data-version';
import { toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { parseMoney, toEditable } from '../../core/money';
import { FeaturesStore } from '../../shared/stores';
import { KIND_LABELS } from '../documents/document-labels';
import { uploadDocument } from '../documents/upload-dialog';
import { LoanDialog } from '../loans/loans';
import { applicationInput } from './applications';

export interface AcceptanceData {
  application: ApplicationView;
  offer: SimulationView;
}

/** Destination d'un document du dossier (R-CRE-27). */
type Destination = 'ASSET' | 'LOAN';

const LOAN_KINDS: DocumentKind[] = ['LOAN_OFFER', 'AMORTIZATION_SCHEDULE', 'INSURANCE'];
const ASSET_KINDS: DocumentKind[] = ['SALE_AGREEMENT', 'DEED', 'QUOTE', 'DIAGNOSTIC', 'WARRANTY'];

interface Candidate {
  id: string;
  title: string;
  label: string;
  destination: Destination;
}

/** E41 — Assistant d'acceptation : le bien, le prêt, puis les documents du dossier (CRE-10, R-CRE-27). */
@Component({
  selector: 'app-acceptance-dialog',
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
    MatStepperModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Offre acceptée : créer le bien et le prêt</h2>
    <mat-dialog-content>
      <mat-stepper orientation="vertical" [linear]="false" #stepper>
        <mat-step label="Bien" [completed]="!!assetId()">
          @if (data.application.asset_id && data.application.asset_name) {
            <p>Le prêt financera « {{ data.application.asset_name }} ».</p>
            <button mat-flat-button matStepperNext>Suivant</button>
          } @else if (assetId()) {
            <p>Bien créé : « {{ assetName() }} ».</p>
            <button mat-flat-button matStepperNext>Suivant</button>
          } @else {
            <div class="form">
              <mat-form-field>
                <mat-label>Nom</mat-label>
                <input
                  matInput
                  maxlength="80"
                  [ngModel]="assetName()"
                  (ngModelChange)="assetName.set($event)"
                />
              </mat-form-field>
              <div class="pair">
                <mat-form-field>
                  <mat-label>Type</mat-label>
                  <mat-select [ngModel]="assetKind()" (ngModelChange)="assetKind.set($event)">
                    <mat-option value="REAL_ESTATE">Bien immobilier</mat-option>
                    <mat-option value="VEHICLE">Véhicule</mat-option>
                    <mat-option value="OTHER">Autre bien</mat-option>
                  </mat-select>
                </mat-form-field>
                <mat-form-field>
                  <mat-label>Prix d'achat</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="price()"
                    (ngModelChange)="price.set($event)"
                  />
                  <span matTextSuffix>€</span>
                </mat-form-field>
              </div>
              <mat-form-field>
                <mat-label>Date d'achat</mat-label>
                <input
                  matInput
                  [matDatepicker]="purchasePicker"
                  [ngModel]="purchase()"
                  (ngModelChange)="purchase.set($event)"
                />
                <mat-datepicker-toggle matIconSuffix [for]="purchasePicker" />
                <mat-datepicker #purchasePicker />
                <mat-hint>Signature de l'acte</mat-hint>
              </mat-form-field>
              @if (!features.assets()) {
                <p class="muted small">La rubrique Biens sera activée.</p>
              }
            </div>
            <div class="actions">
              <button mat-flat-button [disabled]="!assetValid() || busy()" (click)="createAsset()">
                Créer le bien
              </button>
              <button mat-button matStepperNext>Passer</button>
            </div>
          }
        </mat-step>

        <mat-step label="Prêt" [completed]="!!loanId()">
          @if (loanId() || data.offer.loan_id) {
            <p>Prêt créé à partir de l'offre « {{ data.offer.name }} ».</p>
            <button mat-flat-button matStepperNext>Suivant</button>
          } @else {
            <p>
              Le prêt reprend les conditions de l'offre « {{ data.offer.name }} »
              @if (assetId()) {
                et finance « {{ assetName() }} »
              }
              ; vous saisissez le déblocage, la première échéance et le compte qui paie.
            </p>
            <div class="actions">
              <button mat-flat-button (click)="createLoan()">Créer le prêt</button>
              <button mat-button matStepperNext>Passer</button>
            </div>
          }
        </mat-step>

        <mat-step label="Documents">
          @if (candidates().length === 0) {
            <p class="muted">Aucun document du dossier à ranger.</p>
          }
          @for (c of candidates(); track c.id) {
            <mat-checkbox
              [checked]="chosen().has(c.id)"
              [disabled]="!target(c.destination)"
              (change)="toggle(c.id)"
            >
              {{ c.title }}
              <span class="muted small"
                >{{ c.label }} → {{ c.destination === 'LOAN' ? 'prêt' : 'bien' }}</span
              >
            </mat-checkbox>
          }
          <div class="actions">
            @if (loanId()) {
              <button mat-stroked-button (click)="uploadOffer()">
                <mat-icon>upload_file</mat-icon>Envoyer l'offre de prêt
              </button>
            }
            <button mat-flat-button [disabled]="busy()" (click)="finish()">Terminer</button>
          </div>
        </mat-step>
      </mat-stepper>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      margin-top: 0.5rem;
    }
    .pair {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--field-gap);
    }
    .actions {
      display: flex;
      gap: 0.5rem;
      flex-wrap: wrap;
      margin-top: 0.75rem;
    }
    mat-checkbox {
      display: block;
    }
    .small {
      font: var(--mat-sys-body-small);
      margin-left: 0.25rem;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class AcceptanceDialog {
  protected readonly data = inject<AcceptanceData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<AcceptanceDialog, string | null>);
  private readonly dialog = inject(MatDialog);
  private readonly credit = inject(CreditService);
  private readonly assets = inject(AssetsService);
  private readonly documents = inject(DocumentsService);
  private readonly system = inject(SystemService);
  private readonly version = inject(DataVersion);
  protected readonly features = inject(FeaturesStore);

  private readonly a = this.data.application;
  protected readonly assetId = signal<string | null>(this.a.asset_id ?? null);
  protected readonly assetName = signal(this.a.asset_name ?? this.a.name);
  protected readonly assetKind = signal<AssetKind>(
    this.a.project_kind === 'VEHICLE'
      ? 'VEHICLE'
      : this.a.project_kind === 'OTHER'
        ? 'OTHER'
        : 'REAL_ESTATE',
  );
  protected readonly price = signal(this.a.price_minor ? toEditable(this.a.price_minor) : '');
  protected readonly purchase = signal<Date | null>(null);
  protected readonly loanId = signal<string | null>(this.data.offer.loan_id ?? null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly assetValid = computed(() => {
    const price = parseMoney(this.price());
    return !!this.assetName().trim() && !!this.purchase() && price !== null && price > 0;
  });

  /** Documents sans rattachement des types du dossier, et pièces de la demande (R-CRE-27). */
  private readonly loose = rxResource({
    params: () => true,
    stream: () => this.documents.listDocuments({ unlinked: true }),
  });

  protected readonly candidates = computed<Candidate[]>(() => {
    const found = new Map<string, Candidate>();
    for (const p of this.a.pieces) {
      if (p.piece !== 'SALE_AGREEMENT' && p.piece !== 'QUOTES') continue;
      for (const d of p.documents) {
        found.set(d.id, { id: d.id, title: d.title, label: p.label, destination: 'ASSET' });
      }
    }
    for (const d of this.loose.value() ?? []) {
      if (!d.kind || found.has(d.id)) continue;
      const destination: Destination | null = LOAN_KINDS.includes(d.kind)
        ? 'LOAN'
        : ASSET_KINDS.includes(d.kind)
          ? 'ASSET'
          : null;
      if (destination)
        found.set(d.id, { id: d.id, title: d.title, label: KIND_LABELS[d.kind], destination });
    }
    return [...found.values()];
  });

  /** Documents cochés : tous par défaut. */
  private readonly unchecked = signal<ReadonlySet<string>>(new Set());
  protected readonly chosen = computed(
    () =>
      new Set(
        this.candidates()
          .map((c) => c.id)
          .filter((id) => !this.unchecked().has(id)),
      ),
  );

  protected toggle(id: string): void {
    this.unchecked.update((set) => {
      const next = new Set(set);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  protected target(destination: Destination): string | null {
    return destination === 'LOAN' ? this.loanId() : this.assetId();
  }

  protected async createAsset(): Promise<void> {
    const purchase = this.purchase();
    const price = parseMoney(this.price());
    if (!purchase || price === null) return;
    this.busy.set(true);
    try {
      if (!this.features.assets()) {
        await firstValueFrom(this.system.updateFeatures({ body: { assets: true } }));
      }
      const asset = await firstValueFrom(
        this.assets.createAsset({
          body: {
            name: this.assetName().trim(),
            kind: this.assetKind(),
            purchase_date: toIsoDate(purchase),
            purchase_price_minor: price,
            sold_on: null,
            sale_price_minor: null,
            include_in_net_worth: true,
            notes: null,
          },
        }),
      );
      // La demande est rattachée au bien (R-CRE-27).
      await firstValueFrom(
        this.credit.updateLoanApplication({
          id: this.a.id,
          body: applicationInput(this.a, { asset_id: asset.summary.id }),
        }),
      );
      this.assetId.set(asset.summary.id);
      this.version.bump();
      this.error.set(null);
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.busy.set(false);
    }
  }

  protected async createLoan(): Promise<void> {
    const ref = this.dialog.open(LoanDialog, {
      data: { simulation: this.data.offer, assetId: this.assetId() },
      maxWidth: '100vw',
    });
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.loanId.set(id);
      this.version.bump();
    }
  }

  protected async uploadOffer(): Promise<void> {
    const loan = this.loanId();
    if (!loan) return;
    const created = await uploadDocument(this.dialog, {
      kind: 'LOAN_OFFER',
      target: { kind: 'LOAN', id: loan },
    });
    if (created) this.version.bump();
  }

  /** Rattache les documents cochés à leur destination, puis ferme sur le prêt (R-CRE-27). */
  protected async finish(): Promise<void> {
    this.busy.set(true);
    try {
      for (const c of this.candidates()) {
        const target = this.target(c.destination);
        if (!target || !this.chosen().has(c.id)) continue;
        await firstValueFrom(
          this.documents.linkDocument({
            id: c.id,
            body: { kind: c.destination === 'LOAN' ? 'LOAN' : 'ASSET', id: target },
          }),
        );
      }
      this.version.bump();
      this.ref.close(this.loanId());
    } catch (e) {
      this.error.set(errorMessage(e));
    } finally {
      this.busy.set(false);
    }
  }
}

/** Ouvre l'assistant d'acceptation ; résout au prêt créé, s'il y en a un. */
export async function acceptOffer(dialog: MatDialog, data: AcceptanceData): Promise<string | null> {
  const ref = dialog.open<AcceptanceDialog, AcceptanceData, string | null>(AcceptanceDialog, {
    data,
    maxWidth: '100vw',
    width: '40rem',
    disableClose: true,
  });
  return (await firstValueFrom(ref.afterClosed())) ?? null;
}
