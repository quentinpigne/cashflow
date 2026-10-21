//! Épargne (spec 06) : moteur d'intérêts par quinzaines pour l'épargne à taux déterminé, et mesure de
//! performance (plus-value, TRI, TWR) pour l'épargne à rendement variable.
//!
//! Tout est calculé en décimal exact (ADR-0001) : aucun flottant, y compris pour le TRI.

use chrono::{Datelike, NaiveDate};
use rust_decimal::prelude::{MathematicalOps, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};

use crate::shared::{Date, Money, Rate};

// ======================================================================= Taux déterminé

/// Un mouvement du compte, daté pour le calcul des intérêts (R-EPA-0 : date de valeur, à défaut date
/// bancaire, à défaut date).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Movement {
    pub date: Date,
    pub amount: Money,
}

/// Taux annuel en vigueur à partir d'une date (`RatePeriod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatePoint {
    pub from: Date,
    pub rate: Rate,
}

/// Taux en vigueur à une date : le dernier dont la date d'effet est passée, zéro sinon.
pub fn rate_at(rates: &[RatePoint], date: Date) -> Rate {
    rates.iter().filter(|r| r.from <= date).max_by_key(|r| r.from).map_or(Rate::ZERO, |r| r.rate)
}

/// Numéro absolu de la quinzaine qui contient `date` (R-EPA-1) : 24 par an, du 1er au 15 puis du 16 à
/// la fin du mois.
fn fortnight(date: Date) -> i64 {
    i64::from(date.year()) * 24 + i64::from(date.month0()) * 2 + i64::from(date.day() > 15)
}

fn fortnight_bounds(number: i64) -> (Date, Date) {
    let year = i32::try_from(number.div_euclid(24)).unwrap_or_default();
    let index = number.rem_euclid(24);
    let month = u32::try_from(index / 2 + 1).unwrap_or(1);
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default();
    if index % 2 == 0 {
        (first, first.with_day(15).unwrap_or(first))
    } else {
        let next = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)
        };
        (first.with_day(16).unwrap_or(first), next.and_then(|n| n.pred_opt()).unwrap_or(first))
    }
}

/// Une quinzaine du calcul, pour le détail affiché (EPA-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortnightInterest {
    pub start: Date,
    pub end: Date,
    /// Solde en valeur de la quinzaine (jamais négatif).
    pub balance: Money,
    pub rate: Rate,
    /// Intérêt de la quinzaine, en centimes, non arrondi (R-EPA-6).
    pub interest: Decimal,
}

/// Les intérêts d'une année.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearInterest {
    pub year: i32,
    pub fortnights: Vec<FortnightInterest>,
}

/// Arrondi au centime le plus proche, demis vers le haut (R-EPA-6).
fn round_minor(value: Decimal) -> Money {
    let rounded = value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    Money::from_minor(rounded.to_i64().unwrap_or_default())
}

impl YearInterest {
    /// Total de l'année, arrondi une seule fois (R-EPA-6).
    pub fn total(&self) -> Money {
        round_minor(self.fortnights.iter().map(|f| f.interest).sum())
    }

    /// Intérêts courus : quinzaines terminées au plus tard le `date` (EPA-02).
    pub fn accrued(&self, date: Date) -> Money {
        round_minor(self.fortnights.iter().filter(|f| f.end <= date).map(|f| f.interest).sum())
    }
}

/// Paramètres du calcul par quinzaines d'un compte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FortnightlyTerms {
    pub opened: Date,
    pub closed: Option<Date>,
    /// Arrondir chaque quinzaine plutôt que le total (option de R-EPA-6).
    pub round_each: bool,
}

