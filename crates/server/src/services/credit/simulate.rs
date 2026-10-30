//! Onglet Simuler (CRE-07, CRE-08, CRE-11, CRE-12) : la situation d'emprunteur, les conditions par défaut de chaque
//! type de projet, et les trois questions : que puis-je acheter, puis-je financer cet achat, que coûte ce prêt.
//! Les calculs sont dans `cashflow_domain::credit`.

use std::collections::HashSet;

use cashflow_domain::credit::{self, Conditions, FinancingPlan, ProjectKind, PurchaseFinancing};
use cashflow_domain::loans::{self, Insurance, LoanKind};
use cashflow_domain::shared::ids::{AccountId, LoanId, LoanSimulationId};
use cashflow_domain::{Date, Money, Rate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    ApplicationInput, ApplicationView, ChargeLine, ComputeInput, ComputeResult, SimulationGoal, SimulationInput,
};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::services::projections;

/// Réglages de l'onglet (R-CRE-30, R-CRE-38, R-CRE-39).
const SITUATION_SETTING: &str = "credit.borrower_situation";
const DEFAULTS_SETTING: &str = "credit.project_defaults";
const STATE_SETTING: &str = "credit.simulate_state";

// ======================================================================= Conditions (R-CRE-38, R-CRE-39)

/// Conditions d'un crédit, sans le capital (R-CRE-38).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreditConditions {
    pub kind: LoanKind,
    pub annual_rate_ppm: i64,
    pub duration_months: u32,
    pub insurance: Insurance,
    pub fees_minor: i64,
    pub guarantee_fees_minor: i64,
}

impl From<Conditions> for CreditConditions {
    fn from(c: Conditions) -> Self {
        Self {
            kind: c.kind,
            annual_rate_ppm: c.annual_rate.ppm(),
            duration_months: c.duration_months,
            insurance: c.insurance,
            fees_minor: c.fees.minor(),
            guarantee_fees_minor: c.guarantee_fees.minor(),
        }
    }
}

impl CreditConditions {
    fn domain(&self) -> AppResult<Conditions> {
        let conditions = Conditions {
            kind: self.kind,
            annual_rate: Rate::from_ppm(self.annual_rate_ppm),
            duration_months: self.duration_months,
            insurance: self.insurance,
            fees: Money::from_minor(self.fees_minor),
            guarantee_fees: Money::from_minor(self.guarantee_fees_minor),
        };
        conditions.validate()?;
        Ok(conditions)
    }
}

/// Conditions par défaut d'un type de projet (R-CRE-38).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectDefaultsInput {
    pub project_kind: ProjectKind,
    pub conditions: CreditConditions,
}

/// Conditions par défaut d'un type de projet et ses valeurs initiales (E42).
#[derive(Debug, Serialize, ToSchema)]
pub struct ProjectDefaults {
    pub project_kind: ProjectKind,
    pub conditions: CreditConditions,
    pub initial: CreditConditions,
}

/// Conditions par défaut de chaque type de projet ; un type absent du réglage prend ses valeurs initiales.
pub async fn project_defaults(db: &Db) -> AppResult<Vec<ProjectDefaults>> {
    let saved: Vec<ProjectDefaultsInput> = db::settings::get(&db.read, DEFAULTS_SETTING).await?.unwrap_or_default();
    Ok(ProjectKind::ALL
        .into_iter()
        .map(|kind| {
            let initial = CreditConditions::from(kind.initial_conditions());
            let conditions = saved.iter().find(|s| s.project_kind == kind).map_or(initial, |s| s.conditions);
            ProjectDefaults { project_kind: kind, conditions, initial }
        })
        .collect())
}

async fn defaults_of(db: &Db, kind: ProjectKind) -> AppResult<CreditConditions> {
    Ok(project_defaults(db)
        .await?
        .into_iter()
        .find(|d| d.project_kind == kind)
        .map_or_else(|| CreditConditions::from(kind.initial_conditions()), |d| d.conditions))
}

/// Garde les conditions par défaut (R-CRE-38).
pub async fn save_project_defaults(db: &Db, input: &[ProjectDefaultsInput]) -> AppResult<Vec<ProjectDefaults>> {
    for entry in input {
        entry.conditions.domain()?;
    }
    db::settings::set(&db.write, DEFAULTS_SETTING, &input).await?;
    project_defaults(db).await
}

// ======================================================================= Dernier état de l'onglet (R-CRE-39)

