//! Transactions, ventilations et virements (spec 02).

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::ledger::{ClearingStatus, TransactionOrigin};
use cashflow_domain::shared::ids::{
    AccountId, AssetId, CategoryId, LoanId, ProjectId, RecurringRuleId, SplitId, TransactionId, TransferId,
};
use cashflow_domain::{Date, Money};
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct TransactionRow {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub date: Date,
    pub bank_date: Option<Date>,
    pub balance_date: Date,
    pub value_date: Option<Date>,
    pub amount: Money,
    pub bank_label: Option<String>,
    pub label: String,
    pub note: Option<String>,
    pub clearing_status: ClearingStatus,
    pub origin: TransactionOrigin,
}

#[derive(Debug, Clone)]
pub struct SplitRow {
    pub id: SplitId,
    pub transaction_id: TransactionId,
    pub amount: Money,
    pub category_id: Option<CategoryId>,
    pub note: Option<String>,
    pub project_id: Option<ProjectId>,
    /// Ventilation de remboursement d'un prêt (R-PRE-7).
    pub loan_id: Option<LoanId>,
    /// Ventilation d'acquisition d'un bien (R-BIE-8).
    pub asset_id: Option<AssetId>,
}

/// Le virement auquel appartient une transaction, vu depuis celle-ci (ADR-0021 : il relie deux ventilations).
#[derive(Debug, Clone, Copy)]
pub struct TransferSide {
    pub transfer_id: TransferId,
    pub transaction_id: TransactionId,
    /// Ventilation de virement de cette transaction.
    pub split_id: SplitId,
    pub counterpart_transaction_id: TransactionId,
    pub counterpart_split_id: SplitId,
    pub counterpart_account_id: AccountId,
    pub is_outflow: bool,
}

pub struct NewTransaction<'a> {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub date: Date,
    pub bank_date: Option<Date>,
    pub value_date: Option<Date>,
    pub amount: Money,
    pub bank_label: Option<&'a str>,
    pub label: &'a str,
    pub note: Option<&'a str>,
    pub clearing_status: ClearingStatus,
    pub origin: TransactionOrigin,
    pub recurring_rule_id: Option<RecurringRuleId>,
    pub fingerprint: Option<&'a str>,
}

#[derive(Default)]
pub struct NewSplit<'a> {
    pub amount: Money,
    pub category_id: Option<CategoryId>,
    pub note: Option<&'a str>,
    pub project_id: Option<ProjectId>,
    pub loan_id: Option<LoanId>,
    pub asset_id: Option<AssetId>,
}

macro_rules! transaction_row {
    ($r:expr) => {{
        let r = $r;
        anyhow::Ok(TransactionRow {
            id: id_from_db(&r.id)?,
            account_id: id_from_db(&r.account_id)?,
            date: r.date,
            bank_date: r.bank_date,
            balance_date: r.balance_date,
            value_date: r.value_date,
            amount: Money::from_minor(r.amount),
            bank_label: r.bank_label,
            label: r.label,
            note: r.note,
            clearing_status: enum_from_db(&r.clearing_status)?,
            origin: enum_from_db(&r.origin)?,
        })
    }};
}

macro_rules! split_row {
    ($r:expr) => {{
        let r = $r;
        anyhow::Ok(SplitRow {
            id: id_from_db(&r.id)?,
            transaction_id: id_from_db(&r.transaction_id)?,
            amount: Money::from_minor(r.amount),
            category_id: opt_id_from_db(r.category_id.as_deref())?,
            note: r.note,
            project_id: opt_id_from_db(r.project_id.as_deref())?,
            loan_id: opt_id_from_db(r.loan_id.as_deref())?,
            asset_id: opt_id_from_db(r.asset_id.as_deref())?,
        })
    }};
}

