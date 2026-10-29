//! Simulations et demandes de crédit (spec 13, ADR-0027) : simulations et calculs, demandes, capacité d'emprunt,
//! pièces à fournir, démarches et export du dossier. Les calculs sont dans `cashflow_domain::credit`.

use std::collections::{HashMap, HashSet};
use std::io::Write;

use anyhow::Context;
use cashflow_domain::accounts::{AccountFamily, AccountKind};
use cashflow_domain::credit::pieces::{self, LibraryDocument, Piece, PieceContext, PieceStatus};
use cashflow_domain::credit::{
    self, ApplicationStatus, CapacityInput, FinancingPlan, LenderEventKind, LenderRequestStatus, ProjectKind,
    SimulationTerms,
};
use cashflow_domain::documents as docs;
use cashflow_domain::loans::{self, Insurance, LoanKind};
use cashflow_domain::shared::ids::{
    AccountId, AssetId, DocumentId, InstitutionId, LenderEventId, LenderRequestId, LoanApplicationId, LoanId,
    LoanSimulationId,
};
use cashflow_domain::{Date, DomainError, Money, Rate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::loans::{InstallmentView, LoanInput};
use super::salaries::SalaryBook;
use super::{clean, required};
use crate::config::DataDir;
use crate::db::credit::{ApplicationRow, EventRow, LenderRequestRow, SimulationRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ======================================================================= Simulations (CRE-01 à CRE-03)

/// Résultat d'une simulation (R-CRE-2 à R-CRE-4).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SimulationResult {
    /// Mensualité hors assurance.
    pub payment_minor: i64,
    pub insurance_minor: i64,
    /// Échéance prélevée : mensualité + assurance.
    pub total_payment_minor: i64,
    pub total_interest_minor: i64,
    pub total_insurance_minor: i64,
    /// Intérêts + assurance + frais (R-CRE-3).
    pub total_cost_minor: i64,
    /// TAEG en millionièmes (R-CRE-4).
    pub apr_ppm: i64,
}

fn result(simulation: &credit::Simulation) -> SimulationResult {
    SimulationResult {
        payment_minor: simulation.payment.minor(),
        insurance_minor: simulation.insurance.minor(),
        total_payment_minor: simulation.total_payment().minor(),
        total_interest_minor: simulation.total_interest.minor(),
        total_insurance_minor: simulation.total_insurance.minor(),
        total_cost_minor: simulation.total_cost.minor(),
        apr_ppm: simulation.apr.ppm(),
    }
}

/// Ce que cherche le simulateur (R-CRE-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SimulationGoal {
    /// La mensualité, à partir du capital et de la durée.
    Payment,
    /// Le capital empruntable, à partir de la mensualité visée et de la durée.
    Principal,
    /// La durée, à partir du capital et de la mensualité visée.
    Duration,
}

/// Calcul du simulateur, sans rien enregistrer (E36).
#[derive(Debug, Deserialize, ToSchema)]
pub struct ComputeInput {
    pub goal: SimulationGoal,
    pub kind: LoanKind,
    /// Requis sauf pour chercher le capital.
    pub principal_minor: Option<i64>,
    /// Requise sauf pour chercher la durée.
    pub duration_months: Option<u32>,
    /// Mensualité visée, assurance comprise : requise pour chercher le capital ou la durée.
    pub target_payment_minor: Option<i64>,
    pub annual_rate_ppm: i64,
    pub insurance: Insurance,
    #[serde(default)]
    pub fees_minor: i64,
    #[serde(default)]
    pub guarantee_fees_minor: i64,
}

/// Résultat du simulateur : le capital et la durée retenus, le coût et le tableau.
#[derive(Debug, Serialize, ToSchema)]
pub struct ComputeResult {
    pub principal_minor: i64,
    pub duration_months: u32,
    pub result: SimulationResult,
    pub installments: Vec<InstallmentView>,
}

fn installments(simulation: &credit::Simulation) -> Vec<InstallmentView> {
    simulation
        .schedule
        .iter()
        .map(|i| InstallmentView {
            number: i.number,
            date: i.date,
            payment_minor: i.payment.minor(),
            principal_minor: i.principal.minor(),
            interest_minor: i.interest.minor(),
            insurance_minor: i.insurance.minor(),
            total_minor: i.total().minor(),
            remaining_minor: i.remaining.minor(),
            paid_transaction_id: None,
        })
        .collect()
}

fn missing(field: &'static str) -> AppError {
    AppError::invalid_field("REQUIRED", field, "ce champ est obligatoire pour ce calcul")
}

/// Calcule une simulation, ou cherche son capital ou sa durée (R-CRE-2 à R-CRE-5).
pub fn compute(input: &ComputeInput, today: Date) -> AppResult<ComputeResult> {
    let rate = Rate::from_ppm(input.annual_rate_ppm);
    let target = || input.target_payment_minor.map(Money::from_minor).ok_or(missing("target_payment_minor"));
    let (principal, duration_months) = match input.goal {
        SimulationGoal::Payment => (
            Money::from_minor(input.principal_minor.ok_or(missing("principal_minor"))?),
            input.duration_months.ok_or(missing("duration_months"))?,
        ),
        SimulationGoal::Principal => {
            let months = input.duration_months.ok_or(missing("duration_months"))?;
            (credit::borrowing_capacity(target()?, rate, months, input.insurance)?, months)
        }
        SimulationGoal::Duration => {
            let principal = Money::from_minor(input.principal_minor.ok_or(missing("principal_minor"))?);
            (principal, credit::shortest_duration(principal, rate, input.insurance, target()?)?)
        }
    };
    let terms = SimulationTerms {
        kind: input.kind,
        principal,
        annual_rate: rate,
        duration_months,
        insurance: input.insurance,
        fees: Money::from_minor(input.fees_minor),
        guarantee_fees: Money::from_minor(input.guarantee_fees_minor),
    };
    let simulation = credit::simulate(&terms, today)?;
    Ok(ComputeResult {
        principal_minor: principal.minor(),
        duration_months,
        result: result(&simulation),
        installments: installments(&simulation),
    })
}