/// Question posée dans l'onglet Simuler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SimulateQuestion {
    /// Que puis-je acheter ? (R-CRE-31)
    MaxPrice,
    /// Puis-je financer cet achat ? (R-CRE-40)
    FinancePurchase,
    /// Que coûte ce prêt ? (R-CRE-41)
    Loan,
}

/// Dernier état de l'onglet : projet, question, conditions du calcul et montants saisis (R-CRE-39).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SimulateState {
    pub project_kind: ProjectKind,
    pub question: SimulateQuestion,
    pub conditions: CreditConditions,
    pub price_minor: Option<i64>,
    /// Apport saisi ; absent, l'apport proposé.
    pub down_payment_minor: Option<i64>,
    /// Frais de notaire saisis ; absents, estimés.
    pub notary_fees_minor: Option<i64>,
    /// « Que coûte ce prêt ? » : ce que cherche le simulateur.
    pub goal: SimulationGoal,
    pub principal_minor: Option<i64>,
    /// Mensualité visée : fixée pour un achat, ou saisie dans le simulateur.
    pub target_payment_minor: Option<i64>,
    /// « Puis-je financer cet achat ? » : la mensualité est fixée, on cherche la durée.
    pub fix_payment: bool,
}

pub async fn simulate_state(db: &Db) -> AppResult<SimulateState> {
    if let Some(state) = db::settings::get::<SimulateState>(&db.read, STATE_SETTING).await? {
        return Ok(state);
    }
    let project_kind = ProjectKind::ExistingHome;
    Ok(SimulateState {
        project_kind,
        question: SimulateQuestion::MaxPrice,
        conditions: defaults_of(db, project_kind).await?,
        price_minor: None,
        down_payment_minor: None,
        notary_fees_minor: None,
        goal: SimulationGoal::Payment,
        principal_minor: None,
        target_payment_minor: None,
        fix_payment: false,
    })
}

pub async fn save_simulate_state(db: &Db, input: &SimulateState) -> AppResult<SimulateState> {
    input.conditions.domain()?;
    let amounts = [input.price_minor, input.down_payment_minor, input.notary_fees_minor, input.principal_minor];
    if amounts.iter().flatten().any(|m| *m < 0) || input.target_payment_minor.is_some_and(|m| m < 0) {
        return Err(AppError::invalid("INVALID_AMOUNT", "montant négatif"));
    }
    db::settings::set(&db.write, STATE_SETTING, input).await?;
    Ok(input.clone())
}

// ======================================================================= Situation d'emprunteur (R-CRE-30)

/// Situation d'emprunteur, commune aux trois questions et à tous les projets (R-CRE-30).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BorrowerSituation {
    /// Taux d'endettement visé.
    pub max_debt_ratio_ppm: i64,
    /// Réserve de sécurité, en mois de dépenses moyennes.
    pub reserve_months: u32,
    /// Comptes mobilisables ; absent : ceux de l'épargne de précaution.
    pub account_ids: Option<Vec<AccountId>>,
    pub other_income_minor: i64,
    pub other_charges_minor: i64,
    pub co_borrower_income_minor: i64,
    pub co_borrower_charges_minor: i64,
    pub excluded_loan_ids: Vec<LoanId>,
}

impl Default for BorrowerSituation {
    fn default() -> Self {
        Self {
            max_debt_ratio_ppm: credit::MAX_DEBT_RATIO.ppm(),
            reserve_months: 6,
            account_ids: None,
            other_income_minor: 0,
            other_charges_minor: 0,
            co_borrower_income_minor: 0,
            co_borrower_charges_minor: 0,
            excluded_loan_ids: vec![],
        }
    }
}

/// Un compte proposé pour l'apport (R-CRE-30).
#[derive(Debug, Serialize, ToSchema)]
pub struct SavingsAccount {
    pub id: AccountId,
    pub name: String,
    pub balance_minor: i64,
    pub selected: bool,
}

/// La situation d'emprunteur et son résumé (E36).
#[derive(Debug, Serialize, ToSchema)]
pub struct SituationView {
    pub situation: BorrowerSituation,
    pub accounts: Vec<SavingsAccount>,
    pub loans: Vec<ChargeLine>,
    pub income_minor: i64,
    pub charges_minor: i64,
    /// Dépenses mensuelles moyennes (R-TDB-8).
    pub expenses_minor: i64,
    /// Épargne mobilisable.
    pub savings_minor: i64,
    pub reserve_minor: i64,
    pub possible_down_payment_minor: i64,
}