/// Intérêts de l'année `year` par la méthode des quinzaines (R-EPA-1 à R-EPA-6). Le solde d'ouverture
/// est un mouvement à la date d'ouverture ; les intérêts des années précédentes sont des transactions
/// (R-EPA-5), donc des mouvements comme les autres.
pub fn fortnightly_interest(
    movements: &[Movement],
    rates: &[RatePoint],
    year: i32,
    terms: FortnightlyTerms,
) -> YearInterest {
    let first = i64::from(year) * 24;
    // Un retrait cesse de porter intérêt dès sa quinzaine (R-EPA-3) : la clôture aussi.
    let last = terms.closed.map_or(first + 23, |c| (fortnight(c) - 1).min(first + 23));
    let opening = fortnight(terms.opened);
    let mut fortnights = Vec::new();
    for number in first..=last {
        if number <= opening {
            continue;
        }
        let balance: Money = movements
            .iter()
            .filter(|m| {
                let at = fortnight(m.date);
                // Versement : à partir de la quinzaine suivante (R-EPA-2) ; retrait : dès la sienne (R-EPA-3).
                if m.amount.is_negative() { at <= number } else { at < number }
            })
            .map(|m| m.amount)
            .sum();
        let balance = if balance.is_negative() { Money::ZERO } else { balance };
        let (start, end) = fortnight_bounds(number);
        let rate = rate_at(rates, start);
        let mut interest = Decimal::from(balance.minor()) * rate.to_fraction() / Decimal::from(24);
        if terms.round_each {
            interest = Decimal::from(round_minor(interest).minor());
        }
        fortnights.push(FortnightInterest { start, end, balance, rate, interest });
    }
    YearInterest { year, fortnights }
}

/// Premier versement qui porte le capital versé au-delà du plafond (R-EPA-9) : un avertissement, pas un
/// blocage. Les intérêts capitalisés ne comptent pas dans le capital versé : l'appelant les exclut.
pub fn cap_exceeded(deposits: &[Movement], cap: Money) -> Option<(Date, Money)> {
    let mut sorted = deposits.to_vec();
    sorted.sort_by_key(|m| m.date);
    let mut capital = Money::ZERO;
    for movement in sorted {
        capital += movement.amount;
        if movement.amount.is_positive() && capital > cap {
            return Some((movement.date, capital));
        }
    }
    None
}

// ======================================================================= Rendement variable

/// Une valorisation constatée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub date: Date,
    pub value: Money,
}

/// Apport (positif) ou retrait (négatif) externe sur le compte (R-EPA-20).
pub type Flow = Movement;

/// Apports nets sur `]from, to]`.
pub fn net_flows(flows: &[Flow], from: Date, to: Date) -> Money {
    flows.iter().filter(|f| f.date > from && f.date <= to).map(|f| f.amount).sum()
}

/// Plus-value latente sur `[start, end]` (R-EPA-21).
pub fn gain(start: Point, end: Point, flows: &[Flow]) -> Money {
    end.value - start.value - net_flows(flows, start.date, end.date)
}

/// Capital investi (R-EPA-25) : solde d'ouverture et apports nets.
pub fn invested(opening: Money, flows: &[Flow]) -> Money {
    opening + flows.iter().map(|f| f.amount).sum::<Money>()
}

fn days(from: Date, to: Date) -> Decimal {
    Decimal::from((to - from).num_days())
}

/// `(1 + r)^t`, pour `r > −1`.
fn growth(rate: Decimal, years: Decimal) -> Option<Decimal> {
    let base = Decimal::ONE + rate;
    if base <= Decimal::ZERO {
        return None;
    }
    if years.is_zero() {
        return Some(Decimal::ONE);
    }
    base.checked_ln().and_then(|ln| (ln * years).checked_exp())
}

/// Nombre minimal de jours pour un TRI (R-EPA-22).
const MIN_IRR_DAYS: i64 = 30;

