//! Points de contrôle et rapprochement (spec 04 : RAP-01 à RAP-07).

use std::collections::HashMap;

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::ledger::ClearingStatus;
use cashflow_domain::reconciliation::{self, CheckpointState, GapTransaction, Lock};
use cashflow_domain::shared::ids::{AccountId, CheckpointId, ImportBatchId, TransactionId};
use cashflow_domain::{Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::accounts::Ledger;
use crate::db::checkpoints::CheckpointRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Ancienneté au-delà de laquelle une transaction `PENDING` est signalée (R-TDB-10).
const OLD_PENDING_DAYS: u64 = 10;

#[derive(Debug, Serialize, ToSchema)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub account_id: AccountId,
    pub date: Date,
    pub balance_minor: i64,
    /// Solde calculé au soir de `date` (R-RAP-1).
    pub computed_minor: i64,
    /// Écart = solde annoncé − solde calculé, jamais stocké (R-RAP-3).
    pub gap_minor: i64,
    /// `STATEMENT` (relevé importé) ou `MANUAL`.
    pub source: String,
    pub import_batch_id: Option<ImportBatchId>,
    pub reconciled: bool,
    /// Non rapproché mais antérieur à un point rapproché : son écart est figé (R-RAP-4).
    pub frozen: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CheckpointInput {
    pub date: Date,
    pub balance_minor: i64,
    /// Remplace un point de contrôle existant à la même date (R-RAP-2).
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GapTransactionView {
    pub id: TransactionId,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub clearing_status: ClearingStatus,
}

/// Ce que valider le rapprochement va faire (R-RAP-4, R-RAP-5), affiché avant validation.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReconcilePreview {
    pub checkpoint: Checkpoint,
    /// Points antérieurs dont l'écart sera figé.
    pub frozen: Vec<Checkpoint>,
    /// Transactions encore `PENDING` qui seront rapprochées : souvent suspectes.
    pub pending: Vec<GapTransactionView>,
}

/// Pistes pour expliquer un écart (R-RAP-9).
#[derive(Debug, Serialize, ToSchema)]
pub struct GapHintsView {
    pub gap_minor: i64,
    pub from: Option<Date>,
    pub to: Date,
    pub pending: Vec<GapTransactionView>,
    pub singles: Vec<GapTransactionView>,
    pub pairs: Vec<[GapTransactionView; 2]>,
    pub sign_errors: Vec<GapTransactionView>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ClearInput {
    pub cleared: bool,
}

/// Santé d'un compte (RAP-06, R-TDB-10).
#[derive(Debug, Serialize, ToSchema)]
pub struct AccountHealth {
    pub account_id: AccountId,
    pub account_name: String,
    pub last_reconciled: Option<Date>,
    /// Écart du point de contrôle le plus récent, s'il n'est pas rapproché.
    pub current_gap_minor: Option<i64>,
    pub current_gap_date: Option<Date>,
    /// Transactions `PENDING` de plus de 10 jours.
    pub old_pending: i64,
}

/// Période verrouillée d'un compte (R-RAP-7).
pub async fn lock(conn: &mut SqliteConnection, account: AccountId) -> AppResult<Lock> {
    Ok(Lock { until: db::checkpoints::locked_until(&mut *conn, account).await? })
}

fn views(rows: Vec<CheckpointRow>, account: &db::accounts::AccountRow, ledger: &Ledger) -> Vec<Checkpoint> {
    let locked = rows.iter().filter(|r| r.reconciled_at.is_some()).map(|r| r.date).max();
    rows.into_iter()
        .map(|r| {
            let (computed, _) = ledger.value(account, r.date);
            let reconciled = r.reconciled_at.is_some();
            Checkpoint {
                id: r.id,
                account_id: r.account_id,
                date: r.date,
                balance_minor: r.balance.minor(),
                computed_minor: computed.minor(),
                gap_minor: reconciliation::gap(r.balance, computed).minor(),
                source: r.source,
                import_batch_id: r.import_batch_id,
                reconciled,
                frozen: !reconciled && locked.is_some_and(|l| r.date < l),
            }
        })
        .collect()
}

async fn account_of(db: &Db, account: AccountId) -> AppResult<db::accounts::AccountRow> {
    db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))
}

pub async fn list(db: &Db, account: AccountId) -> AppResult<Vec<Checkpoint>> {
    let row = account_of(db, account).await?;
    let ledger = Ledger::load(db).await?;
    Ok(views(db::checkpoints::list_for_account(&db.read, account).await?, &row, &ledger))
}

async fn get(db: &Db, id: CheckpointId) -> AppResult<Checkpoint> {
    let point = db::checkpoints::find(&db.read, id).await?.ok_or(AppError::NotFound("point de contrôle"))?;
    list(db, point.account_id).await?.into_iter().find(|c| c.id == id).ok_or(AppError::NotFound("point de contrôle"))
}

