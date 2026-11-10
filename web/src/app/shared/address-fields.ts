import {
  ChangeDetectionStrategy,
  Component,
  OnInit,
  computed,
  inject,
  input,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import {
  MatAutocompleteModule,
  MatAutocompleteSelectedEvent,
} from '@angular/material/autocomplete';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTooltipModule } from '@angular/material/tooltip';
import { of, switchMap, timer } from 'rxjs';

import { AddressInput } from '../api/models/address-input';
import { AddressSuggestion } from '../api/models/address-suggestion';
import { AddressView } from '../api/models/address-view';
import { PayeesService } from '../api/services/payees.service';
import { errorMessage } from '../core/http-errors';
import { countryList } from './address';
import { FeaturesStore } from './stores';

/**
 * Section « Adresse » d'un tiers (E31, E32) ou d'une transaction (E7, E14) : le pays, puis la recherche dans ce
 * pays si elle est activée (Base Adresse Nationale en France, Photon ailleurs), puis la saisie champ par champ
 * (R-TIE-23 à R-TIE-26, R-TRX-59, ADR-0030).
 */
@Component({
  selector: 'app-address-fields',
  imports: [
    FormsModule,
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
        <button mat-button type="button" (click)="clear()">Effacer l'adresse</button>
      }
    </div>
    <mat-form-field>
      <mat-label>Pays</mat-label>
      <mat-select [ngModel]="country()" (ngModelChange)="edit(country, $event)">
        @for (c of countries; track c.code) {
          <mat-option [value]="c.code">{{ c.name }}</mat-option>
        }
      </mat-select>
    </mat-form-field>
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
        <mat-hint>{{ searchHint() }}</mat-hint>
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
    @if (problem(); as problem) {
      <p class="warn">{{ problem }}</p>
    }
  `,
  styles: `
    :host {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
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
export class AddressFields implements OnInit {
  protected readonly features = inject(FeaturesStore);
  private readonly api = inject(PayeesService);
  protected readonly countries = countryList();

  /** Adresse enregistrée, lue à l'ouverture. */
  readonly initial = input<AddressView | null>(null);

  protected readonly line = signal('');
  protected readonly postalCode = signal('');
  protected readonly city = signal('');
  protected readonly country = signal('FR');
  /** Coordonnées de la proposition choisie (R-TIE-26). */
  protected readonly point = signal<{ latitude_e6: number; longitude_e6: number } | null>(null);
  protected readonly query = signal<string | AddressSuggestion>('');

  /** Propositions dans le pays choisi, demandées par le serveur après une courte pause de saisie. */
  protected readonly suggestions = rxResource({
    params: () => {
      const query = this.query();
      return typeof query === 'string' && query.trim().length >= 3
        ? { q: query.trim(), country: this.country() }
        : undefined;
    },
    stream: ({ params }) =>
      timer(300).pipe(switchMap(() => (params ? this.api.searchAddress(params) : of([])))),
  });

  /** Le service qui reçoit le texte saisi, selon le pays (ADR-0007, ADR-0030). */
  protected readonly searchHint = computed(() =>
    this.country() === 'FR'
      ? 'Envoyé à la Base Adresse Nationale (api-adresse.data.gouv.fr)'
      : 'Envoyé à Photon (photon.komoot.io), données OpenStreetMap',
  );

  protected readonly hasAddress = computed(
    () => !!(this.line().trim() || this.postalCode().trim() || this.city().trim()),
  );

  /** Ville requise dès qu'un champ est rempli ; code postal français à 5 chiffres (R-TIE-23). */
  readonly problem = computed(() => {
    if (!this.hasAddress()) return null;
    if (!this.city().trim()) return 'Indiquer la ville.';
    const postal = this.postalCode().trim();
    if (this.country() === 'FR' && postal && !/^\d{5}$/.test(postal)) {
      return 'Un code postal français a 5 chiffres.';
    }
    return null;
  });

  /** L'adresse saisie ; `null` : pas d'adresse. */
  readonly value = computed<AddressInput | null>(() =>
    this.hasAddress()
      ? {
          line: this.line().trim() || null,
          postal_code: this.postalCode().trim() || null,
          city: this.city().trim(),
          country_code: this.country(),
          latitude_e6: this.point()?.latitude_e6 ?? null,
          longitude_e6: this.point()?.longitude_e6 ?? null,
        }
      : null,
  );

  ngOnInit(): void {
    const initial = this.initial();
    if (!initial) return;
    this.line.set(initial.line ?? '');
    this.postalCode.set(initial.postal_code ?? '');
    this.city.set(initial.city);
    this.country.set(initial.country_code);
    if (initial.latitude_e6 != null && initial.longitude_e6 != null) {
      this.point.set({ latitude_e6: initial.latitude_e6, longitude_e6: initial.longitude_e6 });
    }
  }

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

  protected clear(): void {
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
