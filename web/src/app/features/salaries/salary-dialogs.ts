import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  linkedSignal,
  signal,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { firstValueFrom } from 'rxjs';

import { BonusKind } from '../../api/models/bonus-kind';
import { BonusView } from '../../api/models/bonus-view';
import { EmploymentView } from '../../api/models/employment-view';
import { SalaryChangeReason } from '../../api/models/salary-change-reason';
import { SalaryView } from '../../api/models/salary-view';
import { SalariesService } from '../../api/services/salaries.service';
import { fromIsoDate, todayIso, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { BONUS_KIND_LABELS, SALARY_REASON_LABELS } from '../../core/labels';
import { parseMoney, toEditable } from '../../core/money';
import { CategoriesStore } from '../../shared/stores';

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

/** Mois de l'année, pour le mois des augmentations et le mois d'effet. */
export const MONTH_NAMES = [
  'Janvier',
  'Février',
  'Mars',
  'Avril',
  'Mai',
  'Juin',
  'Juillet',
  'Août',
  'Septembre',
  'Octobre',
  'Novembre',
  'Décembre',
];

/** Création ou modification d'un emploi (E30, R-SAL-1). */
@Component({
  selector: 'app-employment-dialog',
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
    <h2 mat-dialog-title>{{ job ? "Modifier l'emploi" : 'Nouvel emploi' }}</h2>
    <mat-dialog-content class="form">
      <div class="pair">
        <mat-form-field>
          <mat-label>Employeur</mat-label>
          <input
            matInput
            maxlength="80"
            [ngModel]="employer()"
            (ngModelChange)="employer.set($event)"
          />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Poste</mat-label>
          <input matInput maxlength="80" [ngModel]="title()" (ngModelChange)="title.set($event)" />
        </mat-form-field>
      </div>
      <div class="pair">
        <mat-form-field>
          <mat-label>Début</mat-label>
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
          <mat-hint>Changement d'emploi ou fin d'activité, même prévus.</mat-hint>
        </mat-form-field>
      </div>
      <div class="pair">
        <mat-form-field>
          <mat-label>Catégorie de revenu</mat-label>
          <mat-select [ngModel]="category()" (ngModelChange)="category.set($event)">
            @for (c of incomeCategories(); track c.id) {
              <mat-option [value]="c.id">{{ c.label }}</mat-option>
            }
          </mat-select>
          <mat-hint>Le salaire reçu sur cette catégorie est comparé au net attendu.</mat-hint>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Mois des augmentations</mat-label>
          <mat-select [ngModel]="raiseMonth()" (ngModelChange)="raiseMonth.set($event)">
            @for (m of months; track $index) {
              <mat-option [value]="$index + 1">{{ m }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
      </div>
      <mat-checkbox [ngModel]="paidNextMonth()" (ngModelChange)="paidNextMonth.set($event)">
        Versement décalé : le salaire du mois est versé le mois suivant
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
        {{ job ? 'Enregistrer' : "Créer l'emploi" }}
      </button>
    </mat-dialog-actions>
  `,
  styles: FORM_STYLES,
})
export class EmploymentDialog {
  protected readonly job = inject<EmploymentView | null>(MAT_DIALOG_DATA);
  private readonly api = inject(SalariesService);
  private readonly ref = inject(MatDialogRef<EmploymentDialog>);
  private readonly categories = inject(CategoriesStore);

  protected readonly months = MONTH_NAMES;
  protected readonly incomeCategories = computed(() =>
    this.categories.flat().filter((c) => c.kind === 'INCOME' && !c.archived),
  );
  private readonly s = this.job?.summary;
  protected readonly employer = signal(this.s?.employer ?? '');
  protected readonly title = signal(this.s?.job_title ?? '');
  protected readonly start = signal<Date | null>(
    this.s ? fromIsoDate(this.s.start_date) : fromIsoDate(todayIso()),
  );
  protected readonly end = signal<Date | null>(
    this.s?.end_date ? fromIsoDate(this.s.end_date) : null,
  );
  /** « Salaires » par défaut (R-SAL-1). */
  protected readonly category = linkedSignal<string | null>(
    () =>
      this.s?.income_category_id ??
      this.incomeCategories().find((c) => c.name === 'Salaires' && !c.parentId)?.id ??
      null,
  );
  protected readonly raiseMonth = signal(this.job?.raise_month ?? 1);
  protected readonly paidNextMonth = signal(this.job?.paid_next_month ?? false);
  protected readonly notes = signal(this.job?.notes ?? '');
  protected readonly error = signal<string | null>(null);

  protected readonly body = computed(() => {
    const employer = this.employer().trim();
    const start = this.start();
    const category = this.category();
    if (!employer || !start || !category) return null;
    const end = this.end();
    return {
      employer,
      job_title: this.title().trim() || null,
      start_date: toIsoDate(start),
      end_date: end ? toIsoDate(end) : null,
      income_category_id: category,
      raise_month: this.raiseMonth(),
      paid_next_month: this.paidNextMonth(),
      notes: this.notes().trim() || null,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    try {
      const saved = this.job
        ? await firstValueFrom(this.api.updateEmployment({ id: this.job.summary.id, body }))
        : await firstValueFrom(this.api.createEmployment({ body }));
      this.ref.close(saved.summary.id);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

export interface SalaryDialogData {
  employmentId: string;
  /** Rémunération modifiée, ou la dernière pour préremplir une nouvelle (R-SAL-2). */
  salary: SalaryView | null;
  editing: boolean;
}

/** Rémunération en vigueur à partir d'un mois (R-SAL-2). */
@Component({
  selector: 'app-salary-dialog',
  imports: [
    FormsModule,
    MatDialogModule,
    MatButtonModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>
      {{ data.editing ? 'Modifier la rémunération' : 'Nouvelle rémunération' }}
    </h2>
    <mat-dialog-content class="form">
      <div class="pair">
        <mat-form-field>
          <mat-label>Mois d'effet</mat-label>
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
      <mat-form-field>
        <mat-label>Motif</mat-label>
        <mat-select [ngModel]="reason()" (ngModelChange)="reason.set($event)">
          @for (r of reasons; track r) {
            <mat-option [value]="r">{{ reasonLabels[r] }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <div class="pair">
        <mat-form-field>
          <mat-label>Brut annuel de base</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="gross()"
            (ngModelChange)="gross.set($event)"
          />
          <span matTextSuffix>€</span>
          <mat-hint>Hors primes et 13e mois.</mat-hint>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Net mensuel versé</mat-label>
          <input matInput inputmode="decimal" [ngModel]="net()" (ngModelChange)="net.set($event)" />
          <span matTextSuffix>€</span>
          <mat-hint>Net payé, après prélèvement à la source.</mat-hint>
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
  styles: FORM_STYLES,
})
export class SalaryDialog {
  protected readonly data = inject<SalaryDialogData>(MAT_DIALOG_DATA);
  private readonly api = inject(SalariesService);
  private readonly ref = inject(MatDialogRef<SalaryDialog>);

  protected readonly months = MONTH_NAMES;
  protected readonly reasons: SalaryChangeReason[] = [
    'HIRE',
    'RAISE',
    'PROMOTION',
    'TAX_RATE',
    'OTHER',
  ];
  protected readonly reasonLabels = SALARY_REASON_LABELS;
  private readonly s = this.data.salary;
  private readonly initial = this.data.editing && this.s ? this.s.effective_month : todayIso();
  protected readonly month = signal(Number(this.initial.slice(5, 7)));
  protected readonly year = signal(Number(this.initial.slice(0, 4)));
  protected readonly reason = signal<SalaryChangeReason>(
    this.data.editing && this.s ? this.s.reason : this.s ? 'RAISE' : 'HIRE',
  );
  protected readonly gross = signal(this.s ? toEditable(this.s.annual_gross_minor) : '');
  protected readonly net = signal(this.s ? toEditable(this.s.monthly_net_minor) : '');
  protected readonly note = signal(this.data.editing ? (this.s?.note ?? '') : '');
  protected readonly error = signal<string | null>(null);

  protected readonly body = computed(() => {
    const gross = parseMoney(this.gross());
    const net = parseMoney(this.net());
    const year = Number(this.year());
    if (gross === null || gross <= 0 || net === null || net <= 0) return null;
    if (!Number.isInteger(year) || year < 1950 || year > 2100) return null;
    return {
      effective_month: `${year}-${String(this.month()).padStart(2, '0')}-01`,
      annual_gross_minor: gross,
      monthly_net_minor: net,
      reason: this.reason(),
      note: this.note().trim() || null,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    const id = this.data.employmentId;
    try {
      if (this.data.editing && this.s) {
        await firstValueFrom(this.api.updateSalary({ id, salary_id: this.s.id, body }));
      } else {
        await firstValueFrom(this.api.createSalary({ id, body }));
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}

export interface BonusDialogData {
  employmentId: string;
  bonus: BonusView | null;
}

/** Prime versée à une date (R-SAL-5). */
@Component({
  selector: 'app-bonus-dialog',
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
    <h2 mat-dialog-title>{{ data.bonus ? 'Modifier la prime' : 'Nouvelle prime' }}</h2>
    <mat-dialog-content class="form">
      <div class="pair">
        <mat-form-field>
          <mat-label>Nature</mat-label>
          <mat-select [ngModel]="kind()" (ngModelChange)="changeKind($event)">
            @for (k of kinds; track k) {
              <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Libellé</mat-label>
          <input matInput maxlength="80" [ngModel]="label()" (ngModelChange)="label.set($event)" />
        </mat-form-field>
      </div>
      <mat-form-field>
        <mat-label>Date de versement</mat-label>
        <input
          matInput
          [matDatepicker]="picker"
          [ngModel]="paidOn()"
          (ngModelChange)="paidOn.set($event)"
        />
        <mat-datepicker-toggle matIconSuffix [for]="picker" />
        <mat-datepicker #picker />
      </mat-form-field>
      <div class="pair">
        <mat-form-field>
          <mat-label>Net versé</mat-label>
          <input matInput inputmode="decimal" [ngModel]="net()" (ngModelChange)="net.set($event)" />
          <span matTextSuffix>€</span>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Brut</mat-label>
          <input
            matInput
            inputmode="decimal"
            [ngModel]="gross()"
            (ngModelChange)="gross.set($event)"
          />
          <span matTextSuffix>€</span>
          <mat-hint>Facultatif.</mat-hint>
        </mat-form-field>
      </div>
      <mat-checkbox [ngModel]="recurring()" (ngModelChange)="recurring.set($event)">
        Récurrente : se répète chaque année dans la projection
      </mat-checkbox>
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
  styles: FORM_STYLES,
})
export class BonusDialog {
  protected readonly data = inject<BonusDialogData>(MAT_DIALOG_DATA);
  private readonly api = inject(SalariesService);
  private readonly ref = inject(MatDialogRef<BonusDialog>);

  protected readonly kinds: BonusKind[] = ['THIRTEENTH_MONTH', 'BONUS', 'PROFIT_SHARING', 'OTHER'];
  protected readonly kindLabels = BONUS_KIND_LABELS;
  private readonly b = this.data.bonus;
  protected readonly kind = signal<BonusKind>(this.b?.kind ?? 'THIRTEENTH_MONTH');
  protected readonly label = signal(this.b?.label ?? BONUS_KIND_LABELS.THIRTEENTH_MONTH);
  protected readonly paidOn = signal<Date | null>(fromIsoDate(this.b?.paid_on ?? todayIso()));
  protected readonly net = signal(this.b ? toEditable(this.b.net_amount_minor) : '');
  protected readonly gross = signal(
    this.b?.gross_amount_minor != null ? toEditable(this.b.gross_amount_minor) : '',
  );
  /** Oui par défaut pour un 13e mois (R-SAL-5). */
  protected readonly recurring = signal(this.b?.recurring ?? true);
  protected readonly note = signal(this.b?.note ?? '');
  protected readonly error = signal<string | null>(null);

  protected changeKind(kind: BonusKind): void {
    const previous = this.kind();
    if (!this.b && this.label() === BONUS_KIND_LABELS[previous]) {
      this.label.set(BONUS_KIND_LABELS[kind]);
    }
    if (!this.b) this.recurring.set(kind === 'THIRTEENTH_MONTH');
    this.kind.set(kind);
  }

  protected readonly body = computed(() => {
    const label = this.label().trim();
    const date = this.paidOn();
    const net = parseMoney(this.net());
    const gross = this.gross().trim() ? parseMoney(this.gross()) : null;
    if (!label || !date || net === null || net <= 0) return null;
    if (this.gross().trim() && (gross === null || gross <= 0)) return null;
    return {
      label,
      kind: this.kind(),
      paid_on: toIsoDate(date),
      net_amount_minor: net,
      gross_amount_minor: gross,
      recurring: this.recurring(),
      note: this.note().trim() || null,
    };
  });

  protected async save(): Promise<void> {
    const body = this.body();
    if (!body) return;
    const id = this.data.employmentId;
    try {
      if (this.b) {
        await firstValueFrom(this.api.updateBonus({ id, bonus_id: this.b.id, body }));
      } else {
        await firstValueFrom(this.api.createBonus({ id, body }));
      }
      this.ref.close(true);
    } catch (e) {
      this.error.set(errorMessage(e));
    }
  }
}
