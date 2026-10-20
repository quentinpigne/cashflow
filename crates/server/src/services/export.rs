//! Export complet au format JSON et restauration (ADR-0008, format décrit dans `docs/export.md`).

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::clock::timestamp;
use crate::db::{self, Db, MIGRATOR};

pub const FORMAT: &str = "cashflow-export";

/// Version du schéma : celle de la dernière migration connue.
pub fn schema_version() -> i64 {
    MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0)
}

pub async fn export(db: &Db) -> anyhow::Result<Value> {
    // Une transaction de lecture : toutes les tables vues au même instant.
    let mut tx = db.read.begin().await?;
    let tables = db::export::dump(&mut tx).await?;
    tx.commit().await?;
    Ok(json!({
        "format": FORMAT,
        "schema_version": schema_version(),
        "app_version": env!("CARGO_PKG_VERSION"),
        "exported_at": timestamp(),
        "tables": tables,
    }))
}

/// Restaure un export dans une base neuve (migrations appliquées, aucune donnée).
pub async fn restore(db: &Db, export: &Value) -> anyhow::Result<()> {
    if export.get("format").and_then(Value::as_str) != Some(FORMAT) {
        bail!("ce fichier n'est pas un export Cashflow");
    }
    let version = export.get("schema_version").and_then(Value::as_i64).context("schema_version manquant")?;
    if version != schema_version() {
        bail!("export au schéma {version}, application au schéma {} : versions incompatibles", schema_version());
    }
    let tables = export.get("tables").and_then(Value::as_object).context("tables manquantes")?;
    let mut tx = db.write.begin().await?;
    // Les références internes (catégorie parente, version précédente d'une ligne de budget) peuvent
    // précéder leur cible dans l'ordre d'export : contrôle des clés étrangères à la validation.
    sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;
    db::export::restore(&mut tx, tables).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use cashflow_domain::accounts::AccountKind;
    use cashflow_domain::ledger::category::CategoryKind;

    use super::*;
    use crate::services::{accounts, categories, seed, transactions, transfers};

    async fn sample(db: &Db) {
        seed::run(db).await.unwrap();
        let today: cashflow_domain::Date = "2026-03-15".parse().unwrap();
        let account = |name: &str, kind| accounts::AccountInput {
            name: name.into(),
            kind,
            institution_id: None,
            currency: None,
            opening_date: "2026-01-01".parse().unwrap(),
            opening_balance_minor: 100_000,
            iban: Some("FR76 3000 6000 0112 3456 7890 189".into()),
            account_number: None,
            include_in_net_worth: None,
            include_in_emergency_fund: None,
            include_in_budget: None,
            display_order: None,
            color: None,
            notes: Some("note".into()),
        };
        let checking = accounts::create(db, &account("Courant", AccountKind::Checking), today).await.unwrap();
        let livret = accounts::create(db, &account("Livret A", AccountKind::LivretA), today).await.unwrap();
        let category = categories::create(
            db,
            &categories::CategoryInput {
                name: "Test".into(),
                kind: CategoryKind::Expense,
                parent_id: None,
                archived: false,
                display_order: None,
                color: None,
                icon: None,
            },
        )
        .await
        .unwrap();
        transactions::create(
            db,
            &transactions::TransactionInput {
                account_id: checking.id,
                date: "2026-03-10".parse().unwrap(),
                amount_minor: -12_000,
                label: "Courses".into(),
                note: None,
                category_id: None,
                splits: Some(vec![
                    transactions::SplitInput {
                        amount_minor: -9_500,
                        category_id: Some(category),
                        note: None,
                        project_id: None,
                    },
                    transactions::SplitInput { amount_minor: -2_500, category_id: None, note: None, project_id: None },
                ]),
            },
        )
        .await
        .unwrap();
        transfers::create(
            db,
            &transfers::TransferInput {
                from_account_id: checking.id,
                to_account_id: livret.id,
                amount_minor: 20_000,
                date: "2026-03-05".parse().unwrap(),
                label: "Épargne".into(),
                note: None,
            },
        )
        .await
        .unwrap();
    }

    /// Un export suivi d'une restauration restitue exactement les mêmes données (ADR-0008).
    #[tokio::test]
    async fn export_roundtrip() {
        let (source, _dir) = db::test_db().await;
        sample(&source).await;
        let first = export(&source).await.unwrap();

        let (target, _dir2) = db::test_db().await;
        restore(&target, &first).await.unwrap();
        let second = export(&target).await.unwrap();
        assert_eq!(first["tables"], second["tables"]);
        assert!(first["tables"]["ledger_transaction"].as_array().unwrap().len() >= 3);
    }

    #[tokio::test]
    async fn restore_refuses_non_empty_database() {
        let (source, _dir) = db::test_db().await;
        sample(&source).await;
        let data = export(&source).await.unwrap();
        assert!(restore(&source, &data).await.is_err());
    }
}