/// Toutes les transactions d'un compte, par date de solde puis ordre de création (TRX-02).
pub async fn list_for_account(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<Vec<TransactionRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", account_id AS "account_id!", date AS "date!: Date", bank_date AS "bank_date?: Date",
                  balance_date AS "balance_date!: Date", value_date AS "value_date?: Date", amount AS "amount!",
                  bank_label AS "bank_label?", label AS "label!", note AS "note?", clearing_status AS "clearing_status!",
                  origin AS "origin!"
           FROM ledger_transaction WHERE account_id = ? ORDER BY balance_date, id"#,
        account
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| transaction_row!(r)).collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: TransactionId) -> anyhow::Result<Option<TransactionRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id AS "id!", account_id AS "account_id!", date AS "date!: Date", bank_date AS "bank_date?: Date",
                  balance_date AS "balance_date!: Date", value_date AS "value_date?: Date", amount AS "amount!",
                  bank_label AS "bank_label?", label AS "label!", note AS "note?", clearing_status AS "clearing_status!",
                  origin AS "origin!"
           FROM ledger_transaction WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| transaction_row!(r)).transpose()
}

pub async fn splits_for_account(db: impl SqliteExecutor<'_>, account: AccountId) -> anyhow::Result<Vec<SplitRow>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT s.id, s.transaction_id, s.amount, s.category_id, s.note, s.project_id, s.loan_id, s.asset_id
           FROM split s JOIN ledger_transaction t ON t.id = s.transaction_id
           WHERE t.account_id = ? ORDER BY s.transaction_id, s.position"#,
        account
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| split_row!(r)).collect()
}

pub async fn splits_of(db: impl SqliteExecutor<'_>, transaction: TransactionId) -> anyhow::Result<Vec<SplitRow>> {
    let transaction = transaction.to_string();
    let rows = sqlx::query!(
        "SELECT id, transaction_id, amount, category_id, note, project_id, loan_id, asset_id
         FROM split WHERE transaction_id = ? ORDER BY position",
        transaction
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| split_row!(r)).collect()
}

pub async fn find_split(db: impl SqliteExecutor<'_>, id: SplitId) -> anyhow::Result<Option<SplitRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        "SELECT id, transaction_id, amount, category_id, note, project_id, loan_id, asset_id FROM split WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| split_row!(r)).transpose()
}

/// Les virements dont une jambe est sur le compte.
pub async fn transfers_for_account(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
) -> anyhow::Result<Vec<TransferSide>> {
    let account = account.to_string();
    let rows = sqlx::query!(
        r#"SELECT tr.id AS "id!", t.id AS "transaction_id!", other.id AS "counterpart_id!",
                  other.account_id AS "counterpart_account!",
                  CASE WHEN tr.outflow_transaction_id = t.id THEN tr.outflow_split_id ELSE tr.inflow_split_id END
                      AS "split_id!: String",
                  CASE WHEN tr.outflow_transaction_id = t.id THEN tr.inflow_split_id ELSE tr.outflow_split_id END
                      AS "counterpart_split_id!: String",
                  (tr.outflow_transaction_id = t.id) AS "is_outflow!: bool"
           FROM transfer_leg tr
           JOIN ledger_transaction t ON t.id IN (tr.outflow_transaction_id, tr.inflow_transaction_id)
           JOIN ledger_transaction other
             ON other.id = CASE WHEN tr.outflow_transaction_id = t.id THEN tr.inflow_transaction_id
                                ELSE tr.outflow_transaction_id END
           WHERE t.account_id = ?"#,
        account
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(TransferSide {
                transfer_id: id_from_db(&r.id)?,
                transaction_id: id_from_db(&r.transaction_id)?,
                split_id: id_from_db(&r.split_id)?,
                counterpart_transaction_id: id_from_db(&r.counterpart_id)?,
                counterpart_split_id: id_from_db(&r.counterpart_split_id)?,
                counterpart_account_id: id_from_db(&r.counterpart_account)?,
                is_outflow: r.is_outflow,
            })
        })
        .collect()
}

