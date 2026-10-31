//! Tests d'intégration du foyer (MBR-01 à MBR-03, R-MBR-1 à R-MBR-7).

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp, SplitShare};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, MemberId};

use super::accounts::{self, AccountInput};
use super::dashboard::{self, DashboardQuery};
use super::features::{self, FeaturesInput};
use super::members::HOUSEHOLD;
use super::members::{self, MemberInput};
use super::reports::{self, EvolutionQuery};
use super::rules::{self, ApplyInput, RuleInput};
use super::salaries::{self, EmploymentInput};
use super::tests::{TODAY, account, category, d, domain_error, setup, split, tx};
use super::transactions::{self, SplitInput, TransactionInput, TransactionUpdateInput};
use super::transfers::{self, TransferInput};
use crate::db::Db;
use crate::error::AppError;

async fn member(db: &Db, name: &str) -> MemberId {
    members::create(db, &MemberInput { name: name.into(), color: None }).await.unwrap()
}

fn account_input(name: &str, member: Option<MemberId>) -> AccountInput {
    AccountInput {
        name: name.into(),
        kind: AccountKind::LivretA,
        institution_id: None,
        currency: None,
        opening_date: d("2026-01-01"),
        opening_balance_minor: 50_000,
        iban: None,
        account_number: None,
        include_in_net_worth: None,
        include_in_emergency_fund: None,
        include_in_budget: None,
        display_order: None,
        color: None,
        notes: None,
        member_id: member,
    }
}

#[tokio::test]
async fn r_mbr_1_names_are_unique_without_case_and_colors_are_checked() {
    let (db, _dir) = setup().await;
    let claire = member(&db, "Claire").await;
    let quentin = member(&db, "Quentin").await;
    let taken = members::create(&db, &MemberInput { name: "CLAIRE".into(), color: None }).await;
    assert_eq!(domain_error(taken), DomainError::MemberNameTaken);
    let bad = members::create(&db, &MemberInput { name: "Léo".into(), color: Some("bleu".into()) }).await;
    assert_eq!(domain_error(bad), DomainError::InvalidMemberColor);
    // Renommer sans changer de nom, ou en changeant la casse, reste possible.
    members::update(&db, claire, &MemberInput { name: "claire".into(), color: Some("#1a73e8".into()) }).await.unwrap();

    let list = members::list(&db).await.unwrap();
    assert_eq!(list.iter().map(|m| m.id).collect::<Vec<_>>(), vec![claire, quentin], "après le dernier");
    assert_eq!(list[0].color.as_deref(), Some("#1a73e8"));
    members::reorder(&db, &members::MemberReorderInput { ids: vec![quentin, claire] }).await.unwrap();
    assert_eq!(members::list(&db).await.unwrap()[0].id, quentin);
}

#[tokio::test]
async fn r_mbr_3_the_household_section_turns_off_only_without_members() {
    let (db, _dir) = setup().await;
    assert!(!features::get(&db).await.unwrap().household, "désactivée par défaut");
    let on = FeaturesInput { household: Some(true), ..Default::default() };
    assert!(features::update(&db, &on).await.unwrap().household);
    let claire = member(&db, "Claire").await;
    let off = FeaturesInput { household: Some(false), ..Default::default() };
    assert!(matches!(features::update(&db, &off).await, Err(AppError::Invalid { .. })));
    members::delete(&db, claire).await.unwrap();
    assert!(!features::update(&db, &off).await.unwrap().household);
}

