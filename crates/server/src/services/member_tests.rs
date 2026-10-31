//! Tests d'intégration du foyer (MBR-01 à MBR-03, R-MBR-1 à R-MBR-7).

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::assets::AssetKind;
use cashflow_domain::documents::{DocumentKind, LinkKind, LinkTarget};
use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp, SplitShare};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, MemberId};

use super::accounts::{self, AccountInput};
use super::assets;
use super::dashboard::{self, DashboardQuery};
use super::documents;
use super::features::{self, FeaturesInput};
use super::members::HOUSEHOLD;
use super::members::{self, MemberInput};
use super::reports::{self, EvolutionQuery};
use super::rules::{self, ApplyInput, RuleInput};
use super::salaries::{self, EmploymentInput};
use super::savings::{self, SavingsOverviewQuery};
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
        reassign_splits: false,
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

/// Critères d'acceptation de MBR-04 : patrimoine par titulaire, flux par membre, somme des cases = tout.
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
    // Sans ventilation saisie, la dépense revient au titulaire du compte (R-MBR-6).
    transactions::create(&db, &tx(checking, "2026-03-10", -5_000, Some(food))).await.unwrap();
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

/// L'encadré Épargne ne compte que les comptes des titulaires cochés (R-MBR-9).
#[tokio::test]
async fn r_mbr_9_savings_overview_counts_the_checked_titulars() {
    let (db, _dir) = setup().await;
    let claire = member(&db, "Claire").await;
    accounts::create(&db, &account_input("Livret de Claire", Some(claire)), d(TODAY)).await.unwrap();
    accounts::create(&db, &account_input("Livret du foyer", None), d(TODAY)).await.unwrap();
    let overview = |members: Option<String>| {
        let db = db.clone();
        async move { savings::overview(&db, &SavingsOverviewQuery { members }, d(TODAY)).await.unwrap() }
    };
    assert_eq!(overview(None).await.emergency_fund_minor, 100_000);
    assert_eq!(overview(Some(claire.to_string())).await.emergency_fund_minor, 50_000);
    assert_eq!(overview(Some(HOUSEHOLD.into())).await.emergency_fund_minor, 50_000);
}

/// Affectation en masse : toutes les ventilations, même rapprochées ; les virements sont ignorés (R-MBR-5).
#[tokio::test]
async fn r_mbr_5_members_are_assigned_in_bulk_except_transfers() {
    let (db, _dir) = setup().await;
    let leo = member(&db, "Léo").await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let savings = account(&db, "Livret", AccountKind::LivretA, 0).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let input = TransactionInput {
        splits: Some(vec![split(-6_000, Some(food)), split(-4_000, None)]),
        ..tx(checking, "2026-03-10", -10_000, None)
    };
    let spent = transactions::create(&db, &input).await.unwrap();
    let transfer = TransferInput {
        from_account_id: checking,
        to_account_id: savings,
        amount_minor: 1_000,
        date: d("2026-03-11"),
        label: "Épargne".into(),
        note: None,
    };
    let leg = transfers::create(&db, &transfer).await.unwrap().outflow_transaction_id;

    let bulk = transactions::BulkMemberInput { transaction_ids: vec![spent, leg], member_id: Some(leo) };
    let result = transactions::set_member_many(&db, &bulk).await.unwrap();
    assert_eq!((result.updated, result.skipped), (1, 1));
    assert!(transactions::get(&db, spent).await.unwrap().splits.iter().all(|s| s.member_id == Some(leo)));
    assert_eq!(transactions::get(&db, leg).await.unwrap().splits[0].member_id, None);

    let back = transactions::BulkMemberInput { transaction_ids: vec![spent], member_id: None };
    transactions::set_member_many(&db, &back).await.unwrap();
    assert!(transactions::get(&db, spent).await.unwrap().splits.iter().all(|s| s.member_id.is_none()));
}

