//! Salaires : emplois, rémunérations et primes (spec 10, ADR-0022).

use cashflow_domain::documents::MediaType;
use cashflow_domain::salaries::{BonusKind, SalaryChangeReason};
use cashflow_domain::shared::ids::{CategoryId, DocumentId, EmploymentId, PayslipId, SalaryBonusId, SalaryId};
use cashflow_domain::{Date, Money};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use super::documents::media_type_from_db;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct EmploymentRow {
    pub id: EmploymentId,
    pub employer: String,
    pub job_title: Option<String>,
    pub start_date: Date,
    pub end_date: Option<Date>,
    pub income_category_id: CategoryId,
    pub raise_month: u32,
    pub paid_next_month: bool,
    pub notes: Option<String>,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<EmploymentRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, employer, job_title, start_date AS "start_date: Date", end_date AS "end_date: Date",
                  income_category_id, raise_month, paid_next_month AS "paid_next_month: bool", notes
           FROM employment ORDER BY start_date, employer"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(EmploymentRow {
                id: id_from_db(&r.id)?,
                employer: r.employer,
                job_title: r.job_title,
                start_date: r.start_date,
                end_date: r.end_date,
                income_category_id: id_from_db(&r.income_category_id)?,
                raise_month: u32::try_from(r.raise_month)?,
                paid_next_month: r.paid_next_month,
                notes: r.notes,
            })
        })
        .collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: EmploymentId) -> anyhow::Result<Option<EmploymentRow>> {
    Ok(list(db).await?.into_iter().find(|e| e.id == id))
}

pub async fn upsert(db: impl SqliteExecutor<'_>, row: &EmploymentRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let category = row.income_category_id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO employment (id, employer, job_title, start_date, end_date, income_category_id, raise_month,
                                 paid_next_month, notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             employer = excluded.employer, job_title = excluded.job_title, start_date = excluded.start_date,
             end_date = excluded.end_date, income_category_id = excluded.income_category_id,
             raise_month = excluded.raise_month, paid_next_month = excluded.paid_next_month, notes = excluded.notes,
             updated_at = excluded.updated_at",
        id,
        row.employer,
        row.job_title,
        row.start_date,
        row.end_date,
        category,
        row.raise_month,
        row.paid_next_month,
        row.notes,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: EmploymentId) -> anyhow::Result<u64> {
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM employment WHERE id = ?", id).execute(db).await?.rows_affected())
}

pub async fn count(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!: i64" FROM employment"#).fetch_one(db).await?)
}

// ---------------------------------------------------------------- Rémunérations (R-SAL-2)

#[derive(Debug, Clone)]
pub struct SalaryRow {
    pub id: SalaryId,
    pub employment_id: EmploymentId,
    pub effective_month: Date,
    pub annual_gross: Money,
    pub monthly_net: Money,
    pub reason: SalaryChangeReason,
    pub note: Option<String>,
}

pub async fn salaries(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<SalaryRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, employment_id, effective_month AS "effective_month: Date", annual_gross, monthly_net, reason, note
           FROM salary ORDER BY employment_id, effective_month"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(SalaryRow {
                id: id_from_db(&r.id)?,
                employment_id: id_from_db(&r.employment_id)?,
                effective_month: r.effective_month,
                annual_gross: Money::from_minor(r.annual_gross),
                monthly_net: Money::from_minor(r.monthly_net),
                reason: enum_from_db(&r.reason)?,
                note: r.note,
            })
        })
        .collect()
}

pub async fn upsert_salary(db: impl SqliteExecutor<'_>, row: &SalaryRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let employment = row.employment_id.to_string();
    let gross = row.annual_gross.minor();
    let net = row.monthly_net.minor();
    let reason = enum_to_db(&row.reason);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO salary (id, employment_id, effective_month, annual_gross, monthly_net, reason, note, created_at,
                             updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             effective_month = excluded.effective_month, annual_gross = excluded.annual_gross,
             monthly_net = excluded.monthly_net, reason = excluded.reason, note = excluded.note,
             updated_at = excluded.updated_at",
        id,
        employment,
        row.effective_month,
        gross,
        net,
        reason,
        row.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_salary(db: impl SqliteExecutor<'_>, employment: EmploymentId, id: SalaryId) -> anyhow::Result<u64> {
    let employment = employment.to_string();
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM salary WHERE employment_id = ? AND id = ?", employment, id)
        .execute(db)
        .await?
        .rows_affected())
}

// ---------------------------------------------------------------- Primes (R-SAL-5)

#[derive(Debug, Clone)]
pub struct BonusRow {
    pub id: SalaryBonusId,
    pub employment_id: EmploymentId,
    pub label: String,
    pub kind: BonusKind,
    pub paid_on: Date,
    pub net_amount: Money,
    pub gross_amount: Option<Money>,
    pub recurring: bool,
    pub note: Option<String>,
}

pub async fn bonuses(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<BonusRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, employment_id, label, kind, paid_on AS "paid_on: Date", net_amount, gross_amount,
                  recurring AS "recurring: bool", note
           FROM salary_bonus ORDER BY employment_id, paid_on"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(BonusRow {
                id: id_from_db(&r.id)?,
                employment_id: id_from_db(&r.employment_id)?,
                label: r.label,
                kind: enum_from_db(&r.kind)?,
                paid_on: r.paid_on,
                net_amount: Money::from_minor(r.net_amount),
                gross_amount: r.gross_amount.map(Money::from_minor),
                recurring: r.recurring,
                note: r.note,
            })
        })
        .collect()
}

