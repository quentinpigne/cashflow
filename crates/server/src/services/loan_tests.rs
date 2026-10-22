//! Tests d'intégration des prêts (CPT-09, ADR-0020), sur une vraie base SQLite.

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::loans::Insurance;
use cashflow_domain::shared::ids::{AccountId, CategoryId};

use super::loans::{self, LoanPaymentInput, LoanPreviewInput, LoanTermsInput};
use super::tests::{account, category, d, setup, tx};
use super::{accounts, transactions};
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

// ---------------------------------------------------------------- Ventilation des mensualités (CPT-10)

async fn spend(db: &Db, account: AccountId, date: &str, amount: i64) -> cashflow_domain::shared::ids::TransactionId {
    transactions::create(db, &tx(account, date, amount, None)).await.unwrap()
}

async fn value(db: &Db, account: AccountId, at: &str) -> i64 {
    accounts::get(db, account, d(at)).await.unwrap().balance_minor
}

#[tokio::test]
async fn cpt_10_r_cpt_17_monthly_payments_are_split_into_principal_interest_and_insurance() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    loans::save_terms(&db, f.loan, &input(&f), d("2026-01-31")).await.unwrap();
    let first = spend(&db, f.checking, "2026-01-05", -115_920).await;
    // 40 centimes d'assurance en plus, payé deux jours plus tard : toujours reconnu (±1 €, ±5 jours).
    let second = spend(&db, f.checking, "2026-02-07", -115_960).await;
    spend(&db, f.checking, "2026-01-10", -5_000).await;

    let proposals = loans::proposals(&db, f.loan, d("2026-02-28")).await.unwrap();
    assert_eq!(
        proposals.iter().map(|p| (p.transaction_id, p.installment_number)).collect::<Vec<_>>(),
        [(first, 1), (second, 2)]
    );
    let inputs: Vec<LoanPaymentInput> = proposals
        .iter()
        .map(|p| LoanPaymentInput { transaction_id: p.transaction_id, installment_number: p.installment_number })
        .collect();
    assert_eq!(loans::pay(&db, f.loan, &inputs).await.unwrap().paid, 2);

    // Le capital rembourse la dette ; seuls intérêts et assurance sont des dépenses.
    let detail = transactions::get(&db, first).await.unwrap();
    let mut parts: Vec<(i64, bool)> = detail.splits.iter().map(|s| (s.amount_minor, s.category_id.is_some())).collect();
    parts.sort_unstable();
    assert_eq!(parts, [(-60_920, false), (-50_000, true), (-5_000, true)]);
    assert_eq!(value(&db, f.loan, "2026-01-31").await, -20_000_000 + 60_920);
    let january = super::dashboard::get(&db, d("2026-01-31")).await.unwrap();
    assert_eq!(january.month.expenses_minor, 55_000);
    let view = loans::get(&db, f.loan, d("2026-02-28")).await.unwrap();
    assert_eq!(view.installments[0].paid_transaction_id, Some(first));
    assert_eq!(view.gap_minor, 0);
    assert!(loans::proposals(&db, f.loan, d("2026-02-28")).await.unwrap().is_empty());
    assert_eq!(code(loans::pay(&db, f.loan, &inputs[..1]).await.unwrap_err()), "INSTALLMENT_ALREADY_PAID");
}

