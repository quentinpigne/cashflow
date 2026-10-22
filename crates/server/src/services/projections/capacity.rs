//! Capacité d'épargne dans la projection du patrimoine (R-PRJ-42 à R-PRJ-47) et projection du salaire (SAL-04),
//! ADR-0022.

use std::collections::{HashMap, HashSet};

use cashflow_domain::accounts::{AccountFamily, AccountKind};
use cashflow_domain::projection::{self, Baseline, CapacityFact, CapacityInput, CapacityMonth};
use cashflow_domain::reporting;
use cashflow_domain::savings::Movement;
use cashflow_domain::shared::ids::{AccountId, CategoryId, LoanId, ScenarioId};
use cashflow_domain::{Currency, Date, Money, Rate};
use chrono::{Datelike, Days, Months};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{Context, ScenarioView, fixed_parts, recurring};
use crate::db::accounts::AccountRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::services::loans::LoanBook;
use crate::services::salaries::SalaryBook;

/// Taux de hausse des salaires et des dépenses d'un scénario sans hypothèse (R-SAL-12, R-PRJ-44).
pub const DEFAULT_GROWTH_PPM: i64 = 20_000;
/// Mois de moyenne des autres revenus et des dépenses (R-PRJ-43, R-PRJ-44).
const BASELINE_MONTHS: u32 = 12;

fn yes() -> bool {
    true
}

fn default_growth() -> i64 {
    DEFAULT_GROWTH_PPM
}

/// Hypothèses de capacité d'épargne d'un scénario (R-PRJ-42 à R-PRJ-47).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CapacityAssumptions {
    /// Activée par défaut ; désactivée, les liquidités ne bougent que des versements programmés (R-PRJ-47).
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Taux de hausse des salaires (R-SAL-12).
    #[serde(default = "default_growth")]
    pub salary_growth_ppm: i64,
    /// Taux de hausse des dépenses, appliqué chaque 1er janvier (R-PRJ-44).
    #[serde(default = "default_growth")]
    pub expense_growth_ppm: i64,
    /// Compte d'épargne qui reçoit le reste de la capacité d'épargne (R-PRJ-45).
    #[serde(default)]
    pub surplus_account_id: Option<AccountId>,
}

impl Default for CapacityAssumptions {
    fn default() -> Self {
        Self {
            enabled: true,
            salary_growth_ppm: DEFAULT_GROWTH_PPM,
            expense_growth_ppm: DEFAULT_GROWTH_PPM,
            surplus_account_id: None,
        }
    }
}

/// Vérifie les hypothèses de capacité d'un scénario avant de l'enregistrer.
pub(super) async fn validate(db: &Db, assumptions: &CapacityAssumptions) -> AppResult<()> {
    for (field, value) in
        [("salary_growth_ppm", assumptions.salary_growth_ppm), ("expense_growth_ppm", assumptions.expense_growth_ppm)]
    {
        if !(-500_000..=1_000_000).contains(&value) {
            return Err(AppError::invalid_field("INVALID_GROWTH", field, "entre −50 % et 100 %"));
        }
    }
    if let Some(id) = assumptions.surplus_account_id {
        let row = db::accounts::find(&db.read, id).await?.ok_or(AppError::NotFound("compte d'affectation"))?;
        if row.kind.family() == AccountFamily::Cash {
            return Err(AppError::invalid_field(
                "SURPLUS_NOT_SAVINGS",
                "surplus_account_id",
                "le reste de la capacité d'épargne va sur un compte d'épargne (R-PRJ-45)",
            ));
        }
    }
    Ok(())
}

fn months_between(from: Date, to: Date) -> u32 {
    let span = (to.year() - from.year()) * 12 + i32::try_from(to.month()).unwrap_or(0)
        - i32::try_from(from.month()).unwrap_or(0);
    u32::try_from(span).unwrap_or(0)
}

