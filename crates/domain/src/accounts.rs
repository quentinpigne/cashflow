//! Comptes et établissements (spec 01).

pub mod iban;

use serde::{Deserialize, Serialize};

use crate::shared::{Date, DomainError, Money};

/// Famille d'un compte : fixe son comportement dans l'application (spec 01).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountFamily {
    Cash,
    FixedSavings,
    VariableSavings,
    Liability,
    /// Bien possédé (logement, véhicule), valorisé comme l'épargne variable (R-CPT-14, ADR-0019).
    Property,
}

impl AccountFamily {
    pub const ALL: [AccountFamily; 5] = [
        AccountFamily::Cash,
        AccountFamily::FixedSavings,
        AccountFamily::VariableSavings,
        AccountFamily::Property,
        AccountFamily::Liability,
    ];

    /// Le solde se calcule à partir des transactions (R-CPT-4) ; sinon c'est une valeur (R-CPT-6).
    pub fn has_computed_balance(self) -> bool {
        matches!(self, AccountFamily::Cash | AccountFamily::FixedSavings)
    }
}

macro_rules! account_kinds {
    ($($kind:ident => $family:ident),* $(,)?) => {
        /// Nature d'un compte. La famille en découle (R-CPT-1).
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum AccountKind { $($kind),* }

        impl AccountKind {
            pub const ALL: &'static [AccountKind] = &[$(AccountKind::$kind),*];

            pub fn family(self) -> AccountFamily {
                match self { $(AccountKind::$kind => AccountFamily::$family),* }
            }
        }
    };
}

account_kinds! {
    Checking => Cash,
    CashWallet => Cash,
    OtherCash => Cash,
    LivretA => FixedSavings,
    Ldds => FixedSavings,
    Lep => FixedSavings,
    LivretJeune => FixedSavings,
    Cel => FixedSavings,
    Pel => FixedSavings,
    TermDeposit => FixedSavings,
    SavingsOther => FixedSavings,
    LifeInsurance => VariableSavings,
    Pea => VariableSavings,
    PeaPme => VariableSavings,
    Per => VariableSavings,
    Securities => VariableSavings,
    EmployeeSavings => VariableSavings,
    Crypto => VariableSavings,
    VariableOther => VariableSavings,
    Mortgage => Liability,
    ConsumerLoan => Liability,
    LoanOther => Liability,
    RealEstate => Property,
    Vehicle => Property,
    PropertyOther => Property,
}

impl AccountKind {
    /// Compté par défaut dans l'épargne de précaution (R-CPT-13).
    pub fn default_in_emergency_fund(self) -> bool {
        self.family() == AccountFamily::Cash
            || matches!(self, AccountKind::LivretA | AccountKind::Ldds | AccountKind::Lep | AccountKind::LivretJeune)
    }

    /// Compté par défaut dans les budgets (R-CPT-11, R-BUD-1).
    pub fn default_in_budget(self) -> bool {
        self.family() == AccountFamily::Cash
    }

    /// Les drapeaux qui ne peuvent pas être activés pour cette nature.
    pub fn allows_emergency_fund(self) -> bool {
        !matches!(self.family(), AccountFamily::Liability | AccountFamily::Property)
    }

    pub fn allows_budget(self) -> bool {
        !matches!(self.family(), AccountFamily::VariableSavings | AccountFamily::Liability | AccountFamily::Property)
    }

    /// Conditions d'épargne préremplies d'un compte à taux déterminé (R-CPT-9).
    pub fn fixed_savings_defaults(self) -> Option<FixedSavingsDefaults> {
        use AccountKind::*;
        use Capitalization as C;
        use InterestMethod as M;
        let (method, capitalization, schedule, cap_euros) = match self {
            LivretA => (M::Fortnightly, C::Annual, ScheduleChoice::Shared(SharedSchedule::LivretA), Some(22_950)),
            Ldds => (M::Fortnightly, C::Annual, ScheduleChoice::Shared(SharedSchedule::Ldds), Some(12_000)),
            Lep => (M::Fortnightly, C::Annual, ScheduleChoice::Shared(SharedSchedule::Lep), Some(10_000)),
            // Taux fixé par chaque banque : barème propre au compte (R-CPT-9).
            LivretJeune => (M::Fortnightly, C::Annual, ScheduleChoice::Own, Some(1_600)),
            Cel => (M::Fortnightly, C::Annual, ScheduleChoice::Own, Some(15_300)),
            Pel => (M::Monthly, C::Annual, ScheduleChoice::Own, Some(61_200)),
            TermDeposit => (M::Daily, C::AtMaturity, ScheduleChoice::Own, None),
            SavingsOther => (M::Fortnightly, C::Annual, ScheduleChoice::Own, None),
            _ => return None,
        };
        // Plafonds réglementaires, en euros : convertis en centimes ici (devise EUR de ces produits).
        let deposit_cap = cap_euros.map(|euros: i64| Money::from_minor(euros * 100));
        Some(FixedSavingsDefaults { interest_method: method, capitalization, schedule, deposit_cap })
    }
}

/// Méthode de calcul des intérêts (spec 06).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterestMethod {
    Fortnightly,
    Daily,
    Monthly,
}

/// Rythme de capitalisation des intérêts (spec 06).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Capitalization {
    Annual,
    Monthly,
    AtMaturity,
}

/// Barèmes réglementés partagés entre comptes (R-EPA-11). Leurs taux sont des données initiales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedSchedule {
    LivretA,
    Ldds,
    Lep,
}

