//! Montants en unités mineures (ADR-0001).

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use super::{Currency, DomainError};

/// Montant signé en **unités mineures** de la devise (centimes pour l'euro).
///
/// La devise est celle du compte qui porte le montant (modèle de données) : `Money` ne la répète pas,
/// mais toute conversion depuis ou vers un décimal la demande, pour ne jamais supposer deux décimales.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_minor(minor: i64) -> Self {
        Money(minor)
    }

    pub const fn minor(self) -> i64 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub const fn abs(self) -> Self {
        Money(self.0.abs())
    }

    /// Valeur décimale exacte dans la devise (12,34 pour 1 234 centimes d'euro).
    pub fn to_decimal(self, currency: Currency) -> Decimal {
        Decimal::new(self.0, currency.minor_digits())
    }

    /// Convertit un décimal en montant, arrondi à l'unité mineure, demi vers le haut (ADR-0001).
    pub fn from_decimal(value: Decimal, currency: Currency) -> Result<Self, DomainError> {
        let scale = Decimal::from(10_i64.pow(currency.minor_digits()));
        let minor = (value * scale).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
        minor.to_i64().map(Money).ok_or(DomainError::AmountOutOfRange)
    }
}

impl Add for Money {
    type Output = Money;

    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        self.0 += rhs.0;
    }
}

impl Sub for Money {
    type Output = Money;

    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl SubAssign for Money {
    fn sub_assign(&mut self, rhs: Money) {
        self.0 -= rhs.0;
    }
}

impl Neg for Money {
    type Output = Money;

    fn neg(self) -> Money {
        Money(-self.0)
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, Add::add)
    }
}

impl<'a> Sum<&'a Money> for Money {
    fn sum<I: Iterator<Item = &'a Money>>(iter: I) -> Money {
        iter.copied().sum()
    }
}

impl fmt::Display for Money {
    /// Affichage technique (unités mineures), pour les journaux et les messages d'erreur.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn converts_from_decimal_with_half_up_rounding() {
        let eur = Currency::EUR;
        assert_eq!(Money::from_decimal(Decimal::from_str("12.345").unwrap(), eur).unwrap(), Money(1235));
        assert_eq!(Money::from_decimal(Decimal::from_str("-12.345").unwrap(), eur).unwrap(), Money(-1235));
        assert_eq!(Money::from_decimal(Decimal::from_str("12.344").unwrap(), eur).unwrap(), Money(1234));
    }

    #[test]
    fn respects_currency_minor_digits() {
        let jpy = Currency::from_str("JPY").unwrap();
        assert_eq!(Money::from_decimal(Decimal::from(1500), jpy).unwrap(), Money(1500));
        assert_eq!(Money(1500).to_decimal(Currency::EUR), Decimal::from_str("15.00").unwrap());
    }

    #[test]
    fn sums() {
        let total: Money = [Money(100), Money(-30), Money(5)].iter().sum();
        assert_eq!(total, Money(75));
    }
}
