import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  OnInit,
  computed,
  effect,
  inject,
  input,
  signal,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatStepperModule } from '@angular/material/stepper';
import { Router } from '@angular/router';
import { rxResource } from '@angular/core/rxjs-interop';
import { firstValueFrom } from 'rxjs';

import { ColumnRef } from '../../api/models/column-ref';
import { ImportPreview } from '../../api/models/import-preview';
import { ProfileSettings } from '../../api/models/profile-settings';
import { ImportsService } from '../../api/services/imports.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { FrDatePipe } from '../../shared/date.pipe';
import { MoneyPipe } from '../../shared/money.pipe';
import { AccountsStore } from '../../shared/stores';

const NEW_PROFILE = '__new__';

/** Réglages de départ d'un nouveau profil : un relevé français typique. */
function defaultSettings(): ProfileSettings {
  return {
    header_row: 1,
    skip_rows_after_header: 0,
    skip_footer_rows: 0,
    date: { column: 1, format: 'dd/MM/yyyy' },
    label: { columns: [2], separator: ' ' },
    amount: {
      mode: 'single',
      column: 3,
      debit_is_positive: true,
      invert_sign: false,
      decimal_separator: ',',
      thousands_separator: ' ',
    },
    row_filters: [],
  };
}

interface ColumnOption {
  value: ColumnRef;
  label: string;
}

