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
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { Observable, firstValueFrom } from 'rxjs';

import { ApplicationInput } from '../../api/models/application-input';
import { ApplicationStatus } from '../../api/models/application-status';
import { ApplicationView } from '../../api/models/application-view';
import { DocumentKind } from '../../api/models/document-kind';
import { LenderEventView } from '../../api/models/lender-event-view';
import { LenderRequestView } from '../../api/models/lender-request-view';
import { Piece } from '../../api/models/piece';
import { PieceView } from '../../api/models/piece-view';
import { SimulationInput } from '../../api/models/simulation-input';
import { SimulationView } from '../../api/models/simulation-view';
import { CreditService } from '../../api/services/credit.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { formatRate } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { pickDocuments } from '../documents/link-dialogs';
import { acceptOffer } from './acceptance';
import { uploadDocument } from '../documents/upload-dialog';
import { LoanDialog } from '../loans/loans';
import {
  ApplicationDialog,
  EVENT_KIND_ICONS,
  EVENT_KIND_LABELS,
  LenderEventDialog,
  LenderEventDialogData,
  LenderRequestDialog,
  LenderRequestDialogData,
  applicationInput,
  editable,
  moneyOrZero,
} from './applications';
import {
  APPLICATION_STATUSES,
  APPLICATION_STATUS_LABELS,
  LENDER_STATUS_LABELS,
  PIECE_STATUS_LABELS,
  PROJECT_KIND_LABELS,
  compareSimulations,
  editSimulation,
  formatDuration,
  showSchedule,
} from './credit-shared';

/** Type proposé à l'envoi d'un document pour une pièce (R-CRE-17). */
const PIECE_KINDS: Partial<Record<Piece, DocumentKind>> = {
  IDENTITY: 'IDENTITY',
  CO_BORROWER_IDENTITY: 'IDENTITY',
  PROOF_OF_ADDRESS: 'PROOF_OF_ADDRESS',
  PAYSLIPS: 'PAYSLIP',
  CO_BORROWER_PAYSLIPS: 'PAYSLIP',
  EMPLOYMENT_CONTRACT: 'CONTRACT',
  CO_BORROWER_CONTRACT: 'CONTRACT',
  TAX_NOTICES: 'TAX',
  BANK_STATEMENTS: 'BANK_STATEMENT',
  SAVINGS_STATEMENT: 'BANK_STATEMENT',
  LOAN_DOCUMENTS: 'LOAN_OFFER',
  SALE_AGREEMENT: 'SALE_AGREEMENT',
  QUOTES: 'QUOTE',
};

const PIECE_ICONS: Record<PieceView['status'], string> = {
  PROVIDED: 'check_circle',
  INCOMPLETE: 'pending',
  MISSING: 'cancel',
  NOT_REQUIRED: 'remove_circle_outline',
};

