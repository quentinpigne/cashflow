//! Tests d'intégration des services, sur une vraie base SQLite (architecture.md § Stratégie de test).

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, CategoryId};
use cashflow_domain::{Date, DomainError};

use super::accounts::{self, AccountInput, CloseInput};
use super::categories::{self, CategoryInput, MergeInput};
use super::transactions::{self, SplitInput, TransactionFilter, TransactionInput, TransactionUpdateInput};
use super::transfers::{self, MarkAsTransferInput, TransferInput};
use super::{dashboard, seed};
use crate::db::{self, Db};
use crate::error::AppError;

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

const TODAY: &str = "2026-03-31";

async fn setup() -> (Db, tempfile::TempDir) {
    let (db, dir) = db::test_db().await;
    seed::run(&db).await.unwrap();
    (db, dir)
}

async fn account(db: &Db, name: &str, kind: AccountKind, opening_minor: i64) -> AccountId {
    let input = AccountInput {
        name: name.into(),
        kind,
        institution_id: None,
        currency: None,
        opening_date: d("2026-01-01"),
        opening_balance_minor: opening_minor,
        iban: None,
        account_number: None,
        include_in_net_worth: None,
        include_in_emergency_fund: None,
        include_in_budget: None,
        display_order: None,
        color: None,
        notes: None,
    };
    accounts::create(db, &input, d(TODAY)).await.unwrap().id
}

async fn category(db: &Db, name: &str, kind: CategoryKind, parent: Option<CategoryId>) -> CategoryId {
    let input = CategoryInput {
        name: name.into(),
        kind,
        parent_id: parent,
        archived: false,
        display_order: None,
        color: None,
        icon: None,
    };
    categories::create(db, &input).await.unwrap()
}

fn split(amount: i64, category: Option<CategoryId>) -> SplitInput {
    SplitInput { amount_minor: amount, category_id: category, note: None, project_id: None }
}

fn tx(account: AccountId, date: &str, amount: i64, category: Option<CategoryId>) -> TransactionInput {
    TransactionInput {
        account_id: account,
        date: d(date),
        amount_minor: amount,
        label: "Test".into(),
        note: None,
        category_id: category,
        splits: None,
    }
}

async fn balance(db: &Db, account: AccountId, at: &str) -> i64 {
    accounts::get(db, account, d(at)).await.unwrap().balance_minor
}

fn domain_error(result: Result<impl std::fmt::Debug, AppError>) -> DomainError {
    match result {
        Err(AppError::Domain(error)) => error,
        other => panic!("erreur métier attendue, obtenu {other:?}"),
    }
}

#[tokio::test]
async fn cpt_01_and_cpt_03_balance_follows_transactions() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 136_993).await;
    assert_eq!(balance(&db, checking, "2026-01-01").await, 136_993);
    transactions::create(&db, &tx(checking, "2026-03-10", -5_000, None)).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-03-31", 200_000, None)).await.unwrap();
    assert_eq!(balance(&db, checking, "2026-03-15").await, 131_993);
    assert_eq!(balance(&db, checking, "2026-03-31").await, 331_993);
}

#[tokio::test]
async fn r_cpt_9_fixed_savings_get_prefilled_terms() {
    let (db, _dir) = setup().await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    assert!(db::accounts::has_terms(&db.read, livret).await.unwrap());
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    assert!(!db::accounts::has_terms(&db.read, checking).await.unwrap());
}

#[tokio::test]
async fn r_cpt_3_transaction_outside_account_period_is_refused() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let error = domain_error(transactions::create(&db, &tx(checking, "2025-12-31", -100, None)).await);
    assert!(matches!(error, DomainError::DateOutsideAccountPeriod { .. }));
}

