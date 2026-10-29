//! Simulations et demandes de crédit (spec 13, ADR-0027) : coût et TAEG d'une simulation, calculs inverses, plan
//! de financement, capacité d'emprunt, statuts des démarches. Les pièces à fournir sont dans `pieces`.
//!
//! Montants en unités mineures, calculs intermédiaires décimaux (ADR-0001) ; le TAEG et le capital empruntable se
//! trouvent par dichotomie, sans flottant.

pub mod pieces;

use chrono::Datelike;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::loans::{self, Installment, Insurance, LoanKind, LoanTerms, MAX_DURATION_MONTHS};
use crate::salaries::{self, Bonus, Job, Pay};
use crate::shared::{Date, DomainError, Money, Rate};

/// Taux d'endettement maximal selon le HCSF, assurance comprise (R-CRE-15) : 35 %.
pub const MAX_DEBT_RATIO: Rate = Rate::from_ppm(350_000);
/// Durée maximale selon le HCSF : 25 ans, 27 ans pour un achat dans le neuf (R-CRE-15).
pub const MAX_DURATION: u32 = 300;
pub const MAX_DURATION_NEW_HOME: u32 = 324;

// ======================================================================= Simulation (CRE-01)

/// Ce qui décrit une simulation (R-CRE-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationTerms {
    pub kind: LoanKind,
    pub principal: Money,
    pub annual_rate: Rate,
    pub duration_months: u32,
    pub insurance: Insurance,
    /// Frais de dossier.
    pub fees: Money,
    /// Frais de garantie (caution, hypothèque).
    pub guarantee_fees: Money,
}

impl SimulationTerms {
    /// Conditions de prêt de la simulation, la première échéance un mois après `today` (R-CRE-2).
    pub fn loan_terms(&self, today: Date) -> LoanTerms {
        LoanTerms {
            principal: self.principal,
            annual_rate: self.annual_rate,
            duration_months: self.duration_months,
            first_installment: today.checked_add_months(chrono::Months::new(1)).unwrap_or(today),
            insurance: self.insurance,
        }
    }

    /// Vérifie la simulation (R-CRE-1) : conditions de prêt valides, frais positifs ou nuls.
    pub fn validate(&self, today: Date) -> Result<(), DomainError> {
        if self.fees.is_negative() || self.guarantee_fees.is_negative() {
            return Err(DomainError::InvalidSimulation);
        }
        self.loan_terms(today).validate().map_err(|_| DomainError::InvalidSimulation)
    }
}

/// Résultat d'une simulation (R-CRE-2 à R-CRE-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulation {
    /// Mensualité hors assurance.
    pub payment: Money,
    /// Assurance de chaque échéance.
    pub insurance: Money,
    pub total_interest: Money,
    pub total_insurance: Money,
    /// Intérêts + assurance + frais (R-CRE-3).
    pub total_cost: Money,
    /// TAEG (R-CRE-4).
    pub apr: Rate,
    pub schedule: Vec<Installment>,
}

impl Simulation {
    /// Échéance prélevée chaque mois : mensualité + assurance.
    pub fn total_payment(&self) -> Money {
        self.payment + self.insurance
    }
}

/// Calcule une simulation (R-CRE-2 à R-CRE-4).
pub fn simulate(terms: &SimulationTerms, today: Date) -> Result<Simulation, DomainError> {
    terms.validate(today)?;
    let loan = terms.loan_terms(today);
    let schedule = loans::schedule(&loan).map_err(|_| DomainError::InvalidSimulation)?;
    let total_interest: Money = schedule.iter().map(|i| i.interest).sum();
    let total_insurance: Money = schedule.iter().map(|i| i.insurance).sum();
    let fees = terms.fees + terms.guarantee_fees;
    let payments: Vec<Money> = schedule.iter().map(Installment::total).collect();
    Ok(Simulation {
        payment: loan.monthly_payment(),
        insurance: loan.monthly_insurance(),
        total_interest,
        total_insurance,
        total_cost: total_interest + total_insurance + fees,
        apr: apr(terms.principal - fees, &payments),
        schedule,
    })
}

