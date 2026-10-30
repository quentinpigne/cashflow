//! Erreurs métier typées. Chacune porte un code stable (ADR-0016) et, si elle existe, la règle violée.

use super::ids::DocumentId;
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
    #[error(
        "conditions de prêt invalides : capital positif, taux et assurance positifs ou nuls, durée de 1 à 600 mois"
    )]
    InvalidLoanTerms,
    #[error("bien invalide : prix d'achat positif, vente postérieure à l'achat, prix de vente seulement si vendu")]
    InvalidAsset,
    #[error("une estimation est positive et datée entre l'achat et la vente du bien")]
    InvalidAssetValuation,
    #[error("prêt invalide : capital positif, première échéance après le déblocage, soldé après le déblocage")]
    InvalidLoan,
    #[error("une ventilation a au plus un rattachement : une catégorie, un prêt ou un bien")]
    SplitAttachmentConflict,
    #[error("le montant ne correspond pas à l'échéance, à 1 € près")]
    InstallmentMismatch,
    #[error("cette échéance est déjà payée")]
    InstallmentAlreadyPaid,
    #[error(
        "une mensualité ventilée selon le tableau d'amortissement ne change ni de montant ni de ventilation : annuler d'abord sa ventilation"
    )]
    InstallmentPaymentLocked,
    #[error("ce champ n'est pas modifiable après la création du compte")]
    AccountFieldLocked,

    // ----- Salaires (spec 10)
    #[error("emploi invalide : fin postérieure au début, mois des augmentations de 1 à 12")]
    InvalidEmployment,
    #[error("rémunération invalide : un premier du mois pendant l'emploi, brut et net positifs")]
    InvalidSalary,
    #[error("prime invalide : versée pendant l'emploi, net positif, brut positif s'il est connu")]
    InvalidBonus,
    #[error("le mois de paie d'une fiche tombe pendant l'emploi")]
    InvalidPayslipMonth,

    // ----- Crédit (spec 13)
    #[error("simulation invalide : capital positif, durée de 1 à 600 mois, taux et frais positifs ou nuls")]
    InvalidSimulation,
    #[error("la mensualité visée ne permet pas ce crédit : elle doit couvrir l'assurance, sur 600 mois au plus")]
    TargetPaymentTooLow,
    #[error("plan de financement invalide : montants positifs ou nuls, apport au plus égal au coût du projet")]
    InvalidFinancingPlan,
    #[error("cette simulation a déjà été transformée en prêt")]
    SimulationAlreadyConverted,
    #[error("seule une démarche avec une offre reçue peut être acceptée")]
    OfferNotAcceptable,

    // ----- Documents (spec 12, ADR-0024)
    #[error("le fichier est vide")]
    EmptyDocument,
    #[error("le fichier dépasse 10 Mo")]
    DocumentTooLarge,
    #[error("type de fichier non accepté (PDF, JPEG ou PNG)")]
    UnsupportedDocumentType,
    #[error("un relevé de compte est un fichier PDF")]
    StatementNotPdf,
    #[error("ce fichier est déjà archivé ({title})")]
    DocumentAlreadyArchived { id: DocumentId, title: String },
    #[error("le titre d'un document fait de 1 à 120 caractères")]
    InvalidDocumentTitle,
    #[error("le début de la période précède sa fin")]
    InvalidDocumentPeriod,
    #[error("un relevé ou une fiche de paie a un mois (un premier du mois), les autres documents n'en ont pas")]
    InvalidDocumentMonth,
    #[error("un relevé est rattaché à un compte, une fiche de paie à un emploi")]
    DocumentOwnerRequired,
    #[error("un relevé est rattaché à un seul compte, une fiche de paie à un seul emploi")]
    DocumentOwnerNotUnique,

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

    // ----- Tiers (spec 11)
    #[error("un tiers de ce nom existe déjà")]
    PayeeNameTaken,
    #[error("un groupe de tiers de ce nom existe déjà")]
    PayeeGroupNameTaken,
    #[error("un tiers utilisé ne peut pas être supprimé : l'archiver ou le fusionner")]
    PayeeInUse,
    #[error("un tiers se fusionne dans un autre tiers")]
    InvalidPayeeMerge,
    #[error("une jambe de virement interne n'a pas de tiers")]
    PayeeOnTransferLeg,
    #[error("une adresse a au moins une ville et un pays")]
    AddressIncomplete,
    #[error("code pays invalide : deux lettres attendues (ISO 3166-1)")]
    InvalidCountryCode,
    #[error("code postal invalide : cinq chiffres attendus en France")]
    InvalidPostalCode,
    #[error("coordonnées invalides : latitude entre -90 et 90, longitude entre -180 et 180, avec une adresse")]
    InvalidCoordinates,

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

    // ----- Remboursements de frais
    #[error(
        "un remboursement est une part positive d'une entrée, sur une catégorie de dépense, qui vise une autre transaction de dépense"
    )]
    InvalidRefund,
    #[error("le remboursé d'une dépense ne dépasse pas son montant")]
    RefundExceedsExpense,
    #[error("le remboursement attendu d'une dépense est positif et ne dépasse pas son montant")]
    InvalidExpectedRefund,

    // ----- Rapprochement (spec 04)
    #[error("point de contrôle introuvable")]
    CheckpointNotFound,
    #[error("ce point de contrôle est déjà rapproché")]
    CheckpointAlreadyReconciled,
    #[error("l'écart vaut {gap} : on ne rapproche qu'un point de contrôle à écart nul")]
    CheckpointGapNotZero { gap: Money },
    #[error("la période est rapprochée jusqu'au {until} : déverrouiller ce point de contrôle pour la modifier")]
    PeriodReconciled { until: Date },
    #[error("ce type de compte n'a pas de point de contrôle : il se suit par valorisations")]
    NoCheckpointForFamily,
    #[error("un point de contrôle est rapproché : le solde d'ouverture ne peut plus changer")]
    OpeningBalanceLocked,

    // ----- Budgets (spec 05)
    #[error("mois attendu au format AAAA-MM")]
    InvalidMonth,
    #[error("le montant prévu doit être positif")]
    BudgetAmountNotPositive,
    #[error("la fin de validité précède le début")]
    BudgetPeriodInverted,
    #[error(
        "ligne de budget incohérente : une catégorie pour une dépense ou un revenu, un compte d'épargne pour un objectif"
    )]
    InvalidBudgetLine,
    #[error("une ligne de revenu n'a pas de report")]
    IncomeRollover,
    #[error("cette catégorie a déjà une ligne en vigueur sur cette période")]
    BudgetLineOverlap,
    #[error("on ne rattache de nouvelles ventilations qu'à un projet en cours")]
    ProjectClosed,
    #[error("un projet a soit un montant global, soit des postes, chacun nommé une seule fois et de montant positif")]
    InvalidProjectPlan,
    #[error("une catégorie ne propose qu'un seul poste du projet")]
    ProjectLineCategoryTaken,
    #[error("ce poste n'appartient pas au projet de la ventilation")]
    ProjectLineMismatch,

    // ----- Import (spec 03)
    #[error("une règle doit avoir au moins une condition")]
    RuleWithoutCondition,
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
            InvalidLoanTerms => "INVALID_LOAN_TERMS",
            InvalidAsset => "INVALID_ASSET",
            InvalidAssetValuation => "INVALID_ASSET_VALUATION",
            InvalidLoan => "INVALID_LOAN",
            SplitAttachmentConflict => "SPLIT_ATTACHMENT_CONFLICT",
            InstallmentMismatch => "INSTALLMENT_MISMATCH",
            InstallmentAlreadyPaid => "INSTALLMENT_ALREADY_PAID",
            InstallmentPaymentLocked => "INSTALLMENT_PAYMENT_LOCKED",
            AccountFieldLocked => "ACCOUNT_FIELD_LOCKED",
            InvalidEmployment => "INVALID_EMPLOYMENT",
            InvalidSalary => "INVALID_SALARY",
            InvalidBonus => "INVALID_BONUS",
            InvalidPayslipMonth => "INVALID_PAYSLIP_MONTH",
            InvalidSimulation => "INVALID_SIMULATION",
            TargetPaymentTooLow => "TARGET_PAYMENT_TOO_LOW",
            InvalidFinancingPlan => "INVALID_FINANCING_PLAN",
            SimulationAlreadyConverted => "SIMULATION_ALREADY_CONVERTED",
            OfferNotAcceptable => "OFFER_NOT_ACCEPTABLE",
            EmptyDocument => "EMPTY_DOCUMENT",
            DocumentTooLarge => "DOCUMENT_TOO_LARGE",
            UnsupportedDocumentType => "UNSUPPORTED_DOCUMENT_TYPE",
            StatementNotPdf => "STATEMENT_NOT_PDF",
            DocumentAlreadyArchived { .. } => "DOCUMENT_ALREADY_ARCHIVED",
            InvalidDocumentTitle => "INVALID_DOCUMENT_TITLE",
            InvalidDocumentPeriod => "INVALID_DOCUMENT_PERIOD",
            InvalidDocumentMonth => "INVALID_DOCUMENT_MONTH",
            DocumentOwnerRequired => "DOCUMENT_OWNER_REQUIRED",
            DocumentOwnerNotUnique => "DOCUMENT_OWNER_NOT_UNIQUE",
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
            PayeeNameTaken => "PAYEE_NAME_TAKEN",
            PayeeGroupNameTaken => "PAYEE_GROUP_NAME_TAKEN",
            PayeeInUse => "PAYEE_IN_USE",
            InvalidPayeeMerge => "INVALID_PAYEE_MERGE",
            PayeeOnTransferLeg => "PAYEE_ON_TRANSFER_LEG",
            AddressIncomplete => "ADDRESS_INCOMPLETE",
            InvalidCountryCode => "INVALID_COUNTRY_CODE",
            InvalidPostalCode => "INVALID_POSTAL_CODE",
            InvalidCoordinates => "INVALID_COORDINATES",
            TransferSameAccount => "TRANSFER_SAME_ACCOUNT",
            TransferCurrencyMismatch => "TRANSFER_CURRENCY_MISMATCH",
            TransferAmountsNotOpposite => "TRANSFER_AMOUNTS_NOT_OPPOSITE",
            TransferLegCategorized => "TRANSFER_LEG_CATEGORIZED",
            AlreadyTransferLeg => "ALREADY_TRANSFER_LEG",
            InvalidRefund => "INVALID_REFUND",
            RefundExceedsExpense => "REFUND_EXCEEDS_EXPENSE",
            InvalidExpectedRefund => "INVALID_EXPECTED_REFUND",
            CheckpointNotFound => "CHECKPOINT_NOT_FOUND",
            CheckpointAlreadyReconciled => "CHECKPOINT_ALREADY_RECONCILED",
            CheckpointGapNotZero { .. } => "CHECKPOINT_GAP_NOT_ZERO",
            PeriodReconciled { .. } => "PERIOD_RECONCILED",
            NoCheckpointForFamily => "NO_CHECKPOINT_FOR_FAMILY",
            OpeningBalanceLocked => "OPENING_BALANCE_LOCKED",
            InvalidMonth => "INVALID_MONTH",
            BudgetAmountNotPositive => "BUDGET_AMOUNT_NOT_POSITIVE",
            BudgetPeriodInverted => "BUDGET_PERIOD_INVERTED",
            InvalidBudgetLine => "INVALID_BUDGET_LINE",
            IncomeRollover => "INCOME_ROLLOVER",
            BudgetLineOverlap => "BUDGET_LINE_OVERLAP",
            ProjectClosed => "PROJECT_CLOSED",
            InvalidProjectPlan => "INVALID_PROJECT_PLAN",
            ProjectLineCategoryTaken => "PROJECT_LINE_CATEGORY_TAKEN",
            ProjectLineMismatch => "PROJECT_LINE_MISMATCH",
            RuleWithoutCondition => "RULE_WITHOUT_CONDITION",
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
            InvalidLoanTerms => "R-PRE-2",
            InvalidLoan => "R-PRE-1",
            InvalidAsset => "R-BIE-1",
            InvalidAssetValuation => "R-BIE-3",
            SplitAttachmentConflict => "R-TRX-37",
            InstallmentMismatch | InstallmentAlreadyPaid => "R-PRE-6",
            InstallmentPaymentLocked => "R-PRE-8",
            InvalidEmployment => "R-SAL-1",
            InvalidSalary => "R-SAL-2",
            InvalidBonus => "R-SAL-5",
            InvalidPayslipMonth => "R-SAL-15",
            InvalidSimulation => "R-CRE-1",
            TargetPaymentTooLow => "R-CRE-5",
            InvalidFinancingPlan => "R-CRE-11",
            SimulationAlreadyConverted => "R-CRE-7",
            OfferNotAcceptable => "R-CRE-22",
            EmptyDocument | DocumentTooLarge | UnsupportedDocumentType | StatementNotPdf => "R-DOC-1",
            DocumentAlreadyArchived { .. } => "R-DOC-3",
            InvalidDocumentTitle => "R-DOC-4",
            InvalidDocumentPeriod => "R-DOC-6",
            InvalidDocumentMonth => "R-DOC-7",
            DocumentOwnerRequired | DocumentOwnerNotUnique => "R-DOC-10",
            AmountZero => "R-TRX-1",
            BankLabelImmutable => "R-TRX-2",
            TransactionLocked => "R-TRX-3",
            NoSplit | SplitSumMismatch { .. } => "R-TRX-10",
            SplitAmountZero => "R-TRX-11",
            InvalidAllocation => "R-TRX-14",
            CategoryDepthExceeded | CategoryKindMismatch | CategoryNameTaken => "R-TRX-20",
            CategoryInUse | InvalidCategoryMerge => "R-TRX-22",
            PayeeNameTaken => "R-TIE-1",
            PayeeGroupNameTaken => "R-TIE-2",
            PayeeInUse => "R-TIE-5",
            InvalidPayeeMerge => "R-TIE-7",
            PayeeOnTransferLeg => "R-TIE-11",
            AddressIncomplete | InvalidCountryCode | InvalidPostalCode => "R-TIE-23",
            InvalidCoordinates => "R-TIE-25",
            TransferSameAccount | TransferCurrencyMismatch | TransferAmountsNotOpposite | AlreadyTransferLeg => {
                "R-TRX-30"
            }
            TransferLegCategorized => "R-TRX-32",
            InvalidRefund => "R-TRX-50",
            RefundExceedsExpense => "R-TRX-52",
            InvalidExpectedRefund => "R-TRX-53",
            CheckpointNotFound => return None,
            CheckpointAlreadyReconciled | CheckpointGapNotZero { .. } => "R-RAP-4",
            PeriodReconciled { .. } => "R-RAP-7",
            NoCheckpointForFamily => "R-RAP-10",
            OpeningBalanceLocked => "R-RAP-8",
            InvalidMonth | BudgetPeriodInverted => return None,
            BudgetAmountNotPositive | InvalidBudgetLine => "R-BUD-10",
            IncomeRollover => "R-BUD-19",
            BudgetLineOverlap => "R-BUD-11",
            ProjectClosed => "R-BUD-24",
            InvalidProjectPlan => "R-BUD-26",
            ProjectLineCategoryTaken | ProjectLineMismatch => "R-BUD-27",
            RuleWithoutCondition => "R-IMP-12",
        })
    }
}
