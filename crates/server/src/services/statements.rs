//! Relevés de compte archivés (CPT-11, ADR-0024) : envoi, archivage à la validation d'un import PDF (R-IMP-28),
//! consultation, modification et suppression.

use cashflow_domain::accounts::statements;
use cashflow_domain::documents::MediaType;
use cashflow_domain::shared::ids::{AccountId, BankStatementId};
use cashflow_domain::{Date, DomainError};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::{clean, documents};
use crate::config::DataDir;
use crate::db::statements::{NewStatement, StatementRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Réglage qui marque la reprise des imports PDF validés avant CPT-11 comme faite (ADR-0024).
const IMPORTS_TAKEN_OVER: &str = "statements.imports_taken_over";

/// Relevé archivé (R-CPT-18).
#[derive(Debug, Serialize, ToSchema)]
pub struct StatementView {
    pub id: BankStatementId,
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

/// Mois et note d'un relevé ; le fichier ne se modifie pas (R-CPT-20).
#[derive(Debug, Deserialize, ToSchema)]
pub struct StatementInput {
    /// Premier jour du mois du relevé.
    pub statement_month: Date,
    pub note: Option<String>,
}

/// Relevé envoyé depuis la fiche du compte (R-CPT-19).
pub struct StatementUpload {
    pub file_name: String,
    pub content: Vec<u8>,
    pub statement_month: Date,
    pub note: Option<String>,
}

fn view(row: StatementRow) -> StatementView {
    StatementView {
        id: row.id,
        statement_month: row.statement_month,
        period_start: row.period_start,
        period_end: row.period_end,
        file_name: row.file_name,
        media_type: row.media_type,
        size_bytes: row.size_bytes,
        note: row.note,
    }
}

pub async fn list(db: &Db, account: AccountId, today: Date) -> AppResult<AccountStatements> {
    let account = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    let rows = db::statements::of_account(&db.read, account.id).await?;
    let months: Vec<Date> = rows.iter().map(|s| s.statement_month).collect();
    Ok(AccountStatements {
        missing_months: statements::missing_months(&months, account.closed_on, today),
        statements: rows.into_iter().map(view).collect(),
    })
}

async fn statement(db: &Db, account: AccountId, id: BankStatementId) -> AppResult<StatementRow> {
    db::statements::of_account(&db.read, account)
        .await?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or(AppError::NotFound("relevé"))
}

/// Archive un relevé envoyé depuis le compte (R-CPT-18, R-CPT-19).
pub async fn add(
    db: &Db,
    data: &DataDir,
    account: AccountId,
    upload: StatementUpload,
    today: Date,
) -> AppResult<AccountStatements> {
    db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    statements::check(upload.statement_month, None)?;
    statements::check_file(&upload.content)?;
    let staged = documents::stage(data, &upload.file_name, &upload.content)?;
    if let Some(month) = db::statements::archived_month(&db.read, account, &staged.row.sha256).await? {
        return Err(DomainError::StatementAlreadyArchived { statement_month: month }.into());
    }
    let note = clean(upload.note.as_deref());
    let mut tx = db.write.begin().await?;
    db::documents::insert(&mut *tx, &staged.row).await?;
    let new = NewStatement {
        account,
        statement_month: upload.statement_month,
        period: None,
        document: staged.row.id,
        note: note.as_deref(),
    };
    db::statements::insert(&mut *tx, &new).await?;
    tx.commit().await?;
    staged.publish()?;
    list(db, account, today).await
}

/// Relevé d'un import PDF à archiver avec la validation du lot (R-IMP-28).
pub struct FromImport {
    pub account: AccountId,
    pub file_name: String,
    pub content: Vec<u8>,
    pub period: Option<(Date, Date)>,
    pub period_end: Option<Date>,
}

/// Prépare l'archivage d'un relevé importé dans la transaction de validation : rien si le fichier n'est pas un
/// PDF ou s'il est déjà archivé pour ce compte (R-CPT-19). Le document renvoyé est publié après la validation.
pub async fn archive_import(
    conn: &mut SqliteConnection,
    data: &DataDir,
    import: &FromImport,
    committed_on: Date,
) -> AppResult<Option<documents::Staged>> {
    if statements::check_file(&import.content).is_err() {
        return Ok(None);
    }
    let staged = documents::stage(data, &import.file_name, &import.content)?;
    if db::statements::archived_month(&mut *conn, import.account, &staged.row.sha256).await?.is_some() {
        return Ok(None);
    }
    let period = import.period.filter(|(start, end)| start <= end);
    let new = NewStatement {
        account: import.account,
        statement_month: statements::month_of_import(period, import.period_end, committed_on),
        period,
        document: staged.row.id,
        note: None,
    };
    db::documents::insert(&mut *conn, &staged.row).await?;
    db::statements::insert(&mut *conn, &new).await?;
    Ok(Some(staged))
}

/// Modifie le mois et la note d'un relevé (R-CPT-20).
pub async fn update(
    db: &Db,
    account: AccountId,
    id: BankStatementId,
    input: &StatementInput,
    today: Date,
) -> AppResult<AccountStatements> {
    statements::check(input.statement_month, None)?;
    let note = clean(input.note.as_deref());
    if db::statements::update(&db.write, account, id, input.statement_month, note.as_deref()).await? == 0 {
        return Err(AppError::NotFound("relevé"));
    }
    list(db, account, today).await
}

/// Supprime un relevé et son document (R-CPT-20).
pub async fn delete(db: &Db, data: &DataDir, account: AccountId, id: BankStatementId, today: Date) -> AppResult<()> {
    let row = statement(db, account, id).await?;
    let mut tx = db.write.begin().await?;
    db::statements::delete(&mut *tx, account, id).await?;
    db::documents::delete(&mut *tx, row.document_id).await?;
    tx.commit().await?;
    documents::trash(data, &[row.document_id], today);
    Ok(())
}

/// Fichier d'un relevé, avec son nom et son type (R-CPT-21).
pub async fn file(
    db: &Db,
    data: &DataDir,
    account: AccountId,
    id: BankStatementId,
) -> AppResult<(StatementRow, Vec<u8>)> {
    let row = statement(db, account, id).await?;
    let content = documents::read(data, row.document_id)?;
    Ok((row, content))
}

/// Reprend une fois les imports PDF validés avant CPT-11 dont le fichier source existe encore (ADR-0024) ;
/// renvoie le nombre de relevés archivés.
pub async fn take_over_imports(db: &Db, data: &DataDir, today: Date) -> AppResult<usize> {
    if db::settings::get::<bool>(&db.read, IMPORTS_TAKEN_OVER).await?.unwrap_or(false) {
        return Ok(0);
    }
    let mut archived = 0;
    for batch in db::statements::committed_batches(&db.read).await? {
        let Some(content) = super::imports::stored_source(data, batch.id) else { continue };
        let committed_on = batch
            .committed_at
            .as_deref()
            .and_then(|at| at.get(..10))
            .and_then(|day| day.parse::<Date>().ok())
            .unwrap_or(today);
        let import = FromImport {
            account: batch.account_id,
            file_name: batch.file_name,
            content,
            period: batch.period_start.zip(batch.period_end),
            period_end: batch.period_end,
        };
        let mut tx = db.write.begin().await?;
        let staged = archive_import(&mut tx, data, &import, committed_on).await?;
        tx.commit().await?;
        if let Some(staged) = staged {
            staged.publish()?;
            archived += 1;
        }
    }
    db::settings::set(&db.write, IMPORTS_TAKEN_OVER, &true).await?;
    Ok(archived)
}
