//! Relevés de compte (CPT-11) : les documents de type relevé rattachés à un compte (spec 12, R-CPT-18 à R-CPT-21).
//! Envoi, modification et suppression passent par la bibliothèque (`services::documents`).

use cashflow_domain::Date;
use cashflow_domain::accounts::statements;
use cashflow_domain::documents::{DocumentKind, MediaType};
use cashflow_domain::shared::ids::{AccountId, DocumentId};
use serde::Serialize;
use utoipa::ToSchema;

use super::documents;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Relevé archivé (R-CPT-18) : un document de la bibliothèque.
#[derive(Debug, Serialize, ToSchema)]
pub struct StatementView {
    pub id: DocumentId,
    pub title: String,
    /// Premier jour du mois du relevé.
    pub statement_month: Date,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub file_name: String,
    pub media_type: MediaType,
    pub size_bytes: i64,
    pub note: Option<String>,
}

/// Relevés d'un compte et mois sans relevé (R-CPT-21).
#[derive(Debug, Serialize, ToSchema)]
pub struct AccountStatements {
    /// Du mois le plus ancien au plus récent.
    pub statements: Vec<StatementView>,
    /// Du plus récent au plus ancien.
    pub missing_months: Vec<Date>,
}

pub async fn list(db: &Db, account: AccountId, today: Date) -> AppResult<AccountStatements> {
    let account = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    let rows = documents::owned(db, DocumentKind::BankStatement, *account.id.as_uuid()).await?;
    let statements: Vec<StatementView> = rows
        .into_iter()
        .filter_map(|row| {
            Some(StatementView {
                statement_month: row.month?,
                id: row.id,
                title: row.title,
                period_start: row.period_start,
                period_end: row.period_end,
                file_name: row.file_name,
                media_type: row.media_type,
                size_bytes: row.size_bytes,
                note: row.note,
            })
        })
        .collect();
    let months: Vec<Date> = statements.iter().map(|s| s.statement_month).collect();
    Ok(AccountStatements { missing_months: statements::missing_months(&months, account.closed_on, today), statements })
}
