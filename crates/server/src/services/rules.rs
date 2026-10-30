//! Règles de catégorisation (IMP-08, R-IMP-12, R-IMP-19, R-IMP-20).

use std::collections::HashMap;

use cashflow_domain::import::rules::{Actions, Conditions, Rule, Subject};
use cashflow_domain::ledger::split;
use cashflow_domain::shared::ids::{AccountId, CategorizationRuleId, TransactionId};
use cashflow_domain::{Date, Money};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

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

/// Transaction existante à laquelle la règle s'appliquerait (R-IMP-19, R-TIE-19).
#[derive(Debug, Serialize, ToSchema)]
pub struct RuleMatch {
    pub transaction_id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    /// Non catégorisée : la catégorie (et le reste des actions) changera.
    pub sets_category: bool,
    /// Sans tiers, pour une règle qui en affecte un : le tiers changera.
    pub sets_payee: bool,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PreviewQuery {
    /// La règle affecte un tiers : l'aperçu inclut aussi les transactions sans tiers, même catégorisées (R-TIE-19).
    pub sets_payee: Option<bool>,
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

/// Transactions auxquelles s'appliqueraient ces conditions (R-IMP-19) : non catégorisées (ventilation unique sans
/// catégorie, hors virements) et, pour une règle qui affecte un tiers, sans tiers (hors virements, R-TIE-19).
pub async fn preview(db: &Db, conditions: &Conditions, query: &PreviewQuery) -> AppResult<Vec<RuleMatch>> {
    conditions.check()?;
    let sets_payee = query.sets_payee.unwrap_or(false);
    let (items, _) = db::transactions::uncategorized(&db.read, i64::MAX).await?;
    let uncategorized: HashMap<TransactionId, ()> =
        items.into_iter().filter(|i| i.split_count == 1).map(|i| (i.transaction_id, ())).collect();
    let transfers: HashMap<TransactionId, ()> = db::transactions::all_transfers(&db.read)
        .await?
        .into_iter()
        .flat_map(|t| [(t.outflow_transaction_id, ()), (t.inflow_transaction_id, ())])
        .collect();
    let accounts: HashMap<AccountId, String> =
        db::accounts::list(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect();
    let mut result = Vec::new();
    for t in db::transactions::list_all(&db.read).await?.into_iter().rev() {
        let sets_category = uncategorized.contains_key(&t.id);
        let sets_payee = sets_payee && t.payee_id.is_none() && !transfers.contains_key(&t.id);
        if !sets_category && !sets_payee {
            continue;
        }
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
                account_name: accounts.get(&t.account_id).cloned().unwrap_or_default(),
                date: t.date,
                label: t.label,
                amount_minor: t.amount.minor(),
                sets_category,
                sets_payee,
            });
        }
    }
    Ok(result)
}

/// Applique une règle à des transactions existantes choisies dans l'aperçu (R-IMP-19). Une transaction déjà
/// catégorisée ne reçoit que le tiers ; un tiers existant n'est jamais remplacé (R-TIE-19).
pub async fn apply(db: &Db, id: CategorizationRuleId, input: &ApplyInput) -> AppResult<ApplyResult> {
    let rule = list(db).await?.into_iter().find(|r| r.id == id).ok_or(AppError::NotFound("règle"))?;
    let query = PreviewQuery { sets_payee: Some(rule.actions.set_payee.is_some()) };
    let eligible: HashMap<TransactionId, RuleMatch> =
        preview(db, &rule.conditions, &query).await?.into_iter().map(|m| (m.transaction_id, m)).collect();
    let mut applied = 0;
    for target in input.transaction_ids.iter().filter_map(|t| eligible.get(t)) {
        let transaction = &target.transaction_id;
        if target.sets_category
            && let Some(account) = rule.actions.mark_transfer_to
        {
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
        let payee = match (t.payee_id, rule.actions.set_payee) {
            (None, Some(payee)) => {
                super::transactions::check_payee(&mut tx, Some(payee)).await?;
                Some(payee)
            }
            (current, _) => current,
        };
        if !target.sets_category {
            db::transactions::set_payee(&mut *tx, t.id, payee).await?;
            db::rules::add_hit(&mut *tx, id).await?;
            tx.commit().await?;
            applied += 1;
            continue;
        }
        let proposer = match rule.actions.set_project {
            Some(project) => {
                super::budgets::check_active(&mut tx, project).await?;
                Some(super::budgets::LineProposer::load(&mut tx, project).await?)
            }
            None => None,
        };
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
            payee_id: payee,
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
                    project_id: rule.actions.set_project,
                    project_line_id: proposer.as_ref().and_then(|p| p.propose(s.category_id)),
                    ..Default::default()
                })
                .collect();
            db::transactions::replace_splits(&mut tx, t.id, &splits).await?;
        } else if let (Some(project), Some(proposer)) = (rule.actions.set_project, &proposer) {
            for split in db::transactions::splits_of(&mut *tx, t.id).await? {
                // Déjà dans ce projet, la ventilation garde son poste (R-BUD-27).
                let line = if split.project_id == Some(project) {
                    split.project_line_id
                } else {
                    proposer.propose(split.category_id)
                };
                db::budgets::set_split_project(&mut *tx, split.id, Some(project), line).await?;
            }
        }
        db::rules::add_hit(&mut *tx, id).await?;
        check_split_sums(&mut tx, &[t.id]).await?;
        tx.commit().await?;
        applied += 1;
    }
    Ok(ApplyResult { applied })
}
