//! Prêts : conditions (ADR-0020, R-CPT-15).

use cashflow_domain::loans::{Insurance, LoanTerms};
use cashflow_domain::shared::ids::{AccountId, CategoryId};
use cashflow_domain::{Date, Money, Rate};
use sqlx::SqliteExecutor;

use super::codec::{id_from_db, opt_id_from_db};
use crate::clock::timestamp;

/// Conditions d'un prêt, avec le compte payeur et les catégories de ses dépenses.
#[derive(Debug, Clone)]
pub struct LoanTermsRow {
    pub account_id: AccountId,
    pub terms: LoanTerms,
    pub payer_account_id: AccountId,
    pub interest_category_id: CategoryId,
    pub insurance_category_id: Option<CategoryId>,
}

fn insurance_from_db(mode: &str, amount: Option<i64>, rate: Option<i64>) -> anyhow::Result<Insurance> {
    Ok(match (mode, amount, rate) {
        ("NONE", _, _) => Insurance::None,
        ("FIXED", Some(amount_minor), _) => Insurance::Fixed { amount_minor },
        ("RATE_ON_PRINCIPAL", _, Some(rate_ppm)) => Insurance::RateOnPrincipal { rate_ppm },
        _ => anyhow::bail!("assurance de prêt incohérente en base : {mode}"),
    })
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<LoanTermsRow>> {
    let rows = sqlx::query!(
        r#"SELECT account_id, principal, annual_rate, duration_months,
                  first_installment_date AS "first_installment_date: Date", insurance_mode, insurance_amount,
                  insurance_rate, payer_account_id, interest_category_id, insurance_category_id
           FROM loan_terms"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(LoanTermsRow {
                account_id: id_from_db(&r.account_id)?,
                terms: LoanTerms {
                    principal: Money::from_minor(r.principal),
                    annual_rate: Rate::from_ppm(r.annual_rate),
                    duration_months: u32::try_from(r.duration_months)?,
                    first_installment: r.first_installment_date,
                    insurance: insurance_from_db(&r.insurance_mode, r.insurance_amount, r.insurance_rate)?,
                },
                payer_account_id: id_from_db(&r.payer_account_id)?,
                interest_category_id: id_from_db(&r.interest_category_id)?,
                insurance_category_id: opt_id_from_db(r.insurance_category_id.as_deref())?,
            })
        })
        .collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<Option<LoanTermsRow>> {
    Ok(list(db).await?.into_iter().find(|t| t.account_id == account))
}

/// Crée ou remplace les conditions d'un prêt.
pub async fn upsert(db: impl SqliteExecutor<'_>, row: &LoanTermsRow) -> anyhow::Result<()> {
    let account = row.account_id.to_string();
    let t = &row.terms;
    let principal = t.principal.minor();
    let rate = t.annual_rate.ppm();
    let duration = i64::from(t.duration_months);
    let (mode, amount, insurance_rate) = match t.insurance {
        Insurance::None => ("NONE", None, None),
        Insurance::Fixed { amount_minor } => ("FIXED", Some(amount_minor), None),
        Insurance::RateOnPrincipal { rate_ppm } => ("RATE_ON_PRINCIPAL", None, Some(rate_ppm)),
    };
    let payer = row.payer_account_id.to_string();
    let interest_category = row.interest_category_id.to_string();
    let insurance_category = row.insurance_category_id.map(|c| c.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO loan_terms (account_id, principal, annual_rate, duration_months, first_installment_date,
                                 insurance_mode, insurance_amount, insurance_rate, payer_account_id,
                                 interest_category_id, insurance_category_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (account_id) DO UPDATE SET
             principal = excluded.principal, annual_rate = excluded.annual_rate,
             duration_months = excluded.duration_months, first_installment_date = excluded.first_installment_date,
             insurance_mode = excluded.insurance_mode, insurance_amount = excluded.insurance_amount,
             insurance_rate = excluded.insurance_rate, payer_account_id = excluded.payer_account_id,
             interest_category_id = excluded.interest_category_id,
             insurance_category_id = excluded.insurance_category_id, updated_at = excluded.updated_at",
        account,
        principal,
        rate,
        duration,
        t.first_installment,
        mode,
        amount,
        insurance_rate,
        payer,
        interest_category,
        insurance_category,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<()> {
    let account = account.to_string();
    sqlx::query!("DELETE FROM loan_terms WHERE account_id = ?", account).execute(db).await?;
    Ok(())
}

/// Le compte paie-t-il les mensualités d'un prêt ?
pub async fn is_payer(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<bool> {
    let account = account.to_string();
    let count = sqlx::query_scalar!("SELECT count(*) FROM loan_terms WHERE payer_account_id = ?", account)
        .fetch_one(db)
        .await?;
    Ok(count > 0)
}
