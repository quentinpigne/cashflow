//! Projections (spec 07) : calculs purs, qui ne créent ni ne stockent aucune transaction (R-PRJ-1).
//! Décimal exact en interne, arrondi au centime seulement à la sortie (R-PRJ-2).

use chrono::{Datelike, Days, Months, NaiveDate};
use rust_decimal::prelude::{MathematicalOps, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};

use crate::recurrence::end_of_month;
use crate::savings::{self, FortnightlyTerms, Movement, RatePoint};
use crate::shared::{Date, Money, Rate};

fn to_money(value: Decimal) -> Money {
    Money::from_minor(
        value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_i64().unwrap_or_default(),
    )
}

fn dec(money: Money) -> Decimal {
    Decimal::from(money.minor())
}

// ======================================================================= Trésorerie (PRJ-01)

/// Un mouvement connu à venir : transaction en attente datée dans le futur, ou échéance de récurrence
/// non encore honorée (R-PRJ-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashEvent<L> {
    pub date: Date,
    pub amount: Money,
    pub label: L,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashDay {
    pub date: Date,
    /// Solde projeté en fin de journée.
    pub balance: Money,
    /// Dépense variable moyenne cumulée depuis aujourd'hui, comptée à part (R-PRJ-11).
    pub variable: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashProjection {
    pub days: Vec<CashDay>,
    /// Solde minimum projeté et sa date (R-PRJ-12).
    pub minimum: Money,
    pub minimum_date: Date,
}

/// Solde jour par jour de demain à `today + days` (R-PRJ-10, R-PRJ-11). `variable_monthly` est la
/// dépense variable moyenne d'un mois (positive), répartie uniformément sur les jours.
pub fn cash<L>(
    start: Money,
    today: Date,
    days: u32,
    events: &[CashEvent<L>],
    variable_monthly: Option<Money>,
) -> CashProjection {
    let per_day = variable_monthly.map_or(Decimal::ZERO, |m| dec(m) * Decimal::from(12) / Decimal::from(365));
    let mut known = dec(start);
    let mut variable = Decimal::ZERO;
    let mut result = Vec::new();
    let mut minimum = (start, today);
    for offset in 1..=days {
        let Some(date) = today.checked_add_days(Days::new(u64::from(offset))) else { break };
        known += events.iter().filter(|e| e.date == date).map(|e| dec(e.amount)).sum::<Decimal>();
        variable += per_day;
        let balance = to_money(known - variable);
        if balance < minimum.0 {
            minimum = (balance, date);
        }
        result.push(CashDay { date, balance, variable: to_money(variable) });
    }
    // Les mouvements datés d'aujourd'hui ou avant sont déjà dans le solde de départ.
    CashProjection { days: result, minimum: minimum.0, minimum_date: minimum.1 }
}

// ======================================================================= Fins de mois

/// Les `months` fins de mois à partir de celle du mois de `today` (exclue).
pub fn month_ends(today: Date, months: u32) -> Vec<Date> {
    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    (1..=months).filter_map(|m| first.checked_add_months(Months::new(m)).map(end_of_month)).collect()
}

// ======================================================================= Épargne à taux déterminé (PRJ-02)

/// Projection d'un compte à taux déterminé.
pub struct FixedInput<'a> {
    /// Mouvements réels, solde d'ouverture compris, datés pour les intérêts (R-EPA-0).
    pub movements: &'a [Movement],
    /// Barème réel, suivi de la trajectoire du scénario (R-PRJ-21).
    pub rates: &'a [RatePoint],
    pub terms: FortnightlyTerms,
    /// Versements programmés à venir (R-PRJ-4).
    pub deposits: &'a [Movement],
    /// Plafond et capital déjà versé (R-PRJ-22).
    pub cap: Option<Money>,
    pub capital: Money,
    /// Années dont les intérêts sont déjà inscrits en transactions.
    pub recorded_years: &'a [i32],
}

/// Versements programmés réellement faits : ceux qui portent le capital versé au-delà du plafond sont
/// réduits puis ignorés (R-PRJ-22).
pub fn accepted(
    deposits: &[Movement],
    cap: Option<Money>,
    capital: Money,
    today: Date,
    horizon: Date,
) -> Vec<Movement> {
    let mut sorted: Vec<Movement> = deposits.iter().copied().filter(|d| d.date > today && d.date <= horizon).collect();
    sorted.sort_by_key(|d| d.date);
    let mut capital = capital;
    let mut result = Vec::new();
    for deposit in sorted {
        let room = cap.map_or(deposit.amount, |cap| (cap - capital).min(deposit.amount));
        if room.is_positive() {
            result.push(Movement { date: deposit.date, amount: room });
            capital += room;
        }
    }
    result
}

/// Valeur en fin de chaque mois : versements (arrêtés au plafond) et intérêts capitalisés au 31/12,
/// calculés par le même moteur que le réel (R-PRJ-20).
pub fn fixed(input: &FixedInput<'_>, today: Date, months: u32) -> Vec<(Date, Money)> {
    let ends = month_ends(today, months);
    let Some(horizon) = ends.last().copied() else { return Vec::new() };
    let mut all: Vec<Movement> = input.movements.to_vec();
    all.extend(accepted(input.deposits, input.cap, input.capital, today, horizon));
    // Capitalisation annuelle des intérêts non encore inscrits, année par année (R-EPA-5).
    for year in today.year()..=horizon.year() {
        if input.recorded_years.contains(&year) {
            continue;
        }
        let interest = savings::fortnightly_interest(&all, input.rates, year, input.terms).total();
        if interest.is_positive()
            && let Some(date) = NaiveDate::from_ymd_opt(year, 12, 31)
        {
            all.push(Movement { date, amount: interest });
        }
    }
    ends.into_iter().map(|end| (end, all.iter().filter(|m| m.date <= end).map(|m| m.amount).sum())).collect()
}

