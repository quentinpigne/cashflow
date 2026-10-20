//! Profils, lots et lignes d'import (spec 03).

use cashflow_domain::import::{BatchStatus, ImportDecision, MatchKind};
use cashflow_domain::shared::ids::{AccountId, ImportBatchId, ImportLineId, ImportProfileId, TransactionId};
use cashflow_domain::{Date, Money};
use serde_json::Value;
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;
use crate::readers::FileFormat;

// ---------------------------------------------------------------- Profils

#[derive(Debug, Clone)]
pub struct ProfileRow {
    pub id: ImportProfileId,
    pub name: String,
    pub format: FileFormat,
    pub settings: Value,
    pub default_account_id: Option<AccountId>,
}

pub async fn list_profiles(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ProfileRow>> {
    let rows = sqlx::query!(
        "SELECT id, name, format, settings, default_account_id FROM import_profile ORDER BY name COLLATE NOCASE"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ProfileRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                format: enum_from_db(&r.format)?,
                settings: serde_json::from_str(&r.settings)?,
                default_account_id: opt_id_from_db(r.default_account_id.as_deref())?,
            })
        })
        .collect()
}

pub async fn find_profile(db: impl SqliteExecutor<'_>, id: ImportProfileId) -> anyhow::Result<Option<ProfileRow>> {
    let id = id.to_string();
    let row =
        sqlx::query!("SELECT id, name, format, settings, default_account_id FROM import_profile WHERE id = ?", id)
            .fetch_optional(db)
            .await?;
    row.map(|r| {
        Ok(ProfileRow {
            id: id_from_db(&r.id)?,
            name: r.name,
            format: enum_from_db(&r.format)?,
            settings: serde_json::from_str(&r.settings)?,
            default_account_id: opt_id_from_db(r.default_account_id.as_deref())?,
        })
    })
    .transpose()
}

pub async fn profile_name_taken(
    db: impl SqliteExecutor<'_>,
    name: &str,
    except: Option<ImportProfileId>,
) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string()).unwrap_or_default();
    let count = sqlx::query_scalar!(
        "SELECT count(*) FROM import_profile WHERE name = ? COLLATE NOCASE AND id <> ?",
        name,
        except
    )
    .fetch_one(db)
    .await?;
    Ok(count > 0)
}