/// Simulation enregistrée (R-CRE-1).
#[derive(Debug, Deserialize, ToSchema)]
pub struct SimulationInput {
    pub name: String,
    pub kind: LoanKind,
    pub principal_minor: i64,
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub insurance: Insurance,
    #[serde(default)]
    pub fees_minor: i64,
    #[serde(default)]
    pub guarantee_fees_minor: i64,
    pub institution_id: Option<InstitutionId>,
    pub application_id: Option<LoanApplicationId>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SimulationView {
    pub id: LoanSimulationId,
    pub name: String,
    pub kind: LoanKind,
    pub principal_minor: i64,
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub insurance: Insurance,
    pub fees_minor: i64,
    pub guarantee_fees_minor: i64,
    pub institution_id: Option<InstitutionId>,
    pub institution_name: Option<String>,
    pub application_id: Option<LoanApplicationId>,
    pub application_name: Option<String>,
    /// Prêt créé à partir de la simulation (R-CRE-7).
    pub loan_id: Option<LoanId>,
    pub note: Option<String>,
    pub result: SimulationResult,
}

/// Noms des établissements et des demandes, pour les vues.
struct Names {
    institutions: HashMap<InstitutionId, String>,
    applications: HashMap<LoanApplicationId, String>,
}

async fn names(db: &Db) -> AppResult<Names> {
    Ok(Names {
        institutions: db::institutions::list(&db.read).await?.into_iter().map(|i| (i.id, i.name)).collect(),
        applications: db::credit::applications(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect(),
    })
}

fn simulation_view(row: SimulationRow, names: &Names, today: Date) -> AppResult<SimulationView> {
    let simulation = credit::simulate(&row.terms, today)?;
    Ok(SimulationView {
        id: row.id,
        name: row.name,
        kind: row.terms.kind,
        principal_minor: row.terms.principal.minor(),
        annual_rate_ppm: row.terms.annual_rate.ppm(),
        duration_months: row.terms.duration_months,
        insurance: row.terms.insurance,
        fees_minor: row.terms.fees.minor(),
        guarantee_fees_minor: row.terms.guarantee_fees.minor(),
        institution_name: row.institution_id.and_then(|i| names.institutions.get(&i).cloned()),
        institution_id: row.institution_id,
        application_name: row.application_id.and_then(|a| names.applications.get(&a).cloned()),
        application_id: row.application_id,
        loan_id: row.loan_id,
        note: row.note,
        result: result(&simulation),
    })
}

pub async fn simulations(db: &Db, today: Date) -> AppResult<Vec<SimulationView>> {
    let names = names(db).await?;
    db::credit::simulations(&db.read).await?.into_iter().map(|r| simulation_view(r, &names, today)).collect()
}

async fn simulation_row(db: &Db, id: LoanSimulationId) -> AppResult<SimulationRow> {
    db::credit::simulations(&db.read).await?.into_iter().find(|s| s.id == id).ok_or(AppError::NotFound("simulation"))
}

pub async fn simulation(db: &Db, id: LoanSimulationId, today: Date) -> AppResult<SimulationView> {
    simulation_view(simulation_row(db, id).await?, &names(db).await?, today)
}

/// Tableau d'amortissement d'une simulation (R-CRE-2).
pub async fn simulation_schedule(db: &Db, id: LoanSimulationId, today: Date) -> AppResult<Vec<InstallmentView>> {
    let row = simulation_row(db, id).await?;
    Ok(installments(&credit::simulate(&row.terms, today)?))
}

async fn check_references(
    db: &Db,
    institution: Option<InstitutionId>,
    application: Option<LoanApplicationId>,
) -> AppResult<()> {
    if let Some(institution) = institution
        && !db::institutions::list(&db.read).await?.iter().any(|i| i.id == institution)
    {
        return Err(AppError::NotFound("établissement"));
    }
    if let Some(application) = application {
        db::credit::find_application(&db.read, application).await?.ok_or(AppError::NotFound("demande de crédit"))?;
    }
    Ok(())
}

fn simulation_row_of(id: LoanSimulationId, input: &SimulationInput, loan: Option<LoanId>) -> AppResult<SimulationRow> {
    Ok(SimulationRow {
        id,
        name: required(&input.name, "name", 80)?,
        terms: SimulationTerms {
            kind: input.kind,
            principal: Money::from_minor(input.principal_minor),
            annual_rate: Rate::from_ppm(input.annual_rate_ppm),
            duration_months: input.duration_months,
            insurance: input.insurance,
            fees: Money::from_minor(input.fees_minor),
            guarantee_fees: Money::from_minor(input.guarantee_fees_minor),
        },
        institution_id: input.institution_id,
        application_id: input.application_id,
        loan_id: loan,
        note: clean(input.note.as_deref()),
    })
}

/// Enregistre une simulation, nouvelle (`id` absent) ou modifiée (R-CRE-1).
pub async fn save_simulation(
    db: &Db,
    id: Option<LoanSimulationId>,
    input: &SimulationInput,
    today: Date,
) -> AppResult<SimulationView> {
    let loan = match id {
        Some(id) => simulation_row(db, id).await?.loan_id,
        None => None,
    };
    let row = simulation_row_of(id.unwrap_or_else(LoanSimulationId::new), input, loan)?;
    row.terms.validate(today)?;
    check_references(db, row.institution_id, row.application_id).await?;
    db::credit::upsert_simulation(&db.write, &row).await?;
    simulation(db, row.id, today).await
}

/// Supprime une simulation ; les démarches et la demande qui la portaient perdent le lien (R-CRE-23).
pub async fn delete_simulation(db: &Db, id: LoanSimulationId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::credit::unselect_simulation(&mut *tx, id).await?;
    if db::credit::delete_simulation(&mut *tx, id).await? == 0 {
        return Err(AppError::NotFound("simulation"));
    }
    tx.commit().await?;
    Ok(())
}

/// Transforme une simulation en prêt (R-CRE-7) : le prêt est créé avec les conditions saisies (préremplies par
/// l'interface à partir de la simulation), et la simulation garde le lien.
pub async fn convert(db: &Db, id: LoanSimulationId, input: &LoanInput, today: Date) -> AppResult<LoanId> {
    let mut row = simulation_row(db, id).await?;
    if row.loan_id.is_some() {
        return Err(DomainError::SimulationAlreadyConverted.into());
    }
    let loan = super::loans::create(db, input, today).await?;
    row.loan_id = Some(loan.summary.id);
    db::credit::upsert_simulation(&db.write, &row).await?;
    Ok(loan.summary.id)
}

// ======================================================================= Demandes (CRE-04)

/// Demande de crédit saisie (R-CRE-10, R-CRE-11, R-CRE-13, R-CRE-14, R-CRE-16).
#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationInput {
    pub name: String,
    pub project_kind: ProjectKind,
    pub asset_id: Option<AssetId>,
    pub price_minor: i64,
    /// Absent : estimés (R-CRE-12).
    pub notary_fees_minor: Option<i64>,
    #[serde(default)]
    pub works_minor: i64,
    #[serde(default)]
    pub other_costs_minor: i64,
    #[serde(default)]
    pub down_payment_minor: i64,
    #[serde(default)]
    pub other_income_minor: i64,
    #[serde(default)]
    pub other_charges_minor: i64,
    pub co_borrower_name: Option<String>,
    #[serde(default)]
    pub co_borrower_income_minor: i64,
    #[serde(default)]
    pub co_borrower_charges_minor: i64,
    #[serde(default)]
    pub excluded_loan_ids: Vec<LoanId>,
    pub selected_simulation_id: Option<LoanSimulationId>,
    /// Absent : inchangé (`PREPARING` à la création).
    pub status: Option<ApplicationStatus>,
    pub note: Option<String>,
}

/// Une demande dans la liste (E38).
#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationSummary {
    pub id: LoanApplicationId,
    pub name: String,
    pub project_kind: ProjectKind,
    pub status: ApplicationStatus,
    pub to_borrow_minor: i64,
    pub missing_pieces: usize,
    /// Démarches à relancer (R-CRE-26).
    pub follow_ups_due: usize,
}

/// Revenu d'un emploi en cours (R-CRE-13).
#[derive(Debug, Serialize, ToSchema)]
pub struct IncomeLine {
    pub employment_id: cashflow_domain::shared::ids::EmploymentId,
    pub employer: String,
    pub monthly_minor: i64,
}

/// Échéance d'un prêt en cours (R-CRE-14).
#[derive(Debug, Serialize, ToSchema)]
pub struct ChargeLine {
    pub loan_id: LoanId,
    pub name: String,
    /// Échéance totale ; absente en suivi simple.
    pub monthly_minor: Option<i64>,
    /// Remboursé par le nouveau crédit : hors des charges.
    pub excluded: bool,
}

/// Capacité d'emprunt (R-CRE-13 à R-CRE-15).
#[derive(Debug, Serialize, ToSchema)]
pub struct CapacityView {
    pub employments: Vec<IncomeLine>,
    pub loans: Vec<ChargeLine>,
    pub income_minor: i64,
    pub charges_minor: i64,
    /// Mensualité de la simulation retenue, assurance comprise.
    pub payment_minor: Option<i64>,
    pub debt_ratio_ppm: Option<i64>,
    pub remaining_income_minor: i64,
    pub max_payment_minor: i64,
    pub borrowing_capacity_minor: Option<i64>,
    pub savings_minor: i64,
    pub savings_after_minor: i64,
    pub debt_ratio_exceeded: bool,
    pub duration_exceeded: bool,
    pub max_debt_ratio_ppm: i64,
    pub max_duration_months: u32,
}

/// Document d'une pièce (R-CRE-18).
#[derive(Debug, Serialize, ToSchema)]
pub struct PieceDocument {
    pub id: DocumentId,
    pub title: String,
    pub file_name: String,
    /// Attribué à la main ; sinon trouvé dans la bibliothèque.
    pub assigned: bool,
}

/// Une pièce à fournir et son état (R-CRE-17, R-CRE-18).
#[derive(Debug, Serialize, ToSchema)]
pub struct PieceView {
    pub piece: Piece,
    pub label: String,
    pub expected: usize,
    pub status: PieceStatus,
    pub documents: Vec<PieceDocument>,
}

/// Une démarche (R-CRE-21).
#[derive(Debug, Serialize, ToSchema)]
pub struct LenderRequestView {
    pub id: LenderRequestId,
    pub institution_id: Option<InstitutionId>,
    pub lender_name: Option<String>,
    /// Nom de l'établissement, sinon le nom libre.
    pub name: String,
    pub status: LenderRequestStatus,
    pub sent_on: Date,
    pub answered_on: Option<Date>,
    /// L'offre reçue.
    pub simulation_id: Option<LoanSimulationId>,
    pub follow_up_on: Option<Date>,
    /// À relancer (R-CRE-26).
    pub follow_up_due: bool,
    pub note: Option<String>,
    /// Journal, du plus récent au plus ancien (R-CRE-24).
    pub events: Vec<LenderEventView>,
}

/// Un échange au journal d'une démarche (R-CRE-24).
#[derive(Debug, Serialize, ToSchema)]
pub struct LenderEventView {
    pub id: LenderEventId,
    pub date: Date,
    pub kind: LenderEventKind,
    pub note: Option<String>,
}

/// Fiche d'une demande (E39).
#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationView {
    pub id: LoanApplicationId,
    pub name: String,
    pub project_kind: ProjectKind,
    pub status: ApplicationStatus,
    pub asset_id: Option<AssetId>,
    pub asset_name: Option<String>,
    pub price_minor: i64,
    pub notary_fees_minor: i64,
    /// Estimation des frais de notaire pour ce type de projet et ce prix (R-CRE-12).
    pub estimated_notary_fees_minor: i64,
    pub works_minor: i64,
    pub other_costs_minor: i64,
    pub down_payment_minor: i64,
    pub project_cost_minor: i64,
    pub to_borrow_minor: i64,
    pub other_income_minor: i64,
    pub other_charges_minor: i64,
    pub co_borrower_name: Option<String>,
    pub co_borrower_income_minor: i64,
    pub co_borrower_charges_minor: i64,
    pub excluded_loan_ids: Vec<LoanId>,
    pub selected_simulation_id: Option<LoanSimulationId>,
    pub note: Option<String>,
    pub capacity: CapacityView,
    pub simulations: Vec<SimulationView>,
    pub pieces: Vec<PieceView>,
    pub lender_requests: Vec<LenderRequestView>,
}

