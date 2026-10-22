//! Projections (spec 07) : scénarios (PRJ-03), trésorerie (PRJ-01), épargne fixe (PRJ-02) et variable
//! (PRJ-04), patrimoine (PRJ-05) et son historique (TDB-04). Rien n'est écrit : ce sont des calculs purs
//! sur l'état réel du jour (R-PRJ-1).

use std::collections::{BTreeMap, HashMap, HashSet};

use cashflow_domain::accounts::AccountFamily;
use cashflow_domain::ledger::TransactionOrigin;
use cashflow_domain::projection::{self, Assumption, CashEvent, FixedInput};
use cashflow_domain::recurrence::{self, Occurrence};
use cashflow_domain::reporting;
use cashflow_domain::savings::{FortnightlyTerms, Movement, RatePoint};
use cashflow_domain::shared::ids::{AccountId, ScenarioId, TransactionId};
use cashflow_domain::{Date, Money, Rate};
use chrono::{Datelike, Days};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::accounts::Ledger;
use super::recurring;
use crate::db::accounts::AccountRow;
use crate::db::codec::enum_to_db;
use crate::db::projections::ScenarioRow;
use crate::db::recurring::RuleRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------- Scénarios (PRJ-03)

/// Rendement et frais annuels, en millionièmes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Yield {
    pub annual_return_ppm: i64,
    #[serde(default)]
    pub annual_fees_ppm: i64,
}

/// Hypothèses propres à un compte : elles l'emportent sur la nature et la famille (R-PRJ-31).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccountAssumption {
    pub annual_return_ppm: Option<i64>,
    pub annual_fees_ppm: Option<i64>,
    /// Remplace les versements programmés des récurrences (R-PRJ-4).
    pub monthly_contribution_minor: Option<i64>,
}