/// Crée (ou remplace, après confirmation) un point de contrôle, manuel ou issu d'un relevé (RAP-01, R-RAP-2).
pub async fn upsert(
    conn: &mut SqliteConnection,
    account: &db::accounts::AccountRow,
    date: Date,
    balance: Money,
    source: &str,
    batch: Option<ImportBatchId>,
    replace: bool,
) -> AppResult<CheckpointId> {
    // Épargne variable et prêts se suivent par valorisations (R-RAP-10).
    if !account.kind.family().has_computed_balance() {
        return Err(DomainError::NoCheckpointForFamily.into());
    }
    match db::checkpoints::find_at(&mut *conn, account.id, date).await? {
        Some(existing) if existing.balance == balance && existing.source == source => Ok(existing.id),
        Some(existing) => {
            if existing.reconciled_at.is_some() {
                return Err(DomainError::CheckpointAlreadyReconciled.into());
            }
            if !replace {
                return Err(AppError::invalid(
                    "CHECKPOINT_EXISTS",
                    format!(
                        "un point de contrôle existe déjà au {date} avec un autre solde : confirmer le remplacement"
                    ),
                ));
            }
            db::checkpoints::update_balance(&mut *conn, existing.id, balance, source, batch).await?;
            Ok(existing.id)
        }
        None => Ok(db::checkpoints::insert(&mut *conn, account.id, date, balance, source, batch).await?),
    }
}

pub async fn create(db: &Db, account: AccountId, input: &CheckpointInput) -> AppResult<Checkpoint> {
    let row = account_of(db, account).await?;
    if input.date < row.opening_date {
        return Err(
            DomainError::DateOutsideAccountPeriod { opening: row.opening_date, closed_on: row.closed_on }.into()
        );
    }
    let mut tx = db.write.begin().await?;
    let id = upsert(&mut tx, &row, input.date, Money::from_minor(input.balance_minor), "MANUAL", None, input.replace)
        .await?;
    tx.commit().await?;
    get(db, id).await
}

pub async fn delete(db: &Db, id: CheckpointId) -> AppResult<()> {
    let point = db::checkpoints::find(&db.read, id).await?.ok_or(AppError::NotFound("point de contrôle"))?;
    if point.reconciled_at.is_some() {
        return Err(DomainError::CheckpointAlreadyReconciled.into());
    }
    db::checkpoints::delete(&db.write, id).await?;
    Ok(())
}

fn states(points: &[Checkpoint]) -> Vec<CheckpointState<CheckpointId>> {
    points
        .iter()
        .map(|c| CheckpointState {
            id: c.id,
            date: c.date,
            gap: Money::from_minor(c.gap_minor),
            reconciled: c.reconciled,
        })
        .collect()
}

async fn transactions_view(
    db: &Db,
    account: AccountId,
    from: Option<Date>,
    to: Date,
) -> AppResult<Vec<GapTransactionView>> {
    Ok(db::transactions::list_for_account(&db.read, account)
        .await?
        .into_iter()
        .filter(|t| from.is_none_or(|f| t.balance_date > f) && t.balance_date <= to)
        .map(|t| GapTransactionView {
            id: t.id,
            date: t.balance_date,
            label: t.label,
            amount_minor: t.amount.minor(),
            clearing_status: t.clearing_status,
        })
        .collect())
}

pub async fn reconcile_preview(db: &Db, id: CheckpointId) -> AppResult<ReconcilePreview> {
    let point = get(db, id).await?;
    let points = list(db, point.account_id).await?;
    let plan = reconciliation::plan_reconcile(&states(&points), id)?;
    let locked = points.iter().filter(|c| c.reconciled).map(|c| c.date).max();
    let pending = transactions_view(db, point.account_id, locked, point.date)
        .await?
        .into_iter()
        .filter(|t| t.clearing_status == ClearingStatus::Pending)
        .collect();
    let frozen = points.into_iter().filter(|c| plan.frozen.contains(&c.id)).collect();
    Ok(ReconcilePreview { checkpoint: point, frozen, pending })
}

/// Rapproche un point à écart nul : il et les points antérieurs à écart nul sont validés, toutes les
/// transactions de date de solde ≤ D passent en `RECONCILED` (R-RAP-4, R-RAP-5).
pub async fn reconcile(db: &Db, id: CheckpointId) -> AppResult<()> {
    let point = get(db, id).await?;
    let points = list(db, point.account_id).await?;
    let plan = reconciliation::plan_reconcile(&states(&points), id)?;
    let mut tx = db.write.begin().await?;
    for checkpoint in &plan.reconcile {
        db::checkpoints::set_reconciled(&mut *tx, *checkpoint, true).await?;
    }
    db::checkpoints::reconcile_transactions(&mut *tx, point.account_id, point.date).await?;
    tx.commit().await?;
    Ok(())
}

