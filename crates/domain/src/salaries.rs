//! Salaires : emplois, rémunérations et primes déclarées ; revenu annuel, comparaison au reçu et projection
//! (spec 10, ADR-0022). Un mois est représenté par son premier jour.

use chrono::{Datelike, Months, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::recurrence::end_of_month;
use crate::shared::{Date, DomainError, Money, Rate};

/// Motif d'une nouvelle rémunération (R-SAL-2).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SalaryChangeReason {
    Hire,
    Raise,
    Promotion,
    TaxRate,
    Other,
}

/// Nature d'une prime (R-SAL-5).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BonusKind {
    ThirteenthMonth,
    Bonus,
    ProfitSharing,
    Other,
}

/// Premier jour du mois d'une date.
pub fn month_of(date: Date) -> Date {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date)
}

fn add_months(month: Date, count: u32) -> Date {
    month.checked_add_months(Months::new(count)).unwrap_or(month)
}

fn previous_month(month: Date) -> Date {
    month.checked_sub_months(Months::new(1)).unwrap_or(month)
}

fn to_money(value: Decimal) -> Money {
    let rounded = value.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
    Money::from_minor(rounded.to_i64().unwrap_or_default())
}

fn grown(amount: Money, growth: Rate, times: u32) -> Decimal {
    let factor = Decimal::ONE + growth.to_fraction();
    (0..times).fold(Decimal::from(amount.minor()), |value, _| value * factor)
}

/// Les dates et rythmes d'un emploi (R-SAL-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Job {
    pub start_date: Date,
    pub end_date: Option<Date>,
    /// Mois des augmentations, de 1 à 12.
    pub raise_month: u32,
    /// Le salaire du mois M est versé en M+1.
    pub paid_next_month: bool,
}

/// Une rémunération en vigueur à partir d'un mois (R-SAL-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pay {
    pub effective_month: Date,
    pub annual_gross: Money,
    pub monthly_net: Money,
}

/// Une prime versée à une date (R-SAL-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bonus {
    pub label: String,
    pub paid_on: Date,
    pub net: Money,
    pub gross: Option<Money>,
    pub recurring: bool,
}

impl Job {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.end_date.is_some_and(|end| end < self.start_date) || !(1..=12).contains(&self.raise_month) {
            return Err(DomainError::InvalidEmployment);
        }
        Ok(())
    }

    /// L'emploi couvre-t-il au moins un jour de ce mois ?
    pub fn covers(&self, month: Date) -> bool {
        month >= month_of(self.start_date) && self.end_date.is_none_or(|end| month <= month_of(end))
    }

    /// Mois de début ou de fin qui n'est pas entièrement travaillé (R-SAL-9).
    pub fn is_partial(&self, month: Date) -> bool {
        (month == month_of(self.start_date) && self.start_date.day() != 1)
            || self.end_date.is_some_and(|end| month == month_of(end) && end != end_of_month(end))
    }

    /// Une rémunération prend effet un premier du mois, pendant l'emploi, avec des montants positifs (R-SAL-2).
    pub fn check_pay(&self, pay: &Pay) -> Result<(), DomainError> {
        if pay.effective_month.day() != 1
            || !self.covers(pay.effective_month)
            || !pay.annual_gross.is_positive()
            || !pay.monthly_net.is_positive()
        {
            return Err(DomainError::InvalidSalary);
        }
        Ok(())
    }

    /// Une prime est versée pendant l'emploi, avec un net positif et un brut positif s'il est connu (R-SAL-5).
    pub fn check_bonus(&self, bonus: &Bonus) -> Result<(), DomainError> {
        if !bonus.net.is_positive()
            || bonus.gross.is_some_and(|g| !g.is_positive())
            || bonus.paid_on < self.start_date
            || self.end_date.is_some_and(|end| bonus.paid_on > end)
        {
            return Err(DomainError::InvalidBonus);
        }
        Ok(())
    }
}

