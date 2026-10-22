//! Virements internes (ADR-0004, R-TRX-30 à R-TRX-36).

use crate::accounts::AccountFamily;
use crate::ledger::ClearingStatus;
use crate::shared::ids::{AccountId, CategoryId};
use crate::shared::{Currency, Date, DomainError, Money};

/// Une jambe de virement, telle que vue par les règles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferLeg {
    pub account_id: AccountId,
    pub currency: Currency,
    pub amount: Money,
    pub balance_date: Date,
    /// Catégories des ventilations de la jambe.
    pub split_categories: Vec<Option<CategoryId>>,
}

/// Vérifie qu'une jambe sortante et une jambe entrante forment un virement (R-TRX-30, R-TRX-32).
pub fn check_transfer(outflow: &TransferLeg, inflow: &TransferLeg) -> Result<(), DomainError> {
    if outflow.account_id == inflow.account_id {
        return Err(DomainError::TransferSameAccount);
    }
    if outflow.currency != inflow.currency {
        return Err(DomainError::TransferCurrencyMismatch);
    }
    if !outflow.amount.is_negative() || outflow.amount != -inflow.amount {
        return Err(DomainError::TransferAmountsNotOpposite);
    }
    for leg in [outflow, inflow] {
        if leg.split_categories.len() != 1 || leg.split_categories[0].is_some() {
            return Err(DomainError::TransferLegCategorized);
        }
    }
    Ok(())
}

/// Un écart de plus de 7 jours entre les deux jambes est signalé (R-TRX-31).
pub fn dates_far_apart(outflow: Date, inflow: Date) -> bool {
    (inflow - outflow).num_days().abs() > 7
}

/// Statut de la jambe opposée créée automatiquement (R-TRX-36).
pub fn opposite_leg_status(counterpart_family: AccountFamily) -> ClearingStatus {
    match counterpart_family {
        AccountFamily::Cash | AccountFamily::FixedSavings => ClearingStatus::Pending,
        AccountFamily::VariableSavings => ClearingStatus::Cleared,
    }
}

/// Une transaction candidate pour former un virement avec `source` (R-TRX-35).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateCheck {
    pub source_amount: Money,
    pub source_date: Date,
    pub candidate_amount: Money,
    pub candidate_date: Date,
    pub tolerance_days: i64,
}

impl CandidateCheck {
    pub fn matches(&self) -> bool {
        self.candidate_amount == -self.source_amount
            && (self.candidate_date - self.source_date).num_days().abs() <= self.tolerance_days
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn leg(account: AccountId, minor: i64, categories: Vec<Option<CategoryId>>) -> TransferLeg {
        TransferLeg {
            account_id: account,
            currency: Currency::EUR,
            amount: Money::from_minor(minor),
            balance_date: d("2026-03-05"),
            split_categories: categories,
        }
    }

    #[test]
    fn r_trx_30_valid_transfer() {
        let (a, b) = (AccountId::new(), AccountId::new());
        assert!(check_transfer(&leg(a, -20_000, vec![None]), &leg(b, 20_000, vec![None])).is_ok());
    }

    #[test]
    fn r_trx_30_rejects_same_account_and_non_opposite_amounts() {
        let (a, b) = (AccountId::new(), AccountId::new());
        assert_eq!(
            check_transfer(&leg(a, -20_000, vec![None]), &leg(a, 20_000, vec![None])),
            Err(DomainError::TransferSameAccount)
        );
        assert_eq!(
            check_transfer(&leg(a, -20_000, vec![None]), &leg(b, 19_999, vec![None])),
            Err(DomainError::TransferAmountsNotOpposite)
        );
        assert_eq!(
            check_transfer(&leg(a, 20_000, vec![None]), &leg(b, -20_000, vec![None])),
            Err(DomainError::TransferAmountsNotOpposite)
        );
    }

    #[test]
    fn r_trx_32_legs_have_one_uncategorized_split() {
        let (a, b) = (AccountId::new(), AccountId::new());
        assert_eq!(
            check_transfer(&leg(a, -20_000, vec![Some(CategoryId::new())]), &leg(b, 20_000, vec![None])),
            Err(DomainError::TransferLegCategorized)
        );
        assert_eq!(
            check_transfer(&leg(a, -20_000, vec![None, None]), &leg(b, 20_000, vec![None])),
            Err(DomainError::TransferLegCategorized)
        );
    }

    #[test]
    fn r_trx_31_flags_legs_more_than_seven_days_apart() {
        assert!(!dates_far_apart(d("2026-03-01"), d("2026-03-08")));
        assert!(dates_far_apart(d("2026-03-01"), d("2026-03-09")));
    }

    #[test]
    fn r_trx_36_opposite_leg_status_depends_on_family() {
        assert_eq!(opposite_leg_status(AccountFamily::Cash), ClearingStatus::Pending);
        assert_eq!(opposite_leg_status(AccountFamily::VariableSavings), ClearingStatus::Cleared);
    }

    #[test]
    fn r_trx_35_candidates_have_opposite_amount_within_tolerance() {
        let check = |date: &str| CandidateCheck {
            source_amount: Money::from_minor(-30_000),
            source_date: d("2026-03-10"),
            candidate_amount: Money::from_minor(30_000),
            candidate_date: d(date),
            tolerance_days: 5,
        };
        assert!(check("2026-03-15").matches());
        assert!(!check("2026-03-16").matches());
    }
}
