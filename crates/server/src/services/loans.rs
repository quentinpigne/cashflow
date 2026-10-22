//! Prêts : conditions et tableau d'amortissement (CPT-09, ADR-0020, R-CPT-15 et R-CPT-16).

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::loans::{self, Installment, Insurance, LoanTerms};
use cashflow_domain::shared::ids::{AccountId, CategoryId};
use cashflow_domain::{Date, DomainError, Money, Rate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::loans::LoanTermsRow;
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
}

impl From<&Installment> for InstallmentView {
    fn from(i: &Installment) -> Self {
        Self {
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
        installments: schedule.iter().map(InstallmentView::from).collect(),
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
