//! Transactions et ventilations (TRX-01 à TRX-03).

use std::collections::{HashMap, HashSet};

use cashflow_domain::accounts::AccountPeriod;
use cashflow_domain::ledger::balance::{self, DatedAmount};
use cashflow_domain::ledger::split;
use cashflow_domain::ledger::{self, BalanceAffectingChange, ClearingStatus, TransactionOrigin};
use cashflow_domain::shared::ids::{AccountId, CategoryId, ProjectId, SplitId, TransactionId, TransferId};
use cashflow_domain::{Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::{IntoParams, ToSchema};

use super::{check_split_sums, clean, reconciliation, required};
use crate::db::accounts::AccountRow;
use crate::db::transactions::{NewSplit, NewTransaction, SplitRow, TransactionUpdate, TransferSide};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------- Types exposés

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Split {
    pub id: SplitId,
    pub amount_minor: i64,
    pub category_id: Option<CategoryId>,
    pub note: Option<String>,
    pub project_id: Option<ProjectId>,
}

impl From<SplitRow> for Split {
    fn from(r: SplitRow) -> Self {
        Split {
            id: r.id,
            amount_minor: r.amount.minor(),
            category_id: r.category_id,
            note: r.note,
            project_id: r.project_id,
        }
    }
}

/// L'autre jambe d'un virement, vue depuis une transaction.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TransferSummary {
    pub transfer_id: TransferId,
    pub counterpart_transaction_id: TransactionId,
    pub counterpart_account_id: AccountId,
    pub counterpart_account_name: String,
    /// Vrai pour la jambe sortante (« Virement vers… »).
    pub is_outflow: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionListItem {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub date: Date,
    pub bank_date: Option<Date>,
    pub balance_date: Date,
    pub label: String,
    pub note: Option<String>,
    pub amount_minor: i64,
    pub clearing_status: ClearingStatus,
    pub origin: TransactionOrigin,
    pub splits: Vec<Split>,
    pub transfer: Option<TransferSummary>,
    /// Solde du compte après cette opération (TRX-02) ; absent dans la vue transverse (TRX-08).
    pub balance_after_minor: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionPage {
    /// Les plus récentes d'abord.
    pub items: Vec<TransactionListItem>,
    /// À repasser dans `cursor` pour la page suivante ; absent à la dernière page.
    pub next_cursor: Option<String>,
    /// Nombre total de transactions qui correspondent aux filtres.
    pub total: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TransactionDetail {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub currency: String,
    pub date: Date,
    pub bank_date: Option<Date>,
    pub value_date: Option<Date>,
    pub balance_date: Date,
    pub amount_minor: i64,
    pub bank_label: Option<String>,
    pub label: String,
    pub note: Option<String>,
    pub clearing_status: ClearingStatus,
    pub origin: TransactionOrigin,
    pub splits: Vec<Split>,
    pub transfer: Option<TransferSummary>,
    /// Rapprochée : montant, compte et date de solde sont verrouillés (R-TRX-3).
    pub locked: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SplitInput {
    pub amount_minor: i64,
    pub category_id: Option<CategoryId>,
    pub note: Option<String>,
    pub project_id: Option<ProjectId>,
}

/// Saisie d'une transaction (TRX-01). Sans `splits`, une ventilation unique sur `category_id`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct TransactionInput {
    pub account_id: AccountId,
    pub date: Date,
    pub amount_minor: i64,
    pub label: String,
    pub note: Option<String>,
    pub category_id: Option<CategoryId>,
    pub splits: Option<Vec<SplitInput>>,
}

/// Modification d'une transaction. Sans `splits`, la ventilation existante est conservée ; une
/// ventilation unique suit alors le nouveau montant (R-TRX-4).
#[derive(Debug, Deserialize, ToSchema)]
pub struct TransactionUpdateInput {
    pub account_id: AccountId,
    pub date: Date,
    pub amount_minor: i64,
    pub label: String,
    pub note: Option<String>,
    pub splits: Option<Vec<SplitInput>>,
}

/// Catégorisation en masse (TRX-06).
#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkCategorizeInput {
    pub transaction_ids: Vec<TransactionId>,
    pub category_id: CategoryId,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkCategorizeResult {
    pub categorized: usize,
    /// Virements et transactions ventilées, laissés tels quels.
    pub skipped: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CategorizeInput {
    pub category_id: Option<CategoryId>,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TransactionFilter {
    /// Vue transverse (TRX-08) : limite à ces comptes, séparés par des virgules.
    pub accounts: Option<String>,
    /// Seulement les transactions qui ont une ventilation non catégorisée (hors virements).
    pub uncategorized: Option<bool>,
    /// Date de l'opération, incluse.
    pub from: Option<Date>,
    pub to: Option<Date>,
    /// Inclut les sous-catégories (R-TRX-21).
    pub category_id: Option<CategoryId>,
    pub status: Option<ClearingStatus>,
    /// Bornes sur la valeur absolue du montant.
    pub min_amount_minor: Option<i64>,
    pub max_amount_minor: Option<i64>,
    /// Recherche dans le libellé, le libellé bancaire et la note.
    pub text: Option<String>,
    pub cursor: Option<String>,
    /// 200 par défaut.
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------- Lecture

async fn account_names(db: &Db) -> AppResult<HashMap<AccountId, String>> {
    Ok(db::accounts::list(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect())
}

fn transfer_summary(side: &TransferSide, names: &HashMap<AccountId, String>) -> TransferSummary {
    TransferSummary {
        transfer_id: side.transfer_id,
        counterpart_transaction_id: side.counterpart_transaction_id,
        counterpart_account_id: side.counterpart_account_id,
        counterpart_account_name: names.get(&side.counterpart_account_id).cloned().unwrap_or_default(),
        is_outflow: side.is_outflow,
    }
}

/// Transactions d'un compte, filtrées, avec le solde après chaque opération (TRX-02).
pub async fn list(db: &Db, account: AccountId, filter: &TransactionFilter) -> AppResult<TransactionPage> {
    let row = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    let transactions = db::transactions::list_for_account(&db.read, account).await?;
    let mut splits: HashMap<TransactionId, Vec<Split>> = HashMap::new();
    for s in db::transactions::splits_for_account(&db.read, account).await? {
        splits.entry(s.transaction_id).or_default().push(s.into());
    }
    let transfers: HashMap<TransactionId, TransferSide> = db::transactions::transfers_for_account(&db.read, account)
        .await?
        .into_iter()
        .map(|t| (t.transaction_id, t))
        .collect();
    let names = account_names(db).await?;

    // Le solde après opération se calcule sur toutes les transactions, avant filtrage.
    let movements: Vec<DatedAmount> =
        transactions.iter().map(|t| DatedAmount { balance_date: t.balance_date, amount: t.amount }).collect();
    let running = balance::running_balances(row.opening_balance, &movements);

    let category_ids = category_family(db, filter.category_id).await?;
    let text = clean(filter.text.as_deref()).map(|t| t.to_lowercase());

    let mut items: Vec<TransactionListItem> = transactions
        .into_iter()
        .zip(running)
        .filter_map(|(t, balance_after)| {
            let tx_splits = splits.remove(&t.id).unwrap_or_default();
            let keep = matches_filter(
                &t,
                &tx_splits,
                transfers.contains_key(&t.id),
                filter,
                category_ids.as_ref(),
                text.as_deref(),
            );
            keep.then(|| TransactionListItem {
                id: t.id,
                account_name: row.name.clone(),
                account_id: t.account_id,
                date: t.date,
                bank_date: t.bank_date,
                balance_date: t.balance_date,
                label: t.label,
                note: t.note,
                amount_minor: t.amount.minor(),
                clearing_status: t.clearing_status,
                origin: t.origin,
                splits: tx_splits,
                transfer: transfers.get(&t.id).map(|side| transfer_summary(side, &names)),
                balance_after_minor: Some(balance_after.minor()),
            })
        })
        .collect();
    items.reverse();

    let total = items.len();
    let start = match &filter.cursor {
        Some(cursor) => items.iter().position(|i| i.id.to_string() == *cursor).map_or(total, |p| p + 1),
        None => 0,
    };
    let limit = filter.limit.unwrap_or(200).clamp(1, 1000);
    let mut page: Vec<TransactionListItem> = items.into_iter().skip(start).collect();
    let next_cursor = (page.len() > limit).then(|| page[limit - 1].id.to_string());
    page.truncate(limit);
    Ok(TransactionPage { items: page, next_cursor, total })
}

pub async fn get(db: &Db, id: TransactionId) -> AppResult<TransactionDetail> {
    let t = db::transactions::find(&db.write, id).await?.ok_or(AppError::NotFound("transaction"))?;
    let account = db::accounts::find(&db.write, t.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    let splits = db::transactions::splits_of(&db.write, id).await?.into_iter().map(Into::into).collect();
    let names = account_names(db).await?;
    let transfer = db::transactions::transfer_of(&db.write, id).await?.map(|side| transfer_summary(&side, &names));
    Ok(TransactionDetail {
        id: t.id,
        account_id: t.account_id,
        account_name: account.name,
        currency: account.currency.code().to_owned(),
        date: t.date,
        bank_date: t.bank_date,
        value_date: t.value_date,
        balance_date: t.balance_date,
        amount_minor: t.amount.minor(),
        bank_label: t.bank_label,
        label: t.label,
        note: t.note,
        locked: t.clearing_status == ClearingStatus::Reconciled,
        clearing_status: t.clearing_status,
        origin: t.origin,
        splits,
        transfer,
    })
}

// ---------------------------------------------------------------- Écriture

pub(crate) fn period(account: &AccountRow) -> AccountPeriod {
    AccountPeriod { opening: account.opening_date, closed_on: account.closed_on }
}

/// Vérifie les catégories et projets cités par des ventilations. Un projet clos garde les ventilations
/// qui y sont déjà rattachées (`kept`), mais n'en reçoit pas de nouvelles (R-BUD-24).
async fn check_references(conn: &mut SqliteConnection, splits: &[SplitInput], kept: &[ProjectId]) -> AppResult<()> {
    for s in splits {
        if let Some(category) = s.category_id
            && db::categories::find(&mut *conn, category).await?.is_none()
        {
            return Err(AppError::NotFound("catégorie"));
        }
        if let Some(project) = s.project_id
            && !kept.contains(&project)
        {
            super::budgets::check_active(conn, project).await?;
        }
    }
    Ok(())
}

fn new_splits(splits: &[SplitInput]) -> Vec<NewSplit<'_>> {
    splits
        .iter()
        .map(|s| NewSplit {
            amount: Money::from_minor(s.amount_minor),
            category_id: s.category_id,
            note: s.note.as_deref().map(str::trim).filter(|n| !n.is_empty()),
            project_id: s.project_id,
        })
        .collect()
}

fn amounts(splits: &[SplitInput]) -> Vec<Money> {
    splits.iter().map(|s| Money::from_minor(s.amount_minor)).collect()
}

/// Saisie manuelle : `PENDING`, origine `MANUAL` (TRX-01).
pub async fn create(db: &Db, input: &TransactionInput) -> AppResult<TransactionId> {
    let amount = Money::from_minor(input.amount_minor);
    ledger::check_amount(amount)?;
    let label = required(&input.label, "label", 200)?;
    let splits = input.splits.clone().unwrap_or_else(|| {
        vec![SplitInput {
            amount_minor: input.amount_minor,
            category_id: input.category_id,
            note: None,
            project_id: None,
        }]
    });
    split::check_splits(amount, &amounts(&splits))?;

    let mut tx = db.write.begin().await?;
    let account = db::accounts::find(&mut *tx, input.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    period(&account).check(input.date)?;
    reconciliation::lock(&mut tx, input.account_id).await?.check(input.date)?;
    check_references(&mut tx, &splits, &[]).await?;
    let id = TransactionId::new();
    let note = clean(input.note.as_deref());
    let new = NewTransaction {
        id,
        account_id: input.account_id,
        date: input.date,
        bank_date: None,
        value_date: None,
        amount,
        bank_label: None,
        label: &label,
        note: note.as_deref(),
        clearing_status: ClearingStatus::Pending,
        origin: TransactionOrigin::Manual,
        recurring_rule_id: None,
        fingerprint: None,
    };
    db::transactions::insert(&mut tx, &new, &new_splits(&splits)).await?;
    check_split_sums(&mut tx, &[id]).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update(db: &Db, id: TransactionId, input: &TransactionUpdateInput) -> AppResult<()> {
    let amount = Money::from_minor(input.amount_minor);
    ledger::check_amount(amount)?;
    let label = required(&input.label, "label", 200)?;
    let mut tx = db.write.begin().await?;
    let current = db::transactions::find(&mut *tx, id).await?.ok_or(AppError::NotFound("transaction"))?;
    let account = db::accounts::find(&mut *tx, input.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    let new_balance_date = current.bank_date.unwrap_or(input.date);
    ledger::check_modification(
        current.clearing_status,
        BalanceAffectingChange {
            amount: amount != current.amount,
            account: input.account_id != current.account_id,
            balance_date: new_balance_date != current.balance_date,
        },
    )?;
    period(&account).check(input.date)?;
    period(&account).check(new_balance_date)?;
    // Une écriture qui change le solde à une date rapprochée est refusée, avant comme après (R-RAP-7).
    let moves_balance =
        amount != current.amount || input.account_id != current.account_id || new_balance_date != current.balance_date;
    if moves_balance {
        reconciliation::lock(&mut tx, current.account_id).await?.check(current.balance_date)?;
        reconciliation::lock(&mut tx, input.account_id).await?.check(new_balance_date)?;
    }

    let transfer = db::transactions::transfer_of(&mut *tx, id).await?;
    let existing = db::transactions::splits_of(&mut *tx, id).await?;
    let splits: Vec<SplitInput> = match &input.splits {
        Some(splits) => splits.clone(),
        None if existing.len() == 1 => vec![SplitInput {
            amount_minor: input.amount_minor,
            category_id: existing[0].category_id,
            note: existing[0].note.clone(),
            project_id: existing[0].project_id,
        }],
        None => existing
            .iter()
            .map(|s| SplitInput {
                amount_minor: s.amount.minor(),
                category_id: s.category_id,
                note: s.note.clone(),
                project_id: s.project_id,
            })
            .collect(),
    };
    split::check_splits(amount, &amounts(&splits))?;
    let kept: Vec<ProjectId> = existing.iter().filter_map(|s| s.project_id).collect();
    check_references(&mut tx, &splits, &kept).await?;

    let mut touched = vec![id];
    if let Some(side) = &transfer {
        if splits.len() != 1 || splits[0].category_id.is_some() {
            return Err(DomainError::TransferLegCategorized.into());
        }
        let counterpart = db::transactions::find(&mut *tx, side.counterpart_transaction_id)
            .await?
            .ok_or(AppError::NotFound("jambe de virement"))?;
        let counterpart_account =
            db::accounts::find(&mut *tx, counterpart.account_id).await?.ok_or(AppError::NotFound("compte"))?;
        if counterpart.account_id == input.account_id {
            return Err(DomainError::TransferSameAccount.into());
        }
        if counterpart_account.currency != account.currency {
            return Err(DomainError::TransferCurrencyMismatch.into());
        }
        if amount.is_negative() != side.is_outflow {
            return Err(DomainError::TransferAmountsNotOpposite.into());
        }
        // Les deux jambes gardent des montants opposés (R-TRX-30).
        if amount != current.amount {
            ledger::check_modification(
                counterpart.clearing_status,
                BalanceAffectingChange { amount: true, ..Default::default() },
            )?;
            reconciliation::lock(&mut tx, counterpart.account_id).await?.check(counterpart.balance_date)?;
            let update = TransactionUpdate {
                account_id: counterpart.account_id,
                date: counterpart.date,
                bank_date: counterpart.bank_date,
                value_date: counterpart.value_date,
                amount: -amount,
                label: &counterpart.label,
                note: counterpart.note.as_deref(),
                clearing_status: counterpart.clearing_status,
            };
            db::transactions::update(&mut *tx, counterpart.id, &update).await?;
            // Les ventilations reliées changent de montant sur place : les remplacer délierait le virement.
            db::transactions::set_split_amount(&mut *tx, side.counterpart_split_id, -amount).await?;
            touched.push(counterpart.id);
        }
    }

    let note = clean(input.note.as_deref());
    let update = TransactionUpdate {
        account_id: input.account_id,
        date: input.date,
        bank_date: current.bank_date,
        value_date: current.value_date,
        amount,
        label: &label,
        note: note.as_deref(),
        clearing_status: current.clearing_status,
    };
    db::transactions::update(&mut *tx, id, &update).await?;
    match &transfer {
        Some(side) => db::transactions::set_split_amount(&mut *tx, side.split_id, amount).await?,
        None => db::transactions::replace_splits(&mut tx, id, &new_splits(&splits)).await?,
    }
    check_split_sums(&mut tx, &touched).await?;
    tx.commit().await?;
    Ok(())
}

/// Supprime une transaction. Une jambe de virement supprimée délie l'autre, qui reste (R-TRX-34).
pub async fn delete(db: &Db, id: TransactionId) -> AppResult<()> {
    let current = db::transactions::find(&db.read, id).await?.ok_or(AppError::NotFound("transaction"))?;
    if current.clearing_status == ClearingStatus::Reconciled {
        return Err(DomainError::TransactionLocked.into());
    }
    let mut conn = db.write.acquire().await?;
    reconciliation::lock(&mut conn, current.account_id).await?.check(current.balance_date)?;
    db::transactions::delete(&mut *conn, id).await?;
    Ok(())
}

/// Catégorise une ventilation sur place, depuis l'accueil (`ecrans.md` E1).
pub async fn categorize_split(db: &Db, split_id: SplitId, input: &CategorizeInput) -> AppResult<()> {
    let split = db::transactions::find_split(&db.read, split_id).await?.ok_or(AppError::NotFound("ventilation"))?;
    if db::transactions::transfer_of(&db.read, split.transaction_id).await?.is_some() {
        return Err(DomainError::TransferLegCategorized.into());
    }
    if let Some(category) = input.category_id
        && db::categories::find(&db.read, category).await?.is_none()
    {
        return Err(AppError::NotFound("catégorie"));
    }
    db::transactions::set_split_category(&db.write, split_id, input.category_id).await?;
    Ok(())
}

/// Insère une transaction déjà validée et sa ventilation, dans une transaction SQLite ouverte.
pub(crate) async fn insert_validated(
    conn: &mut SqliteConnection,
    t: &NewTransaction<'_>,
    splits: &[NewSplit<'_>],
) -> AppResult<()> {
    ledger::check_amount(t.amount)?;
    split::check_splits(t.amount, &splits.iter().map(|s| s.amount).collect::<Vec<_>>())?;
    db::transactions::insert(conn, t, splits).await?;
    Ok(())
}

/// Filtre commun aux listes de transactions.
fn matches_filter(
    t: &crate::db::transactions::TransactionRow,
    splits: &[Split],
    is_transfer: bool,
    filter: &TransactionFilter,
    category_ids: Option<&HashSet<CategoryId>>,
    text: Option<&str>,
) -> bool {
    filter.from.is_none_or(|from| t.date >= from)
        && filter.to.is_none_or(|to| t.date <= to)
        && filter.status.is_none_or(|status| t.clearing_status == status)
        && filter.min_amount_minor.is_none_or(|min| t.amount.minor().abs() >= min)
        && filter.max_amount_minor.is_none_or(|max| t.amount.minor().abs() <= max)
        && category_ids.is_none_or(|ids| splits.iter().any(|s| s.category_id.is_some_and(|c| ids.contains(&c))))
        && (!filter.uncategorized.unwrap_or(false) || (!is_transfer && splits.iter().any(|s| s.category_id.is_none())))
        && text.is_none_or(|text| {
            [Some(&t.label), t.bank_label.as_ref(), t.note.as_ref()]
                .into_iter()
                .flatten()
                .any(|field| field.to_lowercase().contains(text))
        })
}

async fn category_family(db: &Db, category: Option<CategoryId>) -> AppResult<Option<HashSet<CategoryId>>> {
    let Some(category) = category else { return Ok(None) };
    let mut ids: HashSet<CategoryId> = HashSet::from([category]);
    ids.extend(
        db::categories::list(&db.read).await?.into_iter().filter(|c| c.parent_id == Some(category)).map(|c| c.id),
    );
    Ok(Some(ids))
}

/// Toutes les transactions, tous comptes confondus, les plus récentes d'abord (TRX-08).
pub async fn list_all(db: &Db, filter: &TransactionFilter) -> AppResult<TransactionPage> {
    let accounts: HashMap<AccountId, String> =
        db::accounts::list(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect();
    let wanted: Option<HashSet<AccountId>> =
        filter.accounts.as_deref().map(|list| list.split(',').filter_map(|id| id.trim().parse().ok()).collect());
    let mut splits: HashMap<TransactionId, Vec<Split>> = HashMap::new();
    for s in db::transactions::all_splits(&db.read).await? {
        splits.entry(s.transaction_id).or_default().push(s.into());
    }
    let transactions = db::transactions::list_all(&db.read).await?;
    let account_of: HashMap<TransactionId, AccountId> = transactions.iter().map(|t| (t.id, t.account_id)).collect();
    let mut sides: HashMap<TransactionId, TransferSide> = HashMap::new();
    for t in db::transactions::all_transfers(&db.read).await? {
        let (outflow, inflow) = (t.outflow_transaction_id, t.inflow_transaction_id);
        for (this, this_split, other, other_split, is_outflow) in [
            (outflow, t.outflow_split_id, inflow, t.inflow_split_id, true),
            (inflow, t.inflow_split_id, outflow, t.outflow_split_id, false),
        ] {
            if let Some(account) = account_of.get(&other) {
                sides.insert(
                    this,
                    TransferSide {
                        transfer_id: t.id,
                        transaction_id: this,
                        split_id: this_split,
                        counterpart_transaction_id: other,
                        counterpart_split_id: other_split,
                        counterpart_account_id: *account,
                        is_outflow,
                    },
                );
            }
        }
    }
    let category_ids = category_family(db, filter.category_id).await?;
    let text = clean(filter.text.as_deref()).map(|t| t.to_lowercase());
    let mut items: Vec<TransactionListItem> = transactions
        .into_iter()
        .filter(|t| wanted.as_ref().is_none_or(|w| w.contains(&t.account_id)))
        .filter_map(|t| {
            let tx_splits = splits.remove(&t.id).unwrap_or_default();
            let side = sides.get(&t.id);
            matches_filter(&t, &tx_splits, side.is_some(), filter, category_ids.as_ref(), text.as_deref()).then(|| {
                TransactionListItem {
                    id: t.id,
                    account_name: accounts.get(&t.account_id).cloned().unwrap_or_default(),
                    account_id: t.account_id,
                    date: t.date,
                    bank_date: t.bank_date,
                    balance_date: t.balance_date,
                    label: t.label,
                    note: t.note,
                    amount_minor: t.amount.minor(),
                    clearing_status: t.clearing_status,
                    origin: t.origin,
                    splits: tx_splits,
                    transfer: side.map(|s| transfer_summary(s, &accounts)),
                    balance_after_minor: None,
                }
            })
        })
        .collect();
    items.reverse();
    let total = items.len();
    let start = match &filter.cursor {
        Some(cursor) => items.iter().position(|i| i.id.to_string() == *cursor).map_or(total, |p| p + 1),
        None => 0,
    };
    let limit = filter.limit.unwrap_or(200).clamp(1, 1000);
    let mut page: Vec<TransactionListItem> = items.into_iter().skip(start).collect();
    let next_cursor = (page.len() > limit).then(|| page[limit - 1].id.to_string());
    page.truncate(limit);
    Ok(TransactionPage { items: page, next_cursor, total })
}

/// Catégorise plusieurs transactions d'un coup (TRX-06) : seules les transactions à ventilation unique,
/// hors virements, sont modifiées.
pub async fn categorize_many(db: &Db, input: &BulkCategorizeInput) -> AppResult<BulkCategorizeResult> {
    if db::categories::find(&db.read, input.category_id).await?.is_none() {
        return Err(AppError::NotFound("catégorie"));
    }
    let mut result = BulkCategorizeResult { categorized: 0, skipped: 0 };
    let mut tx = db.write.begin().await?;
    for id in &input.transaction_ids {
        let splits = db::transactions::splits_of(&mut *tx, *id).await?;
        let is_transfer = db::transactions::transfer_of(&mut *tx, *id).await?.is_some();
        match splits.as_slice() {
            [single] if !is_transfer => {
                db::transactions::set_split_category(&mut *tx, single.id, Some(input.category_id)).await?;
                result.categorized += 1;
            }
            _ => result.skipped += 1,
        }
    }
    tx.commit().await?;
    Ok(result)
}