/// TAEG (R-CRE-4) : taux annuel `(1 + i)^12 − 1`, où `i` actualise les échéances à la somme reçue.
/// La somme reçue est le capital diminué des frais. Dichotomie sur `i` entre 0 et 100 % par mois.
pub fn apr(received: Money, payments: &[Money]) -> Rate {
    let received = Decimal::from(received.minor());
    let present_value = |i: Decimal| {
        let discount = Decimal::ONE / (Decimal::ONE + i);
        let mut factor = Decimal::ONE;
        let mut total = Decimal::ZERO;
        for payment in payments {
            factor *= discount;
            total += Decimal::from(payment.minor()) * factor;
        }
        total
    };
    if !received.is_sign_positive() || received.is_zero() || present_value(Decimal::ZERO) <= received {
        return Rate::from_ppm(0);
    }
    let (mut low, mut high) = (Decimal::ZERO, Decimal::ONE);
    let precision = Decimal::new(1, 15);
    for _ in 0..200 {
        if high - low < precision {
            break;
        }
        let middle = (low + high) / Decimal::TWO;
        if present_value(middle) > received {
            low = middle;
        } else {
            high = middle;
        }
    }
    let monthly = (low + high) / Decimal::TWO;
    let mut annual = Decimal::ONE;
    for _ in 0..12 {
        annual *= Decimal::ONE + monthly;
    }
    let ppm = ((annual - Decimal::ONE) * Decimal::from(1_000_000))
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    Rate::from_ppm(ppm.to_i64().unwrap_or_default())
}

// ======================================================================= Calculs inverses (CRE-02)

/// Échéance totale (mensualité + assurance) d'un capital.
fn total_payment(principal: Money, annual_rate: Rate, duration_months: u32, insurance: Insurance) -> Money {
    let terms = LoanTerms { principal, annual_rate, duration_months, first_installment: Date::MIN, insurance };
    terms.monthly_payment() + terms.monthly_insurance()
}

/// Capital empruntable, à l'euro inférieur, dont l'échéance totale ne dépasse pas `target` (R-CRE-5).
pub fn borrowing_capacity(
    target: Money,
    annual_rate: Rate,
    duration_months: u32,
    insurance: Insurance,
) -> Result<Money, DomainError> {
    if duration_months == 0 || duration_months > MAX_DURATION_MONTHS || annual_rate.ppm() < 0 {
        return Err(DomainError::InvalidSimulation);
    }
    // Une mensualité nulle ne permet aucun crédit, même si l'échéance d'un euro s'arrondit à zéro.
    if !target.is_positive() {
        return Err(DomainError::TargetPaymentTooLow);
    }
    let euro = 100;
    let fits =
        |euros: i64| total_payment(Money::from_minor(euros * euro), annual_rate, duration_months, insurance) <= target;
    if !fits(1) {
        return Err(DomainError::TargetPaymentTooLow);
    }
    // À taux nul et sans assurance, le capital vaut au plus la mensualité × la durée.
    let (mut low, mut high) = (1_i64, target.minor() / euro * i64::from(duration_months) + 1);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if fits(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(Money::from_minor(low * euro))
}

/// Durée la plus courte, en mois, dont l'échéance totale ne dépasse pas `target` (R-CRE-5).
pub fn shortest_duration(
    principal: Money,
    annual_rate: Rate,
    insurance: Insurance,
    target: Money,
) -> Result<u32, DomainError> {
    if !principal.is_positive() || annual_rate.ppm() < 0 {
        return Err(DomainError::InvalidSimulation);
    }
    (1..=MAX_DURATION_MONTHS)
        .find(|&months| total_payment(principal, annual_rate, months, insurance) <= target)
        .ok_or(DomainError::TargetPaymentTooLow)
}

// ======================================================================= Plan de financement (CRE-04)

/// Type de projet d'une demande (R-CRE-10).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectKind {
    ExistingHome,
    NewHome,
    Works,
    Vehicle,
    Other,
}

impl ProjectKind {
    /// Achat d'un logement : le compromis de vente est demandé (R-CRE-17).
    pub fn is_home_purchase(self) -> bool {
        matches!(self, Self::ExistingHome | Self::NewHome)
    }

    /// Durée maximale selon le HCSF (R-CRE-15).
    pub fn max_duration(self) -> u32 {
        if self == Self::NewHome { MAX_DURATION_NEW_HOME } else { MAX_DURATION }
    }
}

