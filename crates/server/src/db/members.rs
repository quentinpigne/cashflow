//! Membres du foyer (spec 14).

use cashflow_domain::shared::ids::MemberId;
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::id_from_db;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct MemberRow {
    pub id: MemberId,
    pub name: String,
    pub color: Option<String>,
    pub display_order: i64,
    /// Ce qui lui est affecté (R-MBR-2).
    pub account_count: i64,
    pub split_count: i64,
    pub employment_count: i64,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<MemberRow>> {
    let rows = sqlx::query!(
        r#"SELECT m.id, m.name, m.color, m.display_order,
                  (SELECT count(*) FROM account a WHERE a.member_id = m.id) AS "account_count!: i64",
                  (SELECT count(*) FROM split s WHERE s.member_id = m.id) AS "split_count!: i64",
                  (SELECT count(*) FROM employment e WHERE e.member_id = m.id) AS "employment_count!: i64"
           FROM member m ORDER BY m.display_order, m.name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            anyhow::Ok(MemberRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                color: r.color,
                display_order: r.display_order,
                account_count: r.account_count,
                split_count: r.split_count,
                employment_count: r.employment_count,
            })
        })
        .collect()
}

pub async fn exists(db: impl SqliteExecutor<'_>, id: MemberId) -> anyhow::Result<bool> {
    let id = id.to_string();
    let count = sqlx::query_scalar!("SELECT count(*) FROM member WHERE id = ?", id).fetch_one(db).await?;
    Ok(count > 0)
}

pub async fn count(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!("SELECT count(*) FROM member").fetch_one(db).await?)
}

/// Un nom de membre est unique sans distinction de casse (R-MBR-1).
pub async fn name_taken(db: impl SqliteExecutor<'_>, name: &str, except: Option<MemberId>) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string()).unwrap_or_default();
    let count =
        sqlx::query_scalar!("SELECT count(*) FROM member WHERE name = ? COLLATE NOCASE AND id <> ?", name, except)
            .fetch_one(db)
            .await?;
    Ok(count > 0)
}

/// Un nouveau membre se range après le dernier (R-MBR-1).
pub async fn next_display_order(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    let last = sqlx::query_scalar!(r#"SELECT max(display_order) AS "last?: i64" FROM member"#).fetch_one(db).await?;
    Ok(last.map_or(0, |order| order + 1))
}

pub async fn insert(
    db: impl SqliteExecutor<'_>,
    id: MemberId,
    name: &str,
    color: Option<&str>,
    display_order: i64,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO member (id, name, color, display_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        id,
        name,
        color,
        display_order,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Faux si le membre n'existe pas.
pub async fn update(
    db: impl SqliteExecutor<'_>,
    id: MemberId,
    name: &str,
    color: Option<&str>,
) -> anyhow::Result<bool> {
    let id = id.to_string();
    let now = timestamp();
    let result =
        sqlx::query!("UPDATE member SET name = ?, color = ?, updated_at = ? WHERE id = ?", name, color, now, id)
            .execute(db)
            .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn set_display_order(db: impl SqliteExecutor<'_>, id: MemberId, order: i64) -> anyhow::Result<bool> {
    let id = id.to_string();
    let now = timestamp();
    let result = sqlx::query!("UPDATE member SET display_order = ?, updated_at = ? WHERE id = ?", order, now, id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Remet au foyer ce qui est affecté au membre, puis le supprime (R-MBR-2). Faux s'il n'existe pas.
pub async fn delete(conn: &mut SqliteConnection, id: MemberId) -> anyhow::Result<bool> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE account SET member_id = NULL, updated_at = ? WHERE member_id = ?", now, id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("UPDATE split SET member_id = NULL, updated_at = ? WHERE member_id = ?", now, id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("UPDATE employment SET member_id = NULL, updated_at = ? WHERE member_id = ?", now, id)
        .execute(&mut *conn)
        .await?;
    let result = sqlx::query!("DELETE FROM member WHERE id = ?", id).execute(&mut *conn).await?;
    Ok(result.rows_affected() > 0)
}
