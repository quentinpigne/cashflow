//! Catégories (spec 02, TRX-04).

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::CategoryId;
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct CategoryRow {
    pub id: CategoryId,
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<CategoryId>,
    pub archived: bool,
    pub excluded_from_analyses: bool,
    pub display_order: i64,
    pub color: Option<String>,
    pub icon: Option<String>,
    /// Nombre de ventilations qui utilisent la catégorie.
    pub usage: i64,
}

pub struct CategoryFields<'a> {
    pub name: &'a str,
    pub kind: CategoryKind,
    pub parent_id: Option<CategoryId>,
    pub archived: bool,
    pub excluded_from_analyses: bool,
    pub display_order: i64,
    pub color: Option<&'a str>,
    pub icon: Option<&'a str>,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<CategoryRow>> {
    let rows = sqlx::query!(
        r#"SELECT c.id, c.name, c.kind, c.parent_id, c.archived AS "archived: bool",
                  c.excluded_from_analyses AS "excluded_from_analyses: bool", c.display_order, c.color, c.icon,
                  (SELECT count(*) FROM split s WHERE s.category_id = c.id) AS "usage!: i64"
           FROM category c ORDER BY c.display_order, c.name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(CategoryRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                kind: enum_from_db(&r.kind)?,
                parent_id: opt_id_from_db(r.parent_id.as_deref())?,
                archived: r.archived,
                excluded_from_analyses: r.excluded_from_analyses,
                display_order: r.display_order,
                color: r.color,
                icon: r.icon,
                usage: r.usage,
            })
        })
        .collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: CategoryId) -> anyhow::Result<Option<CategoryRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT c.id, c.name, c.kind, c.parent_id, c.archived AS "archived: bool",
                  c.excluded_from_analyses AS "excluded_from_analyses: bool", c.display_order, c.color, c.icon,
                  (SELECT count(*) FROM split s WHERE s.category_id = c.id) AS "usage!: i64"
           FROM category c WHERE c.id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| {
        Ok(CategoryRow {
            id: id_from_db(&r.id)?,
            name: r.name,
            kind: enum_from_db(&r.kind)?,
            parent_id: opt_id_from_db(r.parent_id.as_deref())?,
            archived: r.archived,
            excluded_from_analyses: r.excluded_from_analyses,
            display_order: r.display_order,
            color: r.color,
            icon: r.icon,
            usage: r.usage,
        })
    })
    .transpose()
}

pub async fn count(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!("SELECT count(*) FROM category").fetch_one(db).await?)
}

pub async fn has_children(db: impl SqliteExecutor<'_>, id: CategoryId) -> anyhow::Result<bool> {
    let id = id.to_string();
    let count = sqlx::query_scalar!("SELECT count(*) FROM category WHERE parent_id = ?", id).fetch_one(db).await?;
    Ok(count > 0)
}

/// Un nom est unique au sein d'un même parent et d'une même nature (R-TRX-20).
pub async fn name_taken(
    db: impl SqliteExecutor<'_>,
    name: &str,
    kind: CategoryKind,
    parent: Option<CategoryId>,
    except: Option<CategoryId>,
) -> anyhow::Result<bool> {
    let kind = enum_to_db(&kind);
    let parent = parent.map(|p| p.to_string()).unwrap_or_default();
    let except = except.map(|e| e.to_string()).unwrap_or_default();
    let count = sqlx::query_scalar!(
        "SELECT count(*) FROM category
         WHERE name = ? COLLATE NOCASE AND kind = ? AND coalesce(parent_id, '') = ? AND id <> ?",
        name,
        kind,
        parent,
        except
    )
    .fetch_one(db)
    .await?;
    Ok(count > 0)
}