#[tokio::test]
async fn r_cpt_5_closing_with_balance_needs_confirmation_and_r_cpt_7_no_delete() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 1_000).await;
    transactions::create(&db, &tx(checking, "2026-02-01", -400, None)).await.unwrap();
    let close = |confirm| CloseInput { closed_on: d("2026-03-01"), confirm };
    assert!(matches!(
        domain_error(accounts::close(&db, checking, &close(false), d(TODAY)).await),
        DomainError::ClosingBalanceNotZero { .. }
    ));
    let closed = accounts::close(&db, checking, &close(true), d(TODAY)).await.unwrap();
    assert_eq!(closed.closed_on, Some(d("2026-03-01")));
    assert_eq!(domain_error(accounts::delete(&db, checking).await), DomainError::AccountNotEmpty);

    // Un compte vide se supprime, conditions d'épargne comprises.
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    accounts::delete(&db, livret).await.unwrap();
}

#[tokio::test]
async fn r_trx_10_rejects_unbalanced_splits() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let mut input = tx(checking, "2026-03-10", -12_000, None);
    input.splits = Some(vec![split(-9_500, None), split(-1_500, None)]);
    assert!(matches!(domain_error(transactions::create(&db, &input).await), DomainError::SplitSumMismatch { .. }));
}

#[tokio::test]
async fn r_trx_4_single_split_follows_amount_multiple_splits_must_be_rebalanced() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Nourriture", CategoryKind::Expense, None).await;
    let single = transactions::create(&db, &tx(checking, "2026-03-10", -1_000, Some(food))).await.unwrap();
    let update = |amount| TransactionUpdateInput {
        account_id: checking,
        date: d("2026-03-10"),
        amount_minor: amount,
        label: "Test".into(),
        note: None,
        splits: None,
    };
    transactions::update(&db, single, &update(-1_500)).await.unwrap();
    let detail = transactions::get(&db, single).await.unwrap();
    assert_eq!(detail.splits.len(), 1);
    assert_eq!(detail.splits[0].amount_minor, -1_500);
    assert_eq!(detail.splits[0].category_id, Some(food));

    let mut input = tx(checking, "2026-03-11", -1_000, None);
    input.splits = Some(vec![split(-600, Some(food)), split(-400, None)]);
    let multiple = transactions::create(&db, &input).await.unwrap();
    assert!(matches!(
        domain_error(transactions::update(&db, multiple, &update(-2_000)).await),
        DomainError::SplitSumMismatch { .. }
    ));
}

#[tokio::test]
async fn trx_02_running_balance_and_filters() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 10_000).await;
    let food = category(&db, "Nourriture", CategoryKind::Expense, None).await;
    let market = category(&db, "Marché", CategoryKind::Expense, Some(food)).await;
    transactions::create(&db, &tx(checking, "2026-03-01", -1_000, Some(market))).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-03-02", 500, None)).await.unwrap();
    let page = transactions::list(&db, checking, &TransactionFilter::default()).await.unwrap();
    assert_eq!(page.items.iter().map(|i| i.balance_after_minor).collect::<Vec<_>>(), vec![Some(9_500), Some(9_000)]);

    // Le filtre par catégorie parente inclut ses sous-catégories (R-TRX-21).
    let filter = TransactionFilter { category_id: Some(food), ..Default::default() };
    let page = transactions::list(&db, checking, &filter).await.unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].balance_after_minor, Some(9_000));
}

