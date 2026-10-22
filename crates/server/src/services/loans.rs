//! Prêts : fiche, suivi avec tableau d'amortissement ou simple, ventilation des mensualités (spec 09, ADR-0019,
//! ADR-0020).

use std::collections::{HashMap, HashSet};

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::loans::{
    self, Installment, Insurance, LoanKind, LoanMode, LoanPeriod, LoanTerms, PaymentParts, Tracking,
};
use cashflow_domain::shared::ids::{AccountId, AssetId, CategoryId, InstitutionId, LoanId, TransactionId};
use cashflow_domain::{Currency, Date, DomainError, Money, Rate};
use chrono::Days;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::clean;
use crate::db::loans::{BalanceRow, LoanRow, ScheduleRow};
use crate::db::transactions::NewSplit;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------- Registre

/// Tous les prêts, leurs tableaux et leurs capitaux restants dus saisis, chargés une fois.
pub struct LoanBook {
    pub loans: Vec<LoanRow>,
    schedules: HashMap<LoanId, Vec<Installment>>,
    balances: HashMap<LoanId, Vec<(Date, Money)>>,
}

impl LoanBook {
    pub async fn load(db: &Db) -> AppResult<Self> {
        let loans = db::loans::list(&db.read).await?;
        let mut schedules = HashMap::new();
        for loan in &loans {
            if let Some(s) = &loan.schedule {
                schedules.insert(loan.id, loans::schedule(&s.terms)?);
            }
        }
        let mut balances: HashMap<LoanId, Vec<(Date, Money)>> = HashMap::new();
        for b in db::loans::balances(&db.read).await? {
            balances.entry(b.loan_id).or_default().push((b.date, b.outstanding));
        }
        Ok(Self { loans, schedules, balances })
    }

    pub fn schedule(&self, loan: LoanId) -> &[Installment] {
        self.schedules.get(&loan).map(Vec::as_slice).unwrap_or_default()
    }

    /// Capital restant dû à une date (R-PRE-4), positif ; rien avant le déblocage ni une fois soldé.
    pub fn outstanding(&self, loan: &LoanRow, date: Date) -> Option<Money> {
        let period = LoanPeriod { principal: loan.principal, start_date: loan.start_date, closed_on: loan.closed_on };
        let tracking = match loan.schedule {
            Some(_) => Tracking::Schedule(self.schedule(loan.id)),
            None => Tracking::Manual(self.balances.get(&loan.id).map(Vec::as_slice).unwrap_or_default()),
        };
        period.outstanding_at(tracking, date)
    }
}