/// TRI annualisé sur `[start, end]` (R-EPA-22) : `r` tel que `Σ flux × (1 + r)^((end − d)/365) = V(end)`,
/// la valeur de départ comptant comme flux initial. Newton, puis dichotomie si Newton ne converge pas.
pub fn irr(start: Point, flows: &[Flow], end: Point) -> Option<Rate> {
    if (end.date - start.date).num_days() < MIN_IRR_DAYS {
        return None;
    }
    let year = Decimal::from(365);
    let mut cash: Vec<(Decimal, Decimal)> =
        vec![(Decimal::from(start.value.minor()), days(start.date, end.date) / year)];
    cash.extend(
        flows
            .iter()
            .filter(|f| f.date > start.date && f.date <= end.date)
            .map(|f| (Decimal::from(f.amount.minor()), days(f.date, end.date) / year)),
    );
    if cash.iter().all(|(amount, _)| amount.is_zero()) {
        return None;
    }
    let target = Decimal::from(end.value.minor());
    let f = |r: Decimal| -> Option<Decimal> {
        let mut total = -target;
        for (amount, t) in &cash {
            total += *amount * growth(r, *t)?;
        }
        Some(total)
    };
    let derivative = |r: Decimal| -> Option<Decimal> {
        let mut total = Decimal::ZERO;
        for (amount, t) in &cash {
            total += *amount * *t * growth(r, *t - Decimal::ONE)?;
        }
        Some(total)
    };
    let tolerance = Decimal::new(1, 12);
    let to_rate = |r: Decimal| Rate::from_ppm((r * Decimal::from(1_000_000)).round().to_i64().unwrap_or_default());

    let mut r = Decimal::new(5, 2);
    for _ in 0..50 {
        let (Some(value), Some(slope)) = (f(r), derivative(r)) else { break };
        if slope.is_zero() {
            break;
        }
        let next = r - value / slope;
        if next <= Decimal::NEGATIVE_ONE {
            break;
        }
        if (next - r).abs() < tolerance {
            return Some(to_rate(next));
        }
        r = next;
    }
    // Dichotomie sur ]−99 %, 1 000 %] : f décroît quand r croît si les flux sont des apports.
    let (mut low, mut high) = (Decimal::new(-99, 2), Decimal::from(10));
    let (mut f_low, f_high) = (f(low)?, f(high)?);
    if f_low.is_sign_positive() == f_high.is_sign_positive() {
        return None;
    }
    for _ in 0..200 {
        let mid = (low + high) / Decimal::TWO;
        let f_mid = f(mid)?;
        if f_mid.is_sign_positive() == f_low.is_sign_positive() {
            low = mid;
            f_low = f_mid;
        } else {
            high = mid;
        }
        if high - low < tolerance {
            break;
        }
    }
    Some(to_rate((low + high) / Decimal::TWO))
}

/// Performance pondérée par le temps (R-EPA-23) : rendements de Dietz modifié chaînés entre valorisations
/// consécutives. Non annualisée ; c'est une approximation.
pub fn twr(points: &[Point], flows: &[Flow]) -> Option<Rate> {
    let mut sorted = points.to_vec();
    sorted.sort_by_key(|p| p.date);
    if sorted.len() < 2 {
        return None;
    }
    let mut product = Decimal::ONE;
    for pair in sorted.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let period = days(a.date, b.date);
        if period.is_zero() {
            continue;
        }
        let inside: Vec<&Flow> = flows.iter().filter(|f| f.date > a.date && f.date <= b.date).collect();
        let net: Decimal = inside.iter().map(|f| Decimal::from(f.amount.minor())).sum();
        let weighted: Decimal =
            inside.iter().map(|f| Decimal::from(f.amount.minor()) * days(f.date, b.date) / period).sum();
        let base = Decimal::from(a.value.minor()) + weighted;
        if base <= Decimal::ZERO {
            return None;
        }
        let change = Decimal::from(b.value.minor()) - Decimal::from(a.value.minor()) - net;
        product *= Decimal::ONE + change / base;
    }
    let result = product - Decimal::ONE;
    Some(Rate::from_ppm((result * Decimal::from(1_000_000)).round().to_i64().unwrap_or_default()))
}

/// Écart maximal entre une valorisation et le 31 décembre, pour la performance annuelle (R-EPA-24).
const YEAR_END_TOLERANCE_DAYS: i64 = 15;

/// Valorisation la plus proche d'une date, à ±15 jours.
pub fn nearest(points: &[Point], date: Date) -> Option<Point> {
    points
        .iter()
        .filter(|p| (p.date - date).num_days().abs() <= YEAR_END_TOLERANCE_DAYS)
        .min_by_key(|p| (p.date - date).num_days().abs())
        .copied()
}

/// Performance d'une année civile (R-EPA-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearPerformance {
    pub year: i32,
    pub start: Point,
    pub end: Point,
    pub gain: Money,
    pub irr: Option<Rate>,
}

