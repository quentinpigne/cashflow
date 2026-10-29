import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
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
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ApplicationInput } from '../../api/models/application-input';
import { ApplicationView } from '../../api/models/application-view';
import { LenderRequestInput } from '../../api/models/lender-request-input';
import { LenderRequestView } from '../../api/models/lender-request-view';
import { ProjectKind } from '../../api/models/project-kind';
import { AssetsService } from '../../api/services/assets.service';
import { CreditService } from '../../api/services/credit.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, todayIso, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { parseMoney, toEditable } from '../../core/money';
import { MoneyPipe } from '../../shared/money.pipe';
import { FeaturesStore, InstitutionsStore } from '../../shared/stores';
import {
  APPLICATION_STATUS_LABELS,
  LENDER_STATUSES,
  LENDER_STATUS_LABELS,
  LoansTabs,
  PROJECT_KINDS,
  PROJECT_KIND_LABELS,
} from './credit-shared';

/** Saisie d'une demande à partir de sa fiche, avec des changements (R-CRE-10 à R-CRE-16). */
export function applicationInput(
  view: ApplicationView,
  patch: Partial<ApplicationInput> = {},
): ApplicationInput {
  return {
    name: view.name,
    project_kind: view.project_kind,
    asset_id: view.asset_id ?? null,
    price_minor: view.price_minor,
    notary_fees_minor: view.notary_fees_minor,
    works_minor: view.works_minor,
    other_costs_minor: view.other_costs_minor,
    down_payment_minor: view.down_payment_minor,
    other_income_minor: view.other_income_minor,
    other_charges_minor: view.other_charges_minor,
    co_borrower_name: view.co_borrower_name ?? null,
    co_borrower_income_minor: view.co_borrower_income_minor,
    co_borrower_charges_minor: view.co_borrower_charges_minor,
    excluded_loan_ids: view.excluded_loan_ids,
    selected_simulation_id: view.selected_simulation_id ?? null,
    status: view.status,
    note: view.note ?? null,
    ...patch,
  };
}