pub async fn transfer_of(
    db: impl SqliteExecutor<'_>,
    transaction: TransactionId,
) -> anyhow::Result<Option<TransferSide>> {
    let transaction = transaction.to_string();
    let row = sqlx::query!(
        r#"SELECT tr.id AS "id!", other.id AS "counterpart_id!", other.account_id AS "counterpart_account!",
                  CASE WHEN tr.outflow_transaction_id = ?1 THEN tr.outflow_split_id ELSE tr.inflow_split_id END
                      AS "split_id!: String",
                  CASE WHEN tr.outflow_transaction_id = ?1 THEN tr.inflow_split_id ELSE tr.outflow_split_id END
                      AS "counterpart_split_id!: String",
                  (tr.outflow_transaction_id = ?1) AS "is_outflow!: bool"
           FROM transfer_leg tr
           JOIN ledger_transaction other
             ON other.id = CASE WHEN tr.outflow_transaction_id = ?1 THEN tr.inflow_transaction_id
                                ELSE tr.outflow_transaction_id END
           WHERE ?1 IN (tr.outflow_transaction_id, tr.inflow_transaction_id)"#,
        transaction
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| {
        Ok(TransferSide {
            transfer_id: id_from_db(&r.id)?,
            transaction_id: id_from_db(&transaction)?,
            split_id: id_from_db(&r.split_id)?,
            counterpart_transaction_id: id_from_db(&r.counterpart_id)?,
            counterpart_split_id: id_from_db(&r.counterpart_split_id)?,
            counterpart_account_id: id_from_db(&r.counterpart_account)?,
            is_outflow: r.is_outflow,
        })
    })
    .transpose()
}