/// Frais de notaire estimés (R-CRE-12) : 7,5 % du prix dans l'ancien, 2,5 % dans le neuf, à l'euro.
pub fn estimated_notary_fees(kind: ProjectKind, price: Money) -> Money {
    let percent = match kind {
        ProjectKind::ExistingHome => Decimal::new(75, 3),
        ProjectKind::NewHome => Decimal::new(25, 3),
        _ => return Money::ZERO,
    };
    let euros = (Decimal::from(price.minor()) * percent / Decimal::from(100))
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    Money::from_minor(euros.to_i64().unwrap_or_default() * 100)
}

/// Plan de financement d'une demande (R-CRE-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinancingPlan {
    pub price: Money,
    pub notary_fees: Money,
    pub works: Money,
    pub other_costs: Money,
    pub down_payment: Money,
}

impl FinancingPlan {
    /// Coût du projet : prix, frais de notaire, travaux et autres frais.
    pub fn project_cost(&self) -> Money {
        self.price + self.notary_fees + self.works + self.other_costs
    }

    /// Montant à emprunter : coût du projet − apport.
    pub fn to_borrow(&self) -> Money {
        self.project_cost() - self.down_payment
    }

    /// Montants positifs ou nuls ; l'apport ne dépasse pas le coût du projet (R-CRE-11).
    pub fn validate(&self) -> Result<(), DomainError> {
        let amounts = [self.price, self.notary_fees, self.works, self.other_costs, self.down_payment];
        if amounts.iter().any(|m| m.is_negative()) || self.down_payment > self.project_cost() {
            return Err(DomainError::InvalidFinancingPlan);
        }
        Ok(())
    }
}

// ======================================================================= Capacité d'emprunt (CRE-04)

/// Revenu mensuel d'un emploi pour une demande (R-CRE-13) : le net en vigueur ce mois, plus les primes
/// récurrentes des douze derniers mois ramenées au mois. Rien pour un emploi terminé ou pas commencé.
pub fn employment_income(job: &Job, pays: &[Pay], bonuses: &[Bonus], today: Date) -> Money {
    if today < job.start_date || job.end_date.is_some_and(|end| end < today) {
        return Money::ZERO;
    }
    let net = salaries::in_force(pays, salaries::month_of(today)).map_or(Money::ZERO, |p| p.monthly_net);
    let year_ago = today.with_year(today.year() - 1).unwrap_or(today);
    let recurring: Money =
        bonuses.iter().filter(|b| b.recurring && b.paid_on > year_ago && b.paid_on <= today).map(|b| b.net).sum();
    net + round(Decimal::from(recurring.minor()) / Decimal::from(12))
}

fn round(value: Decimal) -> Money {
    let rounded = value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    Money::from_minor(rounded.to_i64().unwrap_or_default())
}

/// Ce qui entre dans la capacité d'emprunt (R-CRE-13 à R-CRE-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityInput {
    /// Revenus des emplois en cours et des autres revenus de l'emprunteur, et du co-emprunteur.
    pub income: Money,
    /// Échéances des prêts en cours non exclus, autres charges, charges du co-emprunteur.
    pub charges: Money,
    /// Mensualité envisagée, assurance comprise : celle de la simulation retenue.
    pub payment: Option<Money>,
    /// Taux, durée et assurance de la simulation retenue, pour le capital empruntable.
    pub simulation: Option<(Rate, u32, Insurance)>,
    pub project: ProjectKind,
    /// Soldes des comptes de l'épargne de précaution.
    pub savings: Money,
    pub down_payment: Money,
}

/// Capacité d'emprunt (R-CRE-15). Les dépassements des normes du HCSF sont signalés, rien n'est bloqué.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity {
    pub income: Money,
    pub charges: Money,
    pub payment: Option<Money>,
    /// (Charges + mensualité envisagée) / revenus ; sans revenus, absent.
    pub debt_ratio: Option<Rate>,
    pub remaining_income: Money,
    /// 35 % des revenus − charges, jamais négative.
    pub max_payment: Money,
    /// Capital empruntable avec la mensualité maximale, aux conditions de la simulation retenue.
    pub borrowing_capacity: Option<Money>,
    /// Épargne de précaution − apport.
    pub savings_after: Money,
    pub debt_ratio_exceeded: bool,
    pub duration_exceeded: bool,
}