/// Critères d'acceptation de MBR-01 : supprimer un membre remet ses affectations au foyer, sans changer de solde.
#[tokio::test]
async fn r_mbr_2_deleting_a_member_returns_its_assignments_to_the_household() {
    let (db, _dir) = setup().await;
    let claire = member(&db, "Claire").await;
    let livret = accounts::create(&db, &account_input("Livret de Claire", Some(claire)), d(TODAY)).await.unwrap();
    assert_eq!(livret.member_id, Some(claire));
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let gift = TransactionInput {
        splits: Some(vec![SplitInput { member_id: Some(claire), ..split(-3_000, None) }]),
        ..tx(checking, "2026-03-10", -3_000, None)
    };
    let gift = transactions::create(&db, &gift).await.unwrap();
    let pay = category(&db, "Paie", CategoryKind::Income, None).await;
    let job = EmploymentInput {
        employer: "Acme".into(),
        job_title: None,
        start_date: d("2025-01-01"),
        end_date: None,
        income_category_id: pay,
        raise_month: None,
        paid_next_month: false,
        notes: None,
        member_id: Some(claire),
    };
    let job = salaries::create(&db, &job, d(TODAY)).await.unwrap();
    assert_eq!(job.summary.member_id, Some(claire));
    let counts = &members::list(&db).await.unwrap()[0];
    assert_eq!((counts.account_count, counts.split_count, counts.employment_count), (1, 1, 1));

    members::delete(&db, claire).await.unwrap();
    let livret = accounts::get(&db, livret.id, d(TODAY)).await.unwrap();
    assert_eq!((livret.member_id, livret.balance_minor), (None, 50_000));
    assert_eq!(transactions::get(&db, gift).await.unwrap().splits[0].member_id, None);
    assert_eq!(salaries::get(&db, job.summary.id, d(TODAY)).await.unwrap().summary.member_id, None);
    assert!(matches!(members::delete(&db, claire).await, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn r_mbr_4_an_account_has_at_most_one_member_that_must_exist() {
    let (db, _dir) = setup().await;
    let quentin = member(&db, "Quentin").await;
    let created = accounts::create(&db, &account_input("Livret", None), d(TODAY)).await.unwrap();
    assert_eq!(created.member_id, None, "sans membre : compte du foyer");
    let updated = accounts::update(&db, created.id, &account_input("Livret", Some(quentin)), d(TODAY)).await.unwrap();
    assert_eq!(updated.member_id, Some(quentin));
    let unknown = accounts::create(&db, &account_input("Autre", Some(MemberId::new())), d(TODAY)).await;
    assert!(matches!(unknown, Err(AppError::NotFound(_))));
}

/// Critères d'acceptation de MBR-03 : chaque ventilation a son membre, jamais une ventilation de virement.
#[tokio::test]
async fn r_mbr_5_each_split_has_its_own_member_except_a_transfer_split() {
    let (db, _dir) = setup().await;
    let leo = member(&db, "Léo").await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let clothes = category(&db, "Habillement", CategoryKind::Expense, None).await;
    let input = TransactionInput {
        splits: Some(vec![
            split(-6_000, Some(food)),
            SplitInput { member_id: Some(leo), ..split(-4_000, Some(clothes)) },
        ]),
        ..tx(checking, "2026-03-10", -10_000, None)
    };
    let id = transactions::create(&db, &input).await.unwrap();
    let detail = transactions::get(&db, id).await.unwrap();
    assert_eq!(detail.splits.iter().map(|s| s.member_id).collect::<Vec<_>>(), vec![None, Some(leo)]);

    let savings: AccountId = account(&db, "Livret", AccountKind::LivretA, 0).await;
    let transfer = TransferInput {
        from_account_id: checking,
        to_account_id: savings,
        amount_minor: 1_000,
        date: d("2026-03-11"),
        label: "Épargne".into(),
        note: None,
    };
    let leg = transfers::create(&db, &transfer).await.unwrap().outflow_transaction_id;
    let update = TransactionUpdateInput {
        account_id: checking,
        date: d("2026-03-11"),
        amount_minor: -1_000,
        label: "Épargne".into(),
        note: None,
        splits: Some(vec![SplitInput { member_id: Some(leo), ..split(-1_000, None) }]),
        payee_id: None,
    };
    assert_eq!(domain_error(transactions::update(&db, leg, &update).await), DomainError::MemberOnTransfer);
}

/// Réappliquer une règle qui ventile garde le membre commun aux anciennes ventilations (R-MBR-5).
#[tokio::test]
async fn r_mbr_5_reapplying_rules_keeps_the_common_member() {
    let (db, _dir) = setup().await;
    let leo = member(&db, "Léo").await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let school = category(&db, "Scolarité", CategoryKind::Expense, None).await;
    let input = TransactionInput {
        label: "CANTINE".into(),
        splits: Some(vec![SplitInput { member_id: Some(leo), ..split(-5_000, None) }]),
        ..tx(checking, "2026-03-10", -5_000, None)
    };
    let id = transactions::create(&db, &input).await.unwrap();
    let rule = RuleInput {
        name: "Cantine".into(),
        priority: 0,
        conditions: Conditions {
            label: Some(LabelCondition { op: LabelOp::Contains, value: "CANTINE".into(), field: LabelField::Label }),
            ..Default::default()
        },
        actions: Actions {
            set_splits: vec![
                SplitShare { category_id: Some(food), weight: 5_000 },
                SplitShare { category_id: Some(school), weight: 5_000 },
            ],
            ..Default::default()
        },
        stop_processing: false,
        active: true,
    };
    let rule = rules::save(&db, None, &rule).await.unwrap();
    rules::apply(&db, rule.id, &ApplyInput { transaction_ids: vec![id] }).await.unwrap();
    let detail = transactions::get(&db, id).await.unwrap();
    assert_eq!(detail.splits.len(), 2);
    assert!(detail.splits.iter().all(|s| s.member_id == Some(leo)));
}

#[tokio::test]
async fn r_mbr_7_an_employment_has_at_most_one_member() {
    let (db, _dir) = setup().await;
    let quentin = member(&db, "Quentin").await;
    let pay = category(&db, "Paie", CategoryKind::Income, None).await;
    let mut job = EmploymentInput {
        employer: "Acme".into(),
        job_title: None,
        start_date: d("2025-01-01"),
        end_date: None,
        income_category_id: pay,
        raise_month: None,
        paid_next_month: false,
        notes: None,
        member_id: None,
    };
    let created = salaries::create(&db, &job, d(TODAY)).await.unwrap();
    assert_eq!(created.summary.member_id, None, "sans membre : emploi du foyer");
    job.member_id = Some(quentin);
    let updated = salaries::update(&db, created.summary.id, &job, d(TODAY)).await.unwrap();
    assert_eq!(updated.summary.member_id, Some(quentin));
}

/// Critères d'acceptation de MBR-04 : patrimoine par titulaire, flux par membre retenu, somme des cases = tout.
#[tokio::test]
async fn r_mbr_6_r_mbr_9_r_mbr_10_member_selection_filters_net_worth_and_flows() {
    let (db, _dir) = setup().await;
    let quentin = member(&db, "Quentin").await;
    let claire = member(&db, "Claire").await;
    let mut checking_input = account_input("Courant de Quentin", Some(quentin));
    checking_input.kind = AccountKind::Checking;
    checking_input.opening_balance_minor = 100_000;
    let checking = accounts::create(&db, &checking_input, d(TODAY)).await.unwrap().id;
    accounts::create(&db, &account_input("Livret de Claire", Some(claire)), d(TODAY)).await.unwrap();
    let joint = account(&db, "Joint", AccountKind::Checking, 200_000).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let spend = |account, amount, member: Option<MemberId>| TransactionInput {
        splits: Some(vec![SplitInput { member_id: member, ..split(amount, Some(food)) }]),
        ..tx(account, "2026-03-10", amount, None)
    };
    transactions::create(&db, &spend(checking, -5_000, None)).await.unwrap();
    transactions::create(&db, &spend(checking, -3_000, Some(claire))).await.unwrap();
    transactions::create(&db, &spend(joint, -2_000, None)).await.unwrap();

    let home = |members: Option<String>| {
        let db = db.clone();
        async move { dashboard::get_for(&db, &DashboardQuery { members }, d(TODAY)).await.unwrap() }
    };
    let only = |token: String| Some(token);
    let claire_home = home(only(claire.to_string())).await;
    assert_eq!(claire_home.net_worth.total_minor, 50_000, "seulement le livret de Claire");
    assert_eq!(claire_home.month.expenses_minor, 3_000);
    assert_eq!(home(only(quentin.to_string())).await.month.expenses_minor, 5_000, "titulaire du compte");
    let household = home(only(HOUSEHOLD.into())).await;
    assert_eq!((household.month.expenses_minor, household.net_worth.total_minor), (2_000, 198_000));
    let all = home(Some(format!("{HOUSEHOLD},{quentin},{claire}"))).await;
    assert_eq!(all.month.expenses_minor, 10_000);
    assert_eq!(all.net_worth.total_minor, home(None).await.net_worth.total_minor);

    let evolution = |members| {
        let db = db.clone();
        async move {
            let query = EvolutionQuery { months: Some(12), planned_projects: None, members };
            reports::evolution(&db, &query, d(TODAY)).await
        }
    };
    let partial = evolution(only(claire.to_string())).await.unwrap();
    assert!(partial.partial_members);
    assert_eq!(partial.months.last().unwrap().expenses_minor, 3_000);
    assert!(!evolution(None).await.unwrap().partial_members);
    let bad = members::selection(&db, Some("pas-un-membre")).await;
    assert!(matches!(bad, Err(AppError::Invalid { .. })));
}
