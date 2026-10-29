//! Transactions, ventilations, catégories, virements et soldes (spec 02).

pub mod balance;
pub mod category;
pub mod refund;
pub mod split;
pub mod transfer;

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

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