pub fn capacity(input: &CapacityInput) -> Capacity {
    let payment = input.payment.unwrap_or(Money::ZERO);
    let income = Decimal::from(input.income.minor());
    let debt_ratio = input.income.is_positive().then(|| {
        let ratio = Decimal::from((input.charges + payment).minor()) * Decimal::from(1_000_000) / income;
        Rate::from_ppm(ratio.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_i64().unwrap_or(0))
    });
    let ceiling = round(income * MAX_DEBT_RATIO.to_fraction());
    let max_payment = if ceiling > input.charges { ceiling - input.charges } else { Money::ZERO };
    let borrowing_capacity = input
        .simulation
        .and_then(|(rate, months, insurance)| borrowing_capacity(max_payment, rate, months, insurance).ok());
    Capacity {
        income: input.income,
        charges: input.charges,
        payment: input.payment,
        debt_ratio,
        remaining_income: input.income - input.charges - payment,
        max_payment,
        borrowing_capacity,
        savings_after: input.savings - input.down_payment,
        debt_ratio_exceeded: debt_ratio.is_some_and(|r| r > MAX_DEBT_RATIO),
        duration_exceeded: input.simulation.is_some_and(|(_, months, _)| months > input.project.max_duration()),
    }
}

// ======================================================================= Capacité d'achat (CRE-07)

/// Ce qui entre dans la capacité d'achat à une date (R-CRE-31, R-CRE-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurchaseInput {
    pub income: Money,
    pub charges: Money,
    /// Taux d'endettement visé.
    pub max_debt_ratio: Rate,
    pub annual_rate: Rate,
    pub duration_months: u32,
    pub insurance: Insurance,
    pub fees: Money,
    pub guarantee_fees: Money,
    pub project: ProjectKind,
    /// Épargne mobilisable.
    pub savings: Money,
    /// Réserve de sécurité gardée hors de l'apport.
    pub reserve: Money,
}

/// Capacité d'achat à une date (R-CRE-31, R-CRE-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Purchase {
    pub max_payment: Money,
    pub capital: Money,
    pub down_payment: Money,
    pub budget: Money,
    pub max_price: Money,
    pub notary_fees: Money,
    /// Échéance totale du crédit au capital empruntable : mensualité + assurance.
    pub payment: Money,
    /// Épargne mobilisable − apport.
    pub savings_after: Money,
}

/// Plus grand prix, à la centaine d'euros, dont le prix et les frais de notaire estimés tiennent dans le budget
/// (R-CRE-31). Renvoie le prix et ses frais de notaire.
pub fn max_price(kind: ProjectKind, budget: Money) -> (Money, Money) {
    let hundred = 10_000;
    let rate = match kind {
        ProjectKind::ExistingHome => Decimal::new(1075, 3),
        ProjectKind::NewHome => Decimal::new(1025, 3),
        _ => Decimal::ONE,
    };
    let steps = (Decimal::from(budget.minor().max(0)) / rate / Decimal::from(hundred)).floor();
    let mut price = Money::from_minor(steps.to_i64().unwrap_or_default() * hundred);
    let mut notary = estimated_notary_fees(kind, price);
    while price.is_positive() && price + notary > budget {
        price -= Money::from_minor(hundred);
        notary = estimated_notary_fees(kind, price);
    }
    (price, notary)
}

/// Capacité d'achat (R-CRE-31, R-CRE-32).
pub fn purchase(input: &PurchaseInput) -> Purchase {
    let ceiling = round(Decimal::from(input.income.minor()) * input.max_debt_ratio.to_fraction());
    let max_payment = if ceiling > input.charges { ceiling - input.charges } else { Money::ZERO };
    let capital = borrowing_capacity(max_payment, input.annual_rate, input.duration_months, input.insurance)
        .unwrap_or(Money::ZERO);
    let down_payment = if input.savings > input.reserve { input.savings - input.reserve } else { Money::ZERO };
    let costs = input.fees + input.guarantee_fees;
    let funds = capital + down_payment;
    let budget = if funds > costs { funds - costs } else { Money::ZERO };
    let (max_price, notary_fees) = max_price(input.project, budget);
    let payment = if capital.is_positive() {
        total_payment(capital, input.annual_rate, input.duration_months, input.insurance)
    } else {
        Money::ZERO
    };
    Purchase {
        max_payment,
        capital,
        down_payment,
        budget,
        max_price,
        notary_fees,
        payment,
        savings_after: input.savings - down_payment,
    }
}

