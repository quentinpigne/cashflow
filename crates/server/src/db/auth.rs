//! Utilisateur unique et sessions (ADR-0006, ADR-0015).

use cashflow_domain::shared::ids::{SessionId, UserId};
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