#[tokio::test]
async fn trx_05_transfer_moves_money_without_changing_net_worth() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let before = dashboard::get(&db, d(TODAY)).await.unwrap();
    let input = TransferInput {
        from_account_id: checking,
        to_account_id: livret,
        amount_minor: 20_000,
        date: d("2026-03-05"),
        label: "Épargne".into(),
        note: None,
    };
    let created = transfers::create(&db, &input).await.unwrap();
    assert_eq!(balance(&db, checking, TODAY).await, 80_000);
    assert_eq!(balance(&db, livret, TODAY).await, 20_000);
    let after = dashboard::get(&db, d(TODAY)).await.unwrap();
    assert_eq!(before.net_worth.total_minor, after.net_worth.total_minor);
    assert_eq!(after.month.income_minor, 0);
    assert_eq!(after.month.expenses_minor, 0);

    // Modifier le montant d'une jambe met à jour l'autre (R-TRX-30).
    let update = TransactionUpdateInput {
        account_id: checking,
        date: d("2026-03-05"),
        amount_minor: -25_000,
        label: "Épargne".into(),
        note: None,
        splits: None,
    };
    transactions::update(&db, created.outflow_transaction_id, &update).await.unwrap();
    assert_eq!(balance(&db, livret, TODAY).await, 25_000);

    // Une jambe ne se catégorise pas (R-TRX-32).
    let food = category(&db, "Nourriture", CategoryKind::Expense, None).await;
    let outflow = transactions::get(&db, created.outflow_transaction_id).await.unwrap();
    let categorize = transactions::CategorizeInput { category_id: Some(food) };
    assert_eq!(
        domain_error(transactions::categorize_split(&db, outflow.splits[0].id, &categorize).await),
        DomainError::TransferLegCategorized
    );

    // Supprimer une jambe délie l'autre, qui reste (R-TRX-34).
    transactions::delete(&db, created.outflow_transaction_id).await.unwrap();
    let inflow = transactions::get(&db, created.inflow_transaction_id).await.unwrap();
    assert!(inflow.transfer.is_none());
}

#[tokio::test]
async fn r_trx_36_marking_as_transfer_creates_opposite_leg_or_links_candidate() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let life = account(&db, "Assurance vie", AccountKind::LifeInsurance, 0).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let food = category(&db, "Nourriture", CategoryKind::Expense, None).await;

    // Aucune candidate : la jambe opposée est créée, CLEARED sur un compte d'épargne variable.
    let source = transactions::create(&db, &tx(checking, "2026-03-10", -30_000, Some(food))).await.unwrap();
    let mark =
        |confirm| MarkAsTransferInput { counterpart_account_id: life, counterpart_transaction_id: None, confirm };
    let error = transfers::mark_as_transfer(&db, source, &mark(false)).await;
    assert!(matches!(error, Err(AppError::Invalid { code: "TRANSFER_REPLACES_CATEGORIES", .. })));
    transfers::mark_as_transfer(&db, source, &mark(true)).await.unwrap();
    let detail = transactions::get(&db, source).await.unwrap();
    let side = detail.transfer.unwrap();
    let opposite = transactions::get(&db, side.counterpart_transaction_id).await.unwrap();
    assert_eq!(opposite.amount_minor, 30_000);
    assert_eq!(opposite.clearing_status, cashflow_domain::ledger::ClearingStatus::Cleared);
    assert_eq!(detail.splits[0].category_id, None);

    // Une candidate existe à ±5 jours : on la lie.
    let outflow = transactions::create(&db, &tx(checking, "2026-03-20", -5_000, None)).await.unwrap();
    let inflow = transactions::create(&db, &tx(livret, "2026-03-22", 5_000, None)).await.unwrap();
    let candidates =
        transfers::candidates(&db, outflow, &transfers::CandidatesQuery { account_id: livret }).await.unwrap();
    assert_eq!(candidates.iter().map(|c| c.id).collect::<Vec<_>>(), vec![inflow]);
    let link = MarkAsTransferInput {
        counterpart_account_id: livret,
        counterpart_transaction_id: Some(inflow),
        confirm: false,
    };
    transfers::mark_as_transfer(&db, outflow, &link).await.unwrap();
    assert!(transactions::get(&db, inflow).await.unwrap().transfer.is_some());
}