/// Ce que les calculs lisent de la situation : le jour même et le scénario par défaut.
struct Situation {
    view: SituationView,
    income: Money,
    charges: Money,
    savings: Money,
    reserve: Money,
    expenses: Money,
    excluded_payments: Money,
    projection: projections::NetWorthProjection,
}

impl Situation {
    fn max_debt_ratio(&self) -> Rate {
        Rate::from_ppm(self.view.situation.max_debt_ratio_ppm)
    }

    fn extra_income(&self) -> Money {
        let s = &self.view.situation;
        Money::from_minor(s.other_income_minor + s.co_borrower_income_minor)
    }

    fn extra_charges(&self) -> Money {
        let s = &self.view.situation;
        Money::from_minor(s.other_charges_minor + s.co_borrower_charges_minor)
    }

    fn possible_down_payment(&self) -> Money {
        Money::from_minor(self.view.possible_down_payment_minor)
    }

    fn scenario(&self) -> Option<&projections::ScenarioSeries> {
        self.projection.scenarios.iter().find(|s| s.is_default)
    }

    /// Capacité d'épargne du mois prochain, scénario par défaut (R-CRE-32).
    fn savings_capacity(&self) -> Option<i64> {
        self.scenario().and_then(|s| s.capacity.as_ref()).and_then(|c| c.points.first()).map(|p| p.total_minor)
    }
}

/// Charge la situation ; `months` est l'horizon du scénario par défaut lu.
async fn situation(db: &Db, today: Date, months: u32) -> AppResult<Situation> {
    let s: BorrowerSituation = db::settings::get(&db.read, SITUATION_SETTING).await?.unwrap_or_default();
    let sources = super::context(db, today).await?;
    let excluded: HashSet<LoanId> = s.excluded_loan_ids.iter().copied().collect();

    let accounts: Vec<SavingsAccount> = crate::services::accounts::list(db, today)
        .await?
        .into_iter()
        .filter(|a| a.closed_on.is_none_or(|c| c > today))
        .map(|a| SavingsAccount {
            selected: s.account_ids.as_ref().map_or(a.include_in_emergency_fund, |ids| ids.contains(&a.id)),
            id: a.id,
            name: a.name,
            balance_minor: a.balance_minor,
        })
        .collect();
    let savings: Money = accounts.iter().filter(|a| a.selected).map(|a| Money::from_minor(a.balance_minor)).sum();

    let projection = projections::net_worth(db, &projections::NetWorthQuery { months: Some(months) }, today).await?;
    let expenses = Money::from_minor(projection.capacity.expenses_minor);

    // Revenus et charges comme une demande (R-CRE-13, R-CRE-14).
    let extra_income = Money::from_minor(s.other_income_minor + s.co_borrower_income_minor);
    let extra_charges = Money::from_minor(s.other_charges_minor + s.co_borrower_charges_minor);
    let income = sources.employments.iter().map(|e| Money::from_minor(e.monthly_minor)).sum::<Money>() + extra_income;
    let loans: Vec<ChargeLine> = sources
        .loans
        .iter()
        .map(|(id, name, monthly)| ChargeLine {
            loan_id: *id,
            name: name.clone(),
            monthly_minor: monthly.map(Money::minor),
            excluded: excluded.contains(id),
        })
        .collect();
    let monthly = |excluded: bool| -> Money {
        loans.iter().filter(|l| l.excluded == excluded).filter_map(|l| l.monthly_minor).map(Money::from_minor).sum()
    };
    let charges = monthly(false) + extra_charges;
    let excluded_payments = monthly(true);
    let reserve = Money::from_minor(expenses.minor() * i64::from(s.reserve_months));
    let possible = if savings > reserve { savings - reserve } else { Money::ZERO };

    Ok(Situation {
        view: SituationView {
            situation: s,
            accounts,
            loans,
            income_minor: income.minor(),
            charges_minor: charges.minor(),
            expenses_minor: expenses.minor(),
            savings_minor: savings.minor(),
            reserve_minor: reserve.minor(),
            possible_down_payment_minor: possible.minor(),
        },
        income,
        charges,
        savings,
        reserve,
        expenses,
        excluded_payments,
        projection,
    })
}

pub async fn borrower_situation(db: &Db, today: Date) -> AppResult<SituationView> {
    Ok(situation(db, today, 1).await?.view)
}

