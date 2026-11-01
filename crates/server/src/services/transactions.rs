//! Transactions et ventilations (TRX-01 à TRX-03).

use std::collections::{HashMap, HashSet};

use cashflow_domain::accounts::AccountPeriod;
use cashflow_domain::budgeting::Month;
use cashflow_domain::household;
use cashflow_domain::ledger::balance::{self, DatedAmount};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::ledger::refund;
use cashflow_domain::ledger::split;
use cashflow_domain::ledger::{self, BalanceAffectingChange, ClearingStatus, TransactionOrigin};
use cashflow_domain::payees;
use cashflow_domain::shared::ids::{
    AccountId, AssetId, CategoryId, LoanId, MemberId, PayeeGroupId, PayeeId, ProjectId, ProjectLineId, SplitId,
    TransactionId, TransferId,
};
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
    /// Poste du projet où la ventilation est rangée ; absent : hors postes (R-BUD-27).
    pub project_line_id: Option<ProjectLineId>,
    /// Ventilation de remboursement d'un prêt (R-PRE-7).
    pub loan_id: Option<LoanId>,
    /// Ventilation d'acquisition d'un bien (R-BIE-8).
    pub asset_id: Option<AssetId>,
    /// Dépense que la ventilation rembourse (R-TRX-50).
    pub refunds_transaction_id: Option<TransactionId>,
    /// Membre que la ventilation concerne ; absent : celui du compte, à défaut le foyer (R-MBR-5, R-MBR-6).
    pub member_id: Option<MemberId>,
}

impl From<SplitRow> for Split {
    fn from(r: SplitRow) -> Self {
        Split {
            id: r.id,
            amount_minor: r.amount.minor(),
            category_id: r.category_id,
            note: r.note,
            project_id: r.project_id,
            project_line_id: r.project_line_id,
            loan_id: r.loan_id,
            asset_id: r.asset_id,
            refunds_transaction_id: r.refunds_transaction_id,
            member_id: r.member_id,
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
    pub payee_id: Option<PayeeId>,
    pub splits: Vec<Split>,
    pub transfer: Option<TransferSummary>,
    /// A au moins un justificatif (DOC-04).
    pub has_documents: bool,
    /// Mois d'analyse `AAAA-MM`, s'il diffère du mois de la date (R-TRX-57).
    pub analysis_month: Option<String>,
    /// Solde du compte après cette opération (TRX-02) ; absent dans la vue transverse (TRX-08).
    pub balance_after_minor: Option<i64>,
    /// Remboursements de frais (TRX-12) ; absent si la transaction n'en rembourse aucune, n'est remboursée par
    /// aucune et n'attend aucun remboursement.
    pub refund: Option<RefundBadge>,
}

/// Dépense remboursée par une entrée d'argent, vue depuis la liste.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RefundedExpense {
    pub transaction_id: TransactionId,
    pub date: Date,
    pub label: String,
}

/// Remboursements de frais d'une ligne de liste (TRX-12).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RefundBadge {
    /// Dépenses que cette entrée rembourse (R-TRX-50), dans l'ordre de leurs dates.
    pub refunds: Vec<RefundedExpense>,
    /// Remboursé de cette dépense (R-TRX-52).
    pub refunded_minor: i64,
    /// Remboursement attendu de cette dépense (R-TRX-53).
    pub expected_minor: Option<i64>,
    /// Reste à recevoir (R-TRX-53).
    pub pending_minor: i64,
    /// La dépense est remboursée en totalité (R-TRX-52).
    pub fully_refunded: bool,
}

/// Les liens de remboursement, rangés pour les listes : par entrée, et remboursé par dépense.
#[derive(Default)]
struct RefundLinks {
    by_income: HashMap<TransactionId, Vec<RefundedExpense>>,
    refunded: HashMap<TransactionId, Money>,
}

async fn refund_links(db: &Db) -> AppResult<RefundLinks> {
    let mut links = RefundLinks::default();
    for l in db::transactions::refund_links(&db.read).await? {
        *links.refunded.entry(l.expense_id).or_insert(Money::ZERO) += l.amount;
        let refunds = links.by_income.entry(l.transaction_id).or_default();
        if !refunds.iter().any(|r| r.transaction_id == l.expense_id) {
            refunds.push(RefundedExpense {
                transaction_id: l.expense_id,
                date: l.expense_date,
                label: l.expense_label,
            });
        }
    }
    Ok(links)
}

