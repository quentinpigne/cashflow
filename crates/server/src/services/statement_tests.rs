//! Tests d'intégration des relevés de compte (CPT-11, R-IMP-28) et de la bibliothèque de documents (spec 12), sur
//! une vraie base SQLite.

use cashflow_domain::Date;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::documents::{DocumentKind, LinkKind, LinkTarget};
use cashflow_domain::shared::ids::AccountId;

use super::accounts::{self, AccountInput};
use super::documents::{self, DocumentFilter, DocumentInput, Upload as DocumentUpload};
use super::imports::{self, CommitInput, Upload};
use super::seed;
use super::statements;
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

fn on(account: AccountId) -> LinkTarget {
    LinkTarget { kind: LinkKind::Account, id: *account.as_uuid() }
}

/// Relevé envoyé depuis la section Relevés du compte (R-CPT-19, R-DOC-15).
fn upload(account: AccountId, month: &str, content: Vec<u8>) -> DocumentUpload {
    DocumentUpload {
        file_name: format!("releve-{month}.pdf"),
        content,
        kind: Some(DocumentKind::BankStatement),
        month: Some(d(month)),
        target: Some(on(account)),
        ..Default::default()
    }
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
async fn r_cpt_19_statement_is_archived_once_and_r_cpt_21_missing_months() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    let today = d(TODAY);

    documents::create(&db, &data, upload(account, "2025-12-01", pdf("decembre")), today).await.unwrap();
    documents::create(&db, &data, upload(account, "2026-02-01", pdf("fevrier")), today).await.unwrap();
    let list = statements::list(&db, account, today).await.unwrap();
    assert_eq!(list.statements.len(), 2);
    assert_eq!(list.statements[0].statement_month, d("2025-12-01"));
    assert_eq!(list.statements[0].title, "Relevé Courant décembre 2025", "R-DOC-15");
    assert_eq!(list.missing_months, [d("2026-01-01")], "mars, mois en cours, n'est pas encore attendu");

    // Le même fichier, même sous un autre nom et un autre mois, est refusé (R-CPT-19, R-DOC-3).
    let again = DocumentUpload { file_name: "copie.pdf".into(), ..upload(account, "2026-01-01", pdf("fevrier")) };
    let error = documents::create(&db, &data, again, today).await.unwrap_err();
    assert_eq!(code(error), "DOCUMENT_ALREADY_ARCHIVED");

    // Un PDF, et seulement un PDF (R-DOC-1) ; un premier du mois (R-DOC-7) ; un compte (R-DOC-10).
    let jpeg = upload(account, "2026-01-01", vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]);
    assert_eq!(code(documents::create(&db, &data, jpeg, today).await.unwrap_err()), "STATEMENT_NOT_PDF");
    let mid_month = upload(account, "2026-01-15", pdf("janvier"));
    assert_eq!(code(documents::create(&db, &data, mid_month, today).await.unwrap_err()), "INVALID_DOCUMENT_MONTH");
    let orphan = DocumentUpload { target: None, ..upload(account, "2026-01-01", pdf("janvier")) };
    assert_eq!(code(documents::create(&db, &data, orphan, today).await.unwrap_err()), "DOCUMENT_OWNER_REQUIRED");
}

#[tokio::test]
async fn r_cpt_20_statement_month_and_note_change_and_deletion_trashes_the_document() {
    let (db, data, _dir) = setup().await;
    let account = checking(&db).await;
    let today = d(TODAY);
    let doc = documents::create(&db, &data, upload(account, "2026-01-01", pdf("janvier")), today).await.unwrap();

    let input = DocumentInput {
        kind: doc.kind,
        title: doc.title.clone(),
        document_date: None,
        period_start: None,
        period_end: None,
        month: Some(d("2026-02-01")),
        note: Some(" rectificatif ".into()),
    };
    documents::update(&db, doc.id, &input, today).await.unwrap();
    let list = statements::list(&db, account, today).await.unwrap();
    assert_eq!(list.statements[0].statement_month, d("2026-02-01"));
    assert_eq!(list.statements[0].note.as_deref(), Some("rectificatif"));
    let (_, content) = documents::file(&db, &data, doc.id).await.unwrap();
    assert_eq!(content, pdf("janvier"), "le fichier ne change pas");

    documents::delete(&db, &data, doc.id, today).await.unwrap();
    assert!(statements::list(&db, account, today).await.unwrap().statements.is_empty());
    assert_eq!(std::fs::read_dir(data.documents_trash()).unwrap().count(), 1, "document à la corbeille");
}

