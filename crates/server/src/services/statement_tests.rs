//! Tests d'intégration des relevés de compte archivés (CPT-11, R-IMP-28), sur une vraie base SQLite.

use cashflow_domain::Date;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::shared::ids::AccountId;

use super::accounts::{self, AccountInput};
use super::imports::{self, CommitInput, Upload};
use super::seed;
use super::statements::{self, StatementInput, StatementUpload};
use crate::config::DataDir;
use crate::db::{self, Db};
use crate::error::AppError;

const TODAY: &str = "2026-03-15";

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

fn code(error: AppError) -> String {
    match error {
        AppError::Invalid { code, .. } => code.to_owned(),
        AppError::Domain(e) => e.code().to_owned(),
        other => format!("{other:?}"),
    }
}

async fn setup() -> (Db, DataDir, tempfile::TempDir) {
    let (db, dir) = db::test_db().await;
    seed::run(&db).await.unwrap();
    let data = DataDir::new(dir.path().join("data"));
    data.ensure().unwrap();
    (db, data, dir)
}

async fn checking(db: &Db) -> AccountId {
    let input = AccountInput {
        name: "Courant".into(),
        kind: AccountKind::Checking,
        institution_id: None,
        currency: None,
        opening_date: d("2025-12-02"),
        opening_balance_minor: 125_000,
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

fn pdf(text: &str) -> Vec<u8> {
    format!("%PDF-1.4 {text}").into_bytes()
}

fn upload(month: &str, content: Vec<u8>) -> StatementUpload {
    StatementUpload { file_name: format!("releve-{month}.pdf"), content, statement_month: d(month), note: None }
}

/// Le relevé LCL fictif, du 3 décembre 2025 au 2 janvier 2026.
fn lcl_pdf(account: AccountId) -> Upload {
    Upload {
        file_name: "releve-decembre.pdf".into(),
        content: std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/lcl/releve-fictif.pdf")).unwrap(),
        account_id: account,
        profile_id: None,
        format: None,
        settings: None,
        save_profile_as: None,
    }
}

#[tokio::test]
async fn r_cpt_19_statement_is_archived_once_per_account_and_r_cpt_21_missing_months() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    let today = d(TODAY);

    statements::add(&db, &data, account, upload("2025-12-01", pdf("decembre")), today).await.unwrap();
    let list = statements::add(&db, &data, account, upload("2026-02-01", pdf("fevrier")), today).await.unwrap();
    assert_eq!(list.statements.len(), 2);
    assert_eq!(list.statements[0].statement_month, d("2025-12-01"));
    assert_eq!(list.missing_months, [d("2026-01-01")], "mars, mois en cours, n'est pas encore attendu");

    // Le même fichier, même sous un autre nom et un autre mois, est refusé (R-CPT-19).
    let again = StatementUpload { file_name: "copie.pdf".into(), ..upload("2026-01-01", pdf("fevrier")) };
    let error = statements::add(&db, &data, account, again, today).await.unwrap_err();
    assert_eq!(code(error), "STATEMENT_ALREADY_ARCHIVED");

    // Un PDF, et seulement un PDF (R-CPT-18).
    let jpeg = upload("2026-01-01", vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]);
    assert_eq!(code(statements::add(&db, &data, account, jpeg, today).await.unwrap_err()), "STATEMENT_NOT_PDF");
    let mid_month = upload("2026-01-15", pdf("janvier"));
    assert_eq!(code(statements::add(&db, &data, account, mid_month, today).await.unwrap_err()), "INVALID_STATEMENT");
}

#[tokio::test]
async fn r_cpt_20_statement_month_and_note_change_and_deletion_trashes_the_document() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    let today = d(TODAY);
    let list = statements::add(&db, &data, account, upload("2026-01-01", pdf("janvier")), today).await.unwrap();
    let id = list.statements[0].id;

    let input = StatementInput { statement_month: d("2026-02-01"), note: Some(" rectificatif ".into()) };
    let list = statements::update(&db, account, id, &input, today).await.unwrap();
    assert_eq!(list.statements[0].statement_month, d("2026-02-01"));
    assert_eq!(list.statements[0].note.as_deref(), Some("rectificatif"));
    let (_, content) = statements::file(&db, &data, account, id).await.unwrap();
    assert_eq!(content, pdf("janvier"), "le fichier ne change pas");

    statements::delete(&db, &data, account, id, today).await.unwrap();
    assert!(statements::list(&db, account, today).await.unwrap().statements.is_empty());
    assert_eq!(std::fs::read_dir(data.documents_trash()).unwrap().count(), 1, "document à la corbeille");
}

