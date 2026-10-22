//! Ventilation des transactions (ADR-0003, R-TRX-10 à R-TRX-14).

use crate::shared::{DomainError, Money};

/// Vérifie l'invariant de ventilation : au moins une ventilation, aucune nulle, Σ = montant (R-TRX-10, R-TRX-11).
pub fn check_splits(total: Money, splits: &[Money]) -> Result<(), DomainError> {
    if splits.is_empty() {
        return Err(DomainError::NoSplit);
    }
    if splits.iter().any(|amount| amount.is_zero()) {
        return Err(DomainError::SplitAmountZero);
    }
    let actual: Money = splits.iter().sum();
    if actual != total {
        return Err(DomainError::SplitSumMismatch { expected: total, actual });
    }
    Ok(())
}

/// Une ventilation de signe opposé à la transaction est un remboursement : l'interface le signale (R-TRX-11).
pub fn is_opposite_sign(total: Money, split: Money) -> bool {
    total.is_negative() != split.is_negative()
}

/// Répartit `total` selon des poids positifs (pourcentages, parts…), en reportant l'arrondi sur la
/// dernière ventilation pour que la somme tombe juste (R-TRX-14).
///
/// Chaque part est arrondie à l'unité mineure, demi vers le haut en valeur absolue (ADR-0001).
pub fn allocate(total: Money, weights: &[u64]) -> Result<Vec<Money>, DomainError> {
    let weight_sum: u128 = weights.iter().map(|&w| u128::from(w)).sum();
    if weights.is_empty() || weight_sum == 0 {
        return Err(DomainError::InvalidAllocation);
    }
    let magnitude = u128::from(total.minor().unsigned_abs());
    let sign = if total.is_negative() { -1 } else { 1 };
    let mut parts = Vec::with_capacity(weights.len());
    let mut allocated: i64 = 0;
    for (index, &weight) in weights.iter().enumerate() {
        let minor = if index + 1 == weights.len() {
            total.minor() - allocated
        } else {
            let share = (magnitude * u128::from(weight) * 2 + weight_sum) / (weight_sum * 2);
            let share = i64::try_from(share).map_err(|_| DomainError::AmountOutOfRange)?;
            sign * share
        };
        allocated += minor;
        parts.push(Money::from_minor(minor));
    }
    Ok(parts)
}

/// Une ventilation a au plus un rattachement : une catégorie, un prêt (remboursement) ou un bien
/// (acquisition) (R-TRX-37).
pub fn check_attachment(category: bool, loan: bool, asset: bool) -> Result<(), DomainError> {
    if [category, loan, asset].iter().filter(|x| **x).count() > 1 {
        return Err(DomainError::SplitAttachmentConflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn m(minor: i64) -> Money {
        Money::from_minor(minor)
    }

    #[test]
    fn r_trx_10_splits_must_sum_to_amount() {
        assert!(check_splits(m(-12_000), &[m(-9_500), m(-1_500), m(-1_000)]).is_ok());
        assert_eq!(
            check_splits(m(-12_000), &[m(-9_500), m(-1_500)]),
            Err(DomainError::SplitSumMismatch { expected: m(-12_000), actual: m(-11_000) })
        );
        assert_eq!(check_splits(m(-12_000), &[]), Err(DomainError::NoSplit));
    }

    #[test]
    fn r_trx_11_split_cannot_be_zero_but_may_be_opposite() {
        assert_eq!(check_splits(m(-100), &[m(-100), m(0)]), Err(DomainError::SplitAmountZero));
        assert!(check_splits(m(-8_000), &[m(-10_000), m(2_000)]).is_ok());
        assert!(is_opposite_sign(m(-8_000), m(2_000)));
    }

    #[test]
    fn trx_03_percentages_50_30_20() {
        assert_eq!(allocate(m(-12_000), &[50, 30, 20]).unwrap(), vec![m(-6_000), m(-3_600), m(-2_400)]);
    }

    #[test]
    fn trx_03_thirds_put_rounding_on_last_split() {
        assert_eq!(allocate(m(-10_000), &[1, 1, 1]).unwrap(), vec![m(-3_333), m(-3_333), m(-3_334)]);
    }

    #[test]
    fn r_trx_14_rejects_empty_or_zero_weights() {
        assert_eq!(allocate(m(-100), &[]), Err(DomainError::InvalidAllocation));
        assert_eq!(allocate(m(-100), &[0, 0]), Err(DomainError::InvalidAllocation));
    }

    proptest! {
        #[test]
        fn r_trx_14_allocation_always_sums_to_total(
            total in -10_000_000_i64..10_000_000,
            weights in prop::collection::vec(0_u64..10_000, 1..8),
        ) {
            prop_assume!(weights.iter().any(|&w| w > 0));
            let parts = allocate(m(total), &weights).unwrap();
            prop_assert_eq!(parts.iter().sum::<Money>(), m(total));
            prop_assert_eq!(parts.len(), weights.len());
        }
    }

    #[test]
    fn r_trx_37_one_attachment_at_most() {
        assert!(check_attachment(false, false, false).is_ok());
        assert!(check_attachment(true, false, false).is_ok());
        assert!(check_attachment(false, true, false).is_ok());
        assert_eq!(check_attachment(true, true, false), Err(DomainError::SplitAttachmentConflict));
        assert_eq!(check_attachment(false, true, true), Err(DomainError::SplitAttachmentConflict));
    }
}
