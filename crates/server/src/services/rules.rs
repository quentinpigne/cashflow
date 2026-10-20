//! Règles de catégorisation (IMP-08, R-IMP-12, R-IMP-19, R-IMP-20).

use std::collections::HashMap;

use cashflow_domain::import::rules::{Actions, Conditions, Rule, Subject};
use cashflow_domain::ledger::split;
use cashflow_domain::shared::ids::{AccountId, CategorizationRuleId, TransactionId};
use cashflow_domain::{Date, Money};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{check_split_sums, required, transfers};
use crate::db::rules::RuleRow;
use crate::db::transactions::{NewSplit, TransactionUpdate};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct RuleView {
    pub id: CategorizationRuleId,
    pub name: String,
    /// Plus petit = évaluée en premier.
    pub priority: i64,
    pub conditions: Conditions,
    pub actions: Actions,
    pub stop_processing: bool,
    pub active: bool,
    pub hit_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RuleInput {
    pub name: String,
    #[serde(default)]
    pub priority: i64,
    pub conditions: Conditions,
    pub actions: Actions,
    #[serde(default = "yes")]
    pub stop_processing: bool,
    #[serde(default = "yes")]
    pub active: bool,
}

fn yes() -> bool {
    true
}

/// Transaction existante non catégorisée à laquelle la règle s'appliquerait (R-IMP-19).
#[derive(Debug, Serialize, ToSchema)]
pub struct RuleMatch {
    pub transaction_id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplyInput {
    /// Transactions choisies dans l'aperçu.
    pub transaction_ids: Vec<TransactionId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplyResult {
    pub applied: usize,
}

fn view(row: RuleRow) -> AppResult<RuleView> {
    Ok(RuleView {
        id: row.id,
        name: row.name,
        priority: row.priority,
        conditions: serde_json::from_value(row.conditions).map_err(anyhow::Error::from)?,
        actions: serde_json::from_value(row.actions).map_err(anyhow::Error::from)?,
        stop_processing: row.stop_processing,
        active: row.active,
        hit_count: row.hit_count,
    })
}

pub async fn list(db: &Db) -> AppResult<Vec<RuleView>> {
    db::rules::list(&db.read).await?.into_iter().map(view).collect()
}

/// Règles actives, par priorité croissante, pour le moteur (R-IMP-12).
pub async fn active(db: &Db) -> AppResult<Vec<Rule<CategorizationRuleId>>> {
    Ok(list(db)
        .await?
        .into_iter()
        .filter(|r| r.active)
        .map(|r| Rule { id: r.id, conditions: r.conditions, actions: r.actions, stop_processing: r.stop_processing })
        .collect())
}

pub async fn save(db: &Db, id: Option<CategorizationRuleId>, input: &RuleInput) -> AppResult<RuleView> {
    let name = required(&input.name, "name", 120)?;
    input.conditions.check()?;
    let hit_count = match id {
        Some(id) => {
            db::rules::list(&db.read)
                .await?
                .into_iter()
                .find(|r| r.id == id)
                .ok_or(AppError::NotFound("règle"))?
                .hit_count
        }
        None => 0,
    };
    let row = RuleRow {
        id: id.unwrap_or_else(CategorizationRuleId::new),
        name,
        priority: input.priority,
        conditions: serde_json::to_value(&input.conditions).map_err(anyhow::Error::from)?,
        actions: serde_json::to_value(&input.actions).map_err(anyhow::Error::from)?,
        stop_processing: input.stop_processing,
        active: input.active,
        hit_count,
    };
    db::rules::upsert(&db.write, &row).await?;
    view(row)
}

pub async fn delete(db: &Db, id: CategorizationRuleId) -> AppResult<()> {
    db::rules::delete(&db.write, id).await?;
    Ok(())
}

/// Transactions non catégorisées (ventilation unique sans catégorie, hors virements) auxquelles
/// s'appliqueraient ces conditions (R-IMP-19).
pub async fn preview(db: &Db, conditions: &Conditions) -> AppResult<Vec<RuleMatch>> {
    conditions.check()?;
    let (items, _) = db::transactions::uncategorized(&db.read, i64::MAX).await?;
    let mut result = Vec::new();
    for item in items.into_iter().filter(|i| i.split_count == 1) {
        let t =
            db::transactions::find(&db.read, item.transaction_id).await?.ok_or(AppError::NotFound("transaction"))?;
        let subject = Subject {
            account: t.account_id,
            bank_label: t.bank_label.as_deref().unwrap_or_default(),
            label: &t.label,
            amount: t.amount,
        };
        if conditions.matches(&subject) {
            result.push(RuleMatch {
                transaction_id: t.id,
                account_id: t.account_id,
                account_name: item.account_name,
                date: t.date,
                label: t.label,
                amount_minor: t.amount.minor(),
            });
        }
    }
    Ok(result)
}

/// Applique une règle à des transactions existantes choisies dans l'aperçu (R-IMP-19).
pub async fn apply(db: &Db, id: CategorizationRuleId, input: &ApplyInput) -> AppResult<ApplyResult> {
    let rule = list(db).await?.into_iter().find(|r| r.id == id).ok_or(AppError::NotFound("règle"))?;
    let eligible: HashMap<TransactionId, ()> =
        preview(db, &rule.conditions).await?.into_iter().map(|m| (m.transaction_id, ())).collect();
    let mut applied = 0;
    for transaction in input.transaction_ids.iter().filter(|t| eligible.contains_key(t)) {
        if let Some(account) = rule.actions.mark_transfer_to {
            let mark = transfers::MarkAsTransferInput {
                counterpart_account_id: account,
                counterpart_transaction_id: None,
                confirm: true,
            };
            transfers::mark_as_transfer(db, *transaction, &mark).await?;
            applied += 1;
            continue;
        }
        let mut tx = db.write.begin().await?;
        let t = db::transactions::find(&mut *tx, *transaction).await?.ok_or(AppError::NotFound("transaction"))?;
        let label = rule.actions.set_label.clone().unwrap_or_else(|| t.label.clone());
        let note = rule.actions.set_note.clone().or_else(|| t.note.clone());
        let update = TransactionUpdate {
            account_id: t.account_id,
            date: t.date,
            bank_date: t.bank_date,
            value_date: t.value_date,
            amount: t.amount,
            label: &label,
            note: note.as_deref(),
            clearing_status: t.clearing_status,
        };
        db::transactions::update(&mut *tx, t.id, &update).await?;
        if !rule.actions.set_splits.is_empty() {
            let weights: Vec<u64> = rule.actions.set_splits.iter().map(|s| s.weight).collect();
            let amounts = split::allocate(t.amount, &weights)?;
            let splits: Vec<NewSplit<'_>> = rule
                .actions
                .set_splits
                .iter()
                .zip(amounts)
                .map(|(s, amount): (_, Money)| NewSplit {
                    amount,
                    category_id: s.category_id,
                    note: None,
                    project_id: None,
                })
                .collect();
            db::transactions::replace_splits(&mut tx, t.id, &splits).await?;
        }
        db::rules::add_hit(&mut *tx, id).await?;
        check_split_sums(&mut tx, &[t.id]).await?;
        tx.commit().await?;
        applied += 1;
    }
    Ok(ApplyResult { applied })
}
