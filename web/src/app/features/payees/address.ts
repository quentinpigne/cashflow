import { AddressView } from '../../api/models/address-view';

const regionNames = new Intl.DisplayNames(['fr'], { type: 'region', fallback: 'none' });

/** Codes de région connus d'`Intl` qui ne sont pas des pays ISO 3166-1 (ou en doublonnent un). */
const NOT_COUNTRIES = new Set([
  'AC',
  'CP',
  'CQ',
  'DG',
  'EA',
  'EU',
  'EZ',
  'IC',
  'QO',
  'TA',
  'UK',
  'UN',
  'XA',
  'XB',
  'ZZ',
]);

/** Nom français d'un pays à partir de son code ISO 3166-1 alpha-2. */
export function countryName(code: string): string {
  return regionNames.of(code) ?? code;
}

export interface Country {
  code: string;
  name: string;
}

let countries: Country[] | null = null;

/** Pays proposés à la saisie d'une adresse, la France d'abord puis par ordre alphabétique (R-TIE-23). */
export function countryList(): Country[] {
  if (!countries) {
    const all: Country[] = [];
    for (let a = 65; a <= 90; a++) {
      for (let b = 65; b <= 90; b++) {
        const code = String.fromCharCode(a, b);
        const name = regionNames.of(code);
        if (name && !NOT_COUNTRIES.has(code)) all.push({ code, name });
      }
    }
    all.sort((x, y) => x.name.localeCompare(y.name, 'fr'));
    countries = [...all.filter((c) => c.code === 'FR'), ...all.filter((c) => c.code !== 'FR')];
  }
  return countries;
}

/** Ville, précédée du code postal, suivie du pays hors de France : « 38240 Meylan ». */
export function formatCity(address: AddressView): string {
  const city = [address.postal_code, address.city].filter(Boolean).join(' ');
  return address.country_code === 'FR' ? city : `${city}, ${countryName(address.country_code)}`;
}

/** Adresse complète sur une ligne. */
export function formatAddress(address: AddressView): string {
  return [address.line, formatCity(address)].filter(Boolean).join(', ');
}

/** Lien OpenStreetMap vers des coordonnées, ouvert seulement sur action explicite (ecrans.md, ADR-0025). */
export function mapLink(address: AddressView): string | null {
  if (address.latitude_e6 == null || address.longitude_e6 == null) return null;
  const lat = (address.latitude_e6 / 1_000_000).toFixed(6);
  const lon = (address.longitude_e6 / 1_000_000).toFixed(6);
  return `https://www.openstreetmap.org/?mlat=${lat}&mlon=${lon}#map=18/${lat}/${lon}`;
}
