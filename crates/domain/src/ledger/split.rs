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

/// Part d'une ligne d'un modèle de ventilation appliqué à plusieurs transactions (R-TRX-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitShare {
    /// Centièmes de pourcent : 3 333 = 33,33 %.
    BasisPoints(u32),
    /// Montant signé, comme la transaction.
    Amount(Money),
}

/// Total des pourcentages d'un modèle, en centièmes de pourcent (100 %).
pub const FULL_BASIS_POINTS: u32 = 10_000;

/// Vérifie qu'un modèle est tout en pourcentages, positifs et de total 100 %, ou tout en montants (R-TRX-16).
pub fn check_split_template(shares: &[SplitShare]) -> Result<(), DomainError> {
    if shares.is_empty() {
        return Err(DomainError::NoSplit);
    }
    if shares.iter().all(|s| matches!(s, SplitShare::Amount(_))) {
        return Ok(());
    }
    let mut total: u32 = 0;
    for share in shares {
        match share {
            SplitShare::BasisPoints(bp) if *bp > 0 => total = total.saturating_add(*bp),
            _ => return Err(DomainError::InvalidSplitTemplate),
        }
    }
    if total != FULL_BASIS_POINTS {
        return Err(DomainError::InvalidSplitTemplate);
    }
    Ok(())
}

/// Montants des ventilations d'une transaction de montant `total` selon un modèle (R-TRX-16) : au prorata des
/// pourcentages, l'arrondi sur la dernière ligne (R-TRX-14), ou les montants tels quels, qui doivent alors faire
/// le montant de la transaction (R-TRX-10). Aucune ventilation ne peut être nulle (R-TRX-11).
pub fn apply_split_template(total: Money, shares: &[SplitShare]) -> Result<Vec<Money>, DomainError> {
    check_split_template(shares)?;
    let amounts = match shares.first() {
        Some(SplitShare::BasisPoints(_)) => {
            let weights: Vec<u64> = shares
                .iter()
                .map(|s| match s {
                    SplitShare::BasisPoints(bp) => u64::from(*bp),
                    SplitShare::Amount(_) => 0,
                })
                .collect();
            allocate(total, &weights)?
        }
        _ => shares
            .iter()
            .map(|s| match s {
                SplitShare::Amount(amount) => *amount,
                SplitShare::BasisPoints(_) => Money::ZERO,
            })
            .collect(),
    };
    check_splits(total, &amounts)?;
    Ok(amounts)
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
    fn r_trx_16_template_in_percentages_follows_each_amount() {
        let template = [SplitShare::BasisPoints(6_000), SplitShare::BasisPoints(4_000)];
        assert_eq!(apply_split_template(m(-12_000), &template).unwrap(), vec![m(-7_200), m(-4_800)]);
        assert_eq!(apply_split_template(m(-1_001), &template).unwrap(), vec![m(-601), m(-400)]);
        let thirds = [SplitShare::BasisPoints(3_333), SplitShare::BasisPoints(3_333), SplitShare::BasisPoints(3_334)];
        assert_eq!(apply_split_template(m(10_000), &thirds).unwrap(), vec![m(3_333), m(3_333), m(3_334)]);
    }

    #[test]
    fn r_trx_16_template_in_amounts_must_match_the_amount() {
        let template = [SplitShare::Amount(m(-3_000)), SplitShare::Amount(m(-1_000))];
        assert_eq!(apply_split_template(m(-4_000), &template).unwrap(), vec![m(-3_000), m(-1_000)]);
        assert_eq!(
            apply_split_template(m(-5_000), &template),
            Err(DomainError::SplitSumMismatch { expected: m(-5_000), actual: m(-4_000) })
        );
    }

    #[test]
    fn r_trx_16_template_is_all_percentages_totalling_100_or_all_amounts() {
        assert_eq!(
            check_split_template(&[SplitShare::BasisPoints(5_000), SplitShare::BasisPoints(4_000)]),
            Err(DomainError::InvalidSplitTemplate)
        );
        assert_eq!(
            check_split_template(&[SplitShare::BasisPoints(10_000), SplitShare::Amount(m(-100))]),
            Err(DomainError::InvalidSplitTemplate)
        );
        assert_eq!(
            check_split_template(&[SplitShare::BasisPoints(0), SplitShare::BasisPoints(10_000)]),
            Err(DomainError::InvalidSplitTemplate)
        );
        assert_eq!(check_split_template(&[]), Err(DomainError::NoSplit));
    }

    #[test]
    fn r_trx_16_a_share_rounded_to_zero_is_refused() {
        let template = [SplitShare::BasisPoints(5_000), SplitShare::BasisPoints(5_000)];
        assert_eq!(apply_split_template(m(-1), &template), Err(DomainError::SplitAmountZero));
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
