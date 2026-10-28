//! Tests d'intégration des tiers (spec 11).

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp, SplitShare};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::recurrence::Frequency;
use cashflow_domain::shared::ids::{AccountId, CategoryId, PayeeGroupId, PayeeId};

use super::imports::{self, CommitInput, Upload};
use super::payees::{
    self, AddressInput, LocationGrouping, LocationKind, LocationReportQuery, MergePayeeInput, PayeeGroupInput,
    PayeeInput, PayeeReportQuery, ReportGrouping, ReportPeriod,
};
use super::reconciliation::{self, CheckpointInput};
use super::recurring::{self, MaterializeInput, RecurringRuleInput};
use super::rules::{self, ApplyInput, PreviewQuery, RuleInput};
use super::tests::{TODAY, account, category, d, domain_error, setup, tx};
use super::transactions::{self, BulkPayeeInput, TransactionFilter, TransactionInput, TransactionUpdateInput};
use super::transfers::{self, MarkAsTransferInput};
use crate::config::DataDir;
use crate::db::Db;
use crate::error::AppError;

async fn group(db: &Db, name: &str) -> PayeeGroupId {
    payees::create_group(db, &PayeeGroupInput { name: name.into(), note: None }).await.unwrap()
}

async fn payee(db: &Db, name: &str, group: Option<PayeeGroupId>) -> PayeeId {
    payees::create(db, &PayeeInput { name: name.into(), group_id: group, archived: false, note: None, address: None })
        .await
        .unwrap()
}

fn paid(account: AccountId, date: &str, amount: i64, category: Option<CategoryId>, payee: PayeeId) -> TransactionInput {
    TransactionInput { payee_id: Some(payee), ..tx(account, date, amount, category) }
}

fn label_rule(name: &str, contains: &str, actions: Actions) -> RuleInput {
    RuleInput {
        name: name.into(),
        priority: 0,
        conditions: Conditions {
            label: Some(LabelCondition { op: LabelOp::Contains, value: contains.into(), field: LabelField::BankLabel }),
            ..Default::default()
        },
        actions,
        stop_processing: true,
        active: true,
    }
}

#[tokio::test]
async fn r_tie_1_r_tie_2_names_are_unique_whatever_the_case() {
    let (db, _dir) = setup().await;
    payee(&db, "Carrefour Meylan", None).await;
    let duplicate =
        PayeeInput { name: "CARREFOUR MEYLAN".into(), group_id: None, archived: false, note: None, address: None };
    assert_eq!(domain_error(payees::create(&db, &duplicate).await), DomainError::PayeeNameTaken);
    group(&db, "Carrefour").await;
    let duplicate = PayeeGroupInput { name: "carrefour".into(), note: None };
    assert_eq!(domain_error(payees::create_group(&db, &duplicate).await), DomainError::PayeeGroupNameTaken);
}

/// Critères d'acceptation de TIE-01.
#[tokio::test]
async fn r_tie_4_r_tie_5_r_tie_7_delete_group_keep_used_payees_and_merge() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let carrefour = group(&db, "Carrefour").await;
    let meylan = payee(&db, "Carrefour Meylan", Some(carrefour)).await;
    let market = payee(&db, "Carrefour Market Meylan", Some(carrefour)).await;
    let unused = payee(&db, "Jamais servi", None).await;
    for day in ["2026-03-01", "2026-03-08"] {
        transactions::create(&db, &paid(checking, day, -2_000, None, meylan)).await.unwrap();
    }
    let rule = rules::save(
        &db,
        None,
        &label_rule("Carrefour", "carrefour", Actions { set_payee: Some(meylan), ..Default::default() }),
    )
    .await
    .unwrap();

    assert_eq!(domain_error(payees::delete(&db, meylan).await), DomainError::PayeeInUse);
    payees::delete(&db, unused).await.unwrap();
    assert_eq!(
        domain_error(payees::merge(&db, meylan, &MergePayeeInput { target_id: meylan }).await),
        DomainError::InvalidPayeeMerge
    );

    let merged = payees::merge(&db, meylan, &MergePayeeInput { target_id: market }).await.unwrap();
    assert_eq!(merged.moved_transactions, 2);
    let filter = TransactionFilter { payee_id: Some(market), ..Default::default() };
    assert_eq!(transactions::list_all(&db, &filter).await.unwrap().total, 2);
    let rule = rules::list(&db).await.unwrap().into_iter().find(|r| r.id == rule.id).unwrap();
    assert_eq!(rule.actions.set_payee, Some(market));
    let list = payees::list(&db).await.unwrap();
    assert!(list.payees.iter().all(|p| p.id != meylan));

    payees::delete_group(&db, carrefour).await.unwrap();
    let list = payees::list(&db).await.unwrap();
    assert!(list.groups.is_empty());
    assert_eq!(list.payees.iter().find(|p| p.id == market).unwrap().group_id, None);
}