fn plan(row: &ApplicationRow) -> FinancingPlan {
    FinancingPlan {
        price: row.price,
        notary_fees: row.notary_fees,
        works: row.works,
        other_costs: row.other_costs,
        down_payment: row.down_payment,
    }
}

/// Ce que les calculs d'une demande lisent ailleurs : emplois, prêts, comptes, documents.
struct Sources {
    employments: Vec<IncomeLine>,
    /// Prêts en cours : identifiant, nom, échéance totale (avec tableau).
    loans: Vec<(LoanId, String, Option<Money>)>,
    checking_accounts: Vec<uuid::Uuid>,
    savings_accounts: Vec<uuid::Uuid>,
    savings: Money,
    library: Vec<LibraryDocument>,
    documents: HashMap<DocumentId, (String, String)>,
}

async fn context(db: &Db, today: Date) -> AppResult<Sources> {
    let book = SalaryBook::load(db).await?;
    let employments = book
        .employments
        .iter()
        .filter(|e| e.start_date <= today && e.end_date.is_none_or(|end| end >= today))
        .map(|e| {
            let p = book.payroll(e);
            IncomeLine {
                employment_id: e.id,
                employer: e.employer.clone(),
                monthly_minor: credit::employment_income(&p.job, &p.pays, &p.bonuses, today).minor(),
            }
        })
        .collect();
    let mut open_loans = Vec::new();
    for loan in db::loans::list(&db.read).await? {
        if loan.start_date > today || loan.closed_on.is_some_and(|c| c <= today) {
            continue;
        }
        let monthly = match &loan.schedule {
            Some(s) => {
                let schedule = loans::schedule(&s.terms)?;
                let next = schedule.iter().find(|i| i.date >= today);
                if next.is_none() {
                    continue;
                }
                next.map(|i| i.total())
            }
            None => None,
        };
        open_loans.push((loan.id, loan.name, monthly));
    }
    let accounts = super::accounts::list(db, today).await?;
    let open = |a: &&super::accounts::Account| a.closed_on.is_none_or(|c| c > today);
    let checking_accounts =
        accounts.iter().filter(open).filter(|a| a.kind == AccountKind::Checking).map(|a| *a.id.as_uuid()).collect();
    let savings_accounts =
        accounts.iter().filter(open).filter(|a| a.family != AccountFamily::Cash).map(|a| *a.id.as_uuid()).collect();
    let savings = accounts
        .iter()
        .filter(open)
        .filter(|a| a.include_in_emergency_fund)
        .map(|a| Money::from_minor(a.balance_minor))
        .sum();
    let mut links: HashMap<DocumentId, Vec<docs::LinkTarget>> = HashMap::new();
    for link in db::documents::raw_links(&db.read).await? {
        links.entry(link.document_id).or_default().push(link.target);
    }
    let mut library = Vec::new();
    let mut documents = HashMap::new();
    for row in db::documents::all(&db.read).await? {
        let reference_date = docs::reference_date(&row.description(), row.sent_on().unwrap_or(today));
        library.push(LibraryDocument {
            id: row.id,
            kind: row.kind,
            reference_date,
            links: links.remove(&row.id).unwrap_or_default(),
        });
        documents.insert(row.id, (row.title, row.file_name));
    }
    Ok(Sources { employments, loans: open_loans, checking_accounts, savings_accounts, savings, library, documents })
}

fn capacity_view(
    row: &ApplicationRow,
    ctx: &Sources,
    selected: Option<&credit::Simulation>,
    terms: Option<&SimulationTerms>,
) -> CapacityView {
    let income = ctx.employments.iter().map(|e| Money::from_minor(e.monthly_minor)).sum::<Money>()
        + row.other_income
        + row.co_borrower_income;
    let loans: Vec<ChargeLine> = ctx
        .loans
        .iter()
        .map(|(id, name, monthly)| ChargeLine {
            loan_id: *id,
            name: name.clone(),
            monthly_minor: monthly.map(Money::minor),
            excluded: row.excluded_loan_ids.contains(id),
        })
        .collect();
    let loan_charges: Money =
        loans.iter().filter(|l| !l.excluded).filter_map(|l| l.monthly_minor).map(Money::from_minor).sum();
    let capacity = credit::capacity(&CapacityInput {
        income,
        charges: loan_charges + row.other_charges + row.co_borrower_charges,
        payment: selected.map(credit::Simulation::total_payment),
        simulation: terms.map(|t| (t.annual_rate, t.duration_months, t.insurance)),
        project: row.project_kind,
        savings: ctx.savings,
        down_payment: row.down_payment,
    });
    CapacityView {
        employments: ctx
            .employments
            .iter()
            .map(|e| IncomeLine {
                employment_id: e.employment_id,
                employer: e.employer.clone(),
                monthly_minor: e.monthly_minor,
            })
            .collect(),
        loans,
        income_minor: capacity.income.minor(),
        charges_minor: capacity.charges.minor(),
        payment_minor: capacity.payment.map(Money::minor),
        debt_ratio_ppm: capacity.debt_ratio.map(Rate::ppm),
        remaining_income_minor: capacity.remaining_income.minor(),
        max_payment_minor: capacity.max_payment.minor(),
        borrowing_capacity_minor: capacity.borrowing_capacity.map(Money::minor),
        savings_minor: ctx.savings.minor(),
        savings_after_minor: capacity.savings_after.minor(),
        debt_ratio_exceeded: capacity.debt_ratio_exceeded,
        duration_exceeded: capacity.duration_exceeded,
        max_debt_ratio_ppm: credit::MAX_DEBT_RATIO.ppm(),
        max_duration_months: row.project_kind.max_duration(),
    }
}

async fn piece_views(db: &Db, row: &ApplicationRow, ctx: &Sources, today: Date) -> AppResult<Vec<PieceView>> {
    let piece_ctx = PieceContext {
        project: row.project_kind,
        co_borrower: row.co_borrower_name.is_some(),
        has_down_payment: row.down_payment.is_positive(),
        has_works: row.works.is_positive(),
        asset: row.asset_id.map(|a| *a.as_uuid()),
        employments: ctx.employments.iter().map(|e| *e.employment_id.as_uuid()).collect(),
        checking_accounts: ctx.checking_accounts.clone(),
        savings_accounts: ctx.savings_accounts.clone(),
        loans: ctx.loans.iter().map(|(id, _, _)| *id.as_uuid()).collect(),
        today,
    };
    let stored = db::credit::pieces(&db.read, row.id).await?;
    let document = |id: DocumentId, assigned: bool| {
        let (title, file_name) = ctx.documents.get(&id).cloned().unwrap_or_default();
        PieceDocument { id, title, file_name, assigned }
    };
    Ok(pieces::required(&piece_ctx)
        .into_iter()
        .map(|(piece, expected)| {
            let assigned: Vec<DocumentId> =
                stored.iter().filter(|s| s.piece == piece).filter_map(|s| s.document_id).collect();
            let not_required = stored.iter().any(|s| s.piece == piece && s.not_required);
            let documents: Vec<PieceDocument> = if assigned.is_empty() {
                pieces::found(piece, &piece_ctx, &ctx.library).into_iter().map(|id| document(id, false)).collect()
            } else {
                assigned.into_iter().map(|id| document(id, true)).collect()
            };
            PieceView {
                piece,
                label: piece.label().to_owned(),
                expected,
                status: pieces::status(expected, documents.len(), not_required),
                documents,
            }
        })
        .collect())
}

fn lender_request_view(row: LenderRequestRow, names: &Names, events: &[EventRow], today: Date) -> LenderRequestView {
    let name = row
        .institution_id
        .and_then(|i| names.institutions.get(&i).cloned())
        .or_else(|| row.lender_name.clone())
        .unwrap_or_default();
    LenderRequestView {
        id: row.id,
        institution_id: row.institution_id,
        lender_name: row.lender_name,
        name,
        status: row.status,
        sent_on: row.sent_on,
        answered_on: row.answered_on,
        simulation_id: row.simulation_id,
        follow_up_due: credit::follow_up_due(row.status, row.follow_up_on, today),
        follow_up_on: row.follow_up_on,
        note: row.note,
        events: events
            .iter()
            .filter(|e| e.lender_request_id == row.id)
            .map(|e| LenderEventView { id: e.id, date: e.date, kind: e.kind, note: e.note.clone() })
            .collect(),
    }
}

