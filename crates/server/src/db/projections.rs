//! Projections (spec 07) : scénarios et données de départ.

use cashflow_domain::shared::ids::{AccountId, ScenarioId, TransactionId};
use cashflow_domain::{Date, Money, Rate};
use serde_json::Value;
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::id_from_db;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct ScenarioRow {
    pub id: ScenarioId,
    pub name: String,
    pub horizon_months: u32,
    pub inflation_rate: Option<Rate>,
    pub assumptions: Value,
    pub is_default: bool,
}

pub async fn scenarios(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ScenarioRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, horizon_months, inflation_rate, assumptions, is_default AS "is_default: bool"
           FROM projection_scenario ORDER BY created_at, name"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ScenarioRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                horizon_months: u32::try_from(r.horizon_months)?,
                inflation_rate: r.inflation_rate.map(Rate::from_ppm),
                assumptions: serde_json::from_str(&r.assumptions)?,
                is_default: r.is_default,
            })
        })
        .collect()
}

/// Enregistre un scénario ; s'il est le scénario par défaut, les autres cessent de l'être.
pub async fn upsert(conn: &mut SqliteConnection, s: &ScenarioRow) -> anyhow::Result<()> {
    let id = s.id.to_string();
    let inflation = s.inflation_rate.map(Rate::ppm);
    let assumptions = serde_json::to_string(&s.assumptions)?;
    let now = timestamp();
    if s.is_default {
        sqlx::query!("UPDATE projection_scenario SET is_default = 0 WHERE id <> ?", id).execute(&mut *conn).await?;
    }
    sqlx::query!(
        "INSERT INTO projection_scenario (id, name, horizon_months, inflation_rate, assumptions, is_default, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, horizon_months = excluded.horizon_months,
                                        inflation_rate = excluded.inflation_rate, assumptions = excluded.assumptions,
                                        is_default = excluded.is_default, updated_at = excluded.updated_at",
        id,
        s.name,
        s.horizon_months,
        inflation,
        assumptions,
        s.is_default,
        now
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: ScenarioId) -> anyhow::Result<u64> {
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM projection_scenario WHERE id = ?", id).execute(db).await?.rows_affected())
}

/// Un mouvement d'un compte, daté par sa date de solde.
pub struct DatedRow {
    pub id: TransactionId,
    pub date: Date,
    pub amount: Money,
    pub label: String,
}

/// Transactions d'un compte de date de solde postérieure à `after` (R-PRJ-10).
pub async fn future(db: impl SqliteExecutor<'_>, account: AccountId, after: Date) -> anyhow::Result<Vec<DatedRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT id, balance_date AS "date!: Date", amount AS "amount!: i64", label AS "label!"
           FROM ledger_transaction WHERE account_id = ? AND balance_date > ? ORDER BY balance_date"#,
        account,
        after
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(DatedRow { id: id_from_db(&r.id)?, date: r.date, amount: Money::from_minor(r.amount), label: r.label })
        })
        .collect()
}

/// Ventilations de dépense d'un compte (hors virements), par transaction (R-PRJ-11).
pub async fn expenses(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    from: Date,
    to: Date,
) -> anyhow::Result<Vec<DatedRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT t.id, t.balance_date AS "date!: Date", s.amount AS "amount!: i64", t.label AS "label!"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN category c ON c.id = s.category_id
           WHERE t.account_id = ? AND t.balance_date BETWEEN ? AND ? AND c.kind = 'EXPENSE'
             AND NOT EXISTS (SELECT 1 FROM transfer tr WHERE t.id IN (tr.outflow_transaction_id, tr.inflow_transaction_id))"#,
        account,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(DatedRow { id: id_from_db(&r.id)?, date: r.date, amount: Money::from_minor(r.amount), label: r.label })
        })
        .collect()
}