/// Garde la situation d'emprunteur (R-CRE-30) et renvoie son résumé.
pub async fn save_borrower_situation(db: &Db, input: &BorrowerSituation, today: Date) -> AppResult<SituationView> {
    let amounts = [
        input.other_income_minor,
        input.other_charges_minor,
        input.co_borrower_income_minor,
        input.co_borrower_charges_minor,
    ];
    if amounts.iter().any(|m| *m < 0)
        || !(1..=1_000_000).contains(&input.max_debt_ratio_ppm)
        || input.reserve_months > 120
    {
        return Err(AppError::invalid("INVALID_SITUATION", "situation d'emprunteur invalide"));
    }
    db::settings::set(&db.write, SITUATION_SETTING, input).await?;
    borrower_situation(db, today).await
}

// ======================================================================= Endettement et effet de l'achat

/// Endettement d'une échéance face à la situation (R-CRE-15, R-CRE-32, R-CRE-41).
#[derive(Debug, Serialize, ToSchema)]
pub struct AffordabilityView {
    /// Échéance totale envisagée : mensualité + assurance.
    pub payment_minor: i64,
    pub income_minor: i64,
    pub charges_minor: i64,
    /// Absent sans revenus.
    pub debt_ratio_ppm: Option<i64>,
    pub max_debt_ratio_ppm: i64,
    /// Au-delà du taux d'endettement visé.
    pub debt_ratio_exceeded: bool,
    /// Au-delà de la durée maximale du HCSF, pour un logement (R-CRE-15).
    pub duration_exceeded: bool,
    pub remaining_income_minor: i64,
    /// Capacité d'épargne mensuelle avant et après (R-CRE-32) ; absente sans capacité dans le scénario.
    pub savings_capacity_before_minor: Option<i64>,
    pub savings_capacity_after_minor: Option<i64>,
}

fn affordability(s: &Situation, project: ProjectKind, payment: Money, duration_months: u32) -> AffordabilityView {
    let a = credit::affordability(s.income, s.charges, payment, s.max_debt_ratio());
    let before = s.savings_capacity();
    AffordabilityView {
        payment_minor: payment.minor(),
        income_minor: s.income.minor(),
        charges_minor: s.charges.minor(),
        debt_ratio_ppm: a.debt_ratio.map(Rate::ppm),
        max_debt_ratio_ppm: s.max_debt_ratio().ppm(),
        debt_ratio_exceeded: a.debt_ratio_exceeded,
        duration_exceeded: project.is_home_purchase() && duration_months > project.max_duration(),
        remaining_income_minor: a.remaining_income.minor(),
        savings_capacity_before_minor: before,
        savings_capacity_after_minor: before.map(|b| b - payment.minor() + s.excluded_payments.minor()),
    }
}

/// Épargne restante après l'apport, et en mois de dépenses (dixièmes) (R-CRE-32).
fn savings_after(s: &Situation, down_payment: Money) -> (i64, Option<i64>) {
    let after = (s.savings - down_payment).minor();
    (after, s.expenses.is_positive().then(|| after * 10 / s.expenses.minor()))
}

// ======================================================================= Que puis-je acheter ? (R-CRE-31 à R-CRE-35)

/// Le projet et les conditions d'un calcul.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CapacityRequest {
    pub project_kind: ProjectKind,
    pub conditions: CreditConditions,
}

/// Capacité d'achat à une date (R-CRE-31, R-CRE-33).
#[derive(Debug, Serialize, ToSchema)]
pub struct PurchaseView {
    pub date: Date,
    pub income_minor: i64,
    pub charges_minor: i64,
    pub max_payment_minor: i64,
    pub capital_minor: i64,
    pub savings_minor: i64,
    pub reserve_minor: i64,
    pub down_payment_minor: i64,
    pub budget_minor: i64,
    pub max_price_minor: i64,
    pub notary_fees_minor: i64,
    /// Échéance totale du crédit au capital empruntable.
    pub payment_minor: i64,
}

/// Prix maximal pour une durée et un taux (R-CRE-34).
#[derive(Debug, Serialize, ToSchema)]
pub struct SensitivityCell {
    pub duration_months: u32,
    pub annual_rate_ppm: i64,
    pub max_price_minor: i64,
}

/// Patrimoine projeté sans et avec l'achat (R-CRE-35).
#[derive(Debug, Serialize, ToSchema)]
pub struct NetWorthComparison {
    pub date: Date,
    pub without_minor: i64,
    pub with_minor: i64,
}