async fn application_row(db: &Db, id: LoanApplicationId) -> AppResult<ApplicationRow> {
    db::credit::find_application(&db.read, id).await?.ok_or(AppError::NotFound("demande de crédit"))
}

pub async fn application(db: &Db, id: LoanApplicationId, today: Date) -> AppResult<ApplicationView> {
    let row = application_row(db, id).await?;
    let ctx = context(db, today).await?;
    let names = names(db).await?;
    let simulation_rows: Vec<SimulationRow> =
        db::credit::simulations(&db.read).await?.into_iter().filter(|s| s.application_id == Some(id)).collect();
    let selected = simulation_rows.iter().find(|s| Some(s.id) == row.selected_simulation_id);
    let computed = selected.map(|s| credit::simulate(&s.terms, today)).transpose()?;
    let capacity = capacity_view(&row, &ctx, computed.as_ref(), selected.map(|s| &s.terms));
    let pieces = piece_views(db, &row, &ctx, today).await?;
    let events = db::credit::events(&db.read, id).await?;
    let lender_requests = db::credit::lender_requests(&db.read, id)
        .await?
        .into_iter()
        .map(|r| lender_request_view(r, &names, &events, today))
        .collect();
    let asset_name = match row.asset_id {
        Some(asset) => db::assets::find(&db.read, asset).await?.map(|a| a.name),
        None => None,
    };
    let plan = plan(&row);
    Ok(ApplicationView {
        id: row.id,
        estimated_notary_fees_minor: credit::estimated_notary_fees(row.project_kind, row.price).minor(),
        project_cost_minor: plan.project_cost().minor(),
        to_borrow_minor: plan.to_borrow().minor(),
        capacity,
        simulations: simulation_rows
            .into_iter()
            .map(|s| simulation_view(s, &names, today))
            .collect::<AppResult<_>>()?,
        pieces,
        lender_requests,
        asset_name,
        name: row.name,
        project_kind: row.project_kind,
        status: row.status,
        asset_id: row.asset_id,
        price_minor: row.price.minor(),
        notary_fees_minor: row.notary_fees.minor(),
        works_minor: row.works.minor(),
        other_costs_minor: row.other_costs.minor(),
        down_payment_minor: row.down_payment.minor(),
        other_income_minor: row.other_income.minor(),
        other_charges_minor: row.other_charges.minor(),
        co_borrower_name: row.co_borrower_name,
        co_borrower_income_minor: row.co_borrower_income.minor(),
        co_borrower_charges_minor: row.co_borrower_charges.minor(),
        excluded_loan_ids: row.excluded_loan_ids,
        selected_simulation_id: row.selected_simulation_id,
        note: row.note,
    })
}

pub async fn applications(db: &Db, today: Date) -> AppResult<Vec<ApplicationSummary>> {
    let ctx = context(db, today).await?;
    let mut list = Vec::new();
    for row in db::credit::applications(&db.read).await? {
        let missing = piece_views(db, &row, &ctx, today)
            .await?
            .iter()
            .filter(|p| matches!(p.status, PieceStatus::Missing | PieceStatus::Incomplete))
            .count();
        let follow_ups_due = db::credit::lender_requests(&db.read, row.id)
            .await?
            .iter()
            .filter(|r| credit::follow_up_due(r.status, r.follow_up_on, today))
            .count();
        list.push(ApplicationSummary {
            id: row.id,
            to_borrow_minor: plan(&row).to_borrow().minor(),
            name: row.name,
            project_kind: row.project_kind,
            status: row.status,
            missing_pieces: missing,
            follow_ups_due,
        });
    }
    Ok(list)
}

/// Enregistre une demande, nouvelle (`id` absent) ou modifiée (R-CRE-10 à R-CRE-16).
pub async fn save_application(
    db: &Db,
    id: Option<LoanApplicationId>,
    input: &ApplicationInput,
    today: Date,
) -> AppResult<ApplicationView> {
    let current = match id {
        Some(id) => Some(application_row(db, id).await?),
        None => None,
    };
    let id = id.unwrap_or_else(LoanApplicationId::new);
    let price = Money::from_minor(input.price_minor);
    let co_borrower_name = clean(input.co_borrower_name.as_deref());
    if co_borrower_name.as_ref().is_some_and(|n| n.chars().count() > 80) {
        return Err(AppError::invalid_field("TOO_LONG", "co_borrower_name", "80 caractères au maximum"));
    }
    let has_co_borrower = co_borrower_name.is_some();
    let row = ApplicationRow {
        id,
        name: required(&input.name, "name", 80)?,
        project_kind: input.project_kind,
        status: input.status.or(current.as_ref().map(|c| c.status)).unwrap_or(ApplicationStatus::Preparing),
        asset_id: input.asset_id,
        price,
        notary_fees: input
            .notary_fees_minor
            .map_or_else(|| credit::estimated_notary_fees(input.project_kind, price), Money::from_minor),
        works: Money::from_minor(input.works_minor),
        other_costs: Money::from_minor(input.other_costs_minor),
        down_payment: Money::from_minor(input.down_payment_minor),
        other_income: Money::from_minor(input.other_income_minor),
        other_charges: Money::from_minor(input.other_charges_minor),
        co_borrower_income: Money::from_minor(if has_co_borrower { input.co_borrower_income_minor } else { 0 }),
        co_borrower_charges: Money::from_minor(if has_co_borrower { input.co_borrower_charges_minor } else { 0 }),
        co_borrower_name,
        excluded_loan_ids: input.excluded_loan_ids.clone(),
        selected_simulation_id: input.selected_simulation_id,
        note: clean(input.note.as_deref()),
    };
    plan(&row).validate()?;
    if [row.other_income, row.other_charges, row.co_borrower_income, row.co_borrower_charges]
        .iter()
        .any(|m| m.is_negative())
    {
        return Err(AppError::invalid("INVALID_AMOUNT", "revenus et charges sont positifs ou nuls"));
    }
    if let Some(asset) = row.asset_id {
        db::assets::find(&db.read, asset).await?.ok_or(AppError::NotFound("bien"))?;
    }
    if let Some(selected) = row.selected_simulation_id
        && simulation_row(db, selected).await?.application_id != Some(id)
    {
        return Err(AppError::invalid_field(
            "SIMULATION_OF_ANOTHER_APPLICATION",
            "selected_simulation_id",
            "la simulation retenue doit être une simulation de cette demande",
        ));
    }
    db::credit::upsert_application(&db.write, &row).await?;
    application(db, id, today).await
}

/// Supprime une demande : ses démarches et ses pièces partent avec elle ; ses simulations restent (R-CRE-23).
pub async fn delete_application(db: &Db, id: LoanApplicationId) -> AppResult<()> {
    if db::credit::delete_application(&db.write, id).await? == 0 {
        return Err(AppError::NotFound("demande de crédit"));
    }
    Ok(())
}

// ======================================================================= Pièces (CRE-05)

/// Attribue un document à une pièce (R-CRE-18).
pub async fn assign(
    db: &Db,
    id: LoanApplicationId,
    piece: Piece,
    document: DocumentId,
    today: Date,
) -> AppResult<ApplicationView> {
    application_row(db, id).await?;
    db::documents::find(&db.read, document).await?.ok_or(AppError::NotFound("document"))?;
    db::credit::assign_piece(&db.write, id, piece, document).await?;
    application(db, id, today).await
}

pub async fn unassign(
    db: &Db,
    id: LoanApplicationId,
    piece: Piece,
    document: DocumentId,
    today: Date,
) -> AppResult<ApplicationView> {
    if db::credit::unassign_piece(&db.write, id, piece, document).await? == 0 {
        return Err(AppError::NotFound("pièce attribuée"));
    }
    application(db, id, today).await
}

/// Marque une pièce non requise, ou de nouveau requise (R-CRE-18).
pub async fn set_required(
    db: &Db,
    id: LoanApplicationId,
    piece: Piece,
    not_required: bool,
    today: Date,
) -> AppResult<ApplicationView> {
    application_row(db, id).await?;
    db::credit::set_not_required(&db.write, id, piece, not_required).await?;
    application(db, id, today).await
}

// ======================================================================= Démarches (CRE-06)

/// Démarche saisie (R-CRE-21).
#[derive(Debug, Deserialize, ToSchema)]
pub struct LenderRequestInput {
    pub institution_id: Option<InstitutionId>,
    pub lender_name: Option<String>,
    pub status: LenderRequestStatus,
    pub sent_on: Date,
    pub answered_on: Option<Date>,
    pub simulation_id: Option<LoanSimulationId>,
    /// Prochaine relance ; à la création, absente : l'envoi + 14 jours (R-CRE-25).
    pub follow_up_on: Option<Date>,
    pub note: Option<String>,
}

