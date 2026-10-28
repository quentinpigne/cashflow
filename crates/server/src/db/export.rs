//! Export complet et restauration (ADR-0008). Format décrit dans `docs/export.md`.
//!
//! Les colonnes sont lues dynamiquement (`PRAGMA table_info`) pour que l'export suive le schéma sans
//! liste à maintenir : ce sont les seules requêtes non vérifiées à la compilation, couvertes par le
//! test d'aller-retour (`services::export`). Le SQL dynamique est sûr : noms de tables constants
//! (`TABLES`), noms de colonnes lus dans le schéma, valeurs toujours passées en paramètres.

use anyhow::{Context, bail};
use serde_json::{Map, Value};
use sqlx::{AssertSqlSafe, Row, SqliteConnection};

/// Tables exportées, dans un ordre compatible avec les clés étrangères.
///
/// Les sessions et l'utilisateur ne sont pas exportés : après une restauration, on redéfinit le mot
/// de passe avec `cashflow set-password`.
pub const TABLES: &[&str] = &[
    "app_setting",
    "institution",
    "category",
    "account",
    "rate_schedule",
    "rate_period",
    "fixed_savings_terms",
    "valuation",
    "asset",
    "asset_valuation",
    "loan",
    "loan_balance",
    "employment",
    "salary",
    "salary_bonus",
    "document",
    "payslip",
    "bank_statement",
    "payee_group",
    "payee",
    "recurring_rule",
    "project",
    "ledger_transaction",
    "split",
    "transfer",
    "loan_payment",
    "import_profile",
    "import_batch",
    "import_line",
    "categorization_rule",
    "balance_checkpoint",
    "projection_scenario",
    "budget_line",
    "budget_override",
    "notification",
];

async fn columns(conn: &mut SqliteConnection, table: &str) -> anyhow::Result<Vec<String>> {
    // `table_info` ne liste pas les colonnes générées (balance_date), qu'on ne réinsère pas.
    let rows = sqlx::query(AssertSqlSafe(format!("PRAGMA table_info({table})"))).fetch_all(&mut *conn).await?;
    rows.iter().map(|r| r.try_get::<String, _>("name").map_err(Into::into)).collect()
}

/// Toutes les lignes de toutes les tables exportées, par table.
pub async fn dump(conn: &mut SqliteConnection) -> anyhow::Result<Map<String, Value>> {
    let mut tables = Map::new();
    for table in TABLES {
        let columns = columns(conn, table).await?;
        let pairs = columns.iter().map(|c| format!("'{c}', \"{c}\"")).collect::<Vec<_>>().join(", ");
        // Ordre stable : les UUIDv7 suivent l'ordre de création ; `rowid` départage les autres tables.
        let sql = format!("SELECT json_object({pairs}) FROM \"{table}\" ORDER BY rowid");
        let rows: Vec<String> = sqlx::query_scalar(AssertSqlSafe(sql)).fetch_all(&mut *conn).await?;
        let rows = rows.iter().map(|r| serde_json::from_str(r)).collect::<Result<Vec<Value>, _>>()?;
        tables.insert((*table).to_owned(), Value::Array(rows));
    }
    Ok(tables)
}

/// Insère le contenu d'un export dans une base vide (migrations appliquées).
pub async fn restore(conn: &mut SqliteConnection, tables: &Map<String, Value>) -> anyhow::Result<()> {
    for table in TABLES {
        let count: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM \"{table}\"")))
            .fetch_one(&mut *conn)
            .await?;
        if count > 0 {
            bail!("la base n'est pas vide (table {table}) : la restauration se fait dans une base neuve");
        }
    }
    for table in TABLES {
        let Some(rows) = tables.get(*table) else { continue };
        let rows = rows.as_array().with_context(|| format!("table {table} : tableau attendu"))?;
        let known = columns(conn, table).await?;
        for row in rows {
            let object = row.as_object().with_context(|| format!("table {table} : objet attendu"))?;
            let names: Vec<&String> = object.keys().filter(|k| known.contains(k)).collect();
            if names.len() != object.len() {
                bail!("table {table} : colonne inconnue dans l'export");
            }
            let placeholders = vec!["?"; names.len()].join(", ");
            let list = names.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ");
            let sql = format!("INSERT INTO \"{table}\" ({list}) VALUES ({placeholders})");
            let mut query = sqlx::query(AssertSqlSafe(sql));
            for name in &names {
                query = match &object[*name] {
                    Value::Null => query.bind(None::<String>),
                    Value::String(s) => query.bind(s.clone()),
                    Value::Number(n) => {
                        query.bind(n.as_i64().with_context(|| format!("{table}.{name} : entier attendu"))?)
                    }
                    Value::Bool(b) => query.bind(i64::from(*b)),
                    other => bail!("{table}.{name} : valeur non scalaire {other}"),
                };
            }
            query.execute(&mut *conn).await.with_context(|| format!("insertion dans {table}"))?;
        }
    }
    Ok(())
}
