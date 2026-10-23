//! Tests d'intégration des budgets (spec 05), sur une vraie base SQLite.

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::budgeting::{BudgetKind, Periodicity};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{CategoryId, TransactionId};

use super::budgets::{
    self, AlertLevel, AmountChange, AmountScope, AttachInput, BudgetLineInput, MonthBudgetView, MonthInput, MonthQuery,
    ProjectInput,
};
use super::reports;
use super::tests::{TODAY, account, category, d, domain_error, setup, split, tx};
use super::transactions::{self, TransactionInput, TransactionUpdateInput};
use super::transfers::{self, TransferInput};
use crate::db::Db;

fn line(kind: BudgetKind, category: CategoryId, amount: i64, from: &str) -> BudgetLineInput {
    BudgetLineInput {
        kind,
        category_id: Some(category),
        target_account_id: None,
        periodicity: Periodicity::Monthly,
        year_start_month: 1,
        amount_minor: amount,
        rollover: false,
        from: from.into(),
        alert_thresholds: None,
    }
}

async fn view(db: &Db, month: &str) -> MonthBudgetView {
    budgets::month(db, &MonthQuery { month: Some(month.into()) }, d(TODAY)).await.unwrap()
}

fn actual(view: &MonthBudgetView, name: &str) -> i64 {
    view.lines.iter().find(|l| l.name == name).map(|l| l.actual_minor).unwrap()
}

#[tokio::test]
async fn bud_03_month_view_and_synthesis() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Alim B", CategoryKind::Expense, None).await;
    let restaurants = category(&db, "Resto B", CategoryKind::Expense, Some(food)).await;
    let market = category(&db, "Marché B", CategoryKind::Expense, Some(food)).await;
    let gifts = category(&db, "Cadeaux B", CategoryKind::Expense, None).await;
    let salary = category(&db, "Salaire B", CategoryKind::Income, None).await;
    budgets::create_line(&db, &line(BudgetKind::Expense, food, 80_000, "2026-01")).await.unwrap();
    budgets::create_line(&db, &line(BudgetKind::Expense, restaurants, 15_000, "2026-01")).await.unwrap();
    budgets::create_line(&db, &line(BudgetKind::Income, salary, 320_000, "2026-01")).await.unwrap();
    for (date, amount, category) in [
        ("2026-03-01", 320_000, Some(salary)),
        ("2026-03-05", -6_000, Some(market)),
        ("2026-03-06", -4_000, Some(restaurants)),
        ("2026-03-07", -3_780, Some(gifts)),
        ("2026-03-08", -1_200, None),
    ] {
        transactions::create(&db, &tx(checking, date, amount, category)).await.unwrap();
    }
    let march = view(&db, "2026-03").await;
    assert_eq!(actual(&march, "Alim B"), 6_000);
    assert_eq!(actual(&march, "Resto B"), 4_000);
    assert_eq!(actual(&march, "Salaire B"), 320_000);
    assert_eq!(march.off_budget.len(), 1);
    assert_eq!(march.off_budget[0].amount_minor, 3_780);
    assert_eq!(march.uncategorized_out_minor, 1_200);
    assert_eq!(march.to_budget_minor, 320_000 - 80_000 - 15_000);
    assert_eq!(march.balance_minor, 320_000 - 6_000 - 4_000 - 3_780 - 1_200);

    // R-BUD-11 : une seule ligne en vigueur par catégorie.
    let duplicate = budgets::create_line(&db, &line(BudgetKind::Expense, food, 1_000, "2026-03")).await;
    assert_eq!(domain_error(duplicate), DomainError::BudgetLineOverlap);
    // R-BUD-10 : la nature de la ligne suit celle de la catégorie.
    let wrong = budgets::create_line(&db, &line(BudgetKind::Income, gifts, 1_000, "2026-03")).await;
    assert_eq!(domain_error(wrong), DomainError::InvalidBudgetLine);
}

#[tokio::test]
async fn r_bud_51_versions_overrides_and_end() {
    let (db, _dir) = setup().await;
    let gifts = category(&db, "Cadeaux B", CategoryKind::Expense, None).await;
    let created = budgets::create_line(&db, &line(BudgetKind::Expense, gifts, 5_000, "2026-01")).await.unwrap();

    let this_month = AmountChange { month: "2026-12".into(), amount_minor: 30_000, scope: AmountScope::ThisMonth };
    budgets::change_amount(&db, created.id, &this_month).await.unwrap();
    assert_eq!(view(&db, "2026-12").await.lines[0].planned_minor, 30_000);
    assert_eq!(view(&db, "2026-11").await.lines[0].planned_minor, 5_000);

    let from_april = AmountChange { month: "2026-04".into(), amount_minor: 6_000, scope: AmountScope::FromMonth };
    let next = budgets::change_amount(&db, created.id, &from_april).await.unwrap();
    assert_eq!(next.line_id, created.id);
    assert_eq!(view(&db, "2026-03").await.lines[0].planned_minor, 5_000);
    assert_eq!(view(&db, "2026-04").await.lines[0].planned_minor, 6_000);
    // La surcharge de décembre a suivi la nouvelle version.
    assert_eq!(view(&db, "2026-12").await.lines[0].planned_minor, 30_000);
    assert_eq!(budgets::versions(&db).await.unwrap().len(), 2);

    budgets::end_line(&db, created.id, &MonthInput { month: "2026-06".into() }).await.unwrap();
    assert_eq!(view(&db, "2026-05").await.lines.len(), 1);
    assert!(view(&db, "2026-06").await.lines.is_empty());
    // L'historique reste visible (R-BUD-53).
    assert_eq!(budgets::versions(&db).await.unwrap().len(), 2);
}