/** E39 — Fiche d'une demande de crédit (CRE-04 à CRE-06). */
@Component({
  selector: 'app-application-page',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatProgressBarModule,
    MatSelectModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      @if (view(); as a) {
        <div class="page-header">
          <a mat-icon-button routerLink="/prets/demandes" aria-label="Retour aux demandes"
            ><mat-icon>arrow_back</mat-icon></a
          >
          <h1>{{ a.name }}</h1>
          <mat-form-field subscriptSizing="dynamic" class="status">
            <mat-label>Statut</mat-label>
            <mat-select [value]="a.status" (selectionChange)="setStatus(a, $event.value)">
              @for (s of statuses; track s) {
                <mat-option [value]="s">{{ statusLabels[s] }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          <button mat-icon-button [matMenuTriggerFor]="menu" aria-label="Actions sur la demande">
            <mat-icon>more_vert</mat-icon>
          </button>
          <mat-menu #menu>
            <button mat-menu-item (click)="edit(a)"><mat-icon>edit</mat-icon>Modifier</button>
            <a mat-menu-item [href]="summaryUrl(a)" target="_blank" rel="noopener">
              <mat-icon>print</mat-icon>Synthèse
            </a>
            <button mat-menu-item (click)="export(a)">
              <mat-icon>folder_zip</mat-icon>Exporter le dossier
            </button>
            <button mat-menu-item (click)="remove(a)"><mat-icon>delete</mat-icon>Supprimer</button>
          </mat-menu>
        </div>
        <p class="muted subtitle">
          {{ projectLabels[a.project_kind] }}
          @if (a.asset_name) {
            · <a [routerLink]="['/biens', a.asset_id]">{{ a.asset_name }}</a>
          }
          @if (a.note) {
            · {{ a.note }}
          }
        </p>

        <div class="columns">
          <section class="card form">
            <h2>Plan de financement</h2>
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
                <mat-label>Frais de notaire</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="notary()"
                  (ngModelChange)="notary.set($event)"
                />
                <span matTextSuffix>€</span>
                <mat-hint>Estimation : {{ a.estimated_notary_fees_minor | money }}</mat-hint>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Travaux</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="works()"
                  (ngModelChange)="works.set($event)"
                />
                <span matTextSuffix>€</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Autres frais</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="otherCosts()"
                  (ngModelChange)="otherCosts.set($event)"
                />
                <span matTextSuffix>€</span>
                <mat-hint>Agence, dossier, garantie…</mat-hint>
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
            <div class="totals">
              <span>Coût du projet</span><strong>{{ a.project_cost_minor | money }}</strong>
              <span>Montant à emprunter</span
              ><strong class="primary">{{ a.to_borrow_minor | money }}</strong>
            </div>

            <h3>Autres revenus et charges</h3>
            <div class="pair">
              <mat-form-field>
                <mat-label>Autres revenus</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="otherIncome()"
                  (ngModelChange)="otherIncome.set($event)"
                />
                <span matTextSuffix>€</span>
                <mat-hint>Par mois : loyers perçus, pensions…</mat-hint>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Autres charges</mat-label>
                <input
                  matInput
                  inputmode="decimal"
                  [ngModel]="otherCharges()"
                  (ngModelChange)="otherCharges.set($event)"
                />
                <span matTextSuffix>€</span>
                <mat-hint>Par mois : loyer conservé, pension…</mat-hint>
              </mat-form-field>
            </div>
            <h3>Co-emprunteur</h3>
            <mat-form-field>
              <mat-label>Nom</mat-label>
              <input
                matInput
                maxlength="80"
                [ngModel]="coName()"
                (ngModelChange)="coName.set($event)"
              />
              <mat-hint>Laisser vide sans co-emprunteur</mat-hint>
            </mat-form-field>
            @if (coName().trim()) {
              <div class="pair">
                <mat-form-field>
                  <mat-label>Revenus mensuels</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="coIncome()"
                    (ngModelChange)="coIncome.set($event)"
                  />
                  <span matTextSuffix>€</span>
                </mat-form-field>
                <mat-form-field>
                  <mat-label>Charges mensuelles</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="coCharges()"
                    (ngModelChange)="coCharges.set($event)"
                  />
                  <span matTextSuffix>€</span>
                </mat-form-field>
              </div>
            }
            @if (planError()) {
              <p class="error" role="alert">{{ planError() }}</p>
            }
            <div class="save">
              <button
                mat-flat-button
                [disabled]="!planDirty() || !planInput()"
                (click)="savePlan(a)"
              >
                Enregistrer
              </button>
            </div>
          </section>

          <section class="card capacity">
            <h2>Capacité d'emprunt</h2>
            @let c = a.capacity;
            <div class="lines">
              @for (e of c.employments; track e.employment_id) {
                <span>Salaire — {{ e.employer }}</span
                ><span class="amount">{{ e.monthly_minor | money }}</span>
              }
              @if (a.other_income_minor) {
                <span>Autres revenus</span
                ><span class="amount">{{ a.other_income_minor | money }}</span>
              }
              @if (a.co_borrower_name) {
                <span>{{ a.co_borrower_name }}</span
                ><span class="amount">{{ a.co_borrower_income_minor | money }}</span>
              }
              <strong>Revenus mensuels</strong
              ><strong class="amount">{{ c.income_minor | money }}</strong>
            </div>
            <div class="lines">
              @for (l of c.loans; track l.loan_id) {
                <mat-checkbox
                  [checked]="!l.excluded"
                  (change)="toggleLoan(a, l.loan_id)"
                  [attr.aria-label]="'Compter ' + l.name"
                  >{{ l.name
                  }}{{ (l.monthly_minor ?? null) === null ? ' (suivi simple)' : '' }}</mat-checkbox
                >
                <span class="amount" [class.muted]="l.excluded">{{ l.monthly_minor | money }}</span>
              }
              @if (a.other_charges_minor) {
                <span>Autres charges</span
                ><span class="amount">{{ a.other_charges_minor | money }}</span>
              }
              @if (a.co_borrower_name && a.co_borrower_charges_minor) {
                <span>Charges de {{ a.co_borrower_name }}</span
                ><span class="amount">{{ a.co_borrower_charges_minor | money }}</span>
              }
              <strong>Charges mensuelles</strong
              ><strong class="amount">{{ c.charges_minor | money }}</strong>
            </div>
            @if (c.loans.length > 0) {
              <p class="muted small">Décochez un prêt que ce crédit remboursera.</p>
            }

            <mat-form-field>
              <mat-label>Simulation retenue</mat-label>
              <mat-select
                [value]="a.selected_simulation_id ?? null"
                (selectionChange)="select(a, $event.value)"
              >
                <mat-option [value]="null">Aucune</mat-option>
                @for (s of a.simulations; track s.id) {
                  <mat-option [value]="s.id">{{ s.name }}</mat-option>
                }
              </mat-select>
            </mat-form-field>

            <div class="figures">
              @if ((c.payment_minor ?? null) !== null) {
                <div>
                  <span class="muted">Mensualité envisagée</span>
                  <strong>{{ c.payment_minor | money }}</strong>
                </div>
              }
              @if (c.debt_ratio_ppm !== null && c.debt_ratio_ppm !== undefined) {
                <div>
                  <span class="muted">Taux d'endettement</span>
                  <strong [class.warn]="c.debt_ratio_exceeded">{{ rate(c.debt_ratio_ppm) }}</strong>
                  <mat-progress-bar
                    mode="determinate"
                    [class.warn]="c.debt_ratio_exceeded"
                    [value]="gauge(c.debt_ratio_ppm)"
                  />
                  <span class="small muted">seuil indicatif {{ rate(c.max_debt_ratio_ppm) }}</span>
                </div>
              }
              <div>
                <span class="muted">Reste à vivre</span>
                <strong>{{ c.remaining_income_minor | money }}</strong>
              </div>
              <div>
                <span class="muted">Mensualité maximale</span>
                <strong>{{ c.max_payment_minor | money }}</strong>
                @if ((c.borrowing_capacity_minor ?? null) !== null) {
                  <span class="small muted"
                    >capital empruntable {{ c.borrowing_capacity_minor | money }}</span
                  >
                }
              </div>
              <div>
                <span class="muted">Épargne restante</span>
                <strong [class.warn]="c.savings_after_minor < 0">{{
                  c.savings_after_minor | money
                }}</strong>
                <span class="small muted"
                  >épargne de précaution {{ c.savings_minor | money }} − apport</span
                >
              </div>
            </div>
            @if (c.debt_ratio_exceeded) {
              <p class="warning">
                <mat-icon>warning</mat-icon>Endettement au-delà de
                {{ rate(c.max_debt_ratio_ppm) }} (norme du HCSF, indicative).
              </p>
            }
            @if (c.duration_exceeded) {
              <p class="warning">
                <mat-icon>warning</mat-icon>Durée au-delà de
                {{ duration(c.max_duration_months) }} (norme du HCSF, indicative).
              </p>
            }
          </section>
        </div>

        <section class="card">
          <header>
            <h2>Simulations</h2>
            <span class="actions">
              @if (a.simulations.length > 1) {
                <button mat-stroked-button (click)="compare(a.simulations)">
                  <mat-icon>compare_arrows</mat-icon>Comparer
                </button>
              }
              <button mat-stroked-button (click)="newSimulation(a)">
                <mat-icon>add</mat-icon>Nouvelle simulation
              </button>
            </span>
          </header>
          @for (s of a.simulations; track s.id) {
            <div class="row">
              <span class="name">
                <span>
                  {{ s.name }}
                  @if (s.id === a.selected_simulation_id) {
                    <span class="tag">retenue</span>
                  }
                </span>
                <span class="muted small">
                  {{ s.principal_minor | money }} · {{ duration(s.duration_months) }} ·
                  {{ rate(s.annual_rate_ppm) }} · TAEG {{ rate(s.result.apr_ppm) }} · coût
                  {{ s.result.total_cost_minor | money }}
                  @if (s.institution_name) {
                    · {{ s.institution_name }}
                  }
                </span>
              </span>
              <span class="amount">{{ s.result.total_payment_minor | money }}</span>
              <button
                mat-icon-button
                [matMenuTriggerFor]="simMenu"
                [attr.aria-label]="'Actions sur ' + s.name"
              >
                <mat-icon>more_vert</mat-icon>
              </button>
              <mat-menu #simMenu>
                <button mat-menu-item (click)="editSim(s)">
                  <mat-icon>edit</mat-icon>Modifier
                </button>
                <button mat-menu-item (click)="schedule(s)">
                  <mat-icon>table_chart</mat-icon>Tableau d'amortissement
                </button>
                @if (s.loan_id) {
                  <a mat-menu-item [routerLink]="['/prets', s.loan_id]">
                    <mat-icon>real_estate_agent</mat-icon>Voir le prêt
                  </a>
                } @else {
                  <button mat-menu-item (click)="convert(s, a)">
                    <mat-icon>real_estate_agent</mat-icon>Transformer en prêt
                  </button>
                }
              </mat-menu>
            </div>
          } @empty {
            <p class="muted">
              Aucune simulation : « Nouvelle simulation » part du montant à emprunter.
            </p>
          }
        </section>

        <section class="card">
          <header>
            <h2>Pièces à fournir</h2>
            <span class="muted">{{ missing(a) }}</span>
          </header>
          @for (p of a.pieces; track p.piece) {
            <div class="piece" [attr.data-status]="p.status">
              <mat-icon class="state" [attr.aria-label]="pieceLabels[p.status]">{{
                pieceIcons[p.status]
              }}</mat-icon>
              <span class="name">
                <span>
                  {{ p.label }}
                  @if (p.status === 'INCOMPLETE') {
                    <span class="muted small">({{ p.documents.length }}/{{ p.expected }})</span>
                  }
                </span>
                <span class="docs">
                  @for (d of p.documents; track d.id) {
                    <span class="doc" [class.found]="!d.assigned">
                      <a [routerLink]="['/documents', d.id]">{{ d.title }}</a>
                      @if (d.assigned) {
                        <button
                          mat-icon-button
                          class="small-button"
                          [attr.aria-label]="'Retirer ' + d.title"
                          (click)="unassign(a, p, d.id)"
                        >
                          <mat-icon>close</mat-icon>
                        </button>
                      }
                    </span>
                  }
                </span>
              </span>
              <button
                mat-icon-button
                [matMenuTriggerFor]="pieceMenu"
                [attr.aria-label]="'Actions sur ' + p.label"
              >
                <mat-icon>more_vert</mat-icon>
              </button>
              <mat-menu #pieceMenu>
                <button mat-menu-item (click)="assign(a, p)">
                  <mat-icon>link</mat-icon>Attribuer un document
                </button>
                <button mat-menu-item (click)="upload(a, p)">
                  <mat-icon>upload_file</mat-icon>Envoyer un document
                </button>
                <button mat-menu-item (click)="toggleRequired(a, p)">
                  <mat-icon>{{
                    p.status === 'NOT_REQUIRED' ? 'add_task' : 'remove_circle_outline'
                  }}</mat-icon
                  >{{ p.status === 'NOT_REQUIRED' ? 'Requise' : 'Non requise' }}
                </button>
              </mat-menu>
            </div>
          }
          <p class="muted small">
            Les documents en italique sont trouvés dans la bibliothèque ; ceux attribués à la main
            les remplacent.
          </p>
        </section>

        <section class="card">
          <header>
            <h2>Démarches</h2>
            <span class="actions">
              @if (offers(a).length > 1) {
                <button mat-stroked-button (click)="compare(offers(a))">
                  <mat-icon>compare_arrows</mat-icon>Comparer les offres
                </button>
              }
              <button mat-stroked-button (click)="request(a, null)">
                <mat-icon>add</mat-icon>Nouvelle démarche
              </button>
            </span>
          </header>
          @for (r of a.lender_requests; track r.id) {
            <div class="request">
              <div class="row">
                <button
                  class="name open"
                  (click)="toggleJournal(r.id)"
                  [attr.aria-expanded]="opened().has(r.id)"
                >
                  <span>
                    {{ r.name }} <span class="tag">{{ lenderLabels[r.status] }}</span>
                    @if (r.follow_up_due) {
                      <span class="tag due">à relancer</span>
                    }
                  </span>
                  <span class="muted small">
                    envoyée le {{ r.sent_on | frDate }}
                    @if (r.answered_on) {
                      · réponse le {{ r.answered_on | frDate }}
                    }
                    @if (r.follow_up_on) {
                      · relance le {{ r.follow_up_on | frDate }}
                    }
                    @if (offerOf(a, r); as o) {
                      · offre : {{ o.name }}, {{ o.result.total_payment_minor | money }}/mois, TAEG
                      {{ rate(o.result.apr_ppm) }}
                    }
                  </span>
                </button>
                @if (r.status === 'SENT' || r.status === 'OFFER') {
                  <button mat-stroked-button class="follow" (click)="followUp(a, r)">
                    <mat-icon>notifications_active</mat-icon>Relancer
                  </button>
                } @else {
                  <span></span>
                }
                <button
                  mat-icon-button
                  [matMenuTriggerFor]="reqMenu"
                  [attr.aria-label]="'Actions sur ' + r.name"
                >
                  <mat-icon>more_vert</mat-icon>
                </button>
                <mat-menu #reqMenu>
                  <button mat-menu-item (click)="request(a, r)">
                    <mat-icon>edit</mat-icon>Modifier
                  </button>
                  <button mat-menu-item (click)="event(a, r, null)">
                    <mat-icon>add_comment</mat-icon>Ajouter un échange
                  </button>
                  <button mat-menu-item (click)="recordOffer(a, r)">
                    <mat-icon>request_quote</mat-icon
                    >{{ r.simulation_id ? "Remplacer l'offre" : "Enregistrer l'offre" }}
                  </button>
                  @if (r.status === 'OFFER' && r.simulation_id) {
                    <button mat-menu-item (click)="accept(a, r)">
                      <mat-icon>task_alt</mat-icon>Accepter l'offre
                    </button>
                  }
                  <button mat-menu-item (click)="removeRequest(a, r)">
                    <mat-icon>delete</mat-icon>Supprimer
                  </button>
                </mat-menu>
              </div>
              @if (opened().has(r.id)) {
                <ol class="journal" aria-label="Journal">
                  @for (e of r.events; track e.id) {
                    <li>
                      <mat-icon>{{ eventIcons[e.kind] }}</mat-icon>
                      <span class="when">{{ e.date | frDate }}</span>
                      <span>
                        <strong>{{ eventLabels[e.kind] }}</strong>
                        @if (e.note) {
                          — {{ e.note }}
                        }
                      </span>
                      <button
                        mat-icon-button
                        class="small-button"
                        [matMenuTriggerFor]="eventMenu"
                        [attr.aria-label]="'Actions sur l’échange du ' + e.date"
                      >
                        <mat-icon>more_vert</mat-icon>
                      </button>
                      <mat-menu #eventMenu>
                        <button mat-menu-item (click)="event(a, r, e)">
                          <mat-icon>edit</mat-icon>Modifier
                        </button>
                        <button mat-menu-item (click)="removeEvent(a, r, e)">
                          <mat-icon>delete</mat-icon>Supprimer
                        </button>
                      </mat-menu>
                    </li>
                  }
                  <li>
                    <button mat-button (click)="event(a, r, null)">
                      <mat-icon>add</mat-icon>Ajouter un échange
                    </button>
                  </li>
                </ol>
              }
            </div>
          } @empty {
            <p class="muted">Aucune démarche : ajoutez chaque banque ou courtier sollicité.</p>
          }
        </section>
      } @else if (resource.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      } @else {
        <p class="empty">
          Demande introuvable. <a routerLink="/prets/demandes">Revenir à la liste</a>
        </p>
      }
    </div>
  `,
  styles: `
    .subtitle {
      margin: -0.5rem 0 1rem;
    }
    .status {
      width: 11rem;
    }
    .columns {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(20rem, 1fr));
      gap: 1rem;
      align-items: start;
    }
    .card {
      margin-bottom: 1rem;
    }
    h2 {
      margin: 0 0 0.75rem;
      font: var(--mat-sys-title-medium);
    }
    h3 {
      margin: 1rem 0 0.5rem;
      font: var(--mat-sys-title-small);
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 0.5rem;
      flex-wrap: wrap;
      h2 {
        margin: 0;
      }
      .actions {
        display: flex;
        gap: 0.5rem;
        flex-wrap: wrap;
      }
    }
    .form {
      display: flex;
      flex-direction: column;
    }
    .pair {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: calc(var(--field-gap) * 1.5) var(--field-gap);
      align-items: start;
      @media (max-width: 599.98px) {
        grid-template-columns: 1fr;
      }
    }
    .form > mat-form-field + .pair,
    .form > mat-form-field + mat-form-field {
      margin-top: var(--field-gap);
    }
    .totals,
    .lines {
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 0.25rem 1rem;
      margin: 0.5rem 0 1rem;
      align-items: center;
    }
    .primary {
      color: var(--mat-sys-primary);
    }
    .save {
      display: flex;
      justify-content: flex-end;
    }
    .figures {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
      gap: 1rem;
      div {
        display: flex;
        flex-direction: column;
        gap: 0.125rem;
      }
      strong {
        font: var(--mat-sys-title-large);
      }
    }
    .warn {
      color: var(--cf-warning, #b26a00);
      --mat-progress-bar-active-indicator-color: var(--cf-warning, #b26a00);
    }
    .warning {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      color: var(--cf-warning, #b26a00);
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
    .row,
    .piece {
      display: grid;
      grid-template-columns: 1fr auto auto;
      align-items: center;
      gap: 0.5rem;
      padding: 0.25rem 0;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .piece {
      grid-template-columns: auto 1fr auto;
      &[data-status='PROVIDED'] .state {
        color: var(--mat-sys-primary);
      }
      &[data-status='MISSING'] .state,
      &[data-status='INCOMPLETE'] .state {
        color: var(--cf-warning, #b26a00);
      }
      &[data-status='NOT_REQUIRED'] {
        opacity: 0.6;
      }
    }
    .name {
      display: flex;
      flex-direction: column;
      min-width: 0;
    }
    .docs {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem 0.75rem;
    }
    .doc {
      display: inline-flex;
      align-items: center;
      font: var(--mat-sys-body-small);
      &.found {
        font-style: italic;
      }
    }
    .small-button {
      --mat-icon-button-state-layer-size: 1.75rem;
      padding: 0.125rem;
      mat-icon {
        font-size: 1rem;
        width: 1rem;
        height: 1rem;
      }
    }
    .request + .request {
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .request .row {
      border-top: 0;
    }
    .open {
      background: none;
      border: 0;
      padding: 0.25rem 0;
      color: inherit;
      font: inherit;
      text-align: left;
      cursor: pointer;
    }
    .journal {
      list-style: none;
      margin: 0 0 0.75rem;
      padding: 0 0 0 1rem;
      border-left: 2px solid var(--mat-sys-outline-variant);
      li {
        display: grid;
        grid-template-columns: auto auto 1fr auto;
        align-items: center;
        gap: 0.5rem;
        font: var(--mat-sys-body-small);
        mat-icon {
          font-size: 1.125rem;
          width: 1.125rem;
          height: 1.125rem;
          color: var(--mat-sys-on-surface-variant);
        }
      }
      .when {
        color: var(--mat-sys-on-surface-variant);
      }
    }
    .tag.due {
      background: var(--cf-warning, #b26a00);
      color: white;
    }
    @media (max-width: 599.98px) {
      .follow {
        display: none;
      }
    }
    .tag {
      font: var(--mat-sys-label-small);
      padding: 0.0625rem 0.5rem;
      border-radius: 1rem;
      background: var(--mat-sys-surface-container-high);
      margin-left: 0.25rem;
    }
  `,
})
export class ApplicationPage {
  private readonly api = inject(CreditService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);

  readonly id = input.required<string>();

  protected readonly resource = rxResource({
    params: () => ({ id: this.id(), version: this.version.value() }),
    stream: ({ params }) => this.api.getLoanApplication({ id: params.id }),
  });
  protected readonly view = computed(() =>
    this.resource.hasValue() ? this.resource.value() : undefined,
  );

  protected readonly statuses = APPLICATION_STATUSES;
  protected readonly statusLabels = APPLICATION_STATUS_LABELS;
  protected readonly projectLabels = PROJECT_KIND_LABELS;
  protected readonly lenderLabels = LENDER_STATUS_LABELS;
  protected readonly pieceLabels = PIECE_STATUS_LABELS;
  protected readonly pieceIcons = PIECE_ICONS;
  protected readonly rate = formatRate;
  protected readonly duration = formatDuration;

  // Plan de financement et revenus saisis, repris à chaque chargement.
  protected readonly price = signal('');
  protected readonly notary = signal('');
  protected readonly works = signal('');
  protected readonly otherCosts = signal('');
  protected readonly down = signal('');
  protected readonly otherIncome = signal('');
  protected readonly otherCharges = signal('');
  protected readonly coName = signal('');
  protected readonly coIncome = signal('');
  protected readonly coCharges = signal('');
  protected readonly planError = signal<string | null>(null);

  constructor() {
    effect(() => {
      const a = this.view();
      if (!a) return;
      this.price.set(editable(a.price_minor));
      this.notary.set(editable(a.notary_fees_minor));
      this.works.set(editable(a.works_minor));
      this.otherCosts.set(editable(a.other_costs_minor));
      this.down.set(editable(a.down_payment_minor));
      this.otherIncome.set(editable(a.other_income_minor));
      this.otherCharges.set(editable(a.other_charges_minor));
      this.coName.set(a.co_borrower_name ?? '');
      this.coIncome.set(editable(a.co_borrower_income_minor));
      this.coCharges.set(editable(a.co_borrower_charges_minor));
      this.planError.set(null);
    });
  }

  protected readonly planInput = computed(() => {
    const values = {
      price_minor: moneyOrZero(this.price()),
      notary_fees_minor: moneyOrZero(this.notary()),
      works_minor: moneyOrZero(this.works()),
      other_costs_minor: moneyOrZero(this.otherCosts()),
      down_payment_minor: moneyOrZero(this.down()),
      other_income_minor: moneyOrZero(this.otherIncome()),
      other_charges_minor: moneyOrZero(this.otherCharges()),
      co_borrower_income_minor: moneyOrZero(this.coIncome()),
      co_borrower_charges_minor: moneyOrZero(this.coCharges()),
    };
    if (Object.values(values).some((v) => v === null || v < 0)) return null;
    return {
      ...(values as Record<keyof typeof values, number>),
      co_borrower_name: this.coName().trim() || null,
    };
  });

  protected readonly planDirty = computed(() => {
    const a = this.view();
    const p = this.planInput();
    if (!a || !p) return false;
    return (
      p.price_minor !== a.price_minor ||
      p.notary_fees_minor !== a.notary_fees_minor ||
      p.works_minor !== a.works_minor ||
      p.other_costs_minor !== a.other_costs_minor ||
      p.down_payment_minor !== a.down_payment_minor ||
      p.other_income_minor !== a.other_income_minor ||
      p.other_charges_minor !== a.other_charges_minor ||
      p.co_borrower_name !== (a.co_borrower_name ?? null) ||
      p.co_borrower_income_minor !== a.co_borrower_income_minor ||
      p.co_borrower_charges_minor !== a.co_borrower_charges_minor
    );
  });

  private async update(a: ApplicationView, patch: Partial<ApplicationInput>): Promise<boolean> {
    try {
      await firstValueFrom(
        this.api.updateLoanApplication({ id: a.id, body: applicationInput(a, patch) }),
      );
      this.version.bump();
      return true;
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
      return false;
    }
  }

  private async run(action: Observable<unknown>): Promise<void> {
    try {
      await firstValueFrom(action);
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  protected async savePlan(a: ApplicationView): Promise<void> {
    const plan = this.planInput();
    if (!plan) return;
    try {
      await firstValueFrom(
        this.api.updateLoanApplication({ id: a.id, body: applicationInput(a, plan) }),
      );
      this.version.bump();
    } catch (e) {
      this.planError.set(errorMessage(e));
    }
  }

  protected setStatus(a: ApplicationView, status: ApplicationStatus): void {
    void this.update(a, { status });
  }

  protected select(a: ApplicationView, simulation: string | null): void {
    void this.update(a, { selected_simulation_id: simulation });
  }

  protected toggleLoan(a: ApplicationView, loan: string): void {
    const excluded = a.excluded_loan_ids.includes(loan)
      ? a.excluded_loan_ids.filter((id) => id !== loan)
      : [...a.excluded_loan_ids, loan];
    void this.update(a, { excluded_loan_ids: excluded });
  }

  protected gauge(ppm: number): number {
    return Math.min(100, ppm / 5_000);
  }

  protected missing(a: ApplicationView): string {
    const count = a.pieces.filter(
      (p) => p.status === 'MISSING' || p.status === 'INCOMPLETE',
    ).length;
    return count === 0
      ? 'Dossier complet'
      : `${count} pièce${count > 1 ? 's' : ''} manquante${count > 1 ? 's' : ''}`;
  }

  protected summaryUrl(a: ApplicationView): string {
    return `/api/v1/loan-applications/${a.id}/summary`;
  }

  protected async edit(a: ApplicationView): Promise<void> {
    const ref = this.dialog.open(ApplicationDialog, { data: a, maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  /** Export du dossier, après avoir signalé les pièces manquantes (R-CRE-19). */
  protected async export(a: ApplicationView): Promise<void> {
    const missing = a.pieces.filter((p) => p.status === 'MISSING' || p.status === 'INCOMPLETE');
    if (missing.length > 0) {
      const ok = await confirm(this.dialog, {
        title: 'Exporter un dossier incomplet ?',
        message: `Pièces manquantes ou incomplètes : ${missing.map((p) => p.label).join(', ')}.`,
        action: 'Exporter',
      });
      if (!ok) return;
    }
    const link = document.createElement('a');
    link.href = `/api/v1/loan-applications/${a.id}/export`;
    link.download = '';
    link.click();
  }

  protected async remove(a: ApplicationView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${a.name} » ?`,
      message: 'Ses démarches et ses pièces attribuées sont supprimées ; ses simulations restent.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    await firstValueFrom(this.api.deleteLoanApplication({ id: a.id }));
    this.version.bump();
    await this.router.navigate(['/prets/demandes']);
  }

  protected async newSimulation(a: ApplicationView): Promise<void> {
    const initial: Partial<SimulationInput> = {
      name: 'Simulation',
      principal_minor: a.to_borrow_minor > 0 ? a.to_borrow_minor : undefined,
      duration_months: 240,
      annual_rate_ppm: 32_000,
      insurance: { mode: 'RATE_ON_PRINCIPAL', rate_ppm: 3_000 },
      application_id: a.id,
    };
    if (await editSimulation(this.dialog, { simulation: null, initial })) this.version.bump();
  }

  protected async editSim(s: SimulationView): Promise<void> {
    if (await editSimulation(this.dialog, { simulation: s })) this.version.bump();
  }

  protected schedule(s: SimulationView): void {
    showSchedule(this.dialog, s);
  }

  protected compare(simulations: SimulationView[]): void {
    compareSimulations(this.dialog, simulations.slice(0, 4));
  }

  /** Transforme une simulation en prêt, rattaché au bien de la demande (R-CRE-7, R-CRE-22). */
  protected async convert(s: SimulationView, a: ApplicationView): Promise<void> {
    const ref = this.dialog.open(LoanDialog, {
      data: { simulation: s, assetId: a.asset_id },
      maxWidth: '100vw',
    });
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.version.bump();
      await this.router.navigate(['/prets', id]);
    }
  }

  protected async assign(a: ApplicationView, p: PieceView): Promise<void> {
    const ids = await pickDocuments(this.dialog);
    for (const id of ids) {
      await this.run(this.api.assignPiece({ id: a.id, piece: p.piece, body: { document_id: id } }));
    }
  }

  protected async upload(a: ApplicationView, p: PieceView): Promise<void> {
    const created = await uploadDocument(this.dialog, { kind: PIECE_KINDS[p.piece] ?? null });
    if (created) {
      await this.run(
        this.api.assignPiece({ id: a.id, piece: p.piece, body: { document_id: created.id } }),
      );
    }
  }

  protected unassign(a: ApplicationView, p: PieceView, document: string): void {
    void this.run(this.api.unassignPiece({ id: a.id, piece: p.piece, document_id: document }));
  }

  protected toggleRequired(a: ApplicationView, p: PieceView): void {
    void this.run(
      this.api.setPieceRequired({
        id: a.id,
        piece: p.piece,
        body: { not_required: p.status !== 'NOT_REQUIRED' },
      }),
    );
  }

  protected readonly eventLabels = EVENT_KIND_LABELS;
  protected readonly eventIcons = EVENT_KIND_ICONS;
  /** Démarches dont le journal est déplié. */
  protected readonly opened = signal<ReadonlySet<string>>(new Set());

  protected toggleJournal(id: string): void {
    this.opened.update((set) => {
      const next = new Set(set);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  private async eventDialog(data: LenderEventDialogData): Promise<void> {
    const ref = this.dialog.open<LenderEventDialog, LenderEventDialogData, boolean>(
      LenderEventDialog,
      {
        data,
        maxWidth: '100vw',
      },
    );
    if (await firstValueFrom(ref.afterClosed())) {
      this.opened.update((set) => new Set(set).add(data.request.id));
      this.version.bump();
    }
  }

  /** Relance une démarche (R-CRE-25). */
  protected followUp(a: ApplicationView, r: LenderRequestView): Promise<void> {
    return this.eventDialog({ applicationId: a.id, request: r, event: null, followUp: true });
  }

  protected event(
    a: ApplicationView,
    r: LenderRequestView,
    e: LenderEventView | null,
  ): Promise<void> {
    return this.eventDialog({ applicationId: a.id, request: r, event: e });
  }

  protected removeEvent(a: ApplicationView, r: LenderRequestView, e: LenderEventView): void {
    void this.run(this.api.deleteLenderEvent({ id: a.id, request_id: r.id, event_id: e.id }));
  }

  protected offers(a: ApplicationView): SimulationView[] {
    const ids = new Set(a.lender_requests.map((r) => r.simulation_id).filter(Boolean));
    return a.simulations.filter((s) => ids.has(s.id));
  }

  protected offerOf(a: ApplicationView, r: LenderRequestView): SimulationView | undefined {
    return a.simulations.find((s) => s.id === r.simulation_id);
  }

  protected async request(a: ApplicationView, r: LenderRequestView | null): Promise<void> {
    const ref = this.dialog.open<LenderRequestDialog, LenderRequestDialogData, boolean>(
      LenderRequestDialog,
      {
        data: { applicationId: a.id, request: r },
        maxWidth: '100vw',
      },
    );
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  /** Enregistre l'offre reçue d'une démarche, comme une simulation de la demande (R-CRE-21). */
  protected async recordOffer(a: ApplicationView, r: LenderRequestView): Promise<void> {
    const current = this.offerOf(a, r);
    const initial: Partial<SimulationInput> = {
      name: `Offre ${r.name}`,
      principal_minor:
        current?.principal_minor ?? (a.to_borrow_minor > 0 ? a.to_borrow_minor : undefined),
      duration_months: current?.duration_months ?? 300,
      annual_rate_ppm: current?.annual_rate_ppm ?? 0,
      insurance: current?.insurance ?? { mode: 'NONE' },
      fees_minor: current?.fees_minor ?? 0,
      guarantee_fees_minor: current?.guarantee_fees_minor ?? 0,
      institution_id: r.institution_id ?? null,
    };
    const saved = await editSimulation(this.dialog, {
      simulation: null,
      initial,
      title: `Offre de ${r.name}`,
      submit: (body) => firstValueFrom(this.api.recordOffer({ id: a.id, request_id: r.id, body })),
    });
    if (saved) this.version.bump();
  }

  /** Accepte une offre, puis propose de la transformer en prêt (R-CRE-22). */
  protected async accept(a: ApplicationView, r: LenderRequestView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Accepter l'offre de ${r.name} ?`,
      message: 'Les autres démarches en cours passent à « retirée », et la demande à « acceptée ».',
      action: 'Accepter',
    });
    if (!ok) return;
    try {
      const view = await firstValueFrom(this.api.acceptOffer({ id: a.id, request_id: r.id }));
      this.version.bump();
      // Assistant d'acceptation : le bien, le prêt, puis les documents (R-CRE-27).
      const offer = view.simulations.find((s) => s.id === r.simulation_id);
      if (offer) {
        const loan = await acceptOffer(this.dialog, { application: view, offer });
        this.version.bump();
        if (loan) await this.router.navigate(['/prets', loan]);
      }
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  protected async removeRequest(a: ApplicationView, r: LenderRequestView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer la démarche auprès de ${r.name} ?`,
      message: 'Son offre éventuelle reste parmi les simulations.',
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) await this.run(this.api.deleteLenderRequest({ id: a.id, request_id: r.id }));
  }
}