/// Critères d'acceptation de TIE-02.
#[tokio::test]
async fn r_tie_11_r_tie_12_r_tie_14_transfer_legs_reconciled_and_suggested_category() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let groceries = category(&db, "Courses", CategoryKind::Expense, None).await;
    let shop = payee(&db, "Carrefour Meylan", None).await;
    let other = payee(&db, "Autre", None).await;

    // R-TIE-14 : la catégorie de la dernière transaction du tiers est proposée.
    transactions::create(&db, &paid(checking, "2026-01-10", -3_000, Some(groceries), shop)).await.unwrap();
    let list = payees::list(&db).await.unwrap();
    assert_eq!(list.payees.iter().find(|p| p.id == shop).unwrap().suggested_category_id, Some(groceries));

    // R-TIE-11 : lier en virement retire le tiers, après confirmation.
    let leg = transactions::create(&db, &paid(checking, "2026-01-12", -10_000, None, shop)).await.unwrap();
    let mark = MarkAsTransferInput { counterpart_account_id: livret, counterpart_transaction_id: None, confirm: false };
    assert!(matches!(transfers::mark_as_transfer(&db, leg, &mark).await, Err(AppError::Invalid { .. })));
    transfers::mark_as_transfer(&db, leg, &MarkAsTransferInput { confirm: true, ..mark }).await.unwrap();
    let detail = transactions::get(&db, leg).await.unwrap();
    assert_eq!(detail.payee_id, None);
    let update = TransactionUpdateInput {
        account_id: checking,
        date: detail.date,
        amount_minor: detail.amount_minor,
        label: detail.label.clone(),
        note: None,
        splits: None,
        payee_id: Some(shop),
    };
    assert_eq!(domain_error(transactions::update(&db, leg, &update).await), DomainError::PayeeOnTransferLeg);
    let bulk = BulkPayeeInput { transaction_ids: vec![leg], payee_id: Some(shop) };
    let result = transactions::set_payee_many(&db, &bulk).await.unwrap();
    assert_eq!((result.updated, result.skipped), (0, 1));

    // R-TIE-12 : une transaction rapprochée change de tiers sans déverrouillage.
    let checkpoint = reconciliation::create(
        &db,
        checking,
        &CheckpointInput { date: d("2026-01-31"), balance_minor: 87_000, replace: false },
    )
    .await
    .unwrap();
    reconciliation::reconcile(&db, checkpoint.id).await.unwrap();
    let locked = transactions::list_all(&db, &TransactionFilter { payee_id: Some(shop), ..Default::default() })
        .await
        .unwrap()
        .items[0]
        .id;
    let detail = transactions::get(&db, locked).await.unwrap();
    assert!(detail.locked);
    let update = TransactionUpdateInput {
        account_id: checking,
        date: detail.date,
        amount_minor: detail.amount_minor,
        label: detail.label,
        note: None,
        splits: None,
        payee_id: Some(other),
    };
    transactions::update(&db, locked, &update).await.unwrap();
    assert_eq!(transactions::get(&db, locked).await.unwrap().payee_id, Some(other));
}

#[tokio::test]
async fn r_tie_13_recurring_rules_carry_their_payee() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let landlord = payee(&db, "Propriétaire", None).await;
    let rent = RecurringRuleInput {
        account_id: checking,
        target_account_id: None,
        label: "Loyer".into(),
        amount_minor: -80_000,
        amount_tolerance_minor: 0,
        frequency: Frequency::Monthly,
        interval: 1,
        anchor_date: d("2026-01-05"),
        end_date: None,
        split_template: Vec::new(),
        active: true,
        payee_id: Some(landlord),
    };
    let rule = recurring::save(&db, None, &rent, d(TODAY)).await.unwrap();
    assert_eq!(rule.payee_id, Some(landlord));
    let created = recurring::materialize(&db, rule.id, &MaterializeInput { date: d("2026-02-05") }).await.unwrap();
    assert_eq!(transactions::get(&db, created.transaction_id).await.unwrap().payee_id, Some(landlord));
    let transfer = RecurringRuleInput { target_account_id: Some(livret), ..rent };
    assert_eq!(domain_error(recurring::save(&db, None, &transfer, d(TODAY)).await), DomainError::PayeeOnTransferLeg);
    assert_eq!(domain_error(payees::delete(&db, landlord).await), DomainError::PayeeInUse);
}

