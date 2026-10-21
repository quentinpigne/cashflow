//! Services : un par cas d'usage. Ils ouvrent la transaction SQLite, chargent les données, appellent le
//! domaine et écrivent le résultat (ADR-0013). Aucun calcul métier ici.

pub mod accounts;
pub mod auth;
pub mod backup;
pub mod budgets;
pub mod categories;
pub mod dashboard;
pub mod demo;
pub mod export;
pub mod imports;
pub mod institutions;
pub mod projections;
pub mod reconciliation;
pub mod recurring;
pub mod reports;
pub mod rules;
pub mod savings;
pub mod seed;
pub mod transactions;
pub mod transfers;

use cashflow_domain::shared::ids::TransactionId;
use sqlx::SqliteConnection;

use crate::error::{AppError, AppResult};

/// Contrôle de l'invariant Σ ventilations avant la validation d'une transaction SQLite (ADR-0017, point 3).
///
/// Le domaine le garantit déjà : un écart ici est un défaut du code, qui annule l'écriture.
pub async fn check_split_sums(conn: &mut SqliteConnection, touched: &[TransactionId]) -> AppResult<()> {
    let mismatches = crate::db::transactions::split_sum_mismatches(conn, touched).await?;
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(AppError::Internal(anyhow::anyhow!("Σ ventilations ≠ montant pour {mismatches:?} : écriture annulée")))
    }
}

/// Texte saisi : espaces retirés, `None` s'il est vide.
pub fn clean(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

/// Texte obligatoire, borné en longueur.
pub fn required(value: &str, field: &'static str, max: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::invalid_field("REQUIRED", field, "ce champ est obligatoire"));
    }
    if value.chars().count() > max {
        return Err(AppError::invalid_field("TOO_LONG", field, format!("{max} caractères au maximum")));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod projection_tests;
#[cfg(test)]
mod savings_tests;
#[cfg(test)]
mod tests;
