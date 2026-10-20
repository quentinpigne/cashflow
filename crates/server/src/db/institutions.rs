//! Établissements (spec 01, CPT-02).

use cashflow_domain::shared::ids::InstitutionId;
use sqlx::SqliteExecutor;

use super::codec::id_from_db;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct InstitutionRow {
    pub id: InstitutionId,
    pub name: String,
    pub bank_code: Option<String>,
    pub bic: Option<String>,
    pub archived: bool,
}

pub struct InstitutionFields<'a> {
    pub name: &'a str,
    pub bank_code: Option<&'a str>,
    pub bic: Option<&'a str>,
    pub archived: bool,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<InstitutionRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, bank_code, bic, archived AS "archived: bool" FROM institution ORDER BY name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(InstitutionRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                bank_code: r.bank_code,
                bic: r.bic,
                archived: r.archived,
            })
        })
        .collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: InstitutionId) -> anyhow::Result<Option<InstitutionRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, name, bank_code, bic, archived AS "archived: bool" FROM institution WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| {
        Ok(InstitutionRow {
            id: id_from_db(&r.id)?,
            name: r.name,
            bank_code: r.bank_code,
            bic: r.bic,
            archived: r.archived,
        })
    })
    .transpose()
}

pub async fn name_taken(
    db: impl SqliteExecutor<'_>,
    name: &str,
    except: Option<InstitutionId>,
) -> anyhow::Result<bool> {
    let except = except.map(|id| id.to_string()).unwrap_or_default();
    let count =
        sqlx::query_scalar!("SELECT count(*) FROM institution WHERE name = ? COLLATE NOCASE AND id <> ?", name, except)
            .fetch_one(db)
            .await?;
    Ok(count > 0)
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: InstitutionId, f: &InstitutionFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO institution (id, name, bank_code, bic, archived, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        id,
        f.name,
        f.bank_code,
        f.bic,
        f.archived,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: InstitutionId, f: &InstitutionFields<'_>) -> anyhow::Result<bool> {
    let id = id.to_string();
    let now = timestamp();
    let result = sqlx::query!(
        "UPDATE institution SET name = ?, bank_code = ?, bic = ?, archived = ?, updated_at = ? WHERE id = ?",
        f.name,
        f.bank_code,
        f.bic,
        f.archived,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}