/// Que puis-je acheter ? (E36)
#[derive(Debug, Serialize, ToSchema)]
pub struct PurchaseCapacity {
    pub today: PurchaseView,
    pub savings_after_minor: i64,
    pub savings_after_tenths_of_months: Option<i64>,
    pub affordability: AffordabilityView,
    /// Les 5 prochaines années (R-CRE-33).
    pub years: Vec<PurchaseView>,
    pub sensitivity: Vec<SensitivityCell>,
    /// Pour un logement ou des travaux seulement (R-CRE-35).
    pub net_worth: Vec<NetWorthComparison>,
}

/// Échéance totale d'un prêt avec tableau dans le mois de `date` ; zéro sinon.
fn installment_in_month(schedule: &[loans::Installment], date: Date) -> Money {
    let month = cashflow_domain::salaries::month_of(date);
    schedule
        .iter()
        .find(|i| cashflow_domain::salaries::month_of(i.date) == month)
        .map_or(Money::ZERO, loans::Installment::total)
}

fn purchase_view(date: Date, input: &credit::PurchaseInput, p: &credit::Purchase) -> PurchaseView {
    PurchaseView {
        date,
        income_minor: input.income.minor(),
        charges_minor: input.charges.minor(),
        max_payment_minor: p.max_payment.minor(),
        capital_minor: p.capital.minor(),
        savings_minor: input.savings.minor(),
        reserve_minor: input.reserve.minor(),
        down_payment_minor: p.down_payment.minor(),
        budget_minor: p.budget.minor(),
        max_price_minor: p.max_price.minor(),
        notary_fees_minor: p.notary_fees.minor(),
        payment_minor: p.payment.minor(),
    }
}

/// Que puis-je acheter ? Aujourd'hui, dans les années à venir, sensibilité, patrimoine projeté (R-CRE-31 à
/// R-CRE-35).
pub async fn purchase_capacity(db: &Db, request: &CapacityRequest, today: Date) -> AppResult<PurchaseCapacity> {
    let conditions = request.conditions.domain()?;
    let project = request.project_kind;
    let s = situation(db, today, 12 * 10 + 12).await?;
    let excluded: HashSet<LoanId> = s.view.situation.excluded_loan_ids.iter().copied().collect();
    let months = i64::from(s.view.situation.reserve_months);

    // Prêts en cours, avec leur tableau, pour les charges des années à venir.
    let mut schedules = Vec::new();
    for loan in db::loans::list(&db.read).await? {
        if let Some(schedule) = loan.schedule.as_ref().filter(|_| !excluded.contains(&loan.id)) {
            schedules.push(loans::schedule(&schedule.terms)?);
        }
    }
    let loans_at = |date: Date| -> Money { schedules.iter().map(|s| installment_in_month(s, date)).sum() };

    let input_at = |income: Money, charges: Money, savings: Money, reserve: Money, duration: u32, rate: Rate| {
        credit::PurchaseInput {
            income,
            charges,
            max_debt_ratio: s.max_debt_ratio(),
            annual_rate: rate,
            duration_months: duration,
            insurance: conditions.insurance,
            fees: conditions.fees,
            guarantee_fees: conditions.guarantee_fees,
            project,
            savings,
            reserve,
        }
    };

    // Aujourd'hui (R-CRE-31) et l'effet de l'achat (R-CRE-32).
    let rate = conditions.annual_rate;
    let now_input = input_at(s.income, s.charges, s.savings, s.reserve, conditions.duration_months, rate);
    let now = credit::purchase(&now_input);
    let (savings_after_minor, savings_after_tenths_of_months) = savings_after(&s, now.down_payment);

    // Dans les années à venir (R-CRE-33).
    let scenario = s.scenario();
    let mut years = vec![purchase_view(today, &now_input, &now)];
    if let Some(points) = scenario.and_then(|sc| sc.capacity.as_ref()).map(|c| &c.points) {
        for year in 1..=5_u32 {
            let start = 12 * year as usize;
            let Some(point) = points.get(start) else { break };
            let next: Vec<i64> = points.iter().skip(start).take(12).map(|p| p.salaries_minor).collect();
            let salaries = next.iter().sum::<i64>() / i64::try_from(next.len().max(1)).unwrap_or(1);
            let cumulative: i64 = points.iter().take(start).map(|p| p.total_minor).sum();
            let date = today.checked_add_months(chrono::Months::new(12 * year)).unwrap_or(today);
            let input = input_at(
                Money::from_minor(salaries) + s.extra_income(),
                loans_at(date) + s.extra_charges(),
                s.savings + Money::from_minor(cumulative),
                Money::from_minor(point.expenses_minor * months),
                conditions.duration_months,
                rate,
            );
            years.push(purchase_view(date, &input, &credit::purchase(&input)));
        }
    }

    // Sensibilité (R-CRE-34).
    let mut sensitivity = Vec::new();
    for duration in project.sensitivity_durations(conditions.duration_months) {
        for delta in [-5_000, 0, 5_000] {
            let rate_ppm = (rate.ppm() + delta).max(0);
            let input = input_at(s.income, s.charges, s.savings, s.reserve, duration, Rate::from_ppm(rate_ppm));
            sensitivity.push(SensitivityCell {
                duration_months: duration,
                annual_rate_ppm: rate_ppm,
                max_price_minor: credit::purchase(&input).max_price.minor(),
            });
        }
    }

    // Patrimoine projeté avec l'achat, sur 10 ans, pour un bien qui garde sa valeur (R-CRE-35).
    let mut net_worth = Vec::new();
    if let Some(scenario) = scenario
        && project.projects_net_worth()
        && now.capital.is_positive()
    {
        let terms = conditions.with_principal(now.capital);
        let simulation = credit::simulate(&terms, today)?;
        let baseline: Vec<(Date, Money)> =
            scenario.points.iter().take(120).map(|p| (p.date, Money::from_minor(p.value_minor))).collect();
        let one_off = now.notary_fees + terms.fees + terms.guarantee_fees;
        let with = credit::net_worth_with_purchase(&baseline, one_off, &simulation.schedule);
        net_worth = baseline
            .iter()
            .zip(with)
            .map(|((date, without), (_, with))| NetWorthComparison {
                date: *date,
                without_minor: without.minor(),
                with_minor: with.minor(),
            })
            .collect();
    }

    Ok(PurchaseCapacity {
        affordability: affordability(&s, project, now.payment, conditions.duration_months),
        savings_after_minor,
        savings_after_tenths_of_months,
        today: purchase_view(today, &now_input, &now),
        years,
        sensitivity,
        net_worth,
    })
}