/// Déverrouille un point rapproché et tous les points postérieurs (R-RAP-6).
pub async fn unlock(db: &Db, id: CheckpointId) -> AppResult<()> {
    let point = get(db, id).await?;
    if !point.reconciled {
        return Err(AppError::invalid("CHECKPOINT_NOT_RECONCILED", "ce point de contrôle n'est pas rapproché"));
    }
    let points = list(db, point.account_id).await?;
    let unlocked = reconciliation::plan_unlock(&states(&points), point.date);
    let still_locked = points.iter().filter(|c| c.reconciled && !unlocked.contains(&c.id)).map(|c| c.date).max();
    let mut tx = db.write.begin().await?;
    for checkpoint in &unlocked {
        db::checkpoints::set_reconciled(&mut *tx, *checkpoint, false).await?;
    }
    db::checkpoints::unreconcile_after(&mut *tx, point.account_id, still_locked).await?;
    tx.commit().await?;
    Ok(())
}

/// Pistes de recherche d'écart entre le dernier point rapproché et ce point (R-RAP-9).
pub async fn gap_hints(db: &Db, id: CheckpointId) -> AppResult<GapHintsView> {
    let point = get(db, id).await?;
    let points = list(db, point.account_id).await?;
    let from = points.iter().filter(|c| c.reconciled && c.date < point.date).map(|c| c.date).max();
    let transactions = transactions_view(db, point.account_id, from, point.date).await?;
    let by_id: HashMap<TransactionId, &GapTransactionView> = transactions.iter().map(|t| (t.id, t)).collect();
    let hints = reconciliation::gap_hints(
        Money::from_minor(point.gap_minor),
        &transactions
            .iter()
            .map(|t| GapTransaction { id: t.id, amount: Money::from_minor(t.amount_minor) })
            .collect::<Vec<_>>(),
    );
    let view = |id: &TransactionId| {
        by_id.get(id).map(|t| GapTransactionView {
            id: t.id,
            date: t.date,
            label: t.label.clone(),
            amount_minor: t.amount_minor,
            clearing_status: t.clearing_status,
        })
    };
    Ok(GapHintsView {
        gap_minor: point.gap_minor,
        from,
        to: point.date,
        pending: transactions
            .iter()
            .filter(|t| t.clearing_status == ClearingStatus::Pending)
            .filter_map(|t| view(&t.id))
            .collect(),
        singles: hints.singles.iter().filter_map(view).collect(),
        pairs: hints.pairs.iter().filter_map(|(a, b)| Some([view(a)?, view(b)?])).collect(),
        sign_errors: hints.sign_errors.iter().filter_map(view).collect(),
    })
}

/// Pointage manuel d'une transaction, sans import (RAP-07).
pub async fn set_cleared(db: &Db, transaction: TransactionId, input: &ClearInput) -> AppResult<()> {
    let row = db::transactions::find(&db.read, transaction).await?.ok_or(AppError::NotFound("transaction"))?;
    if row.clearing_status == ClearingStatus::Reconciled {
        return Err(DomainError::TransactionLocked.into());
    }
    db::checkpoints::set_cleared(&db.write, transaction, input.cleared).await?;
    Ok(())
}

/// Santé des comptes à rapprocher (RAP-06, R-TDB-10).
pub async fn health(db: &Db, today: Date) -> AppResult<Vec<AccountHealth>> {
    let accounts = db::accounts::list(&db.read).await?;
    let ledger = Ledger::load(db).await?;
    let all = db::checkpoints::list_all(&db.read).await?;
    let old = today - chrono::Days::new(OLD_PENDING_DAYS);
    let mut result = Vec::new();
    for account in accounts.iter().filter(|a| {
        a.closed_on.is_none()
            && a.kind.family() != AccountFamily::VariableSavings
            && a.kind.family() != AccountFamily::Liability
    }) {
        let rows: Vec<CheckpointRow> = all.iter().filter(|c| c.account_id == account.id).cloned().collect();
        let points = views(rows, account, &ledger);
        let last_reconciled = points.iter().filter(|c| c.reconciled).map(|c| c.date).max();
        let latest = points.iter().filter(|c| !c.reconciled && !c.frozen).max_by_key(|c| c.date);
        let old_pending = db::transactions::list_for_account(&db.read, account.id)
            .await?
            .iter()
            .filter(|t| t.clearing_status == ClearingStatus::Pending && t.balance_date < old)
            .count();
        result.push(AccountHealth {
            account_id: account.id,
            account_name: account.name.clone(),
            last_reconciled,
            current_gap_minor: latest.map(|c| c.gap_minor),
            current_gap_date: latest.map(|c| c.date),
            old_pending: i64::try_from(old_pending).unwrap_or(i64::MAX),
        });
    }
    Ok(result)
}
