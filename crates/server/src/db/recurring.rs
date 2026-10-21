//! Récurrences (spec 02, TRX-07).

use cashflow_domain::recurrence::Frequency;
use cashflow_domain::shared::ids::{AccountId, CategoryId, RecurringRuleId, TransactionId};
use cashflow_domain::{Date, Money};
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqliteExecutor};
use utoipa::ToSchema;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

/// Une part du modèle de ventilation d'une récurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TemplateSplit {
    pub category_id: Option<CategoryId>,
    pub amount_minor: i64,
}

#[derive(Debug, Clone)]
pub struct RuleRow {
    pub id: RecurringRuleId,
    pub account_id: AccountId,
    pub target_account_id: Option<AccountId>,
    pub label: String,
    /// Montant attendu sur `account_id`, signé (négatif pour une dépense ou un virement sortant).
    pub amount: Money,
    pub tolerance: Money,
    pub frequency: Frequency,
    pub interval: u32,
    pub anchor_date: Date,
    pub end_date: Option<Date>,
    pub split_template: Vec<TemplateSplit>,
    pub active: bool,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<RuleRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, account_id, target_account_id, label, amount, amount_tolerance, frequency, interval,
                  anchor_date AS "anchor_date: Date", end_date AS "end_date: Date", split_template,
                  active AS "active: bool"
           FROM recurring_rule ORDER BY label COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(RuleRow {
                id: id_from_db(&r.id)?,
                account_id: id_from_db(&r.account_id)?,
                target_account_id: opt_id_from_db(r.target_account_id.as_deref())?,
                label: r.label,
                amount: Money::from_minor(r.amount),
                tolerance: Money::from_minor(r.amount_tolerance),
                frequency: enum_from_db(&r.frequency)?,
                interval: u32::try_from(r.interval)?,
                anchor_date: r.anchor_date,
                end_date: r.end_date,
                split_template: serde_json::from_str(&r.split_template)?,
                active: r.active,
            })
        })
        .collect()
}

pub async fn upsert(db: impl SqliteExecutor<'_>, r: &RuleRow) -> anyhow::Result<()> {
    let id = r.id.to_string();
    let account = r.account_id.to_string();
    let target = r.target_account_id.map(|a| a.to_string());
    let amount = r.amount.minor();
    let tolerance = r.tolerance.minor();
    let frequency = enum_to_db(&r.frequency);
    let template = serde_json::to_string(&r.split_template)?;
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO recurring_rule (id, account_id, target_account_id, label, amount, amount_tolerance, frequency, interval,
                                     anchor_date, end_date, split_template, active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
         ON CONFLICT (id) DO UPDATE SET account_id = excluded.account_id, target_account_id = excluded.target_account_id,
                                        label = excluded.label, amount = excluded.amount,
                                        amount_tolerance = excluded.amount_tolerance, frequency = excluded.frequency,
                                        interval = excluded.interval, anchor_date = excluded.anchor_date,
                                        end_date = excluded.end_date, split_template = excluded.split_template,
                                        active = excluded.active, updated_at = excluded.updated_at",
        id,
        account,
        target,
        r.label,
        amount,
        tolerance,
        frequency,
        r.interval,
        r.anchor_date,
        r.end_date,
        template,
        r.active,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Supprime une récurrence ; les transactions qu'elle a générées restent, sans lien.
pub async fn delete(conn: &mut SqliteConnection, id: RecurringRuleId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("UPDATE ledger_transaction SET recurring_rule_id = NULL WHERE recurring_rule_id = ?", id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("DELETE FROM recurring_rule WHERE id = ?", id).execute(&mut *conn).await?;
    Ok(())
}

/// Transaction réelle candidate pour honorer une échéance (R-TRX-42).
#[derive(Debug, Clone)]
pub struct CandidateRow {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub date: Date,
    pub amount: Money,
    pub recurring_rule_id: Option<RecurringRuleId>,
}

/// Transactions de comptes donnés, de date de solde dans `[from, to]`.
pub async fn candidates(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<CandidateRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, account_id, balance_date AS "date!: Date", amount AS "amount!: i64", recurring_rule_id
           FROM ledger_transaction WHERE balance_date BETWEEN ? AND ? ORDER BY balance_date"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(CandidateRow {
                id: id_from_db(&r.id)?,
                account_id: id_from_db(&r.account_id)?,
                date: r.date,
                amount: Money::from_minor(r.amount),
                recurring_rule_id: opt_id_from_db(r.recurring_rule_id.as_deref())?,
            })
        })
        .collect()
}
