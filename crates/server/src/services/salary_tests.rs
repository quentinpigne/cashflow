//! Tests d'intégration des salaires (spec 10) et de la capacité d'épargne (PRJ-10), sur une vraie base SQLite.

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::salaries::{BonusKind, SalaryChangeReason};
use cashflow_domain::shared::ids::{AccountId, CategoryId, EmploymentId};

use super::features::{self, FeaturesInput};
use super::projections::{self, NetWorthQuery, SalaryProjectionQuery, ScenarioInput};
use super::salaries::{self, BonusInput, EmploymentInput, PayslipInput, PayslipUpload, SalaryInput};
use super::tests::{account, category, d, setup, tx};
use super::{dashboard, transactions};
use crate::config::DataDir;
use crate::db::Db;
use crate::error::AppError;

fn code(error: AppError) -> String {
    match error {
        AppError::Invalid { code, .. } => code.to_owned(),
        AppError::Domain(e) => e.code().to_owned(),
        other => format!("{other:?}"),
    }
}

struct Fixture {
    checking: AccountId,
    pay: CategoryId,
    other: CategoryId,
    spending: CategoryId,
}

async fn fixture(db: &Db) -> Fixture {
    Fixture {
        checking: account(db, "Courant", AccountKind::Checking, 100_000).await,
        pay: category(db, "Paie", CategoryKind::Income, None).await,
        other: category(db, "Revenus divers", CategoryKind::Income, None).await,
        spending: category(db, "Courses", CategoryKind::Expense, None).await,
    }
}

fn job(f: &Fixture) -> EmploymentInput {
    EmploymentInput {
        employer: "Acme".into(),
        job_title: None,
        start_date: d("2025-01-01"),
        end_date: None,
        income_category_id: f.pay,
        raise_month: None,
        paid_next_month: false,
        notes: None,
    }
}

fn pay(month: &str, net: i64) -> SalaryInput {
    SalaryInput {
        effective_month: d(month),
        annual_gross_minor: net * 16,
        monthly_net_minor: net,
        reason: SalaryChangeReason::Raise,
        note: None,
    }
}

/// Emploi chez Acme depuis janvier 2025, 2 700 € net.
async fn employed(db: &Db, f: &Fixture) -> EmploymentId {
    let id = salaries::create(db, &job(f), d("2026-04-10")).await.unwrap().summary.id;
    salaries::save_salary(db, id, None, &pay("2025-01-01", 270_000), d("2026-04-10")).await.unwrap();
    id
}

#[tokio::test]
async fn sal_01_r_sal_1_r_sal_2_r_sal_6_employment_pay_and_bonus() {
    let (db, dir) = setup().await;
    let f = fixture(&db).await;
    let today = d("2026-04-10");
    let wrong = EmploymentInput { income_category_id: f.spending, ..job(&f) };
    assert_eq!(code(salaries::create(&db, &wrong, today).await.unwrap_err()), "NOT_INCOME_CATEGORY");
    let id = employed(&db, &f).await;

    let mid_month = SalaryInput { effective_month: d("2026-01-15"), ..pay("2026-01-01", 1) };
    assert_eq!(code(salaries::save_salary(&db, id, None, &mid_month, today).await.unwrap_err()), "INVALID_SALARY");
    let taken = salaries::save_salary(&db, id, None, &pay("2025-01-01", 1), today).await.unwrap_err();
    assert_eq!(code(taken), "SALARY_MONTH_TAKEN");
    let view = salaries::save_salary(&db, id, None, &pay("2026-06-01", 290_000), today).await.unwrap();
    assert_eq!(view.salaries.len(), 2);
    assert!(view.salaries[0].planned, "R-SAL-4 : mois d'effet futur");
    assert_eq!(view.summary.current_net_minor, Some(270_000));

    let bonus = BonusInput {
        label: "13e mois".into(),
        kind: BonusKind::ThirteenthMonth,
        paid_on: d("2025-12-19"),
        net_amount_minor: 260_000,
        gross_amount_minor: None,
        recurring: None,
        note: None,
    };
    let view = salaries::save_bonus(&db, id, None, &bonus, today).await.unwrap();
    assert!(view.bonuses[0].recurring, "R-SAL-5 : un 13e mois est récurrent par défaut");
    let year = view.years.iter().find(|y| y.year == 2025).unwrap();
    assert_eq!(year.net_minor, 12 * 270_000 + 260_000);
    assert!(year.gross_incomplete, "prime sans brut");

    // Les dates de l'emploi ne peuvent plus exclure ses rémunérations.
    let later = EmploymentInput { start_date: d("2025-06-01"), ..job(&f) };
    let error = salaries::update(&db, id, &later, today).await.unwrap_err();
    assert_eq!(code(error), "EMPLOYMENT_PERIOD_EXCLUDES_PAY");

    // R-SAL-6 : rémunérations et primes partent avec l'emploi.
    salaries::delete(&db, &DataDir::new(dir.path()), id, today).await.unwrap();
    assert!(crate::db::salaries::salaries(&db.read).await.unwrap().is_empty());
    assert!(crate::db::salaries::bonuses(&db.read).await.unwrap().is_empty());
}

