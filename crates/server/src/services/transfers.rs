//! Virements internes (TRX-05, R-TRX-30 à R-TRX-36).

use cashflow_domain::ledger::transfer::{self, CandidateCheck, TransferLeg};
use cashflow_domain::ledger::{ClearingStatus, TransactionOrigin};
use cashflow_domain::shared::ids::{AccountId, TransactionId, TransferId};
use cashflow_domain::{Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::transactions::{insert_validated, period};
use super::{check_split_sums, clean, reconciliation, required};
use crate::db::transactions::{NewSplit, NewTransaction, TransactionRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Tolérance de date, en jours, pour proposer une transaction candidate (R-TRX-35, Q6).
pub const DEFAULT_TOLERANCE_DAYS: i64 = 5;
const TOLERANCE_SETTING: &str = "matching.date_tolerance_days";

#[derive(Debug, Deserialize, ToSchema)]
pub struct TransferInput {
    pub from_account_id: AccountId,
    pub to_account_id: AccountId,
    /// Montant transféré, positif.
    pub amount_minor: i64,
    pub date: Date,
    pub label: String,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransferCreated {
    pub transfer_id: TransferId,
    pub outflow_transaction_id: TransactionId,
    pub inflow_transaction_id: TransactionId,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MarkAsTransferInput {
    /// Compte de contrepartie, suivi dans l'application.
    pub counterpart_account_id: AccountId,
    /// Transaction existante à lier ; sans elle, la jambe opposée est créée (R-TRX-36).
    pub counterpart_transaction_id: Option<TransactionId>,
    /// Confirme le remplacement d'une ventilation catégorisée (R-TRX-33).
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CandidatesQuery {
    pub account_id: AccountId,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Candidate {
    pub id: TransactionId,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub clearing_status: ClearingStatus,
}

impl From<TransactionRow> for Candidate {
    fn from(t: TransactionRow) -> Self {
        Candidate {
            id: t.id,
            date: t.date,
            label: t.label,
            amount_minor: t.amount.minor(),
            clearing_status: t.clearing_status,
        }
    }
}

async fn tolerance_days(db: &Db) -> AppResult<i64> {
    Ok(db::settings::get(&db.read, TOLERANCE_SETTING).await?.unwrap_or(DEFAULT_TOLERANCE_DAYS))
}

fn uncategorized(amount: Money) -> [NewSplit<'static>; 1] {
    [NewSplit { amount, category_id: None, note: None, project_id: None, ..Default::default() }]
}

/// Crée les deux jambes d'un virement saisi (R-TRX-33).
pub async fn create(db: &Db, input: &TransferInput) -> AppResult<TransferCreated> {
    create_from(db, input, None).await
}

/// Crée un virement, généré par une récurrence le cas échéant (origine `RECURRING`, R-TRX-43, R-TRX-44).
pub async fn create_from(
    db: &Db,
    input: &TransferInput,
    rule: Option<cashflow_domain::shared::ids::RecurringRuleId>,
) -> AppResult<TransferCreated> {
    if input.amount_minor <= 0 {
        return Err(AppError::invalid_field("AMOUNT_NOT_POSITIVE", "amount_minor", "le montant doit être positif"));
    }
    let label = required(&input.label, "label", 200)?;
    let note = clean(input.note.as_deref());
    let amount = Money::from_minor(input.amount_minor);
    let mut tx = db.write.begin().await?;
    let from =
        db::accounts::find(&mut *tx, input.from_account_id).await?.ok_or(AppError::NotFound("compte de départ"))?;
    let to = db::accounts::find(&mut *tx, input.to_account_id).await?.ok_or(AppError::NotFound("compte d'arrivée"))?;
    let leg = |account: AccountId, amount: Money, currency| TransferLeg {
        account_id: account,
        currency,
        amount,
        balance_date: input.date,
        split_categories: vec![None],
    };
    transfer::check_transfer(&leg(from.id, -amount, from.currency), &leg(to.id, amount, to.currency))?;
    period(&from).check(input.date)?;
    period(&to).check(input.date)?;
    reconciliation::lock(&mut tx, from.id).await?.check(input.date)?;
    reconciliation::lock(&mut tx, to.id).await?.check(input.date)?;

    let (outflow, inflow) = (TransactionId::new(), TransactionId::new());
    for (id, account, signed) in [(outflow, from.id, -amount), (inflow, to.id, amount)] {
        let new = NewTransaction {
            id,
            account_id: account,
            date: input.date,
            bank_date: None,
            value_date: None,
            amount: signed,
            bank_label: None,
            label: &label,
            note: note.as_deref(),
            clearing_status: ClearingStatus::Pending,
            origin: if rule.is_some() { TransactionOrigin::Recurring } else { TransactionOrigin::Manual },
            recurring_rule_id: rule,
            fingerprint: None,
        };
        insert_validated(&mut tx, &new, &uncategorized(signed)).await?;
    }
    let transfer_id = db::transactions::insert_transfer_between(&mut tx, outflow, inflow).await?;
    check_split_sums(&mut tx, &[outflow, inflow]).await?;
    tx.commit().await?;
    Ok(TransferCreated { transfer_id, outflow_transaction_id: outflow, inflow_transaction_id: inflow })
}

/// Transactions candidates sur `account_id` pour former un virement avec `id` (R-TRX-35).
pub async fn candidates(db: &Db, id: TransactionId, query: &CandidatesQuery) -> AppResult<Vec<Candidate>> {
    let source = db::transactions::find(&db.read, id).await?.ok_or(AppError::NotFound("transaction"))?;
    let tolerance = tolerance_days(db).await?;
    let from = source.balance_date - chrono::Days::new(tolerance.unsigned_abs());
    let to = source.balance_date + chrono::Days::new(tolerance.unsigned_abs());
    let rows = db::transactions::transfer_candidates(&db.read, query.account_id, source.amount, from, to).await?;
    Ok(rows
        .into_iter()
        .filter(|c| c.id != source.id)
        .filter(|c| {
            CandidateCheck {
                source_amount: source.amount,
                source_date: source.balance_date,
                candidate_amount: c.amount,
                candidate_date: c.balance_date,
                tolerance_days: tolerance,
            }
            .matches()
        })
        .map(Into::into)
        .collect())
}

/// Marque une transaction comme virement vers ou depuis un compte suivi (R-TRX-33, R-TRX-36).
pub async fn mark_as_transfer(db: &Db, id: TransactionId, input: &MarkAsTransferInput) -> AppResult<TransferId> {
    let mut tx = db.write.begin().await?;
    let source = db::transactions::find(&mut *tx, id).await?.ok_or(AppError::NotFound("transaction"))?;
    if db::transactions::transfer_of(&mut *tx, id).await?.is_some() {
        return Err(DomainError::AlreadyTransferLeg.into());
    }
    let source_account = db::accounts::find(&mut *tx, source.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    let counterpart_account =
        db::accounts::find(&mut *tx, input.counterpart_account_id).await?.ok_or(AppError::NotFound("compte"))?;

    let counterpart = match input.counterpart_transaction_id {
        Some(candidate) => {
            let other = db::transactions::find(&mut *tx, candidate).await?.ok_or(AppError::NotFound("transaction"))?;
            if other.account_id != counterpart_account.id {
                return Err(AppError::invalid("CANDIDATE_WRONG_ACCOUNT", "la transaction n'est pas sur ce compte"));
            }
            if db::transactions::transfer_of(&mut *tx, candidate).await?.is_some() {
                return Err(DomainError::AlreadyTransferLeg.into());
            }
            other
        }
        None => {
            // Jambe opposée créée automatiquement (R-TRX-36), hors période rapprochée (R-RAP-7).
            period(&counterpart_account).check(source.date)?;
            reconciliation::lock(&mut tx, counterpart_account.id).await?.check(source.date)?;
            let created = TransactionId::new();
            let new = NewTransaction {
                id: created,
                account_id: counterpart_account.id,
                date: source.date,
                bank_date: None,
                value_date: None,
                amount: -source.amount,
                bank_label: None,
                label: &source.label,
                note: None,
                clearing_status: transfer::opposite_leg_status(counterpart_account.kind.family()),
                origin: TransactionOrigin::Manual,
                recurring_rule_id: None,
                fingerprint: None,
            };
            insert_validated(&mut tx, &new, &uncategorized(-source.amount)).await?;
            db::transactions::find(&mut *tx, created).await?.ok_or(AppError::NotFound("transaction"))?
        }
    };

    // Les ventilations catégorisées sont remplacées, après confirmation (R-TRX-33).
    let mut categorized = false;
    for t in [&source, &counterpart] {
        let splits = db::transactions::splits_of(&mut *tx, t.id).await?;
        categorized |= splits.len() != 1 || splits.iter().any(|s| s.category_id.is_some());
    }
    if categorized && !input.confirm {
        return Err(AppError::invalid(
            "TRANSFER_REPLACES_CATEGORIES",
            "les ventilations catégorisées seront remplacées par une ventilation sans catégorie : confirmer",
        ));
    }
    for t in [&source, &counterpart] {
        db::transactions::replace_splits(&mut tx, t.id, &uncategorized(t.amount)).await?;
    }
    let (outflow, inflow) = if source.amount.is_negative() { (&source, &counterpart) } else { (&counterpart, &source) };
    let leg = |t: &TransactionRow, currency| TransferLeg {
        account_id: t.account_id,
        currency,
        amount: t.amount,
        balance_date: t.balance_date,
        split_categories: vec![None],
    };
    let (outflow_currency, inflow_currency) = if source.amount.is_negative() {
        (source_account.currency, counterpart_account.currency)
    } else {
        (counterpart_account.currency, source_account.currency)
    };
    transfer::check_transfer(&leg(outflow, outflow_currency), &leg(inflow, inflow_currency))?;
    let transfer_id = db::transactions::insert_transfer_between(&mut tx, outflow.id, inflow.id).await?;
    check_split_sums(&mut tx, &[source.id, counterpart.id]).await?;
    tx.commit().await?;
    Ok(transfer_id)
}

/// Délie un virement : les deux transactions restent, sans catégorie (R-TRX-34).
pub async fn unlink(db: &Db, id: TransferId) -> AppResult<()> {
    db::transactions::delete_transfer(&db.write, id).await?;
    Ok(())
}
