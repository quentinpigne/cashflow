//! Identifiants UUIDv7 typés par entité (ADR-0011).

macro_rules! define_id {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema), schema(value_type = String, format = Uuid))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(uuid::Uuid);

        impl $name {
            /// Nouvel identifiant UUIDv7, triable dans l'ordre de création.
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self(uuid::Uuid::now_v7())
            }

            pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
                Self(uuid)
            }

            pub const fn as_uuid(&self) -> &uuid::Uuid {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // Minuscules avec tirets, comme en base (modèle de données).
                self.0.hyphenated().fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                uuid::Uuid::parse_str(s).map(Self)
            }
        }
    )*};
}

define_id!(
    InstitutionId,
    AccountId,
    RateScheduleId,
    RatePeriodId,
    ValuationId,
    CategoryId,
    PayeeId,
    PayeeGroupId,
    TransactionId,
    SplitId,
    TransferId,
    LoanPaymentId,
    AssetId,
    AssetValuationId,
    LoanId,
    LoanBalanceId,
    LoanSimulationId,
    LoanApplicationId,
    LenderRequestId,
    EmploymentId,
    SalaryId,
    SalaryBonusId,
    PayslipId,
    BankStatementId,
    DocumentId,
    RecurringRuleId,
    ImportProfileId,
    ImportBatchId,
    ImportLineId,
    CategorizationRuleId,
    CheckpointId,
    ScenarioId,
    BudgetLineId,
    BudgetOverrideId,
    ProjectId,
    UserId,
    SessionId,
    AccessTokenId,
    NotificationId,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_lowercase_hyphenated_and_ordered() {
        let first = AccountId::new();
        let second = AccountId::new();
        assert_eq!(first.to_string().len(), 36);
        assert_eq!(first.to_string(), first.to_string().to_lowercase());
        assert!(first < second);
        assert_eq!(first.to_string().parse::<AccountId>().unwrap(), first);
    }
}
