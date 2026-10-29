//! Bibliothèque de documents (spec 12, ADR-0024) : envoi, description, rattachements, recherche, consultation et
//! suppression. Le contenu est sur le disque : écriture en deux temps, suppression différée.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::Context;
use cashflow_domain::documents::{self, Description, DocumentKind, LinkKind, LinkTarget, MediaType};
use cashflow_domain::shared::ids::{DocumentId, EmploymentId};
use cashflow_domain::{Date, DomainError};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;
use utoipa::{IntoParams, ToSchema};

use super::clean;
use crate::clock::timestamp;
use crate::config::DataDir;
use crate::db::documents::{DocumentRow, LinkRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::readers;

/// Durée de séjour d'un document supprimé dans la corbeille : plus que la rétention des sauvegardes (12 mois).
const TRASH_DAYS: i64 = 400;
const TRASH_DATE: &str = "%Y%m%d";

// ---------------------------------------------------------------- Vues

/// Objet auquel un document est rattaché (R-DOC-9).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LinkView {
    pub kind: LinkKind,
    #[schema(value_type = String, format = Uuid)]
    pub id: uuid::Uuid,
    /// Nom du compte, de l'employeur, du tiers… ; libellé d'une transaction.
    pub name: String,
    /// Date d'une transaction.
    pub date: Option<Date>,
    /// Montant d'une transaction, en unités mineures.
    pub amount_minor: Option<i64>,
}

/// Un document de la bibliothèque (R-DOC-4 à R-DOC-8).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DocumentView {
    pub id: DocumentId,
    /// Absent : le document est à classer (R-DOC-5).
    pub kind: Option<DocumentKind>,
    pub title: String,
    pub document_date: Option<Date>,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    /// Premier jour du mois d'un relevé ou d'une fiche de paie (R-DOC-7).
    pub month: Option<Date>,
    pub note: Option<String>,
    pub file_name: String,
    pub media_type: MediaType,
    pub size_bytes: i64,
    /// Du texte a été extrait du fichier et indexé (R-DOC-19).
    pub content_indexed: bool,
    pub sent_on: Date,
    /// Date qui classe le document (R-DOC-8).
    pub reference_date: Date,
    pub links: Vec<LinkView>,
    /// Dans une recherche : extrait du contenu où le texte est trouvé, mots trouvés entre U+0002 et U+0003.
    pub snippet: Option<String>,
}

/// Recherche et filtres de la bibliothèque (R-DOC-18, R-DOC-20).
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DocumentFilter {
    /// Texte cherché dans la description, les rattachements et le contenu.
    pub q: Option<String>,
    pub kind: Option<DocumentKind>,
    /// Seulement les documents à classer (sans type).
    #[serde(default)]
    pub unsorted: bool,
    /// Année de la date de référence.
    pub year: Option<i32>,
    /// Seulement les documents rattachés à un objet de cette nature.
    pub link_kind: Option<LinkKind>,
    /// Seulement les documents rattachés à cet objet.
    #[param(value_type = Option<String>, format = Uuid)]
    pub link_id: Option<uuid::Uuid>,
    /// Seulement les documents sans rattachement.
    #[serde(default)]
    pub unlinked: bool,
}

/// Description d'un document ; le fichier ne se modifie pas (R-DOC-2).
#[derive(Debug, Deserialize, ToSchema)]
pub struct DocumentInput {
    pub kind: Option<DocumentKind>,
    pub title: String,
    pub document_date: Option<Date>,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    /// Premier jour du mois, pour un relevé ou une fiche de paie.
    pub month: Option<Date>,
    pub note: Option<String>,
}

/// Document envoyé, avec sa description éventuelle et l'objet depuis lequel il l'est (R-DOC-14, R-DOC-15).
#[derive(Debug, Default)]
pub struct Upload {
    pub file_name: String,
    pub content: Vec<u8>,
    pub kind: Option<DocumentKind>,
    pub title: Option<String>,
    pub document_date: Option<Date>,
    pub month: Option<Date>,
    pub note: Option<String>,
    pub target: Option<LinkTarget>,
}

/// Documents rattachés à un objet, pour la confirmation de sa suppression (R-DOC-12).
#[derive(Debug, Serialize, ToSchema)]
pub struct AttachedDocuments {
    pub total: usize,
    /// Rattachés seulement à cet objet : ceux que la suppression peut emporter.
    pub exclusive: usize,
}

