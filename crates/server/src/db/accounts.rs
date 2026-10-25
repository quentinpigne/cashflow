//! Comptes, conditions d'épargne fixe et barèmes de taux (spec 01, spec 06).

use cashflow_domain::accounts::{AccountKind, Capitalization, InterestMethod};
use cashflow_domain::shared::ids::{AccountId, CategoryId, InstitutionId, RatePeriodId, RateScheduleId};
use cashflow_domain::{Currency, Date, Money, Rate};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub id: AccountId,
    pub name: String,
    pub kind: AccountKind,
    pub institution_id: Option<InstitutionId>,
    pub currency: Currency,
    pub opening_date: Date,
    pub opening_balance: Money,
    pub iban: Option<String>,
    pub account_number: Option<String>,
    pub closed_on: Option<Date>,
    pub include_in_net_worth: bool,
    pub include_in_emergency_fund: bool,
    pub include_in_budget: bool,
    pub display_order: i64,
    pub color: Option<String>,
    pub notes: Option<String>,
}

/// Champs écrits à la création comme à la modification.
pub struct AccountFields<'a> {
    pub name: &'a str,
    pub kind: AccountKind,
    pub institution_id: Option<InstitutionId>,
    pub currency: Currency,
    pub opening_date: Date,
    pub opening_balance: Money,
    pub iban: Option<&'a str>,
    pub account_number: Option<&'a str>,
    pub include_in_net_worth: bool,
    pub include_in_emergency_fund: bool,
    pub include_in_budget: bool,
    pub display_order: i64,
    pub color: Option<&'a str>,
    pub notes: Option<&'a str>,
}

