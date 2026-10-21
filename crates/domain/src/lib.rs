//! Domaine de Cashflow : types, invariants et moteurs de calcul (ADR-0013).
//!
//! Pur et sans I/O : aucune dépendance à la base, au système de fichiers, au réseau ou à l'horloge.
//! La date du jour est toujours un paramètre. Les flottants y sont interdits (ADR-0001).

pub mod accounts;
pub mod budgeting;
pub mod import;
pub mod ledger;
pub mod projection;
pub mod reconciliation;
pub mod recurrence;
pub mod reporting;
pub mod savings;
pub mod shared;

pub use shared::{Currency, Date, DomainError, Money, Rate};
