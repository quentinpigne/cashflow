//! Types partagés par tous les modules du domaine.

pub mod currency;
pub mod error;
pub mod ids;
pub mod money;
pub mod rate;

pub use currency::Currency;
pub use error::DomainError;
pub use money::Money;
pub use rate::Rate;

/// Date calendaire, sans heure ni fuseau (ADR-0012).
pub type Date = chrono::NaiveDate;
