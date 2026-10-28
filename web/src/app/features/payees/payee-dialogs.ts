import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import {
  MatAutocompleteModule,
  MatAutocompleteSelectedEvent,
} from '@angular/material/autocomplete';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTooltipModule } from '@angular/material/tooltip';
import { of, switchMap, timer } from 'rxjs';

import { AddressInput } from '../../api/models/address-input';
import { AddressSuggestion } from '../../api/models/address-suggestion';
import { AddressView } from '../../api/models/address-view';
import { PayeeView } from '../../api/models/payee-view';
import { PayeesService } from '../../api/services/payees.service';
import { errorMessage } from '../../core/http-errors';
import { FeaturesStore, PayeesStore } from '../../shared/stores';
import { countryList } from './address';

/** Champs d'un tiers ou d'un groupe saisis dans un dialogue. */
export interface PayeeFormValue {
  name: string;
  /** Tiers seulement : groupe choisi, ou nom d'un nouveau groupe à créer. */
  groupId: string | null;
  newGroup: string | null;
  note: string | null;
  /** Tiers seulement ; `null` : pas d'adresse. */
  address: AddressInput | null;
}

export interface PayeeFormData {
  title: string;
  action: string;
  /** Tiers (avec un choix de groupe) ou groupe. */
  kind: 'payee' | 'group';
  value: Partial<Omit<PayeeFormValue, 'address'>> & { address?: AddressView | null };
}

/** L'adresse d'un tiers, saisie ou proposée par la recherche, renvoyée à l'API telle qu'elle (R-TIE-23). */
export function addressInput(address: AddressView | null | undefined): AddressInput | null {
  if (!address) return null;
  return {
    line: address.line ?? null,
    postal_code: address.postal_code ?? null,
    city: address.city,
    country_code: address.country_code,
    latitude_e6: address.latitude_e6 ?? null,
    longitude_e6: address.longitude_e6 ?? null,
  };
}

/** Valeur de l'option « Nouveau groupe… ». */
const NEW_GROUP = '__new__';