fn link_view(link: &LinkRow) -> LinkView {
    LinkView {
        kind: link.target.kind,
        id: link.target.id,
        name: link.name.clone(),
        date: link.date,
        amount_minor: link.amount,
    }
}

fn view(row: DocumentRow, links: &[LinkRow], snippet: Option<String>, today: Date) -> DocumentView {
    let description = row.description();
    let sent_on = row.sent_on().unwrap_or(today);
    DocumentView {
        reference_date: documents::reference_date(&description, sent_on),
        id: row.id,
        kind: row.kind,
        title: row.title,
        document_date: row.document_date,
        period_start: row.period_start,
        period_end: row.period_end,
        month: row.month,
        note: row.note,
        file_name: row.file_name,
        media_type: row.media_type,
        size_bytes: row.size_bytes,
        content_indexed: row.content_indexed,
        sent_on,
        links: links.iter().map(link_view).collect(),
        snippet,
    }
}

// ---------------------------------------------------------------- Lecture

/// Documents de la bibliothèque, filtrés et triés (R-DOC-18 à R-DOC-20).
pub async fn list(db: &Db, filter: &DocumentFilter, today: Date) -> AppResult<Vec<DocumentView>> {
    let mut links: HashMap<DocumentId, Vec<LinkRow>> = HashMap::new();
    for link in db::documents::links(&mut *db.read.acquire().await?).await? {
        links.entry(link.document_id).or_default().push(link);
    }
    let no_links = Vec::new();
    let words = filter.q.as_deref().map(documents::search_words).unwrap_or_default();

    // Un document répond si chacun des mots est dans sa description, son contenu ou le nom d'un rattachement.
    let mut found: Option<HashSet<DocumentId>> = None;
    for word in &words {
        let mut hits: HashSet<DocumentId> =
            db::documents::matching(&db.read, &documents::prefix_query(word)).await?.into_iter().collect();
        hits.extend(
            links.iter().filter(|(_, l)| l.iter().any(|l| documents::name_matches(&l.name, word))).map(|(id, _)| *id),
        );
        found = Some(match found {
            Some(previous) => previous.intersection(&hits).copied().collect(),
            None => hits,
        });
    }
    // Pertinence et extrait, pour les documents trouvés dans l'index par tous les mots.
    let mut ranked: HashMap<DocumentId, (usize, Option<String>)> = HashMap::new();
    if !words.is_empty() {
        let query = words.iter().map(|w| documents::prefix_query(w)).collect::<Vec<_>>().join(" ");
        for (rank, (id, snippet)) in db::documents::search(&db.read, &query).await?.into_iter().enumerate() {
            ranked.insert(id, (rank, snippet.filter(|s| s.contains('\u{2}'))));
        }
    }

    let mut views: Vec<(usize, DocumentView)> = Vec::new();
    for row in db::documents::all(&db.read).await? {
        if found.as_ref().is_some_and(|found| !found.contains(&row.id)) {
            continue;
        }
        let doc_links = links.get(&row.id).unwrap_or(&no_links);
        let keep = filter.kind.is_none_or(|kind| row.kind == Some(kind))
            && (!filter.unsorted || row.kind.is_none())
            && (!filter.unlinked || doc_links.is_empty())
            && filter.link_kind.is_none_or(|kind| doc_links.iter().any(|l| l.target.kind == kind))
            && filter.link_id.is_none_or(|id| doc_links.iter().any(|l| l.target.id == id));
        if !keep {
            continue;
        }
        let (rank, snippet) = ranked.get(&row.id).cloned().unwrap_or((usize::MAX, None));
        let view = view(row, doc_links, snippet, today);
        if filter.year.is_some_and(|year| view.reference_date.year() != year) {
            continue;
        }
        views.push((rank, view));
    }
    // Sans texte, par date de référence décroissante ; avec un texte, par pertinence d'abord.
    views.sort_by(|(rank_a, a), (rank_b, b)| {
        rank_a.cmp(rank_b).then(b.reference_date.cmp(&a.reference_date)).then(b.sent_on.cmp(&a.sent_on))
    });
    Ok(views.into_iter().map(|(_, v)| v).collect())
}

pub async fn get(db: &Db, id: DocumentId, today: Date) -> AppResult<DocumentView> {
    let row = db::documents::find(&db.read, id).await?.ok_or(AppError::NotFound("document"))?;
    let links = db::documents::links_of(&mut *db.read.acquire().await?, id).await?;
    Ok(view(row, &links, None, today))
}

