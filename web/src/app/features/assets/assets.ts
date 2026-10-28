import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
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
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { AssetKind } from '../../api/models/asset-kind';
import { AssetView } from '../../api/models/asset-view';
import { AssetsService } from '../../api/services/assets.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, todayIso, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { ASSET_KIND_LABELS } from '../../core/labels';
import { parseMoney, sum, toEditable } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';

const FORM_STYLES = `
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
    @media (max-width: 599.98px) {
      grid-template-columns: 1fr;
    }
  }
  .error {
    color: var(--mat-sys-error);
    margin: 0;
  }
`;

/** Création ou modification d'un bien (E26, R-BIE-1). */
@Component({
  selector: 'app-asset-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ asset ? 'Modifier le bien' : 'Nouveau bien' }}</h2>
    <mat-dialog-content class="form">
      <div class="pair">
        <mat-form-field>
          <mat-label>Nom</mat-label>
          <input matInput maxlength="80" [ngModel]="name()" (ngModelChange)="name.set($event)" />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Type</mat-label>
          <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
            @for (k of kinds; track k) {
              <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
      </div>
      <div class="pair">
        <mat-form-field>
          <mat-label>Date d'achat</mat-label>
          <input
            matInput
            [matDatepicker]="purchasePicker"
            [ngModel]="purchaseDate()"
            (ngModelChange)="purchaseDate.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="purchasePicker" />
          <mat-datepicker #purchasePicker />
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
          <mat-hint>Apport et capital emprunté, hors frais.</mat-hint>
        </mat-form-field>
      </div>
      @if (asset) {
        <div class="pair">
          <mat-form-field>
            <mat-label>Vendu le</mat-label>
            <input
              matInput
              [matDatepicker]="soldPicker"
              [ngModel]="soldOn()"
              (ngModelChange)="soldOn.set($event)"
            />
            <mat-datepicker-toggle matIconSuffix [for]="soldPicker" />
            <mat-datepicker #soldPicker />
          </mat-form-field>
          <mat-form-field>
            <mat-label>Prix de vente</mat-label>
            <input
              matInput
              inputmode="decimal"
              [ngModel]="salePrice()"
              (ngModelChange)="salePrice.set($event)"
            />
            <span matTextSuffix>€</span>
          </mat-form-field>
        </div>
      }
      <mat-checkbox [ngModel]="netWorth()" (ngModelChange)="netWorth.set($event)">
        Compter dans le patrimoine
      </mat-checkbox>
      <mat-form-field>
        <mat-label>Note</mat-label>
        <textarea
          matInput
          rows="2"
          [ngModel]="notes()"
          (ngModelChange)="notes.set($event)"
        ></textarea>
      </mat-form-field>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!body()" (click)="save()">
        {{ asset ? 'Enregistrer' : 'Créer le bien' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: FORM_STYLES,
})
export class AssetDialog {
  protected readonly asset = inject<AssetView | null>(MAT_DIALOG_DATA);
  private readonly api = inject(AssetsService);
  private readonly ref = inject(MatDialogRef<AssetDialog>);

  protected readonly kinds: AssetKind[] = ['REAL_ESTATE', 'VEHICLE', 'OTHER'];
  protected readonly kindLabels = ASSET_KIND_LABELS;
  private readonly s = this.asset?.summary;
  protected readonly name = signal(this.s?.name ?? '');
  protected readonly kind = signal<AssetKind>(this.s?.kind ?? 'REAL_ESTATE');
  protected readonly purchaseDate = signal<Date | null>(
    this.s ? fromIsoDate(this.s.purchase_date) : fromIsoDate(todayIso()),
  );
  protected readonly price = signal(this.s ? toEditable(this.s.purchase_price_minor) : '');
  protected readonly soldOn = signal<Date | null>(
    this.s?.sold_on ? fromIsoDate(this.s.sold_on) : null,
  );
  protected readonly salePrice = signal(
    this.asset?.sale_price_minor != null ? toEditable(this.asset.sale_price_minor) : '',
  );
  protected readonly netWorth = signal(this.s?.include_in_net_worth ?? true);
  protected readonly notes = signal(this.asset?.notes ?? '');
  protected readonly error = signal<string | null>(null);

  protected readonly body = computed(() => {
    const name = this.name().trim();
    const price = parseMoney(this.price());
    const date = this.purchaseDate();
    const sale = this.salePrice().trim() ? parseMoney(this.salePrice()) : null;
    if (!name || price === null || price <= 0 || !date) return null;
    if (this.salePrice().trim() && sale === null) return null;
    const sold = this.soldOn();
    return {
      name,
      kind: this.kind(),
      purchase_date: toIsoDate(date),
      purchase_price_minor: price,
      sold_on: sold ? toIsoDate(sold) : null,
      sale_price_minor: sold ? sale : null,
      include_in_net_worth: this.netWorth(),
      notes: this.notes().trim() || null,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    try {
      const saved = this.asset
        ? await firstValueFrom(this.api.updateAsset({ id: this.asset.summary.id, body }))
        : await firstValueFrom(this.api.createAsset({ body }));
      this.ref.close(saved.summary.id);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Estimation datée d'un bien (R-BIE-3). */
@Component({
  selector: 'app-asset-valuation-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Nouvelle estimation</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Date</mat-label>
        <input
          matInput
          [matDatepicker]="picker"
          [ngModel]="date()"
          (ngModelChange)="date.set($event)"
        />
        <mat-datepicker-toggle matIconSuffix [for]="picker" />
        <mat-datepicker #picker />
      </mat-form-field>
      <mat-form-field>
        <mat-label>Valeur du bien</mat-label>
        <input
          matInput
          inputmode="decimal"
          [ngModel]="value()"
          (ngModelChange)="value.set($event)"
        />
        <span matTextSuffix>€</span>
      </mat-form-field>
      <mat-form-field>
        <mat-label>Note</mat-label>
        <input matInput [ngModel]="note()" (ngModelChange)="note.set($event)" />
      </mat-form-field>
      @if (error()) {
        <p class="error" role="alert">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="!valid()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: FORM_STYLES,
})
export class AssetValuationDialog {
  private readonly assetId = inject<string>(MAT_DIALOG_DATA);
  private readonly api = inject(AssetsService);
  private readonly ref = inject(MatDialogRef<AssetValuationDialog>);
  protected readonly date = signal<Date | null>(fromIsoDate(todayIso()));
  protected readonly value = signal('');
  protected readonly note = signal('');
  protected readonly error = signal<string | null>(null);
  protected readonly valid = computed(() => {
    const amount = parseMoney(this.value());
    return !!this.date() && amount !== null && amount > 0;
  });

  protected async save(): Promise<void> {
    const date = this.date();
    const amount = parseMoney(this.value());
    if (!date || amount === null) return;
    try {
      await firstValueFrom(
        this.api.saveAssetValuation({
          id: this.assetId,
          body: { date: toIsoDate(date), value_minor: amount, note: this.note().trim() || null },
        }),
      );
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** E25 — Biens (BIE-01). */
@Component({
  selector: 'app-assets-page',
  imports: [
    RouterLink,
    MatButtonModule,
    MatExpansionModule,
    MatIconModule,
    MatProgressBarModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Biens</h1>
        <button mat-flat-button (click)="create()"><mat-icon>add</mat-icon>Nouveau bien</button>
      </div>
      @if (assets.isLoading() && !assets.hasValue()) {
        <mat-progress-bar mode="indeterminate" />
      } @else if (held().length === 0 && sold().length === 0) {
        <div class="empty">
          <p>Aucun bien.</p>
          <button mat-stroked-button (click)="create()">Créer un bien</button>
        </div>
      } @else {
        <section class="card group">
          <header>
            <h2>Possédés</h2>
            <span class="amount">{{ total() | money }}</span>
          </header>
          @for (a of held(); track a.id) {
            <a class="row" [routerLink]="['/biens', a.id]">
              <span class="name">
                <span>{{ a.name }}</span>
                <span class="muted small"
                  >{{ kindLabels[a.kind] }} · acheté le
                  {{ a.purchase_date | frDate: 'short' }}</span
                >
              </span>
              <span class="amount">{{ a.value_minor ?? 0 | money }}</span>
            </a>
          }
        </section>
        @if (sold().length > 0) {
          <mat-accordion>
            <mat-expansion-panel>
              <mat-expansion-panel-header>
                <mat-panel-title>Biens vendus ({{ sold().length }})</mat-panel-title>
              </mat-expansion-panel-header>
              @for (a of sold(); track a.id) {
                <a class="row" [routerLink]="['/biens', a.id]">
                  <span class="name">{{ a.name }}</span>
                  <span class="muted small">Vendu le {{ a.sold_on! | frDate: 'short' }}</span>
                </a>
              }
            </mat-expansion-panel>
          </mat-accordion>
        }
      }
    </div>
  `,
  styleUrl: '../accounts/accounts.scss',
})
export class AssetsPage {
  private readonly api = inject(AssetsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);
  protected readonly kindLabels = ASSET_KIND_LABELS;

  protected readonly assets = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listAssets(),
  });
  protected readonly held = computed(() => (this.assets.value() ?? []).filter((a) => !a.sold_on));
  protected readonly sold = computed(() => (this.assets.value() ?? []).filter((a) => a.sold_on));
  protected readonly total = computed(() => sum(this.held().map((a) => a.value_minor ?? 0)));

  protected async create(): Promise<void> {
    const ref = this.dialog.open(AssetDialog, { data: null, maxWidth: '100vw' });
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.version.bump();
      await this.router.navigate(['/biens', id]);
    }
  }
}

/** E26 — Fiche d'un bien (BIE-01, R-PRE-10). */
@Component({
  selector: 'app-asset-page',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatMenuModule,
    MatProgressBarModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      @if (asset.value(); as a) {
        <header class="head">
          <div class="page-header">
            <a mat-icon-button routerLink="/biens" aria-label="Retour aux biens"
              ><mat-icon>arrow_back</mat-icon></a
            >
            <h1>{{ a.summary.name }}</h1>
            <button mat-icon-button [matMenuTriggerFor]="menu" aria-label="Actions sur le bien">
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              <button mat-menu-item (click)="edit(a)"><mat-icon>edit</mat-icon>Modifier</button>
              <button mat-menu-item (click)="remove(a)">
                <mat-icon>delete</mat-icon>Supprimer
              </button>
            </mat-menu>
          </div>
          <p class="muted meta">
            {{ kindLabels[a.summary.kind] }} · acheté le {{ a.summary.purchase_date | frDate }}
            @if (a.summary.sold_on) {
              · vendu le {{ a.summary.sold_on | frDate }}
            }
          </p>
          <div class="balance">
            <span class="muted">Valeur</span>
            <strong class="amount">{{ a.summary.value_minor ?? 0 | money }}</strong>
          </div>
        </header>

        <section class="card panel">
          <dl class="figures">
            <dt>Prix d'achat</dt>
            <dd class="amount">{{ a.summary.purchase_price_minor | money }}</dd>
            @if (a.sale_price_minor !== null && a.sale_price_minor !== undefined) {
              <dt>Prix de vente</dt>
              <dd class="amount">{{ a.sale_price_minor | money }}</dd>
            }
            @for (l of a.loans; track l.id) {
              <dt>
                Prêt <a [routerLink]="['/prets', l.id]">{{ l.name }}</a>
              </dt>
              <dd class="amount negative">{{ l.outstanding_minor ?? 0 | money }}</dd>
            }
            @if (
              a.loans.length > 0 && a.net_value_minor !== null && a.net_value_minor !== undefined
            ) {
              <dt><strong>Valeur nette</strong></dt>
              <dd class="amount">
                <strong>{{ a.net_value_minor | money }}</strong>
              </dd>
            }
          </dl>
          @if (a.notes) {
            <p class="muted">{{ a.notes }}</p>
          }
        </section>

        <section class="card panel">
          <header>
            <h2>Estimations</h2>
            <button mat-stroked-button (click)="addValuation()">
              <mat-icon>add</mat-icon>Ajouter
            </button>
          </header>
          @for (v of a.valuations; track v.id) {
            <div class="row">
              <span>{{ v.date | frDate }}</span>
              <span class="muted small">{{ v.note }}</span>
              <span class="amount">{{ v.value_minor | money }}</span>
              <button
                mat-icon-button
                [attr.aria-label]="'Supprimer l’estimation du ' + v.date"
                (click)="removeValuation(v.id)"
              >
                <mat-icon>delete</mat-icon>
              </button>
            </div>
          } @empty {
            <p class="muted">Aucune estimation : le bien vaut son prix d'achat.</p>
          }
        </section>

        @if (a.acquisitions.length > 0) {
          <section class="card panel">
            <header>
              <h2>Acquisition</h2>
              <span class="amount">{{ a.acquisitions_total_minor | money }}</span>
            </header>
            @for (p of a.acquisitions; track p.transaction_id) {
              <a class="row link" [routerLink]="['/transactions', p.transaction_id]">
                <span>{{ p.date | frDate: 'short' }}</span>
                <span>{{ p.label }}</span>
                <span class="amount">{{ p.amount_minor | money }}</span>
              </a>
            }
          </section>
        }
      } @else if (asset.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      } @else {
        <p class="empty">Bien introuvable.</p>
      }
    </div>
  `,
  styleUrl: '../loans/loan-page.scss',
})
export class AssetPage {
  private readonly api = inject(AssetsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  protected readonly kindLabels = ASSET_KIND_LABELS;

  readonly id = input.required<string>();

  protected readonly asset = rxResource({
    params: () => ({ id: this.id(), version: this.version.value() }),
    stream: ({ params }) => this.api.getAsset({ id: params.id }),
  });

  protected async edit(a: AssetView): Promise<void> {
    const ref = this.dialog.open(AssetDialog, { data: a, maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async remove(a: AssetView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le bien ?',
      message:
        'Ses estimations sont supprimées ; ses prêts perdent le lien, ses acquisitions redeviennent non catégorisées.',
      action: 'Supprimer',
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteAsset({ id: a.summary.id }));
      this.version.bump();
      await this.router.navigate(['/biens']);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  protected async addValuation(): Promise<void> {
    const ref = this.dialog.open(AssetValuationDialog, { data: this.id() });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async removeValuation(valuationId: string): Promise<void> {
    try {
      await firstValueFrom(
        this.api.deleteAssetValuation({ id: this.id(), valuation_id: valuationId }),
      );
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }
}