/// Ce qui ne dépend pas du scénario : moyennes de départ, échéances, compte de versement (R-PRJ-43 à R-PRJ-45).
pub(super) struct CapacityBase {
    pub salaries: SalaryBook,
    pub baseline: Baseline,
    /// Mois complets moyennés : 12, ou moins si l'historique est plus court.
    pub months: u32,
    /// Échéances futures des prêts avec tableau : date et montant prélevé.
    installments: Vec<(Date, Money)>,
    pub payroll: Option<AccountId>,
    families: HashMap<AccountId, AccountFamily>,
    names: HashMap<AccountId, String>,
    /// Prêts avec tableau dont des échéances récentes ne sont pas payées (R-PRJ-44).
    pub unpaid_loans: Vec<String>,
}

impl CapacityBase {
    pub async fn load(db: &Db, today: Date) -> AppResult<Self> {
        let accounts = db::accounts::list(&db.read).await?;
        let salaries = SalaryBook::load(db).await?;
        let categories: HashSet<CategoryId> = salaries.categories().into_iter().collect();
        // Derniers mois complets, à partir du premier mois entier suivi.
        let last = reporting::end_of_previous_month(today);
        let window =
            reporting::first_of_month(last).checked_sub_months(Months::new(BASELINE_MONTHS - 1)).unwrap_or(last);
        let tracked = accounts
            .iter()
            .map(|a| a.opening_date)
            .min()
            .map(|o| if o.day() == 1 { o } else { reporting::first_of_month(o) + Months::new(1) });
        let from = tracked.map_or(window, |t| t.max(window));
        let months = if from > last { 0 } else { months_between(from, last) + 1 };

        let rows = db::projections::capacity_facts(&db.read, from, last).await?;
        let mut salary_by_account: HashMap<AccountId, Money> = HashMap::new();
        let facts: Vec<CapacityFact> = rows
            .iter()
            .map(|r| {
                let is_salary = [r.category_id, r.parent_id].into_iter().flatten().any(|c| categories.contains(&c));
                if is_salary {
                    *salary_by_account.entry(r.account_id).or_default() += r.amount;
                }
                CapacityFact {
                    amount: r.amount,
                    category_kind: r.category_kind,
                    is_movement: r.is_movement,
                    is_eur: r.currency == Currency::EUR.code(),
                    is_cash: r.account_kind.family() == AccountFamily::Cash,
                    is_interest: r.is_interest,
                    is_installment: r.is_installment,
                    is_salary,
                }
            })
            .collect();

        // Compte de versement : celui qui a reçu le plus de salaire, à défaut le premier compte courant (R-PRJ-45).
        let cash: Vec<&AccountRow> = accounts
            .iter()
            .filter(|a| {
                a.kind.family() == AccountFamily::Cash
                    && a.include_in_net_worth
                    && a.opening_date <= today
                    && a.closed_on.is_none_or(|c| c > today)
            })
            .collect();
        let payroll = cash
            .iter()
            .filter_map(|a| salary_by_account.get(&a.id).filter(|m| m.is_positive()).map(|m| (*m, a.id)))
            .max_by_key(|(m, _)| *m)
            .map(|(_, id)| id)
            .or_else(|| cash.iter().find(|a| a.kind == AccountKind::Checking).map(|a| a.id))
            .or_else(|| cash.first().map(|a| a.id));

        let loans = LoanBook::load(db).await?;
        let paid: HashSet<(LoanId, u32)> =
            db::loans::payments(&db.read).await?.into_iter().map(|p| (p.loan_id, p.installment_number)).collect();
        let mut installments = Vec::new();
        let mut unpaid_loans = Vec::new();
        for loan in loans.loans.iter().filter(|l| l.schedule.is_some()) {
            let schedule = loans.schedule(loan.id);
            installments.extend(
                schedule
                    .iter()
                    .filter(|i| i.date > today && loan.closed_on.is_none_or(|c| i.date < c))
                    .map(|i| (i.date, i.total())),
            );
            if schedule.iter().any(|i| i.date >= from && i.date <= today && !paid.contains(&(loan.id, i.number))) {
                unpaid_loans.push(loan.name.clone());
            }
        }

        Ok(Self {
            salaries,
            baseline: projection::baseline(&facts, months),
            months,
            installments,
            payroll,
            families: accounts.iter().map(|a| (a.id, a.kind.family())).collect(),
            names: accounts.iter().map(|a| (a.id, a.name.clone())).collect(),
            unpaid_loans,
        })
    }

