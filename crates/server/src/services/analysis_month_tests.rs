//! Tests d'intégration du mois d'analyse (TRX-13, IMP-14, ADR-0028), sur une vraie base SQLite.

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::budgeting::{BudgetKind, Periodicity};
use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::TransactionId;

use super::budgets::{self, BudgetLineInput, MonthQuery};
use super::rules::{self, ApplyInput, PreviewQuery, RuleInput};
use super::tests::{account, category, d, domain_error, setup, tx};
use super::transactions::{self, BulkAnalysisMonthInput, TransactionFilter};
use super::{accounts, dashboard};
use crate::db::Db;

async fn set_month(db: &Db, id: TransactionId, month: Option<&str>) {
    let input = BulkAnalysisMonthInput { transaction_ids: vec![id], analysis_month: month.map(Into::into) };
    transactions::set_analysis_month(db, &input).await.unwrap();
}

async fn income(db: &Db, today: &str) -> i64 {
    dashboard::get(db, d(today)).await.unwrap().month.income_minor
}

/// Critère d'acceptation de TRX-13 : le salaire du 28 compte en novembre ; les soldes ne bougent pas (R-TRX-57).
#[tokio::test]
async fn trx_13_r_trx_57_salary_counts_on_the_next_month() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let salary = category(&db, "Salaire M", CategoryKind::Income, None).await;
    let line = BudgetLineInput {
        kind: BudgetKind::Income,
        name: None,
        category_ids: vec![salary],
        target_account_id: None,
        periodicity: Periodicity::Monthly,
        year_start_month: 1,
        amount_minor: 300_000,
        rollover: false,
        from: "2026-01".into(),
        alert_thresholds: None,
    };
    budgets::create_line(&db, &line).await.unwrap();
    let pay = transactions::create(&db, &tx(checking, "2026-10-28", 300_000, Some(salary))).await.unwrap();
    assert_eq!(income(&db, "2026-10-31").await, 300_000);

    set_month(&db, pay, Some("2026-11")).await;
    let detail = transactions::get(&db, pay).await.unwrap();
    assert_eq!(detail.analysis_month.as_deref(), Some("2026-11"));
    assert_eq!(income(&db, "2026-10-31").await, 0);
    assert_eq!(income(&db, "2026-11-30").await, 300_000);
    let november =
        budgets::month(&db, &MonthQuery { month: Some("2026-11".into()), planned_projects: None }, d("2026-10-31"))
            .await
            .unwrap();
    assert_eq!(november.lines[0].actual_minor, 300_000);
    // Le solde au 28 octobre ne change pas (R-TRX-7).
    let balance = accounts::get(&db, checking, d("2026-10-28")).await.unwrap().balance_minor;
    assert_eq!(balance, 300_000);

    // Depuis le budget de novembre, la liste filtrée par mois d'analyse montre le salaire d'octobre (R-TRX-58).
    let filter = TransactionFilter { analysis_month: Some("2026-11".into()), ..Default::default() };
    let page = transactions::list_all(&db, &filter).await.unwrap();
    assert_eq!(page.items.iter().map(|i| i.id).collect::<Vec<_>>(), [pay]);
    assert_eq!(page.items[0].analysis_month.as_deref(), Some("2026-11"));

    // Rapprochée, elle garde un mois d'analyse modifiable (R-TRX-3).
    sqlx::query("UPDATE ledger_transaction SET clearing_status = 'RECONCILED' WHERE id = ?")
        .bind(pay.to_string())
        .execute(&db.write)
        .await
        .unwrap();
    // Choisir le mois de la date vide le mois d'analyse.
    set_month(&db, pay, Some("2026-10")).await;
    assert_eq!(transactions::get(&db, pay).await.unwrap().analysis_month, None);
    assert_eq!(income(&db, "2026-10-31").await, 300_000);
}

/// R-IMP-29 : une règle décale le mois d'analyse des transactions qui n'en ont pas ; un mois choisi reste.
#[tokio::test]
async fn imp_14_r_imp_29_rule_shifts_the_analysis_month() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let salary = category(&db, "Salaire N", CategoryKind::Income, None).await;
    let october = transactions::create(&db, &tx(checking, "2026-10-28", 300_000, Some(salary))).await.unwrap();
    let september = transactions::create(&db, &tx(checking, "2026-09-28", 300_000, Some(salary))).await.unwrap();
    set_month(&db, september, Some("2026-09")).await;
    let chosen = transactions::create(&db, &tx(checking, "2026-08-28", 300_000, Some(salary))).await.unwrap();
    set_month(&db, chosen, Some("2026-07")).await;

    let rule = |shift| RuleInput {
        name: "Salaire".into(),
        priority: 0,
        conditions: Conditions {
            label: Some(LabelCondition { op: LabelOp::Contains, value: "Test".into(), field: LabelField::Label }),
            ..Default::default()
        },
        actions: Actions { shift_analysis_month: Some(shift), ..Default::default() },
        stop_processing: true,
        active: true,
    };
    assert_eq!(domain_error(rules::save(&db, None, &rule(13)).await), DomainError::InvalidMonthShift);
    let saved = rules::save(&db, None, &rule(1)).await.unwrap();
    let query = PreviewQuery { sets_payee: None, shifts_month: Some(true) };
    let matches = rules::preview(&db, &saved.conditions, &query).await.unwrap();
    let mut found: Vec<TransactionId> = matches.iter().filter(|m| m.sets_month).map(|m| m.transaction_id).collect();
    found.sort();
    let mut expected = vec![october, september];
    expected.sort();
    assert_eq!(found, expected, "le mois choisi en août n'est pas proposé");

    rules::apply(&db, saved.id, &ApplyInput { transaction_ids: vec![october, september, chosen] }).await.unwrap();
    for (id, expected) in [(october, "2026-11"), (september, "2026-10"), (chosen, "2026-07")] {
        assert_eq!(transactions::get(&db, id).await.unwrap().analysis_month.as_deref(), Some(expected));
    }
}
