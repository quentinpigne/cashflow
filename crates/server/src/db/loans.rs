//! Prêts : fiche, conditions, capitaux restants dus saisis, échéances payées (spec 09, ADR-0019).

use cashflow_domain::loans::{Insurance, LoanKind, LoanMode, LoanTerms};
use cashflow_domain::shared::ids::{
    AccountId, AssetId, CategoryId, InstitutionId, LoanBalanceId, LoanId, LoanPaymentId, TransactionId,
};
use cashflow_domain::{Date, Money, Rate};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

/// Conditions d'un prêt avec tableau (R-PRE-2).
#[derive(Debug, Clone)]
pub struct ScheduleRow {
    pub terms: LoanTerms,
    pub payer_account_id: AccountId,
    pub interest_category_id: CategoryId,
    pub insurance_category_id: Option<CategoryId>,
}

#[derive(Debug, Clone)]
pub struct LoanRow {
    pub id: LoanId,
    pub name: String,
    pub kind: LoanKind,
    pub institution_id: Option<InstitutionId>,
    pub start_date: Date,
    pub principal: Money,
    /// `Some` avec tableau, `None` en suivi simple.
    pub schedule: Option<ScheduleRow>,
    pub asset_id: Option<AssetId>,
    pub closed_on: Option<Date>,
    pub include_in_net_worth: bool,
    pub notes: Option<String>,
}

impl LoanRow {
    pub fn mode(&self) -> LoanMode {
        if self.schedule.is_some() { LoanMode::Schedule } else { LoanMode::Manual }
    }
}

fn insurance_from_db(mode: &str, amount: Option<i64>, rate: Option<i64>) -> anyhow::Result<Insurance> {
    Ok(match (mode, amount, rate) {
        ("NONE", _, _) => Insurance::None,
        ("FIXED", Some(amount_minor), _) => Insurance::Fixed { amount_minor },
        ("RATE_ON_PRINCIPAL", _, Some(rate_ppm)) => Insurance::RateOnPrincipal { rate_ppm },
        _ => anyhow::bail!("assurance de prêt incohérente en base : {mode}"),
    })
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<LoanRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, kind, institution_id, start_date AS "start_date: Date", principal, mode, annual_rate,
                  duration_months, first_installment_date AS "first_installment_date: Date", insurance_mode,
                  insurance_amount, insurance_rate, payer_account_id, interest_category_id, insurance_category_id,
                  asset_id, closed_on AS "closed_on: Date", include_in_net_worth AS "include_in_net_worth: bool",
                  notes
           FROM loan ORDER BY start_date, name"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            let principal = Money::from_minor(r.principal);
            let schedule = match (r.mode.as_str(), r.annual_rate, r.duration_months, r.first_installment_date) {
                ("SCHEDULE", Some(rate), Some(duration), Some(first)) => Some(ScheduleRow {
                    terms: LoanTerms {
                        principal,
                        annual_rate: Rate::from_ppm(rate),
                        duration_months: u32::try_from(duration)?,
                        first_installment: first,
                        insurance: insurance_from_db(&r.insurance_mode, r.insurance_amount, r.insurance_rate)?,
                    },
                    payer_account_id: id_from_db(r.payer_account_id.as_deref().unwrap_or_default())?,
                    interest_category_id: id_from_db(r.interest_category_id.as_deref().unwrap_or_default())?,
                    insurance_category_id: opt_id_from_db(r.insurance_category_id.as_deref())?,
                }),
                ("MANUAL", ..) => None,
                _ => anyhow::bail!("prêt incohérent en base : {}", r.id),
            };
            Ok(LoanRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                kind: enum_from_db(&r.kind)?,
                institution_id: opt_id_from_db(r.institution_id.as_deref())?,
                start_date: r.start_date,
                principal,
                schedule,
                asset_id: opt_id_from_db(r.asset_id.as_deref())?,
                closed_on: r.closed_on,
                include_in_net_worth: r.include_in_net_worth,
                notes: r.notes,
            })
        })
        .collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: LoanId) -> anyhow::Result<Option<LoanRow>> {
    Ok(list(db).await?.into_iter().find(|l| l.id == id))
}