/// Performance par année civile, de l'année d'ouverture à celle de `today`. Le départ est la valorisation
/// la plus proche du 31/12 précédent, ou l'ouverture pour l'année d'ouverture ; une année sans
/// valorisation proche de ses bornes est « données insuffisantes » (`None`).
pub fn yearly(opening: Point, points: &[Point], flows: &[Flow], today: Date) -> Vec<(i32, Option<YearPerformance>)> {
    let mut result = Vec::new();
    for year in opening.date.year()..=today.year() {
        let start = if year == opening.date.year() {
            Some(opening)
        } else {
            NaiveDate::from_ymd_opt(year - 1, 12, 31).and_then(|d| nearest(points, d))
        };
        let end = NaiveDate::from_ymd_opt(year, 12, 31).and_then(|d| nearest(points, d));
        let performance = match (start, end) {
            (Some(start), Some(end)) if end.date > start.date => {
                Some(YearPerformance { year, start, end, gain: gain(start, end, flows), irr: irr(start, flows, end) })
            }
            _ => None,
        };
        result.push((year, performance));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn eur(euros: i64) -> Money {
        Money::from_minor(euros * 100)
    }

    fn pct(percent_tenths: i64) -> Rate {
        Rate::from_ppm(percent_tenths * 1_000)
    }

    fn terms(opened: &str) -> FortnightlyTerms {
        FortnightlyTerms { opened: d(opened), closed: None, round_each: false }
    }

    fn year_2027(movements: &[Movement], rates: &[RatePoint]) -> Money {
        fortnightly_interest(movements, rates, 2027, terms("2026-06-01")).total()
    }

    const THREE: [RatePoint; 1] = [RatePoint { from: NaiveDate::MIN, rate: Rate::from_ppm(30_000) }];

    #[test]
    fn fortnights_split_the_year_in_24() {
        assert_eq!(fortnight_bounds(fortnight(d("2027-02-20"))), (d("2027-02-16"), d("2027-02-28")));
        assert_eq!(fortnight_bounds(fortnight(d("2027-12-01"))), (d("2027-12-01"), d("2027-12-15")));
        assert_eq!(fortnight(d("2027-12-31")) + 1, fortnight(d("2028-01-01")));
    }

    /// Cas de référence : versement unique.
    #[test]
    fn single_deposit_earns_a_full_year() {
        let movements = [Movement { date: d("2026-12-31"), amount: eur(10_000) }];
        assert_eq!(year_2027(&movements, &THREE), eur(300));
    }

    /// Cas de référence : versement en cours d'année (R-EPA-2).
    #[test]
    fn deposit_earns_from_the_next_fortnight() {
        let movements = [Movement { date: d("2027-03-03"), amount: eur(1_000) }];
        assert_eq!(year_2027(&movements, &THREE), Money::from_minor(2_375));
        for (date, first) in [("2027-03-16", "2027-04-01"), ("2027-03-15", "2027-03-16")] {
            let movements = [Movement { date: d(date), amount: eur(1_000) }];
            let result = fortnightly_interest(&movements, &THREE, 2027, terms("2026-06-01"));
            let earning = result.fortnights.iter().find(|f| f.balance.is_positive()).unwrap();
            assert_eq!(earning.start, d(first));
        }
    }

    /// Cas de référence : retrait (R-EPA-3).
    #[test]
    fn withdrawal_stops_earning_from_its_fortnight() {
        let movements = [
            Movement { date: d("2026-12-31"), amount: eur(2_000) },
            Movement { date: d("2027-03-20"), amount: eur(-1_000) },
        ];
        assert_eq!(year_2027(&movements, &THREE), Money::from_minor(3_625));
    }

    /// Cas de référence : changement de taux (R-EPA-4).
    #[test]
    fn rate_change_applies_from_the_fortnight_start() {
        let movements = [Movement { date: d("2026-12-31"), amount: eur(10_000) }];
        let rates =
            [RatePoint { from: d("2020-01-01"), rate: pct(30) }, RatePoint { from: d("2027-02-01"), rate: pct(24) }];
        assert_eq!(year_2027(&movements, &rates), eur(245));
    }

    #[test]
    fn accrued_counts_finished_fortnights() {
        let movements = [Movement { date: d("2026-12-31"), amount: eur(10_000) }];
        let result = fortnightly_interest(&movements, &THREE, 2027, terms("2026-06-01"));
        assert_eq!(result.accrued(d("2027-03-31")), Money::from_minor(7_500));
        assert_eq!(result.accrued(d("2027-03-30")), Money::from_minor(6_250));
    }

    #[test]
    fn rounding_once_or_each_fortnight() {
        // 333 € à 1 % : 13,875 centimes par quinzaine.
        let movements = [Movement { date: d("2026-12-31"), amount: eur(333) }];
        let rates = [RatePoint { from: d("2020-01-01"), rate: pct(10) }];
        assert_eq!(year_2027(&movements, &rates), Money::from_minor(333));
        let each = FortnightlyTerms { round_each: true, ..terms("2026-06-01") };
        assert_eq!(fortnightly_interest(&movements, &rates, 2027, each).total(), Money::from_minor(336));
    }

    #[test]
    fn closing_stops_interest() {
        let movements = [Movement { date: d("2026-12-31"), amount: eur(10_000) }];
        let closed = FortnightlyTerms { closed: Some(d("2027-02-10")), ..terms("2026-06-01") };
        let result = fortnightly_interest(&movements, &THREE, 2027, closed);
        assert_eq!(result.fortnights.len(), 2);
    }

    #[test]
    fn r_epa_9_cap_warning() {
        let deposits = [
            Movement { date: d("2027-01-10"), amount: eur(20_000) },
            Movement { date: d("2027-02-10"), amount: eur(-1_000) },
            Movement { date: d("2027-03-10"), amount: eur(4_000) },
        ];
        assert_eq!(cap_exceeded(&deposits, eur(22_950)), Some((d("2027-03-10"), eur(23_000))));
        assert_eq!(cap_exceeded(&deposits[..2], eur(22_950)), None);
    }

    /// Critère d'acceptation d'EPA-06.
    #[test]
    fn epa_06_life_insurance_performance() {
        let start = Point { date: d("2025-01-01"), value: eur(10_000) };
        let flows = [Flow { date: d("2025-07-01"), amount: eur(2_000) }];
        let end = Point { date: d("2025-12-31"), value: eur(12_600) };
        assert_eq!(gain(start, end, &flows), eur(600));
        assert_eq!(invested(eur(10_000), &flows), eur(12_000));
        let rate = irr(start, &flows, end).unwrap();
        assert!((54_600..=54_800).contains(&rate.ppm()), "TRI : {}", rate.ppm());
    }

    #[test]
    fn irr_needs_30_days_and_handles_losses() {
        let start = Point { date: d("2025-01-01"), value: eur(10_000) };
        assert_eq!(irr(start, &[], Point { date: d("2025-01-20"), value: eur(10_100) }), None);
        let loss = irr(start, &[], Point { date: d("2026-01-01"), value: eur(9_000) }).unwrap();
        assert_eq!(loss.ppm(), -100_000);
    }

    #[test]
    fn twr_ignores_the_timing_of_flows() {
        let points = [
            Point { date: d("2025-01-01"), value: eur(10_000) },
            Point { date: d("2025-07-01"), value: eur(11_000) },
            Point { date: d("2026-01-01"), value: eur(22_000) },
        ];
        // +10 % puis un apport de 10 000 € le 1er juillet, puis +4,76 % : le TWR chaîne les deux.
        let flows = [Flow { date: d("2025-07-02"), amount: eur(10_000) }];
        let rate = twr(&points, &flows).unwrap();
        assert!((150_000..=160_000).contains(&rate.ppm()), "TWR : {}", rate.ppm());
    }

    #[test]
    fn r_epa_24_yearly_performance_needs_year_end_valuations() {
        let opening = Point { date: d("2024-03-01"), value: eur(5_000) };
        let points =
            [Point { date: d("2024-12-28"), value: eur(5_200) }, Point { date: d("2025-06-30"), value: eur(5_300) }];
        let years = yearly(opening, &points, &[], d("2025-10-01"));
        assert_eq!(years.len(), 2);
        assert_eq!(years[0].1.unwrap().gain, eur(200));
        assert!(years[1].1.is_none());
    }
}