// ======================================================================= Puis-je financer cet achat ? (R-CRE-40)

/// Un achat au prix connu (R-CRE-40).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct FinancingRequest {
    pub project_kind: ProjectKind,
    pub conditions: CreditConditions,
    pub price_minor: i64,
    /// Absent : l'apport proposé, le plus petit de l'apport possible et du coût du projet.
    pub down_payment_minor: Option<i64>,
    /// Absents : estimés (R-CRE-12).
    pub notary_fees_minor: Option<i64>,
    /// Présente : la mensualité est fixée, on cherche la durée (R-CRE-5).
    pub target_payment_minor: Option<i64>,
}

/// Le financement d'un achat au prix connu (E36).
#[derive(Debug, Serialize, ToSchema)]
pub struct FinancingView {
    pub price_minor: i64,
    pub notary_fees_minor: i64,
    pub estimated_notary_fees_minor: i64,
    pub other_costs_minor: i64,
    pub project_cost_minor: i64,
    pub down_payment_minor: i64,
    pub possible_down_payment_minor: i64,
    /// L'apport entame la réserve de sécurité.
    pub down_payment_exceeds_possible: bool,
    pub to_borrow_minor: i64,
    /// Le crédit ; absent pour un achat comptant.
    pub loan: Option<ComputeResult>,
    pub savings_after_minor: i64,
    pub savings_after_tenths_of_months: Option<i64>,
    pub affordability: AffordabilityView,
}

struct Financing {
    plan: FinancingPlan,
    conditions: Conditions,
    view: FinancingView,
}

