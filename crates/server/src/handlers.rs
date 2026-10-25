//! Handlers HTTP : extraient la requête, appellent un service et mettent en forme la réponse. Aucun
//! calcul ni aucune requête SQL ici. Chacun est décrit pour le contrat OpenAPI (ADR-0016).

pub mod accounts;
pub mod assets;
pub mod auth;
pub mod budgets;
pub mod categories;
pub mod dashboard;
pub mod imports;
pub mod institutions;
pub mod loans;
pub mod payees;
pub mod projections;
pub mod reconciliation;
pub mod reports;
pub mod rules;
pub mod salaries;
pub mod savings;
pub mod system;
pub mod transactions;