/** Création ou modification d'un tiers ou d'un groupe de tiers (E31). */
@Component({
  selector: 'app-payee-form',
  imports: [
    FormsModule,
    MatDialogModule,
    MatAutocompleteModule,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
    MatTooltipModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ data.title }}</h2>
    <mat-dialog-content class="form">
      <mat-form-field>
        <mat-label>Nom</mat-label>
        <input
          matInput
          [ngModel]="name()"
          (ngModelChange)="name.set($event)"
          maxlength="80"
          cdkFocusInitial
        />
      </mat-form-field>
      @if (data.kind === 'payee') {
        <mat-form-field>
          <mat-label>Groupe</mat-label>
          <mat-select [ngModel]="group()" (ngModelChange)="group.set($event)">
            <mat-option [value]="null">Sans groupe</mat-option>
            @for (g of store.groups(); track g.id) {
              <mat-option [value]="g.id">{{ g.name }}</mat-option>
            }
            <mat-option [value]="newGroup">Nouveau groupe…</mat-option>
          </mat-select>
        </mat-form-field>
        @if (group() === newGroup) {
          <mat-form-field>
            <mat-label>Nom du nouveau groupe</mat-label>
            <input
              matInput
              [ngModel]="groupName()"
              (ngModelChange)="groupName.set($event)"
              maxlength="80"
            />
          </mat-form-field>
        }

        <div class="address-head">
          <h3>Adresse</h3>
          @if (hasAddress()) {
            <mat-icon
              class="located"
              [class.off]="!point()"
              [matTooltip]="
                point()
                  ? 'Localisée : figure sur la carte des analyses'
                  : 'Non localisée : choisir une proposition de la recherche pour la placer sur la carte'
              "
              >{{ point() ? 'location_on' : 'location_off' }}</mat-icon
            >
            <span class="spacer"></span>
            <button mat-button type="button" (click)="clearAddress()">Effacer l'adresse</button>
          }
        </div>
        @if (features.geocoding()) {
          <mat-form-field subscriptSizing="dynamic">
            <mat-label>Rechercher une adresse</mat-label>
            <mat-icon matPrefix>search</mat-icon>
            <input
              matInput
              [ngModel]="query()"
              (ngModelChange)="query.set($event)"
              [matAutocomplete]="search"
            />
            <mat-autocomplete #search (optionSelected)="choose($event)">
              @for (s of suggestions.value() ?? []; track s.label) {
                <mat-option [value]="s">{{ s.label }}</mat-option>
              }
            </mat-autocomplete>
            <mat-hint>Envoyé à la Base Adresse Nationale (api-adresse.data.gouv.fr)</mat-hint>
          </mat-form-field>
          @if (suggestions.error(); as error) {
            <p class="warn">{{ message(error) }} Saisir l'adresse à la main.</p>
          }
        }
        <mat-form-field>
          <mat-label>Adresse</mat-label>
          <input
            matInput
            [ngModel]="line()"
            (ngModelChange)="edit(line, $event)"
            maxlength="200"
            placeholder="Numéro et voie"
          />
        </mat-form-field>
        <div class="row">
          <mat-form-field class="postal">
            <mat-label>Code postal</mat-label>
            <input
              matInput
              [ngModel]="postalCode()"
              (ngModelChange)="edit(postalCode, $event)"
              maxlength="10"
            />
          </mat-form-field>
          <mat-form-field class="city">
            <mat-label>Ville</mat-label>
            <input
              matInput
              [ngModel]="city()"
              (ngModelChange)="edit(city, $event)"
              maxlength="80"
              [required]="hasAddress()"
            />
          </mat-form-field>
        </div>
        <mat-form-field>
          <mat-label>Pays</mat-label>
          <mat-select [ngModel]="country()" (ngModelChange)="edit(country, $event)">
            @for (c of countries; track c.code) {
              <mat-option [value]="c.code">{{ c.name }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        @if (addressProblem(); as problem) {
          <p class="warn">{{ problem }}</p>
        }
      }
      <mat-form-field>
        <mat-label>Note</mat-label>
        <textarea
          matInput
          rows="2"
          [ngModel]="note()"
          (ngModelChange)="note.set($event)"
        ></textarea>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="result()" [disabled]="!valid()">
        {{ data.action }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(22rem, 80vw);
    }

    .address-head {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      margin-top: 0.5rem;

      h3 {
        margin: 0;
        font: var(--mat-sys-title-small);
      }

      .located {
        color: var(--mat-sys-primary);
      }

      .located.off {
        color: var(--mat-sys-outline);
      }

      .spacer {
        flex: 1;
      }
    }

    .row {
      display: flex;
      gap: var(--field-gap);

      .postal {
        flex: 0 0 8rem;
      }

      .city {
        flex: 1;
        min-width: 0;
      }
    }
  `,
})
export class PayeeFormDialog {
  protected readonly data = inject<PayeeFormData>(MAT_DIALOG_DATA);
  protected readonly store = inject(PayeesStore);
  protected readonly features = inject(FeaturesStore);
  private readonly api = inject(PayeesService);
  protected readonly newGroup = NEW_GROUP;
  protected readonly countries = countryList();
  protected readonly name = signal(this.data.value.name ?? '');
  protected readonly group = signal<string | null>(this.data.value.groupId ?? null);
  protected readonly groupName = signal('');
  protected readonly note = signal(this.data.value.note ?? '');

  private readonly initial = this.data.value.address ?? null;
  protected readonly line = signal(this.initial?.line ?? '');
  protected readonly postalCode = signal(this.initial?.postal_code ?? '');
  protected readonly city = signal(this.initial?.city ?? '');
  protected readonly country = signal(this.initial?.country_code ?? 'FR');
  /** Coordonnées de la proposition choisie (R-TIE-26). */
  protected readonly point = signal<{ latitude_e6: number; longitude_e6: number } | null>(
    this.initial?.latitude_e6 != null && this.initial.longitude_e6 != null
      ? { latitude_e6: this.initial.latitude_e6, longitude_e6: this.initial.longitude_e6 }
      : null,
  );
  protected readonly query = signal<string | AddressSuggestion>('');

  /** Propositions de la BAN, demandées par le serveur après une courte pause de saisie. */
  protected readonly suggestions = rxResource({
    params: () => {
      const query = this.query();
      return typeof query === 'string' && query.trim().length >= 3 ? query.trim() : undefined;
    },
    stream: ({ params }) =>
      timer(300).pipe(switchMap(() => (params ? this.api.searchAddress({ q: params }) : of([])))),
  });

  protected readonly hasAddress = computed(
    () => !!(this.line().trim() || this.postalCode().trim() || this.city().trim()),
  );

  /** Ville requise dès qu'un champ est rempli ; code postal français à 5 chiffres (R-TIE-23). */
  protected readonly addressProblem = computed(() => {
    if (!this.hasAddress()) return null;
    if (!this.city().trim()) return 'Indiquer la ville.';
    const postal = this.postalCode().trim();
    if (this.country() === 'FR' && postal && !/^\d{5}$/.test(postal)) {
      return 'Un code postal français a 5 chiffres.';
    }
    return null;
  });

  protected readonly valid = computed(
    () =>
      !!this.name().trim() &&
      (this.group() !== NEW_GROUP || !!this.groupName().trim()) &&
      !this.addressProblem(),
  );

  protected readonly result = computed<PayeeFormValue>(() => {
    const creating = this.group() === NEW_GROUP;
    return {
      name: this.name().trim(),
      groupId: creating ? null : this.group(),
      newGroup: creating ? this.groupName().trim() : null,
      note: this.note().trim() || null,
      address: this.hasAddress()
        ? {
            line: this.line().trim() || null,
            postal_code: this.postalCode().trim() || null,
            city: this.city().trim(),
            country_code: this.country(),
            latitude_e6: this.point()?.latitude_e6 ?? null,
            longitude_e6: this.point()?.longitude_e6 ?? null,
          }
        : null,
    };
  });

  /** Corriger un champ à la main retire la localisation : elle décrivait l'adresse d'avant (R-TIE-25). */
  protected edit(field: { set(value: string): void }, value: string): void {
    field.set(value);
    this.point.set(null);
  }

  /** Une proposition remplit l'adresse et ses coordonnées (R-TIE-26). */
  protected choose(event: MatAutocompleteSelectedEvent): void {
    const s = event.option.value as AddressSuggestion;
    this.line.set(s.line ?? '');
    this.postalCode.set(s.postal_code ?? '');
    this.city.set(s.city);
    this.country.set(s.country_code);
    this.point.set({ latitude_e6: s.latitude_e6, longitude_e6: s.longitude_e6 });
    this.query.set('');
  }

  protected clearAddress(): void {
    this.line.set('');
    this.postalCode.set('');
    this.city.set('');
    this.country.set('FR');
    this.point.set(null);
  }

  protected message(error: unknown): string {
    return errorMessage(error);
  }
}

/** « Fusionner dans… » (R-TIE-7) : un autre tiers. */
@Component({
  selector: 'app-payee-merge',
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatSelectModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Fusionner « {{ source.name }} »</h2>
    <mat-dialog-content>
      <p>
        Ses {{ source.transaction_count }} transaction(s), ses récurrences et les règles qui
        l'affectent passeront au tiers choisi, puis « {{ source.name }} » sera supprimé.
      </p>
      <mat-form-field class="full-width">
        <mat-label>Fusionner dans</mat-label>
        <mat-select [ngModel]="target()" (ngModelChange)="target.set($event)">
          @for (p of options(); track p.id) {
            <mat-option [value]="p.id">{{ p.name }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="target()" [disabled]="!target()">
        Fusionner
      </button>
    </mat-dialog-actions>
  `,
})
export class PayeeMergeDialog {
  protected readonly source = inject<PayeeView>(MAT_DIALOG_DATA);
  private readonly store = inject(PayeesStore);
  protected readonly target = signal<string | null>(null);
  protected readonly options = computed(() =>
    this.store.list().filter((p) => p.id !== this.source.id && !p.archived),
  );
}

/** Choix d'un groupe pour plusieurs tiers (E31). `null` : sans groupe. */
@Component({
  selector: 'app-payee-regroup',
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatSelectModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Changer de groupe ({{ count }} tiers)</h2>
    <mat-dialog-content>
      <mat-form-field class="full-width">
        <mat-label>Groupe</mat-label>
        <mat-select [ngModel]="group()" (ngModelChange)="group.set($event)">
          <mat-option [value]="none">Sans groupe</mat-option>
          @for (g of store.groups(); track g.id) {
            <mat-option [value]="g.id">{{ g.name }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [mat-dialog-close]="group()" [disabled]="!group()">Déplacer</button>
    </mat-dialog-actions>
  `,
})
export class PayeeRegroupDialog {
  protected readonly count = inject<number>(MAT_DIALOG_DATA);
  protected readonly store = inject(PayeesStore);
  /** Valeur de « Sans groupe », distincte de l'absence de choix. */
  protected readonly none = 'none';
  protected readonly group = signal<string | null>(null);
}
