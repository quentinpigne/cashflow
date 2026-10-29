//! Simulations et demandes de crédit (spec 13, ADR-0027) : simulations, demandes, pièces attribuées, démarches.

use cashflow_domain::credit::pieces::Piece;
use cashflow_domain::credit::{ApplicationStatus, LenderEventKind, LenderRequestStatus, ProjectKind, SimulationTerms};
use cashflow_domain::loans::{Insurance, LoanKind};
use cashflow_domain::shared::ids::{
    AssetId, DocumentId, InstitutionId, LenderEventId, LenderRequestId, LoanApplicationId, LoanId, LoanSimulationId,
};
use cashflow_domain::{Date, Money, Rate};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use super::loans::insurance_from_db;
use crate::clock::timestamp;

fn insurance_to_db(insurance: Insurance) -> (&'static str, Option<i64>, Option<i64>) {
    match insurance {
        Insurance::None => ("NONE", None, None),
        Insurance::Fixed { amount_minor } => ("FIXED", Some(amount_minor), None),
        Insurance::RateOnPrincipal { rate_ppm } => ("RATE_ON_PRINCIPAL", None, Some(rate_ppm)),
    }
}

// ---------------------------------------------------------------- Simulations

#[derive(Debug, Clone)]
pub struct SimulationRow {
    pub id: LoanSimulationId,
    pub name: String,
    pub terms: SimulationTerms,
    pub institution_id: Option<InstitutionId>,
    pub application_id: Option<LoanApplicationId>,
    pub loan_id: Option<LoanId>,
    pub note: Option<String>,
}

/// Toutes les simulations, les plus récentes d'abord.
pub async fn simulations(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<SimulationRow>> {
    let rows = sqlx::query!(
        "SELECT id, name, kind, principal, annual_rate, duration_months, insurance_mode, insurance_amount,
                insurance_rate, fees, guarantee_fees, institution_id, application_id, loan_id, note
         FROM loan_simulation ORDER BY created_at DESC, id DESC"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(SimulationRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                terms: SimulationTerms {
                    kind: enum_from_db::<LoanKind>(&r.kind)?,
                    principal: Money::from_minor(r.principal),
                    annual_rate: Rate::from_ppm(r.annual_rate),
                    duration_months: u32::try_from(r.duration_months)?,
                    insurance: insurance_from_db(&r.insurance_mode, r.insurance_amount, r.insurance_rate)?,
                    fees: Money::from_minor(r.fees),
                    guarantee_fees: Money::from_minor(r.guarantee_fees),
                },
                institution_id: opt_id_from_db(r.institution_id.as_deref())?,
                application_id: opt_id_from_db(r.application_id.as_deref())?,
                loan_id: opt_id_from_db(r.loan_id.as_deref())?,
                note: r.note,
            })
        })
        .collect()
}

