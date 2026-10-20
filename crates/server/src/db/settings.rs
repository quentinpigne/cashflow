//! Réglages clé/valeur (`app_setting`).

use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::SqliteExecutor;

use crate::clock::timestamp;

pub async fn get<T: DeserializeOwned>(db: impl SqliteExecutor<'_>, key: &str) -> anyhow::Result<Option<T>> {
    let value = sqlx::query_scalar!("SELECT value FROM app_setting WHERE key = ?", key).fetch_optional(db).await?;
    Ok(value.map(|v| serde_json::from_str(&v)).transpose()?)
}

pub async fn set<T: Serialize>(db: impl SqliteExecutor<'_>, key: &str, value: &T) -> anyhow::Result<()> {
    let value = serde_json::to_string(value)?;
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO app_setting (key, value, updated_at) VALUES (?, ?, ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        key,
        value,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}