/// Enregistre une démarche, nouvelle (`request` absent) ou modifiée. La première démarche envoyée fait passer la
/// demande à « envoyée » (R-CRE-20) ; une nouvelle démarche inscrit son envoi au journal, un refus une réponse
/// (R-CRE-24).
pub async fn save_lender_request(
    db: &Db,
    id: LoanApplicationId,
    request: Option<LenderRequestId>,
    input: &LenderRequestInput,
    today: Date,
) -> AppResult<ApplicationView> {
    let application = application_row(db, id).await?;
    let current = match request {
        Some(request) => Some(
            db::credit::lender_requests(&db.read, id)
                .await?
                .into_iter()
                .find(|r| r.id == request)
                .ok_or(AppError::NotFound("démarche"))?,
        ),
        None => None,
    };
    let lender_name = clean(input.lender_name.as_deref());
    if input.institution_id.is_none() && lender_name.is_none() {
        return Err(AppError::invalid_field("REQUIRED", "lender_name", "un établissement ou un nom est requis"));
    }
    if lender_name.as_ref().is_some_and(|n| n.chars().count() > 80) {
        return Err(AppError::invalid_field("TOO_LONG", "lender_name", "80 caractères au maximum"));
    }
    if input.answered_on.is_some_and(|a| a < input.sent_on) {
        return Err(AppError::invalid_field("INVALID_DATE", "answered_on", "la réponse suit l'envoi"));
    }
    check_references(db, input.institution_id, None).await?;
    if let Some(simulation) = input.simulation_id {
        simulation_row(db, simulation).await?;
    }
    let row = LenderRequestRow {
        id: request.unwrap_or_else(LenderRequestId::new),
        application_id: id,
        institution_id: input.institution_id,
        lender_name,
        status: input.status,
        sent_on: input.sent_on,
        answered_on: input.answered_on,
        simulation_id: input.simulation_id,
        follow_up_on: match (&current, input.follow_up_on) {
            (None, None) => Some(credit::follow_up_after(input.sent_on, credit::FIRST_FOLLOW_UP_DAYS)),
            (_, date) => date,
        },
        note: clean(input.note.as_deref()),
    };
    let refused = row.status == LenderRequestStatus::Refused
        && current.as_ref().is_some_and(|c| c.status != LenderRequestStatus::Refused);
    let row = LenderRequestRow { follow_up_on: if refused { None } else { row.follow_up_on }, ..row };
    let mut tx = db.write.begin().await?;
    db::credit::upsert_lender_request(&mut *tx, &row).await?;
    if current.is_none() {
        add_event(&mut tx, row.id, row.sent_on, LenderEventKind::Sent, None).await?;
    }
    if refused {
        add_event(&mut tx, row.id, row.answered_on.unwrap_or(today), LenderEventKind::Answer, Some("Refus")).await?;
    }
    let status = credit::status_after_sending(application.status);
    if status != application.status {
        db::credit::set_application_status(&mut *tx, id, status).await?;
    }
    tx.commit().await?;
    super::credit::application(db, id, today).await
}

/// Enregistre l'offre reçue d'une démarche : une simulation de la demande, rattachée à la démarche, qui passe à
/// « offre reçue » (R-CRE-21).
pub async fn record_offer(
    db: &Db,
    id: LoanApplicationId,
    request: LenderRequestId,
    input: &SimulationInput,
    today: Date,
) -> AppResult<ApplicationView> {
    application_row(db, id).await?;
    let mut row = db::credit::lender_requests(&db.read, id)
        .await?
        .into_iter()
        .find(|r| r.id == request)
        .ok_or(AppError::NotFound("démarche"))?;
    let simulation = simulation_row_of(LoanSimulationId::new(), input, None)?;
    let simulation = SimulationRow {
        application_id: Some(id),
        institution_id: input.institution_id.or(row.institution_id),
        ..simulation
    };
    simulation.terms.validate(today)?;
    row.simulation_id = Some(simulation.id);
    row.status = LenderRequestStatus::Offer;
    row.answered_on = Some(row.answered_on.unwrap_or(today).max(row.sent_on));
    row.follow_up_on = None;
    let mut tx = db.write.begin().await?;
    db::credit::upsert_simulation(&mut *tx, &simulation).await?;
    db::credit::upsert_lender_request(&mut *tx, &row).await?;
    add_event(&mut tx, row.id, today, LenderEventKind::Answer, Some(&format!("Offre : {}", simulation.name))).await?;
    tx.commit().await?;
    application(db, id, today).await
}

/// Accepte l'offre d'une démarche (R-CRE-22) : les autres démarches en cours sont retirées, la demande acceptée,
/// et l'offre devient la simulation retenue.
pub async fn accept_offer(
    db: &Db,
    id: LoanApplicationId,
    request: LenderRequestId,
    today: Date,
) -> AppResult<ApplicationView> {
    let mut application = application_row(db, id).await?;
    let mut requests = db::credit::lender_requests(&db.read, id).await?;
    let index = requests.iter().position(|r| r.id == request).ok_or(AppError::NotFound("démarche"))?;
    let statuses: Vec<LenderRequestStatus> = requests.iter().map(|r| r.status).collect();
    let next = credit::accept_offer(&statuses, index, requests[index].simulation_id.is_some())?;
    let mut tx = db.write.begin().await?;
    for (row, status) in requests.iter_mut().zip(next) {
        if row.status != status {
            row.status = status;
            row.answered_on = row.answered_on.or(Some(today.max(row.sent_on)));
            row.follow_up_on = None;
            db::credit::upsert_lender_request(&mut *tx, row).await?;
        }
    }
    application.status = ApplicationStatus::Accepted;
    application.selected_simulation_id = requests[index].simulation_id;
    db::credit::upsert_application(&mut *tx, &application).await?;
    tx.commit().await?;
    super::credit::application(db, id, today).await
}

pub async fn delete_lender_request(db: &Db, id: LoanApplicationId, request: LenderRequestId) -> AppResult<()> {
    if db::credit::delete_lender_request(&db.write, id, request).await? == 0 {
        return Err(AppError::NotFound("démarche"));
    }
    Ok(())
}

// ======================================================================= Journal et relances (CRE-09)

async fn add_event(
    conn: &mut sqlx::SqliteConnection,
    request: LenderRequestId,
    date: Date,
    kind: LenderEventKind,
    note: Option<&str>,
) -> AppResult<()> {
    let row =
        EventRow { id: LenderEventId::new(), lender_request_id: request, date, kind, note: note.map(str::to_owned) };
    db::credit::upsert_event(&mut *conn, &row).await?;
    Ok(())
}

async fn lender_request_row(db: &Db, id: LoanApplicationId, request: LenderRequestId) -> AppResult<LenderRequestRow> {
    application_row(db, id).await?;
    db::credit::lender_requests(&db.read, id)
        .await?
        .into_iter()
        .find(|r| r.id == request)
        .ok_or(AppError::NotFound("démarche"))
}

/// Échange saisi au journal (R-CRE-24).
#[derive(Debug, Deserialize, ToSchema)]
pub struct LenderEventInput {
    pub date: Date,
    pub kind: LenderEventKind,
    pub note: Option<String>,
}

/// Ajoute (`event` absent) ou modifie un échange du journal d'une démarche (R-CRE-24).
pub async fn save_event(
    db: &Db,
    id: LoanApplicationId,
    request: LenderRequestId,
    event: Option<LenderEventId>,
    input: &LenderEventInput,
    today: Date,
) -> AppResult<ApplicationView> {
    lender_request_row(db, id, request).await?;
    if let Some(event) = event
        && !db::credit::events(&db.read, id).await?.iter().any(|e| e.id == event && e.lender_request_id == request)
    {
        return Err(AppError::NotFound("échange"));
    }
    let row = EventRow {
        id: event.unwrap_or_else(LenderEventId::new),
        lender_request_id: request,
        date: input.date,
        kind: input.kind,
        note: clean(input.note.as_deref()),
    };
    db::credit::upsert_event(&db.write, &row).await?;
    application(db, id, today).await
}

pub async fn delete_event(
    db: &Db,
    id: LoanApplicationId,
    request: LenderRequestId,
    event: LenderEventId,
    today: Date,
) -> AppResult<ApplicationView> {
    lender_request_row(db, id, request).await?;
    if db::credit::delete_event(&db.write, request, event).await? == 0 {
        return Err(AppError::NotFound("échange"));
    }
    application(db, id, today).await
}

/// Relance saisie : un texte, et la prochaine relance (par défaut, dans 7 jours) (R-CRE-25).
#[derive(Debug, Deserialize, ToSchema)]
pub struct FollowUpInput {
    pub note: Option<String>,
    /// Absente : dans 7 jours.
    pub next_follow_up_on: Option<Date>,
}

/// Relance une démarche (R-CRE-25) : la relance est inscrite au journal, datée du jour, et la prochaine relance
/// reportée.
pub async fn follow_up(
    db: &Db,
    id: LoanApplicationId,
    request: LenderRequestId,
    input: &FollowUpInput,
    today: Date,
) -> AppResult<ApplicationView> {
    let mut row = lender_request_row(db, id, request).await?;
    row.follow_up_on =
        Some(input.next_follow_up_on.unwrap_or_else(|| credit::follow_up_after(today, credit::NEXT_FOLLOW_UP_DAYS)));
    let note = clean(input.note.as_deref());
    let mut tx = db.write.begin().await?;
    db::credit::upsert_lender_request(&mut *tx, &row).await?;
    add_event(&mut tx, request, today, LenderEventKind::FollowUp, note.as_deref()).await?;
    tx.commit().await?;
    application(db, id, today).await
}

