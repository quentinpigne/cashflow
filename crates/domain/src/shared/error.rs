//! Erreurs métier typées. Chacune porte un code stable (ADR-0016) et, si elle existe, la règle violée.

use super::{Date, Money};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    // ----- Types partagés
    #[error("code de devise invalide : « {0} »")]
    InvalidCurrency(String),
    #[error("montant hors limites")]
    AmountOutOfRange,

    // ----- Comptes (spec 01)
    #[error("la nature d'un compte qui a des transactions ou des valorisations ne peut plus changer")]
    AccountKindLocked,
    #[error("IBAN invalide")]
    InvalidIban,
    #[error("la date doit être comprise entre l'ouverture ({opening}) et la clôture du compte")]
    DateOutsideAccountPeriod { opening: Date, closed_on: Option<Date> },
    #[error("la date de clôture précède la date d'ouverture")]
    ClosingBeforeOpening,
    #[error("le solde à la date de clôture n'est pas nul ({balance}) : confirmation requise")]
    ClosingBalanceNotZero { balance: Money },
    #[error("un compte qui a des transactions ne peut pas être supprimé, seulement clôturé")]
    AccountNotEmpty,
    #[error("le compte est clôturé")]
    AccountClosed,
    #[error("ce champ n'est pas modifiable après la création du compte")]
    AccountFieldLocked,

    // ----- Transactions (spec 02)
    #[error("le montant ne peut pas être nul")]
    AmountZero,
    #[error("une transaction a au moins une ventilation")]
    NoSplit,
    #[error("une ventilation ne peut pas avoir un montant nul")]
    SplitAmountZero,
    #[error("la somme des ventilations ({actual}) diffère du montant de la transaction ({expected})")]
    SplitSumMismatch { expected: Money, actual: Money },
    #[error("les pourcentages de ventilation sont invalides")]
    InvalidAllocation,
    #[error("la transaction est rapprochée : déverrouiller le point de contrôle pour la modifier")]
    TransactionLocked,
    #[error("le libellé bancaire ne peut pas être modifié")]
    BankLabelImmutable,

    // ----- Catégories
    #[error("une catégorie a au plus deux niveaux")]
    CategoryDepthExceeded,
    #[error("une sous-catégorie a la même nature que sa catégorie parente")]
    CategoryKindMismatch,
    #[error("une catégorie utilisée ne peut pas être supprimée : l'archiver ou la fusionner")]
    CategoryInUse,
    #[error("fusion impossible entre ces deux catégories")]
    InvalidCategoryMerge,
    #[error("une catégorie de ce nom existe déjà à cet endroit")]
    CategoryNameTaken,

    // ----- Virements internes
    #[error("un virement relie deux comptes différents")]
    TransferSameAccount,
    #[error("un virement relie deux comptes de même devise")]
    TransferCurrencyMismatch,
    #[error("les deux jambes d'un virement ont des montants opposés")]
    TransferAmountsNotOpposite,
    #[error("une jambe de virement n'a qu'une ventilation, sans catégorie")]
    TransferLegCategorized,
    #[error("cette transaction fait déjà partie d'un virement")]
    AlreadyTransferLeg,
}

impl DomainError {
    /// Code stable exposé par l'API (`application/problem+json`, champ `code`).
    pub fn code(&self) -> &'static str {
        use DomainError::*;
        match self {
            InvalidCurrency(_) => "INVALID_CURRENCY",
            AmountOutOfRange => "AMOUNT_OUT_OF_RANGE",
            AccountKindLocked => "ACCOUNT_KIND_LOCKED",
            InvalidIban => "INVALID_IBAN",
            DateOutsideAccountPeriod { .. } => "DATE_OUTSIDE_ACCOUNT_PERIOD",
            ClosingBeforeOpening => "CLOSING_BEFORE_OPENING",
            ClosingBalanceNotZero { .. } => "CLOSING_BALANCE_NOT_ZERO",
            AccountNotEmpty => "ACCOUNT_NOT_EMPTY",
            AccountClosed => "ACCOUNT_CLOSED",
            AccountFieldLocked => "ACCOUNT_FIELD_LOCKED",
            AmountZero => "AMOUNT_ZERO",
            NoSplit => "NO_SPLIT",
            SplitAmountZero => "SPLIT_AMOUNT_ZERO",
            SplitSumMismatch { .. } => "SPLIT_SUM_MISMATCH",
            InvalidAllocation => "INVALID_ALLOCATION",
            TransactionLocked => "TRANSACTION_LOCKED",
            BankLabelImmutable => "BANK_LABEL_IMMUTABLE",
            CategoryDepthExceeded => "CATEGORY_DEPTH_EXCEEDED",
            CategoryKindMismatch => "CATEGORY_KIND_MISMATCH",
            CategoryInUse => "CATEGORY_IN_USE",
            InvalidCategoryMerge => "INVALID_CATEGORY_MERGE",
            CategoryNameTaken => "CATEGORY_NAME_TAKEN",
            TransferSameAccount => "TRANSFER_SAME_ACCOUNT",
            TransferCurrencyMismatch => "TRANSFER_CURRENCY_MISMATCH",
            TransferAmountsNotOpposite => "TRANSFER_AMOUNTS_NOT_OPPOSITE",
            TransferLegCategorized => "TRANSFER_LEG_CATEGORIZED",
            AlreadyTransferLeg => "ALREADY_TRANSFER_LEG",
        }
    }

    /// Règle métier violée, si l'erreur en fait respecter une.
    pub fn rule(&self) -> Option<&'static str> {
        use DomainError::*;
        Some(match self {
            InvalidCurrency(_) | AmountOutOfRange | AccountFieldLocked => return None,
            AccountKindLocked => "R-CPT-1",
            InvalidIban => "R-CPT-8",
            DateOutsideAccountPeriod { .. } | AccountClosed => "R-CPT-3",
            ClosingBeforeOpening | ClosingBalanceNotZero { .. } => "R-CPT-5",
            AccountNotEmpty => "R-CPT-7",
            AmountZero => "R-TRX-1",
            BankLabelImmutable => "R-TRX-2",
            TransactionLocked => "R-TRX-3",
            NoSplit | SplitSumMismatch { .. } => "R-TRX-10",
            SplitAmountZero => "R-TRX-11",
            InvalidAllocation => "R-TRX-14",
            CategoryDepthExceeded | CategoryKindMismatch | CategoryNameTaken => "R-TRX-20",
            CategoryInUse | InvalidCategoryMerge => "R-TRX-22",
            TransferSameAccount | TransferCurrencyMismatch | TransferAmountsNotOpposite | AlreadyTransferLeg => {
                "R-TRX-30"
            }
            TransferLegCategorized => "R-TRX-32",
        })
    }
}
