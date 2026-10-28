//! Projections (spec 07) : scénarios et données de départ.

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, CategoryId, ScenarioId, TransactionId};
use cashflow_domain::{Date, Money, Rate};
use serde_json::Value;
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{enum_from_db, id_from_db, opt_id_from_db};
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

/// Ventilations de dépense d'un compte (hors virements et catégories hors analyses), par transaction (R-PRJ-11).
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
             AND NOT EXISTS (SELECT 1 FROM transfer tr WHERE s.id IN (tr.outflow_split_id, tr.inflow_split_id))
             AND NOT EXISTS (SELECT 1 FROM excluded_category x WHERE x.id = s.category_id)"#,
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

/// Une ventilation vue par la capacité d'épargne (R-PRJ-43, R-PRJ-44).
pub struct CapacityFactRow {
    pub account_id: AccountId,
    pub amount: Money,
    pub category_id: Option<CategoryId>,
    pub parent_id: Option<CategoryId>,
    pub category_kind: Option<CategoryKind>,
    pub account_kind: AccountKind,
    pub currency: String,
    pub is_movement: bool,
    pub is_interest: bool,
    pub is_installment: bool,
}

pub async fn capacity_facts(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<CapacityFactRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.account_id, s.amount, s.category_id, c.parent_id, c.kind AS "category_kind?", a.kind AS account_kind, a.currency,
                  (s.loan_id IS NOT NULL OR s.asset_id IS NOT NULL
                   OR EXISTS (SELECT 1 FROM transfer tr WHERE s.id IN (tr.outflow_split_id, tr.inflow_split_id))
                   OR EXISTS (SELECT 1 FROM excluded_category x WHERE x.id = s.category_id))
                      AS "is_movement!: bool",
                  t.origin = 'INTEREST' AS "is_interest!: bool",
                  EXISTS (SELECT 1 FROM loan_payment lp WHERE lp.transaction_id = t.id) AS "is_installment!: bool"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN account a ON a.id = t.account_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE t.date BETWEEN ? AND ?"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(CapacityFactRow {
                account_id: id_from_db(&r.account_id)?,
                amount: Money::from_minor(r.amount),
                category_id: opt_id_from_db(r.category_id.as_deref())?,
                parent_id: opt_id_from_db(r.parent_id.as_deref())?,
                category_kind: r.category_kind.as_deref().map(enum_from_db).transpose()?,
                account_kind: enum_from_db(&r.account_kind)?,
                currency: r.currency,
                is_movement: r.is_movement,
                is_interest: r.is_interest,
                is_installment: r.is_installment,
            })
        })
        .collect()
}
