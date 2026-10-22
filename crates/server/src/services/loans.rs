//! Prêts : conditions et tableau d'amortissement (CPT-09, ADR-0020, R-CPT-15 et R-CPT-16).

use std::collections::{HashMap, HashSet};

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::ledger::TransactionOrigin;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::ledger::transfer;
use cashflow_domain::loans::{self, Installment, Insurance, LoanTerms, PaymentParts};
use cashflow_domain::shared::ids::{AccountId, CategoryId, TransactionId};
use cashflow_domain::{Date, DomainError, Money, Rate};
use chrono::Days;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use crate::db::loans::LoanTermsRow;
use crate::db::transactions::{NewSplit, NewTransaction, TransactionRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoanTermsInput {
    /// Capital emprunté, positif.
    pub principal_minor: i64,
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

impl LoanTermsInput {
    fn terms(&self) -> LoanTerms {
        LoanTerms {
            principal: Money::from_minor(self.principal_minor),
            annual_rate: Rate::from_ppm(self.annual_rate_ppm),
            duration_months: self.duration_months,
            first_installment: self.first_installment_date,
            insurance: self.insurance,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoanTermsView {
    pub principal_minor: i64,
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub first_installment_date: Date,
    pub insurance: Insurance,
    pub payer_account_id: AccountId,
    pub interest_category_id: CategoryId,
    pub insurance_category_id: Option<CategoryId>,
    /// Mensualité hors assurance (R-CPT-16).
    pub monthly_payment_minor: i64,
    pub monthly_insurance_minor: i64,
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
    /// Mensualité ventilée selon cette échéance (R-CPT-17).
    pub paid_transaction_id: Option<TransactionId>,
}

impl From<&Installment> for InstallmentView {
    fn from(i: &Installment) -> Self {
        Self {
            paid_transaction_id: None,
            number: i.number,
            date: i.date,
            payment_minor: i.payment.minor(),
            principal_minor: i.principal.minor(),
            interest_minor: i.interest.minor(),
            insurance_minor: i.insurance.minor(),
            total_minor: i.total().minor(),
            remaining_minor: i.remaining.minor(),
        }
    }
}

/// Fiche d'un prêt qui a des conditions (E3, CPT-09).
#[derive(Debug, Serialize, ToSchema)]
pub struct LoanView {
    pub terms: LoanTermsView,
    pub installments: Vec<InstallmentView>,
    /// Capital restant dû selon le compte (R-CPT-6), positif.
    pub outstanding_minor: i64,
    /// Capital restant dû selon le tableau, aujourd'hui (R-CPT-16), positif.
    pub theoretical_outstanding_minor: i64,
    /// Écart signalé sur la fiche : capital restant dû du compte − celui du tableau.
    pub gap_minor: i64,
    /// Coût du crédit : intérêts et assurance de toutes les échéances.
    pub total_interest_minor: i64,
    pub total_insurance_minor: i64,
    pub total_cost_minor: i64,
    pub end_date: Date,
    /// Prochaine échéance, après aujourd'hui.
    pub next_installment: Option<u32>,
}

/// Mensualité calculée pendant la saisie des conditions (E4).
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

fn terms_view(row: &LoanTermsRow) -> LoanTermsView {
    let t = &row.terms;
    LoanTermsView {
        principal_minor: t.principal.minor(),
        annual_rate_ppm: t.annual_rate.ppm(),
        duration_months: t.duration_months,
        first_installment_date: t.first_installment,
        insurance: t.insurance,
        payer_account_id: row.payer_account_id,
        interest_category_id: row.interest_category_id,
        insurance_category_id: row.insurance_category_id,
        monthly_payment_minor: t.monthly_payment().minor(),
        monthly_insurance_minor: t.monthly_insurance().minor(),
    }
}

async fn loan_account(db: &Db, account: AccountId) -> AppResult<db::accounts::AccountRow> {
    let row = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    if row.kind.family() != AccountFamily::Liability {
        return Err(AppError::invalid("NOT_A_LOAN", "seul un prêt a des conditions de prêt (R-CPT-15)"));
    }
    Ok(row)
}

/// Conditions et tableau d'amortissement d'un prêt.
pub async fn get(db: &Db, account: AccountId, today: Date) -> AppResult<LoanView> {
    loan_account(db, account).await?;
    let row = db::loans::find(&db.read, account)
        .await?
        .ok_or_else(|| AppError::invalid("NO_LOAN_TERMS", "ce prêt n'a pas de conditions (R-CPT-15)"))?;
    let schedule = loans::schedule(&row.terms)?;
    let outstanding = -Money::from_minor(super::accounts::get(db, account, today).await?.balance_minor);
    let theoretical = loans::outstanding_at(&row.terms, &schedule, today);
    let interest: Money = schedule.iter().map(|i| i.interest).sum();
    let insurance: Money = schedule.iter().map(|i| i.insurance).sum();
    let paid: HashMap<u32, TransactionId> = db::loans::payments(&db.read, account)
        .await?
        .into_iter()
        .map(|p| (p.installment_number, p.transaction_id))
        .collect();
    Ok(LoanView {
        terms: terms_view(&row),
        outstanding_minor: outstanding.minor(),
        theoretical_outstanding_minor: theoretical.minor(),
        gap_minor: (outstanding - theoretical).minor(),
        total_interest_minor: interest.minor(),
        total_insurance_minor: insurance.minor(),
        total_cost_minor: (interest + insurance).minor(),
        end_date: schedule.last().map_or(row.terms.first_installment, |i| i.date),
        next_installment: schedule.iter().find(|i| i.date > today).map(|i| i.number),
        installments: schedule
            .iter()
            .map(|i| InstallmentView { paid_transaction_id: paid.get(&i.number).copied(), ..InstallmentView::from(i) })
            .collect(),
    })
}

/// Crée ou remplace les conditions d'un prêt (R-CPT-15).
pub async fn save_terms(db: &Db, account: AccountId, input: &LoanTermsInput, today: Date) -> AppResult<LoanView> {
    let loan = loan_account(db, account).await?;
    let terms = input.terms();
    terms.validate()?;
    let payer = db::accounts::find(&db.read, input.payer_account_id).await?.ok_or(AppError::NotFound("compte"))?;
    if payer.kind.family() != AccountFamily::Cash || payer.currency != loan.currency {
        return Err(AppError::invalid_field(
            "INVALID_PAYER",
            "payer_account_id",
            "le compte payeur est un compte de liquidités, de même devise que le prêt (R-CPT-15)",
        ));
    }
    expense_category(db, input.interest_category_id, "interest_category_id").await?;
    match input.insurance_category_id {
        Some(category) => expense_category(db, category, "insurance_category_id").await?,
        None if terms.monthly_insurance().is_positive() => {
            return Err(AppError::invalid_field(
                "INSURANCE_CATEGORY_REQUIRED",
                "insurance_category_id",
                "une catégorie est requise pour l'assurance (R-CPT-15)",
            ));
        }
        None => {}
    }
    let row = LoanTermsRow {
        account_id: account,
        terms,
        payer_account_id: input.payer_account_id,
        interest_category_id: input.interest_category_id,
        insurance_category_id: input.insurance_category_id,
    };
    db::loans::upsert(&db.write, &row).await?;
    get(db, account, today).await
}

async fn expense_category(db: &Db, id: CategoryId, field: &'static str) -> AppResult<()> {
    let category = db::categories::find(&db.read, id).await?.ok_or(AppError::NotFound("catégorie"))?;
    if category.kind != CategoryKind::Expense {
        return Err(AppError::invalid_field("NOT_AN_EXPENSE_CATEGORY", field, "une catégorie de dépense est attendue"));
    }
    Ok(())
}

pub async fn delete_terms(db: &Db, account: AccountId) -> AppResult<()> {
    loan_account(db, account).await?;
    db::loans::delete(&db.write, account).await?;
    Ok(())
}

/// Mensualité d'après des conditions en cours de saisie (R-CPT-16).
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

// ---------------------------------------------------------------- Ventilation des mensualités (CPT-10, R-CPT-17)

/// Un débit du compte payeur qui correspond à une échéance pas encore payée (R-CPT-17).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoanPaymentProposal {
    pub loan_account_id: AccountId,
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

/// Propositions pour un prêt : chaque débit du compte payeur, du plus ancien au plus récent, associé à l'échéance
/// non payée qui lui correspond (R-CPT-17), parmi `only` s'il est donné.
async fn proposals_for(
    db: &Db,
    row: &LoanTermsRow,
    loan_name: &str,
    today: Date,
    only: Option<&HashSet<TransactionId>>,
) -> AppResult<Vec<LoanPaymentProposal>> {
    let schedule = loans::schedule(&row.terms)?;
    let mut paid: Vec<u32> =
        db::loans::payments(&db.read, row.account_id).await?.iter().map(|p| p.installment_number).collect();
    let tolerance = Days::new(u64::try_from(loans::DATE_TOLERANCE_DAYS).unwrap_or(5));
    let from = row.terms.first_installment.checked_sub_days(tolerance).unwrap_or(row.terms.first_installment);
    let mut result = Vec::new();
    for debit in db::loans::payable_debits(&db.read, row.payer_account_id, from, today).await? {
        if only.is_some_and(|ids| !ids.contains(&debit.id)) {
            continue;
        }
        let Some(installment) = loans::matching_installment(&schedule, &paid, debit.amount, debit.date) else {
            continue;
        };
        let parts = loans::payment_parts(installment, debit.amount)?;
        paid.push(installment.number);
        result.push(LoanPaymentProposal {
            loan_account_id: row.account_id,
            loan_name: loan_name.to_owned(),
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

/// Mensualités déjà enregistrées qui correspondent à des échéances non payées, à ventiler en une fois (E3).
pub async fn proposals(db: &Db, account: AccountId, today: Date) -> AppResult<Vec<LoanPaymentProposal>> {
    let loan = loan_account(db, account).await?;
    let row = db::loans::find(&db.read, account)
        .await?
        .ok_or_else(|| AppError::invalid("NO_LOAN_TERMS", "ce prêt n'a pas de conditions (R-CPT-15)"))?;
    proposals_for(db, &row, &loan.name, today, None).await
}

/// Propositions parmi des transactions données, tous prêts confondus : fin d'import (R-IMP-22), fiche d'une
/// transaction.
pub async fn proposals_among(db: &Db, transactions: &[TransactionId]) -> AppResult<Vec<LoanPaymentProposal>> {
    let wanted: HashSet<TransactionId> = transactions.iter().copied().collect();
    // Toutes dates, même futures : les transactions sont déjà choisies.
    let until = Date::from_ymd_opt(9999, 12, 31).ok_or(AppError::NotFound("date"))?;
    let mut result = Vec::new();
    for row in db::loans::list(&db.read).await? {
        let Some(loan) = db::accounts::find(&db.read, row.account_id).await? else { continue };
        result.extend(proposals_for(db, &row, &loan.name, until, Some(&wanted)).await?);
    }
    Ok(result)
}

/// Ventile des mensualités selon leurs échéances (R-CPT-17), en bloc : toutes ou aucune.
pub async fn pay(db: &Db, account: AccountId, inputs: &[LoanPaymentInput]) -> AppResult<LoanPaymentResult> {
    let loan = loan_account(db, account).await?;
    let row = db::loans::find(&db.read, account)
        .await?
        .ok_or_else(|| AppError::invalid("NO_LOAN_TERMS", "ce prêt n'a pas de conditions (R-CPT-15)"))?;
    let schedule = loans::schedule(&row.terms)?;
    let mut paid: Vec<u32> =
        db::loans::payments(&db.read, account).await?.iter().map(|p| p.installment_number).collect();
    let mut tx = db.write.begin().await?;
    for input in inputs {
        let installment =
            schedule.iter().find(|i| i.number == input.installment_number).ok_or(AppError::NotFound("échéance"))?;
        if paid.contains(&installment.number) {
            return Err(DomainError::InstallmentAlreadyPaid.into());
        }
        let debit =
            db::transactions::find(&mut *tx, input.transaction_id).await?.ok_or(AppError::NotFound("transaction"))?;
        if debit.account_id != row.payer_account_id || !debit.amount.is_negative() {
            return Err(AppError::invalid(
                "NOT_A_PAYER_DEBIT",
                "une mensualité est un débit du compte payeur du prêt (R-CPT-17)",
            ));
        }
        if db::transactions::transfer_of(&mut *tx, debit.id).await?.is_some()
            || db::loans::payment_involving(&mut *tx, debit.id).await?.is_some()
        {
            return Err(DomainError::AlreadyTransferLeg.into());
        }
        let parts = loans::payment_parts(installment, debit.amount)?;
        pay_one(&mut tx, &loan, &row, installment.number, &debit, parts).await?;
        paid.push(installment.number);
    }
    tx.commit().await?;
    Ok(LoanPaymentResult { paid: inputs.len() })
}

/// Ventile un débit : virement du capital vers le prêt, intérêts et assurance en dépenses (R-CPT-17).
async fn pay_one(
    tx: &mut SqliteConnection,
    loan: &db::accounts::AccountRow,
    row: &LoanTermsRow,
    number: u32,
    debit: &TransactionRow,
    parts: PaymentParts,
) -> AppResult<()> {
    let mut splits = Vec::new();
    for (amount, category) in [
        (parts.principal, None),
        (parts.interest, Some(row.interest_category_id)),
        (parts.insurance, row.insurance_category_id),
    ] {
        if amount.is_positive() {
            splits.push(NewSplit { amount: -amount, category_id: category, note: None, project_id: None });
        }
    }
    db::transactions::replace_splits(&mut *tx, debit.id, &splits).await?;
    let mut touched = vec![debit.id];
    if parts.principal.is_positive() {
        // La jambe du capital, sur le prêt : pointée d'office, comme toute jambe sur un compte valorisé (R-TRX-36).
        super::transactions::period(loan).check(debit.date)?;
        let leg = TransactionId::new();
        let label = format!("Échéance n° {number}");
        let new = NewTransaction {
            id: leg,
            account_id: loan.id,
            date: debit.date,
            bank_date: None,
            value_date: None,
            amount: parts.principal,
            bank_label: None,
            label: &label,
            note: None,
            clearing_status: transfer::opposite_leg_status(loan.kind.family()),
            origin: TransactionOrigin::Manual,
            recurring_rule_id: None,
            fingerprint: None,
        };
        let single = [NewSplit { amount: parts.principal, category_id: None, note: None, project_id: None }];
        super::transactions::insert_validated(&mut *tx, &new, &single).await?;
        let outflow = db::transactions::splits_of(&mut *tx, debit.id)
            .await?
            .into_iter()
            .find(|s| s.category_id.is_none())
            .ok_or(AppError::NotFound("ventilation du capital"))?;
        let inflow = db::transactions::splits_of(&mut *tx, leg)
            .await?
            .into_iter()
            .next()
            .ok_or(AppError::NotFound("ventilation de la jambe"))?;
        db::transactions::insert_transfer(&mut *tx, outflow.id, inflow.id, None).await?;
        touched.push(leg);
    }
    db::loans::insert_payment(&mut *tx, row.account_id, number, debit.id).await?;
    super::check_split_sums(&mut *tx, &touched).await?;
    Ok(())
}

/// Annule la ventilation d'une échéance : la jambe sur le prêt est supprimée, la mensualité redevient une dépense
/// non catégorisée.
pub async fn unpay(db: &Db, account: AccountId, number: u32) -> AppResult<()> {
    loan_account(db, account).await?;
    let payment = db::loans::payments(&db.read, account)
        .await?
        .into_iter()
        .find(|p| p.installment_number == number)
        .ok_or(AppError::NotFound("échéance payée"))?;
    let debit =
        db::transactions::find(&db.read, payment.transaction_id).await?.ok_or(AppError::NotFound("transaction"))?;
    let mut tx = db.write.begin().await?;
    if let Some(leg) = payment.leg_transaction_id {
        // Supprimer la jambe emporte le virement et la ventilation qu'il reliait.
        db::transactions::delete(&mut *tx, leg).await?;
    }
    db::loans::delete_payment(&mut *tx, debit.id).await?;
    let single = [NewSplit { amount: debit.amount, category_id: None, note: None, project_id: None }];
    db::transactions::replace_splits(&mut tx, debit.id, &single).await?;
    super::check_split_sums(&mut tx, &[debit.id]).await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------- Projections (R-PRJ-10, R-PRJ-40)

/// Une échéance à venir, vue du compte payeur.
pub struct UpcomingDebit {
    pub date: Date,
    /// Montant prélevé (mensualité + assurance), négatif.
    pub amount: Money,
    pub label: String,
}

/// Échéances non payées des prêts payés par ce compte, datées après `after` et au plus tard `until` (R-PRJ-10).
pub async fn upcoming_debits(db: &Db, payer: AccountId, after: Date, until: Date) -> AppResult<Vec<UpcomingDebit>> {
    let mut result = Vec::new();
    for row in db::loans::list(&db.read).await?.into_iter().filter(|r| r.payer_account_id == payer) {
        let Some(loan) = db::accounts::find(&db.read, row.account_id).await? else { continue };
        let paid: HashSet<u32> =
            db::loans::payments(&db.read, row.account_id).await?.iter().map(|p| p.installment_number).collect();
        for i in loans::schedule(&row.terms)? {
            if i.date > after && i.date <= until && !paid.contains(&i.number) {
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
