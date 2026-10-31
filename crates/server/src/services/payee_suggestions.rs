//! Propositions de tiers à partir des libellés bancaires (TIE-05, R-TIE-30 à R-TIE-35).

use std::collections::HashSet;

use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp};
use cashflow_domain::payees::suggestion::{self, Assigned, NamedPayee};
use cashflow_domain::shared::ids::{CategorizationRuleId, PayeeId, TransactionId};
use cashflow_domain::{Date, DomainError};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::required;
use crate::db::payees::PayeeFields;
use crate::db::rules::RuleRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Proposition de tiers (R-TIE-31, R-TIE-32).
#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeSuggestionView {
    /// Clé de tiers (R-TIE-30) : identifie la proposition.
    pub key: String,
    /// Nom suggéré pour un tiers à créer.
    pub name: String,
    /// Tiers existant suggéré à la place d'une création.
    pub payee_id: Option<PayeeId>,
    pub payee_name: Option<String>,
    pub transaction_count: usize,
    pub first_date: Date,
    pub last_date: Date,
    /// Jusqu'à trois libellés bancaires distincts, les plus récents d'abord.
    pub labels: Vec<String>,
}

/// Clé de tiers ignorée (R-TIE-35).
#[derive(Debug, Serialize, ToSchema)]
pub struct IgnoredPayeeKeyView {
    pub key: String,
    pub ignored_at: String,
}

/// Propositions de tiers et clés ignorées (E43).
#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeSuggestionList {
    pub suggestions: Vec<PayeeSuggestionView>,
    pub ignored: Vec<IgnoredPayeeKeyView>,
}

/// Validation d'une proposition (R-TIE-33, R-TIE-34) : un tiers existant, ou un nom pour en créer un.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AcceptSuggestionInput {
    pub key: String,
    /// Tiers existant, non archivé. Absent : un tiers est créé sous `name`.
    pub payee_id: Option<PayeeId>,
    pub name: Option<String>,
    /// Crée aussi une règle qui affecte ce tiers aux imports suivants (R-TIE-34).
    pub create_rule: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AcceptSuggestionResult {
    pub payee_id: PayeeId,
    /// Transactions qui ont reçu le tiers.
    pub assigned: u64,
    pub rule_id: Option<CategorizationRuleId>,
}

/// Clé de tiers à ignorer ou à rétablir (R-TIE-35).
#[derive(Debug, Deserialize, ToSchema)]
pub struct PayeeKeyInput {
    pub key: String,
}

/// Propositions de tiers, recalculées à chaque consultation (R-TIE-31).
pub async fn list(db: &Db) -> AppResult<PayeeSuggestionList> {
    let unassigned = db::payees::unassigned(&db.read).await?;
    let assigned: Vec<Assigned<PayeeId>> = db::payees::labelled_payees(&db.read)
        .await?
        .into_iter()
        .map(|(bank_label, payee)| Assigned { bank_label, payee })
        .collect();
    let payees = db::payees::list(&db.read).await?;
    let named: Vec<NamedPayee<PayeeId>> =
        payees.iter().filter(|p| !p.archived).map(|p| NamedPayee { id: p.id, name: p.name.clone() }).collect();
    let ignored_rows = db::payees::ignored_keys(&db.read).await?;
    let ignored: HashSet<String> = ignored_rows.iter().map(|r| r.key.clone()).collect();
    let suggestions = suggestion::suggestions(&unassigned, &assigned, &named, &ignored)
        .into_iter()
        .map(|s| PayeeSuggestionView {
            payee_name: s.payee.and_then(|id| payees.iter().find(|p| p.id == id)).map(|p| p.name.clone()),
            payee_id: s.payee,
            transaction_count: s.transactions.len(),
            key: s.key,
            name: s.name,
            first_date: s.first_date,
            last_date: s.last_date,
            labels: s.labels,
        })
        .collect();
    let ignored =
        ignored_rows.into_iter().map(|r| IgnoredPayeeKeyView { key: r.key, ignored_at: r.ignored_at }).collect();
    Ok(PayeeSuggestionList { suggestions, ignored })
}

/// Valide une proposition : affecte le tiers choisi ou créé à toutes ses transactions sans tiers, et crée la règle
/// demandée, de façon atomique (R-TIE-33, R-TIE-34).
pub async fn accept(db: &Db, input: &AcceptSuggestionInput) -> AppResult<AcceptSuggestionResult> {
    let mut tx = db.write.begin().await?;
    let unassigned = db::payees::unassigned(&mut *tx).await?;
    let ids: Vec<TransactionId> = suggestion::with_key(&unassigned, &input.key);
    if ids.is_empty() {
        return Err(AppError::NotFound("proposition de tiers"));
    }
    let (payee_id, name) = match input.payee_id {
        Some(id) => {
            let payee = db::payees::find(&mut *tx, id).await?.ok_or(AppError::NotFound("tiers"))?;
            if payee.archived {
                return Err(DomainError::PayeeArchived.into());
            }
            (id, payee.name)
        }
        None => {
            let name = required(input.name.as_deref().unwrap_or_default(), "name", 80)?;
            if db::payees::name_taken(&mut *tx, &name, None).await? {
                return Err(DomainError::PayeeNameTaken.into());
            }
            let id = PayeeId::new();
            let fields = PayeeFields { name: &name, group_id: None, archived: false, note: None, address: None };
            db::payees::insert(&mut *tx, id, &fields).await?;
            (id, name)
        }
    };
    let assigned = db::payees::assign_unassigned(&mut tx, &ids, payee_id).await?;
    let rule_id = if input.create_rule { Some(create_rule(&mut tx, &input.key, payee_id, &name).await?) } else { None };
    tx.commit().await?;
    Ok(AcceptSuggestionResult { payee_id, assigned, rule_id })
}

/// Règle « libellé bancaire correspond à `*MOT1*MOT2*` → tiers », évaluée avant les autres et sans arrêt du
/// traitement : les règles suivantes complètent la catégorie (R-TIE-34, R-IMP-12).
async fn create_rule(
    tx: &mut sqlx::SqliteConnection,
    key: &str,
    payee: PayeeId,
    name: &str,
) -> AppResult<CategorizationRuleId> {
    let rules = db::rules::list(&mut *tx).await?;
    let priority = rules.iter().map(|r| r.priority).min().map_or(0, |p| p - 1);
    let conditions = Conditions {
        label: Some(LabelCondition {
            op: LabelOp::Regex,
            value: suggestion::rule_pattern(key),
            field: LabelField::BankLabel,
        }),
        ..Conditions::default()
    };
    let actions = Actions { set_payee: Some(payee), ..Actions::default() };
    let id = CategorizationRuleId::new();
    let row = RuleRow {
        id,
        name: format!("Tiers : {name}"),
        priority,
        conditions: serde_json::to_value(&conditions).map_err(anyhow::Error::from)?,
        actions: serde_json::to_value(&actions).map_err(anyhow::Error::from)?,
        stop_processing: false,
        active: true,
        hit_count: 0,
    };
    db::rules::upsert(&mut *tx, &row).await?;
    Ok(id)
}

/// Ignore une clé : elle n'est plus proposée (R-TIE-35).
pub async fn ignore(db: &Db, input: &PayeeKeyInput) -> AppResult<()> {
    let key = required(&input.key, "key", 200)?;
    db::payees::ignore_key(&db.write, &key).await?;
    Ok(())
}

/// Rétablit une clé ignorée (R-TIE-35).
pub async fn restore(db: &Db, input: &PayeeKeyInput) -> AppResult<()> {
    db::payees::restore_key(&db.write, input.key.trim()).await?;
    Ok(())
}
