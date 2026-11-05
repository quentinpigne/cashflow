import { ChangeDetectionStrategy, Component, computed, input, output } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepicker, MatDatepickerModule } from '@angular/material/datepicker';
import { MatIconModule } from '@angular/material/icon';

import { formatMonthKey, fromIsoDate, shiftMonth, toIsoDate } from '../core/dates';

/**
 * Choix d'un mois (`AAAA-MM`) ou d'une année : flèches précédent / suivant, et un clic sur la période affichée
 * ouvre le calendrier sur les années, puis les mois, pour aller loin d'un coup.
 */
@Component({
  selector: 'app-period-picker',
  imports: [MatButtonModule, MatDatepickerModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <button
      mat-icon-button
      [attr.aria-label]="mode() === 'year' ? 'Année précédente' : 'Mois précédent'"
      (click)="shift(-1)"
    >
      <mat-icon>chevron_left</mat-icon>
    </button>
    <span class="anchor">
      <button
        mat-button
        class="label"
        [attr.aria-label]="
          (mode() === 'year' ? 'Choisir l’année, ' : 'Choisir le mois, ') + label()
        "
        (click)="picker.open()"
      >
        {{ label() }}
        <mat-icon iconPositionEnd>arrow_drop_down</mat-icon>
      </button>
      <input
        tabindex="-1"
        aria-hidden="true"
        [matDatepicker]="picker"
        [value]="date()"
        [max]="maxDate()"
      />
      <mat-datepicker
        #picker
        startView="multi-year"
        (yearSelected)="mode() === 'year' && chooseYear($event, picker)"
        (monthSelected)="chooseMonth($event, picker)"
      />
    </span>
    <button
      mat-icon-button
      [attr.aria-label]="mode() === 'year' ? 'Année suivante' : 'Mois suivant'"
      (click)="shift(1)"
      [disabled]="atMax()"
    >
      <mat-icon>chevron_right</mat-icon>
    </button>
  `,
  styles: `
    :host {
      display: inline-flex;
      align-items: center;
    }

    .anchor {
      position: relative;
      display: inline-flex;
    }

    .label {
      font-weight: 600;
      text-transform: capitalize;
    }

    /* Le calendrier s'ancre sous la période ; le champ lui-même reste invisible. */
    input {
      position: absolute;
      inset: auto 0 0 0;
      width: 100%;
      height: 0;
      padding: 0;
      border: 0;
      opacity: 0;
      pointer-events: none;
    }
  `,
})
export class PeriodPicker {
  /** `month` : `value` vaut `AAAA-MM` ; `year` : `value` vaut l'année. */
  readonly mode = input<'month' | 'year'>('month');
  readonly value = input.required<string | number>();
  /** Dernière période proposée (le mois ou l'année en cours, par exemple) ; absente : pas de limite. */
  readonly max = input<string | number | null>(null);
  readonly valueChange = output<string | number>();

  protected readonly label = computed(() =>
    this.mode() === 'year' ? String(this.value()) : formatMonthKey(String(this.value())),
  );
  protected readonly date = computed(() => this.toDate(this.value()));
  protected readonly maxDate = computed(() => {
    const max = this.max();
    if (max === null) return null;
    // Dernier jour de la période : tout le mois (ou l'année) reste choisissable.
    if (this.mode() === 'year') return fromIsoDate(`${max}-12-31`);
    const next = fromIsoDate(`${shiftMonth(String(max), 1)}-01`);
    next.setDate(0);
    return next;
  });
  protected readonly atMax = computed(() => {
    const max = this.max();
    return max !== null && String(this.value()) >= String(max);
  });

  protected shift(delta: number): void {
    if (this.mode() === 'year') {
      this.valueChange.emit(Number(this.value()) + delta);
    } else {
      this.valueChange.emit(shiftMonth(String(this.value()), delta));
    }
  }

  protected chooseYear(date: Date, picker: MatDatepicker<Date>): void {
    this.valueChange.emit(date.getFullYear());
    picker.close();
  }

  /** En mode année, le choix s'arrête à l'année : le calendrier est déjà fermé. */
  protected chooseMonth(date: Date, picker: MatDatepicker<Date>): void {
    if (this.mode() !== 'month') return;
    this.valueChange.emit(toIsoDate(date).slice(0, 7));
    picker.close();
  }

  private toDate(value: string | number): Date {
    return this.mode() === 'year' ? fromIsoDate(`${value}-01-01`) : fromIsoDate(`${value}-01`);
  }
}
