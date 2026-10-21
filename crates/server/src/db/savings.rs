//! Épargne (spec 06) : conditions, barèmes de taux, mouvements datés en valeur, valorisations.

use cashflow_domain::accounts::{Capitalization, InterestMethod};
use cashflow_domain::ledger::TransactionOrigin;
use cashflow_domain::shared::ids::{AccountId, CategoryId, RateScheduleId, TransactionId, ValuationId};
use cashflow_domain::{Date, Money, Rate};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

// ---------------------------------------------------------------- Conditions

#[derive(Debug, Clone)]
pub struct TermsRow {
    pub account_id: AccountId,
    pub rate_schedule_id: RateScheduleId,
    pub schedule_name: String,
    pub shared: bool,
    pub interest_method: InterestMethod,
    pub capitalization: Capitalization,
    pub deposit_cap: Option<Money>,
    pub maturity_date: Option<Date>,
    pub interest_category_id: Option<CategoryId>,
    pub round_each_fortnight: bool,
}

pub async fn all_terms(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<TermsRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.account_id, t.rate_schedule_id, s.name AS schedule_name, s.shared AS "shared: bool",
                  t.interest_method, t.capitalization, t.deposit_cap, t.maturity_date AS "maturity_date: Date",
                  t.interest_category_id, t.round_each_fortnight AS "round_each_fortnight: bool"
           FROM fixed_savings_terms t JOIN rate_schedule s ON s.id = t.rate_schedule_id"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(TermsRow {
                account_id: id_from_db(&r.account_id)?,
                rate_schedule_id: id_from_db(&r.rate_schedule_id)?,
                schedule_name: r.schedule_name,
                shared: r.shared,
                interest_method: enum_from_db(&r.interest_method)?,
                capitalization: enum_from_db(&r.capitalization)?,
                deposit_cap: r.deposit_cap.map(Money::from_minor),
                maturity_date: r.maturity_date,
                interest_category_id: opt_id_from_db(r.interest_category_id.as_deref())?,
                round_each_fortnight: r.round_each_fortnight,
            })
        })
        .collect()
}