async fn financing_of(db: &Db, request: &FinancingRequest, today: Date) -> AppResult<Financing> {
    let conditions = request.conditions.domain()?;
    let project = request.project_kind;
    let s = situation(db, today, 1).await?;
    let price = Money::from_minor(request.price_minor);
    let notary_fees = request.notary_fees_minor.map(Money::from_minor);
    let mut purchase = PurchaseFinancing {
        project,
        price,
        notary_fees,
        fees: conditions.fees,
        guarantee_fees: conditions.guarantee_fees,
        down_payment: Money::ZERO,
    };
    purchase.down_payment = match request.down_payment_minor {
        Some(minor) => Money::from_minor(minor),
        None => credit::proposed_down_payment(s.possible_down_payment(), purchase.plan()?.project_cost()),
    };
    let plan = purchase.plan()?;
    let to_borrow = plan.to_borrow();
    let loan = if to_borrow.is_positive() {
        let input = ComputeInput {
            goal: if request.target_payment_minor.is_some() {
                SimulationGoal::Duration
            } else {
                SimulationGoal::Payment
            },
            kind: conditions.kind,
            principal_minor: Some(to_borrow.minor()),
            duration_months: Some(conditions.duration_months),
            target_payment_minor: request.target_payment_minor,
            annual_rate_ppm: conditions.annual_rate.ppm(),
            insurance: conditions.insurance,
            fees_minor: conditions.fees.minor(),
            guarantee_fees_minor: conditions.guarantee_fees.minor(),
        };
        Some(super::compute(&input, today)?)
    } else {
        None
    };
    let payment = loan.as_ref().map_or(Money::ZERO, |l| Money::from_minor(l.result.total_payment_minor));
    let duration = loan.as_ref().map_or(0, |l| l.duration_months);
    let (savings_after_minor, savings_after_tenths_of_months) = savings_after(&s, plan.down_payment);
    let view = FinancingView {
        price_minor: plan.price.minor(),
        notary_fees_minor: plan.notary_fees.minor(),
        estimated_notary_fees_minor: credit::estimated_notary_fees(project, price).minor(),
        other_costs_minor: plan.other_costs.minor(),
        project_cost_minor: plan.project_cost().minor(),
        down_payment_minor: plan.down_payment.minor(),
        possible_down_payment_minor: s.possible_down_payment().minor(),
        down_payment_exceeds_possible: plan.down_payment > s.possible_down_payment(),
        to_borrow_minor: to_borrow.minor(),
        loan,
        savings_after_minor,
        savings_after_tenths_of_months,
        affordability: affordability(&s, project, payment, duration),
    };
    Ok(Financing { plan, conditions, view })
}

/// Puis-je financer cet achat ? (R-CRE-40)
pub async fn financing(db: &Db, request: &FinancingRequest, today: Date) -> AppResult<FinancingView> {
    Ok(financing_of(db, request, today).await?.view)
}

// ======================================================================= Que coûte ce prêt ? (R-CRE-41)

/// Un prêt au montant connu, et le projet qu'il finance.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoanCostRequest {
    pub project_kind: ProjectKind,
    pub loan: ComputeInput,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoanCostView {
    pub loan: ComputeResult,
    pub affordability: AffordabilityView,
}

/// Que coûte ce prêt ? Le simulateur, et l'endettement que donnerait le prêt (R-CRE-41).
pub async fn loan_cost(db: &Db, request: &LoanCostRequest, today: Date) -> AppResult<LoanCostView> {
    let loan = super::compute(&request.loan, today)?;
    let s = situation(db, today, 1).await?;
    let payment = Money::from_minor(loan.result.total_payment_minor);
    let affordability = affordability(&s, request.project_kind, payment, loan.duration_months);
    Ok(LoanCostView { loan, affordability })
}

// ======================================================================= Préparer une demande (R-CRE-36)

/// Les revenus, charges, co-emprunteur et prêts exclus de la situation, dans une demande.
fn application_input(
    name: &str,
    project: ProjectKind,
    plan: &FinancingPlan,
    s: &BorrowerSituation,
) -> ApplicationInput {
    ApplicationInput {
        name: name.to_owned(),
        project_kind: project,
        asset_id: None,
        price_minor: plan.price.minor(),
        notary_fees_minor: Some(plan.notary_fees.minor()),
        works_minor: 0,
        other_costs_minor: plan.other_costs.minor(),
        down_payment_minor: plan.down_payment.minor(),
        other_income_minor: s.other_income_minor,
        other_charges_minor: s.other_charges_minor,
        co_borrower_name: (s.co_borrower_income_minor > 0 || s.co_borrower_charges_minor > 0)
            .then(|| "Co-emprunteur".to_owned()),
        co_borrower_income_minor: s.co_borrower_income_minor,
        co_borrower_charges_minor: s.co_borrower_charges_minor,
        excluded_loan_ids: s.excluded_loan_ids.clone(),
        selected_simulation_id: None,
        status: None,
        note: None,
    }
}

