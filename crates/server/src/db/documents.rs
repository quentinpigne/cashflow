//! Bibliothèque de documents (spec 12, ADR-0024) : description en base, contenu sur le disque, rattachements et
//! index de recherche.

use std::collections::HashMap;

use cashflow_domain::Date;
use cashflow_domain::documents::{Description, DocumentKind, LinkKind, LinkTarget, MediaType};
use cashflow_domain::shared::ids::DocumentId;
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::id_from_db;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct DocumentRow {
    pub id: DocumentId,
    pub kind: Option<DocumentKind>,
    pub title: String,
    pub document_date: Option<Date>,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub month: Option<Date>,
    pub note: Option<String>,
    pub file_name: String,
    pub media_type: MediaType,
    pub size_bytes: i64,
    pub sha256: String,
    pub content_indexed: bool,
    pub created_at: String,
}

impl DocumentRow {
    pub fn description(&self) -> Description {
        Description {
            kind: self.kind,
            title: self.title.clone(),
            document_date: self.document_date,
            period_start: self.period_start,
            period_end: self.period_end,
            month: self.month,
            note: self.note.clone(),
        }
    }

    /// Date d'envoi, jour de `created_at`.
    pub fn sent_on(&self) -> Option<Date> {
        self.created_at.get(..10).and_then(|day| day.parse().ok())
    }
}

pub fn media_type_from_db(value: &str) -> anyhow::Result<MediaType> {
    MediaType::parse(value).ok_or_else(|| anyhow::anyhow!("type de document inconnu en base : {value}"))
}

fn kind_from_db(value: Option<&str>) -> anyhow::Result<Option<DocumentKind>> {
    value
        .map(|v| DocumentKind::parse(v).ok_or_else(|| anyhow::anyhow!("type de document inconnu en base : {v}")))
        .transpose()
}

macro_rules! document_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(DocumentRow {
            id: id_from_db(&r.id)?,
            kind: kind_from_db(r.kind.as_deref())?,
            title: r.title,
            document_date: r.document_date,
            period_start: r.period_start,
            period_end: r.period_end,
            month: r.month,
            note: r.note,
            file_name: r.file_name,
            media_type: media_type_from_db(&r.media_type)?,
            size_bytes: r.size_bytes,
            sha256: r.sha256,
            content_indexed: r.content_indexed,
            created_at: r.created_at,
        })
    }};
}