#[tokio::test]
async fn bud_04_rollover_through_the_service() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let market = category(&db, "Supermarché B", CategoryKind::Expense, None).await;
    let input = BudgetLineInput { rollover: true, ..line(BudgetKind::Expense, market, 60_000, "2026-01") };
    let created = budgets::create_line(&db, &input).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-01-10", -55_000, Some(market))).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-02-10", -64_000, Some(market))).await.unwrap();
    let march = view(&db, "2026-03").await;
    assert_eq!(march.lines[0].carried_minor, 1_000);
    assert_eq!(march.lines[0].available_minor, 61_000);
    // R-BUD-18 : remettre le report à zéro.
    budgets::reset_rollover(&db, created.id, &MonthInput { month: "2026-03".into() }).await.unwrap();
    assert_eq!(view(&db, "2026-03").await.lines[0].carried_minor, 0);
}

#[tokio::test]
async fn bud_07_savings_goal_follows_internal_transfers() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let goal = BudgetLineInput {
        kind: BudgetKind::Savings,
        category_id: None,
        target_account_id: Some(livret),
        ..line(BudgetKind::Savings, CategoryId::new(), 20_000, "2026-01")
    };
    budgets::create_line(&db, &goal).await.unwrap();
    let transfer = |from, to, amount, date: &str| TransferInput {
        from_account_id: from,
        to_account_id: to,
        amount_minor: amount,
        date: d(date),
        label: "Épargne".into(),
        note: None,
    };
    transfers::create(&db, &transfer(checking, livret, 30_000, "2026-03-05")).await.unwrap();
    transfers::create(&db, &transfer(livret, checking, 10_000, "2026-03-20")).await.unwrap();
    let march = view(&db, "2026-03").await;
    assert_eq!(actual(&march, "Livret A"), 20_000);
    assert_eq!(march.lines[0].alert, AlertLevel::Reached);
    assert_eq!(march.savings_minor, 20_000);
    // Février, mois écoulé sans versement : objectif non atteint (R-BUD-61).
    assert_eq!(view(&db, "2026-02").await.lines[0].alert, AlertLevel::Missed);
}

#[tokio::test]
async fn bud_06_projects_leave_period_lines_and_r_bud_24_closed_projects() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let restaurants = category(&db, "Resto B", CategoryKind::Expense, None).await;
    budgets::create_line(&db, &line(BudgetKind::Expense, restaurants, 15_000, "2026-01")).await.unwrap();
    let input = ProjectInput {
        name: "Italie".into(),
        start_date: d("2026-04-01"),
        end_date: Some(d("2026-04-15")),
        amount_minor: 250_000,
        exclude_from_period_budgets: true,
        active: true,
        alert_thresholds: None,
        note: None,
    };
    let project = budgets::save_project(&db, None, &input).await.unwrap();
    let dinner = transactions::create(&db, &tx(checking, "2026-03-10", -8_000, Some(restaurants))).await.unwrap();
    let attached = budgets::attach(&db, &AttachInput { transaction_ids: vec![dinner], project_id: Some(project.id) })
        .await
        .unwrap();
    assert_eq!(attached.attached, 1);
    let march = view(&db, "2026-03").await;
    assert_eq!(actual(&march, "Resto B"), 0);
    assert_eq!(march.excluded_projects_minor, 8_000);
    assert_eq!(march.balance_minor, -8_000);
    let project = budgets::project(&db, project.id).await.unwrap();
    assert_eq!(project.actual_minor, 8_000);
    assert!(project.splits[0].out_of_range);

    // Clôturé : plus de nouveau rattachement, mais les ventilations existantes restent modifiables.
    budgets::save_project(&db, Some(project.id), &ProjectInput { active: false, ..input }).await.unwrap();
    let other = transactions::create(&db, &tx(checking, "2026-03-11", -1_000, None)).await.unwrap();
    let refused =
        budgets::attach(&db, &AttachInput { transaction_ids: vec![other], project_id: Some(project.id) }).await;
    assert_eq!(domain_error(refused), DomainError::ProjectClosed);
    let mut with_project = split(-1_000, None);
    with_project.project_id = Some(project.id);
    let created = transactions::create(
        &db,
        &TransactionInput { splits: Some(vec![with_project.clone()]), ..tx(checking, "2026-03-12", -1_000, None) },
    )
    .await;
    assert_eq!(domain_error(created), DomainError::ProjectClosed);
    let mut kept = split(-8_000, Some(restaurants));
    kept.project_id = Some(project.id);
    let update = TransactionUpdateInput {
        account_id: checking,
        date: d("2026-03-10"),
        amount_minor: -8_000,
        label: "Dîner".into(),
        note: None,
        splits: Some(vec![kept]),
        payee_id: None,
    };
    transactions::update(&db, dinner, &update).await.unwrap();
    assert!(budgets::delete_project(&db, project.id).await.is_err());

    // L'analyse par catégorie compte la dépense du projet, sauf si on l'exclut (R-BUD-21, TDB-07).
    let report = |exclude| reports::CategoryReportQuery {
        month: Some("2026-03".into()),
        kind: None,
        exclude_projects: Some(exclude),
    };
    let restaurants_total = |r: &reports::CategoryReport| {
        r.lines.iter().find(|l| l.category_id == Some(restaurants)).map_or(0, |l| l.amount_minor)
    };
    let with = reports::categories(&db, &report(false), d(TODAY)).await.unwrap();
    assert_eq!(restaurants_total(&with), 8_000);
    let without = reports::categories(&db, &report(true), d(TODAY)).await.unwrap();
    assert_eq!(restaurants_total(&without), 0);
}

