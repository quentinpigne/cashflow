import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  input,
  output,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ProjectKind } from '../../api/models/project-kind';
import { SimulationView } from '../../api/models/simulation-view';
import { CreditService } from '../../api/services/credit.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { formatRate } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { MoneyPipe } from '../../shared/money.pipe';
import { LoanDialog } from '../loans/loans';
import { compareSimulations, editSimulation, formatDuration, showSchedule } from './credit-shared';

/** E36 — Simulations enregistrées : comparer, ouvrir, préparer une demande, transformer en prêt (CRE-01 à CRE-03). */
@Component({
  selector: 'app-simulations',
  imports: [
    RouterLink,
    MatButtonModule,
    MatCheckboxModule,
    MatIconModule,
    MatMenuModule,
    MoneyPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="part">
      <section class="card">
        <header>
          <h2>Simulations enregistrées</h2>
          <button mat-stroked-button [disabled]="checked().size < 2" (click)="compare()">
            <mat-icon>compare_arrows</mat-icon>Comparer
          </button>
        </header>
        @for (s of simulations(); track s.id) {
          <div class="row">
            <mat-checkbox
              [checked]="checked().has(s.id)"
              (change)="toggle(s.id)"
              [attr.aria-label]="'Comparer ' + s.name"
            />
            <button class="open" (click)="load(s)">
              <span>{{ s.name }}</span>
              <span class="muted small">
                {{ s.principal_minor | money }} · {{ duration_(s.duration_months) }} ·
                {{ rateText(s.annual_rate_ppm) }}
                @if (s.application_name) {
                  · {{ s.application_name }}
                }
                @if (s.institution_name) {
                  · {{ s.institution_name }}
                }
                @if (s.loan_id) {
                  · prêt créé
                }
              </span>
            </button>
            <span class="numbers">
              <span class="amount">{{ s.result.total_payment_minor | money }}</span>
              <span class="muted small"
                >TAEG {{ rateText(s.result.apr_ppm) }} · coût
                {{ s.result.total_cost_minor | money }}</span
              >
            </span>
            @if (s.application_id) {
              <a mat-button class="prepare" [routerLink]="['/prets/demandes', s.application_id]">
                Voir la demande
              </a>
            } @else {
              <button mat-stroked-button class="prepare" (click)="prepare(s)">
                Préparer une demande
              </button>
            }
            <button
              mat-icon-button
              [matMenuTriggerFor]="menu"
              [attr.aria-label]="'Actions sur ' + s.name"
            >
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              @if (!s.application_id) {
                <button mat-menu-item (click)="prepare(s)">
                  <mat-icon>assignment</mat-icon>Préparer une demande
                </button>
              }
              <button mat-menu-item (click)="edit(s)"><mat-icon>edit</mat-icon>Modifier</button>
              <button mat-menu-item (click)="duplicate(s)">
                <mat-icon>content_copy</mat-icon>Dupliquer
              </button>
              <button mat-menu-item (click)="schedule(s)">
                <mat-icon>table_chart</mat-icon>Tableau d'amortissement
              </button>
              @if (s.loan_id) {
                <a mat-menu-item [routerLink]="['/prets', s.loan_id]">
                  <mat-icon>real_estate_agent</mat-icon>Voir le prêt
                </a>
              } @else {
                <button mat-menu-item (click)="convert(s)">
                  <mat-icon>real_estate_agent</mat-icon>Transformer en prêt
                </button>
              }
              <button mat-menu-item (click)="remove(s)">
                <mat-icon>delete</mat-icon>Supprimer
              </button>
            </mat-menu>
          </div>
        } @empty {
          <p class="muted">Aucune simulation. Faites un calcul ci-dessus, puis « Enregistrer ».</p>
        }
      </section>
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
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
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .row {
      display: grid;
      grid-template-columns: auto 1fr auto auto auto;
      align-items: center;
      gap: 0.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
      padding: 0.25rem 0;
      @media (max-width: 599.98px) {
        grid-template-columns: auto 1fr auto;
        .prepare {
          display: none;
        }
        .numbers {
          grid-column: 2;
          text-align: left;
        }
      }
    }
    .open {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      background: none;
      border: 0;
      padding: 0.25rem 0;
      color: inherit;
      font: inherit;
      text-align: left;
      cursor: pointer;
    }
    .numbers {
      display: flex;
      flex-direction: column;
      text-align: right;
    }
  `,
})
export class SimulationsPage {
  private readonly api = inject(CreditService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);

  protected readonly rateText = formatRate;
  protected readonly duration_ = formatDuration;

  /** Le projet choisi dans l'onglet : celui de la demande préparée (R-CRE-36). */
  readonly project = input.required<ProjectKind>();
  /** Une simulation ouverte : elle passe dans « Que coûte ce prêt ? » (R-CRE-37). */
  readonly open = output<SimulationView>();

  protected readonly checked = signal<ReadonlySet<string>>(new Set());

  private readonly list = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listLoanSimulations(),
  });
  protected readonly simulations = computed(() => this.list.value() ?? []);

  /** Prépare une demande depuis une simulation (R-CRE-36). */
  protected async prepare(s: SimulationView): Promise<void> {
    try {
      const view = await firstValueFrom(
        this.api.applicationFromSimulation({
          id: s.id,
          body: { name: s.name, project_kind: this.project() },
        }),
      );
      this.version.bump();
      await this.router.navigate(['/prets/demandes', view.id]);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  protected load(s: SimulationView): void {
    this.open.emit(s);
    window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  protected async edit(s: SimulationView): Promise<void> {
    if (await editSimulation(this.dialog, { simulation: s })) this.version.bump();
  }

  protected async duplicate(s: SimulationView): Promise<void> {
    const initial = {
      name: `${s.name} (copie)`,
      kind: s.kind,
      principal_minor: s.principal_minor,
      duration_months: s.duration_months,
      annual_rate_ppm: s.annual_rate_ppm,
      insurance: s.insurance,
      fees_minor: s.fees_minor,
      guarantee_fees_minor: s.guarantee_fees_minor,
      institution_id: s.institution_id ?? null,
      application_id: s.application_id ?? null,
    };
    if (await editSimulation(this.dialog, { simulation: null, initial })) this.version.bump();
  }

  protected schedule(s: SimulationView): void {
    showSchedule(this.dialog, s);
  }

  protected toggle(id: string): void {
    this.checked.update((set) => {
      const next = new Set(set);
      if (next.has(id)) next.delete(id);
      else if (next.size < 4) next.add(id);
      return next;
    });
  }

  protected compare(): void {
    compareSimulations(
      this.dialog,
      this.simulations().filter((s) => this.checked().has(s.id)),
    );
  }

  /** Transforme une simulation en prêt (R-CRE-7). */
  protected async convert(s: SimulationView): Promise<void> {
    const ref = this.dialog.open(LoanDialog, { data: { simulation: s }, maxWidth: '100vw' });
    const id = await firstValueFrom(ref.afterClosed());
    if (id) {
      this.version.bump();
      await this.router.navigate(['/prets', id]);
    }
  }

  protected async remove(s: SimulationView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer « ${s.name} » ?`,
      message: s.loan_id ? 'Le prêt créé à partir de cette simulation reste.' : '',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    await firstValueFrom(this.api.deleteLoanSimulation({ id: s.id }));
    this.version.bump();
  }
}
