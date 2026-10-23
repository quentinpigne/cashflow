//! Récurrences (TRX-07, R-TRX-40 à R-TRX-44) : échéancier, échéances honorées ou non, matérialisation.

use std::collections::HashMap;

use cashflow_domain::ledger::{ClearingStatus, TransactionOrigin, split};
use cashflow_domain::recurrence::{self, Candidate, Frequency, Occurrence, Schedule};
use cashflow_domain::shared::ids::{AccountId, PayeeId, RecurringRuleId, TransactionId};
use cashflow_domain::{Date, Money};
use chrono::Days;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{check_split_sums, reconciliation, required, transfers};
use crate::db::recurring::{CandidateRow, RuleRow, TemplateSplit};
use crate::db::transactions::{NewSplit, NewTransaction};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Jours d'échéances passées examinés pour signaler celles non honorées.
pub const LOOKBACK_DAYS: u64 = 60;

#[derive(Debug, Serialize, ToSchema)]
pub struct RecurringRuleView {
    pub id: RecurringRuleId,
    pub account_id: AccountId,
    pub target_account_id: Option<AccountId>,
    pub label: String,
    /// Montant attendu sur le compte, signé (négatif : dépense ou virement sortant).
    pub amount_minor: i64,
    pub amount_tolerance_minor: i64,
    pub frequency: Frequency,
    pub interval: u32,
    pub anchor_date: Date,
    pub end_date: Option<Date>,
    pub split_template: Vec<TemplateSplit>,
    pub active: bool,
    pub payee_id: Option<PayeeId>,
    pub next_date: Option<Date>,
    /// Échéances des 60 derniers jours non honorées (R-TRX-42).
    pub missed: Vec<Date>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RecurringRuleInput {
    pub account_id: AccountId,
    pub target_account_id: Option<AccountId>,
    pub label: String,
    pub amount_minor: i64,
    #[serde(default)]
    pub amount_tolerance_minor: i64,
    pub frequency: Frequency,
    #[serde(default = "one")]
    pub interval: u32,
    pub anchor_date: Date,
    pub end_date: Option<Date>,
    #[serde(default)]
    pub split_template: Vec<TemplateSplit>,
    #[serde(default = "yes")]
    pub active: bool,
    /// Tiers d'une dépense ou d'un revenu ; jamais sur un virement (R-TIE-13).
    #[serde(default)]
    pub payee_id: Option<PayeeId>,
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

/// État d'une échéance (R-TRX-42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OccurrenceStatus {
    Honored,
    Missed,
    Upcoming,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OccurrenceView {
    pub rule_id: RecurringRuleId,
    pub label: String,
    pub account_id: AccountId,
    pub target_account_id: Option<AccountId>,
    pub date: Date,
    pub amount_minor: i64,
    pub status: OccurrenceStatus,
    pub transaction_id: Option<TransactionId>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ScheduleQuery {
    /// Compte ; tous par défaut.
    pub account_id: Option<AccountId>,
    /// Jours à venir (90 par défaut).
    pub days: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MaterializeInput {
    /// Date de l'échéance à matérialiser.
    pub date: Date,
}

pub(crate) fn schedule(rule: &RuleRow) -> Schedule {
    Schedule { frequency: rule.frequency, interval: rule.interval, anchor: rule.anchor_date, end: rule.end_date }
}

/// Échéances d'une récurrence sur `[from, to]`, rapprochées des transactions réelles du compte.
pub(crate) fn occurrences(
    rule: &RuleRow,
    from: Date,
    to: Date,
    candidates: &[CandidateRow],
    today: Date,
) -> Vec<Occurrence<TransactionId>> {
    let dates = schedule(rule).occurrences(from, to);
    let own: Vec<Candidate<TransactionId>> = candidates
        .iter()
        .filter(|c| c.account_id == rule.account_id)
        .map(|c| Candidate {
            id: c.id,
            date: c.date,
            amount: c.amount,
            generated: c.recurring_rule_id == Some(rule.id),
        })
        .collect();
    recurrence::match_occurrences(&dates, rule.amount, rule.tolerance, &own, today)
}

fn view(rule: &RuleRow, candidates: &[CandidateRow], today: Date) -> RecurringRuleView {
    let from = today.checked_sub_days(Days::new(LOOKBACK_DAYS)).unwrap_or(today);
    let to = today.checked_add_days(Days::new(400)).unwrap_or(today);
    let list = if rule.active { occurrences(rule, from, to, candidates, today) } else { Vec::new() };
    RecurringRuleView {
        id: rule.id,
        account_id: rule.account_id,
        target_account_id: rule.target_account_id,
        label: rule.label.clone(),
        amount_minor: rule.amount.minor(),
        amount_tolerance_minor: rule.tolerance.minor(),
        frequency: rule.frequency,
        interval: rule.interval,
        anchor_date: rule.anchor_date,
        end_date: rule.end_date,
        split_template: rule.split_template.clone(),
        active: rule.active,
        payee_id: rule.payee_id,
        next_date: list.iter().find_map(|o| match o {
            Occurrence::Upcoming { date } => Some(*date),
            _ => None,
        }),
        missed: list
            .iter()
            .filter_map(|o| match o {
                Occurrence::Missed { date } => Some(*date),
                _ => None,
            })
            .collect(),
    }
}

async fn candidates_around(db: &Db, today: Date, ahead: u64) -> AppResult<Vec<CandidateRow>> {
    // Une transaction honore une échéance à ±5 jours : la fenêtre est élargie d'autant.
    let from = today.checked_sub_days(Days::new(LOOKBACK_DAYS + 6)).unwrap_or(today);
    let to = today.checked_add_days(Days::new(ahead + 6)).unwrap_or(today);
    Ok(db::recurring::candidates(&db.read, from, to).await?)
}

pub async fn list(db: &Db, today: Date) -> AppResult<Vec<RecurringRuleView>> {
    let rules = db::recurring::list(&db.read).await?;
    let candidates = candidates_around(db, today, 400).await?;
    Ok(rules.iter().map(|r| view(r, &candidates, today)).collect())
}

pub async fn save(
    db: &Db,
    id: Option<RecurringRuleId>,
    input: &RecurringRuleInput,
    today: Date,
) -> AppResult<RecurringRuleView> {
    let label = required(&input.label, "label", 200)?;
    let amount = Money::from_minor(input.amount_minor);
    if amount.is_zero() {
        return Err(cashflow_domain::DomainError::AmountZero.into());
    }
    if input.amount_tolerance_minor < 0 {
        return Err(AppError::invalid_field("INVALID_TOLERANCE", "amount_tolerance_minor", "écart toléré positif"));
    }
    if input.interval == 0 {
        return Err(AppError::invalid_field("INVALID_INTERVAL", "interval", "au moins 1"));
    }
    if input.end_date.is_some_and(|end| end < input.anchor_date) {
        return Err(AppError::invalid_field("END_BEFORE_START", "end_date", "la fin précède la première échéance"));
    }
    db::accounts::find(&db.read, input.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    if let Some(target) = input.target_account_id {
        db::accounts::find(&db.read, target).await?.ok_or(AppError::NotFound("compte d'arrivée"))?;
        if target == input.account_id {
            return Err(cashflow_domain::DomainError::TransferSameAccount.into());
        }
        // Un virement récurrent part du compte : son montant est une sortie (R-TRX-44).
        if !amount.is_negative() || !input.split_template.is_empty() {
            return Err(AppError::invalid_field(
                "INVALID_RECURRING_TRANSFER",
                "amount_minor",
                "un virement récurrent est une sortie du compte, sans ventilation",
            ));
        }
    }
    cashflow_domain::payees::check_transfer_leg(input.payee_id.is_some(), input.target_account_id.is_some())?;
    if let Some(payee) = input.payee_id {
        db::payees::find(&db.read, payee).await?.ok_or(AppError::NotFound("tiers"))?;
    }
    if !input.split_template.is_empty() {
        let amounts: Vec<Money> = input.split_template.iter().map(|s| Money::from_minor(s.amount_minor)).collect();
        split::check_splits(amount, &amounts)?;
    }
    let row = RuleRow {
        id: id.unwrap_or_else(RecurringRuleId::new),
        account_id: input.account_id,
        target_account_id: input.target_account_id,
        label,
        amount,
        tolerance: Money::from_minor(input.amount_tolerance_minor),
        frequency: input.frequency,
        interval: input.interval,
        anchor_date: input.anchor_date,
        end_date: input.end_date,
        split_template: input.split_template.clone(),
        active: input.active,
        payee_id: input.payee_id,
    };
    if let Some(id) = id
        && !db::recurring::list(&db.read).await?.iter().any(|r| r.id == id)
    {
        return Err(AppError::NotFound("récurrence"));
    }
    db::recurring::upsert(&db.write, &row).await?;
    let candidates = candidates_around(db, today, 400).await?;
    Ok(view(&row, &candidates, today))
}

pub async fn delete(db: &Db, id: RecurringRuleId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::recurring::delete(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Échéancier : échéances des 60 derniers jours et des `days` prochains, avec leur état (TRX-07).
pub async fn upcoming(db: &Db, query: &ScheduleQuery, today: Date) -> AppResult<Vec<OccurrenceView>> {
    let ahead = u64::from(query.days.unwrap_or(90));
    let rules = db::recurring::list(&db.read).await?;
    let candidates = candidates_around(db, today, ahead).await?;
    let from = today.checked_sub_days(Days::new(LOOKBACK_DAYS)).unwrap_or(today);
    let to = today.checked_add_days(Days::new(ahead)).unwrap_or(today);
    let mut result: Vec<OccurrenceView> = rules
        .iter()
        .filter(|r| r.active)
        .filter(|r| query.account_id.is_none_or(|a| r.account_id == a || r.target_account_id == Some(a)))
        .flat_map(|r| {
            occurrences(r, from, to, &candidates, today).into_iter().map(move |o| {
                let (status, transaction_id) = match o {
                    Occurrence::Honored { by, .. } => (OccurrenceStatus::Honored, Some(by)),
                    Occurrence::Missed { .. } => (OccurrenceStatus::Missed, None),
                    Occurrence::Upcoming { .. } => (OccurrenceStatus::Upcoming, None),
                };
                OccurrenceView {
                    rule_id: r.id,
                    label: r.label.clone(),
                    account_id: r.account_id,
                    target_account_id: r.target_account_id,
                    date: o.date(),
                    amount_minor: r.amount.minor(),
                    status,
                    transaction_id,
                }
            })
        })
        .collect();
    result.sort_by_key(|o| o.date);
    Ok(result)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Materialized {
    pub transaction_id: TransactionId,
}

/// Matérialise une échéance en transaction `PENDING` d'origine `RECURRING`, avec la ventilation du
/// modèle (R-TRX-43) ; un virement récurrent crée ses deux jambes (R-TRX-44).
pub async fn materialize(db: &Db, id: RecurringRuleId, input: &MaterializeInput) -> AppResult<Materialized> {
    let rule = db::recurring::list(&db.read)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(AppError::NotFound("récurrence"))?;
    if !schedule(&rule).occurrences(input.date, input.date).contains(&input.date) {
        return Err(AppError::invalid_field("NOT_AN_OCCURRENCE", "date", "aucune échéance à cette date"));
    }
    if let Some(target) = rule.target_account_id {
        let transfer = transfers::TransferInput {
            from_account_id: rule.account_id,
            to_account_id: target,
            amount_minor: -rule.amount.minor(),
            date: input.date,
            label: rule.label.clone(),
            note: None,
        };
        let created = transfers::create_from(db, &transfer, Some(rule.id)).await?;
        return Ok(Materialized { transaction_id: created.outflow_transaction_id });
    }
    let mut tx = db.write.begin().await?;
    let account = db::accounts::find(&mut *tx, rule.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    super::transactions::period(&account).check(input.date)?;
    reconciliation::lock(&mut tx, account.id).await?.check(input.date)?;
    let transaction = TransactionId::new();
    let new = NewTransaction {
        id: transaction,
        account_id: account.id,
        date: input.date,
        bank_date: None,
        value_date: None,
        amount: rule.amount,
        bank_label: None,
        label: &rule.label,
        note: None,
        clearing_status: ClearingStatus::Pending,
        origin: TransactionOrigin::Recurring,
        recurring_rule_id: Some(rule.id),
        fingerprint: None,
        payee_id: rule.payee_id,
    };
    let splits: Vec<NewSplit<'_>> = if rule.split_template.is_empty() {
        vec![NewSplit { amount: rule.amount, category_id: None, note: None, project_id: None, ..Default::default() }]
    } else {
        rule.split_template
            .iter()
            .map(|s| NewSplit {
                amount: Money::from_minor(s.amount_minor),
                category_id: s.category_id,
                note: None,
                project_id: None,
                ..Default::default()
            })
            .collect()
    };
    super::transactions::insert_validated(&mut tx, &new, &splits).await?;
    check_split_sums(&mut tx, &[transaction]).await?;
    tx.commit().await?;
    Ok(Materialized { transaction_id: transaction })
}

/// Transactions qui honorent une échéance, par compte : la dépense variable d'une projection les exclut
/// (R-PRJ-11).
pub(crate) async fn honoring(db: &Db, from: Date, to: Date, today: Date) -> AppResult<HashMap<TransactionId, ()>> {
    let rules = db::recurring::list(&db.read).await?;
    let candidates = db::recurring::candidates(
        &db.read,
        from.checked_sub_days(Days::new(6)).unwrap_or(from),
        to.checked_add_days(Days::new(6)).unwrap_or(to),
    )
    .await?;
    let mut result: HashMap<TransactionId, ()> =
        candidates.iter().filter(|c| c.recurring_rule_id.is_some()).map(|c| (c.id, ())).collect();
    for rule in &rules {
        for o in occurrences(rule, from, to, &candidates, today) {
            if let Occurrence::Honored { by, .. } = o {
                result.insert(by, ());
            }
        }
    }
    Ok(result)
}