fn refund_badge(t: &db::transactions::TransactionRow, links: &mut RefundLinks) -> Option<RefundBadge> {
    let refunds = links.by_income.remove(&t.id).unwrap_or_default();
    let refunded = links.refunded.get(&t.id).copied().unwrap_or(Money::ZERO);
    (!refunds.is_empty() || refunded.is_positive() || t.expected_refund.is_some()).then(|| RefundBadge {
        refunds,
        refunded_minor: refunded.minor(),
        expected_minor: t.expected_refund.map(Money::minor),
        pending_minor: refund::pending(t.expected_refund, refunded).minor(),
        fully_refunded: t.amount.is_negative() && refunded >= -t.amount,
    })
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
    /// Tiers (spec 11), modifiable même rapprochée (R-TIE-12).
    pub payee_id: Option<PayeeId>,
    /// Mois d'analyse `AAAA-MM`, s'il diffère du mois de la date ; modifiable même rapprochée (R-TRX-57, R-TRX-3).
    pub analysis_month: Option<String>,
    pub splits: Vec<Split>,
    pub transfer: Option<TransferSummary>,
    /// Échéance de prêt payée par cette transaction (R-PRE-6).
    pub loan_payment: Option<LoanPaymentRef>,
    /// Rapprochée : montant, compte et date de solde sont verrouillés (R-TRX-3).
    pub locked: bool,
    /// Remboursements de frais, vus depuis la dépense ou depuis l'entrée (TRX-12).
    pub refund: super::refunds::RefundInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoanPaymentRef {
    pub loan_id: LoanId,
    pub loan_name: String,
    pub installment_number: u32,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SplitInput {
    pub amount_minor: i64,
    pub category_id: Option<CategoryId>,
    pub note: Option<String>,
    pub project_id: Option<ProjectId>,
    /// Poste de `project_id` ; absent : hors postes (R-BUD-27).
    #[serde(default)]
    pub project_line_id: Option<ProjectLineId>,
    /// Remboursement d'un prêt, à la place d'une catégorie (R-PRE-7).
    #[serde(default)]
    pub loan_id: Option<LoanId>,
    /// Acquisition d'un bien, à la place d'une catégorie (R-BIE-8).
    #[serde(default)]
    pub asset_id: Option<AssetId>,
    /// Dépense que la ventilation rembourse (R-TRX-50) : à renvoyer à chaque modification, comme la catégorie.
    #[serde(default)]
    pub refunds_transaction_id: Option<TransactionId>,
    /// Membre que la ventilation concerne (R-MBR-5) ; jamais sur une ventilation de virement.
    #[serde(default)]
    pub member_id: Option<MemberId>,
}

/// Saisie d'une transaction (TRX-01). Sans `splits`, une ventilation unique sur `category_id`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TransactionInput {
    pub account_id: AccountId,
    pub date: Date,
    pub amount_minor: i64,
    pub label: String,
    pub note: Option<String>,
    pub category_id: Option<CategoryId>,
    pub splits: Option<Vec<SplitInput>>,
    /// Tiers (R-TIE-10), jamais sur un virement (R-TIE-11).
    #[serde(default)]
    pub payee_id: Option<PayeeId>,
}

/// Modification d'une transaction. Sans `splits`, la ventilation existante est conservée ; une
/// ventilation unique suit alors le nouveau montant (R-TRX-4).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TransactionUpdateInput {
    pub account_id: AccountId,
    pub date: Date,
    pub amount_minor: i64,
    pub label: String,
    pub note: Option<String>,
    pub splits: Option<Vec<SplitInput>>,
    /// Le tiers fait partie de l'état complet : absent, la transaction n'a plus de tiers.
    #[serde(default)]
    pub payee_id: Option<PayeeId>,
}

/// Tiers affecté en masse (TIE-02) ; absent, le tiers est retiré.
#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkPayeeInput {
    pub transaction_ids: Vec<TransactionId>,
    pub payee_id: Option<PayeeId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkPayeeResult {
    pub updated: usize,
    /// Jambes de virement, laissées sans tiers (R-TIE-11).
    pub skipped: usize,
}

