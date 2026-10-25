//! Documents archivés : métadonnées en base, contenu sur le disque (ADR-0024).

use cashflow_domain::documents::MediaType;
use cashflow_domain::shared::ids::DocumentId;
use sqlx::SqliteExecutor;

use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct DocumentRow {
    pub id: DocumentId,
    pub file_name: String,
    pub media_type: MediaType,
    pub size_bytes: i64,
    pub sha256: String,
    pub created_at: String,
}

pub fn media_type_from_db(value: &str) -> anyhow::Result<MediaType> {
    MediaType::parse(value).ok_or_else(|| anyhow::anyhow!("type de document inconnu en base : {value}"))
}

pub async fn insert(db: impl SqliteExecutor<'_>, row: &DocumentRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let media_type = row.media_type.as_str();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO document (id, file_name, media_type, size_bytes, sha256, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        id,
        row.file_name,
        media_type,
        row.size_bytes,
        row.sha256,
        row.created_at,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: DocumentId) -> anyhow::Result<u64> {
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM document WHERE id = ?", id).execute(db).await?.rows_affected())
}