/// Rémunération en vigueur un mois donné : la dernière dont le mois d'effet est au plus tard ce mois (R-SAL-3).
pub fn in_force(pays: &[Pay], month: Date) -> Option<&Pay> {
    pays.iter().filter(|p| p.effective_month <= month).max_by_key(|p| p.effective_month)
}

// ======================================================================= Revenu annuel (SAL-02)

/// Revenu annuel d'un emploi (R-SAL-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnualIncome {
    pub year: i32,
    pub net: Money,
    pub gross: Money,
    /// Mois de l'année couverts par l'emploi.
    pub months: u32,
    /// Un mois de l'emploi n'a pas de rémunération déclarée.
    pub incomplete: bool,
    /// Brut incomplet : année incomplète, ou prime sans brut.
    pub gross_incomplete: bool,
    /// Année en cours ou future : ses mois à venir comptent les rémunérations en vigueur ou prévues.
    pub provisional: bool,
}

impl AnnualIncome {
    /// Une année se compare à une autre si elle couvre 12 mois sans mois non renseigné (R-SAL-8).
    pub fn comparable(&self) -> bool {
        self.months == 12 && !self.incomplete
    }
}

/// Revenu annuel d'un emploi pour une année ; rien si l'emploi ne la touche pas (R-SAL-7). Les mois à venir
/// comptent la rémunération en vigueur ou prévue, les primes prévues et la répétition des primes récurrentes.
pub fn annual(job: &Job, pays: &[Pay], bonuses: &[Bonus], year: i32, today: Date) -> Option<AnnualIncome> {
    let mut net = Money::ZERO;
    let mut gross = Decimal::ZERO;
    let mut months = 0;
    let mut incomplete = false;
    for month in (1..=12).filter_map(|m| NaiveDate::from_ymd_opt(year, m, 1)).filter(|m| job.covers(*m)) {
        months += 1;
        match in_force(pays, month) {
            Some(pay) => {
                net += pay.monthly_net;
                gross += Decimal::from(pay.annual_gross.minor()) / Decimal::from(12);
            }
            None => incomplete = true,
        }
    }
    let paid: Vec<&Bonus> = bonuses.iter().filter(|b| b.paid_on.year() == year).collect();
    if months == 0 && paid.is_empty() {
        return None;
    }
    // Mois à venir : les primes récurrentes se répètent, au montant de leur dernière occurrence.
    let repeated: Vec<&Bonus> = bonuses
        .iter()
        .filter(|b| b.recurring && b.paid_on.year() < year)
        .filter(|b| bonuses.iter().filter(|o| o.label == b.label).map(|o| o.paid_on).max() == Some(b.paid_on))
        .filter(|b| {
            let years = u32::try_from(year - b.paid_on.year()).unwrap_or(0);
            b.paid_on.checked_add_months(Months::new(12 * years)).is_some_and(|date| {
                date > today && date >= job.start_date && job.end_date.is_none_or(|end| date <= end)
            })
        })
        .collect();
    let mut gross_incomplete = incomplete;
    for bonus in paid.into_iter().chain(repeated) {
        net += bonus.net;
        match bonus.gross {
            Some(g) => gross += Decimal::from(g.minor()),
            None => gross_incomplete = true,
        }
    }
    Some(AnnualIncome {
        year,
        net,
        gross: to_money(gross),
        months,
        incomplete,
        gross_incomplete,
        provisional: year >= today.year(),
    })
}

/// Variation d'une année sur la précédente, en millionièmes (R-SAL-8).
pub fn change(previous: Money, current: Money) -> Option<Rate> {
    if !previous.is_positive() {
        return None;
    }
    let ratio =
        Decimal::from((current - previous).minor()) * Decimal::from(Rate::ONE.ppm()) / Decimal::from(previous.minor());
    ratio.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero).to_i64().map(Rate::from_ppm)
}

// ======================================================================= Comparaison au reçu (SAL-03)

