//! Import de relevés (spec 03) : normalisation, empreintes, correspondances, règles, contrôle du relevé.
//!
//! Tout ici est pur : les lecteurs de fichiers (CSV, XLSX, OFX…) vivent côté serveur et produisent des
//! lignes brutes que ces fonctions transforment.

pub mod amount;
pub mod date;
pub mod fingerprint;
pub mod matcher;
pub mod rules;

use serde::{Deserialize, Serialize};

use crate::shared::{Date, Money};

/// Correspondance d'une ligne avec l'existant (modèle de données, `ImportLine.match_kind`).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchKind {
    None,
    Duplicate,
    Candidate,
}

/// Décision de l'utilisateur pour une ligne (glossaire).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImportDecision {
    Create,
    Match,
    Skip,
}

/// Statut d'un lot d'import.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchStatus {
    Reviewing,
    Committed,
    Cancelled,
    Reverted,
}

/// Contrôle interne du relevé (R-IMP-18) : `fin − début − Σ lignes valides`. Non nul : une ligne mal
/// lue ou manquante.
pub fn statement_gap(opening: Money, closing: Money, valid_lines: &[Money]) -> Money {
    closing - opening - valid_lines.iter().sum::<Money>()
}

/// Point de contrôle déduit d'un solde de début de relevé : la veille du début de période (R-RAP-2).
pub fn opening_checkpoint_date(period_start: Date) -> Option<Date> {
    period_start.pred_opt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_imp_18_statement_gap() {
        let lines = [Money::from_minor(-1_000), Money::from_minor(2_500)];
        assert_eq!(statement_gap(Money::from_minor(10_000), Money::from_minor(11_500), &lines), Money::ZERO);
        assert_eq!(
            statement_gap(Money::from_minor(10_000), Money::from_minor(11_000), &lines),
            Money::from_minor(-500)
        );
    }

    #[test]
    fn r_rap_2_opening_checkpoint_is_day_before_period() {
        let start: Date = "2026-03-01".parse().unwrap();
        assert_eq!(opening_checkpoint_date(start), Some("2026-02-28".parse().unwrap()));
    }
}
