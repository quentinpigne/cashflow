//! Tests d'intégration de l'import et du rapprochement (specs 03 et 04).

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp, SplitShare};
use cashflow_domain::import::{ImportDecision, MatchKind};
use cashflow_domain::ledger::ClearingStatus;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::AccountId;
use cashflow_domain::{Date, DomainError};

use super::accounts::{self, AccountInput};
use super::imports::{self, CommitInput, Upload};
use super::reconciliation::{self, CheckpointInput};
use super::transactions::{self, TransactionInput, TransactionUpdateInput};
use super::{categories, rules, seed};
use crate::config::DataDir;
use crate::db::{self, Db};
use crate::error::AppError;

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

const TODAY: &str = "2026-04-15";

const SETTINGS: &str = r#"{
  "header_row": 1,
  "date": { "column": "Date", "format": "dd/MM/yyyy" },
  "label": { "columns": ["Libellé"] },
  "amount": { "mode": "single", "column": "Montant" }
}"#;

async fn setup() -> (Db, DataDir, tempfile::TempDir) {
    let (db, dir) = db::test_db().await;
    seed::run(&db).await.unwrap();
    let data = DataDir::new(dir.path().join("data"));
    data.ensure().unwrap();
    (db, data, dir)
}

async fn account(db: &Db, name: &str, kind: AccountKind, opening: i64) -> AccountId {
    let input = AccountInput {
        name: name.into(),
        kind,
        institution_id: None,
        currency: None,
        opening_date: d("2026-01-01"),
        opening_balance_minor: opening,
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

fn upload(account: AccountId, csv: &str) -> Upload {
    Upload {
        file_name: "releve.csv".into(),
        content: csv.as_bytes().to_vec(),
        account_id: account,
        profile_id: None,
        format: None,
        settings: Some(serde_json::from_str(SETTINGS).unwrap()),
        save_profile_as: None,
    }
}

async fn import(db: &Db, data: &DataDir, account: AccountId, csv: &str) -> imports::BatchDetail {
    let created = imports::start(db, data, &upload(account, csv), d(TODAY)).await.unwrap();
    imports::get(db, data, created.batch_id).await.unwrap()
}

const MARCH: &str = "Date;Libellé;Montant\n05/03/2026;PRLV NETFLIX.COM;-13,99\n11/03/2026;CB BOULANGERIE DUPONT 10/03;-4,20\n11/03/2026;CAFE;-2,50\n11/03/2026;CAFE;-2,50\n";

#[tokio::test]
async fn imp_01_reimporting_a_file_creates_no_duplicate() {
    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let first = import(&db, &data, checking, MARCH).await;
    assert_eq!(first.lines.len(), 4);
    assert!(first.lines.iter().all(|l| l.decision == Some(ImportDecision::Create)));
    let result = imports::commit(&db, first.summary.id, &CommitInput::default()).await.unwrap();
    assert_eq!(result.created, 4, "les deux cafés identiques sont deux transactions (R-IMP-5)");

    let second = import(&db, &data, checking, MARCH).await;
    assert!(
        second.lines.iter().all(|l| l.match_kind == MatchKind::Duplicate && l.decision == Some(ImportDecision::Skip))
    );
    let result = imports::commit(&db, second.summary.id, &CommitInput::default()).await.unwrap();
    assert_eq!(result.created, 0);
    let preview = imports::preview(&db, &upload(checking, MARCH), d(TODAY)).await.unwrap();
    assert!(preview.already_imported, "R-IMP-7");
}

#[tokio::test]
async fn imp_05_manual_transaction_is_pointed_and_keeps_its_date() {
    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let manual = TransactionInput {
        account_id: checking,
        date: d("2026-03-10"),
        amount_minor: -420,
        label: "Boulangerie".into(),
        note: None,
        category_id: None,
        splits: None,
    };
    let manual = transactions::create(&db, &manual).await.unwrap();
    let batch = import(&db, &data, checking, MARCH).await;
    let line = batch.lines.iter().find(|l| l.bank_label.as_deref() == Some("CB BOULANGERIE DUPONT 10/03")).unwrap();
    assert_eq!(line.decision, Some(ImportDecision::Match));
    assert_eq!(line.matched.as_ref().map(|m| m.transaction_id), Some(manual));
    imports::commit(&db, batch.summary.id, &CommitInput::default()).await.unwrap();

    let pointed = transactions::get(&db, manual).await.unwrap();
    assert_eq!(pointed.clearing_status, ClearingStatus::Cleared);
    assert_eq!(pointed.date, d("2026-03-10"));
    assert_eq!(pointed.label, "Boulangerie");
    assert_eq!(pointed.bank_date, Some(d("2026-03-11")));
    assert_eq!(pointed.bank_label.as_deref(), Some("CB BOULANGERIE DUPONT 10/03"));
    // Un point de contrôle au soir du 10 ne l'inclut pas : sa date de solde est le 11 (R-TRX-7).
    // Seul le prélèvement Netflix du 5 compte à cette date.
    assert_eq!(accounts::get(&db, checking, d("2026-03-10")).await.unwrap().balance_minor, -1_399);
    assert_eq!(accounts::get(&db, checking, d("2026-03-11")).await.unwrap().balance_minor, -1_399 - 420 - 500);
}

#[tokio::test]
async fn r_imp_12_rules_suggest_categories_and_transfers() {
    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let livret = account(&db, "Livret A", AccountKind::LivretA, 0).await;
    let streaming = categories::create(
        &db,
        &categories::CategoryInput {
            name: "Streaming".into(),
            kind: CategoryKind::Expense,
            parent_id: None,
            archived: false,
            display_order: None,
            color: None,
            icon: None,
        },
    )
    .await
    .unwrap();
    let rule = |name: &str, contains: &str, actions: Actions| rules::RuleInput {
        name: name.into(),
        priority: 0,
        conditions: Conditions {
            label: Some(LabelCondition { op: LabelOp::Contains, value: contains.into(), field: LabelField::BankLabel }),
            ..Default::default()
        },
        actions,
        stop_processing: true,
        active: true,
    };
    rules::save(
        &db,
        None,
        &rule(
            "Netflix",
            "netflix",
            Actions {
                set_label: Some("Netflix".into()),
                set_splits: vec![SplitShare { category_id: Some(streaming), weight: 1 }],
                ..Default::default()
            },
        ),
    )
    .await
    .unwrap();
    rules::save(
        &db,
        None,
        &rule("Épargne", "vir epargne", Actions { mark_transfer_to: Some(livret), ..Default::default() }),
    )
    .await
    .unwrap();

    let csv = "Date;Libellé;Montant\n05/03/2026;PRLV NETFLIX.COM;-13,99\n06/03/2026;VIR EPARGNE;-200,00\n";
    let batch = import(&db, &data, checking, csv).await;
    let netflix = &batch.lines[0];
    assert!(netflix.suggested_by_rule);
    assert_eq!(netflix.splits[0].category_id, Some(streaming));
    assert_eq!(batch.lines[1].transfer_to, Some(livret));

    let result = imports::commit(&db, batch.summary.id, &CommitInput::default()).await.unwrap();
    assert_eq!(result.transfers, 1);
    assert_eq!(accounts::get(&db, livret, d(TODAY)).await.unwrap().balance_minor, 20_000);
    let created = transactions::get(&db, batch_transaction(&db, &data, batch.summary.id, 0).await).await.unwrap();
    assert_eq!(created.label, "Netflix");
    assert_eq!(created.origin, cashflow_domain::ledger::TransactionOrigin::Import);
    assert_eq!(created.clearing_status, ClearingStatus::Cleared);

    // Le même libellé, réimporté plus tard sans règle, est appris (R-IMP-13).
    let rules_list = rules::list(&db).await.unwrap();
    assert_eq!(rules_list.iter().find(|r| r.name == "Netflix").unwrap().hit_count, 1);
}

async fn batch_transaction(
    db: &Db,
    data: &DataDir,
    batch: cashflow_domain::shared::ids::ImportBatchId,
    index: usize,
) -> cashflow_domain::shared::ids::TransactionId {
    imports::get(db, data, batch).await.unwrap().lines[index].transaction_id.unwrap()
}

#[tokio::test]
async fn rap_01_to_04_statement_checkpoint_reconcile_lock_and_unlock() {
    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 100_000).await;
    let batch = import(&db, &data, checking, "Date;Libellé;Montant\n10/01/2026;LOYER;-250,00\n").await;
    imports::commit(&db, batch.summary.id, &CommitInput::default()).await.unwrap();

    // Critère d'acceptation de RAP-01 : 1 000 − 250 = 750 au 31 janvier.
    let wrong = reconciliation::create(
        &db,
        checking,
        &CheckpointInput { date: d("2026-01-31"), balance_minor: 74_000, replace: false },
    )
    .await
    .unwrap();
    assert_eq!(wrong.gap_minor, -1_000);
    assert!(matches!(
        reconciliation::reconcile(&db, wrong.id).await,
        Err(AppError::Domain(DomainError::CheckpointGapNotZero { .. }))
    ));
    let right = reconciliation::create(
        &db,
        checking,
        &CheckpointInput { date: d("2026-01-31"), balance_minor: 75_000, replace: true },
    )
    .await
    .unwrap();
    assert_eq!(right.gap_minor, 0);
    reconciliation::reconcile(&db, right.id).await.unwrap();

    let locked = batch_transaction(&db, &data, batch.summary.id, 0).await;
    assert_eq!(transactions::get(&db, locked).await.unwrap().clearing_status, ClearingStatus::Reconciled);
    let change = TransactionUpdateInput {
        account_id: checking,
        date: d("2026-01-10"),
        amount_minor: -26_000,
        label: "LOYER".into(),
        note: None,
        splits: None,
    };
    assert!(matches!(
        transactions::update(&db, locked, &change).await,
        Err(AppError::Domain(DomainError::TransactionLocked))
    ));
    let late = TransactionInput {
        account_id: checking,
        date: d("2026-01-20"),
        amount_minor: -100,
        label: "Oubli".into(),
        note: None,
        category_id: None,
        splits: None,
    };
    assert!(matches!(
        transactions::create(&db, &late).await,
        Err(AppError::Domain(DomainError::PeriodReconciled { .. }))
    ));

    // Déverrouiller (R-RAP-6) rend la période modifiable.
    reconciliation::unlock(&db, right.id).await.unwrap();
    assert_eq!(transactions::get(&db, locked).await.unwrap().clearing_status, ClearingStatus::Cleared);
    transactions::update(&db, locked, &change).await.unwrap();
}

#[tokio::test]
async fn r_imp_16_closing_balance_creates_a_checkpoint() {
    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let mut up = upload(
        checking,
        "Date;Libellé;Montant\n10/03/2026;SALAIRE;2 000,00\n12/03/2026;LOYER;-800,00\nSolde;;1 200,00\n",
    );
    let mut settings: serde_json::Value = serde_json::from_str(SETTINGS).unwrap();
    settings["skip_footer_rows"] = serde_json::json!(1);
    settings["statement_balances"] = serde_json::json!({ "closing": { "cell": "C4" } });
    up.settings = Some(serde_json::from_value(settings).unwrap());
    let created = imports::start(&db, &data, &up, d(TODAY)).await.unwrap();
    let result = imports::commit(&db, created.batch_id, &CommitInput::default()).await.unwrap();
    assert_eq!(result.checkpoints.len(), 1);
    assert_eq!(result.checkpoints[0].date, d("2026-03-12"));
    assert_eq!(result.checkpoints[0].gap_minor, 0, "écart nul : l'écran propose de rapprocher (R-RAP-11)");
}

#[tokio::test]
async fn r_imp_17_revert_restores_previous_state() {
    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let manual = TransactionInput {
        account_id: checking,
        date: d("2026-03-10"),
        amount_minor: -420,
        label: "Boulangerie".into(),
        note: None,
        category_id: None,
        splits: None,
    };
    let manual = transactions::create(&db, &manual).await.unwrap();
    let batch = import(&db, &data, checking, MARCH).await;
    imports::commit(&db, batch.summary.id, &CommitInput::default()).await.unwrap();
    assert_eq!(transactions::list(&db, checking, &Default::default()).await.unwrap().total, 4);

    imports::revert(&db, batch.summary.id).await.unwrap();
    let after = transactions::list(&db, checking, &Default::default()).await.unwrap();
    assert_eq!(after.total, 1, "les transactions créées sont supprimées");
    let restored = transactions::get(&db, manual).await.unwrap();
    assert_eq!(restored.clearing_status, ClearingStatus::Pending);
    assert_eq!(restored.bank_date, None);
    assert_eq!(restored.bank_label, None);
}

#[tokio::test]
async fn r_imp_1_cancel_review_changes_nothing() {
    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 0).await;
    let batch = import(&db, &data, checking, MARCH).await;
    imports::cancel(&db, &data, batch.summary.id).await.unwrap();
    assert!(imports::list(&db, &data).await.unwrap().is_empty());
    assert_eq!(transactions::list(&db, checking, &Default::default()).await.unwrap().total, 0);
}