/// Relevés d'un compte ou fiches de paie d'un emploi, du mois le plus ancien au plus récent.
pub async fn owned(db: &Db, kind: DocumentKind, owner: uuid::Uuid) -> AppResult<Vec<DocumentRow>> {
    Ok(db::documents::owned(&db.read, kind, owner).await?)
}

/// Fichier d'un document, avec sa description (R-DOC-21).
pub async fn file(db: &Db, data: &DataDir, id: DocumentId) -> AppResult<(DocumentRow, Vec<u8>)> {
    let row = db::documents::find(&db.read, id).await?.ok_or(AppError::NotFound("document"))?;
    let content = read(data, id)?;
    Ok((row, content))
}

/// Documents rattachés à un objet, et ceux rattachés seulement à lui (R-DOC-12).
pub async fn attached(db: &Db, target: uuid::Uuid) -> AppResult<AttachedDocuments> {
    let attached = db::documents::attached_to(&db.read, target).await?;
    Ok(AttachedDocuments { total: attached.len(), exclusive: attached.iter().filter(|(_, e)| *e).count() })
}

// ---------------------------------------------------------------- Écriture

/// Le mois d'une fiche de paie tombe pendant son emploi (R-SAL-15).
async fn check_payslip_month(
    conn: &mut SqliteConnection,
    description: &Description,
    links: &[LinkTarget],
) -> AppResult<()> {
    let (Some(DocumentKind::Payslip), Some(month)) = (description.kind, description.month) else {
        return Ok(());
    };
    for link in links.iter().filter(|l| l.kind == LinkKind::Employment) {
        super::salaries::check_payslip_month(conn, EmploymentId::from_uuid(link.id), month).await?;
    }
    Ok(())
}

/// Texte d'un PDF pour la recherche, extrait hors de la boucle asynchrone (R-DOC-19).
async fn extract_text(media_type: MediaType, content: Vec<u8>) -> Option<String> {
    if media_type != MediaType::Pdf {
        return None;
    }
    tokio::task::spawn_blocking(move || readers::pdf::plain_text(&content)).await.ok().flatten()
}

/// Enregistre un document préparé, ses rattachements et son index, dans la transaction de l'appelant.
async fn insert(
    conn: &mut SqliteConnection,
    row: &DocumentRow,
    targets: &[LinkTarget],
    text: Option<&str>,
) -> AppResult<()> {
    db::documents::insert(&mut *conn, row).await?;
    for target in targets {
        db::documents::insert_link(&mut *conn, row.id, *target).await?;
    }
    db::documents::index(conn, row, text).await?;
    Ok(())
}

/// Archive un document envoyé, depuis la bibliothèque ou depuis la fiche d'un objet (R-DOC-1 à R-DOC-15).
pub async fn create(db: &Db, data: &DataDir, upload: Upload, today: Date) -> AppResult<DocumentView> {
    let mut staged = stage(data, &upload.file_name, &upload.content)?;
    if let Some((id, title)) = db::documents::with_sha256(&db.read, &staged.row.sha256).await? {
        return Err(DomainError::DocumentAlreadyArchived { id, title }.into());
    }
    let owner_name = match upload.target {
        Some(target) => {
            Some(db::documents::target_name(&db.read, target).await?.ok_or(AppError::NotFound("objet rattaché"))?)
        }
        None => None,
    };
    let row = &mut staged.row;
    row.kind = upload.kind;
    row.month = upload.month;
    row.document_date = upload.document_date;
    row.note = clean(upload.note.as_deref());
    // Titre d'un relevé ou d'une fiche envoyé depuis son compte ou son emploi, sinon le nom du fichier.
    let owned_title = match (upload.kind, upload.target, &owner_name, upload.month) {
        (Some(kind), Some(target), Some(name), Some(month)) if kind.owner() == Some(target.kind) => {
            Some(documents::owned_title(kind, name, month))
        }
        _ => None,
    };
    row.title = clean(upload.title.as_deref()).or(owned_title).unwrap_or_else(|| row.title.clone());
    let targets: Vec<LinkTarget> = upload.target.into_iter().collect();
    let description = row.description();
    documents::check_description(&description, row.media_type)?;
    documents::check_owner(row.kind, &targets.iter().map(|t| t.kind).collect::<Vec<_>>(), true)?;
    let text = extract_text(row.media_type, upload.content).await;
    row.content_indexed = text.is_some();
    let mut tx = db.write.begin().await?;
    check_payslip_month(&mut tx, &description, &targets).await?;
    insert(&mut tx, &staged.row, &targets, text.as_deref()).await?;
    tx.commit().await?;
    let id = staged.row.id;
    staged.publish()?;
    get(db, id, today).await
}

