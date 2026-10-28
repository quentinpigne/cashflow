//! Adresse d'un tiers et lieu de ses transactions (R-TIE-23 à R-TIE-28, ADR-0025).

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::shared::DomainError;

/// Pays d'une adresse française.
pub const FRANCE: &str = "FR";

/// Latitude et longitude en millionièmes de degré : des entiers, jamais de flottant (R-TIE-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeoPoint {
    pub latitude_e6: i32,
    pub longitude_e6: i32,
}

impl GeoPoint {
    pub fn new(latitude_e6: i32, longitude_e6: i32) -> Result<Self, DomainError> {
        if (-90_000_000..=90_000_000).contains(&latitude_e6) && (-180_000_000..=180_000_000).contains(&longitude_e6) {
            Ok(Self { latitude_e6, longitude_e6 })
        } else {
            Err(DomainError::InvalidCoordinates)
        }
    }

    /// Coordonnées écrites en degrés décimaux (« 45.214930 »), comme les renvoie un géocodeur.
    pub fn from_degrees(latitude: &str, longitude: &str) -> Result<Self, DomainError> {
        let micro = |degrees: &str| {
            let value: Decimal = degrees.trim().parse().map_err(|_| DomainError::InvalidCoordinates)?;
            (value * Decimal::from(1_000_000))
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_i32()
                .ok_or(DomainError::InvalidCoordinates)
        };
        Self::new(micro(latitude)?, micro(longitude)?)
    }
}

/// Adresse d'un tiers, contrôlée (R-TIE-23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayeeAddress {
    pub line: Option<String>,
    pub postal_code: Option<String>,
    pub city: String,
    /// ISO 3166-1 alpha-2, en majuscules.
    pub country_code: String,
    pub point: Option<GeoPoint>,
}

impl PayeeAddress {
    /// Assemble une adresse à partir de champs déjà nettoyés (vides absents). Aucun champ : pas d'adresse.
    ///
    /// Dès qu'un champ est renseigné, la ville et le pays sont requis (R-TIE-23) ; des coordonnées n'existent
    /// qu'avec une adresse (R-TIE-25).
    pub fn new(
        line: Option<String>,
        postal_code: Option<String>,
        city: Option<String>,
        country_code: Option<String>,
        point: Option<GeoPoint>,
    ) -> Result<Option<Self>, DomainError> {
        if line.is_none() && postal_code.is_none() && city.is_none() && country_code.is_none() {
            return if point.is_some() { Err(DomainError::InvalidCoordinates) } else { Ok(None) };
        }
        let (Some(city), Some(country)) = (city, country_code) else {
            return Err(DomainError::AddressIncomplete);
        };
        let country_code = country.to_ascii_uppercase();
        if country_code.len() != 2 || !country_code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(DomainError::InvalidCountryCode);
        }
        if country_code == FRANCE
            && postal_code.as_deref().is_some_and(|p| p.len() != 5 || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(DomainError::InvalidPostalCode);
        }
        Ok(Some(Self { line, postal_code, city, country_code, point }))
    }

    pub fn department(&self) -> Option<&'static Department> {
        department(&self.country_code, self.postal_code.as_deref()?)
    }

    /// Les champs de l'adresse, hors coordonnées : les modifier à la main efface les coordonnées (R-TIE-25).
    pub fn same_place(&self, other: &Self) -> bool {
        self.line == other.line
            && self.postal_code == other.postal_code
            && self.city == other.city
            && self.country_code == other.country_code
    }

    /// Retire des coordonnées reprises telles quelles alors que l'adresse a changé (R-TIE-25) : elles venaient
    /// de la recherche de l'ancienne adresse. Des coordonnées nouvelles viennent d'une nouvelle recherche.
    pub fn without_stale_point(mut self, previous: Option<&Self>) -> Self {
        if let Some(previous) = previous
            && !self.same_place(previous)
            && self.point.is_some()
            && self.point == previous.point
        {
            self.point = None;
        }
        self
    }
}

/// Département français.
#[derive(Debug, PartialEq, Eq)]
pub struct Department {
    /// `38`, `2A`, `974`.
    pub code: &'static str,
    pub name: &'static str,
}