#[tokio::test]
async fn r_cpt_20_r_doc_12_deleting_the_account_keeps_its_statements_unless_asked() {
    let (db, data, _dir) = setup().await;
    let today = d(TODAY);
    let account = checking(&db).await;
    let kept = documents::create(&db, &data, upload(account, "2026-01-01", pdf("janvier")), today).await.unwrap();
    accounts::delete(&db, &data, account, false, today).await.unwrap();
    let orphan = documents::get(&db, kept.id, today).await.unwrap();
    assert!(orphan.links.is_empty(), "le relevé reste, sans compte");
    assert_eq!(orphan.kind, Some(DocumentKind::BankStatement));

    let account = checking(&db).await;
    documents::create(&db, &data, upload(account, "2026-02-01", pdf("fevrier")), today).await.unwrap();
    accounts::delete(&db, &data, account, true, today).await.unwrap();
    let all = documents::list(&db, &DocumentFilter::default(), today).await.unwrap();
    assert_eq!(all.iter().map(|d| d.id).collect::<Vec<_>>(), [kept.id]);
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

// ---------------------------------------------------------------- Bibliothèque de documents (spec 12)

fn text_pdf(lines: &[&str]) -> Vec<u8> {
    super::demo::fictitious_pdf(&lines.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>())
}

fn library(file_name: &str, content: Vec<u8>) -> DocumentUpload {
    DocumentUpload { file_name: file_name.into(), content, ..Default::default() }
}

async fn payee(db: &Db, name: &str) -> cashflow_domain::shared::ids::PayeeId {
    let input =
        super::payees::PayeeInput { name: name.into(), group_id: None, archived: false, note: None, address: None };
    super::payees::create(db, &input).await.unwrap()
}

async fn spend(db: &Db, account: AccountId, label: &str) -> cashflow_domain::shared::ids::TransactionId {
    let input = super::transactions::TransactionInput {
        account_id: account,
        date: d("2026-03-10"),
        amount_minor: -4_520,
        label: label.into(),
        note: None,
        category_id: None,
        splits: None,
        payee_id: None,
    };
    super::transactions::create(db, &input).await.unwrap()
}

fn target<T>(kind: LinkKind, id: T) -> LinkTarget
where
    T: Into<uuid::Uuid>,
{
    LinkTarget { kind, id: id.into() }
}

#[tokio::test]
async fn r_doc_3_r_doc_4_r_doc_14_library_upload_is_unsorted_and_unique() {
    let (db, data, _dir) = setup().await;
    let today = d(TODAY);
    let doc = documents::create(&db, &data, library("Contrat MAIF.pdf", pdf("maif")), today).await.unwrap();
    assert_eq!((doc.kind, doc.title.as_str(), doc.links.len()), (None, "Contrat MAIF", 0), "à classer");
    assert_eq!(doc.reference_date, doc.sent_on, "date d'envoi à défaut d'une autre (R-DOC-8)");

    let error = documents::create(&db, &data, library("copie.pdf", pdf("maif")), today).await.unwrap_err();
    match error {
        AppError::Domain(cashflow_domain::DomainError::DocumentAlreadyArchived { id, title }) => {
            assert_eq!((id, title.as_str()), (doc.id, "Contrat MAIF"));
        }
        other => panic!("{other:?}"),
    }
    let word = library("lettre.pdf", b"PK\x03\x04word/document.xml".to_vec());
    assert_eq!(code(documents::create(&db, &data, word, today).await.unwrap_err()), "UNSUPPORTED_DOCUMENT_TYPE");
}

#[tokio::test]
async fn r_doc_9_r_doc_11_r_doc_12_r_doc_13_links() {
    let (db, data, _dir) = setup().await;
    let today = d(TODAY);
    let account = checking(&db).await;
    let garage = payee(&db, "Garage Dupont").await;
    let other = payee(&db, "Garage Durand").await;
    let purchase = spend(&db, account, "CB GARAGE DUPONT").await;
    let invoice = documents::create(&db, &data, library("facture.pdf", pdf("facture")), today).await.unwrap();

    // Plusieurs rattachements, une seule fois chacun (R-DOC-9).
    documents::link(&db, invoice.id, target(LinkKind::Payee, *garage.as_uuid()), today).await.unwrap();
    documents::link(&db, invoice.id, target(LinkKind::Payee, *garage.as_uuid()), today).await.unwrap();
    // Une transaction rapprochée accepte un justificatif sans déverrouillage (R-DOC-11).
    sqlx::query("UPDATE ledger_transaction SET clearing_status = 'RECONCILED' WHERE id = ?")
        .bind(purchase.to_string())
        .execute(&db.write)
        .await
        .unwrap();
    let view =
        documents::link(&db, invoice.id, target(LinkKind::Transaction, *purchase.as_uuid()), today).await.unwrap();
    let names: Vec<&str> = view.links.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Garage Dupont", "CB GARAGE DUPONT"]);
    assert_eq!(view.links[1].amount_minor, Some(-4_520));

    // Un objet qui n'existe pas ne se rattache pas.
    let missing = documents::link(&db, invoice.id, target(LinkKind::Asset, uuid::Uuid::now_v7()), today).await;
    assert!(matches!(missing, Err(AppError::NotFound(_))));

    // Fusionner le tiers reporte ses rattachements (R-DOC-13).
    let merge = super::payees::MergePayeeInput { target_id: other };
    super::payees::merge(&db, garage, &merge).await.unwrap();
    let view = documents::get(&db, invoice.id, today).await.unwrap();
    assert!(view.links.iter().any(|l| l.name == "Garage Durand"));

    // Supprimer un objet : le document reste ; avec la case, seuls partent ceux rattachés seulement à lui (R-DOC-12).
    let ticket =
        documents::create(&db, &data, library("ticket.png", b"\x89PNG\r\n\x1a\nticket".to_vec()), today).await.unwrap();
    documents::link(&db, ticket.id, target(LinkKind::Payee, *other.as_uuid()), today).await.unwrap();
    let attached = documents::attached(&db, *other.as_uuid()).await.unwrap();
    assert_eq!((attached.total, attached.exclusive), (2, 1));
    super::payees::delete(&db, &data, other, true, today).await.unwrap();
    assert!(matches!(documents::get(&db, ticket.id, today).await, Err(AppError::NotFound(_))));
    let view = documents::get(&db, invoice.id, today).await.unwrap();
    assert_eq!(view.links.len(), 1, "la facture reste, rattachée à la transaction");

    // Détacher (R-DOC-9) ; détacher ce qui ne l'est pas est une erreur.
    let view = documents::unlink(&db, invoice.id, target(view.links[0].kind, view.links[0].id), today).await.unwrap();
    assert!(view.links.is_empty());
    let again = documents::unlink(&db, invoice.id, target(LinkKind::Transaction, *purchase.as_uuid()), today).await;
    assert!(matches!(again, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn r_doc_10_statement_and_payslip_owners() {
    let (db, data, _dir) = setup().await;
    let today = d(TODAY);
    let account = checking(&db).await;
    let other = checking(&db).await;
    let statement = documents::create(&db, &data, upload(account, "2026-01-01", pdf("janvier")), today).await.unwrap();
    let second = documents::link(&db, statement.id, on(other), today).await.unwrap_err();
    assert_eq!(code(second), "DOCUMENT_OWNER_NOT_UNIQUE");

    // Classer un document en fiche de paie demande un emploi.
    let doc = documents::create(&db, &data, library("paie.pdf", pdf("paie")), today).await.unwrap();
    let input = DocumentInput {
        kind: Some(DocumentKind::Payslip),
        title: "Fiche de paie".into(),
        document_date: None,
        period_start: None,
        period_end: None,
        month: Some(d("2026-01-01")),
        note: None,
    };
    assert_eq!(code(documents::update(&db, doc.id, &input, today).await.unwrap_err()), "DOCUMENT_OWNER_REQUIRED");
    let no_month = DocumentInput { month: None, kind: Some(DocumentKind::Contract), ..input };
    let contract = documents::update(&db, doc.id, &no_month, today).await.unwrap();
    assert_eq!(contract.kind, Some(DocumentKind::Contract));
}

#[tokio::test]
async fn r_doc_18_r_doc_19_r_doc_20_search_and_filters() {
    let (db, data, _dir) = setup().await;
    let today = d(TODAY);
    let account = checking(&db).await;
    let maif = text_pdf(&["CONTRAT D'ASSURANCE HABITATION", "Mutuelle d'assurance des instituteurs", "Resiliation"]);
    let contract = documents::create(&db, &data, library("scan.pdf", maif), today).await.unwrap();
    assert!(contract.content_indexed, "texte extrait du PDF");
    let letter =
        documents::create(&db, &data, library("Courrier Crédit Agricole.png", b"\x89PNG\r\n\x1a\nx".to_vec()), today)
            .await
            .unwrap();
    assert!(!letter.content_indexed, "une image n'a pas de contenu indexé");
    let statement = documents::create(&db, &data, upload(account, "2025-12-01", pdf("decembre")), today).await.unwrap();

    let search = |q: &str| DocumentFilter { q: Some(q.into()), ..Default::default() };
    let ids = |list: Vec<documents::DocumentView>| list.into_iter().map(|d| d.id).collect::<Vec<_>>();

    // Dans le contenu, sans casse ni accents, par début de mot ; l'extrait marque le mot trouvé.
    let found = documents::list(&db, &search("RÉSILI"), today).await.unwrap();
    assert_eq!(ids(found.clone()), [contract.id]);
    assert!(found[0].snippet.as_deref().unwrap().contains("\u{2}Resiliation\u{3}"));
    // Dans le titre, et dans le nom d'un objet rattaché ; tous les mots doivent être trouvés.
    assert_eq!(ids(documents::list(&db, &search("credit agri"), today).await.unwrap()), [letter.id]);
    assert_eq!(ids(documents::list(&db, &search("courant releve"), today).await.unwrap()), [statement.id]);
    assert!(documents::list(&db, &search("courant maif"), today).await.unwrap().is_empty());

    // Filtres (R-DOC-20) ; sans texte, par date de référence décroissante.
    let all = documents::list(&db, &DocumentFilter::default(), today).await.unwrap();
    assert_eq!(ids(all).last(), Some(&statement.id), "décembre 2025, le plus ancien");
    let unsorted = DocumentFilter { unsorted: true, ..Default::default() };
    assert_eq!(documents::list(&db, &unsorted, today).await.unwrap().len(), 2);
    let statements = DocumentFilter { kind: Some(DocumentKind::BankStatement), year: Some(2025), ..Default::default() };
    assert_eq!(ids(documents::list(&db, &statements, today).await.unwrap()), [statement.id]);
    let by_account = DocumentFilter { link_id: Some(*account.as_uuid()), ..Default::default() };
    assert_eq!(ids(documents::list(&db, &by_account, today).await.unwrap()), [statement.id]);
    let unlinked = DocumentFilter { unlinked: true, link_kind: Some(LinkKind::Account), ..Default::default() };
    assert!(documents::list(&db, &unlinked, today).await.unwrap().is_empty());

    // L'index se reconstruit à partir des fichiers (R-DOC-19).
    sqlx::query("DELETE FROM document_search").execute(&db.write).await.unwrap();
    assert!(documents::list(&db, &search("resiliation"), today).await.unwrap().is_empty());
    assert_eq!(documents::reindex(&db, &data, false).await.unwrap(), 3);
    assert_eq!(ids(documents::list(&db, &search("resiliation"), today).await.unwrap()), [contract.id]);

    // Le titre modifié est cherchable.
    let input = DocumentInput {
        kind: Some(DocumentKind::Letter),
        title: "Hausse des tarifs".into(),
        document_date: Some(d("2024-05-12")),
        period_start: None,
        period_end: None,
        month: None,
        note: None,
    };
    documents::update(&db, letter.id, &input, today).await.unwrap();
    assert_eq!(ids(documents::list(&db, &search("tarifs"), today).await.unwrap()), [letter.id]);
    let in_2024 = DocumentFilter { year: Some(2024), ..Default::default() };
    assert_eq!(ids(documents::list(&db, &in_2024, today).await.unwrap()), [letter.id], "R-DOC-8");
}

#[tokio::test]
async fn r_doc_12_deleting_a_transaction_keeps_its_receipt_unless_asked() {
    let (db, data, _dir) = setup().await;
    let today = d(TODAY);
    let account = checking(&db).await;
    let first = spend(&db, account, "CB CARREFOUR").await;
    let receipt = DocumentUpload {
        kind: Some(DocumentKind::Receipt),
        target: Some(target(LinkKind::Transaction, *first.as_uuid())),
        ..library("ticket.pdf", pdf("ticket"))
    };
    let kept = documents::create(&db, &data, receipt, today).await.unwrap();
    super::transactions::delete(&db, &data, first, false, today).await.unwrap();
    assert!(documents::get(&db, kept.id, today).await.unwrap().links.is_empty());

    let second = spend(&db, account, "CB CARREFOUR").await;
    documents::link(&db, kept.id, target(LinkKind::Transaction, *second.as_uuid()), today).await.unwrap();
    super::transactions::delete(&db, &data, second, true, today).await.unwrap();
    assert!(matches!(documents::get(&db, kept.id, today).await, Err(AppError::NotFound(_))));
}