/// Relevé d'un import PDF à archiver avec la validation du lot (R-IMP-28).
pub struct FromImport {
    pub account: cashflow_domain::shared::ids::AccountId,
    pub account_name: String,
    pub file_name: String,
    pub content: Vec<u8>,
    pub month: Date,
    pub period: Option<(Date, Date)>,
}

/// Prépare l'archivage d'un relevé importé dans la transaction de validation : rien si le fichier n'est pas un
/// PDF ou s'il est déjà dans la bibliothèque (R-DOC-3). Le document renvoyé est publié après la validation.
pub async fn archive_import(
    conn: &mut SqliteConnection,
    data: &DataDir,
    import: FromImport,
) -> AppResult<Option<Staged>> {
    if documents::check(&import.content) != Ok(MediaType::Pdf) {
        return Ok(None);
    }
    let mut staged = stage(data, &import.file_name, &import.content)?;
    if db::documents::with_sha256(&mut *conn, &staged.row.sha256).await?.is_some() {
        return Ok(None);
    }
    let text = extract_text(MediaType::Pdf, import.content).await;
    let row = &mut staged.row;
    row.kind = Some(DocumentKind::BankStatement);
    row.month = Some(import.month);
    row.title = documents::owned_title(DocumentKind::BankStatement, &import.account_name, import.month);
    (row.period_start, row.period_end) = import.period.unzip();
    row.content_indexed = text.is_some();
    let target = LinkTarget { kind: LinkKind::Account, id: *import.account.as_uuid() };
    insert(conn, &staged.row, &[target], text.as_deref()).await?;
    Ok(Some(staged))
}

/// Modifie la description d'un document (R-DOC-2, R-DOC-4 à R-DOC-7, R-DOC-10).
pub async fn update(db: &Db, id: DocumentId, input: &DocumentInput, today: Date) -> AppResult<DocumentView> {
    let row = db::documents::find(&db.read, id).await?.ok_or(AppError::NotFound("document"))?;
    let targets: Vec<LinkTarget> =
        db::documents::links_of(&mut *db.read.acquire().await?, id).await?.iter().map(|l| l.target).collect();
    let description = Description {
        kind: input.kind,
        title: input.title.trim().to_owned(),
        document_date: input.document_date,
        period_start: input.period_start,
        period_end: input.period_end,
        month: input.month,
        note: clean(input.note.as_deref()),
    };
    documents::check_description(&description, row.media_type)?;
    let kinds: Vec<LinkKind> = targets.iter().map(|t| t.kind).collect();
    documents::check_owner(description.kind, &kinds, description.kind != row.kind)?;
    let mut tx = db.write.begin().await?;
    check_payslip_month(&mut tx, &description, &targets).await?;
    db::documents::update_description(&mut *tx, id, &description).await?;
    db::documents::reindex_description(&mut *tx, id, &description).await?;
    tx.commit().await?;
    get(db, id, today).await
}

/// Rattache un document à un objet (R-DOC-9, R-DOC-10, R-DOC-11) ; sans effet s'il l'est déjà.
pub async fn link(db: &Db, id: DocumentId, target: LinkTarget, today: Date) -> AppResult<DocumentView> {
    let row = db::documents::find(&db.read, id).await?.ok_or(AppError::NotFound("document"))?;
    db::documents::target_name(&db.read, target).await?.ok_or(AppError::NotFound("objet rattaché"))?;
    let mut targets: Vec<LinkTarget> =
        db::documents::links_of(&mut *db.read.acquire().await?, id).await?.iter().map(|l| l.target).collect();
    if !targets.contains(&target) {
        targets.push(target);
        documents::check_owner(row.kind, &targets.iter().map(|t| t.kind).collect::<Vec<_>>(), false)?;
        let mut tx = db.write.begin().await?;
        check_payslip_month(&mut tx, &row.description(), &[target]).await?;
        db::documents::insert_link(&mut *tx, id, target).await?;
        tx.commit().await?;
    }
    get(db, id, today).await
}

