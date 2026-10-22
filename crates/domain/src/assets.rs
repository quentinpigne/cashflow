//! Biens : logement, véhicule… possédés, hors des comptes (spec 09, ADR-0019).

use serde::{Deserialize, Serialize};

use crate::shared::{Date, DomainError, Money};

/// Type d'un bien (R-BIE-1).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetKind {
    RealEstate,
    Vehicle,
    Other,
}

/// Achat et vente d'un bien : la période pendant laquelle il compte (R-BIE-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holding {
    pub purchase_date: Date,
    pub purchase_price: Money,
    pub sold_on: Option<Date>,
}

impl Holding {
    /// Vérifie un achat et une vente éventuelle (R-BIE-1).
    pub fn validate(&self, sale_price: Option<Money>) -> Result<(), DomainError> {
        let sale_ok = match (self.sold_on, sale_price) {
            (None, Some(_)) => false,
            (_, Some(price)) => !price.is_negative(),
            _ => true,
        };
        if !self.purchase_price.is_positive() || self.sold_on.is_some_and(|d| d < self.purchase_date) || !sale_ok {
            return Err(DomainError::InvalidAsset);
        }
        Ok(())
    }

    /// Le bien est-il possédé à cette date ? De l'achat (inclus) à la vente (exclue).
    pub fn held_at(&self, date: Date) -> bool {
        date >= self.purchase_date && self.sold_on.is_none_or(|sold| date < sold)
    }

    /// Une estimation est datée entre l'achat et la vente, et positive (R-BIE-3).
    pub fn check_valuation(&self, date: Date, value: Money) -> Result<(), DomainError> {
        if !value.is_positive() || date < self.purchase_date || self.sold_on.is_some_and(|sold| date > sold) {
            return Err(DomainError::InvalidAssetValuation);
        }
        Ok(())
    }

    /// Valeur à une date (R-BIE-2) : dernière estimation datée au plus tard ce jour, sinon prix d'achat ; rien
    /// si le bien n'est pas possédé.
    pub fn value_at(&self, valuations: &[(Date, Money)], date: Date) -> Option<Money> {
        if !self.held_at(date) {
            return None;
        }
        let latest = valuations.iter().filter(|(d, _)| *d <= date).max_by_key(|(d, _)| *d);
        Some(latest.map_or(self.purchase_price, |(_, v)| *v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn m(minor: i64) -> Money {
        Money::from_minor(minor)
    }

    fn flat() -> Holding {
        Holding { purchase_date: d("2019-06-15"), purchase_price: m(25_000_000), sold_on: None }
    }

    #[test]
    fn bie_01_r_bie_2_value_is_the_latest_valuation_or_the_purchase_price() {
        let flat = flat();
        assert_eq!(flat.value_at(&[], d("2026-10-20")), Some(m(25_000_000)));
        let valuations = [(d("2026-01-01"), m(28_000_000)), (d("2027-01-01"), m(29_000_000))];
        assert_eq!(flat.value_at(&valuations, d("2026-10-20")), Some(m(28_000_000)));
        assert_eq!(flat.value_at(&valuations, d("2019-06-14")), None);
    }

    #[test]
    fn r_bie_2_a_sold_asset_no_longer_counts() {
        let sold = Holding { sold_on: Some(d("2026-09-01")), ..flat() };
        assert_eq!(sold.value_at(&[], d("2026-08-31")), Some(m(25_000_000)));
        assert_eq!(sold.value_at(&[], d("2026-10-20")), None);
    }

    #[test]
    fn r_bie_1_r_bie_3_purchase_sale_and_valuations_are_checked() {
        assert!(flat().validate(None).is_ok());
        assert_eq!(Holding { purchase_price: Money::ZERO, ..flat() }.validate(None), Err(DomainError::InvalidAsset));
        assert_eq!(flat().validate(Some(m(1))), Err(DomainError::InvalidAsset), "prix de vente sans vente");
        let sold = Holding { sold_on: Some(d("2019-01-01")), ..flat() };
        assert_eq!(sold.validate(None), Err(DomainError::InvalidAsset));
        assert_eq!(flat().check_valuation(d("2019-01-01"), m(1)), Err(DomainError::InvalidAssetValuation));
        assert_eq!(flat().check_valuation(d("2020-01-01"), Money::ZERO), Err(DomainError::InvalidAssetValuation));
        assert!(flat().check_valuation(d("2020-01-01"), m(1)).is_ok());
    }
}
