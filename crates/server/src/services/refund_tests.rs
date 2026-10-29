//! Tests d'intégration des remboursements de frais (TRX-12, ADR-0026), sur une vraie base SQLite.

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, CategoryId, TransactionId};

use super::refunds::{self, ExpectedRefundInput};
use super::tests::{account, category, d, domain_error, setup, split, tx};
use super::transactions::{self, SplitInput, TransactionInput, TransactionUpdateInput};
use crate::db::Db;

fn refund_split(amount: i64, category: CategoryId, expense: TransactionId) -> SplitInput {
    SplitInput { refunds_transaction_id: Some(expense), ..split(amount, Some(category)) }
}

async fn income(db: &Db, account: AccountId, date: &str, splits: Vec<SplitInput>) -> TransactionId {
    let amount = splits.iter().map(|s| s.amount_minor).sum();
    let input = TransactionInput { splits: Some(splits), ..tx(account, date, amount, None) };
    transactions::create(db, &input).await.unwrap()
}

async fn refunded(db: &Db, expense: TransactionId) -> i64 {
    transactions::get(db, expense).await.unwrap().refund.refunded_minor
}

async fn expenses(db: &Db, today: &str) -> i64 {
    super::dashboard::get(db, d(today)).await.unwrap().month.expenses_minor
}

#[tokio::test]
async fn trx_12_partial_refund_with_expected_amount() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let doctor = category(&db, "Médecin R", CategoryKind::Expense, None).await;
    let visit = transactions::create(&db, &tx(checking, "2026-03-02", -2_500, Some(doctor))).await.unwrap();
    refunds::set_expected(&db, visit, &ExpectedRefundInput { expected_minor: Some(1_750) }).await.unwrap();
    assert_eq!(refunds::pending(&db).await.unwrap().iter().map(|p| p.pending_minor).collect::<Vec<_>>(), [1_750]);

    // L'assurance maladie rembourse en avril : mars ne change pas, avril baisse (R-TRX-51).
    let march = expenses(&db, "2026-03-31").await;
    let cpam = income(&db, checking, "2026-04-03", vec![refund_split(1_750, doctor, visit)]).await;
    assert_eq!(expenses(&db, "2026-03-31").await, march);
    assert_eq!(expenses(&db, "2026-04-30").await, -1_750);

    let detail = transactions::get(&db, visit).await.unwrap();
    assert_eq!((detail.refund.refunded_minor, detail.refund.pending_minor), (1_750, 0));
    assert_eq!(detail.refund.received.iter().map(|r| r.transaction_id).collect::<Vec<_>>(), [cpam]);
    assert!(refunds::pending(&db).await.unwrap().is_empty(), "attente soldée (R-TRX-53)");
    assert_eq!(transactions::get(&db, cpam).await.unwrap().refund.refunds[0].transaction_id, visit);

    // R-TRX-52 : une autre entrée ne peut pas rembourser au-delà de la dépense.
    let mutuelle = TransactionInput {
        splits: Some(vec![refund_split(1_000, doctor, visit)]),
        ..tx(checking, "2026-04-05", 1_000, None)
    };
    assert_eq!(domain_error(transactions::create(&db, &mutuelle).await), DomainError::RefundExceedsExpense);
    // La dépense ne descend pas sous son remboursé, ni son attendu.
    let smaller = TransactionUpdateInput {
        account_id: checking,
        date: d("2026-03-02"),
        amount_minor: -1_500,
        label: "Test".into(),
        note: None,
        splits: None,
        payee_id: None,
    };
    assert_eq!(domain_error(transactions::update(&db, visit, &smaller).await), DomainError::RefundExceedsExpense);
    let too_much = refunds::set_expected(&db, visit, &ExpectedRefundInput { expected_minor: Some(3_000) }).await;
    assert_eq!(domain_error(too_much), DomainError::InvalidExpectedRefund);

    // Supprimer la dépense retire le lien ; la ventilation garde sa catégorie (R-TRX-55).
    transactions::delete(&db, &crate::config::DataDir::new("inutilisé"), visit, false, d("2026-03-15")).await.unwrap();
    let kept = &transactions::get(&db, cpam).await.unwrap().splits[0];
    assert_eq!((kept.refunds_transaction_id, kept.category_id), (None, Some(doctor)));
}

#[tokio::test]
async fn trx_12_one_income_refunds_several_expenses() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let phone = category(&db, "Téléphone R", CategoryKind::Expense, None).await;
    let fuel = category(&db, "Carburant R", CategoryKind::Expense, None).await;
    let salary = category(&db, "Salaire R", CategoryKind::Income, None).await;
    let plan = transactions::create(&db, &tx(checking, "2026-03-05", -3_999, Some(phone))).await.unwrap();
    let full = transactions::create(&db, &tx(checking, "2026-03-12", -6_840, Some(fuel))).await.unwrap();
    let before = expenses(&db, "2026-03-31").await;
    let expenses_note =
        income(&db, checking, "2026-03-28", vec![refund_split(2_000, phone, plan), refund_split(4_200, fuel, full)])
            .await;
    assert_eq!(expenses(&db, "2026-03-31").await, before - 6_200);
    assert_eq!((refunded(&db, plan).await, refunded(&db, full).await), (2_000, 4_200));

    // Propositions : déjà remboursées en partie, elles restent proposées pour le reste (R-TRX-54).
    let other = income(&db, checking, "2026-03-30", vec![split(5_000, Some(salary))]).await;
    let candidates = refunds::candidates(&db, other).await.unwrap();
    let full_candidate = candidates.iter().find(|c| c.transaction_id == full).unwrap();
    assert_eq!((full_candidate.refundable_minor, full_candidate.proposed_minor), (2_640, 2_640));
    assert_eq!(full_candidate.category_id, Some(fuel));

    // Modifier l'entrée sans renvoyer ses ventilations garde les liens.
    let renamed = TransactionUpdateInput {
        account_id: checking,
        date: d("2026-03-28"),
        amount_minor: 6_200,
        label: "Note de frais".into(),
        note: None,
        splits: None,
        payee_id: None,
    };
    transactions::update(&db, expenses_note, &renamed).await.unwrap();
    assert_eq!(refunded(&db, full).await, 4_200);

    // R-TRX-50 : une ventilation qui rembourse est positive, sur une catégorie de dépense.
    let on_income = TransactionInput {
        splits: Some(vec![refund_split(1_000, salary, plan)]),
        ..tx(checking, "2026-03-29", 1_000, None)
    };
    assert_eq!(domain_error(transactions::create(&db, &on_income).await), DomainError::InvalidRefund);
}