/// Le titulaire du compte est le membre par défaut d'une dépense ; « Foyer » se choisit explicitement (R-MBR-6).
#[tokio::test]
async fn r_mbr_6_the_account_holder_is_the_default_member_and_household_can_be_chosen() {
    let (db, _dir) = setup().await;
    let quentin = member(&db, "Quentin").await;
    let mut input = account_input("Courant de Quentin", Some(quentin));
    input.kind = AccountKind::Checking;
    let checking = accounts::create(&db, &input, d(TODAY)).await.unwrap().id;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;

    let own = transactions::create(&db, &tx(checking, "2026-03-10", -5_000, Some(food))).await.unwrap();
    assert_eq!(transactions::get(&db, own).await.unwrap().splits[0].member_id, Some(quentin));
    let shared =
        TransactionInput { splits: Some(vec![split(-8_000, Some(food))]), ..tx(checking, "2026-03-11", -8_000, None) };
    let shared = transactions::create(&db, &shared).await.unwrap();
    assert_eq!(transactions::get(&db, shared).await.unwrap().splits[0].member_id, None, "foyer");
    assert_eq!(features::get(&db).await.unwrap().household_split_count, 1);
}

/// Changer le titulaire d'un compte passe, sur demande, ses ventilations de l'ancien au nouveau (R-MBR-4).
#[tokio::test]
async fn r_mbr_4_changing_the_holder_reassigns_splits_on_request() {
    let (db, _dir) = setup().await;
    let claire = member(&db, "Claire").await;
    let leo = member(&db, "Léo").await;
    let joint = accounts::create(&db, &account_input("Joint", None), d(TODAY)).await.unwrap().id;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let common = transactions::create(&db, &tx(joint, "2026-03-10", -5_000, Some(food))).await.unwrap();
    let gift = TransactionInput {
        splits: Some(vec![SplitInput { member_id: Some(leo), ..split(-2_000, Some(food)) }]),
        ..tx(joint, "2026-03-11", -2_000, None)
    };
    let gift = transactions::create(&db, &gift).await.unwrap();
    let member_of = |id| {
        let db = db.clone();
        async move { transactions::get(&db, id).await.unwrap().splits[0].member_id }
    };

    let without = AccountInput { reassign_splits: false, ..account_input("Joint", Some(claire)) };
    accounts::update(&db, joint, &without, d(TODAY)).await.unwrap();
    assert_eq!(member_of(common).await, None, "sans reprise, la dépense reste au foyer");

    let back = account_input("Joint", None);
    accounts::update(&db, joint, &back, d(TODAY)).await.unwrap();
    let with = AccountInput { reassign_splits: true, ..account_input("Joint", Some(claire)) };
    accounts::update(&db, joint, &with, d(TODAY)).await.unwrap();
    assert_eq!(member_of(common).await, Some(claire));
    assert_eq!(member_of(gift).await, Some(leo), "une affectation à un autre membre ne bouge pas");
}

fn house() -> assets::AssetInput {
    assets::AssetInput {
        name: "Maison".into(),
        kind: AssetKind::RealEstate,
        purchase_date: d("2025-01-01"),
        purchase_price_minor: 42_000_000,
        sold_on: None,
        sale_price_minor: None,
        include_in_net_worth: None,
        notes: None,
    }
}

fn share(member: MemberId, bp: Option<u32>, amount: Option<i64>) -> assets::AssetShareInput {
    assets::AssetShareInput { member_id: member, share_bp: bp, amount_minor: amount }
}