#[tokio::test]
async fn sal_03_r_sal_9_expected_received_and_home_alert() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    features::update(&db, &FeaturesInput { salaries: Some(true), ..Default::default() }).await.unwrap();
    let id = employed(&db, &f).await;
    transactions::create(&db, &tx(f.checking, "2026-01-27", 270_000, Some(f.pay))).await.unwrap();
    transactions::create(&db, &tx(f.checking, "2026-03-27", 270_215, Some(f.pay))).await.unwrap();

    let view = salaries::get(&db, id, d("2026-04-10")).await.unwrap();
    let month = |m: &str| view.checks.iter().find(|c| c.month == d(m)).unwrap();
    assert!(month("2026-04-01").in_progress && !month("2026-04-01").flagged);
    assert!(!month("2026-03-01").flagged);
    assert_eq!(month("2026-03-01").gap_minor, 215);
    assert!(month("2026-02-01").flagged);
    assert_eq!(month("2026-02-01").gap_minor, -270_000);

    // Le dernier mois complet est signalé sur l'accueil.
    let home = dashboard::get(&db, d("2026-03-10")).await.unwrap();
    assert_eq!(home.salary_gaps.len(), 1);
    assert_eq!(home.salary_gaps[0].month, d("2026-02-01"));
    assert!(dashboard::get(&db, d("2026-04-10")).await.unwrap().salary_gaps.is_empty());
}

#[tokio::test]
async fn sal_05_r_sal_14_feature_is_off_by_default_and_disabled_only_when_empty() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let current = features::get(&db).await.unwrap();
    assert!(!current.salaries);
    assert_eq!(current.salary_gap_tolerance_ppm, 50_000);
    features::update(&db, &FeaturesInput { salaries: Some(true), ..Default::default() }).await.unwrap();
    employed(&db, &f).await;
    let off = FeaturesInput { salaries: Some(false), ..Default::default() };
    assert_eq!(code(features::update(&db, &off).await.unwrap_err()), "FEATURE_IN_USE");
}

/// Trois mois complets : 2 700 € de salaire, 50 € d'autres revenus, 1 900 € de dépenses.
async fn three_months(db: &Db, f: &Fixture) {
    for month in ["2026-01", "2026-02", "2026-03"] {
        let day = |dd: &str| format!("{month}-{dd}");
        transactions::create(db, &tx(f.checking, &day("27"), 270_000, Some(f.pay))).await.unwrap();
        transactions::create(db, &tx(f.checking, &day("12"), 5_000, Some(f.other))).await.unwrap();
        transactions::create(db, &tx(f.checking, &day("05"), -190_000, Some(f.spending))).await.unwrap();
    }
}