/// Annonce les démarches à relancer, une fois par démarche et par date de relance (R-CRE-26).
pub async fn notify_follow_ups(db: &Db, today: Date) -> AppResult<usize> {
    let due: Vec<_> = db::credit::follow_ups(&db.read)
        .await?
        .into_iter()
        .filter(|(_, _, _, status, date)| credit::follow_up_due(*status, Some(*date), today))
        .collect();
    if due.is_empty() {
        return Ok(0);
    }
    let known: HashSet<String> = db::budgets::known_keys(&db.read)
        .await?
        .iter()
        .flat_map(|k| k.split(' ').map(str::to_owned).collect::<Vec<_>>())
        .collect();
    let applications: HashMap<LoanApplicationId, String> =
        db::credit::applications(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect();
    let mut sent = 0;
    for (request, application, name, _, date) in due {
        let key = format!("follow-up:{request}:{date}");
        if known.contains(&key) {
            continue;
        }
        let project = applications.get(&application).cloned().unwrap_or_default();
        db::budgets::notify(
            &db.write,
            "LENDER_FOLLOW_UP",
            &format!("Relancer {name}"),
            &format!("Demande « {project} » : relance prévue le {}", date.format("%d/%m/%Y")),
            Some(&format!("/prets/demandes/{application}")),
            Some(&key),
        )
        .await?;
        sent += 1;
    }
    Ok(sent)
}

// ======================================================================= Capacité d'achat (CRE-07)

/// Réglage qui garde les hypothèses de la capacité d'achat (R-CRE-30).
const PURCHASE_SETTING: &str = "credit.purchase_capacity";

/// Hypothèses de la capacité d'achat (R-CRE-30).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PurchaseHypotheses {
    pub project_kind: ProjectKind,
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub insurance: Insurance,
    pub fees_minor: i64,
    pub guarantee_fees_minor: i64,
    /// Taux d'endettement visé.
    pub max_debt_ratio_ppm: i64,
    /// Réserve de sécurité, en mois de dépenses moyennes.
    pub reserve_months: u32,
    /// Comptes mobilisables ; absent : ceux de l'épargne de précaution.
    pub account_ids: Option<Vec<AccountId>>,
    pub other_income_minor: i64,
    pub other_charges_minor: i64,
    pub co_borrower_income_minor: i64,
    pub co_borrower_charges_minor: i64,
    pub excluded_loan_ids: Vec<LoanId>,
}

impl Default for PurchaseHypotheses {
    fn default() -> Self {
        Self {
            project_kind: ProjectKind::ExistingHome,
            annual_rate_ppm: 32_000,
            duration_months: 300,
            insurance: Insurance::RateOnPrincipal { rate_ppm: 3_000 },
            fees_minor: 100_000,
            guarantee_fees_minor: 200_000,
            max_debt_ratio_ppm: credit::MAX_DEBT_RATIO.ppm(),
            reserve_months: 6,
            account_ids: None,
            other_income_minor: 0,
            other_charges_minor: 0,
            co_borrower_income_minor: 0,
            co_borrower_charges_minor: 0,
            excluded_loan_ids: vec![],
        }
    }
}

/// Un compte proposé pour l'apport (R-CRE-30).
#[derive(Debug, Serialize, ToSchema)]
pub struct SavingsAccount {
    pub id: AccountId,
    pub name: String,
    pub balance_minor: i64,
    pub selected: bool,
}

/// Capacité d'achat à une date (R-CRE-31, R-CRE-33).
#[derive(Debug, Serialize, ToSchema)]
pub struct PurchaseView {
    pub date: Date,
    pub income_minor: i64,
    pub charges_minor: i64,
    pub max_payment_minor: i64,
    pub capital_minor: i64,
    pub savings_minor: i64,
    pub reserve_minor: i64,
    pub down_payment_minor: i64,
    pub budget_minor: i64,
    pub max_price_minor: i64,
    pub notary_fees_minor: i64,
    /// Échéance totale du crédit au capital empruntable.
    pub payment_minor: i64,
}

/// Prix maximal pour une durée et un taux (R-CRE-34).
#[derive(Debug, Serialize, ToSchema)]
pub struct SensitivityCell {
    pub duration_months: u32,
    pub annual_rate_ppm: i64,
    pub max_price_minor: i64,
}

/// Patrimoine projeté sans et avec l'achat (R-CRE-35).
#[derive(Debug, Serialize, ToSchema)]
pub struct NetWorthComparison {
    pub date: Date,
    pub without_minor: i64,
    pub with_minor: i64,
}

/// Capacité d'achat (E40).
#[derive(Debug, Serialize, ToSchema)]
pub struct PurchaseCapacity {
    pub hypotheses: PurchaseHypotheses,
    pub accounts: Vec<SavingsAccount>,
    pub loans: Vec<ChargeLine>,
    /// Dépenses mensuelles moyennes (R-TDB-8).
    pub expenses_minor: i64,
    pub today: PurchaseView,
    /// Épargne restante après l'apport, en mois de dépenses (dixièmes).
    pub savings_after_minor: i64,
    pub savings_after_tenths_of_months: Option<i64>,
    pub remaining_income_minor: i64,
    /// Capacité d'épargne mensuelle avant et après l'achat (R-CRE-32) ; absente sans capacité dans le scénario.
    pub savings_capacity_before_minor: Option<i64>,
    pub savings_capacity_after_minor: Option<i64>,
    /// Les 5 prochaines années (R-CRE-33).
    pub years: Vec<PurchaseView>,
    pub sensitivity: Vec<SensitivityCell>,
    pub net_worth: Vec<NetWorthComparison>,
}

async fn hypotheses(db: &Db) -> AppResult<PurchaseHypotheses> {
    Ok(db::settings::get::<PurchaseHypotheses>(&db.read, PURCHASE_SETTING).await?.unwrap_or_default())
}

/// Garde les hypothèses (R-CRE-30) et renvoie la capacité d'achat.
pub async fn save_purchase_hypotheses(db: &Db, input: &PurchaseHypotheses, today: Date) -> AppResult<PurchaseCapacity> {
    let amounts = [
        input.fees_minor,
        input.guarantee_fees_minor,
        input.other_income_minor,
        input.other_charges_minor,
        input.co_borrower_income_minor,
        input.co_borrower_charges_minor,
    ];
    if amounts.iter().any(|m| *m < 0)
        || input.annual_rate_ppm < 0
        || !(1..=loans::MAX_DURATION_MONTHS).contains(&input.duration_months)
        || !(1..=1_000_000).contains(&input.max_debt_ratio_ppm)
        || input.reserve_months > 120
    {
        return Err(AppError::invalid("INVALID_HYPOTHESES", "hypothèses de la capacité d'achat invalides"));
    }
    db::settings::set(&db.write, PURCHASE_SETTING, input).await?;
    purchase_capacity(db, today).await
}

/// Échéance totale d'un prêt avec tableau dans le mois de `date` ; zéro sinon.
fn installment_in_month(schedule: &[loans::Installment], date: Date) -> Money {
    let month = cashflow_domain::salaries::month_of(date);
    schedule
        .iter()
        .find(|i| cashflow_domain::salaries::month_of(i.date) == month)
        .map_or(Money::ZERO, loans::Installment::total)
}

fn purchase_view(date: Date, input: &credit::PurchaseInput, p: &credit::Purchase) -> PurchaseView {
    PurchaseView {
        date,
        income_minor: input.income.minor(),
        charges_minor: input.charges.minor(),
        max_payment_minor: p.max_payment.minor(),
        capital_minor: p.capital.minor(),
        savings_minor: input.savings.minor(),
        reserve_minor: input.reserve.minor(),
        down_payment_minor: p.down_payment.minor(),
        budget_minor: p.budget.minor(),
        max_price_minor: p.max_price.minor(),
        notary_fees_minor: p.notary_fees.minor(),
        payment_minor: p.payment.minor(),
    }
}