pub async fn upsert_simulation(db: impl SqliteExecutor<'_>, row: &SimulationRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let kind = enum_to_db(&row.terms.kind);
    let (insurance_mode, insurance_amount, insurance_rate) = insurance_to_db(row.terms.insurance);
    let (principal, rate, fees, guarantee) = (
        row.terms.principal.minor(),
        row.terms.annual_rate.ppm(),
        row.terms.fees.minor(),
        row.terms.guarantee_fees.minor(),
    );
    let duration = i64::from(row.terms.duration_months);
    let institution = row.institution_id.map(|i| i.to_string());
    let application = row.application_id.map(|a| a.to_string());
    let loan = row.loan_id.map(|l| l.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO loan_simulation (id, name, kind, principal, annual_rate, duration_months, insurance_mode,
                                      insurance_amount, insurance_rate, fees, guarantee_fees, institution_id,
                                      application_id, loan_id, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             name = excluded.name, kind = excluded.kind, principal = excluded.principal,
             annual_rate = excluded.annual_rate, duration_months = excluded.duration_months,
             insurance_mode = excluded.insurance_mode, insurance_amount = excluded.insurance_amount,
             insurance_rate = excluded.insurance_rate, fees = excluded.fees, guarantee_fees = excluded.guarantee_fees,
             institution_id = excluded.institution_id, application_id = excluded.application_id,
             loan_id = excluded.loan_id, note = excluded.note, updated_at = excluded.updated_at",
        id,
        row.name,
        kind,
        principal,
        rate,
        duration,
        insurance_mode,
        insurance_amount,
        insurance_rate,
        fees,
        guarantee,
        institution,
        application,
        loan,
        row.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_simulation(db: impl SqliteExecutor<'_>, id: LoanSimulationId) -> anyhow::Result<u64> {
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM loan_simulation WHERE id = ?", id).execute(db).await?.rows_affected())
}

/// Retire la simulation retenue des demandes qui l'avaient retenue (R-CRE-23).
pub async fn unselect_simulation(db: impl SqliteExecutor<'_>, id: LoanSimulationId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("UPDATE loan_application SET selected_simulation_id = NULL WHERE selected_simulation_id = ?", id)
        .execute(db)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- Demandes

#[derive(Debug, Clone)]
pub struct ApplicationRow {
    pub id: LoanApplicationId,
    pub name: String,
    pub project_kind: ProjectKind,
    pub status: ApplicationStatus,
    pub asset_id: Option<AssetId>,
    pub price: Money,
    pub notary_fees: Money,
    pub works: Money,
    pub other_costs: Money,
    pub down_payment: Money,
    pub other_income: Money,
    pub other_charges: Money,
    pub co_borrower_name: Option<String>,
    pub co_borrower_income: Money,
    pub co_borrower_charges: Money,
    pub excluded_loan_ids: Vec<LoanId>,
    pub selected_simulation_id: Option<LoanSimulationId>,
    pub note: Option<String>,
}

macro_rules! application_row {
    ($r:expr) => {{
        let r = $r;
        let excluded: Vec<String> = serde_json::from_str(&r.excluded_loan_ids)?;
        Ok::<_, anyhow::Error>(ApplicationRow {
            id: id_from_db(&r.id)?,
            name: r.name,
            project_kind: enum_from_db(&r.project_kind)?,
            status: enum_from_db(&r.status)?,
            asset_id: opt_id_from_db(r.asset_id.as_deref())?,
            price: Money::from_minor(r.price),
            notary_fees: Money::from_minor(r.notary_fees),
            works: Money::from_minor(r.works),
            other_costs: Money::from_minor(r.other_costs),
            down_payment: Money::from_minor(r.down_payment),
            other_income: Money::from_minor(r.other_income),
            other_charges: Money::from_minor(r.other_charges),
            co_borrower_name: r.co_borrower_name,
            co_borrower_income: Money::from_minor(r.co_borrower_income),
            co_borrower_charges: Money::from_minor(r.co_borrower_charges),
            excluded_loan_ids: excluded.iter().map(|id| id_from_db(id)).collect::<anyhow::Result<_>>()?,
            selected_simulation_id: opt_id_from_db(r.selected_simulation_id.as_deref())?,
            note: r.note,
        })
    }};
}

/// Toutes les demandes, les plus récentes d'abord.
pub async fn applications(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ApplicationRow>> {
    let rows = sqlx::query!(
        "SELECT id, name, project_kind, status, asset_id, price, notary_fees, works, other_costs, down_payment,
                other_income, other_charges, co_borrower_name, co_borrower_income, co_borrower_charges,
                excluded_loan_ids, selected_simulation_id, note
         FROM loan_application ORDER BY created_at DESC, id DESC"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| application_row!(r)).collect()
}

pub async fn find_application(
    db: impl SqliteExecutor<'_>,
    id: LoanApplicationId,
) -> anyhow::Result<Option<ApplicationRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        "SELECT id, name, project_kind, status, asset_id, price, notary_fees, works, other_costs, down_payment,
                other_income, other_charges, co_borrower_name, co_borrower_income, co_borrower_charges,
                excluded_loan_ids, selected_simulation_id, note
         FROM loan_application WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| application_row!(r)).transpose()
}

pub async fn upsert_application(db: impl SqliteExecutor<'_>, row: &ApplicationRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let (project_kind, status) = (enum_to_db(&row.project_kind), enum_to_db(&row.status));
    let asset = row.asset_id.map(|a| a.to_string());
    let excluded = serde_json::to_string(&row.excluded_loan_ids.iter().map(ToString::to_string).collect::<Vec<_>>())?;
    let selected = row.selected_simulation_id.map(|s| s.to_string());
    let (price, notary, works, other_costs, down) = (
        row.price.minor(),
        row.notary_fees.minor(),
        row.works.minor(),
        row.other_costs.minor(),
        row.down_payment.minor(),
    );
    let (other_income, other_charges, co_income, co_charges) = (
        row.other_income.minor(),
        row.other_charges.minor(),
        row.co_borrower_income.minor(),
        row.co_borrower_charges.minor(),
    );
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO loan_application (id, name, project_kind, status, asset_id, price, notary_fees, works,
                                       other_costs, down_payment, other_income, other_charges, co_borrower_name,
                                       co_borrower_income, co_borrower_charges, excluded_loan_ids,
                                       selected_simulation_id, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             name = excluded.name, project_kind = excluded.project_kind, status = excluded.status,
             asset_id = excluded.asset_id, price = excluded.price, notary_fees = excluded.notary_fees,
             works = excluded.works, other_costs = excluded.other_costs, down_payment = excluded.down_payment,
             other_income = excluded.other_income, other_charges = excluded.other_charges,
             co_borrower_name = excluded.co_borrower_name, co_borrower_income = excluded.co_borrower_income,
             co_borrower_charges = excluded.co_borrower_charges, excluded_loan_ids = excluded.excluded_loan_ids,
             selected_simulation_id = excluded.selected_simulation_id, note = excluded.note,
             updated_at = excluded.updated_at",
        id,
        row.name,
        project_kind,
        status,
        asset,
        price,
        notary,
        works,
        other_costs,
        down,
        other_income,
        other_charges,
        row.co_borrower_name,
        co_income,
        co_charges,
        excluded,
        selected,
        row.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_application_status(
    db: impl SqliteExecutor<'_>,
    id: LoanApplicationId,
    status: ApplicationStatus,
) -> anyhow::Result<()> {
    let (id, status) = (id.to_string(), enum_to_db(&status));
    let now = timestamp();
    sqlx::query!("UPDATE loan_application SET status = ?, updated_at = ? WHERE id = ?", status, now, id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_application(db: impl SqliteExecutor<'_>, id: LoanApplicationId) -> anyhow::Result<u64> {
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM loan_application WHERE id = ?", id).execute(db).await?.rows_affected())
}

// ---------------------------------------------------------------- Pièces attribuées (R-CRE-18)

#[derive(Debug, Clone)]
pub struct PieceRow {
    pub piece: Piece,
    pub document_id: Option<DocumentId>,
    pub not_required: bool,
}

pub async fn pieces(db: impl SqliteExecutor<'_>, application: LoanApplicationId) -> anyhow::Result<Vec<PieceRow>> {
    let application = application.to_string();
    let rows = sqlx::query!(
        r#"SELECT piece, document_id, not_required AS "not_required: bool"
           FROM loan_application_piece WHERE application_id = ? ORDER BY rowid"#,
        application
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PieceRow {
                piece: Piece::parse(&r.piece).ok_or_else(|| anyhow::anyhow!("pièce inconnue en base : {}", r.piece))?,
                document_id: opt_id_from_db(r.document_id.as_deref())?,
                not_required: r.not_required,
            })
        })
        .collect()
}

pub async fn assign_piece(
    db: impl SqliteExecutor<'_>,
    application: LoanApplicationId,
    piece: Piece,
    document: DocumentId,
) -> anyhow::Result<()> {
    let (application, piece, document) = (application.to_string(), piece.code(), document.to_string());
    sqlx::query!(
        "INSERT OR IGNORE INTO loan_application_piece (application_id, piece, document_id, not_required)
         VALUES (?, ?, ?, 0)",
        application,
        piece,
        document
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn unassign_piece(
    db: impl SqliteExecutor<'_>,
    application: LoanApplicationId,
    piece: Piece,
    document: DocumentId,
) -> anyhow::Result<u64> {
    let (application, piece, document) = (application.to_string(), piece.code(), document.to_string());
    Ok(sqlx::query!(
        "DELETE FROM loan_application_piece WHERE application_id = ? AND piece = ? AND document_id = ?",
        application,
        piece,
        document
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn set_not_required(
    db: impl SqliteExecutor<'_> + Copy,
    application: LoanApplicationId,
    piece: Piece,
    not_required: bool,
) -> anyhow::Result<()> {
    let (application, piece) = (application.to_string(), piece.code());
    if not_required {
        sqlx::query!(
            "INSERT OR IGNORE INTO loan_application_piece (application_id, piece, document_id, not_required)
             VALUES (?, ?, NULL, 1)",
            application,
            piece
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            "DELETE FROM loan_application_piece WHERE application_id = ? AND piece = ? AND not_required = 1",
            application,
            piece
        )
        .execute(db)
        .await?;
    }
    Ok(())
}

// ---------------------------------------------------------------- Démarches (R-CRE-21)

#[derive(Debug, Clone)]
pub struct LenderRequestRow {
    pub id: LenderRequestId,
    pub application_id: LoanApplicationId,
    pub institution_id: Option<InstitutionId>,
    pub lender_name: Option<String>,
    pub status: LenderRequestStatus,
    pub sent_on: Date,
    pub answered_on: Option<Date>,
    pub simulation_id: Option<LoanSimulationId>,
    pub follow_up_on: Option<Date>,
    pub note: Option<String>,
}

/// Démarches d'une demande, dans l'ordre d'envoi.
pub async fn lender_requests(
    db: impl SqliteExecutor<'_>,
    application: LoanApplicationId,
) -> anyhow::Result<Vec<LenderRequestRow>> {
    let application = application.to_string();
    let rows = sqlx::query!(
        r#"SELECT id, application_id, institution_id, lender_name, status, sent_on AS "sent_on: Date",
                  answered_on AS "answered_on: Date", simulation_id, follow_up_on AS "follow_up_on: Date", note
           FROM lender_request WHERE application_id = ? ORDER BY sent_on, created_at"#,
        application
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(LenderRequestRow {
                id: id_from_db(&r.id)?,
                application_id: id_from_db(&r.application_id)?,
                institution_id: opt_id_from_db(r.institution_id.as_deref())?,
                lender_name: r.lender_name,
                status: enum_from_db(&r.status)?,
                sent_on: r.sent_on,
                answered_on: r.answered_on,
                simulation_id: opt_id_from_db(r.simulation_id.as_deref())?,
                follow_up_on: r.follow_up_on,
                note: r.note,
            })
        })
        .collect()
}

/// Démarches à relancer de toutes les demandes : celles qui attendent une réponse et ont une relance (R-CRE-26).
pub async fn follow_ups(
    db: impl SqliteExecutor<'_>,
) -> anyhow::Result<Vec<(LenderRequestId, LoanApplicationId, String, LenderRequestStatus, Date)>> {
    let rows = sqlx::query!(
        r#"SELECT r.id, r.application_id, coalesce(i.name, r.lender_name, '') AS "name!: String", r.status,
                  r.follow_up_on AS "follow_up_on!: Date"
           FROM lender_request r LEFT JOIN institution i ON i.id = r.institution_id
           WHERE r.follow_up_on IS NOT NULL"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok((id_from_db(&r.id)?, id_from_db(&r.application_id)?, r.name, enum_from_db(&r.status)?, r.follow_up_on))
        })
        .collect()
}

pub async fn upsert_lender_request(db: impl SqliteExecutor<'_>, row: &LenderRequestRow) -> anyhow::Result<()> {
    let (id, application) = (row.id.to_string(), row.application_id.to_string());
    let institution = row.institution_id.map(|i| i.to_string());
    let status = enum_to_db(&row.status);
    let simulation = row.simulation_id.map(|s| s.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO lender_request (id, application_id, institution_id, lender_name, status, sent_on, answered_on,
                                     simulation_id, follow_up_on, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             institution_id = excluded.institution_id, lender_name = excluded.lender_name, status = excluded.status,
             sent_on = excluded.sent_on, answered_on = excluded.answered_on, simulation_id = excluded.simulation_id,
             follow_up_on = excluded.follow_up_on, note = excluded.note, updated_at = excluded.updated_at",
        id,
        application,
        institution,
        row.lender_name,
        status,
        row.sent_on,
        row.answered_on,
        simulation,
        row.follow_up_on,
        row.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_lender_request(
    db: impl SqliteExecutor<'_>,
    application: LoanApplicationId,
    id: LenderRequestId,
) -> anyhow::Result<u64> {
    let (id, application) = (id.to_string(), application.to_string());
    Ok(sqlx::query!("DELETE FROM lender_request WHERE id = ? AND application_id = ?", id, application)
        .execute(db)
        .await?
        .rows_affected())
}

// ---------------------------------------------------------------- Journal des démarches (R-CRE-24)

#[derive(Debug, Clone)]
pub struct EventRow {
    pub id: LenderEventId,
    pub lender_request_id: LenderRequestId,
    pub date: Date,
    pub kind: LenderEventKind,
    pub note: Option<String>,
}

/// Journal des démarches d'une demande, du plus récent au plus ancien.
pub async fn events(db: impl SqliteExecutor<'_>, application: LoanApplicationId) -> anyhow::Result<Vec<EventRow>> {
    let application = application.to_string();
    let rows = sqlx::query!(
        r#"SELECT e.id, e.lender_request_id, e.date AS "date: Date", e.kind, e.note
           FROM lender_request_event e JOIN lender_request r ON r.id = e.lender_request_id
           WHERE r.application_id = ? ORDER BY e.date DESC, e.created_at DESC"#,
        application
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(EventRow {
                id: id_from_db(&r.id)?,
                lender_request_id: id_from_db(&r.lender_request_id)?,
                date: r.date,
                kind: enum_from_db(&r.kind)?,
                note: r.note,
            })
        })
        .collect()
}

pub async fn upsert_event(db: impl SqliteExecutor<'_>, row: &EventRow) -> anyhow::Result<()> {
    let (id, request, kind) = (row.id.to_string(), row.lender_request_id.to_string(), enum_to_db(&row.kind));
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO lender_request_event (id, lender_request_id, date, kind, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET date = excluded.date, kind = excluded.kind, note = excluded.note,
                                        updated_at = excluded.updated_at",
        id,
        request,
        row.date,
        kind,
        row.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_event(
    db: impl SqliteExecutor<'_>,
    request: LenderRequestId,
    id: LenderEventId,
) -> anyhow::Result<u64> {
    let (id, request) = (id.to_string(), request.to_string());
    Ok(sqlx::query!("DELETE FROM lender_request_event WHERE id = ? AND lender_request_id = ?", id, request)
        .execute(db)
        .await?
        .rows_affected())
}
