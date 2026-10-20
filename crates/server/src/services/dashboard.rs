//! Accueil (TDB-01) : patrimoine, mois en cours, comptes, ventilations à catégoriser.

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::reporting::{self, AccountValue, Flows, Period, SplitFact};
use cashflow_domain::shared::ids::{AccountId, SplitId, TransactionId};
use cashflow_domain::{Currency, Date, Money};
use serde::Serialize;
use utoipa::ToSchema;

use super::accounts::{self, Account};
use crate::db::{self, Db};
use crate::error::AppResult;

/// Nombre de ventilations à catégoriser affichées sur l'accueil.
const UNCATEGORIZED_SHOWN: i64 = 5;

#[derive(Debug, Serialize, ToSchema)]
pub struct NetWorthView {
    pub assets_minor: i64,
    pub liabilities_minor: i64,
    pub total_minor: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlowsView {
    pub from: Date,
    pub to: Date,
    pub income_minor: i64,
    pub expenses_minor: i64,
    pub savings_minor: i64,
    /// Non défini si les revenus valent 0 (R-TDB-7).
    pub savings_rate_ppm: Option<i64>,
    pub uncategorized_in_minor: i64,
    pub uncategorized_out_minor: i64,
}

fn flows_view(period: Period, flows: Flows) -> FlowsView {
    FlowsView {
        from: period.from,
        to: period.to,
        income_minor: flows.income.minor(),
        expenses_minor: flows.expenses.minor(),
        savings_minor: flows.savings.minor(),
        savings_rate_ppm: flows.savings_rate.map(|r| r.ppm()),
        uncategorized_in_minor: flows.uncategorized_in.minor(),
        uncategorized_out_minor: flows.uncategorized_out.minor(),
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FamilyGroup {
    pub family: AccountFamily,
    /// Total des comptes en euros du groupe (R-CPT-2).
    pub total_minor: i64,
    pub accounts: Vec<Account>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UncategorizedItem {
    pub split_id: SplitId,
    pub transaction_id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    /// La transaction a plusieurs ventilations.
    pub is_split: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UncategorizedView {
    pub total: i64,
    pub items: Vec<UncategorizedItem>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Dashboard {
    pub today: Date,
    /// R-TDB-1.
    pub net_worth: NetWorthView,
    /// Patrimoine à la fin du mois précédent (R-TDB-2).
    pub previous_net_worth_minor: i64,
    pub net_worth_variation_minor: i64,
    /// Part de la variation qui ne vient pas de l'épargne du mois : marchés et intérêts (R-TDB-2).
    pub market_effect_minor: i64,
    /// Mois en cours, jusqu'à aujourd'hui (R-TDB-13).
    pub month: FlowsView,
    /// Mois précédent, à la même date.
    pub previous_month: FlowsView,
    pub families: Vec<FamilyGroup>,
    pub uncategorized: UncategorizedView,
    /// Comptes qui demandent une action : écart de rapprochement ou transactions en attente anciennes (R-TDB-10).
    pub alerts: Vec<super::reconciliation::AccountHealth>,
}

fn values(rows: &[db::accounts::AccountRow], ledger: &accounts::Ledger, at: Date) -> Vec<AccountValue> {
    rows.iter()
        .filter(|row| row.opening_date <= at)
        .map(|row| {
            let (value, _) = ledger.value(row, at);
            // Un compte clôturé compte dans le patrimoine jusqu'à sa date de clôture (CPT-04).
            let open = row.closed_on.is_none_or(|closed| closed >= at);
            AccountValue {
                family: row.kind.family(),
                include_in_net_worth: row.include_in_net_worth && open,
                is_eur: row.currency == Currency::EUR,
                value,
            }
        })
        .collect()
}

pub async fn get(db: &Db, today: Date) -> AppResult<Dashboard> {
    let accounts = accounts::list(db, today).await?;
    let rows = db::accounts::list(&db.read).await?;
    let ledger = accounts::Ledger::load(db).await?;

    let net_worth = reporting::net_worth(&values(&rows, &ledger, today));
    let previous_end = reporting::end_of_previous_month(today);
    let previous = reporting::net_worth(&values(&rows, &ledger, previous_end));

    let (current, previous_period) = reporting::month_to_date(today);
    let facts: Vec<SplitFact> = db::transactions::split_facts(&db.read, previous_period.from, current.to)
        .await?
        .into_iter()
        .map(|f| SplitFact {
            date: f.date,
            amount: f.amount,
            category_kind: f.category_kind,
            is_transfer: f.is_transfer,
            is_eur: f.currency == Currency::EUR.code(),
        })
        .collect();
    let month = reporting::flows(&facts, current.from, current.to);
    let previous_month = reporting::flows(&facts, previous_period.from, previous_period.to);

    let variation = net_worth.total - previous.total;
    let families = AccountFamily::ALL
        .into_iter()
        .map(|family| {
            let members: Vec<Account> =
                accounts.iter().filter(|a| a.family == family && a.closed_on.is_none()).cloned().collect();
            let total: Money = members
                .iter()
                .filter(|a| a.currency == Currency::EUR.code())
                .map(|a| Money::from_minor(a.balance_minor))
                .sum();
            FamilyGroup { family, total_minor: total.minor(), accounts: members }
        })
        .filter(|g| !g.accounts.is_empty())
        .collect();

    let (items, total) = db::transactions::uncategorized(&db.read, UNCATEGORIZED_SHOWN).await?;
    let uncategorized = UncategorizedView {
        total,
        items: items
            .into_iter()
            .map(|u| UncategorizedItem {
                split_id: u.split_id,
                transaction_id: u.transaction_id,
                account_id: u.account_id,
                account_name: u.account_name,
                date: u.date,
                label: u.label,
                amount_minor: u.amount.minor(),
                is_split: u.split_count > 1,
            })
            .collect(),
    };

    Ok(Dashboard {
        today,
        net_worth: NetWorthView {
            assets_minor: net_worth.assets.minor(),
            liabilities_minor: net_worth.liabilities.minor(),
            total_minor: net_worth.total.minor(),
        },
        previous_net_worth_minor: previous.total.minor(),
        net_worth_variation_minor: variation.minor(),
        market_effect_minor: (variation - month.savings).minor(),
        month: flows_view(current, month),
        previous_month: flows_view(previous_period, previous_month),
        families,
        uncategorized,
        alerts: super::reconciliation::health(db, today)
            .await?
            .into_iter()
            .filter(|h| h.old_pending > 0 || h.current_gap_minor.is_some_and(|g| g != 0))
            .collect(),
    })
}