#[tokio::test]
async fn tie_04_filters_by_payee_group_and_without_payee() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let carrefour = group(&db, "Carrefour").await;
    let meylan = payee(&db, "Carrefour Meylan", Some(carrefour)).await;
    let grenoble = payee(&db, "Carrefour Market Grenoble", Some(carrefour)).await;
    let bakery = payee(&db, "Boulangerie Dupont", None).await;
    for (p, amount) in [(meylan, -1_000), (grenoble, -2_000), (bakery, -300)] {
        transactions::create(&db, &paid(checking, "2026-03-02", amount, None, p)).await.unwrap();
    }
    transactions::create(&db, &tx(checking, "2026-03-03", -500, None)).await.unwrap();
    let count = async |filter: TransactionFilter| transactions::list(&db, checking, &filter).await.unwrap().total;
    assert_eq!(count(TransactionFilter { payee_group_id: Some(carrefour), ..Default::default() }).await, 2);
    assert_eq!(count(TransactionFilter { payee_id: Some(bakery), ..Default::default() }).await, 1);
    assert_eq!(count(TransactionFilter { without_payee: Some(true), ..Default::default() }).await, 1);
}

/// Critère d'acceptation de TIE-04.
#[tokio::test]
async fn r_tie_20_r_tie_21_report_by_group_and_by_payee() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let carrefour = group(&db, "Carrefour").await;
    let meylan = payee(&db, "Carrefour Meylan", Some(carrefour)).await;
    let grenoble = payee(&db, "Carrefour Market Grenoble", Some(carrefour)).await;
    let bakery = payee(&db, "Boulangerie Dupont", None).await;
    for (p, amount, category) in [
        (meylan, -8_000, Some(food)),
        (grenoble, -2_000, Some(food)),
        (meylan, 500, Some(food)),
        (meylan, -1_500, None),
        (bakery, -400, Some(food)),
    ] {
        transactions::create(&db, &paid(checking, "2026-03-10", amount, category, p)).await.unwrap();
    }
    // Février : période précédente.
    transactions::create(&db, &paid(checking, "2026-02-10", -3_000, Some(food), grenoble)).await.unwrap();

    let query =
        PayeeReportQuery { period: Some(ReportPeriod::Month), month: Some("2026-03".into()), ..Default::default() };
    let report = payees::report(&db, &query, d(TODAY)).await.unwrap();
    assert_eq!(report.lines.len(), 2, "le groupe Carrefour et la boulangerie sans groupe");
    let group_line = &report.lines[0];
    assert_eq!(group_line.group_id, Some(carrefour));
    assert_eq!(group_line.payee_id, None);
    assert_eq!((group_line.amount_minor, group_line.uncategorized_minor), (9_500, 1_500));
    assert_eq!(group_line.previous_minor, 3_000);
    assert_eq!(group_line.children.len(), 2);
    let meylan_line = group_line.children.iter().find(|c| c.payee_id == Some(meylan)).unwrap();
    assert_eq!((meylan_line.amount_minor, meylan_line.transactions), (7_500, 3));
    assert_eq!(report.total_minor, 9_900);
    assert_eq!(report.uncategorized_minor, 1_500);

    let flat = payees::report(&db, &PayeeReportQuery { group_by: Some(ReportGrouping::Payee), ..query }, d(TODAY))
        .await
        .unwrap();
    assert_eq!(flat.lines.len(), 3);
    assert_eq!(flat.lines[0].payee_id, Some(meylan));

    let summary = payees::group_summary(&db, carrefour, d(TODAY)).await.unwrap();
    assert_eq!(summary.members.len(), 2);
    assert_eq!(summary.year.expenses_minor, 12_500);
    let march = summary.months.iter().find(|m| m.month == "2026-03").unwrap();
    assert_eq!(march.expenses_minor, 9_500);
    assert_eq!(march.by_payee.len(), 2);
    assert_eq!(summary.categories[0].category_id, Some(food));
}

// ---------------------------------------------------------------- Import et règles (TIE-03)