/// Net attendu d'un emploi pour un mois de versement (R-SAL-9).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Expected {
    pub amount: Money,
    /// Le mois de paie est couvert par l'emploi, ou une prime est versée ce mois-là.
    pub active: bool,
    /// Mois de début ou de fin d'emploi.
    pub partial: bool,
    /// Mois de paie sans rémunération déclarée.
    pub unknown: bool,
}

impl Expected {
    /// Cumule les attendus de plusieurs emplois reliés à une même catégorie.
    pub fn merge(self, other: Expected) -> Expected {
        Expected {
            amount: self.amount + other.amount,
            active: self.active || other.active,
            partial: self.partial || other.partial,
            unknown: self.unknown || other.unknown,
        }
    }
}

/// Attendu d'un emploi pour le mois de versement `month` : net du mois de paie (M, ou M−1 en versement décalé)
/// et primes versées en M (R-SAL-9).
pub fn expected(job: &Job, pays: &[Pay], bonuses: &[Bonus], month: Date) -> Expected {
    let pay_month = if job.paid_next_month { previous_month(month) } else { month };
    let working = job.covers(pay_month);
    let salary = if working { in_force(pays, pay_month).map(|p| p.monthly_net) } else { None };
    let bonus: Money = bonuses.iter().filter(|b| month_of(b.paid_on) == month).map(|b| b.net).sum();
    Expected {
        amount: salary.unwrap_or(Money::ZERO) + bonus,
        active: working || bonus.is_positive(),
        partial: working && job.is_partial(pay_month),
        unknown: working && salary.is_none(),
    }
}

/// L'écart dépasse-t-il la tolérance, en part de l'attendu (R-SAL-9) ?
pub fn gap_exceeds(expected: Money, received: Money, tolerance: Rate) -> bool {
    let gap = Decimal::from((received - expected).abs().minor()) * Decimal::from(Rate::ONE.ppm());
    gap > Decimal::from(tolerance.ppm()) * Decimal::from(expected.minor().abs())
}

// ======================================================================= Projection (SAL-04)

/// Projection du salaire d'un emploi sous un taux de hausse (R-SAL-10, R-SAL-11).
pub struct Projector<'a> {
    job: &'a Job,
    pays: &'a [Pay],
    bonuses: &'a [Bonus],
    growth: Rate,
    current: Date,
    /// Les hausses automatiques tombent après ce mois : le mois en cours ou la dernière rémunération déclarée.
    threshold: Date,
}

impl<'a> Projector<'a> {
    pub fn new(job: &'a Job, pays: &'a [Pay], bonuses: &'a [Bonus], growth: Rate, today: Date) -> Self {
        let current = month_of(today);
        let last = pays.iter().map(|p| p.effective_month).max().unwrap_or(current);
        Projector { job, pays, bonuses, growth, current, threshold: current.max(last) }
    }

    /// Hausses automatiques appliquées jusqu'à ce mois inclus : mois des augmentations postérieurs au seuil.
    fn raises(&self, month: Date) -> u32 {
        if month <= self.threshold {
            return 0;
        }
        let span = (month.year() - self.threshold.year()) * 12 + month.month() as i32 - self.threshold.month() as i32;
        (1..=span)
            .filter(|offset| {
                let m = add_months(self.threshold, *offset as u32);
                m.month() == self.job.raise_month
            })
            .count() as u32
    }

    /// Net projeté du mois de paie, exact (R-SAL-10).
    fn salary(&self, month: Date) -> Decimal {
        if !self.job.covers(month) {
            return Decimal::ZERO;
        }
        in_force(self.pays, month).map_or(Decimal::ZERO, |p| grown(p.monthly_net, self.growth, self.raises(month)))
    }

