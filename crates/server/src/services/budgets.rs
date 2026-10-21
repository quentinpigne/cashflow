//! Budgets (spec 05) : vue du mois, lignes versionnées et surcharges, projets, assistant et alertes.

use std::collections::{HashMap, HashSet};

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::budgeting::{
    self, BudgetData, BudgetKind, BudgetSplit, LineVersion, Month, Override, Periodicity, SavingsMove,
};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, BudgetLineId, CategoryId, ProjectId, TransactionId};
use cashflow_domain::{Currency, Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{clean, required};
use crate::db::budgets::{LineRow, OverrideRow, ProjectRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Réglage des seuils d'alerte globaux (R-BUD-60).
const THRESHOLDS_SETTING: &str = "budget_alert_thresholds";

// ---------------------------------------------------------------- Vues

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MonthQuery {
    /// Mois, `AAAA-MM` ; le mois en cours par défaut.
    pub month: Option<String>,
}

/// Niveau d'alerte d'une ligne (R-BUD-61).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlertLevel {
    None,
    /// Premier seuil franchi (orange).
    Warning,
    /// Second seuil franchi (rouge).
    Over,
    /// Objectif d'épargne atteint.
    Reached,
    /// Objectif d'épargne non atteint en fin de mois.
    Missed,
}

/// Une ligne de budget pour un mois (BUD-03).
#[derive(Debug, Serialize, ToSchema)]
pub struct BudgetLineMonth {
    /// Version en vigueur ce mois-ci.
    pub id: BudgetLineId,
    /// Première version de la ligne : identité stable d'une version à l'autre.
    pub line_id: BudgetLineId,
    pub kind: BudgetKind,
    pub periodicity: Periodicity,
    pub category_id: Option<CategoryId>,
    pub target_account_id: Option<AccountId>,
    pub name: String,
    /// Catégorie parente, pour l'affichage.
    pub parent_name: Option<String>,
    pub year_start_month: u32,
    /// Montant de la version (mensuel ou annuel).
    pub amount_minor: i64,
    /// Surcharge de ce mois (ou de cette année budgétaire), si elle existe.
    pub override_minor: Option<i64>,
    /// Prévu du mois ; quote-part pour une ligne annuelle (R-BUD-16).
    pub planned_minor: i64,
    /// Réel du mois ; cumul sur l'année budgétaire pour une ligne annuelle (R-BUD-15).
    pub actual_minor: i64,
    pub carried_minor: i64,
    pub available_minor: i64,
    pub remaining_minor: i64,
    pub rollover: bool,
    pub effective_from: String,
    pub effective_to: Option<String>,
    /// Seuils propres à la ligne ; absents, les seuils globaux s'appliquent.
    pub alert_thresholds: Option<Vec<u32>>,
    pub alert: AlertLevel,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OffBudgetLine {
    pub category_id: CategoryId,
    pub name: String,
    pub parent_name: Option<String>,
    pub amount_minor: i64,
}

/// Le budget d'un mois (BUD-03).
#[derive(Debug, Serialize, ToSchema)]
pub struct MonthBudgetView {
    /// `AAAA-MM`.
    pub month: String,
    pub current_month: bool,
    /// Au moins une ligne existe, à n'importe quelle période.
    pub has_lines: bool,
    pub lines: Vec<BudgetLineMonth>,
    pub off_budget: Vec<OffBudgetLine>,
    pub off_budget_income_minor: i64,
    pub uncategorized_out_minor: i64,
    pub uncategorized_in_minor: i64,
    pub excluded_projects_minor: i64,
    /// Reste à budgéter, prévu (R-BUD-40).
    pub to_budget_minor: i64,
    /// Solde du mois, réel (R-BUD-41).
    pub balance_minor: i64,
    pub income_minor: i64,
    pub expenses_minor: i64,
    pub savings_minor: i64,
    pub thresholds: Vec<u32>,
}

/// Une version de ligne, pour l'historique.
#[derive(Debug, Serialize, ToSchema)]
pub struct BudgetLineVersion {
    pub id: BudgetLineId,
    pub line_id: BudgetLineId,
    pub kind: BudgetKind,
    pub periodicity: Periodicity,
    pub category_id: Option<CategoryId>,
    pub target_account_id: Option<AccountId>,
    pub amount_minor: i64,
    pub rollover: bool,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub overrides: Vec<OverrideView>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OverrideView {
    pub period: String,
    pub amount_minor: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BudgetLineInput {
    pub kind: BudgetKind,
    pub category_id: Option<CategoryId>,
    pub target_account_id: Option<AccountId>,
    #[serde(default = "monthly")]
    pub periodicity: Periodicity,
    #[serde(default = "january")]
    pub year_start_month: u32,
    pub amount_minor: i64,
    #[serde(default)]
    pub rollover: bool,
    /// Premier mois, `AAAA-MM`.
    pub from: String,
    #[serde(default)]
    pub alert_thresholds: Option<Vec<u32>>,
}

fn monthly() -> Periodicity {
    Periodicity::Monthly
}

fn january() -> u32 {
    1
}

/// Portée d'un changement de montant (R-BUD-51).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AmountScope {
    /// Nouvelle version à partir du mois.
    FromMonth,
    /// Surcharge du mois (ou de l'année budgétaire d'une ligne annuelle).
    ThisMonth,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AmountChange {
    pub month: String,
    pub amount_minor: i64,
    pub scope: AmountScope,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LineSettings {
    pub rollover: bool,
    #[serde(default)]
    pub alert_thresholds: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MonthInput {
    pub month: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Thresholds {
    pub thresholds: Vec<u32>,
}

/// Proposition de l'assistant pour une catégorie (BUD-10).
#[derive(Debug, Serialize, ToSchema)]
pub struct BudgetSuggestion {
    pub category_id: CategoryId,
    pub kind: CategoryKind,
    pub name: String,
    pub parent_id: Option<CategoryId>,
    pub parent_name: Option<String>,
    /// Montants des mois retenus, du plus ancien au plus récent.
    pub monthly_minor: Vec<i64>,
    pub suggested_minor: i64,
    /// La catégorie a déjà une ligne en vigueur.
    pub has_line: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Suggestions {
    /// Mois complets retenus, `AAAA-MM`.
    pub months: Vec<String>,
    pub categories: Vec<BudgetSuggestion>,
}

// ---------------------------------------------------------------- Chargement

fn parse_month(value: &str) -> AppResult<Month> {
    value.parse().map_err(|_| AppError::invalid_field("INVALID_MONTH", "month", "mois attendu au format AAAA-MM"))
}

fn version(row: &LineRow) -> LineVersion<BudgetLineId, CategoryId, AccountId> {
    LineVersion {
        id: row.id,
        kind: row.kind,
        category: row.category_id,
        target_account: row.target_account_id,
        periodicity: row.periodicity,
        year_start_month: row.year_start_month,
        amount: row.amount,
        rollover: row.rollover,
        from: row.effective_from,
        to: row.effective_to,
    }
}

/// Première version de chaque ligne (R-BUD-50).
fn roots(rows: &[LineRow]) -> HashMap<BudgetLineId, BudgetLineId> {
    let previous: HashMap<BudgetLineId, Option<BudgetLineId>> =
        rows.iter().map(|r| (r.id, r.previous_version_id)).collect();
    rows.iter()
        .map(|r| {
            let mut root = r.id;
            let mut guard = 0;
            while let Some(Some(p)) = previous.get(&root) {
                root = *p;
                guard += 1;
                if guard > 1_000 {
                    break;
                }
            }
            (r.id, root)
        })
        .collect()
}

pub async fn default_thresholds(db: &Db) -> AppResult<Vec<u32>> {
    Ok(db::settings::get(&db.read, THRESHOLDS_SETTING).await?.unwrap_or_else(|| budgeting::DEFAULT_THRESHOLDS.to_vec()))
}

pub async fn set_thresholds(db: &Db, input: &Thresholds) -> AppResult<Thresholds> {
    let thresholds = check_global(&input.thresholds)?;
    db::settings::set(&db.write, THRESHOLDS_SETTING, &thresholds).await?;
    Ok(Thresholds { thresholds })
}

/// Seuils d'une ligne ou d'un projet : absents, les seuils globaux s'appliquent ; une liste **vide**
/// désactive les alertes (une ligne fixe comme le loyer, par exemple, R-BUD-60).
fn check_thresholds(value: Option<&Vec<u32>>) -> AppResult<Option<Vec<u32>>> {
    let Some(value) = value else { return Ok(None) };
    if value.is_empty() {
        return Ok(Some(Vec::new()));
    }
    check_global(value).map(Some)
}

/// Seuils globaux : un ou deux pourcentages, jamais aucun.
fn check_global(value: &[u32]) -> AppResult<Vec<u32>> {
    let mut thresholds = value.to_vec();
    thresholds.sort_unstable();
    thresholds.dedup();
    if thresholds.is_empty() || thresholds.len() > 2 || thresholds.iter().any(|t| !(1..=500).contains(t)) {
        return Err(AppError::invalid_field(
            "INVALID_THRESHOLDS",
            "alert_thresholds",
            "un ou deux seuils, en pour cent, entre 1 et 500",
        ));
    }
    Ok(thresholds)
}

/// Tout ce qu'il faut pour calculer le budget jusqu'à la fin de `month`.
struct Loaded {
    rows: Vec<LineRow>,
    overrides: Vec<OverrideRow>,
    splits: Vec<BudgetSplit<CategoryId, ProjectId>>,
    moves: Vec<SavingsMove<AccountId>>,
    parents: HashMap<CategoryId, CategoryId>,
}

impl Loaded {
    async fn load(db: &Db, month: Month) -> AppResult<Self> {
        let rows = db::budgets::lines(&db.read).await?;
        let overrides = db::budgets::overrides(&db.read).await?;
        // Les reports remontent au début de chaque version ; les lignes annuelles, au début de leur année.
        let start = rows
            .iter()
            .filter(|r| r.effective_from <= month)
            .map(|r| r.effective_from.min(version(r).budget_year_start(month)))
            .min()
            .unwrap_or(month)
            .min(month)
            .first_day();
        let end = month.last_day();
        let splits = db::budgets::facts(&db.read, start, end)
            .await?
            .into_iter()
            .filter(|f| f.currency == Currency::EUR.code())
            .map(|f| BudgetSplit {
                date: f.date,
                amount: f.amount,
                category: f.category_id,
                project: f.project_id.map(|p| (p, f.project_excluded)),
                in_budget: f.in_budget,
                is_transfer: f.is_transfer,
            })
            .collect();
        let moves = db::budgets::savings_moves(&db.read, start, end)
            .await?
            .into_iter()
            .map(|m| SavingsMove { date: m.date, account: m.account_id, amount: m.amount })
            .collect();
        let parents =
            db::categories::list(&db.read).await?.into_iter().filter_map(|c| c.parent_id.map(|p| (c.id, p))).collect();
        Ok(Loaded { rows, overrides, splits, moves, parents })
    }
}

fn level(line: &budgeting::LineMonth<BudgetLineId>, thresholds: &[u32], month_over: bool) -> AlertLevel {
    match line.kind {
        BudgetKind::Income => AlertLevel::None,
        BudgetKind::Savings if line.actual >= line.available => AlertLevel::Reached,
        // Alertes désactivées : ni orange, ni rouge, ni objectif signalé comme manqué.
        _ if thresholds.is_empty() => AlertLevel::None,
        BudgetKind::Savings if budgeting::savings_goal_missed(line.actual, line.available, month_over) => {
            AlertLevel::Missed
        }
        BudgetKind::Savings => AlertLevel::None,
        BudgetKind::Expense => {
            let crossed = budgeting::crossed(line.actual, line.available, thresholds);
            match (thresholds.first(), thresholds.get(1)) {
                (_, Some(second)) if crossed.contains(second) => AlertLevel::Over,
                (Some(first), _) if crossed.contains(first) => {
                    if thresholds.len() == 1 {
                        AlertLevel::Over
                    } else {
                        AlertLevel::Warning
                    }
                }
                _ => AlertLevel::None,
            }
        }
    }
}

/// Le budget d'un mois (BUD-03).
pub async fn month(db: &Db, query: &MonthQuery, today: Date) -> AppResult<MonthBudgetView> {
    let month = match &query.month {
        Some(m) => parse_month(m)?,
        None => Month::of(today),
    };
    compute(db, month, today).await
}

async fn compute(db: &Db, month: Month, today: Date) -> AppResult<MonthBudgetView> {
    let loaded = Loaded::load(db, month).await?;
    let thresholds = default_thresholds(db).await?;
    let versions: Vec<_> = loaded.rows.iter().map(version).collect();
    let overrides: Vec<Override<BudgetLineId>> =
        loaded.overrides.iter().map(|o| Override { line: o.line_id, period: o.period, amount: o.amount }).collect();
    let budget = BudgetData {
        lines: &versions,
        overrides: &overrides,
        splits: &loaded.splits,
        moves: &loaded.moves,
        parents: &loaded.parents,
    }
    .build();
    let result = budget.month(month);

    let categories = db::categories::list(&db.read).await?;
    let names: HashMap<CategoryId, (String, Option<CategoryId>)> =
        categories.iter().map(|c| (c.id, (c.name.clone(), c.parent_id))).collect();
    let accounts: HashMap<AccountId, String> =
        db::accounts::list(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect();
    let roots = roots(&loaded.rows);
    let rows: HashMap<BudgetLineId, &LineRow> = loaded.rows.iter().map(|r| (r.id, r)).collect();
    let month_over = month < Month::of(today);

    let mut lines: Vec<BudgetLineMonth> = result
        .lines
        .iter()
        .filter_map(|l| {
            let row = rows.get(&l.line)?;
            let (name, parent_name) = match (row.category_id, row.target_account_id) {
                (Some(c), _) => {
                    let (name, parent) = names.get(&c).cloned().unwrap_or_default();
                    (name, parent.and_then(|p| names.get(&p).map(|n| n.0.clone())))
                }
                (None, Some(a)) => (accounts.get(&a).cloned().unwrap_or_default(), None),
                (None, None) => (String::new(), None),
            };
            let period = match row.periodicity {
                Periodicity::Monthly => month,
                Periodicity::Yearly => version(row).budget_year_start(month),
            };
            let line_thresholds = row.alert_thresholds.clone().unwrap_or_else(|| thresholds.clone());
            Some(BudgetLineMonth {
                id: row.id,
                line_id: roots.get(&row.id).copied().unwrap_or(row.id),
                kind: row.kind,
                periodicity: row.periodicity,
                category_id: row.category_id,
                target_account_id: row.target_account_id,
                name,
                parent_name,
                year_start_month: row.year_start_month,
                amount_minor: row.amount.minor(),
                override_minor: loaded
                    .overrides
                    .iter()
                    .find(|o| o.line_id == row.id && o.period == period)
                    .map(|o| o.amount.minor()),
                planned_minor: l.planned.minor(),
                actual_minor: l.actual.minor(),
                carried_minor: l.carried.minor(),
                available_minor: l.available.minor(),
                remaining_minor: l.remaining().minor(),
                rollover: row.rollover,
                effective_from: row.effective_from.to_string(),
                effective_to: row.effective_to.map(|m| m.to_string()),
                alert_thresholds: row.alert_thresholds.clone(),
                alert: level(l, &line_thresholds, month_over),
            })
        })
        .collect();
    let order = |l: &BudgetLineMonth| match (l.kind, l.periodicity) {
        (BudgetKind::Income, _) => 0,
        (BudgetKind::Expense, Periodicity::Monthly) => 1,
        (BudgetKind::Expense, Periodicity::Yearly) => 2,
        (BudgetKind::Savings, _) => 3,
    };
    lines.sort_by(|a, b| {
        order(a)
            .cmp(&order(b))
            .then_with(|| a.parent_name.as_deref().unwrap_or(&a.name).cmp(b.parent_name.as_deref().unwrap_or(&b.name)))
            .then_with(|| a.parent_name.is_some().cmp(&b.parent_name.is_some()))
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut off_budget: Vec<OffBudgetLine> = result
        .off_budget
        .iter()
        .filter_map(|(category, amount)| {
            let category = (*category)?;
            Some(OffBudgetLine {
                category_id: category,
                name: names.get(&category).map(|n| n.0.clone()).unwrap_or_default(),
                parent_name: names.get(&category).and_then(|n| n.1).and_then(|p| names.get(&p)).map(|p| p.0.clone()),
                amount_minor: amount.minor(),
            })
        })
        .filter(|l| l.amount_minor > 0)
        .collect();
    off_budget.sort_by_key(|l| std::cmp::Reverse(l.amount_minor));

    Ok(MonthBudgetView {
        month: month.to_string(),
        current_month: month == Month::of(today),
        has_lines: !loaded.rows.is_empty(),
        lines,
        off_budget,
        off_budget_income_minor: result.off_budget_income.minor(),
        uncategorized_out_minor: result.uncategorized_out.minor(),
        uncategorized_in_minor: result.uncategorized_in.minor(),
        excluded_projects_minor: result.excluded_projects.minor(),
        to_budget_minor: result.to_budget.minor(),
        balance_minor: result.balance.minor(),
        income_minor: result.income.minor(),
        expenses_minor: result.expenses.minor(),
        savings_minor: result.savings.minor(),
        thresholds,
    })
}

/// Toutes les versions de lignes, avec leurs surcharges (historique, R-BUD-53).
pub async fn versions(db: &Db) -> AppResult<Vec<BudgetLineVersion>> {
    let rows = db::budgets::lines(&db.read).await?;
    let overrides = db::budgets::overrides(&db.read).await?;
    let roots = roots(&rows);
    Ok(rows
        .iter()
        .map(|r| BudgetLineVersion {
            id: r.id,
            line_id: roots.get(&r.id).copied().unwrap_or(r.id),
            kind: r.kind,
            periodicity: r.periodicity,
            category_id: r.category_id,
            target_account_id: r.target_account_id,
            amount_minor: r.amount.minor(),
            rollover: r.rollover,
            effective_from: r.effective_from.to_string(),
            effective_to: r.effective_to.map(|m| m.to_string()),
            overrides: overrides
                .iter()
                .filter(|o| o.line_id == r.id)
                .map(|o| OverrideView { period: o.period.to_string(), amount_minor: o.amount.minor() })
                .collect(),
        })
        .collect())
}

// ---------------------------------------------------------------- Écriture des lignes

/// Deux versions se chevauchent-elles ?
fn overlaps(a: &LineRow, b: &LineRow) -> bool {
    let a_to = a.effective_to.unwrap_or(Month { year: 9999, month: 12 });
    let b_to = b.effective_to.unwrap_or(Month { year: 9999, month: 12 });
    a.effective_from <= b_to && b.effective_from <= a_to
}

/// R-BUD-11 : une catégorie (ou un compte cible) a au plus une ligne en vigueur à la fois.
fn check_overlap(rows: &[LineRow], line: &LineRow) -> AppResult<()> {
    let same_target = |r: &LineRow| {
        r.id != line.id
            && r.id != line.previous_version_id.unwrap_or(line.id)
            && ((line.category_id.is_some() && r.category_id == line.category_id)
                || (line.target_account_id.is_some() && r.target_account_id == line.target_account_id))
    };
    if rows.iter().filter(|r| same_target(r)).any(|r| overlaps(r, line)) {
        return Err(DomainError::BudgetLineOverlap.into());
    }
    Ok(())
}

pub async fn create_line(db: &Db, input: &BudgetLineInput) -> AppResult<BudgetLineVersion> {
    let row = LineRow {
        id: BudgetLineId::new(),
        kind: input.kind,
        category_id: input.category_id,
        target_account_id: input.target_account_id,
        periodicity: input.periodicity,
        year_start_month: input.year_start_month,
        amount: Money::from_minor(input.amount_minor),
        rollover: input.rollover,
        effective_from: parse_month(&input.from)?,
        effective_to: None,
        alert_thresholds: check_thresholds(input.alert_thresholds.as_ref())?,
        previous_version_id: None,
    };
    if !(1..=12).contains(&row.year_start_month) {
        return Err(AppError::invalid_field("INVALID_MONTH", "year_start_month", "mois entre 1 et 12"));
    }
    budgeting::check_line(&version(&row))?;
    if let Some(category) = row.category_id {
        let category = db::categories::find(&db.read, category).await?.ok_or(AppError::NotFound("catégorie"))?;
        let expected = if row.kind == BudgetKind::Income { CategoryKind::Income } else { CategoryKind::Expense };
        if category.kind != expected {
            return Err(DomainError::InvalidBudgetLine.into());
        }
    }
    if let Some(account) = row.target_account_id {
        let account = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
        if !matches!(account.kind.family(), AccountFamily::FixedSavings | AccountFamily::VariableSavings) {
            return Err(AppError::invalid_field(
                "SAVINGS_TARGET_NOT_SAVINGS",
                "target_account_id",
                "un objectif d'épargne vise un compte d'épargne (R-BUD-30)",
            ));
        }
    }
    let mut tx = db.write.begin().await?;
    let rows = db::budgets::lines(&mut *tx).await?;
    check_overlap(&rows, &row)?;
    db::budgets::upsert_line(&mut *tx, &row).await?;
    tx.commit().await?;
    one_version(db, row.id).await
}

async fn one_version(db: &Db, id: BudgetLineId) -> AppResult<BudgetLineVersion> {
    versions(db).await?.into_iter().find(|v| v.id == id).ok_or(AppError::NotFound("ligne de budget"))
}

async fn find_line(conn: &mut sqlx::SqliteConnection, id: BudgetLineId) -> AppResult<(Vec<LineRow>, LineRow)> {
    let rows = db::budgets::lines(&mut *conn).await?;
    let row = rows.iter().find(|r| r.id == id).cloned().ok_or(AppError::NotFound("ligne de budget"))?;
    Ok((rows, row))
}

/// La version de la ligne `id` (n'importe laquelle de ses versions) en vigueur au mois `month`.
fn version_at(rows: &[LineRow], id: BudgetLineId, month: Month) -> AppResult<LineRow> {
    let roots = roots(rows);
    let root = roots.get(&id).copied().unwrap_or(id);
    rows.iter().find(|r| roots.get(&r.id) == Some(&root) && version(r).in_force(month)).cloned().ok_or_else(|| {
        AppError::invalid_field("LINE_NOT_IN_FORCE", "month", "la ligne n'est pas en vigueur ce mois-là")
    })
}

/// Démarre une nouvelle version au mois `month` : la version courante est close au mois précédent
/// (R-BUD-51, R-BUD-18). Ses surcharges postérieures passent à la nouvelle version.
async fn new_version(
    conn: &mut sqlx::SqliteConnection,
    current: &LineRow,
    month: Month,
    amount: Money,
) -> AppResult<BudgetLineId> {
    if current.effective_from == month {
        let row = LineRow { amount, ..current.clone() };
        db::budgets::upsert_line(&mut *conn, &row).await?;
        return Ok(row.id);
    }
    let closed = LineRow { effective_to: Some(month.previous()), ..current.clone() };
    let next = LineRow {
        id: BudgetLineId::new(),
        amount,
        effective_from: month,
        effective_to: current.effective_to,
        previous_version_id: Some(current.id),
        ..current.clone()
    };
    db::budgets::upsert_line(&mut *conn, &closed).await?;
    db::budgets::upsert_line(&mut *conn, &next).await?;
    db::budgets::move_overrides(&mut *conn, current.id, next.id, month).await?;
    Ok(next.id)
}

/// Change le montant d'une ligne à partir d'un mois, ou pour ce mois seulement (R-BUD-51, R-BUD-52).
pub async fn change_amount(db: &Db, id: BudgetLineId, input: &AmountChange) -> AppResult<BudgetLineVersion> {
    let month = parse_month(&input.month)?;
    let amount = Money::from_minor(input.amount_minor);
    let mut tx = db.write.begin().await?;
    let (rows, _) = find_line(&mut tx, id).await?;
    let current = version_at(&rows, id, month)?;
    let result = match input.scope {
        AmountScope::FromMonth => {
            budgeting::check_line(&LineVersion { amount, ..version(&current) })?;
            new_version(&mut tx, &current, month, amount).await?
        }
        AmountScope::ThisMonth => {
            if amount.is_negative() {
                return Err(DomainError::BudgetAmountNotPositive.into());
            }
            let period = match current.periodicity {
                Periodicity::Monthly => month,
                Periodicity::Yearly => version(&current).budget_year_start(month),
            };
            if amount == current.amount {
                db::budgets::delete_override(&mut *tx, current.id, period).await?;
            } else {
                db::budgets::set_override(&mut *tx, &OverrideRow { line_id: current.id, period, amount }).await?;
            }
            current.id
        }
    };
    tx.commit().await?;
    one_version(db, result).await
}

/// Supprime la surcharge d'une période.
pub async fn delete_override(db: &Db, id: BudgetLineId, period: &str) -> AppResult<()> {
    db::budgets::delete_override(&db.write, id, parse_month(period)?).await?;
    Ok(())
}

/// Report et seuils d'une version.
pub async fn update_settings(db: &Db, id: BudgetLineId, input: &LineSettings) -> AppResult<BudgetLineVersion> {
    let mut tx = db.write.begin().await?;
    let (_, row) = find_line(&mut tx, id).await?;
    let row = LineRow {
        rollover: input.rollover,
        alert_thresholds: check_thresholds(input.alert_thresholds.as_ref())?,
        ..row
    };
    budgeting::check_line(&version(&row))?;
    db::budgets::upsert_line(&mut *tx, &row).await?;
    tx.commit().await?;
    one_version(db, id).await
}

/// « Remettre le report à zéro » : nouvelle version au même montant à partir du mois (R-BUD-18).
pub async fn reset_rollover(db: &Db, id: BudgetLineId, input: &MonthInput) -> AppResult<BudgetLineVersion> {
    let month = parse_month(&input.month)?;
    let mut tx = db.write.begin().await?;
    let (rows, _) = find_line(&mut tx, id).await?;
    let current = version_at(&rows, id, month)?;
    let result = new_version(&mut tx, &current, month, current.amount).await?;
    tx.commit().await?;
    one_version(db, result).await
}

/// Supprimer une ligne à partir d'un mois revient à la clore au mois précédent ; une version qui n'a
/// jamais été en vigueur avant ce mois disparaît (R-BUD-53).
pub async fn end_line(db: &Db, id: BudgetLineId, input: &MonthInput) -> AppResult<()> {
    let month = parse_month(&input.month)?;
    let mut tx = db.write.begin().await?;
    let (rows, _) = find_line(&mut tx, id).await?;
    let roots = roots(&rows);
    let root = roots.get(&id).copied().unwrap_or(id);
    for row in rows.iter().filter(|r| roots.get(&r.id) == Some(&root)) {
        if row.effective_from >= month {
            db::budgets::delete_line(&mut tx, row.id).await?;
        } else if row.effective_to.is_none_or(|to| to >= month) {
            let closed = LineRow { effective_to: Some(month.previous()), ..row.clone() };
            db::budgets::upsert_line(&mut *tx, &closed).await?;
            db::budgets::delete_overrides_after(&mut *tx, row.id, month.previous()).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------- Assistant (BUD-10)

/// Nombre maximal de mois complets pris en compte par l'assistant.
const SUGGESTION_MONTHS: usize = 6;

pub async fn suggestions(db: &Db, today: Date) -> AppResult<Suggestions> {
    let last = Month::of(today).previous();
    let first_data = db::budgets::first_transaction_date(&db.read).await?.map(Month::of);
    let mut months: Vec<Month> = Vec::new();
    let mut month = last;
    while months.len() < SUGGESTION_MONTHS && first_data.is_some_and(|f| f <= month) {
        months.insert(0, month);
        month = month.previous();
    }
    let Some(first) = months.first().copied() else {
        return Ok(Suggestions { months: Vec::new(), categories: Vec::new() });
    };
    let facts = db::budgets::facts(&db.read, first.first_day(), last.last_day()).await?;
    let categories = db::categories::list(&db.read).await?;
    let by_id: HashMap<CategoryId, &db::categories::CategoryRow> = categories.iter().map(|c| (c.id, c)).collect();
    let mut amounts: HashMap<(CategoryId, Month), Money> = HashMap::new();
    for fact in facts
        .iter()
        .filter(|f| f.in_budget && !f.is_transfer && !f.project_excluded && f.currency == Currency::EUR.code())
    {
        let Some(category) = fact.category_id.and_then(|c| by_id.get(&c)) else { continue };
        let signed = if category.kind == CategoryKind::Expense { -fact.amount } else { fact.amount };
        *amounts.entry((category.id, Month::of(fact.date))).or_insert(Money::ZERO) += signed;
    }
    let current = Month::of(today);
    let with_line: HashSet<CategoryId> = db::budgets::lines(&db.read)
        .await?
        .into_iter()
        .filter(|l| version(l).in_force(current) || l.effective_from > current)
        .filter_map(|l| l.category_id)
        .collect();
    let mut result: Vec<BudgetSuggestion> = categories
        .iter()
        .filter(|c| !c.archived)
        .filter_map(|c| {
            let monthly: Vec<Money> =
                months.iter().map(|m| amounts.get(&(c.id, *m)).copied().unwrap_or(Money::ZERO)).collect();
            if monthly.iter().all(|m| !m.is_positive()) {
                return None;
            }
            let monthly: Vec<Money> =
                monthly.into_iter().map(|m| if m.is_negative() { Money::ZERO } else { m }).collect();
            Some(BudgetSuggestion {
                category_id: c.id,
                kind: c.kind,
                name: c.name.clone(),
                parent_id: c.parent_id,
                parent_name: c.parent_id.and_then(|p| by_id.get(&p)).map(|p| p.name.clone()),
                suggested_minor: budgeting::suggested_amount(&monthly).minor(),
                monthly_minor: monthly.iter().map(|m| m.minor()).collect(),
                has_line: with_line.contains(&c.id),
            })
        })
        .collect();
    result.sort_by(|a, b| {
        (a.kind == CategoryKind::Expense)
            .cmp(&(b.kind == CategoryKind::Expense))
            .then(b.suggested_minor.cmp(&a.suggested_minor))
    });
    Ok(Suggestions { months: months.iter().map(Month::to_string).collect(), categories: result })
}

// ---------------------------------------------------------------- Projets

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectCategory {
    pub category_id: Option<CategoryId>,
    pub name: String,
    pub amount_minor: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectSplit {
    pub transaction_id: TransactionId,
    pub date: Date,
    pub label: String,
    pub account_name: String,
    pub category_id: Option<CategoryId>,
    pub amount_minor: i64,
    /// Date hors de [début, fin] : signalée seulement (R-BUD-23).
    pub out_of_range: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub start_date: Date,
    pub end_date: Option<Date>,
    pub amount_minor: i64,
    pub exclude_from_period_budgets: bool,
    pub active: bool,
    pub alert_thresholds: Option<Vec<u32>>,
    pub note: Option<String>,
    /// Réel, remboursements déduits (R-BUD-22).
    pub actual_minor: i64,
    pub alert: AlertLevel,
    pub by_category: Vec<ProjectCategory>,
    pub splits: Vec<ProjectSplit>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProjectInput {
    pub name: String,
    pub start_date: Date,
    pub end_date: Option<Date>,
    pub amount_minor: i64,
    #[serde(default = "yes")]
    pub exclude_from_period_budgets: bool,
    #[serde(default = "yes")]
    pub active: bool,
    #[serde(default)]
    pub alert_thresholds: Option<Vec<u32>>,
    #[serde(default)]
    pub note: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AttachInput {
    pub transaction_ids: Vec<TransactionId>,
    /// Absent : détache les ventilations de leur projet.
    pub project_id: Option<ProjectId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttachResult {
    pub attached: usize,
    /// Virements internes, ignorés.
    pub skipped: usize,
}

pub async fn projects(db: &Db) -> AppResult<Vec<ProjectView>> {
    let rows = db::budgets::projects(&db.read).await?;
    let splits = db::budgets::project_splits(&db.read).await?;
    let thresholds = default_thresholds(db).await?;
    let names: HashMap<CategoryId, String> =
        db::categories::list(&db.read).await?.into_iter().map(|c| (c.id, c.name)).collect();
    Ok(rows
        .into_iter()
        .map(|p| {
            let own: Vec<_> = splits.iter().filter(|s| s.project_id == p.id).collect();
            let facts: Vec<BudgetSplit<CategoryId, ProjectId>> = own
                .iter()
                .map(|s| BudgetSplit {
                    date: s.date,
                    amount: s.amount,
                    category: s.category_id,
                    project: Some((p.id, p.exclude_from_period_budgets)),
                    in_budget: true,
                    is_transfer: false,
                })
                .collect();
            let by_category = budgeting::project_actual(&facts, p.id);
            let actual: Money = by_category.iter().map(|(_, a)| *a).sum();
            let line_thresholds = p.alert_thresholds.clone().unwrap_or_else(|| thresholds.clone());
            let line = budgeting::LineMonth {
                line: BudgetLineId::new(),
                kind: BudgetKind::Expense,
                periodicity: Periodicity::Monthly,
                planned: p.amount,
                actual,
                carried: Money::ZERO,
                available: p.amount,
            };
            let mut by_category: Vec<ProjectCategory> = by_category
                .into_iter()
                .map(|(c, a)| ProjectCategory {
                    category_id: c,
                    name: c.and_then(|c| names.get(&c).cloned()).unwrap_or_else(|| "Non catégorisé".into()),
                    amount_minor: a.minor(),
                })
                .collect();
            by_category.sort_by_key(|c| std::cmp::Reverse(c.amount_minor));
            ProjectView {
                alert: level(&line, &line_thresholds, false),
                actual_minor: actual.minor(),
                by_category,
                splits: own
                    .iter()
                    .map(|s| ProjectSplit {
                        transaction_id: s.transaction_id,
                        date: s.date,
                        label: s.label.clone(),
                        account_name: s.account_name.clone(),
                        category_id: s.category_id,
                        amount_minor: s.amount.minor(),
                        out_of_range: s.date < p.start_date || p.end_date.is_some_and(|end| s.date > end),
                    })
                    .collect(),
                id: p.id,
                name: p.name,
                start_date: p.start_date,
                end_date: p.end_date,
                amount_minor: p.amount.minor(),
                exclude_from_period_budgets: p.exclude_from_period_budgets,
                active: p.active,
                alert_thresholds: p.alert_thresholds,
                note: p.note,
            }
        })
        .collect())
}

pub async fn project(db: &Db, id: ProjectId) -> AppResult<ProjectView> {
    projects(db).await?.into_iter().find(|p| p.id == id).ok_or(AppError::NotFound("projet"))
}

pub async fn save_project(db: &Db, id: Option<ProjectId>, input: &ProjectInput) -> AppResult<ProjectView> {
    let name = required(&input.name, "name", 80)?;
    let amount = Money::from_minor(input.amount_minor);
    if !amount.is_positive() {
        return Err(DomainError::BudgetAmountNotPositive.into());
    }
    if input.end_date.is_some_and(|end| end < input.start_date) {
        return Err(AppError::invalid_field("END_BEFORE_START", "end_date", "la fin précède le début"));
    }
    let existing = db::budgets::projects(&db.read).await?;
    if existing.iter().any(|p| Some(p.id) != id && p.name.eq_ignore_ascii_case(&name)) {
        return Err(AppError::invalid_field("PROJECT_NAME_TAKEN", "name", "un projet porte déjà ce nom"));
    }
    if let Some(id) = id
        && !existing.iter().any(|p| p.id == id)
    {
        return Err(AppError::NotFound("projet"));
    }
    let row = ProjectRow {
        id: id.unwrap_or_else(ProjectId::new),
        name,
        start_date: input.start_date,
        end_date: input.end_date,
        amount,
        exclude_from_period_budgets: input.exclude_from_period_budgets,
        active: input.active,
        alert_thresholds: check_thresholds(input.alert_thresholds.as_ref())?,
        note: clean(input.note.as_deref()),
    };
    db::budgets::upsert_project(&db.write, &row).await?;
    project(db, row.id).await
}

/// Un projet qui a des ventilations se clôture, il ne se supprime pas.
pub async fn delete_project(db: &Db, id: ProjectId) -> AppResult<()> {
    if db::budgets::project_usage(&db.read, id).await? > 0 {
        return Err(AppError::invalid("PROJECT_IN_USE", "ce projet a des ventilations : le clôturer plutôt"));
    }
    db::budgets::delete_project(&db.write, id).await?;
    Ok(())
}

/// Vérifie qu'on peut rattacher de nouvelles ventilations à ce projet (R-BUD-24).
pub async fn check_active(conn: &mut sqlx::SqliteConnection, id: ProjectId) -> AppResult<()> {
    let project = db::budgets::projects(&mut *conn)
        .await?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or(AppError::NotFound("projet"))?;
    if !project.active {
        return Err(DomainError::ProjectClosed.into());
    }
    Ok(())
}

/// Rattache (ou détache) toutes les ventilations de transactions à un projet (BUD-06, en masse).
pub async fn attach(db: &Db, input: &AttachInput) -> AppResult<AttachResult> {
    let mut tx = db.write.begin().await?;
    if let Some(project) = input.project_id {
        check_active(&mut tx, project).await?;
    }
    let mut result = AttachResult { attached: 0, skipped: 0 };
    for id in &input.transaction_ids {
        if db::transactions::transfer_of(&mut *tx, *id).await?.is_some() {
            result.skipped += 1;
            continue;
        }
        let splits = db::transactions::splits_of(&mut *tx, *id).await?;
        if splits.is_empty() {
            return Err(AppError::NotFound("transaction"));
        }
        for split in splits {
            db::budgets::set_split_project(&mut *tx, split.id, input.project_id).await?;
        }
        result.attached += 1;
    }
    tx.commit().await?;
    Ok(result)
}

// ---------------------------------------------------------------- Alertes et notifications (BUD-08)

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationView {
    pub id: cashflow_domain::shared::ids::NotificationId,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
    pub read: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Notifications {
    pub unread: i64,
    pub items: Vec<NotificationView>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReadInput {
    /// Absent : toutes les notifications.
    pub id: Option<cashflow_domain::shared::ids::NotificationId>,
}

pub async fn notifications(db: &Db) -> AppResult<Notifications> {
    let items = db::budgets::notifications(&db.read, 50)
        .await?
        .into_iter()
        .map(|n| NotificationView {
            id: n.id,
            kind: n.kind,
            title: n.title,
            body: n.body,
            link: n.link,
            read: n.read,
            created_at: n.created_at,
        })
        .collect();
    Ok(Notifications { unread: db::budgets::unread_count(&db.read).await?, items })
}

pub async fn mark_read(db: &Db, input: &ReadInput) -> AppResult<Notifications> {
    db::budgets::mark_read(&db.write, input.id).await?;
    notifications(db).await
}

/// Montant en euros, à la française.
fn euros(amount: Money) -> String {
    let minor = amount.minor();
    let sign = if minor < 0 { "−" } else { "" };
    let units = (minor.abs() / 100).to_string();
    let mut grouped = String::new();
    for (i, c) in units.chars().enumerate() {
        if i > 0 && (units.len() - i).is_multiple_of(3) {
            grouped.push('\u{202f}');
        }
        grouped.push(c);
    }
    format!("{sign}{grouped},{:02} €", minor.abs() % 100)
}

struct Crossing {
    key: String,
    text: String,
    link: String,
}

/// Évalue les seuils après une écriture et émet les notifications nouvelles, groupées si plusieurs
/// seuils sont franchis d'un coup (R-BUD-62, R-BUD-63).
pub async fn evaluate_alerts(db: &Db, today: Date) -> AppResult<usize> {
    let current = Month::of(today);
    let mut crossings: Vec<Crossing> = Vec::new();

    let view = compute(db, current, today).await?;
    for line in view.lines.iter().filter(|l| l.kind == BudgetKind::Expense) {
        let thresholds = line.alert_thresholds.clone().unwrap_or_else(|| view.thresholds.clone());
        let period = match line.periodicity {
            Periodicity::Monthly => current,
            Periodicity::Yearly => budgeting::budget_year_start(line.year_start_month, current),
        };
        for t in budgeting::crossed(
            Money::from_minor(line.actual_minor),
            Money::from_minor(line.available_minor),
            &thresholds,
        ) {
            crossings.push(Crossing {
                key: budgeting::alert_key("budget", line.line_id, period, t),
                text: format!(
                    "{} : {} sur {} ({t} %)",
                    line.name,
                    euros(Money::from_minor(line.actual_minor)),
                    euros(Money::from_minor(line.available_minor))
                ),
                link: format!("/budget?mois={current}"),
            });
        }
    }

    // Objectifs d'épargne du mois écoulé, signalés une fois le mois fini (R-BUD-61).
    let previous = compute(db, current.previous(), today).await?;
    for line in previous.lines.iter().filter(|l| l.alert == AlertLevel::Missed) {
        crossings.push(Crossing {
            key: format!("savings:{}:{}:missed", line.line_id, current.previous()),
            text: format!(
                "Objectif {} non atteint : {} sur {}",
                line.name,
                euros(Money::from_minor(line.actual_minor)),
                euros(Money::from_minor(line.available_minor))
            ),
            link: format!("/budget?mois={}", current.previous()),
        });
    }

    let thresholds = view.thresholds.clone();
    for project in projects(db).await?.into_iter().filter(|p| p.active) {
        let line_thresholds = project.alert_thresholds.clone().unwrap_or_else(|| thresholds.clone());
        for t in budgeting::crossed(
            Money::from_minor(project.actual_minor),
            Money::from_minor(project.amount_minor),
            &line_thresholds,
        ) {
            crossings.push(Crossing {
                key: budgeting::alert_key("project", project.id, "total", t),
                text: format!(
                    "Projet {} : {} sur {} ({t} %)",
                    project.name,
                    euros(Money::from_minor(project.actual_minor)),
                    euros(Money::from_minor(project.amount_minor))
                ),
                link: format!("/budget/projets/{}", project.id),
            });
        }
    }

    let known: HashSet<String> = db::budgets::known_keys(&db.read)
        .await?
        .iter()
        .flat_map(|k| k.split(' ').map(str::to_owned).collect::<Vec<_>>())
        .collect();
    let fresh: Vec<Crossing> = crossings.into_iter().filter(|c| !known.contains(&c.key)).collect();
    match fresh.as_slice() {
        [] => {}
        [one] => {
            db::budgets::notify(
                &db.write,
                "BUDGET_ALERT",
                "Seuil de budget franchi",
                &one.text,
                Some(&one.link),
                Some(&one.key),
            )
            .await?;
        }
        many => {
            let keys: Vec<&str> = many.iter().map(|c| c.key.as_str()).collect();
            let body: Vec<&str> = many.iter().map(|c| c.text.as_str()).collect();
            db::budgets::notify(
                &db.write,
                "BUDGET_ALERT",
                &format!("{} seuils de budget franchis", many.len()),
                &body.join("\n"),
                Some(&many[0].link),
                Some(&keys.join(" ")),
            )
            .await?;
        }
    }
    Ok(fresh.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euros_are_formatted_the_french_way() {
        assert_eq!(euros(Money::from_minor(123_456)), "1\u{202f}234,56 €");
        assert_eq!(euros(Money::from_minor(-5)), "−0,05 €");
    }
}