#[tokio::test]
async fn bud_08_grouped_notification_is_emitted_once() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let restaurants = category(&db, "Resto B", CategoryKind::Expense, None).await;
    budgets::create_line(&db, &line(BudgetKind::Expense, restaurants, 15_000, "2026-01")).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-03-02", -11_000, Some(restaurants))).await.unwrap();
    assert_eq!(budgets::evaluate_alerts(&db, d(TODAY)).await.unwrap(), 0);
    let big: TransactionId =
        transactions::create(&db, &tx(checking, "2026-03-20", -5_500, Some(restaurants))).await.unwrap();
    assert_eq!(budgets::evaluate_alerts(&db, d(TODAY)).await.unwrap(), 2);
    let notifications = budgets::notifications(&db).await.unwrap();
    assert_eq!(notifications.items.len(), 1);
    assert_eq!(notifications.unread, 1);
    assert!(notifications.items[0].body.contains("80 %") && notifications.items[0].body.contains("100 %"));

    // Repasser sous le seuil puis le refranchir ne réémet rien.
    transactions::delete(&db, big).await.unwrap();
    assert_eq!(budgets::evaluate_alerts(&db, d(TODAY)).await.unwrap(), 0);
    transactions::create(&db, &tx(checking, "2026-03-21", -5_500, Some(restaurants))).await.unwrap();
    assert_eq!(budgets::evaluate_alerts(&db, d(TODAY)).await.unwrap(), 0);
    let read = budgets::mark_read(&db, &budgets::ReadInput { id: None }).await.unwrap();
    assert_eq!(read.unread, 0);
}

#[tokio::test]
async fn bud_10_suggestions_average_complete_months() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let market = category(&db, "Supermarché B", CategoryKind::Expense, None).await;
    transactions::create(&db, &tx(checking, "2026-01-10", -54_020, Some(market))).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-02-10", -61_000, Some(market))).await.unwrap();
    // Le mois en cours n'est pas complet : il n'entre pas dans la moyenne.
    transactions::create(&db, &tx(checking, "2026-03-10", -90_000, Some(market))).await.unwrap();
    let suggestions = budgets::suggestions(&db, d(TODAY)).await.unwrap();
    assert_eq!(suggestions.months, vec!["2026-01", "2026-02"]);
    let market = suggestions.categories.iter().find(|c| c.category_id == market).unwrap();
    assert_eq!(market.suggested_minor, 57_600);
    assert!(!market.has_line);
}

#[tokio::test]
async fn r_bud_60_alerts_can_be_disabled_per_line() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let rent = category(&db, "Loyer B", CategoryKind::Expense, None).await;
    let input =
        BudgetLineInput { alert_thresholds: Some(Vec::new()), ..line(BudgetKind::Expense, rent, 89_000, "2026-01") };
    let created = budgets::create_line(&db, &input).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-03-03", -95_000, Some(rent))).await.unwrap();
    assert_eq!(budgets::evaluate_alerts(&db, d(TODAY)).await.unwrap(), 0);
    let march = view(&db, "2026-03").await;
    assert_eq!(march.lines[0].alert, AlertLevel::None);
    assert_eq!(march.lines[0].alert_thresholds, Some(Vec::new()));

    // Seuils propres à la ligne : 50 % seulement.
    let settings = budgets::LineSettings { rollover: false, alert_thresholds: Some(vec![50]) };
    budgets::update_settings(&db, created.id, &settings).await.unwrap();
    assert_eq!(view(&db, "2026-03").await.lines[0].alert, AlertLevel::Over);
    assert_eq!(budgets::evaluate_alerts(&db, d(TODAY)).await.unwrap(), 1);

    // Les seuils globaux ne peuvent pas être vides.
    assert!(budgets::set_thresholds(&db, &budgets::Thresholds { thresholds: Vec::new() }).await.is_err());
}