    /// Primes du mois : primes déclarées à venir, et répétition annuelle des primes récurrentes (R-SAL-11).
    fn bonuses(&self, month: Date) -> Decimal {
        let declared: Decimal = self
            .bonuses
            .iter()
            .filter(|b| month_of(b.paid_on) == month && month > self.current)
            .map(|b| Decimal::from(b.net.minor()))
            .sum();
        let mut repeated = Decimal::ZERO;
        for last in self.bonuses.iter().filter(|b| b.recurring) {
            let latest = self.bonuses.iter().filter(|b| b.label == last.label).map(|b| b.paid_on).max();
            if latest != Some(last.paid_on) || month <= month_of(last.paid_on) || month.month() != last.paid_on.month()
            {
                continue;
            }
            let years = (month.year() - last.paid_on.year()) as u32;
            let Some(date) = last.paid_on.checked_add_months(Months::new(12 * years)) else { continue };
            let working = date >= self.job.start_date && self.job.end_date.is_none_or(|end| date <= end);
            if month > self.current && working {
                let times = self.raises(month) - self.raises(month_of(last.paid_on));
                repeated += grown(last.net, self.growth, times);
            }
        }
        declared + repeated
    }

    /// Salaire et primes de ce mois, exacts.
    fn month(&self, month: Date) -> Decimal {
        self.salary(month) + self.bonuses(month)
    }

    /// Net projeté de chacun des `months` mois qui suivent le mois en cours, arrondi au centime (R-PRJ-2).
    pub fn monthly(&self, months: u32) -> Vec<Money> {
        (1..=months).map(|i| to_money(self.month(add_months(self.current, i)))).collect()
    }

