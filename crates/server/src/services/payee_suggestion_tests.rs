//! Tests d'intégration des propositions de tiers (TIE-05, R-TIE-30 à R-TIE-35).

use cashflow_domain::DomainError;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp, SplitShare};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, PayeeId, TransactionId};

use super::imports::{self, Upload};
use super::payee_suggestions::{self, AcceptSuggestionInput, PayeeKeyInput};
use super::payees::{self, PayeeInput};
use super::reconciliation::{self, CheckpointInput};
use super::rules::{self, RuleInput};
use super::tests::{TODAY, account, category, d, domain_error, setup, tx};
use super::transactions;
use super::transfers::{self, MarkAsTransferInput};
use crate::config::DataDir;
use crate::db::Db;
use crate::error::AppError;

async fn spent(db: &Db, account: AccountId, date: &str, label: &str) -> TransactionId {
    let input = transactions::TransactionInput { label: label.into(), ..tx(account, date, -1_000, None) };
    transactions::create(db, &input).await.unwrap()
}

async fn payee(db: &Db, name: &str) -> PayeeId {
    payees::create(db, &PayeeInput { name: name.into(), group_id: None, archived: false, note: None, address: None })
        .await
        .unwrap()
}

fn create(key: &str, name: &str, create_rule: bool) -> AcceptSuggestionInput {
    AcceptSuggestionInput { key: key.into(), payee_id: None, name: Some(name.into()), create_rule }
}

/// Critères d'acceptation de TIE-05 : regroupement par clé, tous comptes, au moins deux transactions.
#[tokio::test]
async fn r_tie_30_r_tie_31_unassigned_transactions_with_the_same_key_are_suggested() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let joint = account(&db, "Joint", AccountKind::Checking, 0).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    spent(&db, checking, "2026-03-14", "CB CARREFOUR MEYLAN 14/03").await;
    spent(&db, checking, "2026-03-02", "CB CARREFOUR MEYLAN 02/03").await;
    spent(&db, joint, "2026-03-20", "PRLV SEPA CARREFOUR MEYLAN").await;
    spent(&db, checking, "2026-03-02", "CB BOULANGERIE DUPONT 02/03").await;
    // Une jambe de virement n'a pas de tiers : elle n'est jamais proposée (R-TIE-11).
    for day in ["2026-03-05", "2026-03-06"] {
        let leg = spent(&db, checking, day, "VIR SEPA EPARGNE MENSUELLE").await;
        let mark =
            MarkAsTransferInput { counterpart_account_id: livret, counterpart_transaction_id: None, confirm: true };
        transfers::mark_as_transfer(&db, leg, &mark).await.unwrap();
    }

    let list = payee_suggestions::list(&db).await.unwrap();
    assert_eq!(list.suggestions.len(), 1, "{:?}", list.suggestions);
    let s = &list.suggestions[0];
    assert_eq!((s.key.as_str(), s.name.as_str()), ("CARREFOUR MEYLAN", "Carrefour Meylan"));
    assert_eq!(s.transaction_count, 3);
    assert_eq!((s.first_date, s.last_date), (d("2026-03-02"), d("2026-03-20")));
    assert_eq!(s.labels[0], "PRLV SEPA CARREFOUR MEYLAN");
    assert_eq!(s.payee_id, None);
}

#[tokio::test]
async fn r_tie_32_an_existing_payee_with_the_same_key_is_suggested() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let edf = payee(&db, "EDF").await;
    let input = transactions::TransactionInput {
        label: "CB EDF 05/01".into(),
        payee_id: Some(edf),
        ..tx(checking, "2026-01-05", -6_000, None)
    };
    transactions::create(&db, &input).await.unwrap();
    spent(&db, checking, "2026-02-05", "PRLV SEPA EDF").await;
    spent(&db, checking, "2026-03-05", "PRLV SEPA EDF").await;

    let s = &payee_suggestions::list(&db).await.unwrap().suggestions[0];
    assert_eq!((s.name.as_str(), s.payee_id, s.payee_name.as_deref()), ("Edf", Some(edf), Some("EDF")));
}

