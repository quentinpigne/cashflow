//! Tiers et groupes de tiers (spec 11) : gestion (TIE-01) et analyses (TIE-04).

use std::collections::{HashMap, HashSet};

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::payees::{self, PayeeFact, PayeeTotals};
use cashflow_domain::reporting::{self, CategorizedFact, Period, SplitFact};
use cashflow_domain::shared::ids::{CategoryId, PayeeGroupId, PayeeId, TransactionId};
use cashflow_domain::{Currency, Date, DomainError, Money};
use chrono::{Datelike, Months};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{clean, required};
use crate::db::payees::{PayeeFields, PayeeRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------- Types exposés

#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeView {
    pub id: PayeeId,
    pub name: String,
    pub group_id: Option<PayeeGroupId>,
    pub archived: bool,
    pub note: Option<String>,
    pub transaction_count: i64,
    /// Utilisé (transactions, récurrences, règles) : il s'archive ou se fusionne, sans se supprimer (R-TIE-5).
    pub in_use: bool,
    /// Catégorie proposée à la saisie quand on choisit ce tiers (R-TIE-14).
    pub suggested_category_id: Option<CategoryId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeGroupView {
    pub id: PayeeGroupId,
    pub name: String,
    pub note: Option<String>,
    pub payee_count: usize,
}

/// Tous les tiers et tous les groupes, par ordre alphabétique (E31).
#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeList {
    pub payees: Vec<PayeeView>,
    pub groups: Vec<PayeeGroupView>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PayeeInput {
    pub name: String,
    pub group_id: Option<PayeeGroupId>,
    #[serde(default)]
    pub archived: bool,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PayeeGroupInput {
    pub name: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MergePayeeInput {
    pub target_id: PayeeId,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MergePayeeResult {
    /// Nombre de transactions réaffectées (R-TIE-7).
    pub moved_transactions: u64,
}

/// Change le groupe de plusieurs tiers d'un coup (E31) ; absent, ils passent « Sans groupe ».
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegroupInput {
    pub payee_ids: Vec<PayeeId>,
    pub group_id: Option<PayeeGroupId>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportPeriod {
    /// Un mois civil (`month`), le mois en cours par défaut.
    #[default]
    Month,
    /// Les 12 derniers mois : les 11 mois complets précédents et le mois en cours.
    Last12Months,
    /// Une année civile (`year`), l'année en cours par défaut.
    Year,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportGrouping {
    /// Groupes de tiers, dépliables en tiers ; les tiers sans groupe sur leur propre ligne.
    #[default]
    Group,
    Payee,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PayeeReportQuery {
    pub period: Option<ReportPeriod>,
    /// `AAAA-MM`, pour `MONTH`.
    pub month: Option<String>,
    /// Pour `YEAR`.
    pub year: Option<i32>,
    /// `EXPENSE` par défaut.
    pub kind: Option<CategoryKind>,
    pub group_by: Option<ReportGrouping>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeLine {
    /// Ligne d'un tiers.
    pub payee_id: Option<PayeeId>,
    /// Ligne d'un groupe, ou groupe du tiers.
    pub group_id: Option<PayeeGroupId>,
    pub name: String,
    pub amount_minor: i64,
    pub previous_minor: i64,
    /// Non catégorisé, du sens de la nature analysée (sorties pour les dépenses), jamais ajouté au montant
    /// (R-TIE-21).
    pub uncategorized_minor: i64,
    pub transactions: u32,
    #[schema(no_recursion)]
    pub children: Vec<PayeeLine>,
}

/// Dépenses (ou revenus) par tiers ou par groupe sur une période, comparés à la période précédente (TIE-04).
#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeReport {
    pub kind: CategoryKind,
    pub period: ReportPeriod,
    pub group_by: ReportGrouping,
    pub from: Date,
    pub to: Date,
    /// La période contient aujourd'hui : la précédente s'arrête à la même date (R-TDB-13).
    pub current: bool,
    pub previous_from: Date,
    pub previous_to: Date,
    pub total_minor: i64,
    pub previous_total_minor: i64,
    pub uncategorized_minor: i64,
    pub lines: Vec<PayeeLine>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TotalsView {
    pub from: Date,
    pub to: Date,
    pub income_minor: i64,
    pub expenses_minor: i64,
    pub uncategorized_in_minor: i64,
    pub uncategorized_out_minor: i64,
    pub transactions: u32,
    /// Dépense moyenne par transaction.
    pub average_expense_minor: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeAmount {
    pub payee_id: PayeeId,
    pub name: String,
    pub expenses_minor: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeMonth {
    /// `AAAA-MM`.
    pub month: String,
    pub expenses_minor: i64,
    pub income_minor: i64,
    /// Pour un groupe, les dépenses de chacun de ses tiers.
    pub by_payee: Vec<PayeeAmount>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CategoryAmount {
    /// Absent : non catégorisé.
    pub category_id: Option<CategoryId>,
    pub name: String,
    pub amount_minor: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeRef {
    pub id: PayeeId,
    pub name: String,
}

/// Fiche d'un tiers ou d'un groupe (E32).
#[derive(Debug, Serialize, ToSchema)]
pub struct PayeeSummary {
    pub name: String,
    pub note: Option<String>,
    pub payee_id: Option<PayeeId>,
    pub group_id: Option<PayeeGroupId>,
    /// Groupe du tiers.
    pub group_name: Option<String>,
    pub archived: bool,
    /// Tiers du groupe.
    pub members: Vec<PayeeRef>,
    pub last_12_months: TotalsView,
    pub year: TotalsView,
    /// 24 mois, du plus ancien au plus récent.
    pub months: Vec<PayeeMonth>,
    /// Dépenses des 12 derniers mois par catégorie, la plus forte d'abord.
    pub categories: Vec<CategoryAmount>,
}

// ---------------------------------------------------------------- Gestion (TIE-01)

pub async fn list(db: &Db) -> AppResult<PayeeList> {
    let rows = db::payees::list(&db.read).await?;
    let suggested = super::imports::payee_categories(db).await?;
    let groups = db::payees::groups(&db.read)
        .await?
        .into_iter()
        .map(|g| PayeeGroupView {
            payee_count: rows.iter().filter(|p| p.group_id == Some(g.id)).count(),
            id: g.id,
            name: g.name,
            note: g.note,
        })
        .collect();
    let payees = rows
        .into_iter()
        .map(|p| PayeeView {
            suggested_category_id: suggested.get(&p.id).copied(),
            id: p.id,
            name: p.name,
            group_id: p.group_id,
            archived: p.archived,
            note: p.note,
            transaction_count: p.transaction_count,
            in_use: p.usage > 0,
        })
        .collect();
    Ok(PayeeList { payees, groups })
}

async fn check_group(db: &Db, group: Option<PayeeGroupId>) -> AppResult<()> {
    if let Some(group) = group
        && db::payees::find_group(&db.read, group).await?.is_none()
    {
        return Err(AppError::NotFound("groupe de tiers"));
    }
    Ok(())
}

pub async fn create(db: &Db, input: &PayeeInput) -> AppResult<PayeeId> {
    let name = required(&input.name, "name", 80)?;
    check_group(db, input.group_id).await?;
    if db::payees::name_taken(&db.read, &name, None).await? {
        return Err(DomainError::PayeeNameTaken.into());
    }
    let id = PayeeId::new();
    let note = clean(input.note.as_deref());
    let fields = PayeeFields { name: &name, group_id: input.group_id, archived: input.archived, note: note.as_deref() };
    db::payees::insert(&db.write, id, &fields).await?;
    Ok(id)
}

/// Renomme, change de groupe, archive ou désarchive un tiers (R-TIE-3, R-TIE-6).
pub async fn update(db: &Db, id: PayeeId, input: &PayeeInput) -> AppResult<()> {
    db::payees::find(&db.read, id).await?.ok_or(AppError::NotFound("tiers"))?;
    let name = required(&input.name, "name", 80)?;
    check_group(db, input.group_id).await?;
    if db::payees::name_taken(&db.read, &name, Some(id)).await? {
        return Err(DomainError::PayeeNameTaken.into());
    }
    let note = clean(input.note.as_deref());
    let fields = PayeeFields { name: &name, group_id: input.group_id, archived: input.archived, note: note.as_deref() };
    db::payees::update(&db.write, id, &fields).await?;
    Ok(())
}

/// Supprime un tiers jamais utilisé (R-TIE-5).
pub async fn delete(db: &Db, id: PayeeId) -> AppResult<()> {
    let payee = db::payees::find(&db.read, id).await?.ok_or(AppError::NotFound("tiers"))?;
    payees::check_deletable(u64::try_from(payee.usage).unwrap_or(u64::MAX))?;
    db::payees::delete(&db.write, id).await.map_err(|_| AppError::Domain(DomainError::PayeeInUse))?;
    Ok(())
}

/// Fusionne un tiers dans un autre, de façon atomique (R-TIE-7).
pub async fn merge(db: &Db, source: PayeeId, input: &MergePayeeInput) -> AppResult<MergePayeeResult> {
    payees::check_merge(source == input.target_id)?;
    let mut tx = db.write.begin().await?;
    db::payees::find(&mut *tx, source).await?.ok_or(AppError::NotFound("tiers"))?;
    db::payees::find(&mut *tx, input.target_id).await?.ok_or(AppError::NotFound("tiers cible"))?;
    let moved_transactions = db::payees::merge(&mut tx, source, input.target_id).await?;
    tx.commit().await?;
    Ok(MergePayeeResult { moved_transactions })
}

pub async fn regroup(db: &Db, input: &RegroupInput) -> AppResult<()> {
    check_group(db, input.group_id).await?;
    let mut tx = db.write.begin().await?;
    for id in &input.payee_ids {
        db::payees::find(&mut *tx, *id).await?.ok_or(AppError::NotFound("tiers"))?;
        db::payees::set_group(&mut *tx, *id, input.group_id).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn create_group(db: &Db, input: &PayeeGroupInput) -> AppResult<PayeeGroupId> {
    let id = PayeeGroupId::new();
    save_group(db, id, None, input).await?;
    Ok(id)
}

pub async fn update_group(db: &Db, id: PayeeGroupId, input: &PayeeGroupInput) -> AppResult<()> {
    db::payees::find_group(&db.read, id).await?.ok_or(AppError::NotFound("groupe de tiers"))?;
    save_group(db, id, Some(id), input).await
}

async fn save_group(db: &Db, id: PayeeGroupId, except: Option<PayeeGroupId>, input: &PayeeGroupInput) -> AppResult<()> {
    let name = required(&input.name, "name", 80)?;
    if db::payees::group_name_taken(&db.read, &name, except).await? {
        return Err(DomainError::PayeeGroupNameTaken.into());
    }
    let note = clean(input.note.as_deref());
    db::payees::upsert_group(&db.write, id, &name, note.as_deref()).await?;
    Ok(())
}

/// Supprime un groupe ; ses tiers restent, sans groupe (R-TIE-4).
pub async fn delete_group(db: &Db, id: PayeeGroupId) -> AppResult<()> {
    db::payees::find_group(&db.read, id).await?.ok_or(AppError::NotFound("groupe de tiers"))?;
    db::payees::delete_group(&db.write, id).await?;
    Ok(())
}

// ---------------------------------------------------------------- Analyses (TIE-04)

async fn facts(db: &Db, from: Date, to: Date) -> AppResult<Vec<db::payees::PayeeFactRow>> {
    Ok(db::payees::facts(&db.read, from, to).await?)
}

fn payee_facts(rows: &[db::payees::PayeeFactRow]) -> Vec<PayeeFact<PayeeId, TransactionId>> {
    rows.iter()
        .map(|r| PayeeFact {
            payee: r.payee_id,
            transaction: r.transaction_id,
            split: SplitFact {
                date: r.date,
                amount: r.amount,
                category_kind: r.category_kind,
                is_movement: r.is_movement,
                is_eur: r.currency == Currency::EUR.code(),
            },
        })
        .collect()
}

/// Période analysée et période de comparaison (R-TDB-13 pour une période en cours).
fn periods(query: &PayeeReportQuery, today: Date) -> AppResult<(Period, Period, bool)> {
    let shift = |date: Date, months: u32| date.checked_sub_months(Months::new(months)).unwrap_or(date);
    Ok(match query.period.unwrap_or_default() {
        ReportPeriod::Month => {
            let start = match &query.month {
                Some(month) => format!("{month}-01")
                    .parse::<Date>()
                    .map_err(|_| AppError::invalid_field("INVALID_MONTH", "month", "mois attendu au format AAAA-MM"))?,
                None => reporting::first_of_month(today),
            };
            if start == reporting::first_of_month(today) {
                let (current, previous) = reporting::month_to_date(today);
                (current, previous, true)
            } else {
                let previous = shift(start, 1);
                (
                    Period { from: start, to: reporting::last_of_month(start) },
                    Period { from: previous, to: reporting::last_of_month(previous) },
                    false,
                )
            }
        }
        ReportPeriod::Last12Months => {
            let from = reporting::months_until(today, 12).first().map_or(today, |p| p.from);
            let previous = Period { from: shift(from, 12), to: shift(today, 12) };
            (Period { from, to: today }, previous, true)
        }
        ReportPeriod::Year => {
            let year = query.year.unwrap_or(today.year());
            let start = Date::from_ymd_opt(year, 1, 1)
                .ok_or_else(|| AppError::invalid_field("INVALID_YEAR", "year", "année invalide"))?;
            let end = Date::from_ymd_opt(year, 12, 31).unwrap_or(start);
            let current = year == today.year();
            let to = if current { today } else { end };
            (Period { from: start, to }, Period { from: shift(start, 12), to: shift(to, 12) }, current)
        }
    })
}

/// Montant et non catégorisé d'un total pour la nature analysée.
fn amounts(totals: &PayeeTotals, kind: CategoryKind) -> (Money, Money) {
    match kind {
        CategoryKind::Expense => (totals.expenses, totals.uncategorized_out),
        CategoryKind::Income => (totals.income, totals.uncategorized_in),
    }
}

pub async fn report(db: &Db, query: &PayeeReportQuery, today: Date) -> AppResult<PayeeReport> {
    let kind = query.kind.unwrap_or(CategoryKind::Expense);
    let (period, previous, current) = periods(query, today)?;
    let rows = facts(db, previous.from.min(period.from), period.to.max(previous.to)).await?;
    let facts = payee_facts(&rows);
    let now = payees::totals(&facts, period);
    let before = payees::totals(&facts, previous);
    let all = db::payees::list(&db.read).await?;
    let groups: HashMap<PayeeGroupId, String> =
        db::payees::groups(&db.read).await?.into_iter().map(|g| (g.id, g.name)).collect();

    let payee_line = |p: &PayeeRow| {
        let totals = now.get(&p.id).copied().unwrap_or_default();
        let (amount, uncategorized) = amounts(&totals, kind);
        let (previous, _) = amounts(&before.get(&p.id).copied().unwrap_or_default(), kind);
        PayeeLine {
            payee_id: Some(p.id),
            group_id: p.group_id,
            name: p.name.clone(),
            amount_minor: amount.minor(),
            previous_minor: previous.minor(),
            uncategorized_minor: uncategorized.minor(),
            transactions: totals.transactions,
            children: Vec::new(),
        }
    };
    let visible = |l: &PayeeLine| l.amount_minor != 0 || l.previous_minor != 0 || l.uncategorized_minor != 0;
    let payee_lines: Vec<PayeeLine> = all.iter().map(payee_line).filter(visible).collect();

    let group_by = query.group_by.unwrap_or_default();
    let mut lines: Vec<PayeeLine> = match group_by {
        ReportGrouping::Payee => payee_lines,
        ReportGrouping::Group => {
            let mut grouped: HashMap<PayeeGroupId, PayeeLine> = HashMap::new();
            let mut lines = Vec::new();
            for line in payee_lines {
                match line.group_id {
                    Some(group) => {
                        let entry = grouped.entry(group).or_insert_with(|| PayeeLine {
                            payee_id: None,
                            group_id: Some(group),
                            name: groups.get(&group).cloned().unwrap_or_default(),
                            amount_minor: 0,
                            previous_minor: 0,
                            uncategorized_minor: 0,
                            transactions: 0,
                            children: Vec::new(),
                        });
                        // Un groupe somme ses tiers (R-TIE-20).
                        entry.amount_minor += line.amount_minor;
                        entry.previous_minor += line.previous_minor;
                        entry.uncategorized_minor += line.uncategorized_minor;
                        entry.transactions += line.transactions;
                        entry.children.push(line);
                    }
                    None => lines.push(line),
                }
            }
            lines.extend(grouped.into_values());
            lines
        }
    };
    let order = |a: &PayeeLine, b: &PayeeLine| {
        b.amount_minor
            .cmp(&a.amount_minor)
            .then(b.uncategorized_minor.cmp(&a.uncategorized_minor))
            .then(a.name.cmp(&b.name))
    };
    for line in &mut lines {
        line.children.sort_by(order);
    }
    lines.sort_by(order);
    let sum = |f: fn(&PayeeLine) -> i64| lines.iter().map(f).sum::<i64>();
    Ok(PayeeReport {
        kind,
        period: query.period.unwrap_or_default(),
        group_by,
        from: period.from,
        to: period.to,
        current,
        previous_from: previous.from,
        previous_to: previous.to,
        total_minor: sum(|l| l.amount_minor),
        previous_total_minor: sum(|l| l.previous_minor),
        uncategorized_minor: sum(|l| l.uncategorized_minor),
        lines,
    })
}

fn totals_view(totals: PayeeTotals, period: Period) -> TotalsView {
    TotalsView {
        from: period.from,
        to: period.to,
        income_minor: totals.income.minor(),
        expenses_minor: totals.expenses.minor(),
        uncategorized_in_minor: totals.uncategorized_in.minor(),
        uncategorized_out_minor: totals.uncategorized_out.minor(),
        transactions: totals.transactions,
        average_expense_minor: totals.average_expense().map(Money::minor),
    }
}

/// Fiche commune à un tiers (un membre) et à un groupe (ses tiers).
async fn summary(
    db: &Db,
    members: &[PayeeRow],
    today: Date,
) -> AppResult<(TotalsView, TotalsView, Vec<PayeeMonth>, Vec<CategoryAmount>)> {
    let months = reporting::months_until(today, 24);
    let start = months.first().map_or(today, |p| p.from);
    let ids: HashSet<PayeeId> = members.iter().map(|p| p.id).collect();
    let rows: Vec<_> = facts(db, start, today).await?.into_iter().filter(|r| ids.contains(&r.payee_id)).collect();
    let facts = payee_facts(&rows);
    let sum_over = |period: Period| -> PayeeTotals { payees::totals(&facts, period).into_values().sum() };

    let last_12 = Period { from: reporting::months_until(today, 12).first().map_or(today, |p| p.from), to: today };
    let year = Period { from: Date::from_ymd_opt(today.year(), 1, 1).unwrap_or(today), to: today };
    let monthly = months
        .iter()
        .map(|p| {
            let period = Period { from: p.from, to: p.to.min(today) };
            let by_payee = payees::totals(&facts, period);
            let total: PayeeTotals = by_payee.values().copied().sum();
            let mut split: Vec<PayeeAmount> = members
                .iter()
                .filter_map(|m| {
                    by_payee.get(&m.id).filter(|t| !t.expenses.is_zero()).map(|t| PayeeAmount {
                        payee_id: m.id,
                        name: m.name.clone(),
                        expenses_minor: t.expenses.minor(),
                    })
                })
                .collect();
            split.sort_by_key(|a| std::cmp::Reverse(a.expenses_minor));
            PayeeMonth {
                month: format!("{:04}-{:02}", p.from.year(), p.from.month()),
                expenses_minor: total.expenses.minor(),
                income_minor: total.income.minor(),
                by_payee: split,
            }
        })
        .collect();

    let categorized: Vec<CategorizedFact<CategoryId>> = rows
        .iter()
        .map(|r| CategorizedFact {
            date: r.date,
            amount: r.amount,
            category: r.category_id,
            kind: r.category_kind,
            is_movement: r.is_movement,
            is_eur: r.currency == Currency::EUR.code(),
        })
        .collect();
    let all_categories = db::categories::list(&db.read).await?;
    let names: HashMap<CategoryId, String> = all_categories.iter().map(|c| (c.id, c.name.clone())).collect();
    let full_name = |id: CategoryId| {
        let category = all_categories.iter().find(|c| c.id == id);
        match category.and_then(|c| c.parent_id).and_then(|p| names.get(&p)) {
            Some(parent) => format!("{parent} › {}", names.get(&id).cloned().unwrap_or_default()),
            None => names.get(&id).cloned().unwrap_or_default(),
        }
    };
    let mut categories: Vec<CategoryAmount> = reporting::by_category(&categorized, CategoryKind::Expense, last_12)
        .into_iter()
        .filter(|(_, amount)| !amount.is_zero())
        .map(|(category, amount)| CategoryAmount {
            category_id: category,
            name: category.map_or_else(|| "Non catégorisé".into(), full_name),
            amount_minor: amount.minor(),
        })
        .collect();
    categories.sort_by(|a, b| b.amount_minor.cmp(&a.amount_minor).then(a.name.cmp(&b.name)));
    Ok((totals_view(sum_over(last_12), last_12), totals_view(sum_over(year), year), monthly, categories))
}

pub async fn payee_summary(db: &Db, id: PayeeId, today: Date) -> AppResult<PayeeSummary> {
    let payee = db::payees::find(&db.read, id).await?.ok_or(AppError::NotFound("tiers"))?;
    let group = match payee.group_id {
        Some(group) => db::payees::find_group(&db.read, group).await?,
        None => None,
    };
    let (last_12_months, year, months, categories) = summary(db, std::slice::from_ref(&payee), today).await?;
    Ok(PayeeSummary {
        name: payee.name,
        note: payee.note,
        payee_id: Some(payee.id),
        group_id: payee.group_id,
        group_name: group.map(|g| g.name),
        archived: payee.archived,
        members: Vec::new(),
        last_12_months,
        year,
        months,
        categories,
    })
}

pub async fn group_summary(db: &Db, id: PayeeGroupId, today: Date) -> AppResult<PayeeSummary> {
    let group = db::payees::find_group(&db.read, id).await?.ok_or(AppError::NotFound("groupe de tiers"))?;
    let members: Vec<PayeeRow> =
        db::payees::list(&db.read).await?.into_iter().filter(|p| p.group_id == Some(id)).collect();
    let (last_12_months, year, months, categories) = summary(db, &members, today).await?;
    Ok(PayeeSummary {
        name: group.name,
        note: group.note,
        payee_id: None,
        group_id: Some(group.id),
        group_name: None,
        archived: false,
        members: members.into_iter().map(|p| PayeeRef { id: p.id, name: p.name }).collect(),
        last_12_months,
        year,
        months,
        categories,
    })
}