    /// Net annuel de l'année en cours puis des années suivantes jusqu'à `last_year` : mois écoulés déclarés,
    /// mois à venir projetés (R-SAL-13).
    pub fn years(&self, last_year: i32) -> Vec<(i32, Money)> {
        (self.current.year()..=last_year)
            .map(|year| {
                let total: Decimal = (1..=12)
                    .filter_map(|m| NaiveDate::from_ymd_opt(year, m, 1))
                    .map(|month| {
                        if month > self.current {
                            self.month(month)
                        } else {
                            let salary = if self.job.covers(month) {
                                in_force(self.pays, month).map_or(Money::ZERO, |p| p.monthly_net)
                            } else {
                                Money::ZERO
                            };
                            let paid: Money =
                                self.bonuses.iter().filter(|b| month_of(b.paid_on) == month).map(|b| b.net).sum();
                            Decimal::from((salary + paid).minor())
                        }
                    })
                    .sum();
                (year, to_money(total))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn eur(units: i64) -> Money {
        Money::from_minor(units * 100)
    }

    /// L'emploi de l'exemple commun de la spec 10.
    fn acme() -> (Job, Vec<Pay>, Vec<Bonus>) {
        let job = Job { start_date: d("2022-09-01"), end_date: None, raise_month: 1, paid_next_month: false };
        let pay = |month: &str, gross, net| Pay {
            effective_month: d(month),
            annual_gross: eur(gross),
            monthly_net: eur(net),
        };
        let pays =
            vec![pay("2022-09-01", 38_000, 2_400), pay("2024-01-01", 40_000, 2_550), pay("2026-01-01", 42_500, 2_700)];
        let bonuses = vec![Bonus {
            label: "13e mois".into(),
            paid_on: d("2025-12-19"),
            net: eur(2_600),
            gross: Some(eur(3_400)),
            recurring: true,
        }];
        (job, pays, bonuses)
    }

    const TODAY: &str = "2026-10-22";
    const MEDIAN: Rate = Rate::from_ppm(20_000);

    #[test]
    fn r_sal_1_r_sal_2_r_sal_5_employment_pay_and_bonus_are_checked() {
        let (job, pays, bonuses) = acme();
        assert!(job.validate().is_ok());
        assert_eq!(Job { raise_month: 13, ..job }.validate(), Err(DomainError::InvalidEmployment));
        assert_eq!(Job { end_date: Some(d("2022-08-31")), ..job }.validate(), Err(DomainError::InvalidEmployment));
        assert!(job.check_pay(&pays[0]).is_ok());
        let mid_month = Pay { effective_month: d("2024-01-15"), ..pays[1] };
        assert_eq!(job.check_pay(&mid_month), Err(DomainError::InvalidSalary));
        let before = Pay { effective_month: d("2022-08-01"), ..pays[0] };
        assert_eq!(job.check_pay(&before), Err(DomainError::InvalidSalary));
        assert_eq!(job.check_pay(&Pay { monthly_net: Money::ZERO, ..pays[0] }), Err(DomainError::InvalidSalary));
        assert!(job.check_bonus(&bonuses[0]).is_ok());
        let early = Bonus { paid_on: d("2022-08-31"), ..bonuses[0].clone() };
        assert_eq!(job.check_bonus(&early), Err(DomainError::InvalidBonus));
    }

    #[test]
    fn r_sal_3_pay_in_force_is_the_latest_effective_one() {
        let (_, pays, _) = acme();
        assert_eq!(in_force(&pays, d("2023-12-01")).map(|p| p.monthly_net), Some(eur(2_400)));
        assert_eq!(in_force(&pays, d("2024-01-01")).map(|p| p.monthly_net), Some(eur(2_550)));
        assert_eq!(in_force(&pays, d("2022-08-01")), None);
    }

    #[test]
    fn sal_02_r_sal_7_annual_income() {
        let (job, pays, bonuses) = acme();
        let today = d(TODAY);
        let y2025 = annual(&job, &pays, &bonuses, 2025, today).unwrap();
        assert_eq!((y2025.net, y2025.gross), (eur(33_200), eur(43_400)));
        assert!(y2025.comparable() && !y2025.provisional && !y2025.gross_incomplete);
        let y2022 = annual(&job, &pays, &bonuses, 2022, today).unwrap();
        assert_eq!((y2022.net, y2022.gross, y2022.months), (eur(9_600), Money::from_minor(1_266_667), 4));
        assert!(!y2022.incomplete && !y2022.comparable());
        // Année en cours : le 13e mois récurrent de décembre est compté (R-SAL-7).
        let y2026 = annual(&job, &pays, &bonuses, 2026, today).unwrap();
        assert!(y2026.provisional);
        assert_eq!(y2026.net, eur(12 * 2_700 + 2_600));
        assert_eq!(annual(&job, &pays, &bonuses, 2021, today), None);
    }

    #[test]
    fn r_sal_7_missing_pay_or_gross_marks_the_year_incomplete() {
        let (job, pays, mut bonuses) = acme();
        let late = [pays[1]];
        let y2023 = annual(&job, &late, &bonuses, 2023, d(TODAY)).unwrap();
        assert!(y2023.incomplete && y2023.gross_incomplete);
        bonuses[0].gross = None;
        let y2025 = annual(&job, &pays, &bonuses, 2025, d(TODAY)).unwrap();
        assert!(!y2025.incomplete && y2025.gross_incomplete);
    }

    #[test]
    fn r_sal_8_change_from_previous_year() {
        assert_eq!(change(eur(30_600), eur(33_200)), Some(Rate::from_ppm(84_967)));
        assert_eq!(change(Money::ZERO, eur(1)), None);
    }

    #[test]
    fn sal_03_r_sal_9_expected_and_gap() {
        let (job, pays, bonuses) = acme();
        let september = expected(&job, &pays, &bonuses, d("2026-09-01"));
        assert_eq!(september, Expected { amount: eur(2_700), active: true, partial: false, unknown: false });
        let tolerance = Rate::from_ppm(50_000);
        assert!(!gap_exceeds(september.amount, Money::from_minor(270_215), tolerance));
        assert!(gap_exceeds(september.amount, Money::ZERO, tolerance));
        let december = expected(&job, &pays, &bonuses, d("2025-12-01"));
        assert_eq!(december.amount, eur(5_150));
        let hired = expected(&job, &pays, &bonuses, d("2022-09-01"));
        assert!(!hired.partial, "embauche un 1er : mois complet");
        let late = Job { start_date: d("2022-09-15"), ..job };
        assert!(expected(&late, &pays, &bonuses, d("2022-09-01")).partial);
    }

    #[test]
    fn r_sal_9_paid_next_month_shifts_the_expected_salary() {
        let (job, pays, bonuses) = acme();
        let shifted = Job { paid_next_month: true, ..job };
        assert_eq!(expected(&shifted, &pays, &bonuses, d("2024-01-01")).amount, eur(2_400));
        assert_eq!(expected(&shifted, &pays, &bonuses, d("2024-02-01")).amount, eur(2_550));
        assert!(!expected(&shifted, &pays, &bonuses, d("2022-09-01")).active);
    }

    #[test]
    fn sal_04_r_sal_10_r_sal_11_projection_with_annual_raises() {
        let (job, pays, bonuses) = acme();
        let projector = Projector::new(&job, &pays, &bonuses, MEDIAN, d(TODAY));
        let months = projector.monthly(15);
        // Novembre 2026, décembre 2026 (13e mois), janvier 2027, …, décembre 2027, janvier 2028.
        assert_eq!(months[0], eur(2_700));
        assert_eq!(months[1], eur(2_700 + 2_600));
        assert_eq!(months[2], eur(2_754));
        assert_eq!(months[13], eur(2_754 + 2_652));
        assert_eq!(months[14], Money::from_minor(280_908));
        let years = projector.years(2027);
        assert_eq!(years[1], (2027, eur(35_700)));
        // 2026 : 12 × 2 700, et le 13e mois de décembre projeté.
        assert_eq!(years[0], (2026, eur(32_400 + 2_600)));
    }

    #[test]
    fn r_sal_4_r_sal_10_a_planned_pay_replaces_the_automatic_raise() {
        let (job, mut pays, bonuses) = acme();
        pays.push(Pay { effective_month: d("2027-03-01"), annual_gross: eur(45_000), monthly_net: eur(2_900) });
        let months = Projector::new(&job, &pays, &bonuses, MEDIAN, d(TODAY)).monthly(15);
        assert_eq!(months[2], eur(2_700), "janvier 2027 : pas de hausse automatique");
        assert_eq!(months[4], eur(2_900), "mars 2027");
        assert_eq!(months[14], eur(2_958), "janvier 2028");
    }

    #[test]
    fn r_sal_10_nothing_after_the_end_of_the_job() {
        let (job, pays, bonuses) = acme();
        let ending = Job { end_date: Some(d("2027-06-30")), ..job };
        let months = Projector::new(&ending, &pays, &bonuses, MEDIAN, d(TODAY)).monthly(14);
        assert_eq!(months[7], eur(2_754), "juin 2027");
        assert_eq!(months[8], Money::ZERO, "juillet 2027");
        assert_eq!(months[13], Money::ZERO, "pas de 13e mois en décembre 2027");
    }

    #[test]
    fn r_sal_10_a_future_job_starts_at_its_start_date() {
        let job = Job { start_date: d("2027-02-01"), end_date: None, raise_month: 1, paid_next_month: false };
        let pays = [Pay { effective_month: d("2027-02-01"), annual_gross: eur(50_000), monthly_net: eur(3_200) }];
        let months = Projector::new(&job, &pays, &[], MEDIAN, d(TODAY)).monthly(16);
        assert_eq!(months[2], Money::ZERO, "janvier 2027");
        assert_eq!(months[3], eur(3_200), "février 2027");
        assert_eq!(months[14], eur(3_264), "janvier 2028");
        assert_eq!(Projector::new(&job, &[], &[], MEDIAN, d(TODAY)).monthly(16)[3], Money::ZERO);
    }
}