#[tokio::test]
async fn r_cpt_20_deleting_the_account_deletes_its_statements() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    statements::add(&db, &data, account, upload("2026-01-01", pdf("janvier")), d(TODAY)).await.unwrap();
    accounts::delete(&db, &data, account, d(TODAY)).await.unwrap();
    assert_eq!(std::fs::read_dir(data.documents_trash()).unwrap().count(), 1);
}

#[tokio::test]
async fn r_imp_28_committed_pdf_import_is_archived_and_survives_its_revert() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    let today = d(TODAY);

    let batch = imports::start(&db, &data, &lcl_pdf(account), today).await.unwrap();
    assert!(statements::list(&db, account, today).await.unwrap().statements.is_empty(), "rien avant la validation");
    imports::commit(&db, &data, batch.batch_id, &CommitInput::default(), today).await.unwrap();
    let list = statements::list(&db, account, today).await.unwrap();
    assert_eq!(list.statements.len(), 1);
    let statement = &list.statements[0];
    assert_eq!(statement.statement_month, d("2025-12-01"), "du 3 décembre au 2 janvier : relevé de décembre");
    assert_eq!((statement.period_start, statement.period_end), (Some(d("2025-12-03")), Some(d("2026-01-02"))));
    assert_eq!(statement.file_name, "releve-decembre.pdf");

    // Annuler l'import, ou supprimer son fichier source, laisse le relevé (R-CPT-20).
    imports::revert(&db, batch.batch_id).await.unwrap();
    imports::delete_file(&db, &data, batch.batch_id).await.unwrap();
    assert_eq!(statements::list(&db, account, today).await.unwrap().statements.len(), 1);

    // Réimporté et validé, le même fichier n'est pas archivé une seconde fois (R-CPT-19).
    let again = imports::start(&db, &data, &lcl_pdf(account), today).await.unwrap();
    imports::commit(&db, &data, again.batch_id, &CommitInput::default(), today).await.unwrap();
    assert_eq!(statements::list(&db, account, today).await.unwrap().statements.len(), 1);
}

#[tokio::test]
async fn r_imp_28_csv_import_is_not_a_statement() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    let csv = Upload {
        file_name: "releve.csv".into(),
        content: "Date;Libellé;Montant\n05/03/2026;CB BOULANGERIE;-4,20\n".as_bytes().to_vec(),
        account_id: account,
        profile_id: None,
        format: None,
        settings: Some(
            serde_json::from_value(serde_json::json!({
                "header_row": 1,
                "date": { "column": "Date", "format": "dd/MM/yyyy" },
                "label": { "columns": ["Libellé"] },
                "amount": { "mode": "single", "column": "Montant" }
            }))
            .unwrap(),
        ),
        save_profile_as: None,
    };
    let batch = imports::start(&db, &data, &csv, d(TODAY)).await.unwrap();
    imports::commit(&db, &data, batch.batch_id, &CommitInput::default(), d(TODAY)).await.unwrap();
    assert!(statements::list(&db, account, d(TODAY)).await.unwrap().statements.is_empty());
}

#[tokio::test]
async fn pdf_imports_committed_before_cpt_11_are_taken_over_once() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    let today = d(TODAY);
    let batch = imports::start(&db, &data, &lcl_pdf(account), today).await.unwrap();
    imports::commit(&db, &data, batch.batch_id, &CommitInput::default(), today).await.unwrap();
    // Comme avant CPT-11 : le lot validé n'a pas de relevé.
    let id = statements::list(&db, account, today).await.unwrap().statements[0].id;
    statements::delete(&db, &data, account, id, today).await.unwrap();

    assert_eq!(statements::take_over_imports(&db, &data, today).await.unwrap(), 1);
    let list = statements::list(&db, account, today).await.unwrap();
    assert_eq!(list.statements.len(), 1);
    assert_eq!(list.statements[0].statement_month, d("2025-12-01"));

    // Une seule fois : un relevé supprimé ensuite ne revient pas.
    statements::delete(&db, &data, account, list.statements[0].id, today).await.unwrap();
    assert_eq!(statements::take_over_imports(&db, &data, today).await.unwrap(), 0);
    assert!(statements::list(&db, account, today).await.unwrap().statements.is_empty());
}
