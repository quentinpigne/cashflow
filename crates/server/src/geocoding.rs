//! Recherche d'adresse dans le pays choisi : la Base Adresse Nationale pour la France, Photon (données
//! OpenStreetMap) pour les autres pays (R-TIE-26, ADR-0025, ADR-0030).
//!
//! Seul le serveur appelle ces services : le navigateur ne contacte jamais un service tiers. L'appel n'a lieu
//! que si la recherche d'adresse est activée dans les réglages (contrôlé par le service).

use std::collections::HashSet;
use std::time::Duration;

use anyhow::Context;
use cashflow_domain::payees::location::{FRANCE, GeoPoint};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

/// Adresse de la BAN par défaut ; configurable si le service déménage (Géoplateforme de l'IGN).
pub const DEFAULT_URL: &str = "https://api-adresse.data.gouv.fr";

/// Adresse de Photon par défaut ; configurable, pour une instance hébergée chez soi (ADR-0030).
pub const DEFAULT_WORLD_URL: &str = "https://photon.komoot.io";

/// Nombre de propositions demandées.
const LIMIT: &str = "6";

/// Longueurs admises par une adresse (R-TIE-23) : un champ plus long est laissé vide, une ville plus longue
/// écarte la proposition.
const MAX_LINE: usize = 200;
const MAX_POSTAL_CODE: usize = 10;
const MAX_CITY: usize = 80;

/// Pays où le numéro précède la voie (« 8 Boulevard du Port ») ; ailleurs, il la suit (« Unter den Linden 77 »).
const NUMBER_FIRST: [&str; 9] = ["AU", "CA", "FR", "GB", "IE", "LU", "MC", "NZ", "US"];

/// Une adresse proposée par la recherche : choisie, elle remplit l'adresse du tiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AddressSuggestion {
    /// Libellé complet, à afficher (« 8 Boulevard du Port 80000 Amiens »).
    pub label: String,
    /// Numéro et voie ; absent pour une commune ou un lieu-dit sans voie.
    pub line: Option<String>,
    pub postal_code: Option<String>,
    pub city: String,
    pub country_code: String,
    pub latitude_e6: i32,
    pub longitude_e6: i32,
}

/// Service interrogé pour un pays (ADR-0030).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Base Adresse Nationale, pour la France.
    Ban,
    /// Photon, pour tout autre pays.
    Photon,
}

impl Provider {
    pub fn for_country(country_code: &str) -> Self {
        if country_code == FRANCE { Self::Ban } else { Self::Photon }
    }

    /// Nom du service, pour dire lequel ne répond pas.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ban => "la Base Adresse Nationale",
            Self::Photon => "Photon",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Geocoder {
    http: reqwest::Client,
    ban_url: String,
    photon_url: String,
}

impl Geocoder {
    pub fn new(ban_url: impl Into<String>, photon_url: impl Into<String>) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(concat!("cashflow/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("client HTTP du géocodage")?;
        let base = |url: String| url.trim_end_matches('/').to_owned();
        Ok(Self { http, ban_url: base(ban_url.into()), photon_url: base(photon_url.into()) })
    }

    /// Propositions pour `query` dans le pays `country_code` (ISO 3166-1 alpha-2, en majuscules).
    pub async fn search(&self, query: &str, country_code: &str) -> anyhow::Result<Vec<AddressSuggestion>> {
        match Provider::for_country(country_code) {
            Provider::Ban => {
                let url = reqwest::Url::parse_with_params(
                    &format!("{}/search/", self.ban_url),
                    [("q", query), ("limit", LIMIT), ("autocomplete", "1")],
                )?;
                Ok(suggestions(&self.get(url).await?))
            }
            Provider::Photon => {
                let url = reqwest::Url::parse_with_params(
                    &format!("{}/api/", self.photon_url),
                    [("q", query), ("limit", LIMIT), ("lang", "fr"), ("countrycode", country_code)],
                )?;
                Ok(world_suggestions(&self.get(url).await?, country_code))
            }
        }
    }

    async fn get(&self, url: reqwest::Url) -> anyhow::Result<Value> {
        let response = self.http.get(url).send().await?.error_for_status()?;
        Ok(serde_json::from_slice(&response.bytes().await?)?)
    }
}

/// Lit la réponse GeoJSON de la BAN ; ignore une proposition incomplète plutôt que d'échouer.
pub fn suggestions(body: &Value) -> Vec<AddressSuggestion> {
    features(body).filter_map(suggestion).collect()
}

/// Lit la réponse GeoJSON de Photon pour le pays demandé ; ignore une proposition incomplète ou d'un autre pays,
/// et un libellé déjà proposé (un même lieu peut revenir sous deux étiquettes OpenStreetMap).
pub fn world_suggestions(body: &Value, country_code: &str) -> Vec<AddressSuggestion> {
    let mut seen = HashSet::new();
    features(body)
        .filter_map(|feature| world_suggestion(feature, country_code))
        .filter(|s| seen.insert(s.label.clone()))
        .collect()
}

fn features(body: &Value) -> impl Iterator<Item = &Value> {
    body.get("features").and_then(Value::as_array).into_iter().flatten()
}

/// Coordonnées d'une proposition. GeoJSON : longitude d'abord. Les nombres sont relus sous leur forme écrite,
/// sans flottant.
fn point(feature: &Value) -> Option<GeoPoint> {
    let coordinates = feature.pointer("/geometry/coordinates")?.as_array()?;
    let (longitude, latitude) = (coordinates.first()?.to_string(), coordinates.get(1)?.to_string());
    GeoPoint::from_degrees(&latitude, &longitude).ok()
}

/// Texte non vide d'une propriété.
fn text<'a>(properties: &'a Value, key: &str) -> Option<&'a str> {
    properties.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty())
}