    pub fn name(&self, id: AccountId) -> Option<String> {
        self.names.get(&id).cloned()
    }
}

/// Capacité d'épargne d'un scénario et son affectation (R-PRJ-42, R-PRJ-45).
pub(super) struct CapacityPlan {
    pub months: Vec<CapacityMonth>,
    pub payroll: Option<AccountId>,
    pub surplus: Option<AccountId>,
    /// Reste viré vers le compte d'affectation, chaque mois.
    pub moved: Vec<Money>,
    pub assumptions: CapacityAssumptions,
}

impl CapacityPlan {
    /// Ce que reçoit le compte de versement, cumulé en fin de chaque mois.
    pub fn payroll_cumulative(&self) -> Vec<Money> {
        let mut sum = Money::ZERO;
        self.months
            .iter()
            .zip(self.moved.iter().copied().chain(std::iter::repeat(Money::ZERO)))
            .map(|(m, moved)| {
                sum += m.total - moved;
                sum
            })
            .collect()
    }

    /// Versements de fin de mois vers le compte d'affectation.
    pub fn surplus_deposits(&self) -> Vec<Movement> {
        self.months
            .iter()
            .zip(&self.moved)
            .filter(|(_, moved)| moved.is_positive())
            .map(|(m, moved)| Movement { date: m.end, amount: *moved })
            .collect()
    }
}

/// Capacité d'épargne de `months` mois sous un scénario ; rien si le scénario la désactive (R-PRJ-47).
pub(super) async fn plan(
    db: &Db,
    ctx: &Context,
    scenario: &ScenarioView,
    months: u32,
    today: Date,
) -> AppResult<Option<CapacityPlan>> {
    let assumptions = scenario.assumptions.capacity.clone();
    if !assumptions.enabled {
        return Ok(None);
    }
    let base = &ctx.capacity;
    let salaries = base.salaries.projected(Rate::from_ppm(assumptions.salary_growth_ppm), today, months);
    let list = projection::capacity(
        today,
        months,
        &CapacityInput {
            salaries: &salaries,
            baseline: base.baseline,
            expense_growth: Rate::from_ppm(assumptions.expense_growth_ppm),
            installments: &base.installments,
        },
    );
    let ends: Vec<Date> = list.iter().map(|m| m.end).collect();
    let Some(horizon) = ends.last().copied() else {
        return Ok(None);
    };
    // Versements programmés nets des comptes de liquidités vers l'épargne, mois par mois (R-PRJ-45).
    let mut transfers = vec![Money::ZERO; list.len()];
    let start = today.checked_add_days(Days::new(1)).unwrap_or(today);
    let family = |id: AccountId| base.families.get(&id).copied();
    for rule in ctx.rules.iter().filter(|r| r.active) {
        let Some(target) = rule.target_account_id else { continue };
        let signed =
            match (family(rule.account_id) == Some(AccountFamily::Cash), family(target) == Some(AccountFamily::Cash)) {
                (true, false) => rule.amount,
                (false, true) => -rule.amount,
                _ => continue,
            };
        for date in recurring::schedule(rule).occurrences(start, horizon) {
            if let Some(i) = ends.iter().position(|end| date <= *end) {
                transfers[i] += signed;
            }
        }
    }
    let mut moved = vec![Money::ZERO; list.len()];
    let surplus = match assumptions.surplus_account_id {
        Some(id) => db::accounts::find(&db.read, id)
            .await?
            .filter(|r| r.kind.family() != AccountFamily::Cash && r.closed_on.is_none_or(|c| c > today)),
        None => None,
    };
    if let Some(row) = &surplus {
        let wanted: Vec<Movement> = list
            .iter()
            .zip(&transfers)
            .map(|(m, t)| Movement { date: m.end, amount: m.total + *t })
            .filter(|m| m.amount.is_positive())
            .collect();
        // Vers un compte plafonné, les versements programmés passent d'abord (R-PRJ-22).
        let accepted = if row.kind.family() == AccountFamily::FixedSavings {
            let own =
                scenario.assumptions.by_account.get(&row.id.to_string()).and_then(|a| a.monthly_contribution_minor);
            let parts = fixed_parts(db, ctx, row, scenario, own, today, &ends).await?;
            let scheduled: Money = projection::accepted(&parts.deposits, parts.cap, parts.capital, today, horizon)
                .iter()
                .map(|m| m.amount)
                .sum();
            projection::accepted(&wanted, parts.cap, parts.capital + scheduled, today, horizon)
        } else {
            wanted
        };
        for deposit in accepted {
            if let Some(i) = ends.iter().position(|end| *end == deposit.date) {
                moved[i] += deposit.amount;
            }
        }
    }
    Ok(Some(CapacityPlan { months: list, payroll: base.payroll, surplus: surplus.map(|r| r.id), moved, assumptions }))
}

