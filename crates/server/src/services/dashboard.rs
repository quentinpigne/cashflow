//! Accueil (TDB-01) : patrimoine, mois en cours, comptes, ventilations à catégoriser.

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::ledger::ClearingStatus;
use cashflow_domain::reporting::{self, Flows, HoldingValue, Period, SplitFact};
use cashflow_domain::shared::ids::{AccountId, CategoryId, SplitId, TransactionId};
use cashflow_domain::{Currency, Date, Money};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::accounts::{self, Account};
use super::assets::{AssetBook, AssetSummary};
use super::loans::{LoanBook, LoanSummary};
use crate::db::{self, Db};
use crate::error::AppResult;

/// Nombre de ventilations à catégoriser affichées sur l'accueil.
const UNCATEGORIZED_SHOWN: i64 = 5;

#[derive(Debug, Serialize, ToSchema)]
pub struct NetWorthView {
    /// Actif brut : comptes et biens.
    pub assets_minor: i64,
    /// Dettes : prêts et comptes à découvert, négatives.
    pub liabilities_minor: i64,
    pub total_minor: i64,
    /// Dont valeur des biens (R-BIE-2).
    pub asset_values_minor: i64,
    /// Dont capital restant dû des prêts (R-PRE-4), positif.
    pub loans_outstanding_minor: i64,
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
    /// Biens possédés, si la rubrique est activée (TDB-08).
    pub assets: Vec<AssetSummary>,
    /// Prêts en cours, si la rubrique est activée (TDB-08).
    pub loans: Vec<LoanSummary>,
    pub uncategorized: UncategorizedView,
    /// Comptes qui demandent une action : écart de rapprochement ou transactions en attente anciennes (R-TDB-10).
    pub alerts: Vec<super::reconciliation::AccountHealth>,
    /// Écarts de salaire du dernier mois complet, si la rubrique est activée (R-SAL-9).
    pub salary_gaps: Vec<super::salaries::SalaryGapAlert>,
}

/// Ce qui compte dans le patrimoine à une date (R-TDB-1) : comptes, biens possédés, prêts en cours.
pub(crate) fn holdings(
    rows: &[db::accounts::AccountRow],
    ledger: &accounts::Ledger,
    assets: &AssetBook,
    loans: &LoanBook,
    at: Date,
) -> Vec<HoldingValue> {
    let accounts = rows.iter().filter(|row| row.opening_date <= at).map(|row| {
        let (value, _) = ledger.value(row, at);
        // Un compte clôturé compte dans le patrimoine jusqu'à sa date de clôture (CPT-04).
        let open = row.closed_on.is_none_or(|closed| closed >= at);
        HoldingValue {
            include_in_net_worth: row.include_in_net_worth && open,
            is_eur: row.currency == Currency::EUR,
            value,
        }
    });
    let goods = assets.assets.iter().filter_map(|a| {
        assets.value(a, at).map(|value| HoldingValue {
            include_in_net_worth: a.include_in_net_worth,
            is_eur: true,
            value,
        })
    });
    let debts = loans.loans.iter().filter_map(|l| {
        loans.outstanding(l, at).map(|debt| HoldingValue {
            include_in_net_worth: l.include_in_net_worth,
            is_eur: true,
            value: -debt,
        })
    });
    accounts.chain(goods).chain(debts).collect()
}

pub async fn get(db: &Db, today: Date) -> AppResult<Dashboard> {
    let accounts = accounts::list(db, today).await?;
    let rows = db::accounts::list(&db.read).await?;
    let ledger = accounts::Ledger::load(db).await?;
    let asset_book = AssetBook::load(db).await?;
    let loan_book = LoanBook::load(db).await?;

    let net_worth = reporting::net_worth(&holdings(&rows, &ledger, &asset_book, &loan_book, today));
    let previous_end = reporting::end_of_previous_month(today);
    let previous = reporting::net_worth(&holdings(&rows, &ledger, &asset_book, &loan_book, previous_end));
    let asset_values: Money =
        asset_book.assets.iter().filter(|a| a.include_in_net_worth).filter_map(|a| asset_book.value(a, today)).sum();
    let loans_outstanding: Money =
        loan_book.loans.iter().filter(|l| l.include_in_net_worth).filter_map(|l| loan_book.outstanding(l, today)).sum();
    let features = super::features::get(db).await?;
    let assets = if features.assets {
        super::assets::list(db, today).await?.into_iter().filter(|a| a.value_minor.is_some()).collect()
    } else {
        Vec::new()
    };
    let salary_gaps = if features.salaries { super::salaries::alerts(db, today).await? } else { Vec::new() };
    let loans = if features.loans {
        super::loans::list(db, today).await?.into_iter().filter(|l| l.outstanding_minor.is_some()).collect()
    } else {
        Vec::new()
    };

    let (current, previous_period) = reporting::month_to_date(today);
    let facts: Vec<SplitFact> = db::transactions::split_facts(&db.read, previous_period.from, current.to)
        .await?
        .into_iter()
        .map(|f| SplitFact {
            date: f.date,
            amount: f.amount,
            category_kind: f.category_kind,
            is_movement: f.is_movement,
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
            asset_values_minor: asset_values.minor(),
            loans_outstanding_minor: loans_outstanding.minor(),
        },
        previous_net_worth_minor: previous.total.minor(),
        net_worth_variation_minor: variation.minor(),
        market_effect_minor: (variation - month.savings).minor(),
        month: flows_view(current, month),
        previous_month: flows_view(previous_period, previous_month),
        families,
        assets,
        loans,
        uncategorized,
        alerts: super::reconciliation::health(db, today)
            .await?
            .into_iter()
            .filter(|h| h.old_pending > 0 || h.current_gap_minor.is_some_and(|g| g != 0))
            .collect(),
        salary_gaps,
    })
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RecentQuery {
    /// Nombre de transactions (10 par défaut, 50 au plus).
    pub limit: Option<i64>,
}

/// Une transaction récemment saisie (TDB-01).
#[derive(Debug, Serialize, ToSchema)]
pub struct RecentTransaction {
    pub id: TransactionId,
    pub account_id: AccountId,
    pub account_name: String,
    pub currency: String,
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub clearing_status: ClearingStatus,
    /// Catégorie de l'unique ventilation, si elle en a une.
    pub category_id: Option<CategoryId>,
    pub split_count: i64,
    /// Compte de l'autre jambe, pour un virement interne.
    pub counterpart_account_name: Option<String>,
}

/// Les dernières transactions saisies, la plus récente d'abord, pour y revenir depuis l'accueil.
pub async fn recent(db: &Db, query: &RecentQuery) -> AppResult<Vec<RecentTransaction>> {
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    Ok(db::transactions::recent(&db.read, limit)
        .await?
        .into_iter()
        .map(|r| RecentTransaction {
            id: r.id,
            account_id: r.account_id,
            account_name: r.account_name,
            currency: r.currency,
            date: r.date,
            label: r.label,
            amount_minor: r.amount.minor(),
            clearing_status: r.clearing_status,
            category_id: r.category_id,
            split_count: r.split_count,
            counterpart_account_name: r.counterpart_account_name,
        })
        .collect())
}