/// Détache un document d'un objet (R-DOC-9, R-DOC-11).
pub async fn unlink(db: &Db, id: DocumentId, target: LinkTarget, today: Date) -> AppResult<DocumentView> {
    if db::documents::delete_link(&db.write, id, target).await? == 0 {
        return Err(AppError::NotFound("rattachement"));
    }
    get(db, id, today).await
}

/// Supprime un document : ses rattachements, son index ; son fichier va à la corbeille (R-DOC-22).
pub async fn delete(db: &Db, data: &DataDir, id: DocumentId, today: Date) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::documents::unindex(&mut *tx, id).await?;
    if db::documents::delete(&mut *tx, id).await? == 0 {
        return Err(AppError::NotFound("document"));
    }
    tx.commit().await?;
    trash(data, &[id], today);
    Ok(())
}

/// Supprime, dans la transaction de suppression d'un objet, les documents rattachés seulement à lui, quand
/// l'utilisateur l'a demandé (R-DOC-12). Les fichiers sont à mettre à la corbeille après la validation.
pub async fn delete_exclusive(conn: &mut SqliteConnection, target: uuid::Uuid) -> AppResult<Vec<DocumentId>> {
    let exclusive: Vec<DocumentId> = db::documents::attached_to(&mut *conn, target)
        .await?
        .into_iter()
        .filter_map(|(id, exclusive)| exclusive.then_some(id))
        .collect();
    for id in &exclusive {
        db::documents::unindex(&mut *conn, *id).await?;
        db::documents::delete(&mut *conn, *id).await?;
    }
    Ok(exclusive)
}

/// Reconstruit l'index de recherche (R-DOC-19) : de tous les documents, ou seulement de ceux qui n'y sont pas
/// (repris d'une migration ou d'une restauration). Renvoie le nombre de documents indexés.
pub async fn reindex(db: &Db, data: &DataDir, all: bool) -> AppResult<usize> {
    let ids: Vec<DocumentId> = if all {
        db::documents::all(&db.read).await?.into_iter().map(|r| r.id).collect()
    } else {
        db::documents::unindexed(&db.read).await?
    };
    for id in &ids {
        let Some(row) = db::documents::find(&db.read, *id).await? else { continue };
        let text = match read(data, *id) {
            Ok(content) => extract_text(row.media_type, content).await,
            Err(error) => {
                tracing::warn!(%error, document = %id, "fichier du document absent : contenu non indexé");
                None
            }
        };
        let mut tx = db.write.begin().await?;
        db::documents::index(&mut tx, &row, text.as_deref()).await?;
        db::documents::set_content_indexed(&mut *tx, *id, text.is_some()).await?;
        tx.commit().await?;
    }
    Ok(ids.len())
}

// ---------------------------------------------------------------- Fichiers (ADR-0024)

/// Nom d'origine conservé, sans chemin, borné à 255 caractères.
fn file_name(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    let name: String = name.chars().filter(|c| !c.is_control()).take(255).collect();
    if name.is_empty() { "document".to_owned() } else { name }
}

/// Document vérifié et écrit sous un nom temporaire, en attente de sa ligne en base. Abandonné (`drop`) sans
/// `publish`, il ne laisse aucun fichier.
pub struct Staged {
    pub row: DocumentRow,
    temp: PathBuf,
    target: PathBuf,
}

/// Vérifie le contenu (R-DOC-1) et l'écrit sous un nom temporaire. La description est celle par défaut : à
/// classer, titré d'après le nom du fichier (R-DOC-4, R-DOC-5).
pub fn stage(data: &DataDir, name: &str, content: &[u8]) -> AppResult<Staged> {
    let media_type = documents::check(content)?;
    let id = DocumentId::new();
    let file_name = file_name(name);
    let row = DocumentRow {
        id,
        kind: None,
        title: documents::default_title(&file_name),
        document_date: None,
        period_start: None,
        period_end: None,
        month: None,
        note: None,
        file_name,
        media_type,
        size_bytes: i64::try_from(content.len()).map_err(anyhow::Error::from)?,
        sha256: hex::encode(Sha256::digest(content)),
        content_indexed: false,
        created_at: timestamp(),
    };
    std::fs::create_dir_all(data.documents()).with_context(|| format!("création de {}", data.documents().display()))?;
    let temp = data.documents().join(format!(".{id}.tmp"));
    std::fs::write(&temp, content).with_context(|| format!("écriture de {}", temp.display()))?;
    Ok(Staged { row, temp, target: data.documents().join(id.to_string()) })
}