/// Département d'une adresse française, déduit du code postal (R-TIE-24).
pub fn department(country_code: &str, postal_code: &str) -> Option<&'static Department> {
    if country_code != FRANCE || postal_code.len() != 5 || !postal_code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let code = match &postal_code[..2] {
        "20" => match &postal_code[2..3] {
            "0" | "1" => "2A",
            _ => "2B",
        },
        "97" => &postal_code[..3],
        prefix => prefix,
    };
    DEPARTMENTS.iter().find(|d| d.code == code)
}

/// Niveau de regroupement de l'analyse par lieu (R-TIE-28).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationLevel {
    Country,
    Department,
    City,
}

/// Lieu d'un tiers selon le niveau analysé (R-TIE-28).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Location {
    Country(String),
    Department(&'static str),
    /// Ville : pays, département (en France) et nom sans distinction de casse.
    City {
        country_code: String,
        department: Option<&'static str>,
        key: String,
    },
    /// Par département : adresse hors de France.
    Abroad,
    /// Par département : adresse française dont le code postal ne donne pas de département.
    UnknownDepartment,
    NoAddress,
}

pub fn location(level: LocationLevel, address: Option<&PayeeAddress>) -> Location {
    let Some(address) = address else { return Location::NoAddress };
    match level {
        LocationLevel::Country => Location::Country(address.country_code.clone()),
        LocationLevel::Department if address.country_code != FRANCE => Location::Abroad,
        LocationLevel::Department => {
            address.department().map_or(Location::UnknownDepartment, |d| Location::Department(d.code))
        }
        LocationLevel::City => Location::City {
            country_code: address.country_code.clone(),
            department: address.department().map(|d| d.code),
            key: address.city.to_lowercase(),
        },
    }
}

