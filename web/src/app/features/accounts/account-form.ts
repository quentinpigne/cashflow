import {
  ChangeDetectionStrategy,
  Component,
  Injectable,
  computed,
  inject,
  signal,
} from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
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
import { MatSelectModule } from '@angular/material/select';
import { Router } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { Account } from '../../api/models/account';
import { AccountKind } from '../../api/models/account-kind';
import { AccountsService } from '../../api/services/accounts.service';
import { InstitutionsService } from '../../api/services/institutions.service';
import { DataVersion } from '../../core/data-version';
import { fromIsoDate, toIsoDate } from '../../core/dates';
import { errorMessage, problemOf } from '../../core/http-errors';
import {
  FAMILY_LABELS,
  FAMILY_ORDER,
  KIND_FAMILY,
  KIND_LABELS,
  defaultFlags,
} from '../../core/labels';
import { Layout } from '../../core/layout';
import { parseMoney, toEditable } from '../../core/money';
import { InstitutionsStore } from '../../shared/stores';

const NEW_INSTITUTION = '__new__';

/** Ouvre le formulaire de compte : dialogue sur ordinateur, plein écran sur mobile (ecrans.md E4). */
@Injectable({ providedIn: 'root' })
export class AccountForm {
  private readonly dialog = inject(MatDialog);
  private readonly layout = inject(Layout);

  open(account?: Account): void {
    const mobile = this.layout.mobile();
    this.dialog.open(AccountFormDialog, {
      data: account ?? null,
      width: mobile ? '100vw' : '36rem',
      maxWidth: '100vw',
      panelClass: mobile ? 'fullscreen-dialog' : undefined,
    });
  }
}