/// Validation : tiers créé, affecté à toutes les transactions sans tiers (même rapprochées), catégories intactes,
/// et règle qui le propose aux imports suivants, avant les règles de catégorie (R-TIE-33, R-TIE-34).
#[tokio::test]
async fn r_tie_33_r_tie_34_accepting_assigns_the_payee_and_creates_a_rule() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path().join("data"));
    data.ensure().unwrap();
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let food = category(&db, "Alimentation", CategoryKind::Expense, None).await;
    let categorized = transactions::TransactionInput {
        label: "CB CARREFOUR MEYLAN 02/03".into(),
        ..tx(checking, "2026-03-02", -1_000, Some(food))
    };
    let first = transactions::create(&db, &categorized).await.unwrap();
    spent(&db, checking, "2026-03-14", "CB CARREFOUR MEYLAN 14/03").await;
    spent(&db, checking, "2026-03-20", "PRLV SEPA CARREFOUR MEYLAN").await;
    let checkpoint = reconciliation::create(
        &db,
        checking,
        &CheckpointInput { date: d("2026-03-05"), balance_minor: 99_000, replace: false },
    )
    .await
    .unwrap();
    reconciliation::reconcile(&db, checkpoint.id).await.unwrap();
    assert!(transactions::get(&db, first).await.unwrap().locked);
    // Une règle de catégorie qui arrête le traitement ne doit pas empêcher la règle de tiers.
    let category_rule = RuleInput {
        name: "Courses".into(),
        priority: 0,
        conditions: Conditions {
            label: Some(LabelCondition {
                op: LabelOp::Contains,
                value: "CARREFOUR".into(),
                field: LabelField::BankLabel,
            }),
            ..Default::default()
        },
        actions: Actions {
            set_splits: vec![SplitShare { category_id: Some(food), weight: 10_000 }],
            ..Default::default()
        },
        stop_processing: true,
        active: true,
    };
    rules::save(&db, None, &category_rule).await.unwrap();

    let result = payee_suggestions::accept(&db, &create("CARREFOUR MEYLAN", "Carrefour Meylan", true)).await.unwrap();
    assert_eq!(result.assigned, 3);
    let filter = transactions::TransactionFilter { payee_id: Some(result.payee_id), ..Default::default() };
    assert_eq!(transactions::list_all(&db, &filter).await.unwrap().total, 3);
    let detail = transactions::get(&db, first).await.unwrap();
    assert_eq!(detail.splits[0].category_id, Some(food), "les catégories ne changent pas");
    assert!(payee_suggestions::list(&db).await.unwrap().suggestions.is_empty());

    let rule = rules::list(&db).await.unwrap().into_iter().find(|r| Some(r.id) == result.rule_id).unwrap();
    assert_eq!(rule.name, "Tiers : Carrefour Meylan");
    assert!(!rule.stop_processing);
    assert!(rule.priority < 0);
    let upload = Upload {
        file_name: "releve.csv".into(),
        content: b"Date;Libelle;Montant\n20/03/2026;CB CARREFOUR MEYLAN 20/03;-12,00\n".to_vec(),
        account_id: checking,
        profile_id: None,
        format: None,
        settings: Some(
            serde_json::from_str(
                r#"{"header_row": 1, "date": {"column": "Date", "format": "dd/MM/yyyy"},
                    "label": {"columns": ["Libelle"]}, "amount": {"mode": "single", "column": "Montant"}}"#,
            )
            .unwrap(),
        ),
        save_profile_as: None,
    };
    let created = imports::start(&db, &data, &upload, d(TODAY)).await.unwrap();
    let batch = imports::get(&db, &data, created.batch_id).await.unwrap();
    let line = &batch.lines[0];
    assert_eq!(line.payee_id, Some(result.payee_id));
    assert_eq!(line.splits[0].category_id, Some(food), "la règle de catégorie complète");
}

#[tokio::test]
async fn r_tie_33_accepting_with_an_existing_payee_and_without_rule() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let edf = payee(&db, "EDF").await;
    spent(&db, checking, "2026-02-05", "PRLV SEPA EDF").await;
    spent(&db, checking, "2026-03-05", "PRLV SEPA EDF").await;
    let rules_before = rules::list(&db).await.unwrap().len();

    let accept = AcceptSuggestionInput { key: "EDF".into(), payee_id: Some(edf), name: None, create_rule: false };
    let result = payee_suggestions::accept(&db, &accept).await.unwrap();
    assert_eq!((result.payee_id, result.assigned, result.rule_id), (edf, 2, None));
    assert_eq!(rules::list(&db).await.unwrap().len(), rules_before);
    // Plus rien à valider pour cette clé.
    assert!(matches!(payee_suggestions::accept(&db, &accept).await, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn r_tie_33_name_taken_or_archived_payee_is_refused_and_nothing_changes() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let edf = payee(&db, "EDF").await;
    spent(&db, checking, "2026-02-05", "PRLV SEPA EDF").await;
    spent(&db, checking, "2026-03-05", "PRLV SEPA EDF").await;

    assert_eq!(
        domain_error(payee_suggestions::accept(&db, &create("EDF", "edf", true)).await),
        DomainError::PayeeNameTaken
    );
    let archived = PayeeInput { name: "EDF".into(), group_id: None, archived: true, note: None, address: None };
    payees::update(&db, edf, &archived).await.unwrap();
    let accept = AcceptSuggestionInput { key: "EDF".into(), payee_id: Some(edf), name: None, create_rule: true };
    assert_eq!(domain_error(payee_suggestions::accept(&db, &accept).await), DomainError::PayeeArchived);
    assert_eq!(payee_suggestions::list(&db).await.unwrap().suggestions[0].transaction_count, 2);
}

#[tokio::test]
async fn r_tie_35_ignored_keys_are_no_longer_suggested_until_restored() {
    let (db, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    spent(&db, checking, "2026-01-31", "FRAIS TENUE COMPTE").await;
    spent(&db, checking, "2026-02-28", "FRAIS TENUE COMPTE").await;
    let key = PayeeKeyInput { key: "FRAIS TENUE COMPTE".into() };

    payee_suggestions::ignore(&db, &key).await.unwrap();
    payee_suggestions::ignore(&db, &key).await.unwrap();
    spent(&db, checking, "2026-03-31", "FRAIS TENUE COMPTE").await;
    let list = payee_suggestions::list(&db).await.unwrap();
    assert!(list.suggestions.is_empty());
    assert_eq!(list.ignored.len(), 1);
    assert_eq!(list.ignored[0].key, "FRAIS TENUE COMPTE");

    payee_suggestions::restore(&db, &key).await.unwrap();
    let list = payee_suggestions::list(&db).await.unwrap();
    assert_eq!(list.suggestions[0].transaction_count, 3);
    assert!(list.ignored.is_empty());
}
