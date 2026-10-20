//! Règles de catégorisation (spec 03, IMP-08).

use cashflow_domain::shared::ids::CategorizationRuleId;
use serde_json::Value;
use sqlx::SqliteExecutor;

use super::codec::id_from_db;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct RuleRow {
    pub id: CategorizationRuleId,
    pub name: String,
    pub priority: i64,
    pub conditions: Value,
    pub actions: Value,
    pub stop_processing: bool,
    pub active: bool,
    pub hit_count: i64,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<RuleRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, priority, conditions, actions, stop_processing AS "stop_processing: bool", active AS "active: bool", hit_count
           FROM categorization_rule ORDER BY priority, name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(RuleRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                priority: r.priority,
                conditions: serde_json::from_str(&r.conditions)?,
                actions: serde_json::from_str(&r.actions)?,
                stop_processing: r.stop_processing,
                active: r.active,
                hit_count: r.hit_count,
            })
        })
        .collect()
}

pub async fn upsert(db: impl SqliteExecutor<'_>, r: &RuleRow) -> anyhow::Result<()> {
    let id = r.id.to_string();
    let conditions = serde_json::to_string(&r.conditions)?;
    let actions = serde_json::to_string(&r.actions)?;
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO categorization_rule (id, name, priority, conditions, actions, stop_processing, active, hit_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, priority = excluded.priority, conditions = excluded.conditions,
                                        actions = excluded.actions, stop_processing = excluded.stop_processing,
                                        active = excluded.active, updated_at = excluded.updated_at",
        id,
        r.name,
        r.priority,
        conditions,
        actions,
        r.stop_processing,
        r.active,
        r.hit_count,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: CategorizationRuleId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM categorization_rule WHERE id = ?", id).execute(db).await?;
    Ok(())
}

pub async fn add_hit(db: impl SqliteExecutor<'_>, id: CategorizationRuleId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("UPDATE categorization_rule SET hit_count = hit_count + 1 WHERE id = ?", id).execute(db).await?;
    Ok(())
}
