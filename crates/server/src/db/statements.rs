//! Relevés de compte archivés (CPT-11, ADR-0024).

use cashflow_domain::Date;
use cashflow_domain::documents::MediaType;
use cashflow_domain::shared::ids::{AccountId, BankStatementId, DocumentId, ImportBatchId};
use sqlx::SqliteExecutor;

use super::codec::id_from_db;
use super::documents::media_type_from_db;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct StatementRow {
    pub id: BankStatementId,
    pub statement_month: Date,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub document_id: DocumentId,
    pub note: Option<String>,
    pub file_name: String,
    pub media_type: MediaType,
    pub size_bytes: i64,
}

/// Relevés d'un compte, du mois le plus ancien au plus récent.
pub async fn of_account(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<Vec<StatementRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT s.id, s.statement_month AS "statement_month: Date", s.period_start AS "period_start: Date",
                  s.period_end AS "period_end: Date", s.document_id, s.note,
                  d.file_name, d.media_type, d.size_bytes
           FROM bank_statement s JOIN document d ON d.id = s.document_id
           WHERE s.account_id = ? ORDER BY s.statement_month, s.created_at"#,
        account
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(StatementRow {
                id: id_from_db(&r.id)?,
                statement_month: r.statement_month,
                period_start: r.period_start,
                period_end: r.period_end,
                document_id: id_from_db(&r.document_id)?,
                note: r.note,
                file_name: r.file_name,
                media_type: media_type_from_db(&r.media_type)?,
                size_bytes: r.size_bytes,
            })
        })
        .collect()
}

/// Mois du relevé déjà archivé de ce fichier pour ce compte, s'il y en a un (R-CPT-19).
pub async fn archived_month(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    sha256: &str,
) -> anyhow::Result<Option<Date>> {
    let account = account.to_string();
    Ok(sqlx::query_scalar!(
        r#"SELECT s.statement_month AS "statement_month: Date"
           FROM bank_statement s JOIN document d ON d.id = s.document_id
           WHERE s.account_id = ? AND d.sha256 = ? LIMIT 1"#,
        account,
        sha256
    )
    .fetch_optional(db)
    .await?)
}

pub struct NewStatement<'a> {
    pub account: AccountId,
    pub statement_month: Date,
    pub period: Option<(Date, Date)>,
    pub document: DocumentId,
    pub note: Option<&'a str>,
}

pub async fn insert(db: impl SqliteExecutor<'_>, statement: &NewStatement<'_>) -> anyhow::Result<BankStatementId> {
    let id = BankStatementId::new();
    let (id_text, account, document) = (id.to_string(), statement.account.to_string(), statement.document.to_string());
    let (start, end) = (statement.period.map(|p| p.0), statement.period.map(|p| p.1));
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO bank_statement (id, account_id, statement_month, period_start, period_end, document_id, note,
                                     created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id_text,
        account,
        statement.statement_month,
        start,
        end,
        document,
        statement.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(id)
}

/// Modifie le mois et la note ; le fichier ne change pas (R-CPT-20).
pub async fn update(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    id: BankStatementId,
    statement_month: Date,
    note: Option<&str>,
) -> anyhow::Result<u64> {
    let (id, account) = (id.to_string(), account.to_string());
    let now = timestamp();
    Ok(sqlx::query!(
        "UPDATE bank_statement SET statement_month = ?, note = ?, updated_at = ? WHERE id = ? AND account_id = ?",
        statement_month,
        note,
        now,
        id,
        account
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn delete(db: impl SqliteExecutor<'_>, account: AccountId, id: BankStatementId) -> anyhow::Result<u64> {
    let (id, account) = (id.to_string(), account.to_string());
    Ok(sqlx::query!("DELETE FROM bank_statement WHERE id = ? AND account_id = ?", id, account)
        .execute(db)
        .await?
        .rows_affected())
}

/// Lots PDF validés, candidats à la reprise des relevés (R-IMP-28) : identifiant, compte, nom du fichier, fin de
/// période et date de validation.
pub struct CommittedBatch {
    pub id: ImportBatchId,
    pub account_id: AccountId,
    pub file_name: String,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub committed_at: Option<String>,
}

pub async fn committed_batches(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<CommittedBatch>> {
    let rows = sqlx::query!(
        r#"SELECT id, account_id, file_name, period_start AS "period_start: Date", period_end AS "period_end: Date",
                  committed_at
           FROM import_batch WHERE status = 'COMMITTED' ORDER BY created_at"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(CommittedBatch {
                id: id_from_db(&r.id)?,
                account_id: id_from_db(&r.account_id)?,
                file_name: r.file_name,
                period_start: r.period_start,
                period_end: r.period_end,
                committed_at: r.committed_at,
            })
        })
        .collect()
}
