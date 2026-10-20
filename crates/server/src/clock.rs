//! Horloge fournie aux services : le domaine reçoit toujours la date du jour en paramètre (ADR-0012).

use chrono::{Local, NaiveDate, Utc};

#[derive(Debug, Clone, Copy)]
pub enum Clock {
    /// Date locale du serveur (fuseau du conteneur, variable `TZ`).
    System,
    /// Date figée, pour les tests et le jeu de démonstration.
    Fixed(NaiveDate),
}

impl Clock {
    pub fn today(&self) -> NaiveDate {
        match self {
            Clock::System => Local::now().date_naive(),
            Clock::Fixed(date) => *date,
        }
    }
}

/// Horodatage d'audit, ISO 8601 UTC terminé par `Z` (modèle de données).
pub fn timestamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}