const SETTINGS: &str = r#"{
  "header_row": 1,
  "date": { "column": "Date", "format": "dd/MM/yyyy" },
  "label": { "columns": ["Libellé"] },
  "amount": { "mode": "single", "column": "Montant" }
}"#;

async fn import(db: &Db, data: &DataDir, account: AccountId, csv: &str) -> imports::BatchDetail {
    let upload = Upload {
        file_name: "releve.csv".into(),
        content: csv.as_bytes().to_vec(),
        account_id: account,
        profile_id: None,
        format: None,
        settings: Some(serde_json::from_str(SETTINGS).unwrap()),
        save_profile_as: None,
    };
    let created = imports::start(db, data, &upload, d(TODAY)).await.unwrap();
    imports::get(db, data, created.batch_id).await.unwrap()
}

/// Critères d'acceptation de TIE-03.
#[tokio::test]
async fn r_tie_16_r_tie_17_r_tie_18_import_suggests_payees() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path().join("data"));
    data.ensure().unwrap();
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let energy = category(&db, "Énergie", CategoryKind::Expense, None).await;
    let meylan = payee(&db, "Carrefour Meylan", None).await;
    let edf = payee(&db, "EDF", None).await;
    rules::save(
        &db,
        None,
        &label_rule("Carrefour", "carrefour meylan", Actions { set_payee: Some(meylan), ..Default::default() }),
    )
    .await
    .unwrap();
    // Une transaction du livret, au même libellé, rattachée à EDF et catégorisée (R-TIE-17, R-TIE-18).
    let earlier = import(&db, &data, livret, "Date;Libellé;Montant\n02/02/2026;PRLV EDF;-60,00\n").await;
    let line = &earlier.lines[0];
    imports::review_line(
        &db,
        earlier.summary.id,
        line.id,
        &imports::LineReviewInput {
            decision: line.decision.unwrap(),
            match_transaction_id: None,
            label: None,
            splits: vec![imports::DraftSplit { category_id: Some(energy), amount_minor: -6_000, note: None }],
            note: None,
            transfer_to: None,
            transfer_transaction_id: None,
            payee_id: Some(edf),
        },
    )
    .await
    .unwrap();
    imports::commit(&db, &data, earlier.summary.id, &CommitInput::default(), d(TODAY)).await.unwrap();
    let filter = TransactionFilter { payee_id: Some(edf), ..Default::default() };
    assert_eq!(transactions::list_all(&db, &filter).await.unwrap().total, 1, "le tiers choisi en revue est créé");

    let batch = import(
        &db,
        &data,
        checking,
        "Date;Libellé;Montant\n14/03/2026;CB CARREFOUR MEYLAN 14/03;-42,50\n15/03/2026;PRLV EDF;-61,00\n16/03/2026;CAFE;-2,50\n",
    )
    .await;
    let by_label = |label: &str| batch.lines.iter().find(|l| l.bank_label.as_deref() == Some(label)).unwrap();
    let carrefour = by_label("CB CARREFOUR MEYLAN 14/03");
    assert_eq!((carrefour.payee_id, carrefour.payee_learned), (Some(meylan), false));
    let power = by_label("PRLV EDF");
    assert_eq!((power.payee_id, power.payee_learned), (Some(edf), true));
    assert_eq!(power.splits[0].category_id, Some(energy));
    assert_eq!(by_label("CAFE").payee_id, None);
    imports::commit(&db, &data, batch.summary.id, &CommitInput::default(), d(TODAY)).await.unwrap();
    assert_eq!(transactions::list_all(&db, &filter).await.unwrap().total, 2);
}