pub async fn insert(
    conn: &mut SqliteConnection,
    t: &NewTransaction<'_>,
    splits: &[NewSplit<'_>],
) -> anyhow::Result<()> {
    let id = t.id.to_string();
    let account = t.account_id.to_string();
    let amount = t.amount.minor();
    let status = enum_to_db(&t.clearing_status);
    let origin = enum_to_db(&t.origin);
    let rule = t.recurring_rule_id.map(|r| r.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO ledger_transaction (id, account_id, date, bank_date, value_date, amount, bank_label, label, note,
                                         clearing_status, origin, recurring_rule_id, fingerprint, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        account,
        t.date,
        t.bank_date,
        t.value_date,
        amount,
        t.bank_label,
        t.label,
        t.note,
        status,
        origin,
        rule,
        t.fingerprint,
        now,
        now
    )
    .execute(&mut *conn)
    .await?;
    replace_splits(conn, t.id, splits).await
}

/// Champs modifiables d'une transaction existante.
pub struct TransactionUpdate<'a> {
    pub account_id: AccountId,
    pub date: Date,
    pub bank_date: Option<Date>,
    pub value_date: Option<Date>,
    pub amount: Money,
    pub label: &'a str,
    pub note: Option<&'a str>,
    pub clearing_status: ClearingStatus,
}

pub async fn update(db: impl SqliteExecutor<'_>, id: TransactionId, u: &TransactionUpdate<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let account = u.account_id.to_string();
    let amount = u.amount.minor();
    let status = enum_to_db(&u.clearing_status);
    let now = timestamp();
    sqlx::query!(
        "UPDATE ledger_transaction SET account_id = ?, date = ?, bank_date = ?, value_date = ?, amount = ?, label = ?,
                                       note = ?, clearing_status = ?, updated_at = ?
         WHERE id = ?",
        account,
        u.date,
        u.bank_date,
        u.value_date,
        amount,
        u.label,
        u.note,
        status,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Remplace les ventilations d'une transaction. Refusé si l'une porte un virement : la supprimer emporterait le
/// virement en cascade (ADR-0021) ; une jambe change de montant par [`set_split_amount`].
pub async fn replace_splits(
    conn: &mut SqliteConnection,
    transaction: TransactionId,
    splits: &[NewSplit<'_>],
) -> anyhow::Result<()> {
    let transaction = transaction.to_string();
    let linked = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!: i64" FROM transfer_leg WHERE ?1 IN (outflow_transaction_id, inflow_transaction_id)"#,
        transaction
    )
    .fetch_one(&mut *conn)
    .await?;
    if linked > 0 {
        anyhow::bail!("les ventilations d'une jambe de virement ne se remplacent pas (ADR-0021)");
    }
    sqlx::query!("DELETE FROM split WHERE transaction_id = ?", transaction).execute(&mut *conn).await?;
    let now = timestamp();
    for (position, split) in splits.iter().enumerate() {
        let id = SplitId::new().to_string();
        let amount = split.amount.minor();
        let category = split.category_id.map(|c| c.to_string());
        let project = split.project_id.map(|p| p.to_string());
        let loan = split.loan_id.map(|l| l.to_string());
        let asset = split.asset_id.map(|a| a.to_string());
        let position = i64::try_from(position)?;
        sqlx::query!(
            "INSERT INTO split (id, transaction_id, amount, category_id, note, position, project_id, loan_id, asset_id,
                                created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            id,
            transaction,
            amount,
            category,
            split.note,
            position,
            project,
            loan,
            asset,
            now,
            now
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn set_split_category(
    db: impl SqliteExecutor<'_>,
    split: SplitId,
    category: Option<CategoryId>,
) -> anyhow::Result<()> {
    let split = split.to_string();
    let category = category.map(|c| c.to_string());
    let now = timestamp();
    // Catégoriser retire un éventuel rattachement à un prêt ou à un bien (R-TRX-37).
    sqlx::query!(
        "UPDATE split SET category_id = ?, loan_id = NULL, asset_id = NULL, updated_at = ? WHERE id = ?",
        category,
        now,
        split
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: TransactionId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM ledger_transaction WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Relie deux ventilations par un virement (ADR-0021).
pub async fn insert_transfer(
    db: impl SqliteExecutor<'_>,
    outflow: SplitId,
    inflow: SplitId,
    note: Option<&str>,
) -> anyhow::Result<TransferId> {
    let transfer = TransferId::new();
    let id = transfer.to_string();
    let outflow = outflow.to_string();
    let inflow = inflow.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO transfer (id, outflow_split_id, inflow_split_id, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        id,
        outflow,
        inflow,
        note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(transfer)
}

/// Relie deux transactions à ventilation unique par un virement ordinaire (R-TRX-32).
pub async fn insert_transfer_between(
    conn: &mut SqliteConnection,
    outflow: TransactionId,
    inflow: TransactionId,
) -> anyhow::Result<TransferId> {
    let single = async |conn: &mut SqliteConnection, id: TransactionId| -> anyhow::Result<SplitId> {
        match splits_of(&mut *conn, id).await?.as_slice() {
            [split] => Ok(split.id),
            _ => anyhow::bail!("une jambe de virement ordinaire a une seule ventilation (R-TRX-32)"),
        }
    };
    let outflow = single(&mut *conn, outflow).await?;
    let inflow = single(&mut *conn, inflow).await?;
    insert_transfer(&mut *conn, outflow, inflow, None).await
}

/// Change le montant d'une ventilation, sans toucher aux autres ni au virement qui la relie.
pub async fn set_split_amount(db: impl SqliteExecutor<'_>, id: SplitId, amount: Money) -> anyhow::Result<()> {
    let id = id.to_string();
    let amount = amount.minor();
    let now = timestamp();
    sqlx::query!("UPDATE split SET amount = ?, updated_at = ? WHERE id = ?", amount, now, id).execute(db).await?;
    Ok(())
}

pub async fn delete_transfer(db: impl SqliteExecutor<'_>, id: TransferId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM transfer WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Transactions candidates pour former un virement (R-TRX-35) : montant opposé, autre jambe libre.
pub async fn transfer_candidates(
    db: impl SqliteExecutor<'_>,
    account: AccountId,
    amount: Money,
    from: Date,
    to: Date,
) -> anyhow::Result<Vec<TransactionRow>> {
    let account = account.to_string();
    let opposite = -amount.minor();
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", account_id AS "account_id!", date AS "date!: Date", bank_date AS "bank_date?: Date",
                  balance_date AS "balance_date!: Date", value_date AS "value_date?: Date", amount AS "amount!",
                  bank_label AS "bank_label?", label AS "label!", note AS "note?", clearing_status AS "clearing_status!",
                  origin AS "origin!"
           FROM ledger_transaction t
           WHERE account_id = ? AND amount = ? AND balance_date BETWEEN ? AND ?
             AND NOT EXISTS (SELECT 1 FROM transfer_leg tr WHERE t.id IN (tr.outflow_transaction_id, tr.inflow_transaction_id))
           ORDER BY balance_date"#,
        account,
        opposite,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| transaction_row!(r)).collect()
}

// ---------------------------------------------------------------- Agrégats lus par les services

/// Mouvement d'un compte, pour les soldes (calculés par le domaine, ADR-0002).
pub struct Movement {
    pub account_id: AccountId,
    pub balance_date: Date,
    pub amount: Money,
    /// Pointée ou rapprochée : compte dans le solde pointé (CPT-03).
    pub cleared: bool,
}

pub async fn all_movements(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<Movement>> {
    let rows = sqlx::query!(
        r#"SELECT account_id AS "account_id!", balance_date AS "balance_date!: Date", amount AS "amount!",
                  (clearing_status <> 'PENDING') AS "cleared!: bool"
           FROM ledger_transaction ORDER BY account_id, balance_date, id"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(Movement {
                account_id: id_from_db(&r.account_id)?,
                balance_date: r.balance_date,
                amount: Money::from_minor(r.amount),
                cleared: r.cleared,
            })
        })
        .collect()
}

/// Ventilation vue par les indicateurs de flux (spec 08).
pub struct SplitFactRow {
    pub date: Date,
    pub amount: Money,
    pub category_kind: Option<CategoryKind>,
    pub is_movement: bool,
    pub currency: String,
}

pub async fn split_facts(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<SplitFactRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.date AS "date: Date", s.amount, c.kind AS "category_kind?", a.currency,
                  (s.loan_id IS NOT NULL OR s.asset_id IS NOT NULL
                   OR EXISTS (SELECT 1 FROM transfer tr WHERE s.id IN (tr.outflow_split_id, tr.inflow_split_id)))
                      AS "is_movement!: bool"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN account a ON a.id = t.account_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE t.date BETWEEN ? AND ?"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(SplitFactRow {
                date: r.date,
                amount: Money::from_minor(r.amount),
                category_kind: r.category_kind.as_deref().map(enum_from_db).transpose()?,
                is_movement: r.is_movement,
                currency: r.currency,
            })
        })
        .collect()
}

/// Ventilation non catégorisée, hors virements, à catégoriser depuis l'accueil (R-TDB-5).
pub struct UncategorizedRow {
    pub split_id: SplitId,
    pub transaction_id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    pub amount: Money,
    pub split_count: i64,
}

pub async fn uncategorized(db: impl SqliteExecutor<'_>, limit: i64) -> anyhow::Result<(Vec<UncategorizedRow>, i64)> {
    let rows = sqlx::query!(
        r#"SELECT s.id, t.id AS "transaction_id!", a.id AS "account_id!", a.name AS "account_name!",
                  t.date AS "date!: Date", t.label AS "label!", s.amount,
                  (SELECT count(*) FROM split x WHERE x.transaction_id = t.id) AS "split_count!: i64",
                  count(*) OVER () AS "total!: i64"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN account a ON a.id = t.account_id
           WHERE s.category_id IS NULL AND s.loan_id IS NULL AND s.asset_id IS NULL
             AND NOT EXISTS (SELECT 1 FROM transfer tr WHERE s.id IN (tr.outflow_split_id, tr.inflow_split_id))
           ORDER BY t.date DESC, t.id DESC, s.position
           LIMIT ?"#,
        limit
    )
    .fetch_all(db)
    .await?;
    let total = rows.first().map_or(0, |r| r.total);
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(UncategorizedRow {
                split_id: id_from_db(&r.id)?,
                transaction_id: id_from_db(&r.transaction_id)?,
                account_id: id_from_db(&r.account_id)?,
                account_name: r.account_name,
                date: r.date,
                label: r.label,
                amount: Money::from_minor(r.amount),
                split_count: r.split_count,
            })
        })
        .collect::<anyhow::Result<_>>()?;
    Ok((items, total))
}

/// Transactions dont la somme des ventilations diffère du montant (ADR-0017, point 3).
pub async fn split_sum_mismatches(conn: &mut SqliteConnection, ids: &[TransactionId]) -> anyhow::Result<Vec<String>> {
    let mut mismatches = Vec::new();
    for id in ids {
        let id = id.to_string();
        let row = sqlx::query!(
            r#"SELECT t.amount AS "amount!", (SELECT coalesce(sum(s.amount), 0) FROM split s WHERE s.transaction_id = t.id) AS "total!: i64"
               FROM ledger_transaction t WHERE t.id = ?"#,
            id
        )
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(row) = row
            && row.amount != row.total
        {
            mismatches.push(id);
        }
    }
    Ok(mismatches)
}

/// Toutes les transactions, tous comptes confondus (TRX-08).
pub async fn list_all(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<TransactionRow>> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", account_id AS "account_id!", date AS "date!: Date", bank_date AS "bank_date?: Date",
                  balance_date AS "balance_date!: Date", value_date AS "value_date?: Date", amount AS "amount!",
                  bank_label AS "bank_label?", label AS "label!", note AS "note?", clearing_status AS "clearing_status!",
                  origin AS "origin!"
           FROM ledger_transaction ORDER BY date, id"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| transaction_row!(r)).collect()
}

pub async fn all_splits(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<SplitRow>> {
    let rows = sqlx::query!(
        "SELECT id, transaction_id, amount, category_id, note, project_id, loan_id, asset_id
         FROM split ORDER BY transaction_id, position"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| split_row!(r)).collect()
}

/// Un virement et ses deux jambes : transactions et ventilations.
pub struct TransferRow {
    pub id: TransferId,
    pub outflow_transaction_id: TransactionId,
    pub inflow_transaction_id: TransactionId,
    pub outflow_split_id: SplitId,
    pub inflow_split_id: SplitId,
}

pub async fn all_transfers(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<TransferRow>> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", outflow_transaction_id AS "outflow!", inflow_transaction_id AS "inflow!",
                  outflow_split_id AS "outflow_split!", inflow_split_id AS "inflow_split!"
           FROM transfer_leg"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(TransferRow {
                id: id_from_db(&r.id)?,
                outflow_transaction_id: id_from_db(&r.outflow)?,
                inflow_transaction_id: id_from_db(&r.inflow)?,
                outflow_split_id: id_from_db(&r.outflow_split)?,
                inflow_split_id: id_from_db(&r.inflow_split)?,
            })
        })
        .collect()
}