/// Patrimoine projeté avec l'achat (R-CRE-35) : chaque point du patrimoine projeté sans achat, diminué des frais
/// payés à l'achat et du cumul des intérêts et de l'assurance des échéances passées à cette date.
pub fn net_worth_with_purchase(
    baseline: &[(Date, Money)],
    one_off: Money,
    schedule: &[Installment],
) -> Vec<(Date, Money)> {
    baseline
        .iter()
        .map(|(date, value)| {
            let cost: Money = schedule.iter().take_while(|i| i.date <= *date).map(|i| i.interest + i.insurance).sum();
            (*date, *value - one_off - cost)
        })
        .collect()
}

// ======================================================================= Statuts (CRE-06)

/// Statut d'une demande de crédit (R-CRE-20).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApplicationStatus {
    Preparing,
    Submitted,
    Accepted,
    Abandoned,
}

/// Statut d'une démarche auprès d'un établissement (R-CRE-21).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LenderRequestStatus {
    Sent,
    Offer,
    Refused,
    Accepted,
    Withdrawn,
}

impl LenderRequestStatus {
    /// Une démarche en cours peut encore aboutir.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Sent | Self::Offer)
    }
}

/// Type d'un échange au journal d'une démarche (R-CRE-24).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LenderEventKind {
    Sent,
    FollowUp,
    Call,
    Meeting,
    Answer,
    Note,
}

/// Délai de la première relance après l'envoi, puis entre deux relances (R-CRE-25).
pub const FIRST_FOLLOW_UP_DAYS: u64 = 14;
pub const NEXT_FOLLOW_UP_DAYS: u64 = 7;

/// Prochaine relance proposée à partir d'une date (envoi : +14 jours ; relance : +7 jours) (R-CRE-25).
pub fn follow_up_after(date: Date, days: u64) -> Date {
    date.checked_add_days(chrono::Days::new(days)).unwrap_or(date)
}

/// Une démarche est à relancer quand elle attend une réponse et que sa relance est passée ou du jour (R-CRE-26).
pub fn follow_up_due(status: LenderRequestStatus, follow_up_on: Option<Date>, today: Date) -> bool {
    status.is_open() && follow_up_on.is_some_and(|date| date <= today)
}

/// Statut de la demande après l'envoi d'une démarche (R-CRE-20) : une demande en préparation passe à envoyée.
pub fn status_after_sending(status: ApplicationStatus) -> ApplicationStatus {
    if status == ApplicationStatus::Preparing { ApplicationStatus::Submitted } else { status }
}