#[tokio::test]
async fn r_tie_19_rules_on_demand_fill_missing_payees_only() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let meylan = payee(&db, "Carrefour Meylan", None).await;
    let other = payee(&db, "Autre", None).await;
    let carrefour = TransactionInput { label: "CARREFOUR MEYLAN".into(), ..tx(checking, "2026-03-01", -1_000, None) };
    let categorized =
        transactions::create(&db, &TransactionInput { category_id: Some(food), ..carrefour.clone() }).await.unwrap();
    let uncategorized = transactions::create(&db, &carrefour).await.unwrap();
    let already = transactions::create(
        &db,
        &TransactionInput { category_id: Some(food), payee_id: Some(other), ..carrefour.clone() },
    )
    .await
    .unwrap();
    let rule = label_rule(
        "Carrefour",
        "carrefour",
        Actions {
            set_payee: Some(meylan),
            set_splits: vec![SplitShare { category_id: Some(food), weight: 1 }],
            ..Default::default()
        },
    );
    let rule = RuleInput {
        conditions: Conditions {
            label: rule.conditions.label.map(|l| LabelCondition { field: LabelField::Label, ..l }),
            ..rule.conditions
        },
        ..rule
    };
    let without = rules::preview(&db, &rule.conditions, &PreviewQuery::default()).await.unwrap();
    assert_eq!(without.len(), 1, "sans set_payee, seules les transactions non catégorisées");
    let matches = rules::preview(&db, &rule.conditions, &PreviewQuery { sets_payee: Some(true) }).await.unwrap();
    assert_eq!(matches.len(), 2, "la transaction qui a déjà un tiers est exclue");
    let only_payee = matches.iter().find(|m| m.transaction_id == categorized).unwrap();
    assert!(only_payee.sets_payee && !only_payee.sets_category);

    let saved = rules::save(&db, None, &rule).await.unwrap();
    let apply = ApplyInput { transaction_ids: vec![categorized, uncategorized, already] };
    assert_eq!(rules::apply(&db, saved.id, &apply).await.unwrap().applied, 2);
    for id in [categorized, uncategorized] {
        let detail = transactions::get(&db, id).await.unwrap();
        assert_eq!(detail.payee_id, Some(meylan));
        assert_eq!(detail.splits[0].category_id, Some(food));
    }
    assert_eq!(transactions::get(&db, already).await.unwrap().payee_id, Some(other));
}

// ---------------------------------------------------------------- Adresse et lieu (TIE-06, TIE-07)

fn address(postal_code: &str, city: &str, country: &str, point: Option<(i32, i32)>) -> Option<AddressInput> {
    Some(AddressInput {
        postal_code: Some(postal_code.into()),
        city: Some(city.into()),
        country_code: Some(country.into()),
        latitude_e6: point.map(|p| p.0),
        longitude_e6: point.map(|p| p.1),
        ..Default::default()
    })
}

async fn located(db: &Db, name: &str, address: Option<AddressInput>) -> PayeeId {
    payees::create(db, &PayeeInput { name: name.into(), group_id: None, archived: false, note: None, address })
        .await
        .unwrap()
}

async fn address_of(db: &Db, id: PayeeId) -> Option<payees::AddressView> {
    payees::list(db).await.unwrap().payees.into_iter().find(|p| p.id == id).unwrap().address
}

/// Critères d'acceptation de TIE-06.
#[tokio::test]
async fn r_tie_23_r_tie_25_address_is_checked_and_hand_edits_drop_coordinates() {
    let (db, _dir) = setup().await;
    let no_city = Some(AddressInput { postal_code: Some("38240".into()), ..Default::default() });
    let refused = payees::create(
        &db,
        &PayeeInput { name: "Boulangerie".into(), group_id: None, archived: false, note: None, address: no_city },
    )
    .await;
    assert_eq!(domain_error(refused), DomainError::AddressIncomplete);

    let point = Some((49_897_442, 2_290_084));
    let shop = located(&db, "Boulangerie", address("80000", "Amiens", "fr", point)).await;
    let saved = address_of(&db, shop).await.unwrap();
    assert_eq!((saved.country_code.as_str(), saved.department_code.as_deref()), ("FR", Some("80")));
    assert_eq!(saved.latitude_e6, Some(49_897_442));

    // Correction à la main du code postal : les coordonnées reprises telles quelles sont retirées (R-TIE-25).
    let corrected = PayeeInput {
        name: "Boulangerie".into(),
        group_id: None,
        archived: false,
        note: None,
        address: address("80090", "Amiens", "FR", point),
    };
    payees::update(&db, shop, &corrected).await.unwrap();
    assert_eq!(address_of(&db, shop).await.unwrap().latitude_e6, None);
    // Sans adresse, le tiers n'en a plus.
    payees::update(&db, shop, &PayeeInput { address: None, ..corrected }).await.unwrap();
    assert!(address_of(&db, shop).await.is_none());
}