#[tokio::test]
async fn r_cpt_17_a_paid_installment_is_locked_until_its_split_is_undone() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    loans::save_terms(&db, f.loan, &input(&f), d("2026-01-31")).await.unwrap();
    let payment = spend(&db, f.checking, "2026-01-05", -115_920).await;
    let pay = [LoanPaymentInput { transaction_id: payment, installment_number: 1 }];
    loans::pay(&db, f.loan, &pay).await.unwrap();
    let detail = transactions::get(&db, payment).await.unwrap();
    let leg = detail.transfer.as_ref().unwrap().counterpart_transaction_id;

    // Le montant ne change plus ; le libellé si.
    let update = |amount: i64, label: &str| transactions::TransactionUpdateInput {
        account_id: f.checking,
        date: d("2026-01-05"),
        amount_minor: amount,
        label: label.into(),
        note: None,
        splits: None,
    };
    assert_eq!(
        code(transactions::update(&db, payment, &update(-120_000, "Échéance")).await.unwrap_err()),
        "INSTALLMENT_PAYMENT_LOCKED"
    );
    transactions::update(&db, payment, &update(-115_920, "Échéance de janvier")).await.unwrap();
    assert_eq!(transactions::get(&db, payment).await.unwrap().label, "Échéance de janvier");
    // La jambe sur le prêt ne se supprime pas seule ; la ventilation de virement reste sans catégorie.
    assert_eq!(code(transactions::delete(&db, leg).await.unwrap_err()), "INSTALLMENT_PAYMENT_LOCKED");
    let transfer_split = detail.splits.iter().find(|s| s.category_id.is_none()).unwrap().id;
    let categorize = transactions::CategorizeInput { category_id: Some(f.interest) };
    assert_eq!(
        code(transactions::categorize_split(&db, transfer_split, &categorize).await.unwrap_err()),
        "TRANSFER_LEG_CATEGORIZED"
    );

    let transfer = detail.transfer.as_ref().unwrap().transfer_id;
    assert_eq!(code(super::transfers::unlink(&db, transfer).await.unwrap_err()), "INSTALLMENT_PAYMENT_LOCKED");
    assert_eq!(detail.loan_payment.as_ref().map(|p| (p.installment_number, p.is_payment)), Some((1, true)));

    // Annuler la ventilation : la jambe disparaît, la mensualité redevient une sortie non catégorisée.
    loans::unpay(&db, f.loan, 1).await.unwrap();
    assert_eq!(value(&db, f.loan, "2026-01-31").await, -20_000_000);
    let detail = transactions::get(&db, payment).await.unwrap();
    assert_eq!(detail.splits.len(), 1);
    assert!(detail.transfer.is_none());
    assert_eq!(transactions::get(&db, leg).await.map(|_| ()).map_err(code), Err("NotFound(\"transaction\")".into()));

    // Supprimer une mensualité ventilée emporte la jambe du capital.
    loans::pay(&db, f.loan, &pay).await.unwrap();
    transactions::delete(&db, payment).await.unwrap();
    assert_eq!(value(&db, f.loan, "2026-01-31").await, -20_000_000);
    assert!(loans::get(&db, f.loan, d("2026-01-31")).await.unwrap().installments[0].paid_transaction_id.is_none());
}

// ---------------------------------------------------------------- Projections (R-PRJ-10, R-PRJ-40)

#[tokio::test]
async fn r_prj_10_payer_cash_projection_counts_each_installment_once() {
    use super::projections::{self, CashQuery};
    use super::recurring::{self, RecurringRuleInput};
    use cashflow_domain::recurrence::Frequency;

    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    loans::save_terms(&db, f.loan, &input(&f), d("2026-01-31")).await.unwrap();
    // Une récurrence du même montant que l'échéance, souvent saisie avant les conditions : ignorée.
    let duplicate = RecurringRuleInput {
        account_id: f.checking,
        target_account_id: None,
        label: "Prêt".into(),
        amount_minor: -115_920,
        amount_tolerance_minor: 0,
        frequency: Frequency::Monthly,
        interval: 1,
        anchor_date: d("2026-01-05"),
        end_date: None,
        split_template: Vec::new(),
        active: true,
    };
    recurring::save(&db, None, &duplicate, d("2026-01-31")).await.unwrap();
    let query = CashQuery { days: Some(60), variable: Some(false), threshold_minor: None };
    let view = projections::cash(&db, f.checking, &query, d("2026-01-31")).await.unwrap();
    let installments: Vec<(cashflow_domain::Date, i64)> =
        view.events.iter().filter(|e| e.amount_minor == -115_920).map(|e| (e.date, e.amount_minor)).collect();
    assert_eq!(installments, [(d("2026-02-05"), -115_920), (d("2026-03-05"), -115_920)]);
    assert!(view.events.iter().all(|e| e.label != "Prêt"));
}

#[tokio::test]
async fn r_prj_40_loan_projection_follows_its_schedule() {
    use super::projections::{self, ProjectionQuery};

    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    loans::save_terms(&db, f.loan, &input(&f), d("2026-01-01")).await.unwrap();
    let query = ProjectionQuery { scenario_id: None, months: Some(3) };
    let view = projections::account(&db, f.loan, &query, d("2026-01-01")).await.unwrap();
    let schedule = loans::get(&db, f.loan, d("2026-01-01")).await.unwrap().installments;
    // À chaque fin de mois, la dette vaut le capital restant dû du tableau après les échéances passées.
    assert_eq!(view.points.len(), 3);
    for point in &view.points {
        let remaining = schedule.iter().rfind(|i| i.date <= point.date).unwrap().remaining_minor;
        assert_eq!(point.value_minor, -remaining, "{}", point.date);
    }
}