/// Ventilation vue par l'analyse par catégorie (TDB-02).
pub struct CategorizedFactRow {
    pub date: Date,
    pub amount: Money,
    pub category_id: Option<CategoryId>,
    pub has_project: bool,
    pub category_kind: Option<CategoryKind>,
    pub is_movement: bool,
    pub currency: String,
}

pub async fn categorized_facts(
    db: impl SqliteExecutor<'_>,
    from: Date,
    to: Date,
) -> anyhow::Result<Vec<CategorizedFactRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.date AS "date!: Date", s.amount, s.category_id, s.project_id IS NOT NULL AS "has_project!: bool",
                  c.kind AS "category_kind?", a.currency,
                  (s.loan_id IS NOT NULL OR s.asset_id IS NOT NULL
                   OR EXISTS (SELECT 1 FROM transfer tr WHERE s.id IN (tr.outflow_split_id, tr.inflow_split_id)))
                      AS "is_movement!: bool"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN account a ON a.id = t.account_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE t.date BETWEEN ? AND ?"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(CategorizedFactRow {
                date: r.date,
                amount: Money::from_minor(r.amount),
                category_id: opt_id_from_db(r.category_id.as_deref())?,
                has_project: r.has_project,
                category_kind: r.category_kind.as_deref().map(enum_from_db).transpose()?,
                is_movement: r.is_movement,
                currency: r.currency,
            })
        })
        .collect()
}

