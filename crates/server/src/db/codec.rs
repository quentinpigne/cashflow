//! Conversions entre colonnes SQLite et types du domaine.

use std::str::FromStr;

use anyhow::{Context, anyhow};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Enum du domaine → valeur stockée (`'LIVRET_A'`), via sa représentation serde.
pub fn enum_to_db<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => String::new(),
    }
}

/// Valeur stockée → enum du domaine.
pub fn enum_from_db<T: DeserializeOwned>(value: &str) -> anyhow::Result<T> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .with_context(|| format!("valeur inconnue en base : {value}"))
}

/// Identifiant stocké → identifiant typé.
pub fn id_from_db<T: FromStr>(value: &str) -> anyhow::Result<T> {
    value.parse().map_err(|_| anyhow!("identifiant invalide en base : {value}"))
}

pub fn opt_id_from_db<T: FromStr>(value: Option<&str>) -> anyhow::Result<Option<T>> {
    value.map(id_from_db).transpose()
}