/// Départements français, métropole et outre-mer.
pub const DEPARTMENTS: &[Department] = &[
    Department { code: "01", name: "Ain" },
    Department { code: "02", name: "Aisne" },
    Department { code: "03", name: "Allier" },
    Department { code: "04", name: "Alpes-de-Haute-Provence" },
    Department { code: "05", name: "Hautes-Alpes" },
    Department { code: "06", name: "Alpes-Maritimes" },
    Department { code: "07", name: "Ardèche" },
    Department { code: "08", name: "Ardennes" },
    Department { code: "09", name: "Ariège" },
    Department { code: "10", name: "Aube" },
    Department { code: "11", name: "Aude" },
    Department { code: "12", name: "Aveyron" },
    Department { code: "13", name: "Bouches-du-Rhône" },
    Department { code: "14", name: "Calvados" },
    Department { code: "15", name: "Cantal" },
    Department { code: "16", name: "Charente" },
    Department { code: "17", name: "Charente-Maritime" },
    Department { code: "18", name: "Cher" },
    Department { code: "19", name: "Corrèze" },
    Department { code: "2A", name: "Corse-du-Sud" },
    Department { code: "2B", name: "Haute-Corse" },
    Department { code: "21", name: "Côte-d'Or" },
    Department { code: "22", name: "Côtes-d'Armor" },
    Department { code: "23", name: "Creuse" },
    Department { code: "24", name: "Dordogne" },
    Department { code: "25", name: "Doubs" },
    Department { code: "26", name: "Drôme" },
    Department { code: "27", name: "Eure" },
    Department { code: "28", name: "Eure-et-Loir" },
    Department { code: "29", name: "Finistère" },
    Department { code: "30", name: "Gard" },
    Department { code: "31", name: "Haute-Garonne" },
    Department { code: "32", name: "Gers" },
    Department { code: "33", name: "Gironde" },
    Department { code: "34", name: "Hérault" },
    Department { code: "35", name: "Ille-et-Vilaine" },
    Department { code: "36", name: "Indre" },
    Department { code: "37", name: "Indre-et-Loire" },
    Department { code: "38", name: "Isère" },
    Department { code: "39", name: "Jura" },
    Department { code: "40", name: "Landes" },
    Department { code: "41", name: "Loir-et-Cher" },
    Department { code: "42", name: "Loire" },
    Department { code: "43", name: "Haute-Loire" },
    Department { code: "44", name: "Loire-Atlantique" },
    Department { code: "45", name: "Loiret" },
    Department { code: "46", name: "Lot" },
    Department { code: "47", name: "Lot-et-Garonne" },
    Department { code: "48", name: "Lozère" },
    Department { code: "49", name: "Maine-et-Loire" },
    Department { code: "50", name: "Manche" },
    Department { code: "51", name: "Marne" },
    Department { code: "52", name: "Haute-Marne" },
    Department { code: "53", name: "Mayenne" },
    Department { code: "54", name: "Meurthe-et-Moselle" },
    Department { code: "55", name: "Meuse" },
    Department { code: "56", name: "Morbihan" },
    Department { code: "57", name: "Moselle" },
    Department { code: "58", name: "Nièvre" },
    Department { code: "59", name: "Nord" },
    Department { code: "60", name: "Oise" },
    Department { code: "61", name: "Orne" },
    Department { code: "62", name: "Pas-de-Calais" },
    Department { code: "63", name: "Puy-de-Dôme" },
    Department { code: "64", name: "Pyrénées-Atlantiques" },
    Department { code: "65", name: "Hautes-Pyrénées" },
    Department { code: "66", name: "Pyrénées-Orientales" },
    Department { code: "67", name: "Bas-Rhin" },
    Department { code: "68", name: "Haut-Rhin" },
    Department { code: "69", name: "Rhône" },
    Department { code: "70", name: "Haute-Saône" },
    Department { code: "71", name: "Saône-et-Loire" },
    Department { code: "72", name: "Sarthe" },
    Department { code: "73", name: "Savoie" },
    Department { code: "74", name: "Haute-Savoie" },
    Department { code: "75", name: "Paris" },
    Department { code: "76", name: "Seine-Maritime" },
    Department { code: "77", name: "Seine-et-Marne" },
    Department { code: "78", name: "Yvelines" },
    Department { code: "79", name: "Deux-Sèvres" },
    Department { code: "80", name: "Somme" },
    Department { code: "81", name: "Tarn" },
    Department { code: "82", name: "Tarn-et-Garonne" },
    Department { code: "83", name: "Var" },
    Department { code: "84", name: "Vaucluse" },
    Department { code: "85", name: "Vendée" },
    Department { code: "86", name: "Vienne" },
    Department { code: "87", name: "Haute-Vienne" },
    Department { code: "88", name: "Vosges" },
    Department { code: "89", name: "Yonne" },
    Department { code: "90", name: "Territoire de Belfort" },
    Department { code: "91", name: "Essonne" },
    Department { code: "92", name: "Hauts-de-Seine" },
    Department { code: "93", name: "Seine-Saint-Denis" },
    Department { code: "94", name: "Val-de-Marne" },
    Department { code: "95", name: "Val-d'Oise" },
    Department { code: "971", name: "Guadeloupe" },
    Department { code: "972", name: "Martinique" },
    Department { code: "973", name: "Guyane" },
    Department { code: "974", name: "La Réunion" },
    Department { code: "976", name: "Mayotte" },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_owned())
    }

    fn address(postal_code: &str, city: &str, country: &str) -> PayeeAddress {
        PayeeAddress::new(None, s(postal_code), s(city), s(country), None).unwrap().unwrap()
    }

    #[test]
    fn r_tie_23_an_address_needs_a_city_and_a_country() {
        assert_eq!(PayeeAddress::new(None, None, None, None, None), Ok(None));
        assert_eq!(PayeeAddress::new(None, s("38240"), None, s("FR"), None), Err(DomainError::AddressIncomplete));
        assert_eq!(PayeeAddress::new(None, None, s("Meylan"), None, None), Err(DomainError::AddressIncomplete));
        let a = PayeeAddress::new(s("1 rue X"), s("38240"), s("Meylan"), s("fr"), None).unwrap().unwrap();
        assert_eq!(a.country_code, "FR");
    }

    #[test]
    fn r_tie_23_country_code_and_french_postal_code_are_checked() {
        assert_eq!(PayeeAddress::new(None, None, s("Meylan"), s("FRA"), None), Err(DomainError::InvalidCountryCode));
        assert_eq!(PayeeAddress::new(None, None, s("Meylan"), s("F1"), None), Err(DomainError::InvalidCountryCode));
        assert_eq!(PayeeAddress::new(None, s("3824"), s("Meylan"), s("FR"), None), Err(DomainError::InvalidPostalCode));
        // Hors de France, un code postal n'a pas de format imposé.
        assert!(PayeeAddress::new(None, s("SW1A 1AA"), s("London"), s("GB"), None).is_ok());
    }

    #[test]
    fn r_tie_24_department_from_the_postal_code() {
        let code = |postal: &str| department(FRANCE, postal).map(|d| d.code);
        assert_eq!(code("38240"), Some("38"));
        assert_eq!(code("01000"), Some("01"));
        assert_eq!(code("20090"), Some("2A"));
        assert_eq!(code("20167"), Some("2A"));
        assert_eq!(code("20200"), Some("2B"));
        assert_eq!(code("20600"), Some("2B"));
        assert_eq!(code("97400"), Some("974"));
        assert_eq!(code("97600"), Some("976"));
        assert_eq!(code("98000"), None);
        assert_eq!(department("BE", "38240"), None);
        assert_eq!(department(FRANCE, "38240").map(|d| d.name), Some("Isère"));
    }

    #[test]
    fn r_tie_25_coordinates_are_integers_within_bounds_and_need_an_address() {
        let p = GeoPoint::from_degrees("45.2149305", "-5.71").unwrap();
        assert_eq!(p, GeoPoint { latitude_e6: 45_214_931, longitude_e6: -5_710_000 });
        assert_eq!(GeoPoint::from_degrees("90.5", "0"), Err(DomainError::InvalidCoordinates));
        assert_eq!(GeoPoint::from_degrees("abc", "0"), Err(DomainError::InvalidCoordinates));
        assert_eq!(GeoPoint::new(0, 180_000_001), Err(DomainError::InvalidCoordinates));
        assert_eq!(PayeeAddress::new(None, None, None, None, Some(p)), Err(DomainError::InvalidCoordinates));
    }

    #[test]
    fn r_tie_25_editing_the_address_by_hand_drops_the_coordinates() {
        let point = GeoPoint::new(45_000_000, 5_000_000).unwrap();
        let geocoded = PayeeAddress { point: Some(point), ..address("38240", "Meylan", "FR") };
        let corrected = PayeeAddress { postal_code: s("38000"), ..geocoded.clone() };
        assert_eq!(corrected.clone().without_stale_point(Some(&geocoded)).point, None);
        // Nouvelle recherche : nouvelles coordonnées, gardées.
        let searched = PayeeAddress { point: Some(GeoPoint::new(45_100_000, 5_000_000).unwrap()), ..corrected };
        assert!(searched.clone().without_stale_point(Some(&geocoded)).point.is_some());
        // Adresse inchangée : coordonnées gardées.
        assert!(geocoded.clone().without_stale_point(Some(&geocoded)).point.is_some());
    }

    #[test]
    fn r_tie_28_locations_by_level() {
        let meylan = address("38240", "Meylan", "FR");
        let shouting = address("38240", "MEYLAN", "FR");
        let london = address("SW1A 1AA", "London", "GB");
        let unknown = address("98000", "Monaco", "FR");
        use LocationLevel::*;
        assert_eq!(location(City, Some(&meylan)), location(City, Some(&shouting)));
        // Deux villes homonymes dans deux départements.
        let valence = |postal| location(City, Some(&address(postal, "Valence", "FR")));
        assert_ne!(valence("26000"), valence("82400"));
        assert_eq!(location(Department, Some(&meylan)), Location::Department("38"));
        assert_eq!(location(Department, Some(&london)), Location::Abroad);
        assert_eq!(location(Department, Some(&unknown)), Location::UnknownDepartment);
        assert_eq!(location(Country, Some(&london)), Location::Country("GB".into()));
        assert_eq!(location(Country, None), Location::NoAddress);
    }
}
