//! Recherche d'adresse par la Base Adresse Nationale (R-TIE-26, ADR-0025).
//!
//! Seul le serveur appelle la BAN : le navigateur ne contacte jamais un service tiers. L'appel n'a lieu que si
//! la recherche d'adresse est activée dans les réglages (contrôlé par le service).

use std::time::Duration;

use anyhow::Context;
use cashflow_domain::payees::location::{FRANCE, GeoPoint};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

/// Adresse de la BAN par défaut ; configurable si le service déménage (Géoplateforme de l'IGN).
pub const DEFAULT_URL: &str = "https://api-adresse.data.gouv.fr";

/// Nombre de propositions demandées.
const LIMIT: &str = "6";

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

#[derive(Debug, Clone)]
pub struct Geocoder {
    http: reqwest::Client,
    base_url: String,
}

impl Geocoder {
    pub fn new(base_url: impl Into<String>) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent(concat!("cashflow/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("client HTTP du géocodage")?;
        Ok(Self { http, base_url: base_url.into().trim_end_matches('/').to_owned() })
    }

    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<AddressSuggestion>> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/search/", self.base_url),
            [("q", query), ("limit", LIMIT), ("autocomplete", "1")],
        )?;
        let response = self.http.get(url).send().await?.error_for_status()?;
        let body: Value = serde_json::from_slice(&response.bytes().await?)?;
        Ok(suggestions(&body))
    }
}

/// Lit la réponse GeoJSON de la BAN ; ignore une proposition incomplète plutôt que d'échouer.
pub fn suggestions(body: &Value) -> Vec<AddressSuggestion> {
    let Some(features) = body.get("features").and_then(Value::as_array) else { return Vec::new() };
    features.iter().filter_map(suggestion).collect()
}

fn suggestion(feature: &Value) -> Option<AddressSuggestion> {
    let properties = feature.get("properties")?;
    let text = |key: &str| properties.get(key).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
    let coordinates = feature.pointer("/geometry/coordinates")?.as_array()?;
    // GeoJSON : longitude d'abord. Les nombres sont relus sous leur forme écrite, sans flottant.
    let (longitude, latitude) = (coordinates.first()?.to_string(), coordinates.get(1)?.to_string());
    let point = GeoPoint::from_degrees(&latitude, &longitude).ok()?;
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
}