/// Une transaction récemment saisie, pour l'accueil (TDB-01).
pub struct RecentRow {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub currency: String,
    pub date: Date,
    pub label: String,
    pub amount: Money,
    pub clearing_status: ClearingStatus,
    /// Catégorie de l'unique ventilation ; absente si plusieurs ventilations ou non catégorisée.
    pub category_id: Option<CategoryId>,
    pub split_count: i64,
    /// Compte de l'autre jambe, pour un virement interne.
    pub counterpart_account_name: Option<String>,
}

/// Les `limit` dernières transactions saisies (ordre de création, la plus récente d'abord).
pub async fn recent(db: impl SqliteExecutor<'_>, limit: i64) -> anyhow::Result<Vec<RecentRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.id AS "id!", a.id AS "account_id!", a.name AS "account_name!", a.currency AS "currency!",
                  t.date AS "date!: Date", t.label AS "label!", t.amount AS "amount!: i64",
                  t.clearing_status AS "clearing_status!",
                  (SELECT count(*) FROM split x WHERE x.transaction_id = t.id) AS "split_count!: i64",
                  (SELECT x.category_id FROM split x WHERE x.transaction_id = t.id ORDER BY x.position LIMIT 1)
                      AS "category_id?",
                  (SELECT oa.name FROM transfer_leg tr
                     JOIN ledger_transaction o
                       ON o.id = CASE WHEN tr.outflow_transaction_id = t.id THEN tr.inflow_transaction_id
                                      ELSE tr.outflow_transaction_id END
                     JOIN account oa ON oa.id = o.account_id
                   WHERE t.id IN (tr.outflow_transaction_id, tr.inflow_transaction_id)) AS "counterpart?"
           FROM ledger_transaction t
           JOIN account a ON a.id = t.account_id
           ORDER BY t.created_at DESC, t.id DESC
           LIMIT ?"#,
        limit
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(RecentRow {
                id: id_from_db(&r.id)?,
                account_id: id_from_db(&r.account_id)?,
                account_name: r.account_name,
                currency: r.currency,
                date: r.date,
                label: r.label,
                amount: Money::from_minor(r.amount),
                clearing_status: enum_from_db(&r.clearing_status)?,
                category_id: if r.split_count == 1 { opt_id_from_db(r.category_id.as_deref())? } else { None },
                split_count: r.split_count,
                counterpart_account_name: r.counterpart,
            })
        })
        .collect()
}

