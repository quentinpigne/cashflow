//! Accès à la base SQLite (ADR-0017) : connexion, migrations et requêtes, par module.

pub mod accounts;
pub mod auth;
pub mod categories;
pub mod codec;
pub mod export;
pub mod institutions;
pub mod settings;
pub mod transactions;

use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Les deux accès à la base : une seule connexion en écriture, plusieurs en lecture (ADR-0017).
#[derive(Debug, Clone)]
pub struct Db {
    pub read: SqlitePool,
    pub write: SqlitePool,
}

impl Db {
    /// Ouvre (ou crée) la base sans appliquer les migrations.
    pub async fn connect(path: &Path) -> anyhow::Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));
        let write = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .with_context(|| format!("ouverture de {}", path.display()))?;
        let read = SqlitePoolOptions::new().max_connections(4).connect_with(options.read_only(true)).await?;
        Ok(Self { read, write })
    }

    /// Migrations en attente, dans l'ordre.
    pub async fn pending_migrations(&self) -> anyhow::Result<Vec<i64>> {
        let table_exists: i64 =
            sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'")
                .fetch_one(&self.write)
                .await?;
        let applied: Vec<i64> = if table_exists == 0 {
            Vec::new()
        } else {
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success = 1").fetch_all(&self.write).await?
        };
        Ok(MIGRATOR.iter().map(|m| m.version).filter(|v| !applied.contains(v)).collect())
    }

    /// La base contient-elle déjà des données (au moins un compte ou un utilisateur) ?
    pub async fn has_data(&self) -> anyhow::Result<bool> {
        let tables: i64 =
            sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'account'")
                .fetch_one(&self.write)
                .await?;
        if tables == 0 {
            return Ok(false);
        }
        let rows: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM account) + (SELECT count(*) FROM app_user)")
            .fetch_one(&self.write)
            .await?;
        Ok(rows > 0)
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        MIGRATOR.run(&self.write).await.context("application des migrations")
    }

    /// Copie cohérente de la base, à chaud (`VACUUM INTO`, ADR-0015).
    pub async fn vacuum_into(&self, target: &Path) -> anyhow::Result<()> {
        let target = target.to_str().context("chemin de sauvegarde non UTF-8")?;
        sqlx::query("VACUUM INTO ?").bind(target).execute(&self.write).await?;
        Ok(())
    }
}

/// Ouvre une base de test dans un dossier temporaire, migrations appliquées.
#[cfg(test)]
pub async fn test_db() -> (Db, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("dossier temporaire");
    let db = Db::connect(&dir.path().join("test.db")).await.expect("ouverture");
    db.migrate().await.expect("migrations");
    (db, dir)
}