// ---------------------------------------------------------------- Vues (R-PRJ-46)

#[derive(Debug, Serialize, ToSchema)]
pub struct CapacityPoint {
    pub date: Date,
    pub salaries_minor: i64,
    pub other_income_minor: i64,
    pub expenses_minor: i64,
    pub loans_minor: i64,
    pub total_minor: i64,
    /// Reste viré vers le compte d'affectation.
    pub moved_minor: i64,
}

/// Capacité d'épargne d'un scénario.
#[derive(Debug, Serialize, ToSchema)]
pub struct CapacitySeries {
    pub salary_growth_ppm: i64,
    pub expense_growth_ppm: i64,
    pub surplus_account_name: Option<String>,
    pub points: Vec<CapacityPoint>,
    /// Premier mois où le compte de versement passe sous zéro (R-PRJ-46).
    pub shortfall_date: Option<Date>,
}

/// Un emploi qui prend fin dans le futur.
#[derive(Debug, Serialize, ToSchema)]
pub struct JobEnd {
    pub employer: String,
    pub end_date: Date,
}

/// Données de départ de la capacité d'épargne, communes aux scénarios (R-PRJ-46).
#[derive(Debug, Serialize, ToSchema)]
pub struct CapacityBaseView {
    /// Mois complets moyennés.
    pub months: u32,
    pub other_income_minor: i64,
    pub expenses_minor: i64,
    pub payroll_account_id: Option<AccountId>,
    pub payroll_account_name: Option<String>,
    /// Prêts avec tableau dont des échéances récentes ne sont pas ventilées (R-PRJ-44).
    pub unpaid_loans: Vec<String>,
    pub job_ends: Vec<JobEnd>,
    /// Emplois en cours ou futurs sans rémunération déclarée (R-SAL-10).
    pub jobs_without_pay: Vec<String>,
}

pub(super) fn base_view(base: &CapacityBase, today: Date) -> CapacityBaseView {
    let jobs = &base.salaries.employments;
    CapacityBaseView {
        months: base.months,
        other_income_minor: base.baseline.other_income.minor(),
        expenses_minor: base.baseline.expenses.minor(),
        payroll_account_id: base.payroll,
        payroll_account_name: base.payroll.and_then(|id| base.name(id)),
        unpaid_loans: base.unpaid_loans.clone(),
        job_ends: jobs
            .iter()
            .filter_map(|e| {
                e.end_date.filter(|d| *d >= today).map(|end_date| JobEnd { employer: e.employer.clone(), end_date })
            })
            .collect(),
        jobs_without_pay: jobs
            .iter()
            .filter(|e| e.end_date.is_none_or(|d| d >= today) && base.salaries.salaries(e.id).is_empty())
            .map(|e| e.employer.clone())
            .collect(),
    }
}