// ======================================================================= Épargne variable (PRJ-04)

/// Hypothèses d'un compte à rendement variable (R-PRJ-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assumption {
    pub annual_return: Rate,
    pub annual_fees: Rate,
    /// Versement de fin de mois.
    pub monthly_contribution: Money,
}

/// Racine douzième de `1 + rate`.
fn monthly_factor(rate: Rate) -> Decimal {
    let base = Decimal::ONE + rate.to_fraction();
    if base <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    base.checked_ln().and_then(|ln| (ln / Decimal::from(12)).checked_exp()).unwrap_or(Decimal::ONE)
}

/// `V(m+1) = V(m) × (1 + r_m) × (1 − f_m) + versement(m)` (R-PRJ-30) : valeur après chaque mois.
pub fn variable(start: Money, assumption: Assumption, months: u32) -> Vec<Money> {
    let growth = monthly_factor(assumption.annual_return);
    // 1 − f_m = (1 − frais)^(1/12).
    let keep = monthly_factor(Rate::from_ppm(-assumption.annual_fees.ppm()));
    let mut value = dec(start);
    let contribution = dec(assumption.monthly_contribution);
    (0..months)
        .map(|_| {
            value = value * growth * keep + contribution;
            to_money(value)
        })
        .collect()
}

/// Valeur en euros constants (R-PRJ-33) : `valeur / (1 + i)^(m/12)`.
pub fn deflate(value: Money, inflation: Rate, months: u32) -> Money {
    let factor = monthly_factor(inflation);
    let mut divisor = Decimal::ONE;
    for _ in 0..months {
        divisor *= factor;
    }
    if divisor.is_zero() { value } else { to_money(dec(value) / divisor) }
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

    /// Critère d'acceptation de PRJ-04.
    #[test]
    fn prj_04_deterministic_variable_projection() {
        let assumption = Assumption {
            annual_return: Rate::from_ppm(50_000),
            annual_fees: Rate::ZERO,
            monthly_contribution: Money::ZERO,
        };
        let values = variable(eur(10_000), assumption, 120);
        assert_eq!(values[11], eur(10_500));
        assert_eq!(values[119], Money::from_minor(1_628_895));
    }

    #[test]
    fn fees_and_contributions() {
        let assumption = Assumption {
            annual_return: Rate::ZERO,
            annual_fees: Rate::from_ppm(10_000),
            monthly_contribution: eur(100),
        };
        let values = variable(eur(10_000), Assumption { monthly_contribution: Money::ZERO, ..assumption }, 12);
        assert_eq!(values[11], eur(9_900));
        assert_eq!(variable(Money::ZERO, Assumption { annual_fees: Rate::ZERO, ..assumption }, 3)[2], eur(300));
    }

    #[test]
    fn r_prj_10_to_12_cash_projection() {
        let events = [
            CashEvent { date: d("2027-03-05"), amount: eur(-890), label: "Loyer" },
            CashEvent { date: d("2027-03-27"), amount: eur(2_850), label: "Salaire" },
        ];
        let projection = cash(eur(1_000), d("2027-03-01"), 30, &events, Some(eur(365)));
        // 12 € de dépense variable par jour : 1 000 − 4 × 12 − 890 le 5 mars.
        let fifth = projection.days.iter().find(|day| day.date == d("2027-03-05")).unwrap();
        assert_eq!(fifth.balance, eur(1_000 - 48 - 890));
        assert_eq!(projection.minimum_date, d("2027-03-26"));
        assert_eq!(projection.minimum, eur(1_000 - 890 - 25 * 12));
        assert_eq!(projection.days.len(), 30);
    }

    #[test]
    fn r_prj_20_22_fixed_projection_uses_the_interest_engine_and_the_cap() {
        let opening = [Movement { date: d("2026-12-31"), amount: eur(10_000) }];
        let rates = [RatePoint { from: d("2020-01-01"), rate: Rate::from_ppm(30_000) }];
        let deposits: Vec<Movement> = (1..=12)
            .map(|m| Movement { date: NaiveDate::from_ymd_opt(2027, m, 1).unwrap(), amount: eur(1_000) })
            .collect();
        let input = FixedInput {
            movements: &opening,
            rates: &rates,
            terms: FortnightlyTerms { opened: d("2026-06-01"), closed: None, round_each: false },
            deposits: &deposits,
            cap: Some(eur(12_500)),
            capital: eur(10_000),
            recorded_years: &[],
        };
        let values = fixed(&input, d("2026-12-31"), 13);
        // Plafond : 2 500 € de versements seulement (janvier, février et la moitié de mars).
        assert_eq!(values[2], (d("2027-03-31"), eur(12_500)));
        // Les intérêts sont capitalisés au 31 décembre, pas avant.
        assert_eq!(values[10].1, eur(12_500));
        assert!(values[11].1 > eur(12_500));
        assert_eq!(values[12].1, values[11].1);
    }

    #[test]
    fn deflation() {
        assert_eq!(deflate(eur(10_200), Rate::from_ppm(20_000), 12), eur(10_000));
    }
}