/// Crée ou remplace un prêt.
pub async fn upsert(db: impl SqliteExecutor<'_>, row: &LoanRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let kind = enum_to_db(&row.kind);
    let institution = row.institution_id.map(|i| i.to_string());
    let principal = row.principal.minor();
    let mode = enum_to_db(&row.mode());
    let s = row.schedule.as_ref();
    let rate = s.map(|s| s.terms.annual_rate.ppm());
    let duration = s.map(|s| i64::from(s.terms.duration_months));
    let first = s.map(|s| s.terms.first_installment);
    let (insurance_mode, insurance_amount, insurance_rate) = match s.map(|s| s.terms.insurance) {
        None | Some(Insurance::None) => ("NONE", None, None),
        Some(Insurance::Fixed { amount_minor }) => ("FIXED", Some(amount_minor), None),
        Some(Insurance::RateOnPrincipal { rate_ppm }) => ("RATE_ON_PRINCIPAL", None, Some(rate_ppm)),
    };
    let payer = s.map(|s| s.payer_account_id.to_string());
    let interest_category = s.map(|s| s.interest_category_id.to_string());
    let insurance_category = s.and_then(|s| s.insurance_category_id).map(|c| c.to_string());
    let asset = row.asset_id.map(|a| a.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO loan (id, name, kind, institution_id, start_date, principal, mode, annual_rate, duration_months,
                           first_installment_date, insurance_mode, insurance_amount, insurance_rate, payer_account_id,
                           interest_category_id, insurance_category_id, asset_id, closed_on, include_in_net_worth,
                           notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             name = excluded.name, kind = excluded.kind, institution_id = excluded.institution_id,
             start_date = excluded.start_date, principal = excluded.principal, mode = excluded.mode,
             annual_rate = excluded.annual_rate, duration_months = excluded.duration_months,
             first_installment_date = excluded.first_installment_date, insurance_mode = excluded.insurance_mode,
             insurance_amount = excluded.insurance_amount, insurance_rate = excluded.insurance_rate,
             payer_account_id = excluded.payer_account_id, interest_category_id = excluded.interest_category_id,
             insurance_category_id = excluded.insurance_category_id, asset_id = excluded.asset_id,
             closed_on = excluded.closed_on, include_in_net_worth = excluded.include_in_net_worth,
             notes = excluded.notes, updated_at = excluded.updated_at",
        id,
        row.name,
        kind,
        institution,
        row.start_date,
        principal,
        mode,
        rate,
        duration,
        first,
        insurance_mode,
        insurance_amount,
        insurance_rate,
        payer,
        interest_category,
        insurance_category,
        asset,
        row.closed_on,
        row.include_in_net_worth,
        row.notes,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: LoanId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM loan WHERE id = ?", id).execute(db).await?;
    Ok(())
}

pub async fn count(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!: i64" FROM loan"#).fetch_one(db).await?)
}

/// Le compte paie-t-il les mensualités d'un prêt ?
pub async fn is_payer(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<bool> {
    let account = account.to_string();
    let count =
        sqlx::query_scalar!("SELECT count(*) FROM loan WHERE payer_account_id = ?", account).fetch_one(db).await?;
    Ok(count > 0)
}

// ---------------------------------------------------------------- Capitaux restants dus saisis (R-PRE-5)

#[derive(Debug, Clone)]
pub struct BalanceRow {
    pub loan_id: LoanId,
    pub date: Date,
    pub outstanding: Money,
    pub note: Option<String>,
}

pub async fn balances(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<BalanceRow>> {
    let rows = sqlx::query!(
        r#"SELECT loan_id, date AS "date: Date", outstanding, note FROM loan_balance ORDER BY loan_id, date"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(BalanceRow {
                loan_id: id_from_db(&r.loan_id)?,
                date: r.date,
                outstanding: Money::from_minor(r.outstanding),
                note: r.note,
            })
        })
        .collect()
}

pub async fn upsert_balance(db: impl SqliteExecutor<'_>, b: &BalanceRow) -> anyhow::Result<()> {
    let id = LoanBalanceId::new().to_string();
    let loan = b.loan_id.to_string();
    let outstanding = b.outstanding.minor();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO loan_balance (id, loan_id, date, outstanding, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (loan_id, date) DO UPDATE SET outstanding = excluded.outstanding, note = excluded.note,
                                                   updated_at = excluded.updated_at",
        id,
        loan,
        b.date,
        outstanding,
        b.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_balance(db: impl SqliteExecutor<'_>, loan: LoanId, date: Date) -> anyhow::Result<u64> {
    let loan = loan.to_string();
    Ok(sqlx::query!("DELETE FROM loan_balance WHERE loan_id = ? AND date = ?", loan, date)
        .execute(db)
        .await?
        .rows_affected())
}

// ---------------------------------------------------------------- Échéances payées (R-PRE-6)

#[derive(Debug, Clone, Copy)]
pub struct PaymentRow {
    pub loan_id: LoanId,
    pub installment_number: u32,
    pub transaction_id: TransactionId,
}

pub async fn payments(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<PaymentRow>> {
    let rows = sqlx::query!("SELECT loan_id, installment_number, transaction_id FROM loan_payment ORDER BY loan_id")
        .fetch_all(db)
        .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PaymentRow {
                loan_id: id_from_db(&r.loan_id)?,
                installment_number: u32::try_from(r.installment_number)?,
                transaction_id: id_from_db(&r.transaction_id)?,
            })
        })
        .collect()
}

pub async fn payment_of(db: impl SqliteExecutor<'_>, transaction: TransactionId) -> anyhow::Result<Option<PaymentRow>> {
    let transaction = transaction.to_string();
    let row = sqlx::query!(
        "SELECT loan_id, installment_number, transaction_id FROM loan_payment WHERE transaction_id = ?",
        transaction
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| {
        Ok(PaymentRow {
            loan_id: id_from_db(&r.loan_id)?,
            installment_number: u32::try_from(r.installment_number)?,
            transaction_id: id_from_db(&r.transaction_id)?,
        })
    })
    .transpose()
}

pub async fn insert_payment(
    db: impl SqliteExecutor<'_>,
    loan: LoanId,
    number: u32,
    transaction: TransactionId,
) -> anyhow::Result<()> {
    let id = LoanPaymentId::new().to_string();
    let loan = loan.to_string();
    let number = i64::from(number);
    let transaction = transaction.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO loan_payment (id, loan_id, installment_number, transaction_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        id,
        loan,
        number,
        transaction,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_payment(db: impl SqliteExecutor<'_>, transaction: TransactionId) -> anyhow::Result<()> {
    let transaction = transaction.to_string();
    sqlx::query!("DELETE FROM loan_payment WHERE transaction_id = ?", transaction).execute(db).await?;
    Ok(())
}

/// Débit du compte payeur qui peut payer une échéance : sortie, hors virement, pas déjà rattaché à une échéance.
#[derive(Debug, Clone)]
pub struct DebitRow {
    pub id: TransactionId,
    pub date: Date,
    pub amount: Money,
    pub label: String,
}

pub async fn payable_debits(
    db: impl SqliteExecutor<'_>,
    payer: AccountId,
    from: Date,
    to: Date,
) -> anyhow::Result<Vec<DebitRow>> {
    let payer = payer.to_string();
    let rows = sqlx::query!(
        r#"SELECT t.id AS "id!", t.date AS "date!: Date", t.amount AS "amount!: i64", t.label AS "label!"
           FROM ledger_transaction t
           WHERE t.account_id = ? AND t.amount < 0 AND t.date BETWEEN ? AND ?
             AND NOT EXISTS (SELECT 1 FROM transfer_leg tl WHERE t.id IN (tl.outflow_transaction_id, tl.inflow_transaction_id))
             AND NOT EXISTS (SELECT 1 FROM loan_payment lp WHERE lp.transaction_id = t.id)
           ORDER BY t.date, t.id"#,
        payer,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(DebitRow { id: id_from_db(&r.id)?, date: r.date, amount: Money::from_minor(r.amount), label: r.label })
        })
        .collect()
}