pub(super) fn series(plan: &CapacityPlan, base: &CapacityBase, shortfall_date: Option<Date>) -> CapacitySeries {
    CapacitySeries {
        salary_growth_ppm: plan.assumptions.salary_growth_ppm,
        expense_growth_ppm: plan.assumptions.expense_growth_ppm,
        surplus_account_name: plan.surplus.and_then(|id| base.name(id)),
        points: plan
            .months
            .iter()
            .zip(plan.moved.iter().copied().chain(std::iter::repeat(Money::ZERO)))
            .map(|(m, moved)| CapacityPoint {
                date: m.end,
                salaries_minor: m.salaries.minor(),
                other_income_minor: m.other_income.minor(),
                expenses_minor: m.expenses.minor(),
                loans_minor: m.loans.minor(),
                total_minor: m.total.minor(),
                moved_minor: moved.minor(),
            })
            .collect(),
        shortfall_date,
    }
}

// ---------------------------------------------------------------- Projection du salaire (SAL-04)

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SalaryProjectionQuery {
    /// Années projetées après l'année en cours (10 par défaut).
    pub years: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct YearAmount {
    pub year: i32,
    pub net_minor: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SalaryScenarioSeries {
    pub scenario_id: ScenarioId,
    pub name: String,
    pub is_default: bool,
    pub salary_growth_ppm: i64,
    /// Année en cours (mois écoulés déclarés, mois à venir projetés) puis années suivantes.
    pub years: Vec<YearAmount>,
}

/// Une rémunération ou une prime prévue (R-SAL-4).
#[derive(Debug, Serialize, ToSchema)]
pub struct PlannedPay {
    pub employer: String,
    pub label: String,
    pub date: Date,
    pub net_minor: i64,
}

/// Net annuel réel puis projeté par scénario, avec ses hypothèses (R-SAL-13).
#[derive(Debug, Serialize, ToSchema)]
pub struct SalaryProjection {
    pub today: Date,
    /// Années passées, revenu annuel déclaré (R-SAL-7).
    pub history: Vec<YearAmount>,
    pub scenarios: Vec<SalaryScenarioSeries>,
    pub planned: Vec<PlannedPay>,
    pub job_ends: Vec<JobEnd>,
    pub jobs_without_pay: Vec<String>,
}

pub async fn salary(db: &Db, query: &SalaryProjectionQuery, today: Date) -> AppResult<SalaryProjection> {
    let years = query.years.unwrap_or(10).clamp(1, 50);
    let last_year = today.year() + i32::try_from(years).unwrap_or(10);
    let base = CapacityBase::load(db, today).await?;
    let overview = crate::services::salaries::overview(db, today).await?;
    let history = overview
        .years
        .iter()
        .filter(|y| y.year < today.year())
        .map(|y| YearAmount { year: y.year, net_minor: y.net_minor })
        .collect();
    let scenarios = super::scenarios(db)
        .await?
        .into_iter()
        .map(|s| {
            let growth = s.assumptions.capacity.salary_growth_ppm;
            SalaryScenarioSeries {
                scenario_id: s.id,
                name: s.name,
                is_default: s.is_default,
                salary_growth_ppm: growth,
                years: base
                    .salaries
                    .projected_years(Rate::from_ppm(growth), today, last_year)
                    .into_iter()
                    .map(|(year, net)| YearAmount { year, net_minor: net.minor() })
                    .collect(),
            }
        })
        .collect();
    let current = reporting::first_of_month(today);
    let mut planned = Vec::new();
    for e in &base.salaries.employments {
        planned.extend(base.salaries.salaries(e.id).iter().filter(|s| s.effective_month > current).map(|s| {
            PlannedPay {
                employer: e.employer.clone(),
                label: "Rémunération".into(),
                date: s.effective_month,
                net_minor: s.monthly_net.minor(),
            }
        }));
        planned.extend(base.salaries.bonuses(e.id).iter().filter(|b| b.paid_on > today).map(|b| PlannedPay {
            employer: e.employer.clone(),
            label: b.label.clone(),
            date: b.paid_on,
            net_minor: b.net_amount.minor(),
        }));
    }
    planned.sort_by_key(|p| p.date);
    let view = base_view(&base, today);
    Ok(SalaryProjection {
        today,
        history,
        scenarios,
        planned,
        job_ends: view.job_ends,
        jobs_without_pay: view.jobs_without_pay,
    })
}
