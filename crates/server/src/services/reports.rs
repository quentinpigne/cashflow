//! Analyses (spec 08) : dépenses par catégorie (TDB-02), évolution (TDB-03).

use std::collections::HashMap;

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::reporting::{self, CategorizedFact, Period, SplitFact};
use cashflow_domain::shared::ids::CategoryId;
use cashflow_domain::{Currency, Date, Money};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Nombre de mois complets de la moyenne (R-TDB-8, Q8).
const AVERAGE_MONTHS: u32 = 12;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CategoryReportQuery {
    /// Mois analysé, `AAAA-MM` ; le mois en cours par défaut.
    pub month: Option<String>,
    /// `EXPENSE` par défaut.
    pub kind: Option<CategoryKind>,
    /// Retire les ventilations rattachées à un projet (TDB-07) ; incluses par défaut.
    pub exclude_projects: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CategoryLine {
    /// Absent : non catégorisé.
    pub category_id: Option<CategoryId>,
    pub name: String,
    pub amount_minor: i64,
    /// Mois précédent (à la même date si le mois analysé est en cours, R-TDB-13).
    pub previous_minor: i64,
    /// Moyenne des 12 mois complets précédents.
    pub average_minor: i64,
    #[schema(no_recursion)]
    pub children: Vec<CategoryLine>,
}

/// Dépenses (ou revenus) par catégorie d'un mois, comparés au mois précédent et à la moyenne (TDB-02).
#[derive(Debug, Serialize, ToSchema)]
pub struct CategoryReport {
    pub kind: CategoryKind,
    pub from: Date,
    pub to: Date,
    pub current_month: bool,
    pub previous_from: Date,
    pub previous_to: Date,
    pub total_minor: i64,
    pub previous_total_minor: i64,
    pub average_total_minor: i64,
    pub lines: Vec<CategoryLine>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EvolutionQuery {
    /// 12 ou 24 (12 par défaut).
    pub months: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MonthFlows {
    /// `AAAA-MM`.
    pub month: String,
    pub from: Date,
    pub to: Date,
    pub income_minor: i64,
    pub expenses_minor: i64,
    pub savings_minor: i64,
    pub savings_rate_ppm: Option<i64>,
    /// Taux d'épargne sur les 12 mois glissants qui finissent ce mois-ci (R-TDB-7).
    pub rolling_rate_ppm: Option<i64>,
    pub current_month: bool,
}

/// Revenus, dépenses, épargne et taux d'épargne mois par mois (TDB-03).
#[derive(Debug, Serialize, ToSchema)]
pub struct Evolution {
    pub months: Vec<MonthFlows>,
}

fn parse_month(month: &str) -> AppResult<Date> {
    let date: Date = format!("{month}-01")
        .parse()
        .map_err(|_| AppError::invalid_field("INVALID_MONTH", "month", "mois attendu au format AAAA-MM"))?;
    Ok(date)
}

pub async fn categories(db: &Db, query: &CategoryReportQuery, today: Date) -> AppResult<CategoryReport> {
    let kind = query.kind.unwrap_or(CategoryKind::Expense);
    let start = match &query.month {
        Some(month) => parse_month(month)?,
        None => reporting::first_of_month(today),
    };
    let current_month = start == reporting::first_of_month(today);
    let (period, previous) = if current_month {
        reporting::month_to_date(today)
    } else {
        let previous_start = start.checked_sub_months(chrono::Months::new(1)).unwrap_or(start);
        (
            Period { from: start, to: reporting::last_of_month(start) },
            Period { from: previous_start, to: reporting::last_of_month(previous_start) },
        )
    };
    let averages = reporting::months_until(start.pred_opt().unwrap_or(start), AVERAGE_MONTHS);
    let earliest = averages.first().map_or(previous.from, |p| p.from.min(previous.from));
    let facts: Vec<CategorizedFact<CategoryId>> = db::transactions::categorized_facts(&db.read, earliest, period.to)
        .await?
        .into_iter()
        .filter(|f| !(query.exclude_projects.unwrap_or(false) && f.has_project))
        .map(|f| CategorizedFact {
            date: f.date,
            amount: f.amount,
            category: f.category_id,
            kind: f.category_kind,
            is_transfer: f.is_transfer,
            is_eur: f.currency == Currency::EUR.code(),
        })
        .collect();
    let current = reporting::by_category(&facts, kind, period);
    let before = reporting::by_category(&facts, kind, previous);
    let history: Vec<HashMap<Option<CategoryId>, Money>> =
        averages.iter().map(|p| reporting::by_category(&facts, kind, *p)).collect();

    let categories = db::categories::list(&db.read).await?;
    let parent: HashMap<CategoryId, Option<CategoryId>> = categories.iter().map(|c| (c.id, c.parent_id)).collect();
    let names: HashMap<CategoryId, String> = categories.iter().map(|c| (c.id, c.name.clone())).collect();
    let average_of = |key: Option<CategoryId>| {
        reporting::average(&history.iter().map(|h| h.get(&key).copied().unwrap_or(Money::ZERO)).collect::<Vec<_>>())
    };
    let line = |key: Option<CategoryId>| CategoryLine {
        category_id: key,
        name: key.and_then(|k| names.get(&k).cloned()).unwrap_or_else(|| "Non catégorisé".into()),
        amount_minor: current.get(&key).copied().unwrap_or(Money::ZERO).minor(),
        previous_minor: before.get(&key).copied().unwrap_or(Money::ZERO).minor(),
        average_minor: average_of(key).minor(),
        children: Vec::new(),
    };

    // Toutes les catégories qui ont un montant sur l'une des périodes.
    let mut keys: Vec<Option<CategoryId>> =
        current.keys().chain(before.keys()).chain(history.iter().flat_map(|h| h.keys())).copied().collect();
    keys.sort();
    keys.dedup();
    // Chaque catégorie remonte dans sa racine, qui cumule ses sous-catégories (R-TRX-21).
    let mut roots: HashMap<Option<CategoryId>, CategoryLine> = HashMap::new();
    for key in &keys {
        let root = key.map(|id| parent.get(&id).copied().flatten().unwrap_or(id));
        let entry = roots.entry(root).or_insert_with(|| CategoryLine {
            amount_minor: 0,
            previous_minor: 0,
            average_minor: 0,
            ..line(root)
        });
        let own = line(*key);
        entry.amount_minor += own.amount_minor;
        entry.previous_minor += own.previous_minor;
        entry.average_minor += own.average_minor;
        if root != *key {
            entry.children.push(own);
        }
    }
    let mut lines: Vec<CategoryLine> = roots.into_values().collect();
    for l in &mut lines {
        l.children.sort_by_key(|c| std::cmp::Reverse(c.amount_minor));
    }
    lines.sort_by(|a, b| b.amount_minor.cmp(&a.amount_minor).then(b.average_minor.cmp(&a.average_minor)));
    let sum = |f: fn(&CategoryLine) -> i64| lines.iter().map(f).sum::<i64>();
    Ok(CategoryReport {
        kind,
        from: period.from,
        to: period.to,
        current_month,
        previous_from: previous.from,
        previous_to: previous.to,
        total_minor: sum(|l| l.amount_minor),
        previous_total_minor: sum(|l| l.previous_minor),
        average_total_minor: sum(|l| l.average_minor),
        lines,
    })
}

pub async fn evolution(db: &Db, query: &EvolutionQuery, today: Date) -> AppResult<Evolution> {
    let count = query.months.unwrap_or(12).clamp(1, 60);
    // Onze mois de plus, pour le taux glissant du premier mois affiché.
    let periods = reporting::months_until(today, count + 11);
    let first = periods.first().map_or(today, |p| p.from);
    let facts: Vec<SplitFact> = db::transactions::split_facts(&db.read, first, today)
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
    let flows: Vec<reporting::Flows> =
        periods.iter().map(|p| reporting::flows(&facts, p.from, p.to.min(today))).collect();
    let months = periods
        .iter()
        .zip(&flows)
        .enumerate()
        .skip(11)
        .map(|(index, (period, f))| MonthFlows {
            month: format!("{:04}-{:02}", period.from.year(), period.from.month()),
            from: period.from,
            to: period.to.min(today),
            income_minor: f.income.minor(),
            expenses_minor: f.expenses.minor(),
            savings_minor: f.savings.minor(),
            savings_rate_ppm: f.savings_rate.map(|r| r.ppm()),
            rolling_rate_ppm: reporting::rolling_rate(&flows[index - 11..=index]).map(|r| r.ppm()),
            current_month: period.from == reporting::first_of_month(today),
        })
        .collect();
    Ok(Evolution { months })
}