// ---------------------------------------------------------------- Vues

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ScheduleInput {
    /// Taux nominal annuel hors assurance, en millionièmes.
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub first_installment_date: Date,
    pub insurance: Insurance,
    /// Compte de liquidités qui paie les mensualités.
    pub payer_account_id: AccountId,
    pub interest_category_id: CategoryId,
    /// Requise si l'assurance n'est pas nulle.
    pub insurance_category_id: Option<CategoryId>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LoanInput {
    pub name: String,
    pub kind: LoanKind,
    pub institution_id: Option<InstitutionId>,
    /// Date de déblocage.
    pub start_date: Date,
    /// Capital emprunté, positif.
    pub principal_minor: i64,
    /// Conditions si le prêt est suivi avec tableau ; absent en suivi simple (R-PRE-1).
    pub schedule: Option<ScheduleInput>,
    pub asset_id: Option<AssetId>,
    pub closed_on: Option<Date>,
    pub include_in_net_worth: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NextInstallment {
    pub number: u32,
    pub date: Date,
    pub total_minor: i64,
}

/// Un prêt dans la liste (E27) et sur l'accueil.
#[derive(Debug, Serialize, ToSchema)]
pub struct LoanSummary {
    pub id: LoanId,
    pub name: String,
    pub kind: LoanKind,
    pub institution_name: Option<String>,
    pub mode: LoanMode,
    pub principal_minor: i64,
    /// Capital restant dû aujourd'hui (R-PRE-4) ; absent s'il n'est pas encore débloqué ou déjà soldé.
    pub outstanding_minor: Option<i64>,
    pub next_installment: Option<NextInstallment>,
    pub closed_on: Option<Date>,
    pub include_in_net_worth: bool,
    pub asset_id: Option<AssetId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InstallmentView {
    pub number: u32,
    pub date: Date,
    /// Mensualité hors assurance.
    pub payment_minor: i64,
    pub principal_minor: i64,
    pub interest_minor: i64,
    pub insurance_minor: i64,
    /// Montant prélevé : mensualité + assurance.
    pub total_minor: i64,
    /// Capital restant dû après l'échéance.
    pub remaining_minor: i64,
    /// Mensualité ventilée selon cette échéance (R-PRE-6).
    pub paid_transaction_id: Option<TransactionId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoanScheduleView {
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub first_installment_date: Date,
    pub insurance: Insurance,
    pub payer_account_id: AccountId,
    pub interest_category_id: CategoryId,
    pub insurance_category_id: Option<CategoryId>,
    /// Mensualité hors assurance (R-PRE-3).
    pub monthly_payment_minor: i64,
    pub monthly_insurance_minor: i64,
    pub total_interest_minor: i64,
    pub total_insurance_minor: i64,
    /// Coût du crédit : intérêts et assurance de toutes les échéances.
    pub total_cost_minor: i64,
    pub end_date: Date,
    pub installments: Vec<InstallmentView>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BalanceView {
    pub date: Date,
    pub outstanding_minor: i64,
    pub note: Option<String>,
}

/// Une transaction ventilée en remboursement de ce prêt (R-PRE-7).
#[derive(Debug, Serialize, ToSchema)]
pub struct RepaymentView {
    pub transaction_id: TransactionId,
    pub date: Date,
    pub label: String,
    /// Part remboursée, positive.
    pub amount_minor: i64,
}

/// Fiche d'un prêt (E28).
#[derive(Debug, Serialize, ToSchema)]
pub struct LoanView {
    pub summary: LoanSummary,
    pub institution_id: Option<InstitutionId>,
    pub start_date: Date,
    pub notes: Option<String>,
    pub asset_name: Option<String>,
    pub schedule: Option<LoanScheduleView>,
    /// Capitaux restants dus saisis (suivi simple), le plus récent d'abord.
    pub balances: Vec<BalanceView>,
    pub repayments: Vec<RepaymentView>,
}

/// Mensualité calculée pendant la saisie des conditions (E28).
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoanPreviewInput {
    pub principal_minor: i64,
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub insurance: Insurance,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoanPreview {
    pub monthly_payment_minor: i64,
    pub monthly_insurance_minor: i64,
    pub monthly_total_minor: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BalanceInput {
    pub date: Date,
    /// Capital restant dû, positif ou nul.
    pub outstanding_minor: i64,
    pub note: Option<String>,
}

// ---------------------------------------------------------------- Lecture

async fn institution_names(db: &Db) -> AppResult<HashMap<InstitutionId, String>> {
    Ok(db::institutions::list(&db.read).await?.into_iter().map(|i| (i.id, i.name)).collect())
}

fn summary(loan: &LoanRow, book: &LoanBook, institutions: &HashMap<InstitutionId, String>, today: Date) -> LoanSummary {
    let next = book
        .schedule(loan.id)
        .iter()
        .find(|i| i.date > today)
        .filter(|_| loan.closed_on.is_none())
        .map(|i| NextInstallment { number: i.number, date: i.date, total_minor: i.total().minor() });
    LoanSummary {
        id: loan.id,
        name: loan.name.clone(),
        kind: loan.kind,
        institution_name: loan.institution_id.and_then(|i| institutions.get(&i).cloned()),
        mode: loan.mode(),
        principal_minor: loan.principal.minor(),
        outstanding_minor: book.outstanding(loan, today).map(Money::minor),
        next_installment: next,
        closed_on: loan.closed_on,
        include_in_net_worth: loan.include_in_net_worth,
        asset_id: loan.asset_id,
    }
}

pub async fn list(db: &Db, today: Date) -> AppResult<Vec<LoanSummary>> {
    let book = LoanBook::load(db).await?;
    let institutions = institution_names(db).await?;
    Ok(book.loans.iter().map(|l| summary(l, &book, &institutions, today)).collect())
}

pub async fn get(db: &Db, id: LoanId, today: Date) -> AppResult<LoanView> {
    let book = LoanBook::load(db).await?;
    let loan = book.loans.iter().find(|l| l.id == id).ok_or(AppError::NotFound("prêt"))?;
    let institutions = institution_names(db).await?;
    let paid: HashMap<u32, TransactionId> = db::loans::payments(&db.read)
        .await?
        .into_iter()
        .filter(|p| p.loan_id == id)
        .map(|p| (p.installment_number, p.transaction_id))
        .collect();
    let schedule = loan.schedule.as_ref().map(|s| {
        let rows = book.schedule(id);
        let interest: Money = rows.iter().map(|i| i.interest).sum();
        let insurance: Money = rows.iter().map(|i| i.insurance).sum();
        LoanScheduleView {
            annual_rate_ppm: s.terms.annual_rate.ppm(),
            duration_months: s.terms.duration_months,
            first_installment_date: s.terms.first_installment,
            insurance: s.terms.insurance,
            payer_account_id: s.payer_account_id,
            interest_category_id: s.interest_category_id,
            insurance_category_id: s.insurance_category_id,
            monthly_payment_minor: s.terms.monthly_payment().minor(),
            monthly_insurance_minor: s.terms.monthly_insurance().minor(),
            total_interest_minor: interest.minor(),
            total_insurance_minor: insurance.minor(),
            total_cost_minor: (interest + insurance).minor(),
            end_date: rows.last().map_or(s.terms.first_installment, |i| i.date),
            installments: rows
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
                    paid_transaction_id: paid.get(&i.number).copied(),
                })
                .collect(),
        }
    });
    let balances = db::loans::balances(&db.read)
        .await?
        .into_iter()
        .rev()
        .filter(|b| b.loan_id == id)
        .map(|b| BalanceView { date: b.date, outstanding_minor: b.outstanding.minor(), note: b.note })
        .collect();
    let repayments = db::transactions::attached_splits(&db.read)
        .await?
        .into_iter()
        .filter(|s| s.loan_id == Some(id))
        .map(|s| RepaymentView {
            transaction_id: s.transaction_id,
            date: s.date,
            label: s.label,
            amount_minor: -s.amount.minor(),
        })
        .collect();
    let asset_name = match loan.asset_id {
        Some(asset) => db::assets::find(&db.read, asset).await?.map(|a| a.name),
        None => None,
    };
    Ok(LoanView {
        summary: summary(loan, &book, &institutions, today),
        institution_id: loan.institution_id,
        start_date: loan.start_date,
        notes: loan.notes.clone(),
        asset_name,
        schedule,
        balances,
        repayments,
    })
}

// ---------------------------------------------------------------- Écriture

async fn expense_category(db: &Db, id: CategoryId, field: &'static str) -> AppResult<()> {
    let category = db::categories::find(&db.read, id).await?.ok_or(AppError::NotFound("catégorie"))?;
    if category.kind != CategoryKind::Expense {
        return Err(AppError::invalid_field("NOT_AN_EXPENSE_CATEGORY", field, "une catégorie de dépense est attendue"));
    }
    Ok(())
}

/// Vérifie une saisie de prêt et la traduit en ligne (R-PRE-1, R-PRE-2).
async fn row_of(db: &Db, id: LoanId, input: &LoanInput) -> AppResult<LoanRow> {
    let name = super::required(&input.name, "name", 80)?;
    let principal = Money::from_minor(input.principal_minor);
    let period = LoanPeriod { principal, start_date: input.start_date, closed_on: input.closed_on };
    period.validate(input.schedule.as_ref().map(|s| s.first_installment_date))?;
    if let Some(institution) = input.institution_id
        && db::institutions::find(&db.read, institution).await?.is_none()
    {
        return Err(AppError::NotFound("établissement"));
    }
    if let Some(asset) = input.asset_id
        && db::assets::find(&db.read, asset).await?.is_none()
    {
        return Err(AppError::NotFound("bien"));
    }
    let schedule = match &input.schedule {
        None => None,
        Some(s) => {
            let terms = LoanTerms {
                principal,
                annual_rate: Rate::from_ppm(s.annual_rate_ppm),
                duration_months: s.duration_months,
                first_installment: s.first_installment_date,
                insurance: s.insurance,
            };
            terms.validate()?;
            let payer = db::accounts::find(&db.read, s.payer_account_id).await?.ok_or(AppError::NotFound("compte"))?;
            if payer.kind.family() != AccountFamily::Cash || payer.currency != Currency::EUR {
                return Err(AppError::invalid_field(
                    "INVALID_PAYER",
                    "payer_account_id",
                    "le compte payeur est un compte de liquidités en euros (R-PRE-2)",
                ));
            }
            expense_category(db, s.interest_category_id, "interest_category_id").await?;
            match s.insurance_category_id {
                Some(category) => expense_category(db, category, "insurance_category_id").await?,
                None if terms.monthly_insurance().is_positive() => {
                    return Err(AppError::invalid_field(
                        "INSURANCE_CATEGORY_REQUIRED",
                        "insurance_category_id",
                        "une catégorie est requise pour l'assurance (R-PRE-2)",
                    ));
                }
                None => {}
            }
            Some(ScheduleRow {
                terms,
                payer_account_id: s.payer_account_id,
                interest_category_id: s.interest_category_id,
                insurance_category_id: s.insurance_category_id,
            })
        }
    };
    Ok(LoanRow {
        id,
        name,
        kind: input.kind,
        institution_id: input.institution_id,
        start_date: input.start_date,
        principal,
        schedule,
        asset_id: input.asset_id,
        closed_on: input.closed_on,
        include_in_net_worth: input.include_in_net_worth.unwrap_or(true),
        notes: clean(input.notes.as_deref()),
    })
}

pub async fn create(db: &Db, input: &LoanInput, today: Date) -> AppResult<LoanView> {
    let id = LoanId::new();
    let row = row_of(db, id, input).await?;
    db::loans::upsert(&db.write, &row).await?;
    get(db, id, today).await
}

pub async fn update(db: &Db, id: LoanId, input: &LoanInput, today: Date) -> AppResult<LoanView> {
    db::loans::find(&db.read, id).await?.ok_or(AppError::NotFound("prêt"))?;
    let row = row_of(db, id, input).await?;
    db::loans::upsert(&db.write, &row).await?;
    get(db, id, today).await
}

/// Supprime un prêt : ses mensualités ventilées et ses remboursements redeviennent des sorties non catégorisées
/// (R-PRE-11).
pub async fn delete(db: &Db, id: LoanId) -> AppResult<()> {
    db::loans::find(&db.read, id).await?.ok_or(AppError::NotFound("prêt"))?;
    let payments: Vec<TransactionId> = db::loans::payments(&db.read)
        .await?
        .into_iter()
        .filter(|p| p.loan_id == id)
        .map(|p| p.transaction_id)
        .collect();
    let mut tx = db.write.begin().await?;
    for transaction in &payments {
        uncategorize(&mut tx, *transaction).await?;
    }
    db::transactions::detach_loan(&mut *tx, id).await?;
    db::loans::delete(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn save_balance(db: &Db, id: LoanId, input: &BalanceInput, today: Date) -> AppResult<LoanView> {
    let loan = db::loans::find(&db.read, id).await?.ok_or(AppError::NotFound("prêt"))?;
    if loan.schedule.is_some() {
        return Err(AppError::invalid(
            "LOAN_FOLLOWS_SCHEDULE",
            "le capital restant dû d'un prêt avec tableau suit le tableau (R-PRE-4)",
        ));
    }
    if input.outstanding_minor < 0 || input.date < loan.start_date {
        return Err(AppError::invalid_field(
            "INVALID_BALANCE",
            "outstanding_minor",
            "un capital restant dû est positif ou nul, daté après le déblocage (R-PRE-5)",
        ));
    }
    let row = BalanceRow {
        loan_id: id,
        date: input.date,
        outstanding: Money::from_minor(input.outstanding_minor),
        note: clean(input.note.as_deref()),
    };
    db::loans::upsert_balance(&db.write, &row).await?;
    get(db, id, today).await
}

pub async fn delete_balance(db: &Db, id: LoanId, date: Date) -> AppResult<()> {
    if db::loans::delete_balance(&db.write, id, date).await? == 0 {
        return Err(AppError::NotFound("capital restant dû"));
    }
    Ok(())
}

/// Mensualité d'après des conditions en cours de saisie (R-PRE-3).
pub fn preview(input: &LoanPreviewInput, today: Date) -> AppResult<LoanPreview> {
    let terms = LoanTerms {
        principal: Money::from_minor(input.principal_minor),
        annual_rate: Rate::from_ppm(input.annual_rate_ppm),
        duration_months: input.duration_months,
        first_installment: today,
        insurance: input.insurance,
    };
    terms.validate()?;
    let (payment, insurance) = (terms.monthly_payment(), terms.monthly_insurance());
    if payment.is_zero() {
        return Err(DomainError::InvalidLoanTerms.into());
    }
    Ok(LoanPreview {
        monthly_payment_minor: payment.minor(),
        monthly_insurance_minor: insurance.minor(),
        monthly_total_minor: (payment + insurance).minor(),
    })
}

// ---------------------------------------------------------------- Ventilation des mensualités (PRE-03, R-PRE-6)

/// Un débit du compte payeur qui correspond à une échéance pas encore payée (R-PRE-6).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoanPaymentProposal {
    pub loan_id: LoanId,
    pub loan_name: String,
    pub installment_number: u32,
    pub installment_date: Date,
    pub transaction_id: TransactionId,
    pub date: Date,
    pub label: String,
    /// Débit, négatif.
    pub amount_minor: i64,
    pub principal_minor: i64,
    pub interest_minor: i64,
    pub insurance_minor: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
pub struct LoanPaymentInput {
    pub transaction_id: TransactionId,
    pub installment_number: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoanPaymentResult {
    /// Mensualités ventilées.
    pub paid: usize,
}

/// Propositions pour un prêt avec tableau : chaque débit du compte payeur, du plus ancien au plus récent, associé à
/// l'échéance non payée qui lui correspond (R-PRE-6), parmi `only` s'il est donné.
async fn proposals_for(
    db: &Db,
    loan: &LoanRow,
    book: &LoanBook,
    until: Date,
    only: Option<&HashSet<TransactionId>>,
) -> AppResult<Vec<LoanPaymentProposal>> {
    let Some(s) = &loan.schedule else { return Ok(Vec::new()) };
    let schedule = book.schedule(loan.id);
    let mut paid: Vec<u32> = db::loans::payments(&db.read)
        .await?
        .iter()
        .filter(|p| p.loan_id == loan.id)
        .map(|p| p.installment_number)
        .collect();
    let tolerance = Days::new(u64::try_from(loans::DATE_TOLERANCE_DAYS).unwrap_or(5));
    let from = s.terms.first_installment.checked_sub_days(tolerance).unwrap_or(s.terms.first_installment);
    let mut result = Vec::new();
    for debit in db::loans::payable_debits(&db.read, s.payer_account_id, from, until).await? {
        if only.is_some_and(|ids| !ids.contains(&debit.id)) {
            continue;
        }
        let Some(installment) = loans::matching_installment(schedule, &paid, debit.amount, debit.date) else {
            continue;
        };
        let parts = loans::payment_parts(installment, debit.amount)?;
        paid.push(installment.number);
        result.push(LoanPaymentProposal {
            loan_id: loan.id,
            loan_name: loan.name.clone(),
            installment_number: installment.number,
            installment_date: installment.date,
            transaction_id: debit.id,
            date: debit.date,
            label: debit.label,
            amount_minor: debit.amount.minor(),
            principal_minor: parts.principal.minor(),
            interest_minor: parts.interest.minor(),
            insurance_minor: parts.insurance.minor(),
        });
    }
    Ok(result)
}

/// Mensualités déjà enregistrées qui correspondent à des échéances non payées, à ventiler en une fois (E28).
pub async fn proposals(db: &Db, id: LoanId, today: Date) -> AppResult<Vec<LoanPaymentProposal>> {
    let book = LoanBook::load(db).await?;
    let loan = book.loans.iter().find(|l| l.id == id).ok_or(AppError::NotFound("prêt"))?;
    proposals_for(db, loan, &book, today, None).await
}

/// Propositions parmi des transactions données, tous prêts confondus : fin d'import (R-IMP-22), fiche d'une
/// transaction. Aucune si la rubrique Prêts est désactivée (R-PRE-12).
pub async fn proposals_among(db: &Db, transactions: &[TransactionId]) -> AppResult<Vec<LoanPaymentProposal>> {
    if !super::features::get(db).await?.loans {
        return Ok(Vec::new());
    }
    let wanted: HashSet<TransactionId> = transactions.iter().copied().collect();
    // Toutes dates, même futures : les transactions sont déjà choisies.
    let until = Date::from_ymd_opt(9999, 12, 31).ok_or(AppError::NotFound("date"))?;
    let book = LoanBook::load(db).await?;
    let mut result = Vec::new();
    for loan in book.loans.iter().filter(|l| l.closed_on.is_none()) {
        result.extend(proposals_for(db, loan, &book, until, Some(&wanted)).await?);
    }
    Ok(result)
}

/// Ventile des mensualités selon leurs échéances (R-PRE-6), en bloc : toutes ou aucune.
pub async fn pay(db: &Db, id: LoanId, inputs: &[LoanPaymentInput]) -> AppResult<LoanPaymentResult> {
    let book = LoanBook::load(db).await?;
    let loan = book.loans.iter().find(|l| l.id == id).ok_or(AppError::NotFound("prêt"))?;
    let s = loan.schedule.as_ref().ok_or_else(|| {
        AppError::invalid("LOAN_WITHOUT_SCHEDULE", "seul un prêt avec tableau a des échéances (R-PRE-6)")
    })?;
    let schedule = book.schedule(id);
    let mut paid: Vec<u32> =
        db::loans::payments(&db.read).await?.iter().filter(|p| p.loan_id == id).map(|p| p.installment_number).collect();
    let mut tx = db.write.begin().await?;
    for input in inputs {
        let installment =
            schedule.iter().find(|i| i.number == input.installment_number).ok_or(AppError::NotFound("échéance"))?;
        if paid.contains(&installment.number) {
            return Err(DomainError::InstallmentAlreadyPaid.into());
        }
        let debit =
            db::transactions::find(&mut *tx, input.transaction_id).await?.ok_or(AppError::NotFound("transaction"))?;
        if debit.account_id != s.payer_account_id || !debit.amount.is_negative() {
            return Err(AppError::invalid(
                "NOT_A_PAYER_DEBIT",
                "une mensualité est un débit du compte payeur du prêt (R-PRE-6)",
            ));
        }
        if db::transactions::transfer_of(&mut *tx, debit.id).await?.is_some()
            || db::loans::payment_of(&mut *tx, debit.id).await?.is_some()
        {
            return Err(DomainError::AlreadyTransferLeg.into());
        }
        let parts = loans::payment_parts(installment, debit.amount)?;
        db::transactions::replace_splits(&mut tx, debit.id, &payment_splits(id, s, parts)).await?;
        db::loans::insert_payment(&mut *tx, id, installment.number, debit.id).await?;
        super::check_split_sums(&mut tx, &[debit.id]).await?;
        paid.push(installment.number);
    }
    tx.commit().await?;
    Ok(LoanPaymentResult { paid: inputs.len() })
}

/// Remboursement du capital, intérêts et assurance d'une mensualité, sans ventilation nulle (R-PRE-6).
fn payment_splits(loan: LoanId, s: &ScheduleRow, parts: PaymentParts) -> Vec<NewSplit<'static>> {
    [
        (parts.principal, NewSplit { loan_id: Some(loan), ..Default::default() }),
        (parts.interest, NewSplit { category_id: Some(s.interest_category_id), ..Default::default() }),
        (parts.insurance, NewSplit { category_id: s.insurance_category_id, ..Default::default() }),
    ]
    .into_iter()
    .filter(|(amount, _)| amount.is_positive())
    .map(|(amount, split)| NewSplit { amount: -amount, ..split })
    .collect()
}

/// Une mensualité redevient une sortie non catégorisée, libérée de son échéance.
async fn uncategorize(tx: &mut sqlx::SqliteConnection, transaction: TransactionId) -> AppResult<()> {
    let debit = db::transactions::find(&mut *tx, transaction).await?.ok_or(AppError::NotFound("transaction"))?;
    db::loans::delete_payment(&mut *tx, transaction).await?;
    db::transactions::replace_splits(&mut *tx, transaction, &[NewSplit { amount: debit.amount, ..Default::default() }])
        .await?;
    Ok(())
}

/// Annule la ventilation d'une échéance (R-PRE-8).
pub async fn unpay(db: &Db, id: LoanId, number: u32) -> AppResult<()> {
    let payment = db::loans::payments(&db.read)
        .await?
        .into_iter()
        .find(|p| p.loan_id == id && p.installment_number == number)
        .ok_or(AppError::NotFound("échéance payée"))?;
    let mut tx = db.write.begin().await?;
    uncategorize(&mut tx, payment.transaction_id).await?;
    super::check_split_sums(&mut tx, &[payment.transaction_id]).await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------- Projections (R-PRJ-10)

/// Une échéance à venir, vue du compte payeur.
pub struct UpcomingDebit {
    pub date: Date,
    /// Montant prélevé (mensualité + assurance), négatif.
    pub amount: Money,
    pub label: String,
}

/// Échéances non payées des prêts payés par ce compte, datées après `after` et au plus tard `until` (R-PRJ-10).
pub async fn upcoming_debits(db: &Db, payer: AccountId, after: Date, until: Date) -> AppResult<Vec<UpcomingDebit>> {
    let book = LoanBook::load(db).await?;
    let paid: HashSet<(LoanId, u32)> =
        db::loans::payments(&db.read).await?.iter().map(|p| (p.loan_id, p.installment_number)).collect();
    let mut result = Vec::new();
    for loan in book.loans.iter().filter(|l| l.schedule.as_ref().is_some_and(|s| s.payer_account_id == payer)) {
        for i in book.schedule(loan.id) {
            let closed = loan.closed_on.is_some_and(|c| i.date >= c);
            if i.date > after && i.date <= until && !closed && !paid.contains(&(loan.id, i.number)) {
                result.push(UpcomingDebit {
                    date: i.date,
                    amount: -i.total(),
                    label: format!("{} — échéance n° {}", loan.name, i.number),
                });
            }
        }
    }
    Ok(result)
}