/** E4 — Créer ou modifier un compte (CPT-01). */
@Component({
  selector: 'app-account-form',
  imports: [
    ReactiveFormsModule,
    MatDialogModule,
    MatButtonModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatExpansionModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './account-form.html',
  styleUrl: './account-form.scss',
})
export class AccountFormDialog {
  protected readonly account = inject<Account | null>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<AccountFormDialog>);
  private readonly api = inject(AccountsService);
  private readonly institutionsApi = inject(InstitutionsService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);
  protected readonly institutions = inject(InstitutionsStore);

  protected readonly kindLabels = KIND_LABELS;
  protected readonly newInstitution = NEW_INSTITUTION;
  protected readonly kindGroups = FAMILY_ORDER.map((family) => ({
    label: FAMILY_LABELS[family],
    kinds: (Object.keys(KIND_FAMILY) as AccountKind[]).filter((k) => KIND_FAMILY[k] === family),
  }));

  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly ibanError = signal<string | null>(null);
  protected readonly creatingInstitution = signal(false);

  private readonly fb = inject(FormBuilder).nonNullable;
  protected readonly form = this.fb.group({
    kind: [this.account?.kind ?? ('CHECKING' as AccountKind), Validators.required],
    name: [this.account?.name ?? '', [Validators.required, Validators.maxLength(80)]],
    institutionId: [this.account?.institution_id ?? ''],
    newInstitutionName: [''],
    openingDate: [
      this.account ? fromIsoDate(this.account.opening_date) : new Date(),
      Validators.required,
    ],
    openingBalance: [
      this.account ? toEditable(this.account.opening_balance_minor, this.account.currency) : '0',
      [
        Validators.required,
        (c: { value: string }) => (parseMoney(c.value) === null ? { amount: true } : null),
      ],
    ],
    iban: [''],
    accountNumber: [this.account?.account_number ?? ''],
    currency: [
      this.account?.currency ?? 'EUR',
      [Validators.required, Validators.pattern(/^[A-Za-z]{3}$/)],
    ],
    netWorth: [this.account?.include_in_net_worth ?? true],
    emergency: [this.account?.include_in_emergency_fund ?? defaultFlags('CHECKING').emergency],
    budget: [this.account?.include_in_budget ?? defaultFlags('CHECKING').budget],
    notes: [this.account?.notes ?? ''],
  });

  private readonly kind = toSignal(this.form.controls.kind.valueChanges, {
    initialValue: this.form.controls.kind.value,
  });
  protected readonly family = computed(() => KIND_FAMILY[this.kind()]);

  constructor() {
    if (this.account?.has_activity) {
      // La nature d'un compte qui a des transactions ne change plus (R-CPT-1).
      this.form.controls.kind.disable();
    }
    if (this.account) {
      this.form.controls.currency.disable();
    }
    this.form.controls.kind.valueChanges.subscribe((kind) => this.applyDefaults(kind));
    this.form.controls.institutionId.valueChanges.subscribe((value) =>
      this.creatingInstitution.set(value === NEW_INSTITUTION),
    );
    this.applyAllowed(this.form.controls.kind.value);
  }

  /** Options préremplies selon la nature, tant que l'utilisateur ne les a pas touchées (R-CPT-11, R-CPT-13). */
  private applyDefaults(kind: AccountKind): void {
    const defaults = defaultFlags(kind);
    if (!this.form.controls.emergency.dirty) {
      this.form.controls.emergency.setValue(defaults.emergency);
    }
    if (!this.form.controls.budget.dirty) {
      this.form.controls.budget.setValue(defaults.budget);
    }
    this.applyAllowed(kind);
  }

  /** L'épargne variable ne compte jamais dans les budgets (R-BUD-1). */
  private applyAllowed(kind: AccountKind): void {
    const family = KIND_FAMILY[kind];
    const toggle = (control: typeof this.form.controls.emergency, allowed: boolean) => {
      if (allowed) {
        control.enable({ emitEvent: false });
      } else {
        control.setValue(false, { emitEvent: false });
        control.disable({ emitEvent: false });
      }
    };
    toggle(this.form.controls.budget, family === 'CASH' || family === 'FIXED_SAVINGS');
  }

  private async institutionId(): Promise<string | null> {
    const value = this.form.controls.institutionId.value;
    if (value !== NEW_INSTITUTION) {
      return value || null;
    }
    const name = this.form.controls.newInstitutionName.value.trim();
    if (!name) {
      return null;
    }
    const created = await firstValueFrom(
      this.institutionsApi.createInstitution({ body: { name, archived: false } }),
    );
    return created.id;
  }

  protected async save(): Promise<void> {
    this.form.markAllAsTouched();
    if (this.form.invalid) {
      return;
    }
    const value = this.form.getRawValue();
    const balance = parseMoney(value.openingBalance, value.currency.toUpperCase()) ?? 0;
    this.busy.set(true);
    this.error.set(null);
    this.ibanError.set(null);
    try {
      const body = {
        name: value.name,
        kind: value.kind,
        institution_id: await this.institutionId(),
        currency: value.currency.toUpperCase(),
        opening_date: toIsoDate(value.openingDate),
        // Prêt : capital restant dû saisi en positif, enregistré en négatif (R-CPT-12).
        opening_balance_minor: balance,
        // L'IBAN n'est renvoyé que masqué : vide, il est conservé tel quel à la modification.
        iban: value.iban.trim() || (this.account ? undefined : null),
        account_number: value.accountNumber || null,
        include_in_net_worth: value.netWorth,
        include_in_emergency_fund: value.emergency,
        include_in_budget: value.budget,
        notes: value.notes || null,
      };
      if (this.account) {
        const iban = body.iban === undefined ? await this.currentIban() : body.iban;
        await firstValueFrom(
          this.api.updateAccount({
            id: this.account.id,
            body: {
              ...body,
              iban,
              display_order: this.account.display_order,
              color: this.account.color,
            },
          }),
        );
        this.version.bump();
        this.ref.close();
      } else {
        const created = await firstValueFrom(
          this.api.createAccount({ body: { ...body, iban: body.iban ?? null } }),
        );
        this.version.bump();
        this.ref.close();
        await this.router.navigate(['/comptes', created.id]);
      }
    } catch (error) {
      const problem = problemOf(error);
      if (problem?.code === 'INVALID_IBAN') {
        this.ibanError.set('IBAN invalide : vérifier la saisie.');
      } else {
        this.error.set(errorMessage(error));
      }
    } finally {
      this.busy.set(false);
    }
  }

  /** IBAN actuel en clair, pour le renvoyer inchangé à la modification. */
  private async currentIban(): Promise<string | null> {
    if (!this.account?.iban_masked) {
      return null;
    }
    const view = await firstValueFrom(this.api.revealIban({ id: this.account.id }));
    return view.iban;
  }
}
