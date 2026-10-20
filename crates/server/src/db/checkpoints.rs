//! Points de contrôle (spec 04).

use cashflow_domain::shared::ids::{AccountId, CheckpointId, ImportBatchId};
use cashflow_domain::{Date, Money};
use sqlx::SqliteExecutor;

use super::codec::{id_from_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct CheckpointRow {
    pub id: CheckpointId,
    pub account_id: AccountId,
    pub date: Date,
    pub balance: Money,
    pub source: String,
    pub import_batch_id: Option<ImportBatchId>,
    pub reconciled_at: Option<String>,
}

macro_rules! checkpoint_row {
    ($r:expr) => {{
        let r = $r;
        anyhow::Ok(CheckpointRow {
            id: id_from_db(&r.id)?,
            account_id: id_from_db(&r.account_id)?,
            date: r.date,
            balance: Money::from_minor(r.balance),
            source: r.source,
            import_batch_id: opt_id_from_db(r.import_batch_id.as_deref())?,
            reconciled_at: r.reconciled_at,
        })
    }};
}

pub async fn list_for_account(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<Vec<CheckpointRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT id, account_id, date AS "date: Date", balance, source, import_batch_id, reconciled_at
           FROM balance_checkpoint WHERE account_id = ? ORDER BY date"#,
        account
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| checkpoint_row!(r)).collect()
}

pub async fn list_all(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<CheckpointRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, account_id, date AS "date: Date", balance, source, import_batch_id, reconciled_at
           FROM balance_checkpoint ORDER BY account_id, date"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| checkpoint_row!(r)).collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: CheckpointId) -> anyhow::Result<Option<CheckpointRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, account_id, date AS "date: Date", balance, source, import_batch_id, reconciled_at
           FROM balance_checkpoint WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| checkpoint_row!(r)).transpose()
}

pub async fn find_at(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    date: Date,
) -> anyhow::Result<Option<CheckpointRow>> {
    let account = account.to_string();
    let row = sqlx::query!(
        r#"SELECT id, account_id, date AS "date: Date", balance, source, import_batch_id, reconciled_at
           FROM balance_checkpoint WHERE account_id = ? AND date = ?"#,
        account,
        date
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| checkpoint_row!(r)).transpose()
}

pub async fn insert(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    date: Date,
    balance: Money,
    source: &str,
    batch: Option<ImportBatchId>,
) -> anyhow::Result<CheckpointId> {
    let checkpoint = CheckpointId::new();
    let id = checkpoint.to_string();
    let account = account.to_string();
    let balance = balance.minor();
    let batch = batch.map(|b| b.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO balance_checkpoint (id, account_id, date, balance, source, import_batch_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        id,
        account,
        date,
        balance,
        source,
        batch,
        now
    )
    .execute(db)
    .await?;
    Ok(checkpoint)
}

pub async fn update_balance(
    db: impl SqliteExecutor<'_>,
    id: CheckpointId,
    balance: Money,
    source: &str,
    batch: Option<ImportBatchId>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let balance = balance.minor();
    let batch = batch.map(|b| b.to_string());
    let now = timestamp();
    sqlx::query!(
        "UPDATE balance_checkpoint SET balance = ?, source = ?, import_batch_id = ?, updated_at = ? WHERE id = ?",
        balance,
        source,
        batch,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: CheckpointId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM balance_checkpoint WHERE id = ?", id).execute(db).await?;
    Ok(())
}

pub async fn delete_for_batch(db: impl SqliteExecutor<'_>, batch: ImportBatchId) -> anyhow::Result<()> {
    let batch = batch.to_string();
    sqlx::query!("DELETE FROM balance_checkpoint WHERE import_batch_id = ? AND reconciled_at IS NULL", batch)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn set_reconciled(db: impl SqliteExecutor<'_>, id: CheckpointId, reconciled: bool) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    let at = reconciled.then(|| now.clone());
    sqlx::query!("UPDATE balance_checkpoint SET reconciled_at = ?, updated_at = ? WHERE id = ?", at, now, id)
        .execute(db)
        .await?;
    Ok(())
}

/// Date du dernier point rapproché du compte : la période verrouillée (R-RAP-7).
pub async fn locked_until(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<Option<Date>> {
    let account = account.to_string();
    Ok(sqlx::query_scalar!(
        r#"SELECT max(date) AS "date: Date" FROM balance_checkpoint WHERE account_id = ? AND reconciled_at IS NOT NULL"#,
        account
    )
    .fetch_one(db)
    .await?)
}

/// Passe en `RECONCILED` toutes les transactions de date de solde ≤ `until` (R-RAP-5).
pub async fn reconcile_transactions(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    until: Date,
) -> anyhow::Result<u64> {
    let account = account.to_string();
    let now = timestamp();
    let result = sqlx::query!(
        "UPDATE ledger_transaction SET clearing_status = 'RECONCILED', updated_at = ?
         WHERE account_id = ? AND balance_date <= ? AND clearing_status <> 'RECONCILED'",
        now,
        account,
        until
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// Repasse en `CLEARED` les transactions rapprochées qui ne sont plus couvertes (R-RAP-6).
pub async fn unreconcile_after(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    still_locked_until: Option<Date>,
) -> anyhow::Result<()> {
    let account = account.to_string();
    let now = timestamp();
    sqlx::query!(
        "UPDATE ledger_transaction SET clearing_status = 'CLEARED', updated_at = ?
         WHERE account_id = ? AND clearing_status = 'RECONCILED' AND (? IS NULL OR balance_date > ?)",
        now,
        account,
        still_locked_until,
        still_locked_until
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Pointage manuel (RAP-07) : `PENDING` ↔ `CLEARED`.
pub async fn set_cleared(
    db: impl SqliteExecutor<'_>,
    transaction: cashflow_domain::shared::ids::TransactionId,
    cleared: bool,
) -> anyhow::Result<u64> {
    let id = transaction.to_string();
    let status = if cleared { "CLEARED" } else { "PENDING" };
    let now = timestamp();
    let result = sqlx::query!(
        "UPDATE ledger_transaction SET clearing_status = ?, updated_at = ? WHERE id = ? AND clearing_status <> 'RECONCILED'",
        status,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}