pub async fn find_by_name(
    db: impl SqliteExecutor<'_>,
    name: &str,
    kind: CategoryKind,
    parent: Option<CategoryId>,
) -> anyhow::Result<Option<CategoryId>> {
    let kind = enum_to_db(&kind);
    let parent = parent.map(|p| p.to_string()).unwrap_or_default();
    let id = sqlx::query_scalar!(
        "SELECT id FROM category WHERE name = ? AND kind = ? AND coalesce(parent_id, '') = ?",
        name,
        kind,
        parent
    )
    .fetch_optional(db)
    .await?;
    id.map(|id| id_from_db(&id)).transpose()
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: CategoryId, f: &CategoryFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let kind = enum_to_db(&f.kind);
    let parent = f.parent_id.map(|p| p.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO category (id, name, kind, parent_id, archived, excluded_from_analyses, display_order, color, icon,
                               created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        f.name,
        kind,
        parent,
        f.archived,
        f.excluded_from_analyses,
        f.display_order,
        f.color,
        f.icon,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: CategoryId, f: &CategoryFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let parent = f.parent_id.map(|p| p.to_string());
    let now = timestamp();
    sqlx::query!(
        "UPDATE category SET name = ?, parent_id = ?, archived = ?, excluded_from_analyses = ?, display_order = ?, color = ?,
                             icon = ?, updated_at = ?
         WHERE id = ?",
        f.name,
        parent,
        f.archived,
        f.excluded_from_analyses,
        f.display_order,
        f.color,
        f.icon,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: CategoryId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM category WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Réaffecte toutes les ventilations, lignes de budget et règles d'une catégorie à une autre (R-TRX-22).
///
/// Les sous-catégories de la source passent sous la cible si celle-ci est une racine, sinon elles
/// sont fusionnées elles aussi dans la cible.
pub async fn merge(conn: &mut SqliteConnection, source: CategoryId, target: CategoryId) -> anyhow::Result<u64> {
    let source = source.to_string();
    let target = target.to_string();
    let now = timestamp();
    let target_is_root =
        sqlx::query_scalar!("SELECT parent_id IS NULL AS \"root!: bool\" FROM category WHERE id = ?", target)
            .fetch_one(&mut *conn)
            .await?;
    let moved = if target_is_root {
        sqlx::query!("UPDATE category SET parent_id = ?, updated_at = ? WHERE parent_id = ?", target, now, source)
            .execute(&mut *conn)
            .await?;
        sqlx::query!("UPDATE split SET category_id = ?, updated_at = ? WHERE category_id = ?", target, now, source)
            .execute(&mut *conn)
            .await?
            .rows_affected()
    } else {
        sqlx::query!(
            "UPDATE split SET category_id = ?, updated_at = ?
             WHERE category_id = ? OR category_id IN (SELECT id FROM category WHERE parent_id = ?)",
            target,
            now,
            source,
            source
        )
        .execute(&mut *conn)
        .await?
        .rows_affected()
    };
    // Références vers la source et, si ses sous-catégories disparaissent, vers elles aussi.
    let with_children = !target_is_root;
    sqlx::query!(
        "UPDATE budget_line SET category_id = ?1, updated_at = ?2
         WHERE category_id = ?3 OR (?4 AND category_id IN (SELECT id FROM category WHERE parent_id = ?3))",
        target,
        now,
        source,
        with_children
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE fixed_savings_terms SET interest_category_id = ?1, updated_at = ?2
         WHERE interest_category_id = ?3
            OR (?4 AND interest_category_id IN (SELECT id FROM category WHERE parent_id = ?3))",
        target,
        now,
        source,
        with_children
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "UPDATE employment SET income_category_id = ?1, updated_at = ?2
         WHERE income_category_id = ?3
            OR (?4 AND income_category_id IN (SELECT id FROM category WHERE parent_id = ?3))",
        target,
        now,
        source,
        with_children
    )
    .execute(&mut *conn)
    .await?;
    if !target_is_root {
        sqlx::query!("DELETE FROM category WHERE parent_id = ?", source).execute(&mut *conn).await?;
    }
    sqlx::query!("DELETE FROM category WHERE id = ?", source).execute(&mut *conn).await?;
    Ok(moved)
}