pub async fn insert(db: impl SqliteExecutor<'_>, row: &DocumentRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let kind = row.kind.map(DocumentKind::as_str);
    let media_type = row.media_type.as_str();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO document (id, kind, title, document_date, period_start, period_end, month, note, file_name,
                               media_type, size_bytes, sha256, content_indexed, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        kind,
        row.title,
        row.document_date,
        row.period_start,
        row.period_end,
        row.month,
        row.note,
        row.file_name,
        media_type,
        row.size_bytes,
        row.sha256,
        row.content_indexed,
        row.created_at,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find(db: impl SqliteExecutor<'_>, id: DocumentId) -> anyhow::Result<Option<DocumentRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, kind, title, document_date AS "document_date: Date", period_start AS "period_start: Date",
                  period_end AS "period_end: Date", month AS "month: Date", note, file_name, media_type, size_bytes,
                  sha256, content_indexed AS "content_indexed: bool", created_at
           FROM document WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| document_row!(r)).transpose()
}

/// Tous les documents, du plus récent envoi au plus ancien.
pub async fn all(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<DocumentRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, kind, title, document_date AS "document_date: Date", period_start AS "period_start: Date",
                  period_end AS "period_end: Date", month AS "month: Date", note, file_name, media_type, size_bytes,
                  sha256, content_indexed AS "content_indexed: bool", created_at
           FROM document ORDER BY created_at DESC, id DESC"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| document_row!(r)).collect()
}

/// Documents d'un type rattachés à un objet : relevés d'un compte, fiches de paie d'un emploi.
pub async fn owned(
    db: impl SqliteExecutor<'_>,
    kind: DocumentKind,
    target: uuid::Uuid,
) -> anyhow::Result<Vec<DocumentRow>> {
    let (kind, target) = (kind.as_str(), target.to_string());
    let rows = sqlx::query!(
        r#"SELECT d.id, d.kind, d.title, d.document_date AS "document_date: Date",
                  d.period_start AS "period_start: Date", d.period_end AS "period_end: Date", d.month AS "month: Date",
                  d.note, d.file_name, d.media_type, d.size_bytes, d.sha256,
                  d.content_indexed AS "content_indexed: bool", d.created_at
           FROM document d
           WHERE d.kind = ?1 AND EXISTS (SELECT 1 FROM document_link l WHERE l.document_id = d.id
                                         AND (l.account_id = ?2 OR l.employment_id = ?2))
           ORDER BY d.month, d.created_at"#,
        kind,
        target
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| document_row!(r)).collect()
}

/// Document qui a déjà ce contenu, s'il y en a un (R-DOC-3) : identifiant et titre.
pub async fn with_sha256(db: impl SqliteExecutor<'_>, sha256: &str) -> anyhow::Result<Option<(DocumentId, String)>> {
    let row = sqlx::query!("SELECT id, title FROM document WHERE sha256 = ?", sha256).fetch_optional(db).await?;
    row.map(|r| Ok((id_from_db(&r.id)?, r.title))).transpose()
}

/// Modifie la description ; le fichier ne change pas (R-DOC-2).
pub async fn update_description(
    db: impl SqliteExecutor<'_>,
    id: DocumentId,
    description: &Description,
) -> anyhow::Result<u64> {
    let id = id.to_string();
    let kind = description.kind.map(DocumentKind::as_str);
    let now = timestamp();
    Ok(sqlx::query!(
        "UPDATE document SET kind = ?, title = ?, document_date = ?, period_start = ?, period_end = ?, month = ?,
                             note = ?, updated_at = ?
         WHERE id = ?",
        kind,
        description.title,
        description.document_date,
        description.period_start,
        description.period_end,
        description.month,
        description.note,
        now,
        id
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn set_content_indexed(db: impl SqliteExecutor<'_>, id: DocumentId, indexed: bool) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("UPDATE document SET content_indexed = ? WHERE id = ?", indexed, id).execute(db).await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: DocumentId) -> anyhow::Result<u64> {
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM document WHERE id = ?", id).execute(db).await?.rows_affected())
}

// ---------------------------------------------------------------- Rattachements (R-DOC-9)

/// Un rattachement, avec ce qu'il faut pour l'afficher : le nom de l'objet, et la date et le montant d'une
/// transaction.
#[derive(Debug, Clone)]
pub struct LinkRow {
    pub document_id: DocumentId,
    pub target: LinkTarget,
    pub name: String,
    pub date: Option<Date>,
    pub amount: Option<i64>,
}

/// Une colonne de cible par nature, une seule renseignée.
struct Columns {
    account: Option<String>,
    institution: Option<String>,
    employment: Option<String>,
    transaction: Option<String>,
    asset: Option<String>,
    loan: Option<String>,
    payee: Option<String>,
    project: Option<String>,
}

impl Columns {
    fn of(target: LinkTarget) -> Self {
        let id = Some(target.id.to_string());
        let pick = |kind: LinkKind| if target.kind == kind { id.clone() } else { None };
        Self {
            account: pick(LinkKind::Account),
            institution: pick(LinkKind::Institution),
            employment: pick(LinkKind::Employment),
            transaction: pick(LinkKind::Transaction),
            asset: pick(LinkKind::Asset),
            loan: pick(LinkKind::Loan),
            payee: pick(LinkKind::Payee),
            project: pick(LinkKind::Project),
        }
    }

    fn target(self) -> anyhow::Result<LinkTarget> {
        let found = [
            (LinkKind::Account, self.account),
            (LinkKind::Institution, self.institution),
            (LinkKind::Employment, self.employment),
            (LinkKind::Transaction, self.transaction),
            (LinkKind::Asset, self.asset),
            (LinkKind::Loan, self.loan),
            (LinkKind::Payee, self.payee),
            (LinkKind::Project, self.project),
        ]
        .into_iter()
        .find_map(|(kind, id)| id.map(|id| (kind, id)));
        let (kind, id) = found.ok_or_else(|| anyhow::anyhow!("rattachement sans cible en base"))?;
        Ok(LinkTarget { kind, id: id_from_db(&id)? })
    }
}

/// Rattachement tel qu'en base : le document et sa cible.
#[derive(Debug, Clone, Copy)]
pub struct RawLink {
    pub document_id: DocumentId,
    pub target: LinkTarget,
}

/// Tous les rattachements, dans l'ordre où ils ont été faits.
pub async fn raw_links(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<RawLink>> {
    let rows = sqlx::query!(
        "SELECT document_id, account_id, institution_id, employment_id, transaction_id, asset_id, loan_id, payee_id,
                project_id
         FROM document_link ORDER BY rowid"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            let columns = Columns {
                account: r.account_id,
                institution: r.institution_id,
                employment: r.employment_id,
                transaction: r.transaction_id,
                asset: r.asset_id,
                loan: r.loan_id,
                payee: r.payee_id,
                project: r.project_id,
            };
            Ok(RawLink { document_id: id_from_db(&r.document_id)?, target: columns.target()? })
        })
        .collect()
}

/// Nom des objets rattachés à un document, par identifiant ; date et montant des transactions.
pub struct TargetNames {
    pub names: HashMap<uuid::Uuid, String>,
    pub transactions: HashMap<uuid::Uuid, (Date, i64)>,
}

/// Noms de tous les objets rattachés à au moins un document.
pub async fn target_names(conn: &mut SqliteConnection) -> anyhow::Result<TargetNames> {
    let mut names = HashMap::new();
    macro_rules! names_of {
        ($sql:literal) => {
            for r in sqlx::query!($sql).fetch_all(&mut *conn).await? {
                names.insert(id_from_db(&r.id)?, r.name);
            }
        };
    }
    names_of!("SELECT id, name FROM account WHERE id IN (SELECT account_id FROM document_link)");
    names_of!("SELECT id, name FROM institution WHERE id IN (SELECT institution_id FROM document_link)");
    names_of!("SELECT id, employer AS name FROM employment WHERE id IN (SELECT employment_id FROM document_link)");
    names_of!("SELECT id, name FROM asset WHERE id IN (SELECT asset_id FROM document_link)");
    names_of!("SELECT id, name FROM loan WHERE id IN (SELECT loan_id FROM document_link)");
    names_of!("SELECT id, name FROM payee WHERE id IN (SELECT payee_id FROM document_link)");
    names_of!("SELECT id, name FROM project WHERE id IN (SELECT project_id FROM document_link)");
    let rows = sqlx::query!(
        r#"SELECT id, label AS "label!", date AS "date!: Date", amount AS "amount!" FROM ledger_transaction
           WHERE id IN (SELECT transaction_id FROM document_link)"#
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut transactions = HashMap::new();
    for r in rows {
        let id = id_from_db(&r.id)?;
        names.insert(id, r.label);
        transactions.insert(id, (r.date, r.amount));
    }
    Ok(TargetNames { names, transactions })
}

impl TargetNames {
    pub fn link(&self, raw: RawLink) -> LinkRow {
        let transaction = self.transactions.get(&raw.target.id);
        LinkRow {
            document_id: raw.document_id,
            target: raw.target,
            name: self.names.get(&raw.target.id).cloned().unwrap_or_default(),
            date: transaction.map(|t| t.0),
            amount: transaction.map(|t| t.1),
        }
    }
}

/// Tous les rattachements, avec le nom de leur objet.
pub async fn links(conn: &mut SqliteConnection) -> anyhow::Result<Vec<LinkRow>> {
    let raw = raw_links(&mut *conn).await?;
    let names = target_names(conn).await?;
    Ok(raw.into_iter().map(|r| names.link(r)).collect())
}

/// Rattachements d'un document, avec le nom de leur objet.
pub async fn links_of(conn: &mut SqliteConnection, document: DocumentId) -> anyhow::Result<Vec<LinkRow>> {
    Ok(links(conn).await?.into_iter().filter(|l| l.document_id == document).collect())
}

/// Rattache un document ; sans effet s'il l'est déjà (R-DOC-9). Renvoie le nombre de lignes créées.
pub async fn insert_link(db: impl SqliteExecutor<'_>, document: DocumentId, target: LinkTarget) -> anyhow::Result<u64> {
    let document = document.to_string();
    let c = Columns::of(target);
    Ok(sqlx::query!(
        "INSERT OR IGNORE INTO document_link (document_id, account_id, institution_id, employment_id, transaction_id,
                                              asset_id, loan_id, payee_id, project_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        document,
        c.account,
        c.institution,
        c.employment,
        c.transaction,
        c.asset,
        c.loan,
        c.payee,
        c.project
    )
    .execute(db)
    .await?
    .rows_affected())
}

pub async fn delete_link(db: impl SqliteExecutor<'_>, document: DocumentId, target: LinkTarget) -> anyhow::Result<u64> {
    let (document, target) = (document.to_string(), target.id.to_string());
    // Une seule cible est renseignée : `coalesce` la donne.
    Ok(sqlx::query!(
        "DELETE FROM document_link
         WHERE document_id = ? AND coalesce(account_id, institution_id, employment_id, transaction_id, asset_id,
                                            loan_id, payee_id, project_id) = ?",
        document,
        target
    )
    .execute(db)
    .await?
    .rows_affected())
}

/// Documents rattachés à un objet, et pour chacun s'il n'est rattaché qu'à lui (R-DOC-12). Les identifiants UUIDv7
/// sont uniques entre les tables : l'objet se reconnaît à son identifiant, quelle que soit sa nature.
pub async fn attached_to(db: impl SqliteExecutor<'_>, target: uuid::Uuid) -> anyhow::Result<Vec<(DocumentId, bool)>> {
    let links = raw_links(db).await?;
    let mut counts: HashMap<DocumentId, usize> = HashMap::new();
    for link in &links {
        *counts.entry(link.document_id).or_default() += 1;
    }
    Ok(links
        .iter()
        .filter(|l| l.target.id == target)
        .map(|l| (l.document_id, counts.get(&l.document_id) == Some(&1)))
        .collect())
}

/// Reporte les rattachements d'un tiers sur un autre ; un document rattaché aux deux garde le sien (R-DOC-13).
pub async fn move_payee_links(db: impl SqliteExecutor<'_>, from: &str, to: &str) -> anyhow::Result<()> {
    sqlx::query!("UPDATE OR IGNORE document_link SET payee_id = ? WHERE payee_id = ?", to, from).execute(db).await?;
    Ok(())
}

/// Nom de l'objet visé, s'il existe.
pub async fn target_name(db: impl SqliteExecutor<'_>, target: LinkTarget) -> anyhow::Result<Option<String>> {
    let id = target.id.to_string();
    Ok(match target.kind {
        LinkKind::Account => {
            sqlx::query_scalar!("SELECT name FROM account WHERE id = ?", id).fetch_optional(db).await?
        }
        LinkKind::Institution => {
            sqlx::query_scalar!("SELECT name FROM institution WHERE id = ?", id).fetch_optional(db).await?
        }
        LinkKind::Employment => {
            sqlx::query_scalar!("SELECT employer FROM employment WHERE id = ?", id).fetch_optional(db).await?
        }
        LinkKind::Transaction => {
            sqlx::query_scalar!(r#"SELECT label AS "label!" FROM ledger_transaction WHERE id = ?"#, id)
                .fetch_optional(db)
                .await?
        }
        LinkKind::Asset => sqlx::query_scalar!("SELECT name FROM asset WHERE id = ?", id).fetch_optional(db).await?,
        LinkKind::Loan => sqlx::query_scalar!("SELECT name FROM loan WHERE id = ?", id).fetch_optional(db).await?,
        LinkKind::Payee => sqlx::query_scalar!("SELECT name FROM payee WHERE id = ?", id).fetch_optional(db).await?,
        LinkKind::Project => {
            sqlx::query_scalar!("SELECT name FROM project WHERE id = ?", id).fetch_optional(db).await?
        }
    })
}

/// Transactions qui ont au moins un justificatif (DOC-04).
pub async fn transactions_with_documents(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT DISTINCT transaction_id AS "transaction_id!: String" FROM document_link
           WHERE transaction_id IS NOT NULL"#
    )
    .fetch_all(db)
    .await?)
}

// ---------------------------------------------------------------- Index de recherche (R-DOC-18, R-DOC-19)
//
// La vérification à la compilation de sqlx ne sait pas analyser une table virtuelle FTS5 (le compilateur s'arrête) :
// ces requêtes, constantes, sont vérifiées à l'exécution et couvertes par les tests de `services::document_tests`.

/// Indexe un document : sa description et le texte de son contenu.
pub async fn index(conn: &mut SqliteConnection, row: &DocumentRow, content: Option<&str>) -> anyhow::Result<()> {
    let id = row.id.to_string();
    sqlx::query("DELETE FROM document_search WHERE document_id = ?").bind(&id).execute(&mut *conn).await?;
    sqlx::query("INSERT INTO document_search (document_id, title, note, file_name, content) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&row.title)
        .bind(&row.note)
        .bind(&row.file_name)
        .bind(content)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Met à jour la description indexée ; le texte du contenu ne change pas.
pub async fn reindex_description(
    db: impl SqliteExecutor<'_>,
    id: DocumentId,
    description: &Description,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE document_search SET title = ?, note = ? WHERE document_id = ?")
        .bind(&description.title)
        .bind(&description.note)
        .bind(id.to_string())
        .execute(db)
        .await?;
    Ok(())
}

pub async fn unindex(db: impl SqliteExecutor<'_>, id: DocumentId) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM document_search WHERE document_id = ?").bind(id.to_string()).execute(db).await?;
    Ok(())
}

/// Documents absents de l'index : repris d'une migration ou d'une restauration.
pub async fn unindexed(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<DocumentId>> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM document
         WHERE id NOT IN (SELECT document_id FROM document_search WHERE document_id IS NOT NULL)",
    )
    .fetch_all(db)
    .await?;
    ids.iter().map(|id| id_from_db(id)).collect()
}

/// Documents qui répondent à une requête FTS5, du plus pertinent au moins pertinent, avec l'extrait du contenu où
/// la requête est trouvée (mots entre les caractères de contrôle U+0002 et U+0003).
pub async fn search(db: impl SqliteExecutor<'_>, query: &str) -> anyhow::Result<Vec<(DocumentId, Option<String>)>> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT document_id, snippet(document_search, 4, char(2), char(3), '…', 12)
         FROM document_search WHERE document_search MATCH ? ORDER BY rank",
    )
    .bind(query)
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|(id, snippet)| Ok((id_from_db(&id)?, snippet))).collect()
}

/// Documents qui répondent à une requête FTS5, sans ordre.
pub async fn matching(db: impl SqliteExecutor<'_>, query: &str) -> anyhow::Result<Vec<DocumentId>> {
    let ids: Vec<String> = sqlx::query_scalar("SELECT document_id FROM document_search WHERE document_search MATCH ?")
        .bind(query)
        .fetch_all(db)
        .await?;
    ids.iter().map(|id| id_from_db(id)).collect()
}
