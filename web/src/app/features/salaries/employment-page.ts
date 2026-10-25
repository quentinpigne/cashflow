import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { BonusView } from '../../api/models/bonus-view';
import { EmploymentView } from '../../api/models/employment-view';
import { MonthCheck } from '../../api/models/month-check';
import { PayslipView } from '../../api/models/payslip-view';
import { SalaryView } from '../../api/models/salary-view';
import { SalariesService } from '../../api/services/salaries.service';
import { DataVersion } from '../../core/data-version';
import { toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { BONUS_KIND_LABELS, SALARY_REASON_LABELS } from '../../core/labels';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { formatChange } from './salaries';
import {
  BonusDialog,
  EmploymentDialog,
  MONTH_NAMES,
  PayslipDialog,
  SalaryDialog,
  formatSize,
} from './salary-dialogs';

/** Une ligne de la liste des fiches : une fiche, ou un mois sans fiche (R-SAL-18). */
interface PayslipEntry {
  month: string;
  payslip: PayslipView | null;
}

interface PayslipYear {
  year: string;
  count: number;
  entries: PayslipEntry[];
}

/** E30 — Fiche d'un emploi : rémunérations, primes, revenu annuel, reçu, fiches de paie (SAL-01 à SAL-03, SAL-09). */
@Component({
  selector: 'app-employment-page',
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
      @if (job.value(); as j) {
        <header class="head">
          <div class="title">
            <a mat-icon-button routerLink="/salaires" aria-label="Retour aux salaires"
              ><mat-icon>arrow_back</mat-icon></a
            >
            <h1>{{ j.summary.employer }}</h1>
            <button mat-icon-button [matMenuTriggerFor]="menu" aria-label="Actions sur l'emploi">
              <mat-icon>more_vert</mat-icon>
            </button>
            <mat-menu #menu>
              <button mat-menu-item (click)="edit(j)"><mat-icon>edit</mat-icon>Modifier</button>
              <button mat-menu-item (click)="remove(j)">
                <mat-icon>delete</mat-icon>Supprimer
              </button>
            </mat-menu>
          </div>
          <p class="muted meta">
            {{ j.summary.job_title ? j.summary.job_title + ' · ' : '' }}depuis le
            {{ j.summary.start_date | frDate }}
            @if (j.summary.end_date) {
              · fin le {{ j.summary.end_date | frDate }}
            }
            · augmentations en {{ monthName(j.raise_month) }} · {{ j.income_category_name
            }}{{ j.paid_next_month ? ' · versement décalé' : '' }}
          </p>
          <div class="balance">
            <span class="muted">Net mensuel</span>
            <strong class="amount">
              @if (
                j.summary.current_net_minor !== null && j.summary.current_net_minor !== undefined
              ) {
                {{ j.summary.current_net_minor | money }}
              } @else {
                —
              }
            </strong>
          </div>
        </header>

        <section class="card panel">
          <header>
            <h2>Rémunérations</h2>
            <button mat-stroked-button (click)="addSalary(j)">
              <mat-icon>add</mat-icon>Ajouter
            </button>
          </header>
          <table>
            <thead>
              <tr>
                <th>Mois d'effet</th>
                <th class="wide">Motif</th>
                <th class="right wide">Brut annuel</th>
                <th class="right">Net mensuel</th>
                <th class="right">Variation</th>
                <th class="state"></th>
              </tr>
            </thead>
            <tbody>
              @for (s of j.salaries; track s.id) {
                <tr>
                  <td>
                    {{ s.effective_month | frDate: 'month' }}
                    @if (s.planned) {
                      <span class="muted small">prévue</span>
                    }
                  </td>
                  <td class="wide">{{ reasonLabels[s.reason] }}</td>
                  <td class="right amount wide">{{ s.annual_gross_minor | money }}</td>
                  <td class="right amount">{{ s.monthly_net_minor | money }}</td>
                  <td class="right">{{ change(s.net_change_ppm) }}</td>
                  <td class="state">
                    <button
                      mat-icon-button
                      [matMenuTriggerFor]="salaryMenu"
                      [attr.aria-label]="'Actions sur la rémunération de ' + s.effective_month"
                    >
                      <mat-icon>more_vert</mat-icon>
                    </button>
                    <mat-menu #salaryMenu>
                      <button mat-menu-item (click)="editSalary(j, s)">
                        <mat-icon>edit</mat-icon>Modifier
                      </button>
                      <button mat-menu-item (click)="removeSalary(j, s)">
                        <mat-icon>delete</mat-icon>Supprimer
                      </button>
                    </mat-menu>
                  </td>
                </tr>
              } @empty {
                <tr>
                  <td colspan="6" class="muted">Aucune rémunération déclarée.</td>
                </tr>
              }
            </tbody>
          </table>
        </section>

        <section class="card panel">
          <header>
            <h2>Primes</h2>
            <button mat-stroked-button (click)="editBonus(j, null)">
              <mat-icon>add</mat-icon>Ajouter
            </button>
          </header>
          @for (b of j.bonuses; track b.id) {
            <div class="row">
              <span>{{ b.paid_on | frDate: 'short' }} {{ b.paid_on.slice(0, 4) }}</span>
              <span>
                {{ b.label }}
                <span class="muted small"
                  >{{ bonusLabels[b.kind] }}{{ b.recurring ? ' · récurrente' : ''
                  }}{{ b.planned ? ' · prévue' : '' }}</span
                >
              </span>
              <span class="amount">{{ b.net_amount_minor | money }}</span>
              <button
                mat-icon-button
                [matMenuTriggerFor]="bonusMenu"
                [attr.aria-label]="'Actions sur la prime ' + b.label"
              >
                <mat-icon>more_vert</mat-icon>
              </button>
              <mat-menu #bonusMenu>
                <button mat-menu-item (click)="editBonus(j, b)">
                  <mat-icon>edit</mat-icon>Modifier
                </button>
                <button mat-menu-item (click)="removeBonus(j, b)">
                  <mat-icon>delete</mat-icon>Supprimer
                </button>
              </mat-menu>
            </div>
          } @empty {
            <p class="muted">Aucune prime.</p>
          }
        </section>

        @if (j.years.length > 0) {
          <section class="card panel">
            <header><h2>Revenu annuel</h2></header>
            <table>
              <thead>
                <tr>
                  <th>Année</th>
                  <th class="right">Net</th>
                  <th class="right wide">Brut</th>
                  <th class="right">Variation</th>
                </tr>
              </thead>
              <tbody>
                @for (y of j.years; track y.year) {
                  <tr>
                    <td>
                      {{ y.year }}
                      @if (y.provisional) {
                        <span class="muted small">prévisionnel</span>
                      } @else if (y.incomplete) {
                        <span class="muted small">incomplet</span>
                      } @else if (y.months < 12) {
                        <span class="muted small">{{ y.months }} mois</span>
                      }
                    </td>
                    <td class="right amount">{{ y.net_minor | money }}</td>
                    <td class="right amount wide">
                      {{ y.gross_minor | money }}{{ y.gross_incomplete ? ' *' : '' }}
                    </td>
                    <td class="right">{{ change(y.net_change_ppm) }}</td>
                  </tr>
                }
              </tbody>
            </table>
          </section>
        }

        <section class="card panel">
          <header>
            <h2>Reçu</h2>
            <span class="muted small">sur « {{ j.income_category_name }} »</span>
          </header>
          <table>
            <thead>
              <tr>
                <th>Mois</th>
                <th class="right wide">Attendu</th>
                <th class="right">Reçu</th>
                <th class="right">Écart</th>
              </tr>
            </thead>
            <tbody>
              @for (c of j.checks; track c.month) {
                <tr
                  class="link"
                  [class.flagged]="c.flagged"
                  tabindex="0"
                  (click)="openMonth(j, c)"
                  (keydown.enter)="openMonth(j, c)"
                >
                  <td>
                    {{ c.month | frDate: 'month' }}
                    @if (c.in_progress) {
                      <span class="muted small">en cours</span>
                    } @else if (c.partial) {
                      <span class="muted small">partiel</span>
                    } @else if (c.unknown) {
                      <span class="muted small">non renseigné</span>
                    }
                  </td>
                  <td class="right amount wide">{{ c.expected_minor | money }}</td>
                  <td class="right amount">{{ c.received_minor | money }}</td>
                  <td class="right amount">
                    @if (c.flagged) {
                      <mat-icon class="warn" aria-label="Écart signalé">warning</mat-icon>
                    }
                    {{ c.gap_minor | money: 'EUR' : { signed: true } }}
                  </td>
                </tr>
              } @empty {
                <tr>
                  <td colspan="4" class="muted">
                    Rien d'attendu ni de reçu sur les 12 derniers mois.
                  </td>
                </tr>
              }
            </tbody>
          </table>
        </section>

        <section
          class="card panel payslips"
          [class.dragging]="dragging()"
          (dragover)="dragOver($event)"
          (dragleave)="dragging.set(false)"
          (drop)="drop($event, j)"
        >
          <header>
            <h2>Fiches de paie</h2>
            <button mat-stroked-button (click)="addPayslip(j, null)">
              <mat-icon>add</mat-icon>Ajouter
            </button>
          </header>
          @for (y of payslipYears(); track y.year; let first = $first) {
            <details [open]="first">
              <summary>
                {{ y.year }}
                <span class="muted small">{{ y.count }} fiche{{ y.count > 1 ? 's' : '' }}</span>
              </summary>
              @for (e of y.entries; track $index) {
                @if (e.payslip; as p) {
                  <div class="row payslip">
                    <span>{{ e.month | frDate: 'month' }}</span>
                    <span class="name">
                      {{ p.file_name }}
                      <span class="muted small"
                        >{{ size(p.size_bytes) }}{{ p.note ? ' · ' + p.note : '' }}</span
                      >
                    </span>
                    <span class="actions">
                      <a
                        mat-icon-button
                        [href]="fileUrl(j, p, false)"
                        target="_blank"
                        rel="noopener"
                        [attr.aria-label]="'Ouvrir la fiche ' + p.file_name"
                        ><mat-icon>open_in_new</mat-icon></a
                      >
                      <a
                        mat-icon-button
                        class="wide"
                        [href]="fileUrl(j, p, true)"
                        download
                        [attr.aria-label]="'Télécharger la fiche ' + p.file_name"
                        ><mat-icon>download</mat-icon></a
                      >
                      <button
                        mat-icon-button
                        [matMenuTriggerFor]="payslipMenu"
                        [attr.aria-label]="'Actions sur la fiche ' + p.file_name"
                      >
                        <mat-icon>more_vert</mat-icon>
                      </button>
                    </span>
                    <mat-menu #payslipMenu>
                      <a mat-menu-item [href]="fileUrl(j, p, true)" download>
                        <mat-icon>download</mat-icon>Télécharger
                      </a>
                      <button mat-menu-item (click)="addPayslip(j, p)">
                        <mat-icon>edit</mat-icon>Modifier
                      </button>
                      <button mat-menu-item (click)="removePayslip(j, p)">
                        <mat-icon>delete</mat-icon>Supprimer
                      </button>
                    </mat-menu>
                  </div>
                } @else {
                  <div class="row payslip missing">
                    <span>{{ e.month | frDate: 'month' }}</span>
                    <span class="small">sans fiche</span>
                  </div>
                }
              }
            </details>
          } @empty {
            <p class="muted">
              Aucune fiche archivée : déposez un PDF, un JPEG ou un PNG ici, ou utilisez « Ajouter
              ».
            </p>
          }
        </section>
      } @else if (job.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      } @else {
        <p class="empty">Emploi introuvable.</p>
      }
    </div>
  `,
  styleUrl: '../loans/loan-page.scss',
  styles: `
    tr.link {
      cursor: pointer;
      &:hover td {
        background: var(--mat-sys-surface-container);
      }
    }
    tr.flagged td {
      color: var(--mat-sys-error);
    }
    .warn {
      vertical-align: middle;
      font-size: 1.1rem;
      width: 1.1rem;
      height: 1.1rem;
    }
    .payslips {
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
    .row.payslip {
      grid-template-columns: 9rem 1fr auto;
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
export class EmploymentPage {
  private readonly api = inject(SalariesService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  protected readonly reasonLabels = SALARY_REASON_LABELS;
  protected readonly bonusLabels = BONUS_KIND_LABELS;
  protected readonly change = formatChange;

  readonly id = input.required<string>();

  protected readonly job = rxResource({
    params: () => ({ id: this.id(), version: this.version.value() }),
    stream: ({ params }) => this.api.getEmployment({ id: params.id }),
  });

  protected readonly size = formatSize;
  protected readonly dragging = signal(false);

  /** Fiches et mois sans fiche, par année, du plus récent au plus ancien (R-SAL-18). */
  protected readonly payslipYears = computed<PayslipYear[]>(() => {
    const j = this.job.value();
    if (!j) return [];
    const entries: PayslipEntry[] = [
      ...j.payslips.map((p) => ({ month: p.pay_month, payslip: p })),
      ...j.missing_payslip_months.map((month) => ({ month, payslip: null })),
    ].sort((a, b) => b.month.localeCompare(a.month));
    const years: PayslipYear[] = [];
    for (const entry of entries) {
      const year = entry.month.slice(0, 4);
      let group = years.at(-1);
      if (group?.year !== year) {
        group = { year, count: 0, entries: [] };
        years.push(group);
      }
      group.entries.push(entry);
      if (entry.payslip) group.count++;
    }
    return years;
  });

  protected monthName(month: number): string {
    return MONTH_NAMES[month - 1]?.toLowerCase() ?? '';
  }

  protected async edit(j: EmploymentView): Promise<void> {
    const ref = this.dialog.open(EmploymentDialog, { data: j, maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async remove(j: EmploymentView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: "Supprimer l'emploi ?",
      message:
        'Ses rémunérations, ses primes et ses fiches de paie sont supprimées ; les transactions ne changent pas.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteEmployment({ id: j.summary.id }));
      this.version.bump();
      await this.router.navigate(['/salaires']);
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  /** Nouvelle rémunération, préremplie avec la dernière (R-SAL-2). */
  protected async addSalary(j: EmploymentView): Promise<void> {
    const data = { employmentId: j.summary.id, salary: j.salaries[0] ?? null, editing: false };
    const ref = this.dialog.open(SalaryDialog, { data, maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async editSalary(j: EmploymentView, salary: SalaryView): Promise<void> {
    const data = { employmentId: j.summary.id, salary, editing: true };
    const ref = this.dialog.open(SalaryDialog, { data, maxWidth: '100vw' });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async removeSalary(j: EmploymentView, salary: SalaryView): Promise<void> {
    try {
      await firstValueFrom(this.api.deleteSalary({ id: j.summary.id, salary_id: salary.id }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  protected async editBonus(j: EmploymentView, bonus: BonusView | null): Promise<void> {
    const ref = this.dialog.open(BonusDialog, {
      data: { employmentId: j.summary.id, bonus },
      maxWidth: '100vw',
    });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected async removeBonus(j: EmploymentView, bonus: BonusView): Promise<void> {
    try {
      await firstValueFrom(this.api.deleteBonus({ id: j.summary.id, bonus_id: bonus.id }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  protected fileUrl(j: EmploymentView, payslip: PayslipView, download: boolean): string {
    const url = `/api/v1/employments/${j.summary.id}/payslips/${payslip.id}/file`;
    return download ? `${url}?download=true` : url;
  }

  /** Nouvelle fiche (`payslip` absent), éventuellement avec un fichier déposé, ou modification (R-SAL-17). */
  protected async addPayslip(
    j: EmploymentView,
    payslip: PayslipView | null,
    file: File | null = null,
  ): Promise<void> {
    const ref = this.dialog.open(PayslipDialog, {
      data: { employmentId: j.summary.id, payslip, file },
      maxWidth: '100vw',
    });
    if (await firstValueFrom(ref.afterClosed())) this.version.bump();
  }

  protected dragOver(event: DragEvent): void {
    if (!event.dataTransfer?.types.includes('Files')) return;
    event.preventDefault();
    this.dragging.set(true);
  }

  protected async drop(event: DragEvent, j: EmploymentView): Promise<void> {
    event.preventDefault();
    this.dragging.set(false);
    const file = event.dataTransfer?.files[0];
    if (file) await this.addPayslip(j, null, file);
  }

  protected async removePayslip(j: EmploymentView, payslip: PayslipView): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer la fiche de paie ?',
      message: `« ${payslip.file_name} » ne sera plus consultable.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deletePayslip({ id: j.summary.id, payslip_id: payslip.id }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 4000 });
    }
  }

  /** Transactions de la catégorie sur ce mois (E14 filtrée). */
  protected async openMonth(j: EmploymentView, check: MonthCheck): Promise<void> {
    const [year, month] = check.month.split('-').map(Number);
    await this.router.navigate(['/transactions'], {
      queryParams: {
        categorie: j.summary.income_category_id,
        du: check.month,
        au: toIsoDate(new Date(year, month, 0)),
      },
    });
  }
}