pub async fn upsert_bonus(db: impl SqliteExecutor<'_>, row: &BonusRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let employment = row.employment_id.to_string();
    let kind = enum_to_db(&row.kind);
    let net = row.net_amount.minor();
    let gross = row.gross_amount.map(Money::minor);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO salary_bonus (id, employment_id, label, kind, paid_on, net_amount, gross_amount, recurring, note,
                                   created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             label = excluded.label, kind = excluded.kind, paid_on = excluded.paid_on,
             net_amount = excluded.net_amount, gross_amount = excluded.gross_amount, recurring = excluded.recurring,
             note = excluded.note, updated_at = excluded.updated_at",
        id,
        employment,
        row.label,
        kind,
        row.paid_on,
        net,
        gross,
        row.recurring,
        row.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_bonus(
    db: impl SqliteExecutor<'_>,
    employment: EmploymentId,
    id: SalaryBonusId,
) -> anyhow::Result<u64> {
    let employment = employment.to_string();
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM salary_bonus WHERE employment_id = ? AND id = ?", employment, id)
        .execute(db)
        .await?
        .rows_affected())
}

// ---------------------------------------------------------------- Reçu (R-SAL-9)

/// Total reçu par catégorie (ou sous-catégorie, rapportée à son parent) et par mois `AAAA-MM`.
#[derive(Debug, Clone)]
pub struct ReceivedRow {
    pub category_id: CategoryId,
    pub parent_id: Option<CategoryId>,
    pub month: String,
    pub amount: Money,
}

pub async fn received(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<ReceivedRow>> {
    let rows = sqlx::query!(
        r#"SELECT c.id AS "category_id!", c.parent_id, strftime('%Y-%m', t.date) AS "month!: String",
                  sum(s.amount) AS "amount!: i64"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN category c ON c.id = s.category_id
           WHERE t.date BETWEEN ? AND ? AND c.kind = 'INCOME'
           GROUP BY c.id, strftime('%Y-%m', t.date)"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ReceivedRow {
                category_id: id_from_db(&r.category_id)?,
                parent_id: opt_id_from_db(r.parent_id.as_deref())?,
                month: r.month,
                amount: Money::from_minor(r.amount),
            })
        })
        .collect()
}

// ---------------------------------------------------------------- Fiches de paie (R-SAL-15)

#[derive(Debug, Clone)]
pub struct PayslipRow {
    pub id: PayslipId,
    pub pay_month: Date,
    pub document_id: DocumentId,
    pub note: Option<String>,
    pub file_name: String,
    pub media_type: MediaType,
    pub size_bytes: i64,
    pub sha256: String,
}

/// Fiches de paie d'un emploi, du mois le plus ancien au plus récent.
pub async fn payslips(db: impl SqliteExecutor<'_>, employment: EmploymentId) -> anyhow::Result<Vec<PayslipRow>> {
    let employment = employment.to_string();
    let rows = sqlx::query!(
        r#"SELECT p.id, p.pay_month AS "pay_month: Date", p.document_id, p.note,
                  d.file_name, d.media_type, d.size_bytes, d.sha256
           FROM payslip p JOIN document d ON d.id = p.document_id
           WHERE p.employment_id = ? ORDER BY p.pay_month, p.created_at"#,
        employment
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PayslipRow {
                id: id_from_db(&r.id)?,
                pay_month: r.pay_month,
                document_id: id_from_db(&r.document_id)?,
                note: r.note,
                file_name: r.file_name,
                media_type: media_type_from_db(&r.media_type)?,
                size_bytes: r.size_bytes,
                sha256: r.sha256,
            })
        })
        .collect()
}

pub async fn insert_payslip(
    db: impl SqliteExecutor<'_>,
    id: PayslipId,
    employment: EmploymentId,
    pay_month: Date,
    document: DocumentId,
    note: Option<&str>,
) -> anyhow::Result<()> {
    let (id, employment, document) = (id.to_string(), employment.to_string(), document.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO payslip (id, employment_id, pay_month, document_id, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        id,
        employment,
        pay_month,
        document,
        note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Modifie le mois de paie et la note ; le fichier ne change pas (R-SAL-17).
pub async fn update_payslip(
    db: impl SqliteExecutor<'_>,
    employment: EmploymentId,
    id: PayslipId,
    pay_month: Date,
    note: Option<&str>,
) -> anyhow::Result<u64> {
    let (id, employment) = (id.to_string(), employment.to_string());
    let now = timestamp();
    Ok(sqlx::query!(
        "UPDATE payslip SET pay_month = ?, note = ?, updated_at = ? WHERE id = ? AND employment_id = ?",
        pay_month,
        note,
        now,
        id,
        employment
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn delete_payslip(
    db: impl SqliteExecutor<'_>,
    employment: EmploymentId,
    id: PayslipId,
) -> anyhow::Result<u64> {
    let (id, employment) = (id.to_string(), employment.to_string());
    Ok(sqlx::query!("DELETE FROM payslip WHERE id = ? AND employment_id = ?", id, employment)
        .execute(db)
        .await?
        .rows_affected())
}