/// Crée la demande, y rattache la simulation et la retient.
async fn create_with_simulation(
    db: &Db,
    mut input: ApplicationInput,
    simulation: LoanSimulationId,
    today: Date,
) -> AppResult<ApplicationView> {
    let mut sim = super::simulation_row(db, simulation).await?;
    let application = super::save_application(db, None, &input, today).await?;
    sim.application_id = Some(application.id);
    db::credit::upsert_simulation(&db.write, &sim).await?;
    input.selected_simulation_id = Some(sim.id);
    super::save_application(db, Some(application.id), &input, today).await
}

async fn save_terms(db: &Db, name: &str, terms: &credit::SimulationTerms, today: Date) -> AppResult<LoanSimulationId> {
    let input = SimulationInput {
        name: name.to_owned(),
        kind: terms.kind,
        principal_minor: terms.principal.minor(),
        annual_rate_ppm: terms.annual_rate.ppm(),
        duration_months: terms.duration_months,
        insurance: terms.insurance,
        fees_minor: terms.fees.minor(),
        guarantee_fees_minor: terms.guarantee_fees.minor(),
        institution_id: None,
        application_id: None,
        note: None,
    };
    Ok(super::save_simulation(db, None, &input, today).await?.id)
}

/// Prépare une demande depuis une simulation enregistrée (R-CRE-36) : le plus grand prix qui tient dans son
/// capital, l'apport possible et ses frais, avec l'apport nécessaire, et la simulation retenue.
pub async fn application_from_simulation(
    db: &Db,
    simulation: LoanSimulationId,
    project: ProjectKind,
    name: &str,
    today: Date,
) -> AppResult<ApplicationView> {
    let sim = super::simulation_row(db, simulation).await?;
    let s = situation(db, today, 1).await?;
    let possible = s.possible_down_payment();
    let costs = sim.terms.fees + sim.terms.guarantee_fees;
    let funds = sim.terms.principal + possible;
    let budget = if funds > costs { funds - costs } else { Money::ZERO };
    let (price, notary_fees) = credit::max_price(project, budget);
    // L'apport nécessaire, au plus l'apport possible : le prix arrondi laisse parfois un reliquat d'épargne.
    let cost = price + notary_fees + costs;
    let needed = if cost > sim.terms.principal { cost - sim.terms.principal } else { Money::ZERO };
    let plan = FinancingPlan {
        price,
        notary_fees,
        works: Money::ZERO,
        other_costs: costs,
        down_payment: needed.min(possible),
    };
    create_with_simulation(db, application_input(name, project, &plan, &s.view.situation), simulation, today).await
}

/// Crée une demande depuis « Que puis-je acheter ? » (R-CRE-36) : une simulation au capital empruntable, aux
/// conditions du calcul, puis la demande préparée depuis cette simulation.
pub async fn application_from_capacity(
    db: &Db,
    request: &CapacityRequest,
    name: &str,
    today: Date,
) -> AppResult<ApplicationView> {
    let capacity = purchase_capacity(db, request, today).await?;
    if capacity.today.capital_minor <= 0 {
        return Err(AppError::invalid("NO_BORROWING_CAPACITY", "aucun capital empruntable dans cette situation"));
    }
    let terms = request.conditions.domain()?.with_principal(Money::from_minor(capacity.today.capital_minor));
    let simulation = save_terms(db, "Capacité d'achat", &terms, today).await?;
    application_from_simulation(db, simulation, request.project_kind, name, today).await
}

/// Crée une demande depuis « Puis-je financer cet achat ? » (R-CRE-36) : le plan reprend le prix, les frais et
/// l'apport saisis ; la simulation enregistrée porte le capital à emprunter.
pub async fn application_from_financing(
    db: &Db,
    request: &FinancingRequest,
    name: &str,
    today: Date,
) -> AppResult<ApplicationView> {
    let f = financing_of(db, request, today).await?;
    let Some(loan) = &f.view.loan else {
        return Err(AppError::invalid("NOTHING_TO_BORROW", "l'apport couvre l'achat : rien à emprunter"));
    };
    let terms = credit::SimulationTerms {
        duration_months: loan.duration_months,
        ..f.conditions.with_principal(f.plan.to_borrow())
    };
    let simulation = save_terms(db, name, &terms, today).await?;
    let s: BorrowerSituation = db::settings::get(&db.read, SITUATION_SETTING).await?.unwrap_or_default();
    create_with_simulation(db, application_input(name, request.project_kind, &f.plan, &s), simulation, today).await
}
