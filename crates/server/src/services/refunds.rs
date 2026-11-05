//! Remboursements de frais (TRX-12, ADR-0026) : lien d'une ventilation d'entrée vers une dépense, remboursement
//! attendu, propositions et remboursements en attente.

use std::collections::HashMap;

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::ledger::refund::{self, Candidate, RefundTarget, Refunder};
use cashflow_domain::shared::ids::{AccountId, CategoryId, PayeeId, SplitId, TransactionId};
use cashflow_domain::{Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::{IntoParams, ToSchema};

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

/// Recherche et page des dépenses proposées (R-TRX-54).
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RefundCandidatesQuery {
    /// Texte du libellé ou du nom du tiers, ou montant de la dépense.
    pub q: Option<String>,
    /// Page demandée, à partir de 1.
    pub page: Option<usize>,
    /// Taille de la page : 10 par défaut, 100 au plus.
    pub limit: Option<usize>,
}

/// Une page des dépenses proposées, dans l'ordre de R-TRX-54.
#[derive(Debug, Serialize, ToSchema)]
pub struct RefundCandidatePage {
    pub items: Vec<RefundCandidate>,
    /// Nombre de dépenses qui correspondent à la recherche.
    pub total: usize,
    /// Page renvoyée, à partir de 1 : la dernière si la page demandée n'existe plus.
    pub page: usize,
    pub page_size: usize,
}

const DEFAULT_CANDIDATE_PAGE: usize = 10;
const MAX_CANDIDATE_PAGE: usize = 100;

/// Dépenses proposées pour l'entrée d'argent `id` : toutes, dans l'ordre de R-TRX-54, filtrées par la recherche
/// et découpées en pages.
pub async fn candidates(db: &Db, id: TransactionId, query: &RefundCandidatesQuery) -> AppResult<RefundCandidatePage> {
    let mut conn = db.read.acquire().await?;
    let t = db::transactions::find(&mut *conn, id).await?.ok_or(AppError::NotFound("transaction"))?;
    if !t.amount.is_positive() {
        return Err(DomainError::InvalidRefund.into());
    }
    let rows = db::transactions::refundable(&mut *conn, id).await?;
    let payees: HashMap<PayeeId, String> =
        db::payees::list(&mut *conn).await?.into_iter().map(|p| (p.id, p.name)).collect();
    let search = query.q.as_deref().unwrap_or_default();
    let rows: Vec<RefundableRow> = rows
        .into_iter()
        .filter(|r| {
            let payee = r.payee_id.and_then(|p| payees.get(&p)).map(String::as_str);
            refund::matches_search(search, &r.label, payee, r.amount)
        })
        .collect();
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
    let total = proposals.len();
    let page_size = query.limit.unwrap_or(DEFAULT_CANDIDATE_PAGE).clamp(1, MAX_CANDIDATE_PAGE);
    let last = total.div_ceil(page_size).max(1);
    let page = query.page.unwrap_or(1).clamp(1, last);
    let items = proposals
        .into_iter()
        .skip((page - 1) * page_size)
        .take(page_size)
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
        .collect();
    Ok(RefundCandidatePage { items, total, page, page_size })
}

/// Une entrée d'argent proposée pour rembourser une dépense (R-TRX-62).
#[derive(Debug, Serialize, ToSchema)]
pub struct RefunderCandidate {
    pub transaction_id: TransactionId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub payee_id: Option<PayeeId>,
    /// Ce que l'entrée n'affecte encore à aucune dépense.
    pub available_minor: i64,
    /// Montant proposé.
    pub proposed_minor: i64,
}

/// Une page des entrées proposées, dans l'ordre de R-TRX-62.
#[derive(Debug, Serialize, ToSchema)]
pub struct RefunderCandidatePage {
    pub items: Vec<RefunderCandidate>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    /// Catégorie de dépense que recevra la ventilation qui rembourse ; absente, la dépense est à catégoriser
    /// d'abord (R-TRX-50).
    pub category_id: Option<CategoryId>,
    /// Ce qui reste à rembourser de la dépense.
    pub refundable_minor: i64,
}

/// Entrées liées depuis la dépense, chacune avec le montant qu'elle rembourse (R-TRX-62).
#[derive(Debug, Deserialize, ToSchema)]
pub struct LinkRefundersInput {
    pub links: Vec<RefunderLinkInput>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RefunderLinkInput {
    /// Entrée d'argent qui rembourse.
    pub transaction_id: TransactionId,
    pub amount_minor: i64,
}

/// Découpe une liste ordonnée en pages.
fn paged<T>(items: Vec<T>, page: Option<usize>, limit: Option<usize>) -> (Vec<T>, usize, usize, usize) {
    let total = items.len();
    let page_size = limit.unwrap_or(DEFAULT_CANDIDATE_PAGE).clamp(1, MAX_CANDIDATE_PAGE);
    let last = total.div_ceil(page_size).max(1);
    let page = page.unwrap_or(1).clamp(1, last);
    let items = items.into_iter().skip((page - 1) * page_size).take(page_size).collect();
    (items, total, page, page_size)
}

/// Catégorie de dépense de la plus grosse ventilation d'une dépense : celle que reçoit la ventilation qui la
/// rembourse (R-TRX-50).
async fn expense_category(conn: &mut SqliteConnection, id: TransactionId) -> AppResult<Option<CategoryId>> {
    let mut splits = db::transactions::splits_of(&mut *conn, id).await?;
    splits.sort_by_key(|s| s.amount);
    for s in splits {
        if let Some(category) = s.category_id
            && db::categories::find(&mut *conn, category).await?.is_some_and(|c| c.kind == CategoryKind::Expense)
        {
            return Ok(Some(category));
        }
    }
    Ok(None)
}

/// Entrées d'argent proposées pour rembourser la dépense `id` (R-TRX-62) : toutes celles qui ont encore un montant
/// à affecter, filtrées par la recherche (libellé, tiers, montant) et découpées en pages.
pub async fn refunders(db: &Db, id: TransactionId, query: &RefundCandidatesQuery) -> AppResult<RefunderCandidatePage> {
    let mut conn = db.read.acquire().await?;
    let t = db::transactions::find(&mut *conn, id).await?.ok_or(AppError::NotFound("transaction"))?;
    if !t.amount.is_negative() || db::transactions::transfer_of(&mut *conn, id).await?.is_some() {
        return Err(DomainError::InvalidRefund.into());
    }
    let refunded: Money = db::transactions::refunds_towards(&mut *conn, id).await?.iter().map(|r| r.amount).sum();
    let expense =
        Candidate { id, date: t.date, amount: t.amount, payee: t.payee_id, expected: t.expected_refund, refunded };
    let wanted = if expense.pending().is_positive() { expense.pending() } else { expense.refundable() };
    let payees: HashMap<PayeeId, String> =
        db::payees::list(&mut *conn).await?.into_iter().map(|p| (p.id, p.name)).collect();
    let search = query.q.as_deref().unwrap_or_default();
    let rows: Vec<db::transactions::RefunderRow> = db::transactions::refunders(&mut *conn, id)
        .await?
        .into_iter()
        .filter(|r| {
            let payee = r.payee_id.and_then(|p| payees.get(&p)).map(String::as_str);
            refund::matches_search(search, &r.label, payee, r.amount)
        })
        .collect();
    let by_id: HashMap<TransactionId, &db::transactions::RefunderRow> = rows.iter().map(|r| (r.id, r)).collect();
    let refunders =
        rows.iter().map(|r| Refunder { id: r.id, date: r.date, payee: r.payee_id, available: r.available }).collect();
    let proposals = refund::refunder_proposals(refunders, t.date, t.payee_id.as_ref(), wanted);
    let names = account_names(&mut conn).await?;
    let (proposals, total, page, page_size) = paged(proposals, query.page, query.limit);
    let items = proposals
        .into_iter()
        .filter_map(|p| {
            let row = by_id.get(&p.refunder.id)?;
            Some(RefunderCandidate {
                transaction_id: row.id,
                account_name: names.get(&row.account_id).cloned().unwrap_or_default(),
                date: row.date,
                label: row.label.clone(),
                amount_minor: row.amount.minor(),
                payee_id: row.payee_id,
                available_minor: row.available.minor(),
                proposed_minor: p.proposed.minor(),
            })
        })
        .collect();
    Ok(RefunderCandidatePage {
        items,
        total,
        page,
        page_size,
        category_id: expense_category(&mut conn, id).await?,
        refundable_minor: expense.refundable().minor(),
    })
}

/// Lie des entrées d'argent à la dépense `id`, depuis la dépense (R-TRX-62) : chaque entrée reçoit une ventilation
/// qui rembourse la dépense, sur sa catégorie, prise sur ce qu'elle n'affectait encore à aucune dépense (les
/// dernières ventilations d'abord). Ne change aucun solde : permis sur des transactions rapprochées (R-TRX-56).
pub async fn link_refunders(db: &Db, id: TransactionId, input: &LinkRefundersInput) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let expense = db::transactions::find(&mut *tx, id).await?.ok_or(AppError::NotFound("transaction"))?;
    if !expense.amount.is_negative() {
        return Err(DomainError::InvalidRefund.into());
    }
    let category = expense_category(&mut tx, id).await?.ok_or(DomainError::InvalidRefund)?;
    for link in &input.links {
        let income =
            db::transactions::find(&mut *tx, link.transaction_id).await?.ok_or(AppError::NotFound("transaction"))?;
        if !income.amount.is_positive() || db::transactions::transfer_of(&mut *tx, income.id).await?.is_some() {
            return Err(DomainError::InvalidRefund.into());
        }
        let existing = db::transactions::splits_of(&mut *tx, income.id).await?;
        let free = |s: &db::transactions::SplitRow| {
            s.amount.is_positive() && s.refunds_transaction_id.is_none() && s.loan_id.is_none() && s.asset_id.is_none()
        };
        let free_amounts: Vec<Money> = existing.iter().filter(|s| free(s)).map(|s| s.amount).collect();
        let mut rest = refund::take_from(&free_amounts, Money::from_minor(link.amount_minor))?.into_iter();
        let member = super::transactions::kept_member(&mut tx, income.id, income.account_id).await?;
        let mut splits: Vec<SplitInput> = Vec::with_capacity(existing.len() + 1);
        for s in &existing {
            let amount = if free(s) { rest.next().unwrap_or(Money::ZERO) } else { s.amount };
            if amount.is_zero() {
                continue;
            }
            splits.push(SplitInput {
                amount_minor: amount.minor(),
                category_id: s.category_id,
                note: s.note.clone(),
                project_id: s.project_id,
                project_line_id: s.project_line_id,
                loan_id: s.loan_id,
                asset_id: s.asset_id,
                refunds_transaction_id: s.refunds_transaction_id,
                member_id: s.member_id,
            });
        }
        splits.push(SplitInput {
            amount_minor: link.amount_minor,
            category_id: Some(category),
            refunds_transaction_id: Some(id),
            member_id: member,
            ..Default::default()
        });
        cashflow_domain::ledger::split::check_splits(
            income.amount,
            &splits.iter().map(|s| Money::from_minor(s.amount_minor)).collect::<Vec<_>>(),
        )?;
        check_splits(&mut tx, income.id, &splits).await?;
        db::transactions::replace_splits(&mut tx, income.id, &super::transactions::new_splits(&splits)).await?;
    }
    let touched: Vec<TransactionId> = input.links.iter().map(|l| l.transaction_id).collect();
    super::check_split_sums(&mut tx, &touched).await?;
    tx.commit().await?;
    Ok(())
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