/// Critères d'acceptation de MBR-06 : quotes-parts, reste au foyer, saisie en montant, plafond de 100 %.
#[tokio::test]
async fn r_mbr_11_r_mbr_12_an_asset_is_shared_between_members_with_the_rest_to_the_household() {
    let (db, _dir) = setup().await;
    features::update(&db, &FeaturesInput { assets: Some(true), ..Default::default() }).await.unwrap();
    let quentin = member(&db, "Quentin").await;
    let claire = member(&db, "Claire").await;
    let id = assets::create(&db, &house(), d(TODAY)).await.unwrap().summary.id;

    let input =
        assets::AssetSharesInput { shares: vec![share(quentin, Some(6_000), None), share(claire, Some(3_000), None)] };
    let view = assets::set_shares(&db, id, &input, d(TODAY)).await.unwrap().summary;
    let values: Vec<_> = view.shares.iter().map(|s| (s.share_bp, s.value_minor)).collect();
    assert_eq!(values, vec![(6_000, Some(25_200_000)), (3_000, Some(12_600_000))]);
    assert_eq!((view.household_share_bp, view.household_value_minor), (1_000, Some(4_200_000)));

    let home = |members: Option<String>| {
        let db = db.clone();
        async move { dashboard::get_for(&db, &DashboardQuery { members }, d(TODAY)).await.unwrap().net_worth }
    };
    assert_eq!(home(Some(quentin.to_string())).await.total_minor, 25_200_000);
    assert_eq!(home(Some(HOUSEHOLD.into())).await.total_minor, 4_200_000);
    assert_eq!(home(None).await.total_minor, 42_000_000);

    // Un montant devient une quote-part d'après la valeur du jour, qui reste quand la valeur change.
    let by_amount = assets::AssetSharesInput { shares: vec![share(claire, None, Some(10_500_000))] };
    assert_eq!(assets::set_shares(&db, id, &by_amount, d(TODAY)).await.unwrap().summary.shares[0].share_bp, 2_500);
    let valuation = assets::AssetValuationInput { date: d("2026-03-01"), value_minor: 44_000_000, note: None };
    let view = assets::save_valuation(&db, id, &valuation, d(TODAY)).await.unwrap().summary;
    assert_eq!(view.shares[0].value_minor, Some(11_000_000));

    let over =
        assets::AssetSharesInput { shares: vec![share(quentin, Some(7_000), None), share(claire, Some(4_000), None)] };
    assert_eq!(domain_error(assets::set_shares(&db, id, &over, d(TODAY)).await), DomainError::InvalidAssetShares);

    // Supprimer un membre rend sa quote-part au foyer (R-MBR-2).
    members::delete(&db, claire).await.unwrap();
    let view = assets::get(&db, id, d(TODAY)).await.unwrap().summary;
    assert!(view.shares.is_empty());
    assert_eq!(view.household_share_bp, 10_000);
}

/// Critères d'acceptation de MBR-07 : membre par défaut d'un relevé et d'une fiche de paie, « Personne », filtre.
#[tokio::test]
async fn r_mbr_13_r_mbr_14_documents_get_the_account_or_employment_member_by_default() {
    let (db, dir) = setup().await;
    let data = crate::config::DataDir::new(dir.path().join("data"));
    data.ensure().unwrap();
    let quentin = member(&db, "Quentin").await;
    let claire = member(&db, "Claire").await;
    let mut input = account_input("Courant de Quentin", Some(quentin));
    input.kind = AccountKind::Checking;
    let checking = accounts::create(&db, &input, d(TODAY)).await.unwrap().id;
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
    let job = salaries::create(&db, &job, d(TODAY)).await.unwrap().summary.id;
    let upload = |name: &str, kind, target| documents::Upload {
        file_name: name.into(),
        content: format!("%PDF-1.4 {name}").into_bytes(),
        kind,
        month: kind.map(|_| d("2026-02-01")),
        target,
        ..Default::default()
    };
    let on_account = LinkTarget { kind: LinkKind::Account, id: *checking.as_uuid() };
    let on_job = LinkTarget { kind: LinkKind::Employment, id: *job.as_uuid() };

    let statement = documents::create(
        &db,
        &data,
        upload("releve.pdf", Some(DocumentKind::BankStatement), Some(on_account)),
        d(TODAY),
    )
    .await
    .unwrap();
    assert_eq!(statement.member_id, Some(quentin));
    let payslip =
        documents::create(&db, &data, upload("fiche.pdf", Some(DocumentKind::Payslip), Some(on_job)), d(TODAY))
            .await
            .unwrap();
    assert_eq!(payslip.member_id, Some(claire));
    let invoice = documents::create(&db, &data, upload("facture.pdf", None, None), d(TODAY)).await.unwrap();
    assert_eq!(invoice.member_id, None);

    let to_nobody = documents::DocumentInput {
        kind: statement.kind,
        title: statement.title.clone(),
        document_date: None,
        period_start: None,
        period_end: None,
        month: statement.month,
        note: None,
        member_id: None,
    };
    assert_eq!(documents::update(&db, statement.id, &to_nobody, d(TODAY)).await.unwrap().member_id, None);
    let nobody = documents::DocumentFilter { without_member: true, ..Default::default() };
    assert_eq!(documents::list(&db, &nobody, d(TODAY)).await.unwrap().len(), 2);
    let of_claire = documents::DocumentFilter { member_id: Some(claire), ..Default::default() };
    assert_eq!(documents::list(&db, &of_claire, d(TODAY)).await.unwrap()[0].id, payslip.id);
}