/// Capacité d'achat : aujourd'hui, dans les années à venir, sensibilité, patrimoine projeté (R-CRE-31 à R-CRE-35).
pub async fn purchase_capacity(db: &Db, today: Date) -> AppResult<PurchaseCapacity> {
    let h = hypotheses(db).await?;
    let sources = context(db, today).await?;
    let excluded: HashSet<LoanId> = h.excluded_loan_ids.iter().copied().collect();

    // Comptes mobilisables (R-CRE-30).
    let accounts: Vec<SavingsAccount> = super::accounts::list(db, today)
        .await?
        .into_iter()
        .filter(|a| a.closed_on.is_none_or(|c| c > today))
        .map(|a| SavingsAccount {
            selected: h.account_ids.as_ref().map_or(a.include_in_emergency_fund, |ids| ids.contains(&a.id)),
            id: a.id,
            name: a.name,
            balance_minor: a.balance_minor,
        })
        .collect();
    let savings: Money = accounts.iter().filter(|a| a.selected).map(|a| Money::from_minor(a.balance_minor)).sum();

    // Scénario par défaut : salaires, dépenses, capacité d'épargne et patrimoine projetés (spec 07).
    let horizon = 12 * 10 + 12;
    let projection =
        super::projections::net_worth(db, &super::projections::NetWorthQuery { months: Some(horizon) }, today).await?;
    let scenario = projection.scenarios.iter().find(|s| s.is_default);
    let capacity_points = scenario.and_then(|s| s.capacity.as_ref()).map(|c| &c.points);
    let expenses = Money::from_minor(projection.capacity.expenses_minor);

    // Prêts en cours, avec leur tableau.
    let mut schedules = Vec::new();
    for loan in db::loans::list(&db.read).await? {
        if excluded.contains(&loan.id) {
            continue;
        }
        if let Some(s) = &loan.schedule {
            schedules.push(loans::schedule(&s.terms)?);
        }
    }
    let loans_at = |date: Date| -> Money { schedules.iter().map(|s| installment_in_month(s, date)).sum() };

    let rate = Rate::from_ppm(h.annual_rate_ppm);
    let input_at = |income: Money, charges: Money, savings: Money, reserve: Money, duration: u32, rate: Rate| {
        credit::PurchaseInput {
            income,
            charges,
            max_debt_ratio: Rate::from_ppm(h.max_debt_ratio_ppm),
            annual_rate: rate,
            duration_months: duration,
            insurance: h.insurance,
            fees: Money::from_minor(h.fees_minor),
            guarantee_fees: Money::from_minor(h.guarantee_fees_minor),
            project: h.project_kind,
            savings,
            reserve,
        }
    };
    let extra_income = Money::from_minor(h.other_income_minor + h.co_borrower_income_minor);
    let extra_charges = Money::from_minor(h.other_charges_minor + h.co_borrower_charges_minor);
    let months = i64::from(h.reserve_months);

    // Aujourd'hui (R-CRE-31) : revenus et charges comme une demande (R-CRE-13, R-CRE-14).
    let income = sources.employments.iter().map(|e| Money::from_minor(e.monthly_minor)).sum::<Money>() + extra_income;
    let loan_lines: Vec<ChargeLine> = sources
        .loans
        .iter()
        .map(|(id, name, monthly)| ChargeLine {
            loan_id: *id,
            name: name.clone(),
            monthly_minor: monthly.map(Money::minor),
            excluded: excluded.contains(id),
        })
        .collect();
    let loan_charges: Money =
        loan_lines.iter().filter(|l| !l.excluded).filter_map(|l| l.monthly_minor).map(Money::from_minor).sum();
    let charges = loan_charges + extra_charges;
    let reserve = Money::from_minor(expenses.minor() * months);
    let now_input = input_at(income, charges, savings, reserve, h.duration_months, rate);
    let now = credit::purchase(&now_input);

    // Effet de l'achat (R-CRE-32).
    let excluded_payments: Money =
        loan_lines.iter().filter(|l| l.excluded).filter_map(|l| l.monthly_minor).map(Money::from_minor).sum();
    let before = capacity_points.and_then(|p| p.first()).map(|p| p.total_minor);
    let after = before.map(|b| b - now.payment.minor() + excluded_payments.minor());

    // Dans les années à venir (R-CRE-33).
    let mut years = vec![purchase_view(today, &now_input, &now)];
    if let Some(points) = capacity_points {
        for year in 1..=5_usize {
            let start = 12 * year;
            let Some(point) = points.get(start) else { break };
            let next: Vec<i64> = points.iter().skip(start).take(12).map(|p| p.salaries_minor).collect();
            let salaries = next.iter().sum::<i64>() / i64::try_from(next.len().max(1)).unwrap_or(1);
            let cumulative: i64 = points.iter().take(start).map(|p| p.total_minor).sum();
            let date = today.checked_add_months(chrono::Months::new(12 * year as u32)).unwrap_or(today);
            let input = input_at(
                Money::from_minor(salaries) + extra_income,
                loans_at(date) + extra_charges,
                savings + Money::from_minor(cumulative),
                Money::from_minor(point.expenses_minor * months),
                h.duration_months,
                rate,
            );
            years.push(purchase_view(date, &input, &credit::purchase(&input)));
        }
    }

    // Sensibilité (R-CRE-34).
    let longest = if h.project_kind == ProjectKind::NewHome { 324 } else { 300 };
    let mut sensitivity = Vec::new();
    for duration in [180, 240, longest] {
        for delta in [-5_000, 0, 5_000] {
            let rate_ppm = (h.annual_rate_ppm + delta).max(0);
            let input = input_at(income, charges, savings, reserve, duration, Rate::from_ppm(rate_ppm));
            sensitivity.push(SensitivityCell {
                duration_months: duration,
                annual_rate_ppm: rate_ppm,
                max_price_minor: credit::purchase(&input).max_price.minor(),
            });
        }
    }

    // Patrimoine projeté avec l'achat, sur 10 ans (R-CRE-35).
    let mut net_worth = Vec::new();
    if let Some(scenario) = scenario
        && now.capital.is_positive()
    {
        let terms = SimulationTerms {
            kind: LoanKind::Mortgage,
            principal: now.capital,
            annual_rate: rate,
            duration_months: h.duration_months,
            insurance: h.insurance,
            fees: Money::from_minor(h.fees_minor),
            guarantee_fees: Money::from_minor(h.guarantee_fees_minor),
        };
        let simulation = credit::simulate(&terms, today)?;
        let baseline: Vec<(Date, Money)> =
            scenario.points.iter().take(120).map(|p| (p.date, Money::from_minor(p.value_minor))).collect();
        let one_off = now.notary_fees + terms.fees + terms.guarantee_fees;
        let with = credit::net_worth_with_purchase(&baseline, one_off, &simulation.schedule);
        net_worth = baseline
            .iter()
            .zip(with)
            .map(|((date, without), (_, with))| NetWorthComparison {
                date: *date,
                without_minor: without.minor(),
                with_minor: with.minor(),
            })
            .collect();
    }

    Ok(PurchaseCapacity {
        accounts,
        loans: loan_lines,
        expenses_minor: expenses.minor(),
        savings_after_minor: now.savings_after.minor(),
        savings_after_tenths_of_months: expenses
            .is_positive()
            .then(|| now.savings_after.minor() * 10 / expenses.minor()),
        remaining_income_minor: (income - charges - now.payment).minor(),
        savings_capacity_before_minor: before,
        savings_capacity_after_minor: after,
        today: purchase_view(today, &now_input, &now),
        years,
        sensitivity,
        net_worth,
        hypotheses: h,
    })
}

/// Prépare une demande depuis une simulation enregistrée (R-CRE-36) : le plus grand prix qui tient dans son
/// capital, l'apport possible et ses frais, avec l'apport nécessaire, et la simulation retenue.
pub async fn application_from_simulation(
    db: &Db,
    simulation: LoanSimulationId,
    name: &str,
    today: Date,
) -> AppResult<ApplicationView> {
    let mut sim = simulation_row(db, simulation).await?;
    let capacity = purchase_capacity(db, today).await?;
    let h = &capacity.hypotheses;
    let costs = sim.terms.fees + sim.terms.guarantee_fees;
    let funds = sim.terms.principal + Money::from_minor(capacity.today.down_payment_minor);
    let budget = if funds > costs { funds - costs } else { Money::ZERO };
    let (price, notary) = credit::max_price(h.project_kind, budget);
    // L'apport nécessaire, au plus l'apport possible : le prix arrondi laisse parfois un reliquat d'épargne.
    let cost = (price + notary + costs).minor();
    let down_payment = (cost - sim.terms.principal.minor()).clamp(0, capacity.today.down_payment_minor);
    let mut input = ApplicationInput {
        name: name.to_owned(),
        project_kind: h.project_kind,
        asset_id: None,
        price_minor: price.minor(),
        notary_fees_minor: Some(notary.minor()),
        works_minor: 0,
        other_costs_minor: costs.minor(),
        down_payment_minor: down_payment,
        other_income_minor: h.other_income_minor,
        other_charges_minor: h.other_charges_minor,
        co_borrower_name: (h.co_borrower_income_minor > 0 || h.co_borrower_charges_minor > 0)
            .then(|| "Co-emprunteur".to_owned()),
        co_borrower_income_minor: h.co_borrower_income_minor,
        co_borrower_charges_minor: h.co_borrower_charges_minor,
        excluded_loan_ids: h.excluded_loan_ids.clone(),
        selected_simulation_id: None,
        status: None,
        note: None,
    };
    let application = save_application(db, None, &input, today).await?;
    sim.application_id = Some(application.id);
    db::credit::upsert_simulation(&db.write, &sim).await?;
    input.selected_simulation_id = Some(sim.id);
    save_application(db, Some(application.id), &input, today).await
}