macro_rules! account_row {
    ($r:expr) => {{
        let r = $r;
        anyhow::Ok(AccountRow {
            id: id_from_db(&r.id)?,
            name: r.name,
            kind: enum_from_db(&r.kind)?,
            institution_id: opt_id_from_db(r.institution_id.as_deref())?,
            currency: r.currency.parse()?,
            opening_date: r.opening_date,
            opening_balance: Money::from_minor(r.opening_balance),
            iban: r.iban,
            account_number: r.account_number,
            closed_on: r.closed_on,
            include_in_net_worth: r.include_in_net_worth,
            include_in_emergency_fund: r.include_in_emergency_fund,
            include_in_budget: r.include_in_budget,
            display_order: r.display_order,
            color: r.color,
            notes: r.notes,
        })
    }};
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<AccountRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, kind, institution_id, currency,
                  opening_date AS "opening_date: Date", opening_balance, iban, account_number,
                  closed_on AS "closed_on: Date",
                  include_in_net_worth AS "include_in_net_worth: bool",
                  include_in_emergency_fund AS "include_in_emergency_fund: bool",
                  include_in_budget AS "include_in_budget: bool",
                  display_order, color, notes
           FROM account ORDER BY display_order, name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| account_row!(r)).collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: AccountId) -> anyhow::Result<Option<AccountRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, name, kind, institution_id, currency,
                  opening_date AS "opening_date: Date", opening_balance, iban, account_number,
                  closed_on AS "closed_on: Date",
                  include_in_net_worth AS "include_in_net_worth: bool",
                  include_in_emergency_fund AS "include_in_emergency_fund: bool",
                  include_in_budget AS "include_in_budget: bool",
                  display_order, color, notes
           FROM account WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| account_row!(r)).transpose()
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: AccountId, f: &AccountFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let kind = enum_to_db(&f.kind);
    let institution = f.institution_id.map(|i| i.to_string());
    let currency = f.currency.code().to_owned();
    let opening_balance = f.opening_balance.minor();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO account (id, name, kind, institution_id, currency, opening_date, opening_balance, iban,
                              account_number, include_in_net_worth, include_in_emergency_fund, include_in_budget,
                              display_order, color, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        f.name,
        kind,
        institution,
        currency,
        f.opening_date,
        opening_balance,
        f.iban,
        f.account_number,
        f.include_in_net_worth,
        f.include_in_emergency_fund,
        f.include_in_budget,
        f.display_order,
        f.color,
        f.notes,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: AccountId, f: &AccountFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let kind = enum_to_db(&f.kind);
    let institution = f.institution_id.map(|i| i.to_string());
    let opening_balance = f.opening_balance.minor();
    let now = timestamp();
    sqlx::query!(
        "UPDATE account SET name = ?, kind = ?, institution_id = ?, opening_date = ?, opening_balance = ?, iban = ?,
                            account_number = ?, include_in_net_worth = ?, include_in_emergency_fund = ?,
                            include_in_budget = ?, display_order = ?, color = ?, notes = ?, updated_at = ?
         WHERE id = ?",
        f.name,
        kind,
        institution,
        f.opening_date,
        opening_balance,
        f.iban,
        f.account_number,
        f.include_in_net_worth,
        f.include_in_emergency_fund,
        f.include_in_budget,
        f.display_order,
        f.color,
        f.notes,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_closed_on(db: impl SqliteExecutor<'_>, id: AccountId, closed_on: Option<Date>) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE account SET closed_on = ?, updated_at = ? WHERE id = ?", closed_on, now, id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: AccountId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM account WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Nombre de transactions et de valorisations du compte (R-CPT-1, R-CPT-7).
pub async fn activity_count(db: impl SqliteExecutor<'_>, id: AccountId) -> anyhow::Result<i64> {
    let id = id.to_string();
    let count = sqlx::query_scalar!(
        "SELECT (SELECT count(*) FROM ledger_transaction WHERE account_id = ?1)
              + (SELECT count(*) FROM valuation WHERE account_id = ?1)",
        id
    )
    .fetch_one(db)
    .await?;
    Ok(count)
}

/// Bornes des dates des transactions du compte, pour vérifier une nouvelle période (R-CPT-3).
pub async fn transaction_date_bounds(
    db: impl SqliteExecutor<'_>,
    id: AccountId,
) -> anyhow::Result<Option<(Date, Date)>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT min(date) AS "min_date: Date", min(balance_date) AS "min_balance: Date",
                  max(date) AS "max_date: Date", max(balance_date) AS "max_balance: Date"
           FROM ledger_transaction WHERE account_id = ?"#,
        id
    )
    .fetch_one(db)
    .await?;
    let first = row.min_date.into_iter().chain(row.min_balance).min();
    let last = row.max_date.into_iter().chain(row.max_balance).max();
    Ok(first.zip(last))
}

// ---------------------------------------------------------------- Barèmes et conditions d'épargne

pub async fn find_schedule_by_name(db: impl SqliteExecutor<'_>, name: &str) -> anyhow::Result<Option<RateScheduleId>> {
    let id = sqlx::query_scalar!("SELECT id FROM rate_schedule WHERE name = ?", name).fetch_optional(db).await?;
    id.map(|id| id_from_db(&id)).transpose()
}

pub async fn insert_schedule(
    db: impl SqliteExecutor<'_>,
    id: RateScheduleId,
    name: &str,
    shared: bool,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO rate_schedule (id, name, shared, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        id,
        name,
        shared,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn schedule_name_taken(db: impl SqliteExecutor<'_>, name: &str) -> anyhow::Result<bool> {
    let count = sqlx::query_scalar!("SELECT count(*) FROM rate_schedule WHERE name = ?", name).fetch_one(db).await?;
    Ok(count > 0)
}

/// Ajoute une période de taux si elle n'existe pas encore (données initiales idempotentes).
pub async fn insert_rate_period_if_missing(
    db: impl SqliteExecutor<'_>,
    schedule: RateScheduleId,
    effective_from: Date,
    rate: Rate,
) -> anyhow::Result<bool> {
    let id = RatePeriodId::new().to_string();
    let schedule = schedule.to_string();
    let ppm = rate.ppm();
    let now = timestamp();
    let result = sqlx::query!(
        "INSERT INTO rate_period (id, schedule_id, effective_from, annual_rate, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT (schedule_id, effective_from) DO NOTHING",
        id,
        schedule,
        effective_from,
        ppm,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub struct TermsFields {
    pub rate_schedule_id: RateScheduleId,
    pub interest_method: InterestMethod,
    pub capitalization: Capitalization,
    pub deposit_cap: Option<Money>,
    pub maturity_date: Option<Date>,
    pub interest_category_id: Option<CategoryId>,
}

pub async fn insert_terms(db: impl SqliteExecutor<'_>, account: AccountId, f: &TermsFields) -> anyhow::Result<()> {
    let account = account.to_string();
    let schedule = f.rate_schedule_id.to_string();
    let method = enum_to_db(&f.interest_method);
    let capitalization = enum_to_db(&f.capitalization);
    let cap = f.deposit_cap.map(Money::minor);
    let category = f.interest_category_id.map(|c| c.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO fixed_savings_terms (account_id, rate_schedule_id, interest_method, capitalization, deposit_cap,
                                          maturity_date, interest_category_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        account,
        schedule,
        method,
        capitalization,
        cap,
        f.maturity_date,
        category,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn has_terms(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<bool> {
    let account = account.to_string();
    let count = sqlx::query_scalar!("SELECT count(*) FROM fixed_savings_terms WHERE account_id = ?", account)
        .fetch_one(db)
        .await?;
    Ok(count > 0)
}

/// Supprime les conditions et, s'il n'est utilisé par aucun autre compte, le barème propre au compte.
pub async fn delete_terms(conn: &mut sqlx::SqliteConnection, account: AccountId) -> anyhow::Result<()> {
    let account = account.to_string();
    let schedule = sqlx::query_scalar!(
        r#"SELECT s.id FROM fixed_savings_terms t JOIN rate_schedule s ON s.id = t.rate_schedule_id
           WHERE t.account_id = ? AND s.shared = 0"#,
        account
    )
    .fetch_optional(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM fixed_savings_terms WHERE account_id = ?", account).execute(&mut *conn).await?;
    if let Some(schedule) = schedule {
        sqlx::query!(
            "DELETE FROM rate_schedule WHERE id = ? AND NOT EXISTS
               (SELECT 1 FROM fixed_savings_terms WHERE rate_schedule_id = rate_schedule.id)",
            schedule
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Toutes les valorisations, pour la valeur des comptes d'épargne variable et des prêts (R-CPT-6).
pub async fn all_valuations(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<(AccountId, Date, Money)>> {
    let rows =
        sqlx::query!(r#"SELECT account_id, date AS "date: Date", value FROM valuation ORDER BY account_id, date"#)
            .fetch_all(db)
            .await?;
    rows.into_iter().map(|r| Ok((id_from_db(&r.account_id)?, r.date, Money::from_minor(r.value)))).collect()
}

/// Ordre d'affichage d'un compte (CPT-07) ; renvoie faux si le compte n'existe pas.
pub async fn set_display_order(db: impl SqliteExecutor<'_>, id: AccountId, order: i64) -> anyhow::Result<bool> {
    let id = id.to_string();
    let now = timestamp();
    let result = sqlx::query!("UPDATE account SET display_order = ?, updated_at = ? WHERE id = ?", order, now, id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}