/// Une mensualité de prêt importée est proposée en fin d'import, pour être ventilée en un clic (R-IMP-22).
#[tokio::test]
async fn r_imp_22_loan_payment_is_proposed_after_import() {
    use cashflow_domain::loans::{Insurance, LoanKind};

    use super::features::{self, FeaturesInput};
    use super::loans::{self, LoanInput, ScheduleInput};

    let (db, data, _dir) = setup().await;
    let checking = account(&db, "Courant", AccountKind::Checking, 500_000).await;
    let interest = categories::create(
        &db,
        &categories::CategoryInput {
            name: "Intérêts d'emprunt".into(),
            kind: CategoryKind::Expense,
            parent_id: None,
            archived: false,
            display_order: None,
            color: None,
            icon: None,
        },
    )
    .await
    .unwrap();
    features::update(&db, &FeaturesInput { assets: None, loans: Some(true) }).await.unwrap();
    let loan = LoanInput {
        name: "Prêt".into(),
        kind: LoanKind::Mortgage,
        institution_id: None,
        start_date: d("2026-02-05"),
        principal_minor: 20_000_000,
        schedule: Some(ScheduleInput {
            annual_rate_ppm: 30_000,
            duration_months: 240,
            first_installment_date: d("2026-03-05"),
            insurance: Insurance::None,
            payer_account_id: checking,
            interest_category_id: interest,
            insurance_category_id: None,
        }),
        asset_id: None,
        closed_on: None,
        include_in_net_worth: None,
        notes: None,
    };
    loans::create(&db, &loan, d(TODAY)).await.unwrap();
    let batch = import(
        &db,
        &data,
        checking,
        "Date;Libellé;Montant\n05/03/2026;ECHEANCE PRET;-1109,20\n06/03/2026;CAFE;-2,50\n",
    )
    .await;
    let result = imports::commit(&db, batch.summary.id, &CommitInput::default()).await.unwrap();
    assert_eq!(result.loan_payments.len(), 1);
    let proposal = &result.loan_payments[0];
    assert_eq!((proposal.installment_number, proposal.principal_minor, proposal.interest_minor), (1, 60_920, 50_000));
}
