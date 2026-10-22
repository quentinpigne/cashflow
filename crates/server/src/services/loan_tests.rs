//! Tests d'intégration des prêts (CPT-09, ADR-0020), sur une vraie base SQLite.

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::loans::Insurance;
use cashflow_domain::shared::ids::{AccountId, CategoryId};

use super::accounts;
use super::loans::{self, LoanPreviewInput, LoanTermsInput};
use super::tests::{account, category, d, setup};
use crate::db::Db;
use crate::error::AppError;

struct Fixture {
    loan: AccountId,
    checking: AccountId,
    interest: CategoryId,
    insurance: CategoryId,
}

async fn fixture(db: &Db) -> Fixture {
    let parent = category(db, "Prêt maison", CategoryKind::Expense, None).await;
    Fixture {
        loan: account(db, "Prêt", AccountKind::Mortgage, -20_000_000).await,
        checking: account(db, "Courant", AccountKind::Checking, 100_000).await,
        interest: category(db, "Intérêts du prêt", CategoryKind::Expense, Some(parent)).await,
        insurance: category(db, "Assurance du prêt", CategoryKind::Expense, Some(parent)).await,
    }
}

/// Prêt de la spec (CPT-09) : 200 000 € à 3 % sur 240 mois, assurance 0,30 % du capital.
fn input(f: &Fixture) -> LoanTermsInput {
    LoanTermsInput {
        principal_minor: 20_000_000,
        annual_rate_ppm: 30_000,
        duration_months: 240,
        first_installment_date: d("2026-01-05"),
        insurance: Insurance::RateOnPrincipal { rate_ppm: 3_000 },
        payer_account_id: f.checking,
        interest_category_id: f.interest,
        insurance_category_id: Some(f.insurance),
    }
}

fn code(error: AppError) -> String {
    match error {
        AppError::Invalid { code, .. } => code.to_owned(),
        AppError::Domain(e) => e.code().to_owned(),
        other => format!("{other:?}"),
    }
}

#[tokio::test]
async fn cpt_09_schedule_and_outstanding_principal() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let view = loans::save_terms(&db, f.loan, &input(&f), d("2026-03-31")).await.unwrap();
    assert_eq!(view.terms.monthly_payment_minor, 110_920);
    assert_eq!(view.terms.monthly_insurance_minor, 5_000);
    assert_eq!(view.installments.len(), 240);
    assert_eq!(view.installments[0].total_minor, 115_920);
    // Trois échéances passées au 31 mars ; le compte n'a encore reçu aucun remboursement.
    assert_eq!(view.next_installment, Some(4));
    assert_eq!(view.outstanding_minor, 20_000_000);
    assert_eq!(view.theoretical_outstanding_minor, view.installments[2].remaining_minor);
    assert_eq!(view.end_date, d("2045-12-05"));
    assert_eq!(view.total_insurance_minor, 240 * 5_000);
    // Les conditions se remplacent.
    let mut changed = input(&f);
    changed.insurance = Insurance::None;
    changed.insurance_category_id = None;
    let view = loans::save_terms(&db, f.loan, &changed, d("2026-03-31")).await.unwrap();
    assert_eq!(view.installments[0].total_minor, 110_920);
}

#[tokio::test]
async fn r_cpt_15_terms_are_checked() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let today = d("2026-03-31");
    let err = loans::save_terms(&db, f.checking, &input(&f), today).await.unwrap_err();
    assert_eq!(code(err), "NOT_A_LOAN");
    let mut bad = input(&f);
    bad.duration_months = 0;
    assert_eq!(code(loans::save_terms(&db, f.loan, &bad, today).await.unwrap_err()), "INVALID_LOAN_TERMS");
    let mut bad = input(&f);
    bad.payer_account_id = f.loan;
    assert_eq!(code(loans::save_terms(&db, f.loan, &bad, today).await.unwrap_err()), "INVALID_PAYER");
    let mut bad = input(&f);
    bad.insurance_category_id = None;
    assert_eq!(code(loans::save_terms(&db, f.loan, &bad, today).await.unwrap_err()), "INSURANCE_CATEGORY_REQUIRED");
    let income = category(&db, "Revenu test", CategoryKind::Income, None).await;
    let mut bad = input(&f);
    bad.interest_category_id = income;
    assert_eq!(code(loans::save_terms(&db, f.loan, &bad, today).await.unwrap_err()), "NOT_AN_EXPENSE_CATEGORY");
    assert_eq!(code(loans::get(&db, f.loan, today).await.unwrap_err()), "NO_LOAN_TERMS");
}

#[tokio::test]
async fn r_cpt_15_payer_account_cannot_be_deleted_and_terms_can_be_removed() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    loans::save_terms(&db, f.loan, &input(&f), d("2026-03-31")).await.unwrap();
    assert_eq!(code(accounts::delete(&db, f.checking).await.unwrap_err()), "ACCOUNT_IS_LOAN_PAYER");
    loans::delete_terms(&db, f.loan).await.unwrap();
    accounts::delete(&db, f.checking).await.unwrap();
    // Supprimer le prêt emporte ses conditions.
    loans::save_terms(
        &db,
        f.loan,
        &{
            let other = account(&db, "Autre courant", AccountKind::Checking, 0).await;
            LoanTermsInput { payer_account_id: other, ..input(&f) }
        },
        d("2026-03-31"),
    )
    .await
    .unwrap();
    accounts::delete(&db, f.loan).await.unwrap();
    assert!(crate::db::loans::list(&db.read).await.unwrap().is_empty());
}

#[tokio::test]
async fn r_cpt_16_preview_while_typing() {
    let preview = loans::preview(
        &LoanPreviewInput {
            principal_minor: 20_000_000,
            annual_rate_ppm: 30_000,
            duration_months: 240,
            insurance: Insurance::Fixed { amount_minor: 4_500 },
        },
        d("2026-03-31"),
    )
    .unwrap();
    assert_eq!((preview.monthly_payment_minor, preview.monthly_total_minor), (110_920, 115_420));
}
