//! Transactions, ventilations, catégories, virements et soldes (spec 02).

pub mod balance;
pub mod category;
pub mod refund;
pub mod split;
pub mod transfer;

use serde::{Deserialize, Serialize};

use crate::Date;
use crate::budgeting::Month;
use crate::shared::{DomainError, Money};

/// État de pointage d'une transaction (R-TRX-5).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClearingStatus {
    Pending,
    Cleared,
    Reconciled,
}

/// Origine d'une transaction.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionOrigin {
    Manual,
    Import,
    Recurring,
    Interest,
}

/// Le montant d'une transaction n'est jamais nul ; son signe porte le sens (R-TRX-1).
pub fn check_amount(amount: Money) -> Result<(), DomainError> {
    if amount.is_zero() { Err(DomainError::AmountZero) } else { Ok(()) }
}

/// Champs qu'une transaction rapprochée ne laisse pas modifier sans déverrouillage (R-TRX-3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BalanceAffectingChange {
    pub amount: bool,
    pub account: bool,
    pub balance_date: bool,
}

impl BalanceAffectingChange {
    pub fn any(self) -> bool {
        self.amount || self.account || self.balance_date
    }
}

/// Vérifie qu'une modification est permise selon l'état de pointage (R-TRX-3).
pub fn check_modification(status: ClearingStatus, change: BalanceAffectingChange) -> Result<(), DomainError> {
    if status == ClearingStatus::Reconciled && change.any() { Err(DomainError::TransactionLocked) } else { Ok(()) }
}

/// Mois sur lequel une transaction compte (R-TRX-57) : son mois d'analyse s'il est choisi, sinon le mois de sa date.
pub fn analysis_month(date: Date, chosen: Option<Month>) -> Month {
    chosen.unwrap_or(Month::of(date))
}

/// Décalage du mois d'analyse par une règle (R-IMP-29) : de −12 à 12 mois, non nul.
pub fn check_month_shift(shift: i32) -> Result<(), DomainError> {
    if shift == 0 || shift.abs() > 12 { Err(DomainError::InvalidMonthShift) } else { Ok(()) }
}

/// Mois d'analyse donné par une règle (R-IMP-29) : le mois de la date décalé de `shift` mois.
pub fn shifted_month(date: Date, shift: i32) -> Result<Month, DomainError> {
    check_month_shift(shift)?;
    let mut month = Month::of(date);
    for _ in 0..shift.abs() {
        month = if shift > 0 { month.next() } else { month.previous() };
    }
    Ok(month)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn r_trx_57_analysis_month_defaults_to_the_date() {
        assert_eq!(analysis_month(d("2026-10-28"), None).to_string(), "2026-10");
        let november: Month = "2026-11".parse().unwrap();
        assert_eq!(analysis_month(d("2026-10-28"), Some(november)), november);
    }

    #[test]
    fn r_imp_29_shifted_month() {
        assert_eq!(shifted_month(d("2026-12-28"), 1).unwrap().to_string(), "2027-01");
        assert_eq!(shifted_month(d("2026-01-02"), -1).unwrap().to_string(), "2025-12");
        assert_eq!(shifted_month(d("2026-01-02"), 0), Err(DomainError::InvalidMonthShift));
        assert_eq!(shifted_month(d("2026-01-02"), 13), Err(DomainError::InvalidMonthShift));
    }

    #[test]
    fn r_trx_1_amount_is_never_zero() {
        assert_eq!(check_amount(Money::ZERO), Err(DomainError::AmountZero));
        assert!(check_amount(Money::from_minor(-1)).is_ok());
    }

    #[test]
    fn r_trx_3_reconciled_transactions_are_locked() {
        let amount_change = BalanceAffectingChange { amount: true, ..Default::default() };
        assert_eq!(check_modification(ClearingStatus::Reconciled, amount_change), Err(DomainError::TransactionLocked));
        assert!(check_modification(ClearingStatus::Reconciled, BalanceAffectingChange::default()).is_ok());
        assert!(check_modification(ClearingStatus::Cleared, amount_change).is_ok());
    }
}
