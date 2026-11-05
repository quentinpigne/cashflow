//! Tests d'intégration de la ventilation en lot (TRX-14, R-TRX-16), sur une vraie base SQLite.

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{CategoryId, TransactionId};

use super::tests::{account, category, d, domain_error, setup, tx};
use super::transactions::{self, BulkSplitInput, BulkSplitLine};
use super::transfers::{self, TransferInput};
use crate::db::Db;

fn percent(basis_points: u32, category: CategoryId) -> BulkSplitLine {
    BulkSplitLine {
        basis_points: Some(basis_points),
        amount_minor: None,
        category_id: Some(category),
        note: None,
        member_id: None,
        keep_member: true,
    }
}

fn amount(minor: i64, category: CategoryId) -> BulkSplitLine {
    BulkSplitLine { basis_points: None, amount_minor: Some(minor), ..percent(0, category) }
}

async fn splits(db: &Db, id: TransactionId) -> Vec<(i64, Option<CategoryId>)> {
    transactions::get(db, id).await.unwrap().splits.into_iter().map(|s| (s.amount_minor, s.category_id)).collect()
}

/// Un modèle en pourcentages suit le montant de chaque transaction ; les virements sont laissés tels quels.
#[tokio::test]
async fn trx_14_r_trx_16_percentages_follow_each_amount() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let livret = account(&db, "Livret", AccountKind::LivretA, 0).await;
    let food = category(&db, "Courses L", CategoryKind::Expense, None).await;
    let home = category(&db, "Maison L", CategoryKind::Expense, None).await;
    let big = transactions::create(&db, &tx(checking, "2026-03-02", -12_000, None)).await.unwrap();
    let small = transactions::create(&db, &tx(checking, "2026-03-03", -1_001, Some(home))).await.unwrap();
    let transfer = transfers::create(
        &db,
        &TransferInput {
            from_account_id: checking,
            to_account_id: livret,
            amount_minor: 5_000,
            date: d("2026-03-04"),
            label: "Épargne".into(),
            note: None,
        },
    )
    .await
    .unwrap();

    let input = BulkSplitInput {
        transaction_ids: vec![big, small, transfer.outflow_transaction_id],
        lines: vec![percent(6_000, food), percent(4_000, home)],
    };
    let result = transactions::split_many(&db, &input).await.unwrap();
    assert_eq!((result.updated, result.skipped), (2, 1));
    assert_eq!(splits(&db, big).await, [(-7_200, Some(food)), (-4_800, Some(home))]);
    // L'arrondi va sur la dernière ligne (R-TRX-14).
    assert_eq!(splits(&db, small).await, [(-601, Some(food)), (-400, Some(home))]);
    assert_eq!(splits(&db, transfer.outflow_transaction_id).await, [(-5_000, None)]);
}

/// Un modèle en montants ne s'applique qu'à des transactions du même montant : sinon, rien n'est modifié.
#[tokio::test]
async fn trx_14_r_trx_16_amounts_require_the_same_amount() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let phone = category(&db, "Téléphone L", CategoryKind::Expense, None).await;
    let internet = category(&db, "Internet L", CategoryKind::Expense, None).await;
    let march = transactions::create(&db, &tx(checking, "2026-03-05", -3_999, None)).await.unwrap();
    let april = transactions::create(&db, &tx(checking, "2026-04-05", -3_999, None)).await.unwrap();
    let lines = vec![amount(-1_999, phone), amount(-2_000, internet)];

    let input = BulkSplitInput { transaction_ids: vec![march, april], lines: lines.clone() };
    assert_eq!(transactions::split_many(&db, &input).await.unwrap().updated, 2);
    assert_eq!(splits(&db, april).await, [(-1_999, Some(phone)), (-2_000, Some(internet))]);

    let other = transactions::create(&db, &tx(checking, "2026-05-05", -4_500, None)).await.unwrap();
    let input = BulkSplitInput { transaction_ids: vec![march, other], lines: vec![amount(-2_000, phone)] };
    assert!(matches!(domain_error(transactions::split_many(&db, &input).await), DomainError::SplitSumMismatch { .. }));
    // Refusé en bloc : la première transaction n'a pas changé (R-TRX-10).
    assert_eq!(splits(&db, march).await, [(-1_999, Some(phone)), (-2_000, Some(internet))]);
}

/// Les pourcentages totalisent 100 %, sans mélanger pourcentages et montants.
#[tokio::test]
async fn r_trx_16_template_must_be_consistent() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Courses M", CategoryKind::Expense, None).await;
    let id = transactions::create(&db, &tx(checking, "2026-03-02", -1_000, None)).await.unwrap();
    for lines in [vec![percent(6_000, food), percent(3_000, food)], vec![percent(5_000, food), amount(-500, food)]] {
        let input = BulkSplitInput { transaction_ids: vec![id], lines };
        assert_eq!(domain_error(transactions::split_many(&db, &input).await), DomainError::InvalidSplitTemplate);
    }
    // Un centime ne se partage pas en deux : la transaction est ignorée (R-TRX-11).
    let cent = transactions::create(&db, &tx(checking, "2026-03-02", -1, None)).await.unwrap();
    let input = BulkSplitInput { transaction_ids: vec![cent], lines: vec![percent(5_000, food), percent(5_000, food)] };
    assert_eq!(transactions::split_many(&db, &input).await.unwrap().skipped, 1);
    // Rapprochée, une transaction se ventile toujours (R-TRX-3).
    sqlx::query("UPDATE ledger_transaction SET clearing_status = 'RECONCILED' WHERE id = ?")
        .bind(id.to_string())
        .execute(&db.write)
        .await
        .unwrap();
    let input = BulkSplitInput { transaction_ids: vec![id], lines: vec![percent(5_000, food), percent(5_000, food)] };
    assert_eq!(transactions::split_many(&db, &input).await.unwrap().updated, 1);
    assert_eq!(splits(&db, id).await, [(-500, Some(food)), (-500, Some(food))]);
}