/// Ventilation rattachée à un prêt (remboursement) ou à un bien (acquisition), avec sa transaction.
pub struct AttachedSplitRow {
    pub transaction_id: TransactionId,
    pub date: Date,
    pub label: String,
    pub amount: Money,
    pub loan_id: Option<LoanId>,
    pub asset_id: Option<AssetId>,
}

pub async fn attached_splits(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<AttachedSplitRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.id AS "transaction_id!", t.date AS "date!: Date", t.label AS "label!", s.amount, s.loan_id,
                  s.asset_id
           FROM split s JOIN ledger_transaction t ON t.id = s.transaction_id
           WHERE s.loan_id IS NOT NULL OR s.asset_id IS NOT NULL
           ORDER BY t.date DESC, t.id"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(AttachedSplitRow {
                transaction_id: id_from_db(&r.transaction_id)?,
                date: r.date,
                label: r.label,
                amount: Money::from_minor(r.amount),
                loan_id: opt_id_from_db(r.loan_id.as_deref())?,
                asset_id: opt_id_from_db(r.asset_id.as_deref())?,
            })
        })
        .collect()
}

/// Les remboursements d'un prêt supprimé redeviennent des ventilations non catégorisées (R-PRE-11).
pub async fn detach_loan(db: impl SqliteExecutor<'_>, loan: LoanId) -> anyhow::Result<()> {
    let loan = loan.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE split SET loan_id = NULL, updated_at = ? WHERE loan_id = ?", now, loan).execute(db).await?;
    Ok(())
}

/// Les acquisitions d'un bien supprimé redeviennent des ventilations non catégorisées (R-BIE-6).
pub async fn detach_asset(db: impl SqliteExecutor<'_>, asset: AssetId) -> anyhow::Result<()> {
    let asset = asset.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE split SET asset_id = NULL, updated_at = ? WHERE asset_id = ?", now, asset).execute(db).await?;
    Ok(())
}