pub async fn upsert_profile(db: impl SqliteExecutor<'_>, p: &ProfileRow) -> anyhow::Result<()> {
    let id = p.id.to_string();
    let format = enum_to_db(&p.format);
    let settings = serde_json::to_string(&p.settings)?;
    let account = p.default_account_id.map(|a| a.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO import_profile (id, name, format, settings, default_account_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, format = excluded.format, settings = excluded.settings,
                                        default_account_id = excluded.default_account_id, updated_at = excluded.updated_at",
        id,
        p.name,
        format,
        settings,
        account,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_profile(db: impl SqliteExecutor<'_>, id: ImportProfileId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("UPDATE import_batch SET profile_id = NULL WHERE profile_id = ?", id).execute(db).await?;
    Ok(())
}

pub async fn delete_profile_row(db: impl SqliteExecutor<'_>, id: ImportProfileId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM import_profile WHERE id = ?", id).execute(db).await?;
    Ok(())
}

// ---------------------------------------------------------------- Lots

#[derive(Debug, Clone)]
pub struct BatchRow {
    pub id: ImportBatchId,
    pub account_id: AccountId,
    pub profile_id: Option<ImportProfileId>,
    pub file_name: String,
    pub file_sha256: String,
    pub status: BatchStatus,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub statement_opening_balance: Option<Money>,
    pub statement_closing_balance: Option<Money>,
    pub committed_at: Option<String>,
    pub created_at: String,
}

macro_rules! batch_row {
    ($r:expr) => {{
        let r = $r;
        anyhow::Ok(BatchRow {
            id: id_from_db(&r.id)?,
            account_id: id_from_db(&r.account_id)?,
            profile_id: opt_id_from_db(r.profile_id.as_deref())?,
            file_name: r.file_name,
            file_sha256: r.file_sha256,
            status: enum_from_db(&r.status)?,
            period_start: r.period_start,
            period_end: r.period_end,
            statement_opening_balance: r.statement_opening_balance.map(Money::from_minor),
            statement_closing_balance: r.statement_closing_balance.map(Money::from_minor),
            committed_at: r.committed_at,
            created_at: r.created_at,
        })
    }};
}

pub async fn insert_batch(db: impl SqliteExecutor<'_>, b: &BatchRow) -> anyhow::Result<()> {
    let id = b.id.to_string();
    let account = b.account_id.to_string();
    let profile = b.profile_id.map(|p| p.to_string());
    let status = enum_to_db(&b.status);
    let opening = b.statement_opening_balance.map(Money::minor);
    let closing = b.statement_closing_balance.map(Money::minor);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO import_batch (id, account_id, profile_id, file_name, file_sha256, status, period_start, period_end,
                                   statement_opening_balance, statement_closing_balance, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        id,
        account,
        profile,
        b.file_name,
        b.file_sha256,
        status,
        b.period_start,
        b.period_end,
        opening,
        closing,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find_batch(db: impl SqliteExecutor<'_>, id: ImportBatchId) -> anyhow::Result<Option<BatchRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, account_id, profile_id, file_name, file_sha256, status, period_start AS "period_start: Date",
                  period_end AS "period_end: Date", statement_opening_balance, statement_closing_balance, committed_at, created_at
           FROM import_batch WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| batch_row!(r)).transpose()
}

pub async fn list_batches(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<BatchRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, account_id, profile_id, file_name, file_sha256, status, period_start AS "period_start: Date",
                  period_end AS "period_end: Date", statement_opening_balance, statement_closing_balance, committed_at, created_at
           FROM import_batch ORDER BY created_at DESC"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| batch_row!(r)).collect()
}

/// Un lot validé avec le même fichier existe-t-il dans ce compte (R-IMP-7) ?
pub async fn committed_with_sha(db: impl SqliteExecutor<'_>, account: AccountId, sha: &str) -> anyhow::Result<bool> {
    let account = account.to_string();
    let count = sqlx::query_scalar!(
        "SELECT count(*) FROM import_batch WHERE account_id = ? AND file_sha256 = ? AND status = 'COMMITTED'",
        account,
        sha
    )
    .fetch_one(db)
    .await?;
    Ok(count > 0)
}

pub async fn set_batch_status(
    db: impl SqliteExecutor<'_>,
    id: ImportBatchId,
    status: BatchStatus,
    committed: bool,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let status = enum_to_db(&status);
    let now = timestamp();
    let committed_at = committed.then(|| now.clone());
    sqlx::query!(
        "UPDATE import_batch SET status = ?, committed_at = coalesce(?, committed_at), updated_at = ? WHERE id = ?",
        status,
        committed_at,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_batch(db: impl SqliteExecutor<'_>, id: ImportBatchId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM import_batch WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Nombre de lots d'un compte (un compte importé ne se supprime pas).
pub async fn batches_for_account(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<i64> {
    let account = account.to_string();
    Ok(sqlx::query_scalar!("SELECT count(*) FROM import_batch WHERE account_id = ?", account).fetch_one(db).await?)
}

// ---------------------------------------------------------------- Lignes

#[derive(Debug, Clone)]
pub struct LineRow {
    pub id: ImportLineId,
    pub batch_id: ImportBatchId,
    pub line_number: i64,
    pub raw: Value,
    pub date: Option<Date>,
    pub value_date: Option<Date>,
    pub amount: Option<Money>,
    pub bank_label: Option<String>,
    pub fingerprint: Option<String>,
    pub errors: Vec<String>,
    pub match_kind: MatchKind,
    pub match_transaction_id: Option<TransactionId>,
    pub match_score: Option<i64>,
    pub suggested: Value,
    pub decision: Option<ImportDecision>,
    pub transaction_id: Option<TransactionId>,
}

macro_rules! line_row {
    ($r:expr) => {{
        let r = $r;
        anyhow::Ok(LineRow {
            id: id_from_db(&r.id)?,
            batch_id: id_from_db(&r.batch_id)?,
            line_number: r.line_number,
            raw: serde_json::from_str(&r.raw)?,
            date: r.date,
            value_date: r.value_date,
            amount: r.amount.map(Money::from_minor),
            bank_label: r.bank_label,
            fingerprint: r.fingerprint,
            errors: serde_json::from_str(&r.errors)?,
            match_kind: enum_from_db(&r.match_kind)?,
            match_transaction_id: opt_id_from_db(r.match_transaction_id.as_deref())?,
            match_score: r.match_score,
            suggested: r.suggested.as_deref().map(serde_json::from_str).transpose()?.unwrap_or(Value::Null),
            decision: r.decision.as_deref().map(enum_from_db).transpose()?,
            transaction_id: opt_id_from_db(r.transaction_id.as_deref())?,
        })
    }};
}

pub async fn insert_line(db: impl SqliteExecutor<'_>, l: &LineRow) -> anyhow::Result<()> {
    let id = l.id.to_string();
    let batch = l.batch_id.to_string();
    let raw = serde_json::to_string(&l.raw)?;
    let amount = l.amount.map(Money::minor);
    let errors = serde_json::to_string(&l.errors)?;
    let kind = enum_to_db(&l.match_kind);
    let matched = l.match_transaction_id.map(|t| t.to_string());
    let suggested = serde_json::to_string(&l.suggested)?;
    let decision = l.decision.map(|d| enum_to_db(&d));
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO import_line (id, batch_id, line_number, raw, date, value_date, amount, bank_label, fingerprint, errors,
                                  match_kind, match_transaction_id, match_score, suggested, decision, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)",
        id,
        batch,
        l.line_number,
        raw,
        l.date,
        l.value_date,
        amount,
        l.bank_label,
        l.fingerprint,
        errors,
        kind,
        matched,
        l.match_score,
        suggested,
        decision,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn lines_of(db: impl SqliteExecutor<'_>, batch: ImportBatchId) -> anyhow::Result<Vec<LineRow>> {
    let batch = batch.to_string();
    let rows = sqlx::query!(
        r#"SELECT id, batch_id, line_number, raw, date AS "date: Date", value_date AS "value_date: Date", amount, bank_label,
                  fingerprint, errors, match_kind, match_transaction_id, match_score, suggested, decision, transaction_id
           FROM import_line WHERE batch_id = ? ORDER BY line_number"#,
        batch
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| line_row!(r)).collect()
}

/// Lignes valides des autres lots en revue : candidates de virement entre lots (R-IMP-14).
pub async fn reviewing_lines_elsewhere(
    db: impl SqliteExecutor<'_>,
    batch: ImportBatchId,
) -> anyhow::Result<Vec<(AccountId, LineRow)>> {
    let batch = batch.to_string();
    let rows = sqlx::query!(
        r#"SELECT l.id, l.batch_id, l.line_number, l.raw, l.date AS "date: Date", l.value_date AS "value_date: Date", l.amount,
                  l.bank_label, l.fingerprint, l.errors, l.match_kind, l.match_transaction_id, l.match_score, l.suggested,
                  l.decision, l.transaction_id, b.account_id AS "account_id!"
           FROM import_line l JOIN import_batch b ON b.id = l.batch_id
           WHERE b.status = 'REVIEWING' AND b.id <> ? AND l.amount IS NOT NULL AND l.errors = '[]'"#,
        batch
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            let account = id_from_db(&r.account_id)?;
            Ok((account, line_row!(r)?))
        })
        .collect()
}

pub async fn update_line_review(
    db: impl SqliteExecutor<'_>,
    id: ImportLineId,
    decision: Option<ImportDecision>,
    match_transaction: Option<TransactionId>,
    suggested: &Value,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let decision = decision.map(|d| enum_to_db(&d));
    let matched = match_transaction.map(|t| t.to_string());
    let suggested = serde_json::to_string(suggested)?;
    let now = timestamp();
    sqlx::query!(
        "UPDATE import_line SET decision = ?, match_transaction_id = ?, suggested = ?, updated_at = ? WHERE id = ?",
        decision,
        matched,
        suggested,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_line_transaction(
    db: impl SqliteExecutor<'_>,
    id: ImportLineId,
    transaction: Option<TransactionId>,
    suggested: &Value,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let transaction = transaction.map(|t| t.to_string());
    let suggested = serde_json::to_string(suggested)?;
    let now = timestamp();
    sqlx::query!(
        "UPDATE import_line SET transaction_id = ?, suggested = ?, updated_at = ? WHERE id = ?",
        transaction,
        suggested,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------- Transactions vues par l'import

/// Empreintes déjà présentes sur les transactions du compte (R-IMP-6).
pub async fn fingerprints_of(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
) -> anyhow::Result<Vec<(String, TransactionId)>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT fingerprint AS "fingerprint!", id FROM ledger_transaction WHERE account_id = ? AND fingerprint IS NOT NULL"#,
        account
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| Ok((r.fingerprint, id_from_db(&r.id)?))).collect()
}

/// Transactions susceptibles d'être pointées par une ligne (R-IMP-9) : `PENDING`, ou `CLEARED` sans empreinte.
pub struct MatchableRow {
    pub id: TransactionId,
    pub balance_date: Date,
    pub amount: Money,
    pub label: String,
}

pub async fn matchable(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    from: Date,
    to: Date,
) -> anyhow::Result<Vec<MatchableRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", balance_date AS "balance_date!: Date", amount AS "amount!", label AS "label!"
           FROM ledger_transaction
           WHERE account_id = ? AND balance_date BETWEEN ? AND ?
             AND (clearing_status = 'PENDING' OR (clearing_status = 'CLEARED' AND fingerprint IS NULL))"#,
        account,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(MatchableRow {
                id: id_from_db(&r.id)?,
                balance_date: r.balance_date,
                amount: Money::from_minor(r.amount),
                label: r.label,
            })
        })
        .collect()
}

/// Libellés bancaires et catégorie unique des transactions du compte, les plus récentes d'abord (R-IMP-13).
pub async fn labelled_categories(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
) -> anyhow::Result<Vec<(String, String)>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT coalesce(t.bank_label, t.label) AS "label!: String", s.category_id AS "category_id!"
           FROM ledger_transaction t JOIN split s ON s.transaction_id = t.id
           WHERE t.account_id = ? AND s.category_id IS NOT NULL
             AND (SELECT count(*) FROM split x WHERE x.transaction_id = t.id) = 1
           ORDER BY t.date DESC, t.id DESC"#,
        account
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.label, r.category_id)).collect())
}

/// État d'une transaction pointée, pour pouvoir annuler l'import (R-IMP-17).
pub async fn pointing_state(db: impl SqliteExecutor<'_>, id: TransactionId) -> anyhow::Result<Option<Value>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT clearing_status AS "clearing_status!", bank_date AS "bank_date?", bank_label AS "bank_label?",
                  value_date AS "value_date?", fingerprint AS "fingerprint?"
           FROM ledger_transaction WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| {
        serde_json::json!({
            "clearing_status": r.clearing_status,
            "bank_date": r.bank_date,
            "bank_label": r.bank_label,
            "value_date": r.value_date,
            "fingerprint": r.fingerprint,
        })
    }))
}

/// Pointe une transaction existante (R-IMP-11) : `CLEARED`, données bancaires de la ligne.
pub async fn point(
    db: impl SqliteExecutor<'_>,
    id: TransactionId,
    bank_date: Date,
    bank_label: &str,
    value_date: Option<Date>,
    fingerprint: Option<&str>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!(
        "UPDATE ledger_transaction SET clearing_status = 'CLEARED', bank_date = ?, bank_label = ?, value_date = ?,
                                       fingerprint = ?, updated_at = ?
         WHERE id = ?",
        bank_date,
        bank_label,
        value_date,
        fingerprint,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Rétablit l'état d'avant le pointage (R-IMP-17).
pub async fn unpoint(db: impl SqliteExecutor<'_>, id: TransactionId, previous: &Value) -> anyhow::Result<()> {
    let id = id.to_string();
    let field = |k: &str| previous.get(k).and_then(Value::as_str).map(str::to_owned);
    let status = field("clearing_status").unwrap_or_else(|| "PENDING".into());
    let (bank_date, bank_label, value_date, fingerprint) =
        (field("bank_date"), field("bank_label"), field("value_date"), field("fingerprint"));
    let now = timestamp();
    sqlx::query!(
        "UPDATE ledger_transaction SET clearing_status = ?, bank_date = ?, bank_label = ?, value_date = ?, fingerprint = ?,
                                       updated_at = ?
         WHERE id = ?",
        status,
        bank_date,
        bank_label,
        value_date,
        fingerprint,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}