pub async fn update_terms(db: impl SqliteExecutor<'_>, t: &TermsRow) -> anyhow::Result<()> {
    let account = t.account_id.to_string();
    let schedule = t.rate_schedule_id.to_string();
    let method = enum_to_db(&t.interest_method);
    let capitalization = enum_to_db(&t.capitalization);
    let cap = t.deposit_cap.map(Money::minor);
    let category = t.interest_category_id.map(|c| c.to_string());
    let now = timestamp();
    sqlx::query!(
        "UPDATE fixed_savings_terms SET rate_schedule_id = ?, interest_method = ?, capitalization = ?, deposit_cap = ?,
                maturity_date = ?, interest_category_id = ?, round_each_fortnight = ?, updated_at = ?
         WHERE account_id = ?",
        schedule,
        method,
        capitalization,
        cap,
        t.maturity_date,
        category,
        t.round_each_fortnight,
        now,
        account
    )
    .execute(db)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------- Barèmes

#[derive(Debug, Clone)]
pub struct ScheduleRow {
    pub id: RateScheduleId,
    pub name: String,
    pub shared: bool,
}

pub async fn schedules(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ScheduleRow>> {
    let rows =
        sqlx::query!(r#"SELECT id, name, shared AS "shared: bool" FROM rate_schedule ORDER BY shared DESC, name"#)
            .fetch_all(db)
            .await?;
    rows.into_iter().map(|r| Ok(ScheduleRow { id: id_from_db(&r.id)?, name: r.name, shared: r.shared })).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct PeriodRow {
    pub schedule_id: RateScheduleId,
    pub effective_from: Date,
    pub rate: Rate,
}

pub async fn periods(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<PeriodRow>> {
    let rows = sqlx::query!(
        r#"SELECT schedule_id, effective_from AS "effective_from: Date", annual_rate FROM rate_period
           ORDER BY schedule_id, effective_from"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PeriodRow {
                schedule_id: id_from_db(&r.schedule_id)?,
                effective_from: r.effective_from,
                rate: Rate::from_ppm(r.annual_rate),
            })
        })
        .collect()
}

/// Ajoute un taux daté, ou remplace celui de la même date.
pub async fn set_period(db: impl SqliteExecutor<'_>, p: &PeriodRow) -> anyhow::Result<()> {
    let id = cashflow_domain::shared::ids::RatePeriodId::new().to_string();
    let schedule = p.schedule_id.to_string();
    let rate = p.rate.ppm();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO rate_period (id, schedule_id, effective_from, annual_rate, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT (schedule_id, effective_from) DO UPDATE SET annual_rate = excluded.annual_rate,
                                                                updated_at = excluded.updated_at",
        id,
        schedule,
        p.effective_from,
        rate,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_period(db: impl SqliteExecutor<'_>, schedule: RateScheduleId, from: Date) -> anyhow::Result<u64> {
    let schedule = schedule.to_string();
    let result = sqlx::query!("DELETE FROM rate_period WHERE schedule_id = ? AND effective_from = ?", schedule, from)
        .execute(db)
        .await?;
    Ok(result.rows_affected())
}

// ---------------------------------------------------------------- Mouvements

/// Un mouvement d'un compte, pour les intérêts et la performance.
#[derive(Debug, Clone)]
pub struct MovementRow {
    pub id: TransactionId,
    /// Date de valeur, à défaut date bancaire, à défaut date (R-EPA-0).
    pub interest_date: Date,
    pub balance_date: Date,
    pub amount: Money,
    pub origin: TransactionOrigin,
    /// Une ventilation porte la catégorie des intérêts du compte.
    pub is_interest: bool,
}

pub async fn movements(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    interest_category: Option<CategoryId>,
) -> anyhow::Result<Vec<MovementRow>> {
    let account = account.to_string();
    let category = interest_category.map(|c| c.to_string());
    let rows = sqlx::query!(
        r#"SELECT t.id, coalesce(t.value_date, t.bank_date, t.date) AS "interest_date!: Date",
                  t.balance_date AS "balance_date!: Date", t.amount AS "amount!: i64", t.origin AS "origin!",
                  EXISTS (SELECT 1 FROM split s WHERE s.transaction_id = t.id AND s.category_id = ?2) AS "is_interest!: bool"
           FROM ledger_transaction t WHERE t.account_id = ?1 ORDER BY t.balance_date, t.id"#,
        account,
        category
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(MovementRow {
                id: id_from_db(&r.id)?,
                interest_date: r.interest_date,
                balance_date: r.balance_date,
                amount: Money::from_minor(r.amount),
                origin: enum_from_db(&r.origin)?,
                is_interest: r.is_interest,
            })
        })
        .collect()
}

// ---------------------------------------------------------------- Valorisations

#[derive(Debug, Clone)]
pub struct ValuationRow {
    pub id: ValuationId,
    pub date: Date,
    pub value: Money,
    pub note: Option<String>,
}

pub async fn valuations(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<Vec<ValuationRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT id, date AS "date: Date", value, note FROM valuation WHERE account_id = ? ORDER BY date"#,
        account
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ValuationRow { id: id_from_db(&r.id)?, date: r.date, value: Money::from_minor(r.value), note: r.note })
        })
        .collect()
}

/// Enregistre une valorisation ; celle de la même date est remplacée (CPT-06).
pub async fn set_valuation(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    date: Date,
    value: Money,
    note: Option<&str>,
) -> anyhow::Result<()> {
    let id = ValuationId::new().to_string();
    let account = account.to_string();
    let value = value.minor();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO valuation (id, account_id, date, value, source, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'MANUAL', ?5, ?6, ?6)
         ON CONFLICT (account_id, date) DO UPDATE SET value = excluded.value, note = excluded.note,
                                                     source = excluded.source, updated_at = excluded.updated_at",
        id,
        account,
        date,
        value,
        note,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_valuation(db: impl SqliteExecutor<'_>, account: AccountId, id: ValuationId) -> anyhow::Result<u64> {
    let account = account.to_string();
    let id = id.to_string();
    let result = sqlx::query!("DELETE FROM valuation WHERE id = ? AND account_id = ?", id, account).execute(db).await?;
    Ok(result.rows_affected())
}