/// Crée une demande depuis la capacité d'achat (R-CRE-36) : une simulation au capital empruntable, aux conditions
/// des hypothèses, puis la demande préparée depuis cette simulation.
pub async fn application_from_capacity(db: &Db, name: &str, today: Date) -> AppResult<ApplicationView> {
    let capacity = purchase_capacity(db, today).await?;
    let h = &capacity.hypotheses;
    if capacity.today.capital_minor <= 0 {
        return Err(AppError::invalid("NO_BORROWING_CAPACITY", "aucun capital empruntable avec ces hypothèses"));
    }
    let simulation = SimulationInput {
        name: "Capacité d'achat".to_owned(),
        kind: LoanKind::Mortgage,
        principal_minor: capacity.today.capital_minor,
        annual_rate_ppm: h.annual_rate_ppm,
        duration_months: h.duration_months,
        insurance: h.insurance,
        fees_minor: h.fees_minor,
        guarantee_fees_minor: h.guarantee_fees_minor,
        institution_id: None,
        application_id: None,
        note: None,
    };
    let simulation = save_simulation(db, None, &simulation, today).await?;
    application_from_simulation(db, simulation.id, name, today).await
}

// ======================================================================= Export (R-CRE-19)

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn euros(minor: i64) -> String {
    let negative = minor < 0;
    let minor = minor.unsigned_abs();
    let units = (minor / 100).to_string();
    let mut grouped = String::new();
    for (i, digit) in units.chars().enumerate() {
        if i > 0 && (units.len() - i).is_multiple_of(3) {
            grouped.push('\u{202f}');
        }
        grouped.push(digit);
    }
    format!("{}{grouped},{:02}\u{a0}€", if negative { "−" } else { "" }, minor % 100)
}

fn percent(ppm: i64) -> String {
    format!("{},{:02}\u{a0}%", ppm / 10_000, (ppm % 10_000 + 50) / 100)
}

const PROJECT_LABELS: [(ProjectKind, &str); 5] = [
    (ProjectKind::ExistingHome, "Achat dans l'ancien"),
    (ProjectKind::NewHome, "Achat dans le neuf"),
    (ProjectKind::Works, "Travaux"),
    (ProjectKind::Vehicle, "Véhicule"),
    (ProjectKind::Other, "Autre projet"),
];

/// Synthèse de la demande, en HTML autonome (R-CRE-19).
fn summary_html(view: &ApplicationView, today: Date) -> String {
    let project = PROJECT_LABELS.iter().find(|(k, _)| *k == view.project_kind).map_or("", |(_, l)| l);
    let row = |label: &str, value: String| format!("<tr><th>{}</th><td>{}</td></tr>", escape(label), escape(&value));
    let mut html = format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><title>{name}</title><style>\
         body{{font-family:system-ui,sans-serif;max-width:48rem;margin:2rem auto;padding:0 1rem;color:#222}}\
         table{{border-collapse:collapse;width:100%;margin-bottom:1.5rem}}th,td{{text-align:left;padding:.3rem .5rem;\
         border-bottom:1px solid #ddd}}td{{text-align:right}}h2{{margin-top:2rem}}.warn{{color:#b45309}}\
         </style></head><body><h1>{name}</h1><p>{project} — synthèse du {today}</p>",
        name = escape(&view.name),
        project = escape(project),
        today = today.format("%d/%m/%Y"),
    );
    html.push_str("<h2>Plan de financement</h2><table>");
    html.push_str(&row("Prix", euros(view.price_minor)));
    html.push_str(&row("Frais de notaire", euros(view.notary_fees_minor)));
    html.push_str(&row("Travaux", euros(view.works_minor)));
    html.push_str(&row("Autres frais", euros(view.other_costs_minor)));
    html.push_str(&row("Coût du projet", euros(view.project_cost_minor)));
    html.push_str(&row("Apport", euros(view.down_payment_minor)));
    html.push_str(&row("Montant à emprunter", euros(view.to_borrow_minor)));
    html.push_str("</table><h2>Revenus et charges mensuels</h2><table>");
    for e in &view.capacity.employments {
        html.push_str(&row(&format!("Salaire — {}", e.employer), euros(e.monthly_minor)));
    }
    if view.other_income_minor > 0 {
        html.push_str(&row("Autres revenus", euros(view.other_income_minor)));
    }
    if let Some(name) = &view.co_borrower_name {
        html.push_str(&row(&format!("Revenus du co-emprunteur ({name})"), euros(view.co_borrower_income_minor)));
    }
    html.push_str(&row("Total des revenus", euros(view.capacity.income_minor)));
    for l in view.capacity.loans.iter().filter(|l| !l.excluded) {
        html.push_str(&row(&format!("Prêt en cours — {}", l.name), l.monthly_minor.map_or("—".into(), euros)));
    }
    if view.other_charges_minor > 0 {
        html.push_str(&row("Autres charges", euros(view.other_charges_minor)));
    }
    if view.co_borrower_name.is_some() {
        html.push_str(&row("Charges du co-emprunteur", euros(view.co_borrower_charges_minor)));
    }
    html.push_str(&row("Total des charges", euros(view.capacity.charges_minor)));
    html.push_str("</table><h2>Capacité</h2><table>");
    if let Some(selected) = view.simulations.iter().find(|s| Some(s.id) == view.selected_simulation_id) {
        html.push_str(&row(
            "Crédit envisagé",
            format!(
                "{} sur {} mois à {}",
                euros(selected.principal_minor),
                selected.duration_months,
                percent(selected.annual_rate_ppm)
            ),
        ));
        html.push_str(&row("Mensualité, assurance comprise", euros(selected.result.total_payment_minor)));
        html.push_str(&row("TAEG", percent(selected.result.apr_ppm)));
        html.push_str(&row("Coût total du crédit", euros(selected.result.total_cost_minor)));
    }
    if let Some(ratio) = view.capacity.debt_ratio_ppm {
        html.push_str(&row("Taux d'endettement", percent(ratio)));
    }
    html.push_str(&row("Reste à vivre", euros(view.capacity.remaining_income_minor)));
    html.push_str(&row("Épargne restante après l'apport", euros(view.capacity.savings_after_minor)));
    html.push_str("</table><h2>Pièces</h2><table>");
    for p in &view.pieces {
        let state = match p.status {
            PieceStatus::Provided => "fournie".to_owned(),
            PieceStatus::Incomplete => format!("incomplète ({}/{})", p.documents.len(), p.expected),
            PieceStatus::Missing => "manquante".to_owned(),
            PieceStatus::NotRequired => "non requise".to_owned(),
        };
        html.push_str(&row(&p.label, state));
    }
    html.push_str("</table></body></html>");
    html
}

/// Archive ZIP du dossier (R-CRE-19) : un dossier par pièce fournie, et la synthèse ; renvoie le nom du fichier et
/// son contenu.
pub async fn export(db: &Db, data: &DataDir, id: LoanApplicationId, today: Date) -> AppResult<(String, Vec<u8>)> {
    let view = application(db, id, today).await?;
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut written: HashSet<String> = HashSet::new();
    for (index, piece) in view.pieces.iter().enumerate() {
        let folder = format!("{:02} {}", index + 1, piece.label.replace(['/', '\\'], "-"));
        for document in &piece.documents {
            let content = match super::documents::read(data, document.id) {
                Ok(content) => content,
                Err(AppError::NotFound(_)) => continue,
                Err(error) => return Err(error),
            };
            let mut path = format!("{folder}/{}", document.file_name);
            let mut n = 2;
            while !written.insert(path.clone()) {
                path = format!("{folder}/{n} {}", document.file_name);
                n += 1;
            }
            archive.start_file(path, options).context("archive du dossier")?;
            archive.write_all(&content).context("archive du dossier")?;
        }
    }
    archive.start_file("Synthèse.html", options).context("archive du dossier")?;
    archive.write_all(summary_html(&view, today).as_bytes()).context("archive du dossier")?;
    let content = archive.finish().context("archive du dossier")?.into_inner();
    let name: String =
        view.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    Ok((format!("Dossier de crédit — {}.zip", name.trim()), content))
}

/// Synthèse HTML de la demande (R-CRE-19).
pub async fn summary(db: &Db, id: LoanApplicationId, today: Date) -> AppResult<String> {
    Ok(summary_html(&application(db, id, today).await?, today))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_are_written_in_french() {
        assert_eq!(euros(27_000_000), "270\u{202f}000,00\u{a0}€");
        assert_eq!(euros(115_920), "1\u{202f}159,20\u{a0}€");
        assert_eq!(euros(-5), "−0,05\u{a0}€");
        assert_eq!(percent(30_416), "3,04\u{a0}%");
        assert_eq!(percent(476_297), "47,63\u{a0}%");
        assert_eq!(escape("<b>A & B</b>"), "&lt;b&gt;A &amp; B&lt;/b&gt;");
    }
}