/** E10 — Import : assistant (IMP-01, IMP-02). */
@Component({
  selector: 'app-import-wizard',
  imports: [
    FormsModule,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
    MatStepperModule,
    MoneyPipe,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './import-wizard.html',
  styleUrl: './import-wizard.scss',
})
export class ImportWizardPage implements OnInit {
  private readonly api = inject(ImportsService);
  private readonly router = inject(Router);
  private readonly version = inject(DataVersion);
  protected readonly accounts = inject(AccountsStore);

  /** Compte présélectionné (lien depuis la fiche d'un compte). */
  readonly compte = input<string>();
  /** Profil présélectionné (lien depuis les profils, E17). */
  readonly profil = input<string>();

  protected readonly newProfile = NEW_PROFILE;
  protected readonly encodings = ['UTF-8', 'UTF-8-BOM', 'windows-1252', 'ISO-8859-1'];
  protected readonly delimiters = [
    { value: ';', label: 'Point-virgule ;' },
    { value: ',', label: 'Virgule ,' },
    { value: '\\t', label: 'Tabulation' },
    { value: '|', label: 'Barre |' },
  ];
  protected readonly dateFormats = [
    'dd/MM/yyyy',
    'dd/MM/yy',
    'yyyy-MM-dd',
    'dd-MM-yyyy',
    'dd.MM.yyyy',
    'MM/dd/yyyy',
  ];
  protected readonly thousands = [
    { value: ' ', label: 'Espace' },
    { value: '.', label: 'Point' },
    { value: "'", label: 'Apostrophe' },
    { value: '', label: 'Aucun' },
  ];

  protected readonly file = signal<File | null>(null);
  protected readonly accountId = signal<string>('');
  protected readonly profileId = signal<string>(NEW_PROFILE);
  protected readonly settings = signal<ProfileSettings>(defaultSettings());
  protected readonly profileName = signal('');
  protected readonly preview = signal<ImportPreview | null>(null);
  protected readonly loading = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly busy = signal(false);
  /** Profil existant dont on modifie les réglages : il est mis à jour au lancement de la revue. */
  protected readonly editingProfile = signal<string | null>(null);
  private timer: ReturnType<typeof setTimeout> | null = null;

  protected readonly profiles = rxResource({ stream: () => this.api.listImportProfiles() });

  protected readonly format = computed(() => this.preview()?.format ?? null);
  /** L'OFX et le PDF LCL se lisent sans mapping ni profil (R-IMP-23). */
  protected readonly needsMapping = computed(
    () => this.profileId() === NEW_PROFILE && this.format() !== 'OFX' && this.format() !== 'PDF',
  );

  /** Colonnes proposées : en-têtes de la ligne choisie, sinon numéros. */
  protected readonly columns = computed<ColumnOption[]>(() => {
    const rows = this.preview()?.raw_rows ?? [];
    const header = this.settings().header_row;
    const width = Math.max(0, ...rows.map((r) => r.length));
    const headers = header ? (rows[header - 1] ?? []) : [];
    return Array.from({ length: width }, (_, i) => {
      const name = headers[i]?.trim();
      return name ? { value: name, label: name } : { value: i + 1, label: `Colonne ${i + 1}` };
    });
  });

  constructor() {
    inject(DestroyRef).onDestroy(() => this.timer && clearTimeout(this.timer));
    // Compte présélectionné dès que la liste arrive : celui demandé, sinon le premier compte courant.
    effect(() => {
      const open = this.accounts.open();
      if (!this.accountId() && open.length > 0) {
        const wanted =
          open.find((a) => a.id === this.compte()) ??
          open.find((a) => a.family === 'CASH') ??
          open[0];
        this.accountId.set(wanted.id);
      }
    });
  }

  ngOnInit(): void {
    if (this.profil()) {
      this.profileId.set(this.profil() as string);
    }
  }

  /** Rouvre l'association des colonnes avec les réglages du profil choisi. */
  protected editProfile(): void {
    const profile = this.profiles.value()?.find((p) => p.id === this.profileId());
    if (!profile?.settings) {
      return;
    }
    this.editingProfile.set(profile.id);
    this.profileName.set(profile.name);
    this.settings.set(structuredClone(profile.settings));
    this.profileId.set(NEW_PROFILE);
    this.refresh(0);
  }

  protected chooseFile(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0] ?? null;
    this.file.set(file);
    if (file && !this.profileName() && !this.editingProfile()) {
      this.profileName.set(file.name.replace(/\.[^.]+$/, ''));
    }
    // Un profil enregistré pour ce compte est proposé d'office.
    const account = this.accountId();
    const known = this.profiles.value()?.find((p) => p.default_account_id === account);
    if (known && this.profileId() === NEW_PROFILE) {
      this.profileId.set(known.id);
    }
    this.refresh(0);
  }

  protected chooseProfile(id: string): void {
    this.profileId.set(id);
    const profile = this.profiles.value()?.find((p) => p.id === id);
    if (profile?.settings) {
      this.settings.set(structuredClone(profile.settings));
    }
    this.refresh(0);
  }

  /** Modifie un réglage et relance l'aperçu (aperçu en direct, IMP-01). */
  protected patch(change: Partial<ProfileSettings>): void {
    this.settings.update((s) => ({ ...s, ...change }));
    this.refresh();
  }

  protected patchAmount(change: Partial<ProfileSettings['amount']>): void {
    this.patch({ amount: { ...this.settings().amount, ...change } });
  }

  protected addFilter(): void {
    const column = this.columns()[1]?.value ?? 1;
    this.patch({
      row_filters: [...(this.settings().row_filters ?? []), { column, pattern: '', regex: false }],
    });
  }

  protected updateFilter(
    index: number,
    change: Partial<{ column: ColumnRef; pattern: string; regex: boolean }>,
  ): void {
    const filters = (this.settings().row_filters ?? []).map((f, i) =>
      i === index ? { ...f, ...change } : f,
    );
    this.patch({ row_filters: filters });
  }

  protected removeFilter(index: number): void {
    this.patch({ row_filters: (this.settings().row_filters ?? []).filter((_, i) => i !== index) });
  }

  protected setBalance(which: 'opening' | 'closing', cell: string): void {
    const current = this.settings().statement_balances ?? {};
    const next = { ...current, [which]: cell.trim() ? { cell: cell.trim() } : null };
    this.patch({ statement_balances: next.opening || next.closing ? next : null });
  }

  protected balanceCell(which: 'opening' | 'closing'): string {
    return this.settings().statement_balances?.[which]?.cell ?? '';
  }

  private body(saveAs: string | null) {
    const file = this.file();
    if (!file) {
      return null;
    }
    const useProfile = this.profileId() !== NEW_PROFILE;
    return {
      file,
      account_id: this.accountId(),
      profile_id: useProfile ? this.profileId() : null,
      settings: useProfile ? null : JSON.stringify(this.settings()),
      save_profile_as: saveAs,
    };
  }

  protected refresh(delay = 400): void {
    if (this.timer) {
      clearTimeout(this.timer);
    }
    this.timer = setTimeout(() => void this.loadPreview(), delay);
  }

  private async loadPreview(): Promise<void> {
    const body = this.body(null);
    if (!body || !body.account_id) {
      return;
    }
    this.loading.set(true);
    this.error.set(null);
    try {
      const preview = await firstValueFrom(this.api.previewImport({ body }));
      this.preview.set(preview);
      // Encodage et séparateur détectés deviennent les réglages de départ.
      if (this.profileId() === NEW_PROFILE && !this.settings().encoding && preview.encoding) {
        this.settings.update((s) => ({
          ...s,
          encoding: preview.encoding,
          delimiter: preview.delimiter,
        }));
      }
    } catch (error) {
      this.preview.set(null);
      this.error.set(errorMessage(error));
    } finally {
      this.loading.set(false);
    }
  }

  protected async start(): Promise<void> {
    const editing = this.editingProfile();
    const saveAs =
      !editing && this.needsMapping() && this.profileName().trim()
        ? this.profileName().trim()
        : null;
    let body = this.body(saveAs);
    if (!body) {
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      if (editing && this.format()) {
        await firstValueFrom(
          this.api.updateImportProfile({
            id: editing,
            body: {
              name: this.profileName().trim() || 'Profil',
              format: this.format()!,
              settings: this.settings(),
              default_account_id: this.accountId(),
            },
          }),
        );
        body = { ...body, profile_id: editing, settings: null };
      }
      const created = await firstValueFrom(this.api.startImport({ body }));
      this.version.bump();
      await this.router.navigate(['/import', created.batch_id]);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