#[tokio::test]
async fn prj_10_r_prj_42_to_r_prj_47_savings_capacity_in_the_net_worth_projection() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    employed(&db, &f).await;
    three_months(&db, &f).await;
    let today = d("2026-04-10");
    let worth = projections::net_worth(&db, &NetWorthQuery { months: Some(12) }, today).await.unwrap();
    assert_eq!(worth.capacity.months, 3, "seuls les mois complets suivis sont moyennés");
    assert_eq!((worth.capacity.other_income_minor, worth.capacity.expenses_minor), (5_000, 190_000));
    assert_eq!(worth.capacity.payroll_account_id, Some(f.checking));
    let median = worth.scenarios.iter().find(|s| s.is_default).unwrap();
    let capacity = median.capacity.as_ref().unwrap();
    // Mai 2026 : 2 700 + 50 − 1 900 ; janvier 2027 : 2 754 + 50 − 1 938.
    assert_eq!(capacity.points[0].total_minor, 85_000);
    assert_eq!(capacity.points[8].total_minor, 275_400 + 5_000 - 193_800);
    assert_eq!(median.points[0].value_minor, worth.start_minor + 85_000);
    let prudent = worth.scenarios.iter().find(|s| s.name == "Prudent").unwrap();
    assert_eq!(prudent.capacity.as_ref().unwrap().points[8].salaries_minor, 272_700);

    // Le reste de la capacité va sur l'assurance vie (R-PRJ-45) ; un compte courant est refusé.
    let life = account(&db, "Assurance vie", AccountKind::LifeInsurance, 0).await;
    let mut scenario = projections::scenarios(&db).await.unwrap().into_iter().find(|s| s.is_default).unwrap();
    let mut input = ScenarioInput {
        name: scenario.name.clone(),
        horizon_months: scenario.horizon_months,
        inflation_rate_ppm: None,
        assumptions: scenario.assumptions.clone(),
        is_default: true,
    };
    input.assumptions.capacity.surplus_account_id = Some(f.checking);
    let error = projections::save_scenario(&db, Some(scenario.id), &input).await.unwrap_err();
    assert_eq!(code(error), "SURPLUS_NOT_SAVINGS");
    input.assumptions.capacity.surplus_account_id = Some(life);
    projections::save_scenario(&db, Some(scenario.id), &input).await.unwrap();
    let worth = projections::net_worth(&db, &NetWorthQuery { months: Some(12) }, today).await.unwrap();
    let median = worth.scenarios.iter().find(|s| s.is_default).unwrap();
    assert_eq!(median.capacity.as_ref().unwrap().points[0].moved_minor, 85_000);
    let checking = projections::account(&db, f.checking, &Default::default(), today).await.unwrap();
    assert_eq!(checking.points[0].value_minor, checking.start_minor, "le reste part sur l'assurance vie");

    // Désactivée, la capacité ne bouge plus les liquidités (R-PRJ-47).
    scenario.assumptions.capacity.enabled = false;
    input.assumptions = scenario.assumptions;
    projections::save_scenario(&db, Some(scenario.id), &input).await.unwrap();
    let worth = projections::net_worth(&db, &NetWorthQuery { months: Some(12) }, today).await.unwrap();
    let median = worth.scenarios.iter().find(|s| s.is_default).unwrap();
    assert!(median.capacity.is_none());
    assert_eq!(median.points[0].value_minor, worth.start_minor);
}

#[tokio::test]
async fn sal_04_r_sal_13_salary_projection_by_scenario() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    employed(&db, &f).await;
    let projection =
        projections::salary(&db, &SalaryProjectionQuery { years: Some(2) }, d("2026-04-10")).await.unwrap();
    assert_eq!(projection.history.iter().map(|y| (y.year, y.net_minor)).collect::<Vec<_>>(), [(2025, 3_240_000)]);
    let median = projection.scenarios.iter().find(|s| s.is_default).unwrap();
    assert_eq!(median.years[1].year, 2027);
    assert_eq!(median.years[1].net_minor, 12 * 275_400);
    assert_eq!(median.years[0].net_minor, 12 * 270_000);
}

