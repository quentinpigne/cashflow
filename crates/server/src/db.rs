//! Accès à la base SQLite (ADR-0017) : connexion, migrations et requêtes, par module.

pub mod accounts;
pub mod assets;
pub mod auth;
pub mod budgets;
pub mod categories;
pub mod checkpoints;
pub mod codec;
pub mod documents;
pub mod export;
pub mod imports;
pub mod institutions;
pub mod loans;
pub mod payees;
pub mod projections;
pub mod recurring;
pub mod rules;
pub mod salaries;
pub mod savings;
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

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    /// Applique les migrations dont la version est inférieure à `before`.
    async fn migrate_until(db: &Db, before: i64) {
        let migrations: Vec<_> = MIGRATOR.iter().filter(|m| m.version < before).cloned().collect();
        let partial = Migrator { migrations: Cow::Owned(migrations), ..Migrator::DEFAULT };
        partial.run(&db.write).await.expect("migrations partielles");
    }

    /// Insère un compte d'une nature donnée, avec une valorisation, dans une base au schéma d'avant la migration des
    /// biens et des prêts.
    async fn legacy_account(db: &Db, n: u32, kind: &str, opening: i64, valuation: i64) {
        let now = "2026-10-22T00:00:00Z";
        let id = format!("0192f000-0000-7000-8000-{n:012}");
        sqlx::query(
            "INSERT INTO account (id, name, kind, opening_date, opening_balance, created_at, updated_at)
             VALUES (?, ?, ?, '2026-01-01', ?, ?, ?)",
        )
        .bind(&id)
        .bind(kind)
        .bind(kind)
        .bind(opening)
        .bind(now)
        .bind(now)
        .execute(&db.write)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO valuation (id, account_id, date, value, source, created_at, updated_at)
             VALUES (?, ?, '2026-06-30', ?, 'MANUAL', ?, ?)",
        )
        .bind(format!("0192f000-0000-7000-8000-{:012}", n + 100))
        .bind(&id)
        .bind(valuation)
        .bind(now)
        .bind(now)
        .execute(&db.write)
        .await
        .unwrap();
    }

    /// Les comptes de bien et de prêt deviennent des biens et des prêts (ADR-0019) : valorisations reprises, rubriques
    /// activées, clés étrangères intactes.
    #[tokio::test]
    async fn adr_0019_property_and_loan_accounts_become_assets_and_loans() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::connect(&dir.path().join("test.db")).await.unwrap();
        migrate_until(&db, 20261022000005).await;
        legacy_account(&db, 1, "REAL_ESTATE", 25_000_000, 26_000_000).await;
        legacy_account(&db, 2, "MORTGAGE", -13_900_000, -13_500_000).await;
        legacy_account(&db, 3, "CHECKING", 100_000, 0).await;

        db.migrate().await.unwrap();

        let accounts: Vec<(String,)> = sqlx::query_as("SELECT kind FROM account").fetch_all(&db.write).await.unwrap();
        assert_eq!(accounts, [("CHECKING".to_owned(),)]);
        let asset: (i64, i64) =
            sqlx::query_as("SELECT a.purchase_price, v.value FROM asset a JOIN asset_valuation v ON v.asset_id = a.id")
                .fetch_one(&db.write)
                .await
                .unwrap();
        assert_eq!(asset, (25_000_000, 26_000_000));
        let loan: (String, i64, i64) = sqlx::query_as(
            "SELECT l.mode, l.principal, b.outstanding FROM loan l JOIN loan_balance b ON b.loan_id = l.id",
        )
        .fetch_one(&db.write)
        .await
        .unwrap();
        assert_eq!(loan, ("MANUAL".to_owned(), 13_900_000, 13_500_000));
        let features: i64 = sqlx::query_scalar("SELECT count(*) FROM app_setting WHERE key LIKE 'features.%'")
            .fetch_one(&db.write)
            .await
            .unwrap();
        assert_eq!(features, 2);
        let broken: Vec<(String,)> = sqlx::query_as("PRAGMA foreign_key_check").fetch_all(&db.write).await.unwrap();
        assert!(broken.is_empty());
        let enforced: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(&db.write).await.unwrap();
        assert_eq!(enforced, 1);
    }

    /// Une donnée que la migration ne sait pas convertir (ici une transaction ordinaire sur un compte de prêt) la fait
    /// échouer, sans rien modifier (ADR-0019).
    #[tokio::test]
    async fn adr_0019_migration_stops_on_unconvertible_data() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::connect(&dir.path().join("test.db")).await.unwrap();
        migrate_until(&db, 20261022000005).await;
        legacy_account(&db, 2, "MORTGAGE", -13_900_000, -13_500_000).await;
        let now = "2026-10-22T00:00:00Z";
        sqlx::query(
            "INSERT INTO ledger_transaction (id, account_id, date, amount, label, clearing_status, origin,
                                             created_at, updated_at)
             VALUES ('0192f000-0000-7000-8000-000000000999', '0192f000-0000-7000-8000-000000000002', '2026-03-01',
                     1000, 'Remboursement', 'CLEARED', 'MANUAL', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&db.write)
        .await
        .unwrap();
        assert!(db.migrate().await.is_err());
        let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM account").fetch_one(&db.write).await.unwrap();
        assert_eq!(accounts, 1, "rien n'a été modifié");
    }

    /// La migration des virements les rattache à l'unique ventilation de chaque jambe (ADR-0021) ; supprimer une
    /// jambe supprime toujours le virement.
    #[tokio::test]
    async fn adr_0021_transfers_move_to_the_split_of_each_leg() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::connect(&dir.path().join("test.db")).await.unwrap();
        migrate_until(&db, 20261022000003).await;
        let now = "2026-10-22T00:00:00Z";
        let id = |n: u32| format!("0192f000-0000-7000-8000-{n:012}");
        for (account, name) in [(1, "Courant"), (2, "Livret")] {
            sqlx::query(
                "INSERT INTO account (id, name, kind, opening_date, opening_balance, created_at, updated_at)
                 VALUES (?, ?, 'CHECKING', '2026-01-01', 0, ?, ?)",
            )
            .bind(id(account))
            .bind(name)
            .bind(now)
            .bind(now)
            .execute(&db.write)
            .await
            .unwrap();
        }
        for (transaction, account, amount) in [(11, 1, -5_000), (12, 2, 5_000)] {
            sqlx::query(
                "INSERT INTO ledger_transaction (id, account_id, date, amount, label, clearing_status, origin,
                                                 created_at, updated_at)
                 VALUES (?, ?, '2026-03-01', ?, 'Virement', 'PENDING', 'MANUAL', ?, ?)",
            )
            .bind(id(transaction))
            .bind(id(account))
            .bind(amount)
            .bind(now)
            .bind(now)
            .execute(&db.write)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO split (id, transaction_id, amount, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id(transaction + 10))
            .bind(id(transaction))
            .bind(amount)
            .bind(now)
            .bind(now)
            .execute(&db.write)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO transfer (id, outflow_transaction_id, inflow_transaction_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id(30))
        .bind(id(11))
        .bind(id(12))
        .bind(now)
        .bind(now)
        .execute(&db.write)
        .await
        .unwrap();

        db.migrate().await.unwrap();

        let (outflow, inflow): (String, String) =
            sqlx::query_as("SELECT outflow_split_id, inflow_split_id FROM transfer WHERE id = ?")
                .bind(id(30))
                .fetch_one(&db.write)
                .await
                .unwrap();
        assert_eq!((outflow, inflow), (id(21), id(22)));
        let legs: (String, String) =
            sqlx::query_as("SELECT outflow_transaction_id, inflow_transaction_id FROM transfer_leg WHERE id = ?")
                .bind(id(30))
                .fetch_one(&db.write)
                .await
                .unwrap();
        assert_eq!(legs, (id(11), id(12)));
        let broken: Vec<(String,)> = sqlx::query_as("PRAGMA foreign_key_check").fetch_all(&db.write).await.unwrap();
        assert!(broken.is_empty());
        sqlx::query("DELETE FROM ledger_transaction WHERE id = ?").bind(id(11)).execute(&db.write).await.unwrap();
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM transfer").fetch_one(&db.write).await.unwrap();
        assert_eq!(left, 0);
    }
}
