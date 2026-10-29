//! Remboursements de frais (TRX-12, ADR-0026) : lien d'une ventilation d'entrée vers une dépense, remboursement
//! attendu, propositions et remboursements en attente.

use std::collections::HashMap;

use cashflow_domain::ledger::refund::{self, Candidate, RefundTarget};
use cashflow_domain::shared::ids::{AccountId, CategoryId, PayeeId, SplitId, TransactionId};
use cashflow_domain::{Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::transactions::SplitInput;
use crate::db::transactions::{RefundableRow, TransactionRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Une transaction de l'autre côté d'un lien de remboursement.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RefundLink {
    /// Ventilation qui rembourse (côté entrée d'argent).
    pub split_id: SplitId,
    pub transaction_id: TransactionId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    /// Montant de la dépense, vue depuis l'entrée ; montant remboursé, vue depuis la dépense.
    pub amount_minor: i64,
}

/// Remboursements d'une transaction (TRX-12).
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct RefundInfo {
    /// Remboursement attendu d'une dépense (R-TRX-53).
    pub expected_minor: Option<i64>,
    /// Remboursé de la dépense (R-TRX-52).
    pub refunded_minor: i64,
    /// Reste à recevoir (R-TRX-53).
    pub pending_minor: i64,
    /// Entrées qui remboursent cette dépense.
    pub received: Vec<RefundLink>,
    /// Dépenses que cette entrée rembourse, une par ventilation liée.
    pub refunds: Vec<RefundLink>,
}

/// Une dépense proposée pour une entrée d'argent (R-TRX-54).
#[derive(Debug, Serialize, ToSchema)]
pub struct RefundCandidate {
    pub transaction_id: TransactionId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub payee_id: Option<PayeeId>,
    pub expected_minor: Option<i64>,
    /// Remboursé par d'autres entrées.
    pub refunded_minor: i64,
    pub pending_minor: i64,
    /// Ce qui reste à rembourser de la dépense.
    pub refundable_minor: i64,
    /// Montant proposé.
    pub proposed_minor: i64,
    /// Catégorie proposée à la ventilation qui rembourse : celle de la dépense.
    pub category_id: Option<CategoryId>,
}

/// Une dépense dont le remboursement attendu n'est pas entièrement reçu (E1).
#[derive(Debug, Serialize, ToSchema)]
pub struct PendingRefund {
    pub transaction_id: TransactionId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub expected_minor: i64,
    pub refunded_minor: i64,
    pub pending_minor: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExpectedRefundInput {
    /// Absent : plus rien n'est attendu (« ne viendra pas »).
    pub expected_minor: Option<i64>,
}

async fn account_names(conn: &mut SqliteConnection) -> AppResult<HashMap<AccountId, String>> {
    Ok(db::accounts::list(&mut *conn).await?.into_iter().map(|a| (a.id, a.name)).collect())
}

/// Remboursements vus depuis la transaction `t`.
pub(super) async fn info(conn: &mut SqliteConnection, t: &TransactionRow) -> AppResult<RefundInfo> {
    let names = account_names(conn).await?;
    let name = |id: &AccountId| names.get(id).cloned().unwrap_or_default();
    let received: Vec<RefundLink> = db::transactions::refunds_towards(&mut *conn, t.id)
        .await?
        .into_iter()
        .map(|r| RefundLink {
            split_id: r.split_id,
            transaction_id: r.transaction_id,
            account_name: name(&r.account_id),
            date: r.date,
            label: r.label,
            amount_minor: r.amount.minor(),
        })
        .collect();
    let refunded: Money = received.iter().map(|r| Money::from_minor(r.amount_minor)).sum();
    let mut refunds = Vec::new();
    for split in db::transactions::splits_of(&mut *conn, t.id).await? {
        let Some(target) = split.refunds_transaction_id else { continue };
        if let Some(expense) = db::transactions::find(&mut *conn, target).await? {
            refunds.push(RefundLink {
                split_id: split.id,
                transaction_id: expense.id,
                account_name: name(&expense.account_id),
                date: expense.date,
                label: expense.label,
                amount_minor: expense.amount.minor(),
            });
        }
    }
    Ok(RefundInfo {
        expected_minor: t.expected_refund.map(Money::minor),
        refunded_minor: refunded.minor(),
        pending_minor: refund::pending(t.expected_refund, refunded).minor(),
        received,
        refunds,
    })
}

/// Vérifie les ventilations qui remboursent, pour la transaction `id` (R-TRX-50, R-TRX-52).
pub(super) async fn check_splits(
    conn: &mut SqliteConnection,
    id: TransactionId,
    splits: &[SplitInput],
) -> AppResult<()> {
    let mut by_target: HashMap<TransactionId, Money> = HashMap::new();
    for s in splits {
        let Some(target) = s.refunds_transaction_id else { continue };
        let expense =
            db::transactions::find(&mut *conn, target).await?.ok_or(AppError::NotFound("dépense remboursée"))?;
        let kind = match s.category_id {
            Some(c) => db::categories::find(&mut *conn, c).await?.map(|c| c.kind),
            None => None,
        };
        let is_transfer_leg = db::transactions::transfer_of(&mut *conn, target).await?.is_some();
        if s.loan_id.is_some() || s.asset_id.is_some() {
            return Err(DomainError::InvalidRefund.into());
        }
        refund::check_refund(
            Money::from_minor(s.amount_minor),
            kind,
            RefundTarget { amount: expense.amount, is_transfer_leg, is_same_transaction: target == id },
        )?;
        *by_target.entry(target).or_insert(Money::ZERO) += Money::from_minor(s.amount_minor);
    }
    for (target, amount) in by_target {
        let expense =
            db::transactions::find(&mut *conn, target).await?.ok_or(AppError::NotFound("dépense remboursée"))?;
        let others = db::transactions::refunded_except(&mut *conn, target, id).await?;
        refund::check_refunded(expense.amount, others + amount)?;
    }
    Ok(())
}

/// Une dépense qui change de montant garde un remboursé et un attendu au plus égaux à son montant
/// (R-TRX-52, R-TRX-53).
pub(super) async fn check_expense(
    conn: &mut SqliteConnection,
    current: &TransactionRow,
    amount: Money,
) -> AppResult<()> {
    let refunded: Money =
        db::transactions::refunds_towards(&mut *conn, current.id).await?.iter().map(|r| r.amount).sum();
    if refunded.is_positive() {
        if !amount.is_negative() {
            return Err(DomainError::RefundExceedsExpense.into());
        }
        refund::check_refunded(amount, refunded)?;
    }
    if let Some(expected) = current.expected_refund {
        refund::check_expected(amount, expected)?;
    }
    Ok(())
}

/// Fixe ou retire le remboursement attendu d'une dépense (R-TRX-53). Ne change aucun solde : permis sur une
/// transaction rapprochée (R-TRX-56).
pub async fn set_expected(db: &Db, id: TransactionId, input: &ExpectedRefundInput) -> AppResult<()> {
    let mut conn = db.write.acquire().await?;
    let t = db::transactions::find(&mut *conn, id).await?.ok_or(AppError::NotFound("transaction"))?;
    let expected = input.expected_minor.map(Money::from_minor);
    if let Some(expected) = expected {
        refund::check_expected(t.amount, expected)?;
    }
    db::transactions::set_expected_refund(&mut *conn, id, expected).await?;
    Ok(())
}

fn candidate(row: &RefundableRow) -> Candidate<TransactionId, PayeeId> {
    Candidate {
        id: row.id,
        date: row.date,
        amount: row.amount,
        payee: row.payee_id,
        expected: row.expected_refund,
        refunded: row.refunded,
    }
}

/// Nombre maximal de dépenses proposées : les premières dans l'ordre de R-TRX-54.
const MAX_CANDIDATES: usize = 50;

/// Dépenses proposées pour l'entrée d'argent `id` (R-TRX-54).
pub async fn candidates(db: &Db, id: TransactionId) -> AppResult<Vec<RefundCandidate>> {
    let mut conn = db.read.acquire().await?;
    let t = db::transactions::find(&mut *conn, id).await?.ok_or(AppError::NotFound("transaction"))?;
    if !t.amount.is_positive() {
        return Err(DomainError::InvalidRefund.into());
    }
    let from = t.date.checked_sub_days(chrono::Days::new(refund::CANDIDATE_DAYS)).unwrap_or(t.date);
    let rows = db::transactions::refundable(&mut *conn, from, t.date, id).await?;
    let by_id: HashMap<TransactionId, &RefundableRow> = rows.iter().map(|r| (r.id, r)).collect();
    // Ce que l'entrée n'affecte encore à aucune dépense.
    let linked: Money = db::transactions::splits_of(&mut *conn, id)
        .await?
        .iter()
        .filter(|s| s.refunds_transaction_id.is_some())
        .map(|s| s.amount)
        .sum();
    let names = account_names(&mut conn).await?;
    let proposals =
        refund::proposals(rows.iter().map(candidate).collect(), t.date, t.payee_id.as_ref(), t.amount - linked);
    Ok(proposals
        .into_iter()
        .take(MAX_CANDIDATES)
        .filter_map(|p| {
            let row = by_id.get(&p.candidate.id)?;
            Some(RefundCandidate {
                transaction_id: row.id,
                account_name: names.get(&row.account_id).cloned().unwrap_or_default(),
                date: row.date,
                label: row.label.clone(),
                amount_minor: row.amount.minor(),
                payee_id: row.payee_id,
                expected_minor: row.expected_refund.map(Money::minor),
                refunded_minor: row.refunded.minor(),
                pending_minor: p.candidate.pending().minor(),
                refundable_minor: p.candidate.refundable().minor(),
                proposed_minor: p.proposed.minor(),
                category_id: row.category_id,
            })
        })
        .collect())
}

/// Dépenses dont le remboursement attendu n'est pas entièrement reçu, les plus anciennes d'abord (R-TRX-53).
pub async fn pending(db: &Db) -> AppResult<Vec<PendingRefund>> {
    let mut conn = db.read.acquire().await?;
    let names = account_names(&mut conn).await?;
    Ok(db::transactions::pending_refunds(&mut *conn)
        .await?
        .into_iter()
        .filter_map(|r| {
            let expected = r.expected_refund?;
            Some(PendingRefund {
                transaction_id: r.id,
                account_name: names.get(&r.account_id).cloned().unwrap_or_default(),
                date: r.date,
                label: r.label,
                amount_minor: r.amount.minor(),
                expected_minor: expected.minor(),
                refunded_minor: r.refunded.minor(),
                pending_minor: refund::pending(Some(expected), r.refunded).minor(),
            })
        })
        .collect())
}
