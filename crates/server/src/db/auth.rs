//! Utilisateur unique, sessions et jetons d'accès (ADR-0006, ADR-0015, ADR-0023).

use cashflow_domain::shared::ids::{AccessTokenId, SessionId, UserId};
use sqlx::SqliteExecutor;

use super::codec::id_from_db;
use crate::clock::timestamp;

pub struct UserRow {
    pub id: UserId,
    pub password_hash: String,
}

pub async fn find_user(db: impl SqliteExecutor<'_>) -> anyhow::Result<Option<UserRow>> {
    let row =
        sqlx::query!("SELECT id, password_hash FROM app_user ORDER BY created_at LIMIT 1").fetch_optional(db).await?;
    row.map(|r| Ok(UserRow { id: id_from_db(&r.id)?, password_hash: r.password_hash })).transpose()
}

pub async fn insert_user(db: impl SqliteExecutor<'_>, display_name: &str, password_hash: &str) -> anyhow::Result<()> {
    let id = UserId::new().to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO app_user (id, display_name, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        id,
        display_name,
        password_hash,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update_password(db: impl SqliteExecutor<'_>, user: UserId, password_hash: &str) -> anyhow::Result<()> {
    let id = user.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE app_user SET password_hash = ?, updated_at = ? WHERE id = ?", password_hash, now, id)
        .execute(db)
        .await?;
    Ok(())
}

pub struct SessionRow {
    pub id: SessionId,
    pub expires_at: String,
    pub last_seen_at: String,
}

pub async fn insert_session(
    db: impl SqliteExecutor<'_>,
    user: UserId,
    token_hash: &str,
    device_label: Option<&str>,
    expires_at: &str,
) -> anyhow::Result<()> {
    let id = SessionId::new().to_string();
    let user = user.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO session (id, user_id, token_hash, device_label, expires_at, last_seen_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        user,
        token_hash,
        device_label,
        expires_at,
        now,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find_session(db: impl SqliteExecutor<'_>, token_hash: &str) -> anyhow::Result<Option<SessionRow>> {
    let row = sqlx::query!("SELECT id, expires_at, last_seen_at FROM session WHERE token_hash = ?", token_hash)
        .fetch_optional(db)
        .await?;
    row.map(|r| Ok(SessionRow { id: id_from_db(&r.id)?, expires_at: r.expires_at, last_seen_at: r.last_seen_at }))
        .transpose()
}

/// Prolonge la session (30 jours glissants, ADR-0015).
pub async fn touch_session(db: impl SqliteExecutor<'_>, session: SessionId, expires_at: &str) -> anyhow::Result<()> {
    let id = session.to_string();
    let now = timestamp();
    sqlx::query!(
        "UPDATE session SET expires_at = ?, last_seen_at = ?, updated_at = ? WHERE id = ?",
        expires_at,
        now,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_session(db: impl SqliteExecutor<'_>, token_hash: &str) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM session WHERE token_hash = ?", token_hash).execute(db).await?;
    Ok(())
}

pub async fn delete_all_sessions(db: impl SqliteExecutor<'_>) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM session").execute(db).await?;
    Ok(())
}

pub async fn delete_expired_sessions(db: impl SqliteExecutor<'_>, now: &str) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM session WHERE expires_at < ?", now).execute(db).await?;
    Ok(())
}

pub struct AccessTokenRow {
    pub id: AccessTokenId,
    pub name: String,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

pub async fn insert_access_token(
    db: impl SqliteExecutor<'_>,
    user: UserId,
    name: &str,
    token_hash: &str,
) -> anyhow::Result<()> {
    let id = AccessTokenId::new().to_string();
    let user = user.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO access_token (id, user_id, name, token_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        id,
        user,
        name,
        token_hash,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find_access_token(
    db: impl SqliteExecutor<'_>,
    token_hash: &str,
) -> anyhow::Result<Option<AccessTokenRow>> {
    let row =
        sqlx::query!("SELECT id, name, last_used_at, created_at FROM access_token WHERE token_hash = ?", token_hash)
            .fetch_optional(db)
            .await?;
    row.map(|r| {
        Ok(AccessTokenRow {
            id: id_from_db(&r.id)?,
            name: r.name,
            last_used_at: r.last_used_at,
            created_at: r.created_at,
        })
    })
    .transpose()
}

pub async fn list_access_tokens(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<AccessTokenRow>> {
    let rows =
        sqlx::query!("SELECT id, name, last_used_at, created_at FROM access_token ORDER BY name").fetch_all(db).await?;
    rows.into_iter()
        .map(|r| {
            Ok(AccessTokenRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                last_used_at: r.last_used_at,
                created_at: r.created_at,
            })
        })
        .collect()
}

/// Note le dernier usage du jeton (ADR-0023).
pub async fn touch_access_token(db: impl SqliteExecutor<'_>, token: AccessTokenId) -> anyhow::Result<()> {
    let id = token.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE access_token SET last_used_at = ?, updated_at = ? WHERE id = ?", now, now, id)
        .execute(db)
        .await?;
    Ok(())
}

/// Révoque un jeton ; renvoie faux s'il n'existe pas.
pub async fn delete_access_token(db: impl SqliteExecutor<'_>, name: &str) -> anyhow::Result<bool> {
    let result = sqlx::query!("DELETE FROM access_token WHERE name = ?", name).execute(db).await?;
    Ok(result.rows_affected() > 0)
}
