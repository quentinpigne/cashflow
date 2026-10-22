//! Import de relevés (spec 03 : IMP-01 à IMP-07, IMP-09, IMP-11).
//!
//! Lecture → normalisation → correspondances → suggestions → revue → validation atomique. Rien n'est
//! écrit dans les transactions avant la validation (R-IMP-1).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use cashflow_domain::import::fingerprint::{self, LineKey, normalize_label};
use cashflow_domain::import::matcher::{self, AUTO_MATCH_SCORE, Candidate, LineToMatch};
use cashflow_domain::import::rules::{Subject, evaluate};
use cashflow_domain::import::{self as domain_import, BatchStatus, ImportDecision, MatchKind};
use cashflow_domain::ledger::split;
use cashflow_domain::ledger::transfer::{self, TransferLeg};
use cashflow_domain::ledger::{ClearingStatus, TransactionOrigin};
use cashflow_domain::shared::ids::{
    AccountId, CategorizationRuleId, CategoryId, ImportBatchId, ImportLineId, ImportProfileId, ProjectId, TransactionId,
};
use cashflow_domain::{Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::reconciliation::{self, Checkpoint};
use super::transactions::period;
use super::transfers::DEFAULT_TOLERANCE_DAYS;
use super::{check_split_sums, clean, required, rules};
use crate::config::DataDir;
use crate::db::imports::{BatchRow, LineRow, ProfileRow};
use crate::db::transactions::{NewSplit, NewTransaction};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::readers::profile::{self, ProfileSettings};
use crate::readers::{self, FileFormat, ReadContext, ReadLine, Statement, StatementReader, Table};

const PREVIEW_ROWS: usize = 30;
const PREVIEW_LINES: usize = 50;

// ---------------------------------------------------------------- Types exposés

#[derive(Debug, Serialize, ToSchema)]
pub struct Profile {
    pub id: ImportProfileId,
    pub name: String,
    pub format: FileFormat,
    /// Réglages CSV/XLSX ; absents pour l'OFX, qui n'a pas besoin de profil.
    pub settings: Option<ProfileSettings>,
    pub default_account_id: Option<AccountId>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProfileInput {
    pub name: String,
    pub format: FileFormat,
    pub settings: Option<ProfileSettings>,
    pub default_account_id: Option<AccountId>,
}

/// Fichier envoyé et choix associés (formulaire `multipart/form-data` côté API).
pub struct Upload {
    pub file_name: String,
    pub content: Vec<u8>,
    pub account_id: AccountId,
    pub profile_id: Option<ImportProfileId>,
    pub format: Option<FileFormat>,
    pub settings: Option<ProfileSettings>,
    /// Enregistre les réglages comme nouveau profil sous ce nom (IMP-02).
    pub save_profile_as: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatementInfo {
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub opening_minor: Option<i64>,
    pub closing_minor: Option<i64>,
    /// `fin − début − Σ lignes valides` (R-IMP-18) ; non nul : une ligne mal lue ou manquante.
    pub gap_minor: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PreviewLine {
    pub line_number: i64,
    pub date: Option<Date>,
    pub amount_minor: Option<i64>,
    pub bank_label: String,
    pub errors: Vec<String>,
}

/// Aperçu en direct de l'assistant (IMP-01).
#[derive(Debug, Serialize, ToSchema)]
pub struct ImportPreview {
    pub format: FileFormat,
    pub encoding: Option<String>,
    pub delimiter: Option<String>,
    pub sheets: Vec<String>,
    /// Premières lignes brutes du fichier (CSV/XLSX).
    pub raw_rows: Vec<Vec<String>>,
    pub lines: Vec<PreviewLine>,
    pub total_lines: usize,
    pub error_lines: usize,
    pub statement: StatementInfo,
    /// Un lot validé avec ce même fichier existe dans le compte (R-IMP-7).
    pub already_imported: bool,
    /// Réglages incomplets ou invalides : l'aperçu montre seulement les lignes brutes.
    pub settings_error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BatchCreated {
    pub batch_id: ImportBatchId,
    pub profile_id: Option<ImportProfileId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct DraftSplit {
    pub category_id: Option<CategoryId>,
    pub amount_minor: i64,
    #[serde(default)]
    pub note: Option<String>,
}

/// Virement repéré automatiquement, à accepter (R-IMP-14).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransferProposal {
    pub account_id: AccountId,
    /// Transaction existante à lier.
    pub transaction_id: Option<TransactionId>,
    /// Ligne d'un autre lot en revue.
    pub line_id: Option<ImportLineId>,
    pub date: Date,
    pub label: String,
}

/// État de revue d'une ligne, stocké dans `ImportLine.suggested`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Review {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    splits: Option<Vec<DraftSplit>>,
    #[serde(default)]
    note: Option<String>,
    /// Virement accepté : vers (ou depuis) ce compte (R-IMP-14, R-TRX-36).
    #[serde(default)]
    transfer_to: Option<AccountId>,
    /// Transaction existante à lier comme autre jambe.
    #[serde(default)]
    transfer_transaction_id: Option<TransactionId>,
    #[serde(default)]
    proposal: Option<TransferProposal>,
    #[serde(default)]
    rules: Vec<CategorizationRuleId>,
    /// Projet proposé par une règle (`set_project`, BUD-06).
    #[serde(default)]
    project: Option<ProjectId>,
    #[serde(default)]
    learned: bool,
    /// État de la transaction pointée avant validation, pour l'annulation (R-IMP-17).
    #[serde(default)]
    previous: Option<Value>,
    /// Jambe opposée créée à la validation (R-TRX-36), supprimée à l'annulation.
    #[serde(default)]
    created_counterpart_id: Option<TransactionId>,
}

impl Review {
    fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MatchView {
    pub transaction_id: TransactionId,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub clearing_status: ClearingStatus,
    pub score: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportLineView {
    pub id: ImportLineId,
    pub line_number: i64,
    pub date: Option<Date>,
    pub value_date: Option<Date>,
    pub amount_minor: Option<i64>,
    pub bank_label: Option<String>,
    pub raw: Value,
    /// Non vide : ligne invalide, exclue de la validation (R-IMP-3).
    pub errors: Vec<String>,
    pub match_kind: MatchKind,
    pub matched: Option<MatchView>,
    pub decision: Option<ImportDecision>,
    /// Libellé de la transaction créée (par défaut le libellé bancaire).
    pub label: Option<String>,
    pub splits: Vec<DraftSplit>,
    pub note: Option<String>,
    pub transfer_to: Option<AccountId>,
    pub transfer_transaction_id: Option<TransactionId>,
    pub proposal: Option<TransferProposal>,
    /// Suggestion issue d'au moins une règle (R-IMP-12) ou de l'apprentissage (R-IMP-13).
    pub suggested_by_rule: bool,
    pub learned: bool,
    pub transaction_id: Option<TransactionId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BatchSummary {
    pub id: ImportBatchId,
    pub account_id: AccountId,
    pub account_name: String,
    pub profile_id: Option<ImportProfileId>,
    pub file_name: String,
    pub status: BatchStatus,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub created_at: String,
    pub committed_at: Option<String>,
    pub line_count: usize,
    pub error_count: usize,
    pub to_create: usize,
    pub to_match: usize,
    pub to_skip: usize,
    /// Le fichier source est encore conservé (R-IMP-21).
    pub has_file: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BatchDetail {
    pub summary: BatchSummary,
    pub currency: String,
    pub statement: StatementInfo,
    pub lines: Vec<ImportLineView>,
}

/// État complet de revue d'une ligne, envoyé par l'écran de revue (IMP-04).
#[derive(Debug, Deserialize, ToSchema)]
pub struct LineReviewInput {
    pub decision: ImportDecision,
    /// Transaction pointée par un `MATCH`.
    pub match_transaction_id: Option<TransactionId>,
    pub label: Option<String>,
    /// Ventilation de la transaction créée ; sans ventilation, une seule non catégorisée.
    #[serde(default)]
    pub splits: Vec<DraftSplit>,
    pub note: Option<String>,
    pub transfer_to: Option<AccountId>,
    pub transfer_transaction_id: Option<TransactionId>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkInput {
    pub line_ids: Vec<ImportLineId>,
    pub decision: Option<ImportDecision>,
    /// Catégorie posée sur les lignes `CREATE` non ventilées.
    pub category_id: Option<CategoryId>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct CommitInput {
    /// Remplace les points de contrôle existants aux mêmes dates (R-RAP-2).
    #[serde(default)]
    pub replace_checkpoints: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CommitResult {
    pub created: usize,
    pub matched: usize,
    pub skipped: usize,
    pub transfers: usize,
    /// Points de contrôle créés ou mis à jour par le relevé (R-IMP-16) ; à écart nul, l'écran propose
    /// « Rapprocher » en un clic (R-RAP-11).
    pub checkpoints: Vec<Checkpoint>,
}

// ---------------------------------------------------------------- Profils

fn profile_view(row: ProfileRow) -> Profile {
    Profile {
        id: row.id,
        name: row.name,
        format: row.format,
        settings: serde_json::from_value(row.settings).ok(),
        default_account_id: row.default_account_id,
    }
}

pub async fn list_profiles(db: &Db) -> AppResult<Vec<Profile>> {
    Ok(db::imports::list_profiles(&db.read).await?.into_iter().map(profile_view).collect())
}

pub async fn save_profile(db: &Db, id: Option<ImportProfileId>, input: &ProfileInput) -> AppResult<Profile> {
    let name = required(&input.name, "name", 80)?;
    if db::imports::profile_name_taken(&db.read, &name, id).await? {
        return Err(AppError::invalid_field("NAME_TAKEN", "name", "un profil porte déjà ce nom"));
    }
    if matches!(input.format, FileFormat::Csv | FileFormat::Xlsx) && input.settings.is_none() {
        return Err(AppError::invalid_field(
            "SETTINGS_REQUIRED",
            "settings",
            "un profil CSV ou XLSX a besoin de réglages",
        ));
    }
    let row = ProfileRow {
        id: id.unwrap_or_else(ImportProfileId::new),
        name,
        format: input.format,
        settings: input
            .settings
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(anyhow::Error::from)?
            .unwrap_or(Value::Object(Default::default())),
        default_account_id: input.default_account_id,
    };
    if let Some(id) = id
        && db::imports::find_profile(&db.read, id).await?.is_none()
    {
        return Err(AppError::NotFound("profil"));
    }
    db::imports::upsert_profile(&db.write, &row).await?;
    Ok(profile_view(row))
}

pub async fn delete_profile(db: &Db, id: ImportProfileId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::imports::delete_profile(&mut *tx, id).await?;
    db::imports::delete_profile_row(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------- Lecture

/// Tableau d'un fichier CSV/XLSX et ce qui a été détecté.
struct Read {
    format: FileFormat,
    encoding: Option<String>,
    delimiter: Option<String>,
    sheets: Vec<String>,
    table: Option<Table>,
}

fn read_table(upload: &Upload, settings: Option<&ProfileSettings>) -> AppResult<Read> {
    let format = upload
        .format
        .or_else(|| FileFormat::detect(&upload.file_name, &upload.content))
        .ok_or_else(|| AppError::invalid("UNKNOWN_FORMAT", "format de fichier non reconnu"))?;
    let unreadable = |e: readers::ReadError| AppError::invalid("UNREADABLE_FILE", e.to_string());
    match format {
        FileFormat::Csv => {
            let encoding = settings
                .and_then(|s| s.encoding.clone())
                .unwrap_or_else(|| readers::csv::detect_encoding(&upload.content).to_owned());
            let text = readers::csv::decode(&upload.content, &encoding).map_err(unreadable)?;
            let delimiter = settings
                .and_then(|s| s.delimiter.as_deref())
                .and_then(|d| if d == "\\t" { Some('\t') } else { d.chars().next() })
                .unwrap_or_else(|| readers::csv::detect_delimiter(&text));
            let quote = settings.and_then(|s| s.quote.chars().next()).unwrap_or('"');
            let table = readers::csv::table(&text, delimiter, quote).map_err(unreadable)?;
            let shown = if delimiter == '\t' { "\\t".to_owned() } else { delimiter.to_string() };
            Ok(Read {
                format,
                encoding: Some(encoding),
                delimiter: Some(shown),
                sheets: Vec::new(),
                table: Some(table),
            })
        }
        FileFormat::Xlsx => {
            let sheets = readers::xlsx::sheet_names(&upload.content).map_err(unreadable)?;
            let table =
                readers::xlsx::table(&upload.content, settings.and_then(|s| s.sheet.as_deref())).map_err(unreadable)?;
            Ok(Read { format, encoding: None, delimiter: None, sheets, table: Some(table) })
        }
        FileFormat::Ofx => Ok(Read { format, encoding: None, delimiter: None, sheets: Vec::new(), table: None }),
        FileFormat::PdfOcr => Err(AppError::invalid("FORMAT_UNAVAILABLE", "l'import PDF n'est pas encore disponible")),
    }
}

fn read_statement(
    upload: &Upload,
    read: &Read,
    settings: Option<&ProfileSettings>,
    context: ReadContext,
) -> AppResult<Statement> {
    let unreadable = |e: readers::ReadError| AppError::invalid("UNREADABLE_FILE", e.to_string());
    match (read.format, &read.table) {
        (FileFormat::Ofx, _) => readers::ofx::OfxReader.read(&upload.content, context).map_err(unreadable),
        (_, Some(table)) => {
            let settings = settings
                .ok_or_else(|| AppError::invalid("SETTINGS_REQUIRED", "choisir un profil ou associer les colonnes"))?;
            profile::apply(table, settings, context).map_err(|e| AppError::invalid("INVALID_SETTINGS", e.to_string()))
        }
        _ => Err(AppError::invalid("UNREADABLE_FILE", "fichier vide")),
    }
}

async fn resolve_settings(db: &Db, upload: &Upload) -> AppResult<Option<ProfileSettings>> {
    if let Some(settings) = &upload.settings {
        return Ok(Some(settings.clone()));
    }
    match upload.profile_id {
        Some(id) => {
            let row = db::imports::find_profile(&db.read, id).await?.ok_or(AppError::NotFound("profil"))?;
            Ok(serde_json::from_value(row.settings).ok())
        }
        None => Ok(None),
    }
}

fn statement_info(statement: &Statement) -> StatementInfo {
    let valid: Vec<Money> = statement.lines.iter().filter(|l| l.is_valid()).filter_map(|l| l.amount).collect();
    let gap = statement
        .opening_balance
        .zip(statement.closing_balance)
        .map(|(o, c)| domain_import::statement_gap(o, c, &valid).minor());
    StatementInfo {
        period_start: statement.period.map(|p| p.0),
        period_end: statement.period.map(|p| p.1),
        opening_minor: statement.opening_balance.map(Money::minor),
        closing_minor: statement.closing_balance.map(Money::minor),
        gap_minor: gap,
    }
}

/// Aperçu en direct (IMP-01) : lignes brutes, lignes normalisées et erreurs, sans rien enregistrer.
pub async fn preview(db: &Db, upload: &Upload, today: Date) -> AppResult<ImportPreview> {
    let account = db::accounts::find(&db.read, upload.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    let settings = resolve_settings(db, upload).await?;
    let read = read_table(upload, settings.as_ref())?;
    let context = ReadContext { today, currency: account.currency };
    let raw_rows = read
        .table
        .as_ref()
        .map(|t| t.iter().take(PREVIEW_ROWS).map(|r| r.iter().map(readers::Cell::text).collect()).collect())
        .unwrap_or_default();
    let (statement, settings_error) = match read_statement(upload, &read, settings.as_ref(), context) {
        Ok(statement) => (statement, None),
        Err(error) => (Statement::default(), Some(error.to_string())),
    };
    let sha = fingerprint::file_sha256(&upload.content);
    Ok(ImportPreview {
        format: read.format,
        encoding: read.encoding.clone(),
        delimiter: read.delimiter.clone(),
        sheets: read.sheets.clone(),
        raw_rows,
        lines: statement
            .lines
            .iter()
            .take(PREVIEW_LINES)
            .map(|l| PreviewLine {
                line_number: l.line_number,
                date: l.date,
                amount_minor: l.amount.map(Money::minor),
                bank_label: l.bank_label.clone(),
                errors: l.errors.clone(),
            })
            .collect(),
        total_lines: statement.lines.len(),
        error_lines: statement.lines.iter().filter(|l| !l.is_valid()).count(),
        statement: statement_info(&statement),
        already_imported: db::imports::committed_with_sha(&db.read, account.id, &sha).await?,
        settings_error,
    })
}

// ---------------------------------------------------------------- Création du lot

fn sanitized(file_name: &str) -> String {
    let name: String = file_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    name.chars().rev().take(80).collect::<Vec<_>>().into_iter().rev().collect()
}

/// Fichier source conservé pour un lot (R-IMP-21).
fn stored_file(data: &DataDir, batch: ImportBatchId, file_name: &str) -> PathBuf {
    data.imports().join(format!("{batch}-{}", sanitized(file_name)))
}

fn find_stored_file(data: &DataDir, batch: ImportBatchId) -> Option<PathBuf> {
    let prefix = batch.to_string();
    std::fs::read_dir(data.imports())
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix)))
}

/// Suggestions de catégorisation : règles, puis apprentissage, puis catégorie de la banque.
struct Suggester {
    rules: Vec<cashflow_domain::import::rules::Rule<CategorizationRuleId>>,
    learned: HashMap<String, CategoryId>,
    categories_by_name: HashMap<String, CategoryId>,
}

impl Suggester {
    async fn load(db: &Db, account: AccountId) -> AppResult<Self> {
        let rules = rules::active(db).await?;
        let mut learned = HashMap::new();
        for (label, category) in db::imports::labelled_categories(&db.read, account).await? {
            if let Ok(category) = category.parse() {
                learned.entry(normalize_label(&label)).or_insert(category);
            }
        }
        let categories_by_name = db::categories::list(&db.read)
            .await?
            .into_iter()
            .filter(|c| !c.archived)
            .map(|c| (normalize_label(&c.name), c.id))
            .collect();
        Ok(Self { rules, learned, categories_by_name })
    }

    fn suggest(&self, account: AccountId, line: &ReadLine, amount: Money) -> Review {
        let subject = Subject { account, bank_label: &line.bank_label, label: &line.bank_label, amount };
        let suggestion = evaluate(&self.rules, &subject);
        let mut review = Review {
            label: suggestion.label.clone(),
            note: suggestion.note.clone(),
            rules: suggestion.rules.clone(),
            project: suggestion.project,
            ..Default::default()
        };
        if let Some(to) = suggestion.transfer_to {
            review.transfer_to = Some(to);
            return review;
        }
        if !suggestion.splits.is_empty() {
            let weights: Vec<u64> = suggestion.splits.iter().map(|s| s.weight).collect();
            if let Ok(amounts) = split::allocate(amount, &weights) {
                review.splits = Some(
                    suggestion
                        .splits
                        .iter()
                        .zip(amounts)
                        .map(|(s, a)| DraftSplit { category_id: s.category_id, amount_minor: a.minor(), note: None })
                        .collect(),
                );
            }
        } else if let Some(category) = self.learned.get(&normalize_label(&line.bank_label)) {
            review.splits =
                Some(vec![DraftSplit { category_id: Some(*category), amount_minor: amount.minor(), note: None }]);
            review.learned = true;
        } else if let Some(category) =
            line.bank_category.as_deref().and_then(|c| self.categories_by_name.get(&normalize_label(c)))
        {
            review.splits =
                Some(vec![DraftSplit { category_id: Some(*category), amount_minor: amount.minor(), note: None }]);
        }
        review
    }
}

async fn tolerance(db: &Db) -> AppResult<i64> {
    Ok(db::settings::get(&db.read, "matching.date_tolerance_days").await?.unwrap_or(DEFAULT_TOLERANCE_DAYS))
}

/// Propose un virement pour une ligne : transaction d'un autre compte, ou ligne d'un autre lot en revue,
/// de montant opposé et de date proche (R-IMP-14, R-TRX-35).
async fn transfer_proposal(
    db: &Db,
    account: AccountId,
    amount: Money,
    date: Date,
    tolerance: i64,
    elsewhere: &[(AccountId, LineRow)],
) -> AppResult<Option<TransferProposal>> {
    let from = date - chrono::Days::new(tolerance.unsigned_abs());
    let to = date + chrono::Days::new(tolerance.unsigned_abs());
    let accounts = db::accounts::list(&db.read).await?;
    for other in accounts.iter().filter(|a| a.id != account && a.closed_on.is_none()) {
        let candidates = db::transactions::transfer_candidates(&db.read, other.id, amount, from, to).await?;
        if let Some(c) = candidates.into_iter().min_by_key(|c| (c.balance_date - date).num_days().abs()) {
            return Ok(Some(TransferProposal {
                account_id: other.id,
                transaction_id: Some(c.id),
                line_id: None,
                date: c.balance_date,
                label: c.label,
            }));
        }
    }
    let found = elsewhere
        .iter()
        .filter(|(other, l)| {
            *other != account
                && l.amount == Some(-amount)
                && l.date.is_some_and(|d| (d - date).num_days().abs() <= tolerance)
        })
        .min_by_key(|(_, l)| l.date.map_or(i64::MAX, |d| (d - date).num_days().abs()));
    Ok(found.map(|(other, l)| TransferProposal {
        account_id: *other,
        transaction_id: None,
        line_id: Some(l.id),
        date: l.date.unwrap_or(date),
        label: l.bank_label.clone().unwrap_or_default(),
    }))
}

/// Lit le fichier et crée un lot en revue (IMP-01 à IMP-05). Aucune transaction n'est modifiée.
pub async fn start(db: &Db, data: &DataDir, upload: &Upload, today: Date) -> AppResult<BatchCreated> {
    let account = db::accounts::find(&db.read, upload.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    if account.closed_on.is_some() {
        return Err(DomainError::AccountClosed.into());
    }
    let settings = resolve_settings(db, upload).await?;
    let read = read_table(upload, settings.as_ref())?;
    let context = ReadContext { today, currency: account.currency };
    let statement = read_statement(upload, &read, settings.as_ref(), context)?;
    if statement.lines.is_empty() {
        return Err(AppError::invalid("EMPTY_STATEMENT", "aucune ligne lue dans ce fichier"));
    }

    // Profil enregistré à partir des réglages de l'assistant (IMP-02).
    let mut profile_id = upload.profile_id;
    if let Some(name) = clean(upload.save_profile_as.as_deref()) {
        let saved = save_profile(
            db,
            None,
            &ProfileInput {
                name,
                format: read.format,
                settings: settings.clone(),
                default_account_id: Some(account.id),
            },
        )
        .await?;
        profile_id = Some(saved.id);
    }

    // Empreintes (R-IMP-5) : FITID pour l'OFX, sinon date | montant | libellé | rang d'occurrence.
    let keys: Vec<LineKey> = statement
        .lines
        .iter()
        .filter(|l| l.is_valid())
        .map(|l| LineKey {
            date: l.date.unwrap_or_default(),
            amount: l.amount.unwrap_or_default(),
            bank_label: l.bank_label.clone(),
        })
        .collect();
    let mut prints = fingerprint::fingerprints(account.id, &keys).into_iter();
    let line_prints: Vec<Option<String>> = statement
        .lines
        .iter()
        .map(|l| {
            if !l.is_valid() {
                return None;
            }
            let print = prints.next();
            Some(match &l.fitid {
                Some(fitid) => fingerprint::ofx_fingerprint(account.id, fitid),
                None => print.unwrap_or_default(),
            })
        })
        .collect();

    // Doublons (R-IMP-6), puis candidats parmi les transactions pointables (R-IMP-9).
    let existing: HashMap<String, TransactionId> =
        db::imports::fingerprints_of(&db.read, account.id).await?.into_iter().collect();
    let tolerance = tolerance(db).await?;
    let to_match: Vec<(usize, LineToMatch)> = statement
        .lines
        .iter()
        .enumerate()
        .filter(|(i, l)| l.is_valid() && line_prints[*i].as_ref().is_none_or(|p| !existing.contains_key(p)))
        .map(|(i, l)| {
            (
                i,
                LineToMatch {
                    date: l.date.unwrap_or_default(),
                    amount: l.amount.unwrap_or_default(),
                    bank_label: l.bank_label.clone(),
                },
            )
        })
        .collect();
    let candidates: Vec<Candidate<TransactionId>> = match statement.period {
        Some((start, end)) => db::imports::matchable(
            &db.read,
            account.id,
            start - chrono::Days::new(tolerance.unsigned_abs()),
            end + chrono::Days::new(tolerance.unsigned_abs()),
        )
        .await?
        .into_iter()
        .map(|m| Candidate { id: m.id, date: m.balance_date, amount: m.amount, label: m.label })
        .collect(),
        None => Vec::new(),
    };
    let matches = matcher::assign(&to_match.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>(), &candidates, tolerance);
    let matched: HashMap<usize, (TransactionId, u8)> =
        to_match.iter().zip(matches).filter_map(|((i, _), m)| m.map(|m| (*i, m))).collect();

    let suggester = Suggester::load(db, account.id).await?;
    let batch_id = ImportBatchId::new();
    let elsewhere = db::imports::reviewing_lines_elsewhere(&db.read, batch_id).await?;

    let mut rows = Vec::new();
    for (index, line) in statement.lines.iter().enumerate() {
        let print = line_prints[index].clone();
        let mut row = LineRow {
            id: ImportLineId::new(),
            batch_id,
            line_number: line.line_number,
            raw: serde_json::json!(line.raw),
            date: line.date,
            value_date: line.value_date,
            amount: line.amount,
            bank_label: Some(line.bank_label.clone()).filter(|l| !l.is_empty()),
            fingerprint: print.clone(),
            errors: line.errors.clone(),
            match_kind: MatchKind::None,
            match_transaction_id: None,
            match_score: None,
            suggested: Value::Null,
            decision: None,
            transaction_id: None,
        };
        if line.is_valid() {
            let (date, amount) = (line.date.unwrap_or_default(), line.amount.unwrap_or_default());
            if let Some(duplicate) = print.as_ref().and_then(|p| existing.get(p)) {
                row.match_kind = MatchKind::Duplicate;
                row.match_transaction_id = Some(*duplicate);
                row.decision = Some(ImportDecision::Skip);
            } else if let Some((transaction, score)) = matched.get(&index) {
                row.match_kind = MatchKind::Candidate;
                row.match_transaction_id = Some(*transaction);
                row.match_score = Some(i64::from(*score));
                row.decision =
                    Some(if *score >= AUTO_MATCH_SCORE { ImportDecision::Match } else { ImportDecision::Create });
            } else {
                row.decision = Some(ImportDecision::Create);
            }
            if row.decision == Some(ImportDecision::Create) || row.match_kind == MatchKind::Candidate {
                let mut review = suggester.suggest(account.id, line, amount);
                if review.transfer_to.is_none() {
                    review.proposal = transfer_proposal(db, account.id, amount, date, tolerance, &elsewhere).await?;
                }
                row.suggested = review.to_value();
            }
        }
        rows.push(row);
    }

    let batch = BatchRow {
        id: batch_id,
        account_id: account.id,
        profile_id,
        file_name: upload.file_name.clone(),
        file_sha256: fingerprint::file_sha256(&upload.content),
        status: BatchStatus::Reviewing,
        period_start: statement.period.map(|p| p.0),
        period_end: statement.closing_date.or(statement.period.map(|p| p.1)),
        statement_opening_balance: statement.opening_balance,
        statement_closing_balance: statement.closing_balance,
        committed_at: None,
        created_at: String::new(),
    };
    data.ensure().map_err(anyhow::Error::from)?;
    std::fs::write(stored_file(data, batch_id, &upload.file_name), &upload.content).map_err(anyhow::Error::from)?;
    let mut tx = db.write.begin().await?;
    db::imports::insert_batch(&mut *tx, &batch).await?;
    for row in &rows {
        db::imports::insert_line(&mut *tx, row).await?;
    }
    tx.commit().await?;
    Ok(BatchCreated { batch_id, profile_id })
}

// ---------------------------------------------------------------- Lecture des lots

fn summary(batch: &BatchRow, lines: &[LineRow], account_name: String, data: &DataDir) -> BatchSummary {
    let count = |d: ImportDecision| lines.iter().filter(|l| l.errors.is_empty() && l.decision == Some(d)).count();
    BatchSummary {
        id: batch.id,
        account_id: batch.account_id,
        account_name,
        profile_id: batch.profile_id,
        file_name: batch.file_name.clone(),
        status: batch.status,
        period_start: batch.period_start,
        period_end: batch.period_end,
        created_at: batch.created_at.clone(),
        committed_at: batch.committed_at.clone(),
        line_count: lines.len(),
        error_count: lines.iter().filter(|l| !l.errors.is_empty()).count(),
        to_create: count(ImportDecision::Create),
        to_match: count(ImportDecision::Match),
        to_skip: count(ImportDecision::Skip),
        has_file: find_stored_file(data, batch.id).is_some(),
    }
}

pub async fn list(db: &Db, data: &DataDir) -> AppResult<Vec<BatchSummary>> {
    let names: HashMap<AccountId, String> =
        db::accounts::list(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect();
    let mut result = Vec::new();
    for batch in db::imports::list_batches(&db.read).await? {
        let lines = db::imports::lines_of(&db.read, batch.id).await?;
        result.push(summary(&batch, &lines, names.get(&batch.account_id).cloned().unwrap_or_default(), data));
    }
    Ok(result)
}

pub async fn get(db: &Db, data: &DataDir, id: ImportBatchId) -> AppResult<BatchDetail> {
    let batch = db::imports::find_batch(&db.write, id).await?.ok_or(AppError::NotFound("lot d'import"))?;
    let account = db::accounts::find(&db.read, batch.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    let lines = db::imports::lines_of(&db.write, id).await?;
    let transactions: HashMap<TransactionId, db::transactions::TransactionRow> =
        db::transactions::list_for_account(&db.read, account.id).await?.into_iter().map(|t| (t.id, t)).collect();
    let valid: Vec<Money> = lines.iter().filter(|l| l.errors.is_empty()).filter_map(|l| l.amount).collect();
    let statement = StatementInfo {
        period_start: batch.period_start,
        period_end: batch.period_end,
        opening_minor: batch.statement_opening_balance.map(Money::minor),
        closing_minor: batch.statement_closing_balance.map(Money::minor),
        gap_minor: batch
            .statement_opening_balance
            .zip(batch.statement_closing_balance)
            .map(|(o, c)| domain_import::statement_gap(o, c, &valid).minor()),
    };
    let views = lines
        .iter()
        .map(|l| {
            let review = Review::from_value(&l.suggested);
            let matched = l.match_transaction_id.and_then(|id| transactions.get(&id)).map(|t| MatchView {
                transaction_id: t.id,
                date: t.date,
                label: t.label.clone(),
                amount_minor: t.amount.minor(),
                clearing_status: t.clearing_status,
                score: l.match_score,
            });
            ImportLineView {
                id: l.id,
                line_number: l.line_number,
                date: l.date,
                value_date: l.value_date,
                amount_minor: l.amount.map(Money::minor),
                bank_label: l.bank_label.clone(),
                raw: l.raw.clone(),
                errors: l.errors.clone(),
                match_kind: l.match_kind,
                matched,
                decision: l.decision,
                label: review.label.clone(),
                splits: review.splits.clone().unwrap_or_default(),
                note: review.note.clone(),
                transfer_to: review.transfer_to,
                transfer_transaction_id: review.transfer_transaction_id,
                proposal: review.proposal.clone(),
                suggested_by_rule: !review.rules.is_empty(),
                learned: review.learned,
                transaction_id: l.transaction_id,
            }
        })
        .collect();
    Ok(BatchDetail {
        summary: summary(&batch, &lines, account.name.clone(), data),
        currency: account.currency.code().to_owned(),
        statement,
        lines: views,
    })
}

// ---------------------------------------------------------------- Revue

async fn reviewing(db: &Db, id: ImportBatchId) -> AppResult<BatchRow> {
    let batch = db::imports::find_batch(&db.read, id).await?.ok_or(AppError::NotFound("lot d'import"))?;
    if batch.status != BatchStatus::Reviewing {
        return Err(AppError::invalid("BATCH_NOT_REVIEWING", "ce lot n'est plus en revue"));
    }
    Ok(batch)
}

/// Enregistre la décision et les choix d'une ligne (IMP-04).
pub async fn review_line(
    db: &Db,
    batch_id: ImportBatchId,
    line_id: ImportLineId,
    input: &LineReviewInput,
) -> AppResult<()> {
    let batch = reviewing(db, batch_id).await?;
    let line = db::imports::lines_of(&db.read, batch_id)
        .await?
        .into_iter()
        .find(|l| l.id == line_id)
        .ok_or(AppError::NotFound("ligne d'import"))?;
    if !line.errors.is_empty() && input.decision != ImportDecision::Skip {
        return Err(AppError::invalid("LINE_INVALID", "une ligne en erreur ne peut pas être importée (R-IMP-3)"));
    }
    let amount = line.amount.unwrap_or_default();
    if input.decision == ImportDecision::Match {
        let target = input.match_transaction_id.ok_or_else(|| {
            AppError::invalid_field("MATCH_REQUIRED", "match_transaction_id", "choisir la transaction à pointer")
        })?;
        let t = db::transactions::find(&db.read, target).await?.ok_or(AppError::NotFound("transaction"))?;
        if t.account_id != batch.account_id || t.amount != amount {
            return Err(AppError::invalid(
                "MATCH_MISMATCH",
                "la transaction pointée doit être du même compte et du même montant",
            ));
        }
    }
    if !input.splits.is_empty() {
        split::check_splits(
            amount,
            &input.splits.iter().map(|s| Money::from_minor(s.amount_minor)).collect::<Vec<_>>(),
        )?;
    }
    let mut review = Review::from_value(&line.suggested);
    review.label = clean(input.label.as_deref());
    review.splits = (!input.splits.is_empty()).then(|| input.splits.clone());
    review.note = clean(input.note.as_deref());
    review.transfer_to = input.transfer_to;
    review.transfer_transaction_id = input.transfer_transaction_id;
    let matched =
        if input.decision == ImportDecision::Match { input.match_transaction_id } else { line.match_transaction_id };
    db::imports::update_line_review(&db.write, line_id, Some(input.decision), matched, &review.to_value()).await?;
    Ok(())
}

/// Décision ou catégorie appliquée à plusieurs lignes (IMP-04, actions en masse).
pub async fn bulk(db: &Db, batch_id: ImportBatchId, input: &BulkInput) -> AppResult<()> {
    reviewing(db, batch_id).await?;
    let lines = db::imports::lines_of(&db.read, batch_id).await?;
    let mut tx = db.write.begin().await?;
    for line in lines.iter().filter(|l| input.line_ids.contains(&l.id) && l.errors.is_empty()) {
        let mut review = Review::from_value(&line.suggested);
        let mut decision = line.decision;
        if let Some(d) = input.decision {
            // Pointer suppose un candidat : sans lui, la ligne reste à créer.
            decision = Some(if d == ImportDecision::Match && line.match_transaction_id.is_none() {
                ImportDecision::Create
            } else {
                d
            });
        }
        if let (Some(category), Some(amount)) = (input.category_id, line.amount)
            && review.transfer_to.is_none()
            && review.splits.as_ref().is_none_or(|s| s.len() <= 1)
        {
            review.splits =
                Some(vec![DraftSplit { category_id: Some(category), amount_minor: amount.minor(), note: None }]);
        }
        db::imports::update_line_review(&mut *tx, line.id, decision, line.match_transaction_id, &review.to_value())
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Annule une revue : le lot et ses lignes sont supprimés, sans autre effet (R-IMP-1).
pub async fn cancel(db: &Db, data: &DataDir, id: ImportBatchId) -> AppResult<()> {
    reviewing(db, id).await?;
    db::imports::delete_batch(&db.write, id).await?;
    if let Some(path) = find_stored_file(data, id) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

// ---------------------------------------------------------------- Validation et annulation

/// Valide le lot de façon atomique (R-IMP-15) : transactions créées et pointées, virements, points de
/// contrôle du relevé (R-IMP-16).
pub async fn commit(db: &Db, id: ImportBatchId, input: &CommitInput) -> AppResult<CommitResult> {
    let batch = reviewing(db, id).await?;
    let lines = db::imports::lines_of(&db.read, id).await?;
    let mut tx = db.write.begin().await?;
    let account = db::accounts::find(&mut *tx, batch.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    let lock = reconciliation::lock(&mut tx, account.id).await?;
    let existing: HashSet<String> =
        db::imports::fingerprints_of(&mut *tx, account.id).await?.into_iter().map(|(p, _)| p).collect();
    let mut result = CommitResult { created: 0, matched: 0, skipped: 0, transfers: 0, checkpoints: Vec::new() };
    let mut touched = Vec::new();
    // Un projet proposé par une règle n'est rattaché que s'il est encore en cours (R-BUD-24).
    let active_projects: HashSet<ProjectId> =
        db::budgets::projects(&mut *tx).await?.into_iter().filter(|p| p.active).map(|p| p.id).collect();

    for line in lines.iter() {
        let (Some(date), Some(amount), Some(bank_label)) = (line.date, line.amount, line.bank_label.as_deref()) else {
            result.skipped += 1;
            continue;
        };
        if !line.errors.is_empty() {
            result.skipped += 1;
            continue;
        }
        let mut review = Review::from_value(&line.suggested);
        match line.decision.unwrap_or(ImportDecision::Create) {
            ImportDecision::Skip => result.skipped += 1,
            ImportDecision::Match => {
                let target = line.match_transaction_id.ok_or_else(|| {
                    AppError::invalid(
                        "MATCH_REQUIRED",
                        format!("ligne {} : transaction à pointer manquante", line.line_number),
                    )
                })?;
                let t =
                    db::transactions::find(&mut *tx, target).await?.ok_or(AppError::NotFound("transaction pointée"))?;
                let pointable = t.account_id == account.id
                    && t.amount == amount
                    && (t.clearing_status == ClearingStatus::Pending
                        || (t.clearing_status == ClearingStatus::Cleared && t.bank_date.is_none()));
                if !pointable {
                    return Err(AppError::invalid(
                        "MATCH_STALE",
                        format!("ligne {} : la transaction à pointer a changé", line.line_number),
                    ));
                }
                // La date de solde devient la date bancaire (R-TRX-7) : vérifier le verrou avant et après.
                lock.check(t.balance_date)?;
                lock.check(date)?;
                review.previous = db::imports::pointing_state(&mut *tx, target).await?;
                let print = line.fingerprint.clone().filter(|p| !existing.contains(p));
                db::imports::point(&mut *tx, target, date, bank_label, line.value_date, print.as_deref()).await?;
                db::imports::set_line_transaction(&mut *tx, line.id, Some(target), &review.to_value()).await?;
                touched.push(target);
                result.matched += 1;
            }
            ImportDecision::Create => {
                period(&account).check(date)?;
                lock.check(date)?;
                let transfer_to = review.transfer_to;
                let project = review.project.filter(|p| transfer_to.is_none() && active_projects.contains(p));
                let splits: Vec<DraftSplit> = match (&review.splits, transfer_to) {
                    (_, Some(_)) | (None, None) => {
                        vec![DraftSplit { category_id: None, amount_minor: amount.minor(), note: None }]
                    }
                    (Some(splits), None) => splits.clone(),
                };
                let label = review.label.clone().unwrap_or_else(|| bank_label.to_owned());
                let label: String = label.chars().take(200).collect();
                // Une ligne forcée en création malgré un doublon n'emporte pas l'empreinte déjà utilisée.
                let print = line.fingerprint.clone().filter(|p| !existing.contains(p));
                let transaction = TransactionId::new();
                let new = NewTransaction {
                    id: transaction,
                    account_id: account.id,
                    date,
                    bank_date: Some(date),
                    value_date: line.value_date,
                    amount,
                    bank_label: Some(bank_label),
                    label: &label,
                    note: review.note.as_deref(),
                    clearing_status: ClearingStatus::Cleared,
                    origin: TransactionOrigin::Import,
                    recurring_rule_id: None,
                    fingerprint: print.as_deref(),
                };
                let new_splits: Vec<NewSplit<'_>> = splits
                    .iter()
                    .map(|s| NewSplit {
                        amount: Money::from_minor(s.amount_minor),
                        category_id: s.category_id,
                        note: s.note.as_deref(),
                        project_id: project,
                    })
                    .collect();
                super::transactions::insert_validated(&mut tx, &new, &new_splits).await?;
                touched.push(transaction);
                result.created += 1;

                if let Some(counterpart_account) = transfer_to {
                    let other = db::accounts::find(&mut *tx, counterpart_account)
                        .await?
                        .ok_or(AppError::NotFound("compte de contrepartie"))?;
                    let counterpart = match review.transfer_transaction_id {
                        Some(existing_leg) => {
                            let leg = db::transactions::find(&mut *tx, existing_leg)
                                .await?
                                .ok_or(AppError::NotFound("jambe de virement"))?;
                            if db::transactions::transfer_of(&mut *tx, existing_leg).await?.is_some() {
                                return Err(DomainError::AlreadyTransferLeg.into());
                            }
                            db::transactions::replace_splits(
                                &mut tx,
                                leg.id,
                                &[NewSplit { amount: leg.amount, category_id: None, note: None, project_id: None }],
                            )
                            .await?;
                            leg
                        }
                        None => {
                            // Jambe opposée créée (R-TRX-36), hors période rapprochée du compte de contrepartie.
                            period(&other).check(date)?;
                            reconciliation::lock(&mut tx, other.id).await?.check(date)?;
                            let created = TransactionId::new();
                            let leg = NewTransaction {
                                id: created,
                                account_id: other.id,
                                date,
                                bank_date: None,
                                value_date: None,
                                amount: -amount,
                                bank_label: None,
                                label: &label,
                                note: None,
                                clearing_status: transfer::opposite_leg_status(other.kind.family()),
                                origin: TransactionOrigin::Manual,
                                recurring_rule_id: None,
                                fingerprint: None,
                            };
                            super::transactions::insert_validated(
                                &mut tx,
                                &leg,
                                &[NewSplit { amount: -amount, category_id: None, note: None, project_id: None }],
                            )
                            .await?;
                            review.created_counterpart_id = Some(created);
                            db::transactions::find(&mut *tx, created)
                                .await?
                                .ok_or(AppError::NotFound("jambe de virement"))?
                        }
                    };
                    let this = db::transactions::find(&mut *tx, transaction)
                        .await?
                        .ok_or(AppError::NotFound("transaction"))?;
                    let (outflow, inflow) =
                        if amount.is_negative() { (&this, &counterpart) } else { (&counterpart, &this) };
                    let leg = |t: &db::transactions::TransactionRow, currency| TransferLeg {
                        account_id: t.account_id,
                        currency,
                        amount: t.amount,
                        balance_date: t.balance_date,
                        split_categories: vec![None],
                    };
                    let (oc, ic) = if amount.is_negative() {
                        (account.currency, other.currency)
                    } else {
                        (other.currency, account.currency)
                    };
                    transfer::check_transfer(&leg(outflow, oc), &leg(inflow, ic))?;
                    db::transactions::insert_transfer(&mut *tx, outflow.id, inflow.id, None).await?;
                    touched.push(counterpart.id);
                    result.transfers += 1;
                }
                db::imports::set_line_transaction(&mut *tx, line.id, Some(transaction), &review.to_value()).await?;
            }
        }
    }

    // Points de contrôle du relevé (R-IMP-16, R-RAP-2).
    let mut checkpoints = Vec::new();
    if account.kind.family().has_computed_balance() {
        if let (Some(balance), Some(date)) = (batch.statement_closing_balance, batch.period_end) {
            checkpoints.push(
                reconciliation::upsert(
                    &mut tx,
                    &account,
                    date,
                    balance,
                    "STATEMENT",
                    Some(id),
                    input.replace_checkpoints,
                )
                .await?,
            );
        }
        if let (Some(balance), Some(start)) = (batch.statement_opening_balance, batch.period_start)
            && let Some(date) = domain_import::opening_checkpoint_date(start)
            && date >= account.opening_date
        {
            checkpoints.push(
                reconciliation::upsert(
                    &mut tx,
                    &account,
                    date,
                    balance,
                    "STATEMENT",
                    Some(id),
                    input.replace_checkpoints,
                )
                .await?,
            );
        }
    }

    check_split_sums(&mut tx, &touched).await?;
    // Statistique d'usage des règles appliquées aux lignes créées.
    for line in lines.iter().filter(|l| l.errors.is_empty() && l.decision == Some(ImportDecision::Create)) {
        for rule in Review::from_value(&line.suggested).rules {
            db::rules::add_hit(&mut *tx, rule).await?;
        }
    }
    db::imports::set_batch_status(&mut *tx, id, BatchStatus::Committed, true).await?;
    tx.commit().await?;
    let all = reconciliation::list(db, account.id).await?;
    result.checkpoints = all.into_iter().filter(|c| checkpoints.contains(&c.id)).collect();
    Ok(result)
}

/// Annule un lot validé tant qu'aucune de ses transactions n'est rapprochée (R-IMP-17).
pub async fn revert(db: &Db, id: ImportBatchId) -> AppResult<()> {
    let batch = db::imports::find_batch(&db.read, id).await?.ok_or(AppError::NotFound("lot d'import"))?;
    if batch.status != BatchStatus::Committed {
        return Err(AppError::invalid("BATCH_NOT_COMMITTED", "seul un lot validé peut être annulé"));
    }
    let lines = db::imports::lines_of(&db.read, id).await?;
    let mut tx = db.write.begin().await?;
    for line in &lines {
        let Some(transaction) = line.transaction_id else { continue };
        let review = Review::from_value(&line.suggested);
        let t = db::transactions::find(&mut *tx, transaction).await?;
        if t.as_ref().is_some_and(|t| t.clearing_status == ClearingStatus::Reconciled) {
            return Err(AppError::invalid(
                "BATCH_RECONCILED",
                format!("ligne {} : transaction rapprochée, déverrouiller d'abord", line.line_number),
            ));
        }
        match line.decision {
            Some(ImportDecision::Match) => {
                if let Some(previous) = &review.previous {
                    db::imports::unpoint(&mut *tx, transaction, previous).await?;
                }
            }
            _ => {
                if let Some(counterpart) = review.created_counterpart_id
                    && let Some(c) = db::transactions::find(&mut *tx, counterpart).await?
                {
                    if c.clearing_status == ClearingStatus::Reconciled {
                        return Err(AppError::invalid(
                            "BATCH_RECONCILED",
                            "une jambe de virement créée est rapprochée",
                        ));
                    }
                    db::transactions::delete(&mut *tx, counterpart).await?;
                }
                if t.is_some() {
                    db::transactions::delete(&mut *tx, transaction).await?;
                }
            }
        }
        db::imports::set_line_transaction(&mut *tx, line.id, None, &line.suggested).await?;
    }
    db::checkpoints::delete_for_batch(&mut *tx, id).await?;
    db::imports::set_batch_status(&mut *tx, id, BatchStatus::Reverted, false).await?;
    tx.commit().await?;
    Ok(())
}

/// Supprime le fichier source conservé ; le lot et ses lignes restent (R-IMP-21).
pub async fn delete_file(db: &Db, data: &DataDir, id: ImportBatchId) -> AppResult<()> {
    db::imports::find_batch(&db.read, id).await?.ok_or(AppError::NotFound("lot d'import"))?;
    let path = find_stored_file(data, id).ok_or(AppError::NotFound("fichier source"))?;
    std::fs::remove_file(path).map_err(anyhow::Error::from)?;
    Ok(())
}

/// Contenu du fichier source conservé, pour le téléchargement.
pub async fn source_file(db: &Db, data: &DataDir, id: ImportBatchId) -> AppResult<(String, Vec<u8>)> {
    let batch = db::imports::find_batch(&db.read, id).await?.ok_or(AppError::NotFound("lot d'import"))?;
    let path = find_stored_file(data, id).ok_or(AppError::NotFound("fichier source"))?;
    Ok((batch.file_name, std::fs::read(path).map_err(anyhow::Error::from)?))
}