fn suggestion(feature: &Value) -> Option<AddressSuggestion> {
    let properties = feature.get("properties")?;
    let text = |key: &str| text(properties, key);
    let point = point(feature)?;
    // Une commune ou une rue sans numéro n'a pas de ligne d'adresse distincte de la ville.
    let line = match text("type") {
        Some("housenumber" | "street" | "locality") => text("name").map(str::to_owned),
        _ => None,
    };
    Some(AddressSuggestion {
        label: text("label")?.to_owned(),
        line,
        postal_code: text("postcode").map(str::to_owned),
        city: text("city")?.to_owned(),
        country_code: FRANCE.to_owned(),
        latitude_e6: point.latitude_e6,
        longitude_e6: point.longitude_e6,
    })
}

fn world_suggestion(feature: &Value, country_code: &str) -> Option<AddressSuggestion> {
    let properties = feature.get("properties")?;
    let text = |key: &str| text(properties, key);
    if !text("countrycode")?.eq_ignore_ascii_case(country_code) {
        return None;
    }
    let point = point(feature)?;
    let (kind, name, street) = (text("type"), text("name"), text("street"));
    // Une ville, un lieu-dit ou un quartier sans ville parente est lui-même la ville ; une région ou un pays
    // n'en a pas.
    let city = match (text("city"), kind) {
        (Some(city), _) => city,
        (None, Some("city" | "locality" | "district")) => name?,
        (None, _) => return None,
    };
    let line = match (kind, street) {
        // La voie elle-même.
        (Some("street"), _) => name.map(str::to_owned),
        (_, Some(street)) => Some(match text("housenumber") {
            Some(number) if NUMBER_FIRST.contains(&country_code) => format!("{number} {street}"),
            Some(number) => format!("{street} {number}"),
            None => street.to_owned(),
        }),
        // Un lieu sans voie (un hameau d'une commune, un bâtiment) : son nom.
        (Some("house" | "other"), None) => name.map(str::to_owned),
        (Some("locality"), None) if text("city").is_some() => name.map(str::to_owned),
        _ => None,
    }
    .filter(|line| line.chars().count() <= MAX_LINE && line != city);
    if city.chars().count() > MAX_CITY {
        return None;
    }
    let postal_code = text("postcode").filter(|p| p.chars().count() <= MAX_POSTAL_CODE);
    // Libellé : le nom d'un lieu s'il n'est pas déjà la voie ou la ville, l'adresse, puis la région pour
    // distinguer deux villes homonymes.
    let place = name.filter(|n| Some(*n) != line.as_deref() && *n != city && *n != street.unwrap_or_default());
    let town = postal_code.map_or_else(|| city.to_owned(), |postal| format!("{postal} {city}"));
    let region = text("state").or(text("county")).filter(|r| *r != city);
    let label = [place, line.as_deref(), Some(town.as_str()), region].into_iter().flatten().collect::<Vec<_>>();
    Some(AddressSuggestion {
        label: label.join(", "),
        line,
        postal_code: postal_code.map(str::to_owned),
        city: city.to_owned(),
        country_code: country_code.to_owned(),
        latitude_e6: point.latitude_e6,
        longitude_e6: point.longitude_e6,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn feature(kind: &str, name: &str, coordinates: Value) -> Value {
        json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": coordinates },
            "properties": {
                "label": format!("{name} 80000 Amiens"), "name": name, "postcode": "80000", "city": "Amiens",
                "type": kind, "score": 0.59
            }
        })
    }

    #[test]
    fn r_tie_26_reads_the_suggestions_of_the_national_address_base() {
        let body = json!({
            "type": "FeatureCollection",
            "features": [
                feature("housenumber", "8 Boulevard du Port", json!([2.290084, 49.897442])),
                feature("municipality", "Amiens", json!([2.290874, 49.901842])),
                feature("housenumber", "Incomplète", json!([2.29])),
            ]
        });
        let found = suggestions(&body);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0],
            AddressSuggestion {
                label: "8 Boulevard du Port 80000 Amiens".into(),
                line: Some("8 Boulevard du Port".into()),
                postal_code: Some("80000".into()),
                city: "Amiens".into(),
                country_code: "FR".into(),
                latitude_e6: 49_897_442,
                longitude_e6: 2_290_084,
            }
        );
        // Une commune : pas de ligne d'adresse.
        assert_eq!(found[1].line, None);
        assert!(suggestions(&json!({ "error": "q must contain at least 3 chars" })).is_empty());
    }

    #[test]
    fn r_tie_26_the_search_follows_the_country() {
        assert_eq!(Provider::for_country("FR"), Provider::Ban);
        assert_eq!(Provider::for_country("DE"), Provider::Photon);
    }

    fn place(properties: Value, coordinates: Value) -> Value {
        json!({ "type": "Feature", "geometry": { "type": "Point", "coordinates": coordinates }, "properties": properties })
    }

    /// Réponse de Photon (`lang=fr`) : lieux publics, aucune donnée personnelle.
    #[test]
    fn r_tie_26_reads_the_suggestions_of_photon_abroad() {
        let berlin = json!({
            "type": "FeatureCollection",
            "features": [
                place(
                    json!({
                        "type": "house", "osm_key": "tourism", "osm_value": "hotel", "name": "Adlon Kempinski",
                        "housenumber": "77", "street": "Unter den Linden", "district": "Mitte", "city": "Berlin",
                        "postcode": "10117", "state": "Berlin", "country": "Allemagne", "countrycode": "DE"
                    }),
                    json!([13.3800699, 52.5155757]),
                ),
                // Le même lieu sous une autre étiquette OpenStreetMap : un seul libellé.
                place(
                    json!({
                        "type": "house", "name": "Adlon Kempinski", "housenumber": "77", "street": "Unter den Linden",
                        "city": "Berlin", "postcode": "10117", "state": "Berlin", "countrycode": "DE"
                    }),
                    json!([13.38, 52.5156]),
                ),
                // Hors du pays demandé, ou sans ville (une région) : écartées.
                place(json!({ "type": "city", "name": "Vienne", "countrycode": "AT" }), json!([16.37, 48.21])),
                place(json!({ "type": "state", "name": "Brandebourg", "countrycode": "DE" }), json!([13.0, 52.5])),
            ]
        });
        assert_eq!(
            world_suggestions(&berlin, "DE"),
            [AddressSuggestion {
                label: "Adlon Kempinski, Unter den Linden 77, 10117 Berlin".into(),
                line: Some("Unter den Linden 77".into()),
                postal_code: Some("10117".into()),
                city: "Berlin".into(),
                country_code: "DE".into(),
                latitude_e6: 52_515_576,
                longitude_e6: 13_380_070,
            }]
        );

        let elsewhere = json!({
            "features": [
                // Une voie : la ligne est son nom.
                place(
                    json!({
                        "type": "street", "name": "Gran Vía", "postcode": "28013", "city": "Madrid",
                        "state": "Communauté de Madrid", "countrycode": "ES"
                    }),
                    json!([-3.7075578, 40.421304]),
                ),
                // Une ville sans ville parente : elle-même ; pas de ligne d'adresse.
                place(
                    json!({
                        "type": "city", "name": "Ostuni", "postcode": "72017", "county": "Brindisi",
                        "state": "Pouilles", "countrycode": "IT"
                    }),
                    json!([17.5796779, 40.7277814]),
                ),
                // Un code postal trop long est laissé vide.
                place(
                    json!({
                        "type": "house", "housenumber": "10", "street": "Downing Street", "city": "Londres",
                        "postcode": "SW1A 2AA LONDON", "countrycode": "GB"
                    }),
                    json!([-0.1276, 51.5034]),
                ),
            ]
        });
        let summary = |country: &str| {
            world_suggestions(&elsewhere, country)
                .into_iter()
                .map(|s| (s.label, s.line, s.postal_code, s.city))
                .collect::<Vec<_>>()
        };
        let s = |v: &str| Some(v.to_owned());
        assert_eq!(
            summary("ES"),
            [("Gran Vía, 28013 Madrid, Communauté de Madrid".into(), s("Gran Vía"), s("28013"), "Madrid".into())]
        );
        assert_eq!(summary("IT"), [("72017 Ostuni, Pouilles".into(), None, s("72017"), "Ostuni".into())]);
        assert_eq!(
            summary("GB"),
            [("10 Downing Street, Londres".into(), s("10 Downing Street"), None, "Londres".into())]
        );
    }
}