/** E38 — Demandes de crédit (CRE-04 à CRE-06). */
@Component({
  selector: 'app-applications',
  imports: [RouterLink, LoansTabs, MatButtonModule, MatIconModule, MoneyPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Prêts</h1>
        <button mat-flat-button (click)="create()"><mat-icon>add</mat-icon>Nouvelle demande</button>
      </div>
      <app-loans-tabs />
      <section class="card">
        @for (a of list(); track a.id) {
          <a class="row" [routerLink]="['/prets/demandes', a.id]">
            <span class="name">
              <span>{{ a.name }}</span>
              <span class="muted small">
                {{ projectLabels[a.project_kind] }}
                @if (a.missing_pieces > 0) {
                  · {{ a.missing_pieces }} pièce{{ a.missing_pieces > 1 ? 's' : '' }} à fournir
                }
              </span>
            </span>
            <span class="status" [attr.data-status]="a.status">{{ statusLabels[a.status] }}</span>
            <span class="amount">{{ a.to_borrow_minor | money }}</span>
          </a>
        } @empty {
          <p class="muted">
            Aucune demande de crédit. Une demande réunit le plan de financement, votre capacité
            d'emprunt, les pièces à fournir et vos démarches auprès des banques.
          </p>
        }
      </section>
    </div>
  `,
  styles: `
    .row {
      display: grid;
      grid-template-columns: 1fr auto auto;
      align-items: center;
      gap: 1rem;
      padding: 0.5rem 0;
      color: inherit;
      text-decoration: none;
      & + .row {
        border-top: 1px solid var(--mat-sys-outline-variant);
      }
    }
    .name {
      display: flex;
      flex-direction: column;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .status {
      font: var(--mat-sys-label-medium);
      padding: 0.125rem 0.625rem;
      border-radius: 1rem;
      background: var(--mat-sys-surface-container-high);
      &[data-status='ACCEPTED'] {
        background: var(--mat-sys-primary-container);
        color: var(--mat-sys-on-primary-container);
      }
    }
  `,
})
export class ApplicationsPage {
  private readonly api = inject(CreditService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);

  protected readonly projectLabels = PROJECT_KIND_LABELS;
  protected readonly statusLabels = APPLICATION_STATUS_LABELS;

  private readonly resource = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listLoanApplications(),
  });
  protected readonly list = computed(() => this.resource.value() ?? []);

  protected async create(): Promise<void> {
    const ref = this.dialog.open<ApplicationDialog, ApplicationView | null, string>(
      ApplicationDialog,
      {
        data: null,
        maxWidth: '100vw',
      },
    );
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.version.bump();
      await this.router.navigate(['/prets/demandes', id]);
    }
  }
}

/** Création ou modification d'une demande : nom, projet, bien, prix, apport, note (E38). */
@Component({
  selector: 'app-application-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ view ? 'Modifier la demande' : 'Nouvelle demande de crédit' }}</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Nom</mat-label>
        <input matInput maxlength="80" [ngModel]="name()" (ngModelChange)="name.set($event)" />
        <mat-hint>« Achat appartement Grenoble »</mat-hint>
      </mat-form-field>
      <mat-form-field>
        <mat-label>Projet</mat-label>
        <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
          @for (k of kinds; track k) {
            <mat-option [value]="k">{{ labels[k] }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      @if (features.assets()) {
        <mat-form-field>
          <mat-label>Bien concerné</mat-label>
          <mat-select [ngModel]="asset()" (ngModelChange)="asset.set($event)">
            <mat-option [value]="null">Aucun</mat-option>
            @for (a of assets.value() ?? []; track a.id) {
              <mat-option [value]="a.id">{{ a.name }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
      }
      @if (!view) {
        <div class="pair">
          <mat-form-field>
            <mat-label>Prix</mat-label>
            <input
              matInput
              inputmode="decimal"
              [ngModel]="price()"
              (ngModelChange)="price.set($event)"
            />
            <span matTextSuffix>€</span>
          </mat-form-field>
          <mat-form-field>
            <mat-label>Apport</mat-label>
            <input
              matInput
              inputmode="decimal"
              [ngModel]="down()"
              (ngModelChange)="down.set($event)"
            />
            <span matTextSuffix>€</span>
          </mat-form-field>
        </div>
        <p class="muted small">
          Les frais de notaire sont estimés d'après le projet ; vous pourrez les modifier.
        </p>
      }
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
      <button mat-flat-button [disabled]="!valid()" (click)="save()">
        {{ view ? 'Enregistrer' : 'Créer' }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(28rem, 80vw);
    }
    .pair {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--field-gap);
    }
    .small {
      font: var(--mat-sys-body-small);
      margin: 0;
    }
    .error {
      color: var(--mat-sys-error);
      margin: 0;
    }
  `,
})
export class ApplicationDialog {
  protected readonly view = inject<ApplicationView | null>(MAT_DIALOG_DATA);
  private readonly api = inject(CreditService);
  private readonly ref = inject(MatDialogRef<ApplicationDialog, string>);
  protected readonly features = inject(FeaturesStore);
  private readonly assetsApi = inject(AssetsService);
  protected readonly assets = rxResource({
    params: () => (this.features.assets() ? true : undefined),
    stream: () => this.assetsApi.listAssets(),
  });

  protected readonly kinds = PROJECT_KINDS;
  protected readonly labels = PROJECT_KIND_LABELS;
  protected readonly name = signal(this.view?.name ?? '');
  protected readonly kind = signal<ProjectKind>(this.view?.project_kind ?? 'EXISTING_HOME');
  protected readonly asset = signal<string | null>(this.view?.asset_id ?? null);
  protected readonly price = signal('');
  protected readonly down = signal('');
  protected readonly note = signal(this.view?.note ?? '');
  protected readonly error = signal<string | null>(null);

  protected readonly valid = computed(
    () => !!this.name().trim() && (!!this.view || parseMoney(this.price() || '0') !== null),
  );

  protected async save(): Promise<void> {
    const common = {
      name: this.name().trim(),
      project_kind: this.kind(),
      asset_id: this.asset(),
      note: this.note().trim() || null,
    };
    try {
      if (this.view) {
        // Changer de projet réestime les frais de notaire (R-CRE-12).
        const body = applicationInput(this.view, {
          ...common,
          notary_fees_minor:
            common.project_kind === this.view.project_kind ? this.view.notary_fees_minor : null,
        });
        const saved = await firstValueFrom(
          this.api.updateLoanApplication({ id: this.view.id, body }),
        );
        this.ref.close(saved.id);
      } else {
        const saved = await firstValueFrom(
          this.api.createLoanApplication({
            body: {
              ...common,
              price_minor: parseMoney(this.price() || '0') ?? 0,
              down_payment_minor: parseMoney(this.down() || '0') ?? 0,
            },
          }),
        );
        this.ref.close(saved.id);
      }
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

export interface LenderRequestDialogData {
  applicationId: string;
  request: LenderRequestView | null;
}

/** Démarche auprès d'un établissement ou d'un courtier (R-CRE-21). */
@Component({
  selector: 'app-lender-request-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDatepickerModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ r ? 'Modifier la démarche' : 'Nouvelle démarche' }}</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Établissement</mat-label>
        <mat-select [ngModel]="institution()" (ngModelChange)="institution.set($event)">
          <mat-option [value]="null">Autre (courtier, banque non listée)</mat-option>
          @for (i of institutions.active(); track i.id) {
            <mat-option [value]="i.id">{{ i.name }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      @if (!institution()) {
        <mat-form-field>
          <mat-label>Nom</mat-label>
          <input
            matInput
            maxlength="80"
            [ngModel]="lender()"
            (ngModelChange)="lender.set($event)"
          />
        </mat-form-field>
      }
      <mat-form-field>
        <mat-label>Statut</mat-label>
        <mat-select [ngModel]="status()" (ngModelChange)="status.set($event)">
          @for (s of statuses; track s) {
            <mat-option [value]="s">{{ statusLabels[s] }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <div class="pair">
        <mat-form-field>
          <mat-label>Envoyée le</mat-label>
          <input
            matInput
            [matDatepicker]="sentPicker"
            [ngModel]="sent()"
            (ngModelChange)="sent.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="sentPicker" />
          <mat-datepicker #sentPicker />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Réponse le</mat-label>
          <input
            matInput
            [matDatepicker]="answerPicker"
            [ngModel]="answered()"
            (ngModelChange)="answered.set($event)"
          />
          <mat-datepicker-toggle matIconSuffix [for]="answerPicker" />
          <mat-datepicker #answerPicker />
        </mat-form-field>
      </div>
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
      <button mat-flat-button [disabled]="!body()" (click)="save()">Enregistrer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(28rem, 80vw);
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
  `,
})
export class LenderRequestDialog {
  private readonly data = inject<LenderRequestDialogData>(MAT_DIALOG_DATA);
  private readonly api = inject(CreditService);
  private readonly ref = inject(MatDialogRef<LenderRequestDialog, boolean>);
  protected readonly institutions = inject(InstitutionsStore);

  protected readonly r = this.data.request;
  protected readonly statuses = LENDER_STATUSES;
  protected readonly statusLabels = LENDER_STATUS_LABELS;
  protected readonly institution = signal<string | null>(this.r?.institution_id ?? null);
  protected readonly lender = signal(this.r?.lender_name ?? '');
  protected readonly status = signal(this.r?.status ?? 'SENT');
  protected readonly sent = signal<Date | null>(fromIsoDate(this.r?.sent_on ?? todayIso()));
  protected readonly answered = signal<Date | null>(
    this.r?.answered_on ? fromIsoDate(this.r.answered_on) : null,
  );
  protected readonly note = signal(this.r?.note ?? '');
  protected readonly error = signal<string | null>(null);

  protected readonly body = computed<LenderRequestInput | null>(() => {
    const sent = this.sent();
    const lender = this.lender().trim();
    if (!sent || (!this.institution() && !lender)) return null;
    const answered = this.answered();
    return {
      institution_id: this.institution(),
      lender_name: this.institution() ? null : lender,
      status: this.status(),
      sent_on: toIsoDate(sent),
      answered_on: answered ? toIsoDate(answered) : null,
      simulation_id: this.r?.simulation_id ?? null,
      note: this.note().trim() || null,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    try {
      const id = this.data.applicationId;
      if (this.r) {
        await firstValueFrom(this.api.updateLenderRequest({ id, request_id: this.r.id, body }));
      } else {
        await firstValueFrom(this.api.createLenderRequest({ id, body }));
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

/** Montant saisi, à défaut zéro. */
export function moneyOrZero(text: string): number | null {
  return text.trim() ? parseMoney(text) : 0;
}

export function editable(minor: number): string {
  return minor ? toEditable(minor) : '';
}