/// Accepter l'offre d'une démarche (R-CRE-22) : elle passe à acceptée, les autres démarches en cours à retirées.
/// Renvoie le nouveau statut de chaque démarche, dans l'ordre reçu. Seule une démarche avec une offre s'accepte.
pub fn accept_offer(
    requests: &[LenderRequestStatus],
    accepted: usize,
    has_offer: bool,
) -> Result<Vec<LenderRequestStatus>, DomainError> {
    if !has_offer || requests.get(accepted) != Some(&LenderRequestStatus::Offer) {
        return Err(DomainError::OfferNotAcceptable);
    }
    Ok(requests
        .iter()
        .enumerate()
        .map(|(i, &status)| match status {
            _ if i == accepted => LenderRequestStatus::Accepted,
            s if s.is_open() => LenderRequestStatus::Withdrawn,
            s => s,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    const TODAY: &str = "2026-10-29";

    fn euros(value: i64) -> Money {
        Money::from_minor(value * 100)
    }

    fn terms() -> SimulationTerms {
        SimulationTerms {
            kind: LoanKind::Mortgage,
            principal: euros(200_000),
            annual_rate: Rate::from_ppm(30_000),
            duration_months: 240,
            insurance: Insurance::RateOnPrincipal { rate_ppm: 3_000 },
            fees: euros(1_000),
            guarantee_fees: euros(2_000),
        }
    }

    #[test]
    fn r_cre_2_r_cre_3_simulation_costs() {
        let simulation = simulate(&terms(), d(TODAY)).unwrap();
        assert_eq!(simulation.payment, Money::from_minor(110_920));
        assert_eq!(simulation.insurance, euros(50));
        assert_eq!(simulation.total_payment(), Money::from_minor(115_920));
        assert_eq!(simulation.schedule.len(), 240);
        assert_eq!(simulation.schedule[0].date, d("2026-11-29"), "un mois après la date du jour");
        assert_eq!(simulation.total_insurance, euros(12_000));
        let interest: Money = simulation.schedule.iter().map(|i| i.interest).sum();
        assert_eq!(simulation.total_interest, interest);
        assert_eq!(simulation.total_cost, simulation.total_interest + euros(12_000) + euros(3_000));
    }

    #[test]
    fn r_cre_4_apr_is_actuarial_and_includes_fees_and_insurance() {
        let plain =
            SimulationTerms { insurance: Insurance::None, fees: Money::ZERO, guarantee_fees: Money::ZERO, ..terms() };
        // 3 % nominal : (1 + 0,25 %)^12 − 1 = 3,0416 %.
        assert_eq!(simulate(&plain, d(TODAY)).unwrap().apr, Rate::from_ppm(30_416));
        let full = simulate(&terms(), d(TODAY)).unwrap().apr;
        assert!(full > Rate::from_ppm(30_416), "frais et assurance augmentent le TAEG : {full}");
        assert!(full < Rate::from_ppm(40_000));
        // Taux nul et sans frais : TAEG nul.
        assert_eq!(apr(euros(1_200), &[euros(100); 12]), Rate::from_ppm(0));
    }

    #[test]
    fn r_cre_1_simulation_is_validated() {
        let negative = SimulationTerms { fees: euros(-1), ..terms() };
        assert_eq!(simulate(&negative, d(TODAY)), Err(DomainError::InvalidSimulation));
        let too_long = SimulationTerms { duration_months: 601, ..terms() };
        assert_eq!(simulate(&too_long, d(TODAY)), Err(DomainError::InvalidSimulation));
    }

    #[test]
    fn r_cre_5_borrowing_capacity_from_a_target_payment() {
        let insurance = Insurance::RateOnPrincipal { rate_ppm: 3_000 };
        let capacity = borrowing_capacity(euros(1_200), Rate::from_ppm(30_000), 240, insurance).unwrap();
        assert!(total_payment(capacity, Rate::from_ppm(30_000), 240, insurance) <= euros(1_200));
        assert!(total_payment(capacity + euros(1), Rate::from_ppm(30_000), 240, insurance) > euros(1_200));
        assert_eq!(capacity.minor() % 100, 0, "à l'euro");
        assert_eq!(
            borrowing_capacity(Money::ZERO, Rate::from_ppm(30_000), 240, Insurance::None),
            Err(DomainError::TargetPaymentTooLow)
        );
        // Une mensualité qui ne couvre pas l'assurance est refusée.
        let fixed = Insurance::Fixed { amount_minor: 5_000 };
        assert_eq!(
            borrowing_capacity(euros(40), Rate::from_ppm(30_000), 240, fixed),
            Err(DomainError::TargetPaymentTooLow)
        );
    }

    #[test]
    fn r_cre_5_shortest_duration_for_a_target_payment() {
        let months = shortest_duration(euros(200_000), Rate::from_ppm(30_000), Insurance::None, euros(1_400)).unwrap();
        assert_eq!(months, 177, "exemple CRE-02");
        assert!(total_payment(euros(200_000), Rate::from_ppm(30_000), months, Insurance::None) <= euros(1_400));
        assert!(total_payment(euros(200_000), Rate::from_ppm(30_000), months - 1, Insurance::None) > euros(1_400));
        assert_eq!(
            shortest_duration(euros(200_000), Rate::from_ppm(30_000), Insurance::None, euros(400)),
            Err(DomainError::TargetPaymentTooLow)
        );
    }

    #[test]
    fn r_cre_11_r_cre_12_financing_plan() {
        assert_eq!(estimated_notary_fees(ProjectKind::ExistingHome, euros(280_000)), euros(21_000));
        assert_eq!(estimated_notary_fees(ProjectKind::NewHome, euros(280_000)), euros(7_000));
        assert_eq!(estimated_notary_fees(ProjectKind::Vehicle, euros(30_000)), Money::ZERO);
        let plan = FinancingPlan {
            price: euros(280_000),
            notary_fees: euros(21_000),
            works: euros(10_000),
            other_costs: Money::ZERO,
            down_payment: euros(41_000),
        };
        assert_eq!(plan.project_cost(), euros(311_000));
        assert_eq!(plan.to_borrow(), euros(270_000));
        assert_eq!(plan.validate(), Ok(()));
        let too_much = FinancingPlan { down_payment: euros(311_001), ..plan };
        assert_eq!(too_much.validate(), Err(DomainError::InvalidFinancingPlan));
    }

    #[test]
    fn r_cre_13_employment_income_counts_recurring_bonuses() {
        let job = Job { start_date: d("2022-09-01"), end_date: None, raise_month: 1, paid_next_month: false };
        let pays = [
            Pay { effective_month: d("2024-01-01"), annual_gross: euros(40_000), monthly_net: euros(2_550) },
            Pay { effective_month: d("2026-01-01"), annual_gross: euros(42_500), monthly_net: euros(2_700) },
            Pay { effective_month: d("2027-03-01"), annual_gross: euros(45_000), monthly_net: euros(2_900) },
        ];
        let bonus = |paid_on: &str, recurring| Bonus {
            label: "13e mois".into(),
            paid_on: d(paid_on),
            net: euros(2_600),
            gross: None,
            recurring,
        };
        let bonuses = [bonus("2025-12-19", true), bonus("2024-12-19", true), bonus("2026-06-30", false)];
        assert_eq!(employment_income(&job, &pays, &bonuses, d(TODAY)), Money::from_minor(270_000 + 21_667));
        let ended = Job { end_date: Some(d("2026-06-30")), ..job };
        assert_eq!(employment_income(&ended, &pays, &bonuses, d(TODAY)), Money::ZERO);
    }

    #[test]
    fn r_cre_15_capacity_and_hcsf_warnings() {
        let input = CapacityInput {
            income: Money::from_minor(291_667),
            charges: euros(230),
            payment: Some(Money::from_minor(115_920)),
            simulation: Some((Rate::from_ppm(30_000), 240, Insurance::RateOnPrincipal { rate_ppm: 3_000 })),
            project: ProjectKind::ExistingHome,
            savings: euros(60_000),
            down_payment: euros(41_000),
        };
        let c = capacity(&input);
        assert_eq!(c.debt_ratio, Some(Rate::from_ppm(476_297)), "(230 + 1 159,20) / 2 916,67");
        assert!(c.debt_ratio_exceeded);
        assert_eq!(c.remaining_income, Money::from_minor(291_667 - 23_000 - 115_920));
        assert_eq!(c.max_payment, Money::from_minor(102_083 - 23_000));
        assert!(c.borrowing_capacity.is_some_and(|m| m.is_positive()));
        assert_eq!(c.savings_after, euros(19_000));
        assert!(!c.duration_exceeded);

        // Avec un co-emprunteur à 2 500 €, l'endettement passe sous 35 %.
        let joint = capacity(&CapacityInput { income: input.income + euros(2_500), ..input });
        assert_eq!(joint.debt_ratio, Some(Rate::from_ppm(256_468)));
        assert!(!joint.debt_ratio_exceeded);

        // 26 ans : au-delà de 25 ans dans l'ancien, pas dans le neuf.
        let long = CapacityInput { simulation: Some((Rate::from_ppm(30_000), 312, Insurance::None)), ..input };
        assert!(capacity(&long).duration_exceeded);
        assert!(!capacity(&CapacityInput { project: ProjectKind::NewHome, ..long }).duration_exceeded);

        // Sans revenus, pas de taux d'endettement.
        let none = capacity(&CapacityInput { income: Money::ZERO, ..input });
        assert_eq!((none.debt_ratio, none.max_payment), (None, Money::ZERO));
    }

    #[test]
    fn r_cre_20_r_cre_22_statuses() {
        use LenderRequestStatus::*;
        assert_eq!(status_after_sending(ApplicationStatus::Preparing), ApplicationStatus::Submitted);
        assert_eq!(status_after_sending(ApplicationStatus::Abandoned), ApplicationStatus::Abandoned);
        assert_eq!(
            accept_offer(&[Offer, Sent, Refused, Offer], 0, true),
            Ok(vec![Accepted, Withdrawn, Refused, Withdrawn])
        );
        assert_eq!(accept_offer(&[Sent, Offer], 0, true), Err(DomainError::OfferNotAcceptable));
        assert_eq!(accept_offer(&[Offer], 0, false), Err(DomainError::OfferNotAcceptable));
    }
    #[test]
    fn r_cre_31_purchase_capacity_example() {
        let input = PurchaseInput {
            income: Money::from_minor(291_667),
            charges: euros(230),
            max_debt_ratio: MAX_DEBT_RATIO,
            annual_rate: Rate::from_ppm(30_000),
            duration_months: 240,
            insurance: Insurance::None,
            fees: Money::ZERO,
            guarantee_fees: Money::ZERO,
            project: ProjectKind::ExistingHome,
            savings: euros(60_000),
            reserve: euros(12_000),
        };
        let p = purchase(&input);
        assert_eq!(p.max_payment, Money::from_minor(102_083 - 23_000));
        assert_eq!(p.capital, euros(142_596));
        assert_eq!(p.down_payment, euros(48_000));
        assert_eq!(p.budget, euros(190_596));
        assert_eq!((p.max_price, p.notary_fees), (euros(177_200), euros(13_290)), "exemple CRE-07");
        assert_eq!(p.savings_after, euros(12_000));
        assert!(p.payment <= p.max_payment);

        // Une réserve plus grande que l'épargne : pas d'apport ; des frais plus grands que les fonds : pas de budget.
        let poor = purchase(&PurchaseInput { savings: euros(5_000), income: Money::ZERO, ..input });
        assert_eq!((poor.down_payment, poor.capital, poor.max_price), (Money::ZERO, Money::ZERO, Money::ZERO));
        assert_eq!(poor.savings_after, euros(5_000));
    }

    #[test]
    fn r_cre_31_max_price_fits_the_budget_with_notary_fees() {
        let (price, notary) = max_price(ProjectKind::NewHome, euros(100_000));
        assert_eq!((price, notary), (euros(97_500), euros(2_438)));
        assert!(price + notary <= euros(100_000));
        assert_eq!(max_price(ProjectKind::Vehicle, Money::from_minor(2_345_678)), (euros(23_400), Money::ZERO));
        assert_eq!(max_price(ProjectKind::ExistingHome, Money::ZERO), (Money::ZERO, Money::ZERO));
    }

    #[test]
    fn r_cre_35_net_worth_with_purchase_loses_fees_interest_and_insurance() {
        let terms = SimulationTerms { fees: Money::ZERO, guarantee_fees: Money::ZERO, ..terms() };
        let simulation = simulate(&terms, d(TODAY)).unwrap();
        let baseline = [(d("2026-10-31"), euros(100_000)), (d("2026-12-31"), euros(110_000))];
        let with = net_worth_with_purchase(&baseline, euros(20_000), &simulation.schedule);
        assert_eq!(with[0], (d("2026-10-31"), euros(80_000)), "aucune échéance encore");
        let first_two: Money = simulation.schedule[..2].iter().map(|i| i.interest + i.insurance).sum();
        assert_eq!(with[1].1, euros(90_000) - first_two);
    }
    #[test]
    fn r_cre_25_r_cre_26_follow_ups() {
        use LenderRequestStatus::*;
        assert_eq!(follow_up_after(d("2026-10-20"), FIRST_FOLLOW_UP_DAYS), d("2026-11-03"));
        assert_eq!(follow_up_after(d("2026-11-04"), NEXT_FOLLOW_UP_DAYS), d("2026-11-11"));
        assert!(follow_up_due(Sent, Some(d("2026-11-03")), d("2026-11-03")));
        assert!(follow_up_due(Offer, Some(d("2026-11-01")), d("2026-11-03")));
        assert!(!follow_up_due(Sent, Some(d("2026-11-04")), d("2026-11-03")));
        assert!(!follow_up_due(Refused, Some(d("2026-11-01")), d("2026-11-03")));
        assert!(!follow_up_due(Sent, None, d("2026-11-03")));
    }
}