/// Membre affecté en masse (R-MBR-5) ; absent, les ventilations reviennent au membre du compte, à défaut au foyer.
#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkMemberInput {
    pub transaction_ids: Vec<TransactionId>,
    pub member_id: Option<MemberId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkMemberResult {
    pub updated: usize,
    /// Virements, laissés sans membre (R-MBR-5).
    pub skipped: usize,
}

/// Mois d'analyse de plusieurs transactions (TRX-13) ; absent, ou le mois de la date : le mois de la date.
#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkAnalysisMonthInput {
    pub transaction_ids: Vec<TransactionId>,
    /// `AAAA-MM`.
    pub analysis_month: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkAnalysisMonthResult {
    pub updated: usize,
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
    /// Tiers (TIE-04).
    pub payee_id: Option<PayeeId>,
    /// Tiers de ce groupe.
    pub payee_group_id: Option<PayeeGroupId>,
    /// Seulement les transactions sans tiers, hors virements.
    pub without_payee: Option<bool>,
    /// Avec (`true`) ou sans (`false`) justificatif (DOC-04).
    pub with_documents: Option<bool>,
    /// Mois d'analyse `AAAA-MM` : les transactions qui comptent dans ce mois, quelle que soit leur date (R-TRX-58).
    pub analysis_month: Option<String>,
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
    let mut links = refund_links(db).await?;

    // Le solde après opération se calcule sur toutes les transactions, avant filtrage.
    let movements: Vec<DatedAmount> =
        transactions.iter().map(|t| DatedAmount { balance_date: t.balance_date, amount: t.amount }).collect();
    let running = balance::running_balances(row.opening_balance, &movements);

    let selection = selection(db, filter).await?;

    let mut items: Vec<TransactionListItem> = transactions
        .into_iter()
        .zip(running)
        .filter_map(|(t, balance_after)| {
            let tx_splits = splits.remove(&t.id).unwrap_or_default();
            let keep = matches_filter(&t, &tx_splits, transfers.contains_key(&t.id), filter, &selection);
            keep.then(|| TransactionListItem {
                refund: refund_badge(&t, &mut links),
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
                payee_id: t.payee_id,
                splits: tx_splits,
                has_documents: selection.documented.contains(&t.id),
                analysis_month: t.analysis_month.map(|m| m.to_string()),
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
    let loan_payment = match db::loans::payment_of(&db.write, id).await? {
        Some(p) => Some(LoanPaymentRef {
            loan_id: p.loan_id,
            loan_name: db::loans::find(&db.write, p.loan_id).await?.map(|l| l.name).unwrap_or_default(),
            installment_number: p.installment_number,
        }),
        None => None,
    };
    let refund = super::refunds::info(&mut *db.write.acquire().await?, &t).await?;
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
        payee_id: t.payee_id,
        analysis_month: t.analysis_month.map(|m| m.to_string()),
        splits,
        transfer,
        loan_payment,
        refund,
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
        split::check_attachment(s.category_id.is_some(), s.loan_id.is_some(), s.asset_id.is_some())?;
        super::members::check_exists(conn, s.member_id).await?;
        if let Some(loan) = s.loan_id
            && db::loans::find(&mut *conn, loan).await?.is_none()
        {
            return Err(AppError::NotFound("prêt"));
        }
        if let Some(asset) = s.asset_id
            && db::assets::find(&mut *conn, asset).await?.is_none()
        {
            return Err(AppError::NotFound("bien"));
        }
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
        match s.project_id {
            Some(project) if s.project_line_id.is_some() => {
                super::budgets::LineProposer::load(conn, project).await?.check(s.project_line_id)?;
            }
            None if s.project_line_id.is_some() => return Err(DomainError::ProjectLineMismatch.into()),
            _ => {}
        }
    }
    Ok(())
}

/// Le tiers cité existe (R-TIE-10).
pub(crate) async fn check_payee(conn: &mut SqliteConnection, payee: Option<PayeeId>) -> AppResult<()> {
    if let Some(payee) = payee
        && db::payees::find(&mut *conn, payee).await?.is_none()
    {
        return Err(AppError::NotFound("tiers"));
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
            project_line_id: s.project_line_id,
            loan_id: s.loan_id,
            asset_id: s.asset_id,
            refunds_transaction_id: s.refunds_transaction_id,
            member_id: s.member_id,
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
    let mut tx = db.write.begin().await?;
    // Sans ventilation saisie, une seule, au titulaire du compte (R-MBR-6) ; saisies, elles portent leur membre.
    let splits = match input.splits.clone() {
        Some(splits) => splits,
        None => vec![SplitInput {
            amount_minor: input.amount_minor,
            category_id: input.category_id,
            member_id: holder(&mut tx, input.account_id).await?,
            ..Default::default()
        }],
    };
    split::check_splits(amount, &amounts(&splits))?;

    let account = db::accounts::find(&mut *tx, input.account_id).await?.ok_or(AppError::NotFound("compte"))?;
    period(&account).check(input.date)?;
    reconciliation::lock(&mut tx, input.account_id).await?.check(input.date)?;
    check_references(&mut tx, &splits, &[]).await?;
    check_payee(&mut tx, input.payee_id).await?;
    let id = TransactionId::new();
    super::refunds::check_splits(&mut tx, id, &splits).await?;
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
        payee_id: input.payee_id,
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
    payees::check_transfer_leg(input.payee_id.is_some(), transfer.is_some())?;
    check_payee(&mut tx, input.payee_id).await?;
    let existing = db::transactions::splits_of(&mut *tx, id).await?;
    // Une mensualité ventilée selon son échéance, et la jambe du capital sur le prêt, ne changent ni de montant,
    // ni de compte, ni de ventilation (R-CPT-17) : seuls le libellé, la note et la date se modifient.
    if db::loans::payment_of(&mut *tx, id).await?.is_some() {
        let same_splits = input.splits.as_ref().is_none_or(|splits| {
            splits.len() == existing.len()
                && splits
                    .iter()
                    .zip(&existing)
                    .all(|(new, old)| new.amount_minor == old.amount.minor() && new.category_id == old.category_id)
        });
        if amount != current.amount || input.account_id != current.account_id || !same_splits {
            return Err(DomainError::InstallmentPaymentLocked.into());
        }
        let note = clean(input.note.as_deref());
        let update = TransactionUpdate {
            account_id: current.account_id,
            date: input.date,
            bank_date: current.bank_date,
            value_date: current.value_date,
            amount,
            label: &label,
            note: note.as_deref(),
            clearing_status: current.clearing_status,
            payee_id: input.payee_id,
        };
        db::transactions::update(&mut *tx, id, &update).await?;
        tx.commit().await?;
        return Ok(());
    }
    let splits: Vec<SplitInput> = match &input.splits {
        Some(splits) => splits.clone(),
        None if existing.len() == 1 => vec![SplitInput {
            amount_minor: input.amount_minor,
            category_id: existing[0].category_id,
            note: existing[0].note.clone(),
            project_id: existing[0].project_id,
            project_line_id: existing[0].project_line_id,
            loan_id: existing[0].loan_id,
            asset_id: existing[0].asset_id,
            refunds_transaction_id: existing[0].refunds_transaction_id,
            member_id: existing[0].member_id,
        }],
        None => existing
            .iter()
            .map(|s| SplitInput {
                amount_minor: s.amount.minor(),
                category_id: s.category_id,
                note: s.note.clone(),
                project_id: s.project_id,
                project_line_id: s.project_line_id,
                loan_id: s.loan_id,
                asset_id: s.asset_id,
                refunds_transaction_id: s.refunds_transaction_id,
                member_id: s.member_id,
            })
            .collect(),
    };
    split::check_splits(amount, &amounts(&splits))?;
    let kept: Vec<ProjectId> = existing.iter().filter_map(|s| s.project_id).collect();
    check_references(&mut tx, &splits, &kept).await?;
    super::refunds::check_splits(&mut tx, id, &splits).await?;
    if amount != current.amount {
        super::refunds::check_expense(&mut tx, &current, amount).await?;
    }

    let mut touched = vec![id];
    if let Some(side) = &transfer {
        if splits.len() != 1 || splits[0].category_id.is_some() {
            return Err(DomainError::TransferLegCategorized.into());
        }
        household::check_split_member(splits[0].member_id.is_some(), true)?;
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
                payee_id: counterpart.payee_id,
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
        payee_id: input.payee_id,
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

/// Supprime une transaction. Une jambe de virement supprimée délie l'autre, qui reste (R-TRX-34) ; une mensualité
/// supprimée libère son échéance (R-PRE-8). Ses justificatifs restent, sauf ceux rattachés seulement à elle si
/// `delete_documents` (R-DOC-12).
pub async fn delete(
    db: &Db,
    data: &crate::config::DataDir,
    id: TransactionId,
    delete_documents: bool,
    today: Date,
) -> AppResult<()> {
    let current = db::transactions::find(&db.read, id).await?.ok_or(AppError::NotFound("transaction"))?;
    if current.clearing_status == ClearingStatus::Reconciled {
        return Err(DomainError::TransactionLocked.into());
    }
    let mut tx = db.write.begin().await?;
    reconciliation::lock(&mut tx, current.account_id).await?.check(current.balance_date)?;
    let documents =
        if delete_documents { super::documents::delete_exclusive(&mut tx, *id.as_uuid()).await? } else { Vec::new() };
    db::transactions::delete(&mut *tx, id).await?;
    tx.commit().await?;
    super::documents::trash(data, &documents, today);
    Ok(())
}

/// Catégorise une ventilation sur place, depuis l'accueil (`ecrans.md` E1).
pub async fn categorize_split(db: &Db, split_id: SplitId, input: &CategorizeInput) -> AppResult<()> {
    let split = db::transactions::find_split(&db.read, split_id).await?.ok_or(AppError::NotFound("ventilation"))?;
    // Seule une ventilation de virement reste sans catégorie (R-TRX-32, ADR-0021).
    if db::transactions::transfer_of(&db.read, split.transaction_id)
        .await?
        .is_some_and(|side| side.split_id == split_id)
    {
        return Err(DomainError::TransferLegCategorized.into());
    }
    let kind = match input.category_id {
        Some(category) => {
            Some(db::categories::find(&db.read, category).await?.ok_or(AppError::NotFound("catégorie"))?.kind)
        }
        None => None,
    };
    // Une ventilation qui rembourse reste sur une catégorie de dépense (R-TRX-50).
    if split.refunds_transaction_id.is_some() && kind != Some(CategoryKind::Expense) {
        return Err(DomainError::InvalidRefund.into());
    }
    db::transactions::set_split_category(&db.write, split_id, input.category_id).await?;
    Ok(())
}

/// Membre par défaut d'une ventilation créée : le titulaire du compte, `None` (foyer) sans titulaire (R-MBR-6).
pub(crate) async fn holder(conn: &mut SqliteConnection, account: AccountId) -> AppResult<Option<MemberId>> {
    Ok(db::accounts::find(&mut *conn, account).await?.and_then(|a| a.member_id))
}

/// Membre des ventilations recalculées d'une transaction (règles, échéance de prêt) : celui commun aux anciennes,
/// à défaut le titulaire du compte (R-MBR-5, R-MBR-6).
pub(crate) async fn kept_member(
    conn: &mut SqliteConnection,
    transaction: TransactionId,
    account: AccountId,
) -> AppResult<Option<MemberId>> {
    let previous: Vec<Option<MemberId>> =
        db::transactions::splits_of(&mut *conn, transaction).await?.into_iter().map(|s| s.member_id).collect();
    match household::common_member(&previous) {
        Some(member) => Ok(Some(member)),
        None => holder(conn, account).await,
    }
}

/// Insère une transaction créée par l'application (import, récurrence, intérêts) : ses ventilations sans membre
/// reçoivent le titulaire du compte (R-MBR-6). Pas pour une jambe de virement, qui n'a jamais de membre.
pub(crate) async fn insert_for_holder(
    conn: &mut SqliteConnection,
    t: &NewTransaction<'_>,
    splits: &[NewSplit<'_>],
) -> AppResult<()> {
    let member = holder(conn, t.account_id).await?;
    let splits: Vec<NewSplit<'_>> =
        splits.iter().map(|s| NewSplit { member_id: s.member_id.or(member), ..s.clone() }).collect();
    insert_validated(conn, t, &splits).await
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

/// Ce que les filtres d'une liste demandent de charger : catégories et tiers visés, texte cherché, transactions
/// qui ont un justificatif (DOC-04).
struct Selection {
    category_ids: Option<HashSet<CategoryId>>,
    payee_ids: Option<HashSet<PayeeId>>,
    text: Option<String>,
    documented: HashSet<TransactionId>,
    analysis_month: Option<Month>,
}

async fn selection(db: &Db, filter: &TransactionFilter) -> AppResult<Selection> {
    let documented = db::documents::transactions_with_documents(&db.read).await?;
    Ok(Selection {
        category_ids: category_family(db, filter.category_id).await?,
        payee_ids: payee_selection(db, filter).await?,
        text: clean(filter.text.as_deref()).map(|t| t.to_lowercase()),
        documented: documented.iter().filter_map(|id| id.parse().ok()).collect(),
        analysis_month: filter.analysis_month.as_deref().map(parse_month).transpose()?,
    })
}

/// Filtre commun aux listes de transactions.
fn matches_filter(
    t: &crate::db::transactions::TransactionRow,
    splits: &[Split],
    is_transfer: bool,
    filter: &TransactionFilter,
    selection: &Selection,
) -> bool {
    let (category_ids, payee_ids, text) =
        (selection.category_ids.as_ref(), selection.payee_ids.as_ref(), selection.text.as_deref());
    let has_documents = selection.documented.contains(&t.id);
    filter.from.is_none_or(|from| t.date >= from)
        && filter.to.is_none_or(|to| t.date <= to)
        && filter.status.is_none_or(|status| t.clearing_status == status)
        && filter.min_amount_minor.is_none_or(|min| t.amount.minor().abs() >= min)
        && filter.max_amount_minor.is_none_or(|max| t.amount.minor().abs() <= max)
        && category_ids.is_none_or(|ids| splits.iter().any(|s| s.category_id.is_some_and(|c| ids.contains(&c))))
        && (!filter.uncategorized.unwrap_or(false) || (!is_transfer && splits.iter().any(|s| s.category_id.is_none())))
        && payee_ids.is_none_or(|ids| t.payee_id.is_some_and(|p| ids.contains(&p)))
        && (!filter.without_payee.unwrap_or(false) || (!is_transfer && t.payee_id.is_none()))
        && filter.with_documents.is_none_or(|wanted| has_documents == wanted)
        && selection.analysis_month.is_none_or(|month| ledger::analysis_month(t.date, t.analysis_month) == month)
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

/// Tiers retenus par le filtre : le tiers choisi, ou les tiers du groupe choisi (TIE-04).
async fn payee_selection(db: &Db, filter: &TransactionFilter) -> AppResult<Option<HashSet<PayeeId>>> {
    let mut selection: Option<HashSet<PayeeId>> = filter.payee_id.map(|p| HashSet::from([p]));
    if let Some(group) = filter.payee_group_id {
        let members: HashSet<PayeeId> =
            db::payees::list(&db.read).await?.into_iter().filter(|p| p.group_id == Some(group)).map(|p| p.id).collect();
        selection = Some(match selection {
            Some(chosen) => chosen.intersection(&members).copied().collect(),
            None => members,
        });
    }
    Ok(selection)
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
    let selection = selection(db, filter).await?;
    let mut links = refund_links(db).await?;
    let mut items: Vec<TransactionListItem> = transactions
        .into_iter()
        .filter(|t| wanted.as_ref().is_none_or(|w| w.contains(&t.account_id)))
        .filter_map(|t| {
            let tx_splits = splits.remove(&t.id).unwrap_or_default();
            let side = sides.get(&t.id);
            let keep = matches_filter(&t, &tx_splits, side.is_some(), filter, &selection);
            keep.then(|| TransactionListItem {
                refund: refund_badge(&t, &mut links),
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
                payee_id: t.payee_id,
                splits: tx_splits,
                has_documents: selection.documented.contains(&t.id),
                analysis_month: t.analysis_month.map(|m| m.to_string()),
                transfer: side.map(|s| transfer_summary(s, &accounts)),
                balance_after_minor: None,
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
/// hors virements, sont modifiées ; une ventilation qui rembourse ne passe pas sur une catégorie de revenu
/// (R-TRX-50).
pub async fn categorize_many(db: &Db, input: &BulkCategorizeInput) -> AppResult<BulkCategorizeResult> {
    let category = db::categories::find(&db.read, input.category_id).await?.ok_or(AppError::NotFound("catégorie"))?;
    let expense = category.kind == CategoryKind::Expense;
    let mut result = BulkCategorizeResult { categorized: 0, skipped: 0 };
    let mut tx = db.write.begin().await?;
    for id in &input.transaction_ids {
        let splits = db::transactions::splits_of(&mut *tx, *id).await?;
        let is_transfer = db::transactions::transfer_of(&mut *tx, *id).await?.is_some();
        match splits.as_slice() {
            [single] if !is_transfer && (expense || single.refunds_transaction_id.is_none()) => {
                db::transactions::set_split_category(&mut *tx, single.id, Some(input.category_id)).await?;
                result.categorized += 1;
            }
            _ => result.skipped += 1,
        }
    }
    tx.commit().await?;
    Ok(result)
}

/// Affecte (ou retire) un tiers à plusieurs transactions (TIE-02). Les jambes de virement sont laissées sans tiers
/// (R-TIE-11) ; une transaction rapprochée change de tiers sans déverrouillage (R-TIE-12).
pub async fn set_payee_many(db: &Db, input: &BulkPayeeInput) -> AppResult<BulkPayeeResult> {
    let mut result = BulkPayeeResult { updated: 0, skipped: 0 };
    let mut tx = db.write.begin().await?;
    check_payee(&mut tx, input.payee_id).await?;
    for id in &input.transaction_ids {
        if db::transactions::find(&mut *tx, *id).await?.is_none() {
            return Err(AppError::NotFound("transaction"));
        }
        let is_transfer = db::transactions::transfer_of(&mut *tx, *id).await?.is_some();
        if payees::check_transfer_leg(input.payee_id.is_some(), is_transfer).is_err() {
            result.skipped += 1;
            continue;
        }
        db::transactions::set_payee(&mut *tx, *id, input.payee_id).await?;
        result.updated += 1;
    }
    tx.commit().await?;
    Ok(result)
}

/// Affecte (ou retire) un membre à toutes les ventilations de plusieurs transactions (R-MBR-5). Les virements sont
/// laissés tels quels ; une transaction rapprochée change de membre sans déverrouillage (R-TRX-3).
pub async fn set_member_many(db: &Db, input: &BulkMemberInput) -> AppResult<BulkMemberResult> {
    let mut result = BulkMemberResult { updated: 0, skipped: 0 };
    let mut tx = db.write.begin().await?;
    super::members::check_exists(&mut tx, input.member_id).await?;
    for id in &input.transaction_ids {
        if db::transactions::find(&mut *tx, *id).await?.is_none() {
            return Err(AppError::NotFound("transaction"));
        }
        let is_transfer = db::transactions::transfer_of(&mut *tx, *id).await?.is_some();
        // Une ventilation de virement n'a jamais de membre (R-MBR-5) : la transaction est laissée telle quelle.
        if is_transfer {
            result.skipped += 1;
            continue;
        }
        db::members::set_transaction_member(&mut *tx, *id, input.member_id).await?;
        result.updated += 1;
    }
    tx.commit().await?;
    Ok(result)
}

fn parse_month(value: &str) -> AppResult<Month> {
    value
        .parse()
        .map_err(|_| AppError::invalid_field("INVALID_MONTH", "analysis_month", "mois attendu au format AAAA-MM"))
}

/// Fixe le mois d'analyse de transactions (TRX-13, R-TRX-57) ; il ne touche pas au solde, même rapprochées (R-TRX-3).
pub async fn set_analysis_month(db: &Db, input: &BulkAnalysisMonthInput) -> AppResult<BulkAnalysisMonthResult> {
    let month = input.analysis_month.as_deref().map(parse_month).transpose()?;
    let mut tx = db.write.begin().await?;
    for id in &input.transaction_ids {
        if db::transactions::find(&mut *tx, *id).await?.is_none() {
            return Err(AppError::NotFound("transaction"));
        }
        db::transactions::set_analysis_month(&mut *tx, *id, month).await?;
    }
    tx.commit().await?;
    Ok(BulkAnalysisMonthResult { updated: input.transaction_ids.len() })
}