impl Staged {
    /// Rend le fichier visible, une fois sa ligne enregistrée.
    pub fn publish(self) -> anyhow::Result<()> {
        std::fs::rename(&self.temp, &self.target).with_context(|| format!("écriture de {}", self.target.display()))
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        // Après `publish`, le fichier temporaire n'existe plus : rien à faire.
        let _ = std::fs::remove_file(&self.temp);
    }
}

/// Contenu d'un document.
pub fn read(data: &DataDir, id: DocumentId) -> AppResult<Vec<u8>> {
    let path = data.documents().join(id.to_string());
    match std::fs::read(&path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(AppError::NotFound("fichier du document")),
        Err(error) => Err(anyhow::Error::from(error).context(format!("lecture de {}", path.display())).into()),
    }
}

/// Déplace les fichiers de documents supprimés en base dans la corbeille, préfixés de la date du jour. La base
/// est déjà à jour : un échec est journalisé, pas remonté.
pub fn trash(data: &DataDir, ids: &[DocumentId], today: Date) {
    for id in ids {
        let from = data.documents().join(id.to_string());
        let to = data.documents_trash().join(format!("{}-{id}", today.format(TRASH_DATE)));
        let moved = std::fs::create_dir_all(data.documents_trash()).and_then(|()| std::fs::rename(&from, &to));
        if let Err(error) = moved {
            tracing::error!(%error, file = %from.display(), "mise à la corbeille d'un document impossible");
        }
    }
}

/// Efface les documents de la corbeille depuis plus de 400 jours (ADR-0024) ; renvoie leur nombre.
pub fn purge_trash(data: &DataDir, today: Date) -> anyhow::Result<usize> {
    let Ok(entries) = std::fs::read_dir(data.documents_trash()) else {
        return Ok(0);
    };
    let mut purged = 0;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let trashed_on = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.split_once('-'))
            .and_then(|(date, _)| chrono::NaiveDate::parse_from_str(date, TRASH_DATE).ok());
        if trashed_on.is_some_and(|on| (today - on).num_days() > TRASH_DAYS) {
            std::fs::remove_file(&path).with_context(|| format!("suppression de {}", path.display()))?;
            purged += 1;
        }
    }
    Ok(purged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn file_name_keeps_only_the_name() {
        assert_eq!(file_name("C:\\Users\\moi\\paie-2026-08.pdf"), "paie-2026-08.pdf");
        assert_eq!(file_name("../../etc/passwd"), "passwd");
        assert_eq!(file_name("  "), "document");
    }

    #[test]
    fn staged_document_is_published_or_left_without_trace() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        data.ensure().unwrap();
        let staged = stage(&data, "paie.pdf", b"%PDF-1.7 fiche").unwrap();
        let id = staged.row.id;
        assert_eq!(staged.row.size_bytes, 14);
        assert_eq!(staged.row.title, "paie");
        drop(staged);
        assert_eq!(std::fs::read_dir(data.documents()).unwrap().count(), 1, "seule la corbeille reste");

        let staged = stage(&data, "paie.pdf", b"%PDF-1.7 fiche").unwrap();
        let published = staged.row.id;
        staged.publish().unwrap();
        assert_eq!(read(&data, published).unwrap(), b"%PDF-1.7 fiche");
        assert!(matches!(read(&data, id), Err(AppError::NotFound(_))));
    }

    #[test]
    fn trashed_document_is_purged_after_400_days_r_doc_22() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        data.ensure().unwrap();
        let staged = stage(&data, "paie.pdf", b"%PDF-1.7 fiche").unwrap();
        let id = staged.row.id;
        staged.publish().unwrap();
        trash(&data, &[id], d("2026-01-10"));
        assert!(matches!(read(&data, id), Err(AppError::NotFound(_))));
        assert_eq!(purge_trash(&data, d("2027-02-14")).unwrap(), 0, "400 jours tout juste");
        assert_eq!(purge_trash(&data, d("2027-02-15")).unwrap(), 1);
        assert_eq!(std::fs::read_dir(data.documents_trash()).unwrap().count(), 0);
    }
}