#[tokio::test]
async fn r_tie_27_merge_keeps_the_target_address_or_takes_the_source_one() {
    let (db, _dir) = setup().await;
    let source = located(&db, "Carrefour Meylan", address("38240", "Meylan", "FR", None)).await;
    let target = located(&db, "Carrefour", None).await;
    payees::merge(&db, source, &MergePayeeInput { target_id: target }).await.unwrap();
    assert_eq!(address_of(&db, target).await.unwrap().city, "Meylan");

    let other = located(&db, "Carrefour Grenoble", address("38000", "Grenoble", "FR", None)).await;
    payees::merge(&db, other, &MergePayeeInput { target_id: target }).await.unwrap();
    assert_eq!(address_of(&db, target).await.unwrap().city, "Meylan");
}

#[tokio::test]
async fn r_tie_26_address_search_is_refused_while_disabled() {
    let (db, _dir) = setup().await;
    // Ne joint rien : la recherche doit être refusée avant tout appel.
    let geocoder = crate::geocoding::Geocoder::new("http://127.0.0.1:9").unwrap();
    let refused = payees::search_address(&db, &geocoder, "8 bd du port").await;
    assert!(matches!(refused, Err(AppError::Invalid { code: "GEOCODING_DISABLED", .. })));
    let input = super::features::FeaturesInput { geocoding: Some(true), ..Default::default() };
    super::features::update(&db, &input).await.unwrap();
    // Trop court : aucune proposition, sans appel.
    assert!(payees::search_address(&db, &geocoder, "8 ").await.unwrap().is_empty());
    let unreachable = payees::search_address(&db, &geocoder, "8 bd du port").await;
    assert!(matches!(unreachable, Err(AppError::Upstream(_))));
}

/// Critère d'acceptation de TIE-07.
#[tokio::test]
async fn r_tie_24_r_tie_28_r_tie_29_report_by_city_department_and_map() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let bakery = located(&db, "Boulangerie", address("38240", "Meylan", "FR", Some((45_231_000, 5_781_000)))).await;
    let carrefour = located(&db, "Carrefour Meylan", address("38240", "MEYLAN", "FR", None)).await;
    let fnac = located(&db, "Fnac", address("38000", "Grenoble", "FR", Some((45_188_000, 5_724_000)))).await;
    let amazon = located(&db, "Amazon", None).await;
    let pub_ = located(&db, "Pub", address("SW1A 1AA", "London", "GB", None)).await;
    for (p, amount) in [(bakery, -4_000), (carrefour, -6_000), (fnac, -3_000), (amazon, -1_000), (pub_, -500)] {
        transactions::create(&db, &paid(checking, "2026-03-10", amount, Some(food), p)).await.unwrap();
    }
    let query = |group_by| LocationReportQuery {
        period: Some(ReportPeriod::Month),
        month: Some("2026-03".into()),
        group_by: Some(group_by),
        ..Default::default()
    };

    let by_city = payees::location_report(&db, &query(LocationGrouping::City), d(TODAY)).await.unwrap();
    let summary: Vec<_> = by_city.lines.iter().map(|l| (l.name.as_str(), l.amount_minor, l.payees.len())).collect();
    // « Meylan » et « MEYLAN » sont la même ville, nommée comme chez son tiers principal (Carrefour).
    assert_eq!(
        summary,
        [("MEYLAN", 10_000, 2), ("Grenoble", 3_000, 1), ("London", 500, 1), ("Sans adresse", 1_000, 1)]
    );
    assert_eq!(by_city.total_minor, 14_500);

    let by_department = payees::location_report(&db, &query(LocationGrouping::Department), d(TODAY)).await.unwrap();
    let isere = &by_department.lines[0];
    assert_eq!(
        (isere.name.as_str(), isere.department_code.as_deref(), isere.amount_minor),
        ("Isère", Some("38"), 13_000)
    );
    let kinds: Vec<_> = by_department.lines.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, [LocationKind::Department, LocationKind::Abroad, LocationKind::NoAddress]);

    // Carte : l'Isère, deux tiers localisés ; Carrefour (sans coordonnées), Londres et Amazon hors carte.
    assert_eq!(by_city.departments.len(), 1);
    assert_eq!(by_city.departments[0].amount_minor, 13_000);
    assert_eq!(by_city.points.len(), 2);
    assert_eq!(by_city.off_map, 3);

    let by_country = payees::location_report(&db, &query(LocationGrouping::Country), d(TODAY)).await.unwrap();
    let countries: Vec<_> = by_country.lines.iter().map(|l| (l.name.as_str(), l.amount_minor)).collect();
    assert_eq!(countries, [("FR", 13_000), ("GB", 500), ("Sans adresse", 1_000)]);
}
