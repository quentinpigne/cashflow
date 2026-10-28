import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ScheduleView } from '../../api/models/schedule-view';
import { SavingsService } from '../../api/services/savings.service';
import { DataVersion } from '../../core/data-version';
import { toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { formatRate, parseRatePpm } from '../../core/money';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';

interface Draft {
  date: Date | null;
  rate: string;
}

/**
 * Barèmes de taux (EPA-01) : les barèmes réglementés partagés et ceux propres à un compte, avec leur
 * historique daté. Les taux réglementés sont à vérifier à la source officielle (R-EPA-12).
 */
@Component({
  selector: 'app-schedules',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/reglages" aria-label="Retour aux réglages">
          <mat-icon>arrow_back</mat-icon>
        </a>
        <h1>Barèmes de taux</h1>
      </div>
      <p class="muted">
        Les taux réglementés (Livret A, LDDS, LEP) sont fournis avec l'application : vérifie-les sur
        le site officiel (service-public.fr, Banque de France) et ajoute chaque nouveau taux à sa
        date d'effet. Les intérêts de tous les comptes du barème sont recalculés aussitôt.
      </p>
      @for (s of schedules.value() ?? []; track s.id) {
        <section class="card">
          <header>
            <h2>{{ s.name }}</h2>
            @if (s.shared) {
              <span class="tag">réglementé</span>
            }
          </header>
          <p class="muted small">
            {{ s.accounts.length ? 'Utilisé par ' + s.accounts.join(', ') : 'Aucun compte' }}
          </p>
          @for (p of reversed(s); track p.effective_from) {
            <div class="row">
              <span>À partir du {{ p.effective_from | frDate }}</span>
              <strong class="amount">{{ rate(p.rate_ppm) }}</strong>
              <button
                mat-icon-button
                [attr.aria-label]="'Supprimer le taux du ' + p.effective_from"
                (click)="remove(s, p.effective_from)"
              >
                <mat-icon>delete</mat-icon>
              </button>
            </div>
          } @empty {
            <p class="muted">Aucun taux : les intérêts valent zéro.</p>
          }
          <div class="add">
            <mat-form-field subscriptSizing="dynamic">
              <mat-label>À partir du</mat-label>
              <input
                matInput
                [matDatepicker]="picker"
                [ngModel]="draft(s).date"
                (ngModelChange)="setDraft(s, { date: $event })"
              />
              <mat-datepicker-toggle matIconSuffix [for]="picker" />
              <mat-datepicker #picker />
            </mat-form-field>
            <mat-form-field subscriptSizing="dynamic" class="rate">
              <mat-label>Taux annuel</mat-label>
              <input
                matInput
                inputmode="decimal"
                [ngModel]="draft(s).rate"
                (ngModelChange)="setDraft(s, { rate: $event })"
              />
              <span matTextSuffix>%</span>
            </mat-form-field>
            <button mat-stroked-button [disabled]="!canAdd(s)" (click)="add(s)">Ajouter</button>
          </div>
        </section>
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    header {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      h2 {
        margin: 0;
        font: var(--mat-sys-title-medium);
      }
    }
    .tag {
      font: var(--mat-sys-label-small);
      background: var(--mat-sys-secondary-container);
      border-radius: 0.5rem;
      padding: 0.125rem 0.5rem;
    }
    .small {
      font: var(--mat-sys-body-small);
    }
    .row {
      display: grid;
      grid-template-columns: 1fr auto auto;
      align-items: center;
      gap: 0.5rem;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .add {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--field-gap);
      margin-top: 0.75rem;
    }
    .rate {
      width: 9rem;
    }
  `,
})
export class SchedulesPage {
  private readonly api = inject(SavingsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);

  protected readonly schedules = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listRateSchedules(),
  });
  private readonly drafts = signal<Record<string, Draft>>({});

  protected rate(ppm: number): string {
    return formatRate(ppm);
  }

  protected reversed(s: ScheduleView) {
    return [...s.periods].reverse();
  }

  protected draft(s: ScheduleView): Draft {
    return this.drafts()[s.id] ?? { date: null, rate: '' };
  }

  protected setDraft(s: ScheduleView, change: Partial<Draft>): void {
    this.drafts.update((d) => ({ ...d, [s.id]: { ...this.draft(s), ...change } }));
  }

  protected canAdd(s: ScheduleView): boolean {
    const d = this.draft(s);
    return !!d.date && parseRatePpm(d.rate) !== null;
  }

  protected async add(s: ScheduleView): Promise<void> {
    const d = this.draft(s);
    const ppm = parseRatePpm(d.rate);
    if (!d.date || ppm === null) return;
    try {
      await firstValueFrom(
        this.api.setRate({ id: s.id, body: { effective_from: toIsoDate(d.date), rate_ppm: ppm } }),
      );
      this.drafts.update((all) => ({ ...all, [s.id]: { date: null, rate: '' } }));
      this.version.bump();
    } catch (e) {
      this.snack.open(errorMessage(e), undefined, { duration: 5000 });
    }
  }

  protected async remove(s: ScheduleView, from: string): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer ce taux ?',
      message: `Les intérêts des comptes du barème « ${s.name} » seront recalculés.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    await firstValueFrom(this.api.deleteRate({ id: s.id, from }));
    this.version.bump();
  }
}