#[tokio::test]
async fn r_trx_22_merge_and_delete_categories() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Nourriture", CategoryKind::Expense, None).await;
    let snacks = category(&db, "Snacks", CategoryKind::Expense, None).await;
    let income = category(&db, "Bonus", CategoryKind::Income, None).await;
    transactions::create(&db, &tx(checking, "2026-03-10", -1_000, Some(snacks))).await.unwrap();

    assert_eq!(domain_error(categories::delete(&db, snacks).await), DomainError::CategoryInUse);
    assert!(matches!(
        domain_error(categories::merge(&db, snacks, &MergeInput { target_id: income }).await),
        DomainError::InvalidCategoryMerge
    ));
    let result = categories::merge(&db, snacks, &MergeInput { target_id: food }).await.unwrap();
    assert_eq!(result.moved_splits, 1);
    let unused = category(&db, "Inutile", CategoryKind::Expense, None).await;
    categories::delete(&db, unused).await.unwrap();

    // Deux niveaux au plus (R-TRX-20).
    let child = category(&db, "Enfant", CategoryKind::Expense, Some(food)).await;
    let input = CategoryInput {
        name: "Petit-enfant".into(),
        kind: CategoryKind::Expense,
        parent_id: Some(child),
        archived: false,
        display_order: None,
        color: None,
        icon: None,
    };
    assert_eq!(domain_error(categories::create(&db, &input).await), DomainError::CategoryDepthExceeded);
}

#[tokio::test]
async fn trx_08_transverse_list_and_trx_06_bulk_categorize() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let food = category(&db, "Nourriture", CategoryKind::Expense, None).await;
    let a = transactions::create(&db, &tx(checking, "2026-03-01", -1_000, None)).await.unwrap();
    let b = transactions::create(&db, &tx(livret, "2026-03-02", 500, None)).await.unwrap();
    let transfer = transfers::create(
        &db,
        &TransferInput {
            from_account_id: checking,
            to_account_id: livret,
            amount_minor: 2_000,
            date: d("2026-03-03"),
            label: "Épargne".into(),
            note: None,
        },
    )
    .await
    .unwrap();

    let all = transactions::list_all(&db, &TransactionFilter::default()).await.unwrap();
    assert_eq!(all.total, 4);
    assert!(all.items.iter().all(|i| i.balance_after_minor.is_none()));
    let uncategorized = TransactionFilter { uncategorized: Some(true), ..Default::default() };
    assert_eq!(
        transactions::list_all(&db, &uncategorized).await.unwrap().total,
        2,
        "les virements ne sont pas à catégoriser"
    );
    let only_livret = TransactionFilter { accounts: Some(livret.to_string()), ..Default::default() };
    assert_eq!(transactions::list_all(&db, &only_livret).await.unwrap().total, 2);

    let input = transactions::BulkCategorizeInput {
        transaction_ids: vec![a, b, transfer.outflow_transaction_id],
        category_id: food,
    };
    let result = transactions::categorize_many(&db, &input).await.unwrap();
    assert_eq!((result.categorized, result.skipped), (2, 1));
}

#[tokio::test]
async fn tdb_02_category_report_and_tdb_03_evolution() {
    use super::reports;

    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let food = category(&db, "Nourriture", CategoryKind::Expense, None).await;
    let market = category(&db, "Marché", CategoryKind::Expense, Some(food)).await;
    let salary = category(&db, "Paie", CategoryKind::Income, None).await;
    transactions::create(&db, &tx(checking, "2026-02-10", -3_000, Some(food))).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-03-05", -1_000, Some(food))).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-03-06", -500, Some(market))).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-03-07", 200_000, Some(salary))).await.unwrap();

    let query = reports::CategoryReportQuery { month: Some("2026-03".into()), kind: None };
    let report = reports::categories(&db, &query, d(TODAY)).await.unwrap();
    assert!(report.current_month);
    let line = report.lines.iter().find(|l| l.category_id == Some(food)).unwrap();
    assert_eq!(line.amount_minor, 1_500, "la racine cumule ses sous-catégories");
    assert_eq!(line.children.len(), 1);
    assert_eq!(line.previous_minor, 3_000);
    assert_eq!(line.average_minor, 250, "3 000 sur 12 mois complets");

    let evolution = reports::evolution(&db, &reports::EvolutionQuery { months: Some(12) }, d(TODAY)).await.unwrap();
    assert_eq!(evolution.months.len(), 12);
    let march = evolution.months.last().unwrap();
    assert_eq!(march.month, "2026-03");
    assert_eq!((march.income_minor, march.expenses_minor), (200_000, 1_500));
}