fn payslip(month: &str, content: &[u8]) -> PayslipUpload {
    PayslipUpload { file_name: format!("paie-{month}.pdf"), content: content.to_vec(), pay_month: d(month), note: None }
}

#[tokio::test]
async fn sal_09_r_sal_15_to_r_sal_18_payslips_are_archived_with_their_employment() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path());
    data.ensure().unwrap();
    let f = fixture(&db).await;
    let id = employed(&db, &f).await;
    let today = d("2026-10-22");

    // R-SAL-15 : un PDF, un JPEG ou un PNG, pendant l'emploi.
    let word = payslip("2026-07-01", b"PK\x03\x04word/document.xml");
    let error = salaries::add_payslip(&db, &data, id, word, today).await.unwrap_err();
    assert_eq!(code(error), "UNSUPPORTED_DOCUMENT_TYPE");
    let before = payslip("2024-12-01", b"%PDF-1.7 decembre");
    let error = salaries::add_payslip(&db, &data, id, before, today).await.unwrap_err();
    assert_eq!(code(error), "INVALID_PAYSLIP_MONTH");

    // Exemple SAL-09 : juillet, août et sa rectificative, octobre en avance.
    for (month, content) in [
        ("2026-07-01", &b"%PDF-1.7 juillet"[..]),
        ("2026-08-01", b"%PDF-1.7 aout"),
        ("2026-08-01", b"%PDF-1.7 aout rectificative"),
        ("2026-10-01", b"\x89PNG\r\n\x1a\n octobre"),
    ] {
        salaries::add_payslip(&db, &data, id, payslip(month, content), today).await.unwrap();
    }
    let view = salaries::get(&db, id, today).await.unwrap();
    let months: Vec<String> = view.payslips.iter().map(|p| p.pay_month.to_string()).collect();
    assert_eq!(months, ["2026-10-01", "2026-08-01", "2026-08-01", "2026-07-01"], "R-SAL-18 : le plus récent d'abord");
    assert_eq!(view.missing_payslip_months, [d("2026-09-01")], "R-SAL-18");
    assert_eq!(view.payslips[0].media_type.as_str(), "image/png");

    // R-SAL-16 : un même fichier ne s'archive qu'une fois.
    let again = payslip("2026-09-01", b"%PDF-1.7 aout");
    let error = salaries::add_payslip(&db, &data, id, again, today).await.unwrap_err();
    assert_eq!(code(error), "PAYSLIP_ALREADY_ARCHIVED");

    // R-SAL-17 : le mois et la note se modifient, pas le fichier.
    let rectified = view.payslips[1].id;
    let input = PayslipInput { pay_month: d("2026-09-01"), note: Some(" rectificative ".into()) };
    let view = salaries::update_payslip(&db, id, rectified, &input, today).await.unwrap();
    assert!(view.missing_payslip_months.is_empty());
    let (row, content) = salaries::payslip_file(&db, &data, id, rectified).await.unwrap();
    assert_eq!(content, b"%PDF-1.7 aout rectificative");
    assert_eq!(row.pay_month, d("2026-09-01"));
    assert_eq!(row.note.as_deref(), Some("rectificative"));

    // Supprimer une fiche met son fichier à la corbeille (ADR-0024).
    salaries::delete_payslip(&db, &data, id, rectified, today).await.unwrap();
    assert!(matches!(salaries::payslip_file(&db, &data, id, rectified).await, Err(AppError::NotFound(_))));
    assert_eq!(std::fs::read_dir(data.documents_trash()).unwrap().count(), 1);

    // L'emploi emporte ses fiches et leurs documents.
    salaries::delete(&db, &data, id, today).await.unwrap();
    let documents: i64 = sqlx::query_scalar("SELECT count(*) FROM document").fetch_one(&db.read).await.unwrap();
    assert_eq!(documents, 0);
    assert_eq!(std::fs::read_dir(data.documents_trash()).unwrap().count(), 4);
}