/// Taux d'un barème à partir d'une date (R-PRJ-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RateStep {
    pub from: Date,
    pub rate_ppm: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Assumptions {
    /// Par famille de comptes (`VARIABLE_SAVINGS`…).
    #[serde(default)]
    pub by_family: BTreeMap<String, Yield>,
    /// Par nature de compte (`LIFE_INSURANCE`, `PEA`…).
    #[serde(default)]
    pub by_kind: BTreeMap<String, Yield>,
    /// Par compte (identifiant).
    #[serde(default)]
    pub by_account: BTreeMap<String, AccountAssumption>,
    /// Trajectoire de taux par barème (identifiant), au-delà du dernier taux connu.
    #[serde(default)]
    pub rate_paths: BTreeMap<String, Vec<RateStep>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ScenarioView {
    pub id: ScenarioId,
    pub name: String,
    pub horizon_months: u32,
    pub inflation_rate_ppm: Option<i64>,
    pub assumptions: Assumptions,
    pub is_default: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ScenarioInput {
    pub name: String,
    pub horizon_months: u32,
    pub inflation_rate_ppm: Option<i64>,
    pub assumptions: Assumptions,
    #[serde(default)]
    pub is_default: bool,
}

fn to_view(row: ScenarioRow) -> ScenarioView {
    ScenarioView {
        id: row.id,
        name: row.name,
        horizon_months: row.horizon_months,
        inflation_rate_ppm: row.inflation_rate.map(Rate::ppm),
        assumptions: serde_json::from_value(row.assumptions).unwrap_or_default(),
        is_default: row.is_default,
    }
}

pub async fn scenarios(db: &Db) -> AppResult<Vec<ScenarioView>> {
    Ok(db::projections::scenarios(&db.read).await?.into_iter().map(to_view).collect())
}

pub async fn save_scenario(db: &Db, id: Option<ScenarioId>, input: &ScenarioInput) -> AppResult<ScenarioView> {
    let name = super::required(&input.name, "name", 60)?;
    if !(1..=600).contains(&input.horizon_months) {
        return Err(AppError::invalid_field("INVALID_HORIZON", "horizon_months", "entre 1 et 600 mois"));
    }
    let existing = db::projections::scenarios(&db.read).await?;
    if existing.iter().any(|s| Some(s.id) != id && s.name.eq_ignore_ascii_case(&name)) {
        return Err(AppError::invalid_field("SCENARIO_NAME_TAKEN", "name", "un scénario porte déjà ce nom"));
    }
    if let Some(id) = id
        && !existing.iter().any(|s| s.id == id)
    {
        return Err(AppError::NotFound("scénario"));
    }
    let row = ScenarioRow {
        id: id.unwrap_or_else(ScenarioId::new),
        name,
        horizon_months: input.horizon_months,
        inflation_rate: input.inflation_rate_ppm.map(Rate::from_ppm),
        assumptions: serde_json::to_value(&input.assumptions).map_err(anyhow::Error::from)?,
        // Le premier scénario est le scénario par défaut.
        is_default: input.is_default || existing.is_empty(),
    };
    let mut tx = db.write.begin().await?;
    db::projections::upsert(&mut tx, &row).await?;
    tx.commit().await?;
    Ok(to_view(row))
}

pub async fn delete_scenario(db: &Db, id: ScenarioId) -> AppResult<()> {
    let all = db::projections::scenarios(&db.read).await?;
    if all.iter().any(|s| s.id == id && s.is_default) {
        return Err(AppError::invalid(
            "DEFAULT_SCENARIO",
            "choisir un autre scénario par défaut avant de supprimer celui-ci",
        ));
    }
    if db::projections::delete(&db.write, id).await? == 0 {
        return Err(AppError::NotFound("scénario"));
    }
    Ok(())
}

/// Les trois scénarios fournis par défaut (R-PRJ-31) : des hypothèses, non des prévisions.
pub async fn ensure_default_scenarios(db: &Db) -> anyhow::Result<()> {
    if !db::projections::scenarios(&db.read).await?.is_empty() {
        return Ok(());
    }
    let defaults = [
        ("Prudent", 15_000, 20_000, 20_000, false),
        ("Médian", 25_000, 50_000, 40_000, true),
        ("Optimiste", 35_000, 70_000, 60_000, false),
    ];
    let mut tx = db.write.begin().await?;
    for (name, life, equities, per, is_default) in defaults {
        let y = |ppm| Yield { annual_return_ppm: ppm, annual_fees_ppm: 0 };
        let mut assumptions = Assumptions::default();
        assumptions.by_family.insert("VARIABLE_SAVINGS".into(), y(life));
        assumptions.by_kind.insert("LIFE_INSURANCE".into(), y(life));
        for kind in ["PEA", "PEA_PME", "SECURITIES"] {
            assumptions.by_kind.insert(kind.into(), y(equities));
        }
        assumptions.by_kind.insert("PER".into(), y(per));
        let row = ScenarioRow {
            id: ScenarioId::new(),
            name: name.into(),
            horizon_months: 240,
            inflation_rate: None,
            assumptions: serde_json::to_value(&assumptions)?,
            is_default,
        };
        db::projections::upsert(&mut tx, &row).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn scenario(db: &Db, id: Option<ScenarioId>) -> AppResult<ScenarioView> {
    let all = scenarios(db).await?;
    let found = match id {
        Some(id) => all.into_iter().find(|s| s.id == id),
        None => all.into_iter().max_by_key(|s| s.is_default),
    };
    found.ok_or(AppError::NotFound("scénario"))
}

// ---------------------------------------------------------------- Trésorerie (PRJ-01)

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CashQuery {
    /// Jours projetés (90 par défaut).
    pub days: Option<u32>,
    /// Inclure la dépense variable moyenne (oui par défaut, R-PRJ-11).
    pub variable: Option<bool>,
    /// Seuil d'avertissement, en centimes (0 par défaut, R-PRJ-12).
    pub threshold_minor: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CashEventKind {
    /// Transaction en attente datée dans le futur.
    Pending,
    /// Échéance d'une récurrence, non encore honorée.
    Scheduled,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CashEventView {
    pub date: Date,
    pub label: String,
    pub amount_minor: i64,
    pub kind: CashEventKind,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CashDayView {
    pub date: Date,
    pub balance_minor: i64,
    pub variable_minor: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CashProjectionView {
    pub account_id: AccountId,
    pub today: Date,
    pub start_minor: i64,
    /// Dépense variable moyenne d'un mois, si elle est incluse (R-PRJ-11).
    pub variable_monthly_minor: Option<i64>,
    pub days: Vec<CashDayView>,
    pub events: Vec<CashEventView>,
    pub minimum_minor: i64,
    pub minimum_date: Date,
    pub threshold_minor: i64,
    /// Le solde projeté passe sous le seuil (R-PRJ-12).
    pub below_threshold: bool,
}

/// Mois complets de la dépense variable moyenne (R-PRJ-11).
const VARIABLE_MONTHS: u32 = 3;

/// Échéances à venir et non honorées d'une récurrence, vues depuis `account` (source ou cible).
fn scheduled_for(
    rules: &[RuleRow],
    account: AccountId,
    today: Date,
    to: Date,
    candidates: &[db::recurring::CandidateRow],
) -> Vec<(Date, Money, String)> {
    let mut result = Vec::new();
    for rule in rules.iter().filter(|r| r.active) {
        let sign = if rule.account_id == account {
            Money::from_minor(1)
        } else if rule.target_account_id == Some(account) {
            Money::from_minor(-1)
        } else {
            continue;
        };
        let start = today.checked_add_days(Days::new(1)).unwrap_or(today);
        for o in recurring::occurrences(rule, start, to, candidates, today) {
            if let Occurrence::Upcoming { date } = o {
                let amount = if sign.is_positive() { rule.amount } else { -rule.amount };
                result.push((date, amount, rule.label.clone()));
            }
        }
    }
    result
}

pub async fn cash(db: &Db, account: AccountId, query: &CashQuery, today: Date) -> AppResult<CashProjectionView> {
    let row = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    if row.kind.family() != AccountFamily::Cash {
        return Err(AppError::invalid(
            "NOT_CASH_ACCOUNT",
            "la projection de trésorerie concerne les comptes de liquidités",
        ));
    }
    let days = query.days.unwrap_or(90).clamp(1, 366);
    let horizon = today.checked_add_days(Days::new(u64::from(days))).unwrap_or(today);
    let start = super::accounts::get(db, account, today).await?.balance_minor;

    let mut events: Vec<CashEventView> = db::projections::future(&db.read, account, today)
        .await?
        .into_iter()
        .filter(|t| t.date <= horizon)
        .map(|t| CashEventView {
            date: t.date,
            label: t.label,
            amount_minor: t.amount.minor(),
            kind: CashEventKind::Pending,
        })
        .collect();
    // Échéances de prêt payées par ce compte (R-PRJ-10). Une récurrence du compte au montant de l'échéance (±1 €)
    // est ignorée : la mensualité ne compte qu'une fois.
    let upcoming = super::loans::upcoming_debits(db, account, today, horizon).await?;
    let installment_amounts: Vec<Money> = upcoming.iter().map(|u| u.amount).collect();
    let rules: Vec<_> = db::recurring::list(&db.read)
        .await?
        .into_iter()
        .filter(|r| {
            !(r.account_id == account
                && r.target_account_id.is_none()
                && installment_amounts
                    .iter()
                    .any(|a| (r.amount - *a).abs() <= cashflow_domain::loans::AMOUNT_TOLERANCE))
        })
        .collect();
    events.extend(upcoming.into_iter().map(|u| CashEventView {
        date: u.date,
        label: u.label,
        amount_minor: u.amount.minor(),
        kind: CashEventKind::Scheduled,
    }));
    let candidates = db::recurring::candidates(
        &db.read,
        today.checked_sub_days(Days::new(10)).unwrap_or(today),
        horizon.checked_add_days(Days::new(6)).unwrap_or(horizon),
    )
    .await?;
    events.extend(scheduled_for(&rules, account, today, horizon, &candidates).into_iter().map(
        |(date, amount, label)| CashEventView {
            date,
            label,
            amount_minor: amount.minor(),
            kind: CashEventKind::Scheduled,
        },
    ));
    events.sort_by_key(|e| e.date);

    // Dépense variable : dépenses non récurrentes des 3 derniers mois complets (R-PRJ-11).
    let variable = if query.variable.unwrap_or(true) {
        let last = reporting::first_of_month(today).pred_opt().unwrap_or(today);
        let months = reporting::months_until(last, VARIABLE_MONTHS);
        let first = months.first().map_or(today, |p| p.from);
        let recurring = recurring::honoring(db, first, last, today).await?;
        // Les mensualités ventilées sont projetées à part, par leurs échéances.
        let installments: HashSet<TransactionId> =
            db::loans::payment_transactions(&db.read).await?.into_iter().collect();
        let spent: Money = db::projections::expenses(&db.read, account, first, last)
            .await?
            .into_iter()
            .filter(|e| !recurring.contains_key(&e.id) && !installments.contains(&e.id))
            .map(|e| -e.amount)
            .sum();
        let months_with_data = i64::from(VARIABLE_MONTHS).max(1);
        Some(Money::from_minor((spent.minor() / months_with_data).max(0)))
    } else {
        None
    };

    let domain_events: Vec<CashEvent<()>> = events
        .iter()
        .map(|e| CashEvent { date: e.date, amount: Money::from_minor(e.amount_minor), label: () })
        .collect();
    let result = projection::cash(Money::from_minor(start), today, days, &domain_events, variable);
    let threshold = query.threshold_minor.unwrap_or(0);
    Ok(CashProjectionView {
        account_id: account,
        today,
        start_minor: start,
        variable_monthly_minor: variable.map(Money::minor),
        days: result
            .days
            .iter()
            .map(|d| CashDayView { date: d.date, balance_minor: d.balance.minor(), variable_minor: d.variable.minor() })
            .collect(),
        events,
        minimum_minor: result.minimum.minor(),
        minimum_date: result.minimum_date,
        threshold_minor: threshold,
        below_threshold: result.minimum.minor() < threshold,
    })
}

// ---------------------------------------------------------------- Comptes d'épargne (PRJ-02, PRJ-04)

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProjectionQuery {
    /// Scénario ; celui par défaut sinon.
    pub scenario_id: Option<ScenarioId>,
    /// Horizon en mois ; celui du scénario sinon.
    pub months: Option<u32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProjectionPoint {
    pub date: Date,
    pub value_minor: i64,
    /// En euros constants, si le scénario définit une inflation (R-PRJ-33).
    pub real_value_minor: Option<i64>,
}

/// Hypothèses retenues, affichées avec la projection (R-PRJ-3).
#[derive(Debug, Serialize, ToSchema)]
pub struct AccountProjectionView {
    pub account_id: AccountId,
    pub name: String,
    pub family: AccountFamily,
    pub scenario_id: ScenarioId,
    pub scenario_name: String,
    pub start_date: Date,
    pub start_minor: i64,
    /// La valeur de départ vient d'une valorisation de plus de 90 jours (R-PRJ-3).
    pub estimated: bool,
    pub annual_return_ppm: Option<i64>,
    pub annual_fees_ppm: Option<i64>,
    /// Taux du barème retenu au départ (épargne à taux déterminé).
    pub rate_ppm: Option<i64>,
    pub monthly_contribution_minor: i64,
    pub deposit_cap_minor: Option<i64>,
    pub points: Vec<ProjectionPoint>,
}

fn resolve(assumptions: &Assumptions, row: &AccountRow) -> (Yield, Option<i64>) {
    let base = assumptions
        .by_kind
        .get(&enum_to_db(&row.kind))
        .or_else(|| assumptions.by_family.get(&enum_to_db(&row.kind.family())))
        .copied()
        .unwrap_or_default();
    let own = assumptions.by_account.get(&row.id.to_string()).cloned().unwrap_or_default();
    (
        Yield {
            annual_return_ppm: own.annual_return_ppm.unwrap_or(base.annual_return_ppm),
            annual_fees_ppm: own.annual_fees_ppm.unwrap_or(base.annual_fees_ppm),
        },
        own.monthly_contribution_minor,
    )
}

/// Versements programmés vers un compte : virements récurrents qui le visent, et récurrences du compte
/// lui-même (R-PRJ-4).
fn scheduled_deposits(rules: &[RuleRow], account: AccountId, today: Date, to: Date) -> Vec<Movement> {
    let start = today.checked_add_days(Days::new(1)).unwrap_or(today);
    rules
        .iter()
        .filter(|r| r.active)
        .flat_map(|r| {
            let amount = if r.target_account_id == Some(account) {
                Some(-r.amount)
            } else if r.account_id == account {
                Some(r.amount)
            } else {
                None
            };
            amount
                .map(|amount| {
                    recurring::schedule(r)
                        .occurrences(start, to)
                        .into_iter()
                        .map(move |date| Movement { date, amount })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect()
}

/// Équivalent mensuel des versements programmés, pour le modèle variable.
fn monthly_equivalent(rules: &[RuleRow], account: AccountId) -> Money {
    rules
        .iter()
        .filter(|r| r.active)
        .filter_map(|r| {
            let amount = if r.target_account_id == Some(account) {
                -r.amount
            } else if r.account_id == account {
                r.amount
            } else {
                return None;
            };
            let per_year = i64::from(recurrence::per_year(&recurring::schedule(r)));
            Some(Money::from_minor(amount.minor() * per_year / 12))
        })
        .sum()
}

struct Context {
    ledger: Ledger,
    rules: Vec<RuleRow>,
    terms: HashMap<AccountId, db::savings::TermsRow>,
    periods: Vec<db::savings::PeriodRow>,
}

impl Context {
    async fn load(db: &Db) -> AppResult<Self> {
        Ok(Context {
            ledger: Ledger::load(db).await?,
            rules: db::recurring::list(&db.read).await?,
            terms: db::savings::all_terms(&db.read).await?.into_iter().map(|t| (t.account_id, t)).collect(),
            periods: db::savings::periods(&db.read).await?,
        })
    }
}

fn points(values: Vec<(Date, Money)>, scenario: &ScenarioView) -> Vec<ProjectionPoint> {
    values
        .into_iter()
        .enumerate()
        .map(|(i, (date, value))| ProjectionPoint {
            date,
            value_minor: value.minor(),
            real_value_minor: scenario
                .inflation_rate_ppm
                .map(|ppm| projection::deflate(value, Rate::from_ppm(ppm), u32::try_from(i + 1).unwrap_or(0)).minor()),
        })
        .collect()
}

/// Ce qu'il faut pour projeter un compte à taux déterminé (R-PRJ-20 à R-PRJ-22).
struct FixedParts {
    movements: Vec<Movement>,
    rates: Vec<RatePoint>,
    terms: FortnightlyTerms,
    deposits: Vec<Movement>,
    cap: Option<Money>,
    capital: Money,
    recorded: Vec<i32>,
}

async fn fixed_parts(
    db: &Db,
    ctx: &Context,
    row: &AccountRow,
    scenario: &ScenarioView,
    contribution: Option<i64>,
    today: Date,
    ends: &[Date],
) -> AppResult<FixedParts> {
    let horizon = ends.last().copied().unwrap_or(today);
    let terms = ctx.terms.get(&row.id);
    let mut rates: Vec<RatePoint> = ctx
        .periods
        .iter()
        .filter(|p| terms.is_some_and(|t| t.rate_schedule_id == p.schedule_id))
        .map(|p| RatePoint { from: p.effective_from, rate: p.rate })
        .collect();
    if let Some(path) = terms.and_then(|t| scenario.assumptions.rate_paths.get(&t.rate_schedule_id.to_string())) {
        rates.extend(path.iter().map(|s| RatePoint { from: s.from, rate: Rate::from_ppm(s.rate_ppm) }));
    }
    let rows = db::savings::movements(&db.read, row.id, terms.and_then(|t| t.interest_category_id)).await?;
    let is_interest = |m: &db::savings::MovementRow| m.origin == TransactionOrigin::Interest || m.is_interest;
    let mut movements = vec![Movement { date: row.opening_date, amount: row.opening_balance }];
    movements.extend(rows.iter().map(|m| Movement { date: m.interest_date, amount: m.amount }));
    let capital = row.opening_balance + rows.iter().filter(|m| !is_interest(m)).map(|m| m.amount).sum::<Money>();
    let recorded = rows
        .iter()
        .filter(|m| is_interest(m))
        .map(|m| if m.balance_date.month() == 1 { m.balance_date.year() - 1 } else { m.balance_date.year() })
        .collect();
    let deposits = match contribution {
        Some(monthly) => ends.iter().map(|d| Movement { date: *d, amount: Money::from_minor(monthly) }).collect(),
        None => scheduled_deposits(&ctx.rules, row.id, today, horizon),
    };
    Ok(FixedParts {
        movements,
        rates,
        terms: FortnightlyTerms {
            opened: row.opening_date,
            closed: row.closed_on,
            round_each: terms.is_some_and(|t| t.round_each_fortnight),
        },
        deposits,
        cap: terms.and_then(|t| t.deposit_cap),
        capital,
        recorded,
    })
}

async fn project_account(
    db: &Db,
    ctx: &Context,
    row: &AccountRow,
    scenario: &ScenarioView,
    months: u32,
    today: Date,
) -> AppResult<AccountProjectionView> {
    let ends = projection::month_ends(today, months);
    let horizon = ends.last().copied().unwrap_or(today);
    let (current, valuation_date) = ctx.ledger.value(row, today);
    let (yields, contribution) = resolve(&scenario.assumptions, row);
    let mut view = AccountProjectionView {
        account_id: row.id,
        name: row.name.clone(),
        family: row.kind.family(),
        scenario_id: scenario.id,
        scenario_name: scenario.name.clone(),
        start_date: today,
        start_minor: current.minor(),
        estimated: false,
        annual_return_ppm: None,
        annual_fees_ppm: None,
        rate_ppm: None,
        monthly_contribution_minor: 0,
        deposit_cap_minor: None,
        points: Vec::new(),
    };
    let loan_terms = match row.kind.family() {
        AccountFamily::Liability => db::loans::find(&db.read, row.id).await?,
        _ => None,
    };
    match row.kind.family() {
        // Prêt avec conditions : la dette suit le tableau d'amortissement (R-PRJ-40).
        AccountFamily::Liability if let Some(terms) = &loan_terms => {
            let schedule = cashflow_domain::loans::schedule(&terms.terms)?;
            let values = cashflow_domain::loans::projected_debt(current, &schedule, today, &ends);
            view.points = points(ends.into_iter().zip(values).collect(), scenario);
        }
        AccountFamily::FixedSavings => {
            let parts = fixed_parts(db, ctx, row, scenario, contribution, today, &ends).await?;
            let (rates, terms) = (&parts.rates, ctx.terms.get(&row.id));
            let input = FixedInput {
                movements: &parts.movements,
                rates: &parts.rates,
                terms: parts.terms,
                deposits: &parts.deposits,
                cap: parts.cap,
                capital: parts.capital,
                recorded_years: &parts.recorded,
            };
            view.rate_ppm = Some(cashflow_domain::savings::rate_at(rates, today).ppm());
            view.deposit_cap_minor = terms.and_then(|t| t.deposit_cap).map(Money::minor);
            view.monthly_contribution_minor =
                contribution.unwrap_or_else(|| monthly_equivalent(&ctx.rules, row.id).minor());
            view.points = points(projection::fixed(&input, today, months), scenario);
        }
        AccountFamily::VariableSavings => {
            let monthly = contribution.map_or_else(|| monthly_equivalent(&ctx.rules, row.id), Money::from_minor);
            let assumption = Assumption {
                annual_return: Rate::from_ppm(yields.annual_return_ppm),
                annual_fees: Rate::from_ppm(yields.annual_fees_ppm),
                monthly_contribution: monthly,
            };
            view.estimated = valuation_date.is_none_or(|d| (today - d).num_days() > 90);
            view.annual_return_ppm = Some(yields.annual_return_ppm);
            view.annual_fees_ppm = Some(yields.annual_fees_ppm);
            view.monthly_contribution_minor = monthly.minor();
            let values = projection::variable(current, assumption, months);
            view.points = points(ends.into_iter().zip(values).collect(), scenario);
        }
        AccountFamily::Cash | AccountFamily::Liability | AccountFamily::Property => {
            // Liquidités : solde constant, moins les virements programmés vers l'épargne (R-PRJ-40, R-PRJ-41).
            // Prêts : dernière valeur, constante faute de tableau d'amortissement. Biens : dernière valeur,
            // sans plus-value ni dépréciation supposées (ADR-0019).
            let start = today.checked_add_days(Days::new(1)).unwrap_or(today);
            // Virements programmés, par compte de contrepartie : (date, montant signé sur ce compte).
            let mut flows: HashMap<AccountId, Vec<(Date, Money)>> = HashMap::new();
            if row.kind.family() == AccountFamily::Cash {
                for rule in ctx.rules.iter().filter(|r| r.active) {
                    let (other, amount) = match rule.target_account_id {
                        Some(target) if rule.account_id == row.id => (target, rule.amount),
                        Some(_) if rule.target_account_id == Some(row.id) => (rule.account_id, -rule.amount),
                        _ => continue,
                    };
                    let dates = recurring::schedule(rule).occurrences(start, horizon);
                    flows.entry(other).or_default().extend(dates.into_iter().map(|d| (d, amount)));
                }
            }
            // Vers un compte plafonné, seuls partent les versements qu'il accepte (R-PRJ-22) : sinon le
            // patrimoine fuirait.
            let mut accepted: HashMap<AccountId, Vec<Movement>> = HashMap::new();
            for other in flows.keys() {
                let Some(target) = db::accounts::find(&db.read, *other).await? else { continue };
                if target.kind.family() != AccountFamily::FixedSavings {
                    continue;
                }
                let own = scenario
                    .assumptions
                    .by_account
                    .get(&target.id.to_string())
                    .and_then(|a| a.monthly_contribution_minor);
                let parts = fixed_parts(db, ctx, &target, scenario, own, today, &ends).await?;
                if parts.cap.is_some() {
                    accepted.insert(
                        *other,
                        projection::accepted(&parts.deposits, parts.cap, parts.capital, today, horizon),
                    );
                }
            }
            let values = ends
                .iter()
                .map(|end| {
                    let moved: Money = flows
                        .iter()
                        .map(|(other, list)| {
                            let out: Money =
                                list.iter().filter(|(d, a)| d <= end && a.is_negative()).map(|(_, a)| *a).sum();
                            let inflow: Money =
                                list.iter().filter(|(d, a)| d <= end && a.is_positive()).map(|(_, a)| *a).sum();
                            let out = match accepted.get(other) {
                                Some(taken) => {
                                    let taken: Money = taken.iter().filter(|m| m.date <= *end).map(|m| m.amount).sum();
                                    -((-out).min(taken))
                                }
                                None => out,
                            };
                            out + inflow
                        })
                        .sum();
                    (*end, current + moved)
                })
                .collect();
            view.points = points(values, scenario);
        }
    }
    Ok(view)
}

pub async fn account(db: &Db, id: AccountId, query: &ProjectionQuery, today: Date) -> AppResult<AccountProjectionView> {
    let row = db::accounts::find(&db.read, id).await?.ok_or(AppError::NotFound("compte"))?;
    let scenario = scenario(db, query.scenario_id).await?;
    let months = query.months.unwrap_or(scenario.horizon_months).clamp(1, 600);
    let ctx = Context::load(db).await?;
    project_account(db, &ctx, &row, &scenario, months, today).await
}

// ---------------------------------------------------------------- Patrimoine (PRJ-05, TDB-04)

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NetWorthQuery {
    /// Horizon en mois (celui du scénario par défaut sinon).
    pub months: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ScenarioSeries {
    pub scenario_id: ScenarioId,
    pub name: String,
    pub is_default: bool,
    pub points: Vec<ProjectionPoint>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetWorthProjection {
    pub today: Date,
    pub start_minor: i64,
    /// Comptes partis d'une valorisation ancienne (R-PRJ-3).
    pub estimated_accounts: Vec<String>,
    pub scenarios: Vec<ScenarioSeries>,
}

/// Patrimoine projeté sous chaque scénario (R-PRJ-40, R-PRJ-41), pour les comparer (PRJ-05).
pub async fn net_worth(db: &Db, query: &NetWorthQuery, today: Date) -> AppResult<NetWorthProjection> {
    let all = scenarios(db).await?;
    let default_horizon = all.iter().find(|s| s.is_default).map_or(240, |s| s.horizon_months);
    let months = query.months.unwrap_or(default_horizon).clamp(1, 600);
    let ctx = Context::load(db).await?;
    let rows: Vec<AccountRow> = db::accounts::list(&db.read)
        .await?
        .into_iter()
        .filter(|a| a.include_in_net_worth && a.closed_on.is_none_or(|c| c > today) && a.opening_date <= today)
        .collect();
    let start: Money = rows.iter().map(|r| ctx.ledger.value(r, today).0).sum();
    let mut series = Vec::new();
    let mut estimated = Vec::new();
    for scenario in &all {
        let mut totals: Vec<(Date, Money)> =
            projection::month_ends(today, months).into_iter().map(|d| (d, Money::ZERO)).collect();
        for row in &rows {
            let p = project_account(db, &ctx, row, scenario, months, today).await?;
            if p.estimated && scenario.is_default {
                estimated.push(p.name.clone());
            }
            for (total, point) in totals.iter_mut().zip(&p.points) {
                total.1 += Money::from_minor(point.value_minor);
            }
        }
        series.push(ScenarioSeries {
            scenario_id: scenario.id,
            name: scenario.name.clone(),
            is_default: scenario.is_default,
            points: points(totals, scenario),
        });
    }
    Ok(NetWorthProjection { today, start_minor: start.minor(), estimated_accounts: estimated, scenarios: series })
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryQuery {
    /// Mois d'historique (24 par défaut).
    pub months: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetWorthPoint {
    pub date: Date,
    pub total_minor: i64,
    pub cash_minor: i64,
    pub fixed_savings_minor: i64,
    pub variable_savings_minor: i64,
    pub liabilities_minor: i64,
    pub property_minor: i64,
}

/// Historique du patrimoine par famille de comptes, en fin de mois puis aujourd'hui (TDB-04).
pub async fn history(db: &Db, query: &HistoryQuery, today: Date) -> AppResult<Vec<NetWorthPoint>> {
    let count = query.months.unwrap_or(24).clamp(1, 240);
    let ledger = Ledger::load(db).await?;
    let rows: Vec<AccountRow> =
        db::accounts::list(&db.read).await?.into_iter().filter(|a| a.include_in_net_worth).collect();
    let first_opening = rows.iter().map(|r| r.opening_date).min();
    let mut dates: Vec<Date> =
        reporting::months_until(today, count + 1).into_iter().map(|p| p.to).filter(|d| *d < today).collect();
    dates.push(today);
    Ok(dates
        .into_iter()
        .filter(|d| first_opening.is_some_and(|f| f <= *d))
        .map(|date| {
            let mut point = NetWorthPoint {
                date,
                total_minor: 0,
                cash_minor: 0,
                fixed_savings_minor: 0,
                variable_savings_minor: 0,
                liabilities_minor: 0,
                property_minor: 0,
            };
            for row in rows.iter().filter(|r| r.opening_date <= date && r.closed_on.is_none_or(|c| c > date)) {
                let value = ledger.value(row, date).0.minor();
                point.total_minor += value;
                match row.kind.family() {
                    AccountFamily::Cash => point.cash_minor += value,
                    AccountFamily::FixedSavings => point.fixed_savings_minor += value,
                    AccountFamily::VariableSavings => point.variable_savings_minor += value,
                    AccountFamily::Liability => point.liabilities_minor += value,
                    AccountFamily::Property => point.property_minor += value,
                }
            }
            point
        })
        .collect())
}
