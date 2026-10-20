//! Taux en millionièmes (ADR-0001).

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Taux en **millionièmes** (ppm) : 1,7 % = 17 000.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rate(i64);

impl Rate {
    pub const ZERO: Rate = Rate(0);
    /// 100 %.
    pub const ONE: Rate = Rate(1_000_000);

    pub const fn from_ppm(ppm: i64) -> Self {
        Rate(ppm)
    }

    pub const fn ppm(self) -> i64 {
        self.0
    }

    /// Fraction décimale exacte : 0,017 pour 1,7 %.
    pub fn to_fraction(self) -> Decimal {
        Decimal::new(self.0, 6)
    }
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} %", (self.to_fraction() * Decimal::from(100)).normalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_as_percentage() {
        assert_eq!(Rate::from_ppm(17_000).to_string(), "1.7 %");
        assert_eq!(Rate::from_ppm(8_500).to_fraction(), Decimal::new(85, 4));
    }
}