impl SharedSchedule {
    /// Nom du barème dans les données initiales.
    pub fn name(self) -> &'static str {
        match self {
            SharedSchedule::LivretA => "Livret A (réglementé)",
            SharedSchedule::Ldds => "LDDS (réglementé)",
            SharedSchedule::Lep => "LEP (réglementé)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleChoice {
    Shared(SharedSchedule),
    /// Un barème propre au compte, créé vide avec lui.
    Own,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedSavingsDefaults {
    pub interest_method: InterestMethod,
    pub capitalization: Capitalization,
    pub schedule: ScheduleChoice,
    pub deposit_cap: Option<Money>,
}

/// Période pendant laquelle un compte peut porter des transactions (R-CPT-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountPeriod {
    pub opening: Date,
    pub closed_on: Option<Date>,
}

impl AccountPeriod {
    pub fn contains(&self, date: Date) -> bool {
        date >= self.opening && self.closed_on.is_none_or(|closed| date <= closed)
    }

    pub fn check(&self, date: Date) -> Result<(), DomainError> {
        if self.contains(date) {
            Ok(())
        } else {
            Err(DomainError::DateOutsideAccountPeriod { opening: self.opening, closed_on: self.closed_on })
        }
    }
}

/// Vérifie qu'un compte peut être clôturé à `closed_on` (R-CPT-5).
///
/// `balance` est le solde calculé à la date de clôture ; un solde non nul exige une confirmation.
pub fn check_closing(opening: Date, closed_on: Date, balance: Money, confirmed: bool) -> Result<(), DomainError> {
    if closed_on < opening {
        return Err(DomainError::ClosingBeforeOpening);
    }
    if !balance.is_zero() && !confirmed {
        return Err(DomainError::ClosingBalanceNotZero { balance });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn r_cpt_1_family_follows_kind() {
        assert_eq!(AccountKind::Checking.family(), AccountFamily::Cash);
        assert_eq!(AccountKind::LivretA.family(), AccountFamily::FixedSavings);
        assert_eq!(AccountKind::Pea.family(), AccountFamily::VariableSavings);
        assert_eq!(AccountKind::Mortgage.family(), AccountFamily::Liability);
        assert_eq!(AccountKind::RealEstate.family(), AccountFamily::Property);
        assert_eq!(AccountKind::ALL.len(), 25);
    }

    #[test]
    fn r_cpt_13_emergency_fund_defaults() {
        assert!(AccountKind::Checking.default_in_emergency_fund());
        assert!(AccountKind::LivretJeune.default_in_emergency_fund());
        assert!(!AccountKind::Pel.default_in_emergency_fund());
        assert!(!AccountKind::Mortgage.allows_emergency_fund());
    }

    #[test]
    fn r_cpt_11_budget_defaults() {
        assert!(AccountKind::Checking.default_in_budget());
        assert!(!AccountKind::LivretA.default_in_budget());
        assert!(!AccountKind::Pea.allows_budget());
    }

    #[test]
    fn r_cpt_14_property_is_outside_emergency_fund_and_budgets() {
        for kind in [AccountKind::RealEstate, AccountKind::Vehicle, AccountKind::PropertyOther] {
            assert!(!kind.default_in_emergency_fund() && !kind.allows_emergency_fund());
            assert!(!kind.default_in_budget() && !kind.allows_budget());
            assert!(!kind.family().has_computed_balance());
        }
    }

    #[test]
    fn r_cpt_9_regulated_accounts_are_prefilled() {
        let livret_a = AccountKind::LivretA.fixed_savings_defaults().unwrap();
        assert_eq!(livret_a.interest_method, InterestMethod::Fortnightly);
        assert_eq!(livret_a.capitalization, Capitalization::Annual);
        assert_eq!(livret_a.schedule, ScheduleChoice::Shared(SharedSchedule::LivretA));
        assert_eq!(AccountKind::LivretJeune.fixed_savings_defaults().unwrap().schedule, ScheduleChoice::Own);
        assert!(AccountKind::Checking.fixed_savings_defaults().is_none());
        for kind in AccountKind::ALL {
            assert_eq!(kind.fixed_savings_defaults().is_some(), kind.family() == AccountFamily::FixedSavings);
        }
    }

    #[test]
    fn r_cpt_3_dates_within_account_period() {
        let period = AccountPeriod { opening: date("2026-01-01"), closed_on: Some(date("2026-06-30")) };
        assert!(period.check(date("2026-01-01")).is_ok());
        assert!(period.check(date("2026-06-30")).is_ok());
        assert!(period.check(date("2025-12-31")).is_err());
        assert!(period.check(date("2026-07-01")).is_err());
    }

    #[test]
    fn r_cpt_5_closing_requires_zero_balance_or_confirmation() {
        let opening = date("2026-01-01");
        assert!(check_closing(opening, date("2026-03-01"), Money::ZERO, false).is_ok());
        assert_eq!(
            check_closing(opening, date("2026-03-01"), Money::from_minor(500), false),
            Err(DomainError::ClosingBalanceNotZero { balance: Money::from_minor(500) })
        );
        assert!(check_closing(opening, date("2026-03-01"), Money::from_minor(500), true).is_ok());
        assert_eq!(
            check_closing(opening, date("2025-12-01"), Money::ZERO, true),
            Err(DomainError::ClosingBeforeOpening)
        );
    }
}
