//! Tests d'intégration des récurrences (TRX-07) et des projections (spec 07), sur une vraie base SQLite.

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::recurrence::Frequency;
use cashflow_domain::shared::ids::AccountId;

use super::projections::{self, CashQuery, HistoryQuery, NetWorthQuery, ProjectionQuery};
use super::recurring::{self, MaterializeInput, OccurrenceStatus, RecurringRuleInput, ScheduleQuery};
use super::tests::{TODAY, account, category, d, setup, tx};
use super::transactions;
use crate::error::AppError;

fn rule(account: AccountId, label: &str, amount: i64, anchor: &str) -> RecurringRuleInput {
    RecurringRuleInput {
        account_id: account,
        target_account_id: None,
        label: label.into(),
        amount_minor: amount,
        amount_tolerance_minor: 0,
        frequency: Frequency::Monthly,
        interval: 1,
        anchor_date: d(anchor),
        end_date: None,
        split_template: Vec::new(),
        active: true,
    }
}

#[tokio::test]
async fn trx_07_schedule_honored_missed_and_materialized() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let internet = rule(checking, "Box internet", -2_999, "2026-01-06");
    let created = recurring::save(&db, None, &internet, d(TODAY)).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-02-08", -2_999, None)).await.unwrap();
    let view = recurring::list(&db, d(TODAY)).await.unwrap().remove(0);
    // Février est honoré (±5 jours, R-TRX-42) ; mars ne l'est pas.
    assert_eq!(view.missed, vec![d("2026-03-06")]);
    assert_eq!(view.next_date, Some(d("2026-04-06")));

    recurring::materialize(&db, created.id, &MaterializeInput { date: d("2026-03-06") }).await.unwrap();
    let query = ScheduleQuery { account_id: Some(checking), days: Some(40) };
    let schedule = recurring::upcoming(&db, &query, d(TODAY)).await.unwrap();
    let march = schedule.iter().find(|o| o.date == d("2026-03-06")).unwrap();
    assert_eq!(march.status, OccurrenceStatus::Honored);
    assert_eq!(schedule.last().unwrap().status, OccurrenceStatus::Upcoming);
    assert!(recurring::materialize(&db, created.id, &MaterializeInput { date: d("2026-03-07") }).await.is_err());

    // Un virement récurrent est une sortie du compte (R-TRX-44).
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let wrong =
        RecurringRuleInput { target_account_id: Some(livret), ..rule(checking, "Épargne", 20_000, "2026-01-02") };
    assert!(matches!(
        recurring::save(&db, None, &wrong, d(TODAY)).await,
        Err(AppError::Invalid { code: "INVALID_RECURRING_TRANSFER", .. })
    ));
}

#[tokio::test]
async fn prj_01_cash_projection_with_schedule_and_variable_spending() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let food = category(&db, "Courses B", CategoryKind::Expense, None).await;
    transactions::create(&db, &tx(checking, "2026-01-15", -30_000, Some(food))).await.unwrap();
    transactions::create(&db, &tx(checking, "2026-02-15", -30_000, Some(food))).await.unwrap();
    recurring::save(&db, None, &rule(checking, "Loyer", -40_000, "2026-04-05"), d(TODAY)).await.unwrap();
    // Une dépense en attente datée dans le futur (R-PRJ-10).
    transactions::create(&db, &tx(checking, "2026-04-10", -5_000, None)).await.unwrap();

    let start = 100_000 - 60_000;
    let without = CashQuery { days: Some(30), variable: Some(false), threshold_minor: Some(0) };
    let p = projections::cash(&db, checking, &without, d(TODAY)).await.unwrap();
    assert_eq!(p.start_minor, start);
    assert_eq!(p.events.len(), 2);
    assert_eq!(p.minimum_minor, start - 45_000);
    assert_eq!(p.minimum_date, d("2026-04-10"));
    assert!(p.below_threshold);

    // Dépense variable : 600 € sur les 3 derniers mois complets, soit 200 € par mois (R-PRJ-11).
    let with = CashQuery { variable: Some(true), ..without };
    let p = projections::cash(&db, checking, &with, d(TODAY)).await.unwrap();
    assert_eq!(p.variable_monthly_minor, Some(20_000));
    assert!(p.days.last().unwrap().variable_minor > 19_000);
}

#[tokio::test]
async fn prj_02_04_05_projections_and_history() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 500_000).await;
    let livret = account(&db, "Livret maison", AccountKind::SavingsOther, 100_000).await;
    let life = account(&db, "Assurance vie", AccountKind::LifeInsurance, 1_000_000).await;
    let terms = super::savings::terms(&db, livret, d(TODAY)).await.unwrap();
    super::savings::set_rate(
        &db,
        terms.rate_schedule_id,
        &super::savings::RatePeriodInput { effective_from: d("2020-01-01"), rate_ppm: 30_000 },
    )
    .await
    .unwrap();
    let saving =
        RecurringRuleInput { target_account_id: Some(livret), ..rule(checking, "Épargne", -10_000, "2026-04-02") };
    recurring::save(&db, None, &saving, d(TODAY)).await.unwrap();

    let scenarios = projections::scenarios(&db).await.unwrap();
    assert_eq!(scenarios.len(), 3);
    let median = scenarios.iter().find(|s| s.is_default).unwrap();
    assert_eq!(median.name, "Médian");

    // Épargne fixe : versements programmés et intérêts capitalisés au 31/12 (PRJ-02).
    let query = ProjectionQuery { scenario_id: None, months: Some(12) };
    let p = projections::account(&db, livret, &query, d(TODAY)).await.unwrap();
    assert_eq!(p.rate_ppm, Some(30_000));
    assert_eq!(p.points[0].value_minor, 110_000);
    let december = p.points.iter().find(|pt| pt.date == d("2026-12-31")).unwrap();
    assert!(december.value_minor > 100_000 + 9 * 10_000);

    // Épargne variable : 10 000 € à 2,5 % (médian) donnent 10 250 € à 12 mois (PRJ-04).
    let p = projections::account(&db, life, &query, d(TODAY)).await.unwrap();
    assert_eq!(p.annual_return_ppm, Some(25_000));
    assert_eq!(p.points[11].value_minor, 1_025_000);

    // Patrimoine : un virement interne ne gonfle pas le total (R-PRJ-41).
    let worth = projections::net_worth(&db, &NetWorthQuery { months: Some(12) }, d(TODAY)).await.unwrap();
    assert_eq!(worth.scenarios.len(), 3);
    assert_eq!(worth.start_minor, 1_600_000);
    let first = &worth.scenarios.iter().find(|s| s.is_default).unwrap().points[0];
    // Fin avril : le virement de 100 € quitte le compte courant et arrive sur le livret ; seul le
    // rendement de l'assurance vie s'ajoute.
    let life_growth = p.points[0].value_minor - 1_000_000;
    assert_eq!(first.value_minor, 1_600_000 + life_growth);

    let history = projections::history(&db, &HistoryQuery { months: Some(6) }, d(TODAY)).await.unwrap();
    let last = history.last().unwrap();
    assert_eq!(last.date, d(TODAY));
    assert_eq!(last.total_minor, 1_600_000);
    assert_eq!(last.variable_savings_minor, 1_000_000);
}
