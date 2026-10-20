//! Indicateurs du tableau de bord (spec 08).

use chrono::{Datelike, Months, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::accounts::AccountFamily;
use crate::ledger::category::CategoryKind;
use crate::shared::{Date, Money, Rate};

/// Valeur d'un compte à une date, et ce qu'il faut savoir pour l'agréger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountValue {
    pub family: AccountFamily,
    pub include_in_net_worth: bool,
    /// Les comptes hors euro sont exclus des agrégats (R-CPT-2).
    pub is_eur: bool,
    pub value: Money,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetWorth {
    pub assets: Money,
    pub liabilities: Money,
    pub total: Money,
}

/// Patrimoine net (R-TDB-1) : actif brut et dettes séparés.
pub fn net_worth(accounts: &[AccountValue]) -> NetWorth {
    let mut result = NetWorth::default();
    for account in accounts.iter().filter(|a| a.include_in_net_worth && a.is_eur) {
        if account.family == AccountFamily::Liability || account.value.is_negative() {
            result.liabilities += account.value;
        } else {
            result.assets += account.value;
        }
    }
    result.total = result.assets + result.liabilities;
    result
}

/// Une ventilation, vue par les indicateurs de flux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitFact {
    /// Date de la transaction (`date`) : les flux sont rattachés au mois par elle (spec 08).
    pub date: Date,
    pub amount: Money,
    /// `None` : non catégorisée.
    pub category_kind: Option<CategoryKind>,
    pub is_transfer: bool,
    pub is_eur: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flows {
    /// R-TDB-3.
    pub income: Money,
    /// R-TDB-4 : positif, remboursements déduits.
    pub expenses: Money,
    /// R-TDB-5 : entrées et sorties non catégorisées, hors virements.
    pub uncategorized_in: Money,
    pub uncategorized_out: Money,
    /// R-TDB-6 : revenus − dépenses.
    pub savings: Money,
    /// R-TDB-7 : non défini si les revenus valent 0.
    pub savings_rate: Option<Rate>,
}

/// Revenus, dépenses, non catégorisé, épargne et taux d'épargne sur [`from`, `to`] (R-TDB-3 à R-TDB-7).
pub fn flows(splits: &[SplitFact], from: Date, to: Date) -> Flows {
    let mut income = Money::ZERO;
    let mut expense_splits = Money::ZERO;
    let mut uncategorized_in = Money::ZERO;
    let mut uncategorized_out = Money::ZERO;
    for split in splits.iter().filter(|s| s.is_eur && !s.is_transfer && s.date >= from && s.date <= to) {
        match split.category_kind {
            Some(CategoryKind::Income) => income += split.amount,
            Some(CategoryKind::Expense) => expense_splits += split.amount,
            None if split.amount.is_positive() => uncategorized_in += split.amount,
            None => uncategorized_out += -split.amount,
        }
    }
    let expenses = -expense_splits;
    let savings = income - expenses;
    Flows {
        income,
        expenses,
        uncategorized_in,
        uncategorized_out,
        savings,
        savings_rate: savings_rate(savings, income),
    }
}

/// Épargne / revenus, arrondi au millionième (R-TDB-7).
pub fn savings_rate(savings: Money, income: Money) -> Option<Rate> {
    if income.is_zero() {
        return None;
    }
    let ratio = Decimal::from(savings.minor()) * Decimal::from(Rate::ONE.ppm()) / Decimal::from(income.minor());
    ratio.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_i64().map(Rate::from_ppm)
}

/// Période d'un indicateur mensuel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub from: Date,
    pub to: Date,
}

/// Mois en cours jusqu'à `today` inclus, et la même période du mois précédent (R-TDB-13).
///
/// Le jour est borné au dernier jour du mois précédent (le 31 mars se compare au 28 février).
pub fn month_to_date(today: Date) -> (Period, Period) {
    let current = Period { from: first_of_month(today), to: today };
    let previous_start = current.from - Months::new(1);
    let previous_end = previous_start.with_day(today.day()).unwrap_or_else(|| last_of_month(previous_start));
    (current, Period { from: previous_start, to: previous_end })
}

pub fn first_of_month(date: Date) -> Date {
    date.with_day(1).unwrap_or(date)
}

pub fn last_of_month(date: Date) -> Date {
    let next = first_of_month(date) + Months::new(1);
    next.pred_opt().unwrap_or(date)
}

/// Dernier jour du mois précédent : référence de la variation de patrimoine (R-TDB-2).
pub fn end_of_previous_month(today: Date) -> NaiveDate {
    first_of_month(today).pred_opt().unwrap_or(today)
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

    fn split(amount: i64, kind: Option<CategoryKind>, is_transfer: bool) -> SplitFact {
        SplitFact { date: d("2026-03-15"), amount: m(amount), category_kind: kind, is_transfer, is_eur: true }
    }

    /// Critère d'acceptation de TDB-01.
    #[test]
    fn tdb_01_monthly_flows_exclude_transfers() {
        use CategoryKind::*;
        let splits = [
            split(300_000, Some(Income), false),
            split(-180_000, Some(Expense), false),
            split(5_000, Some(Expense), false),
            split(-50_000, None, true),
            split(50_000, None, true),
        ];
        let result = flows(&splits, d("2026-03-01"), d("2026-03-31"));
        assert_eq!(result.income, m(300_000));
        assert_eq!(result.expenses, m(175_000));
        assert_eq!(result.savings, m(125_000));
        assert_eq!(result.savings_rate, Some(Rate::from_ppm(416_667)));
        assert_eq!(result.uncategorized_out, Money::ZERO);
    }

    #[test]
    fn tdb_01_transfer_does_not_change_net_worth() {
        let account =
            |family, value| AccountValue { family, include_in_net_worth: true, is_eur: true, value: m(value) };
        let before = [account(AccountFamily::Cash, 100_000), account(AccountFamily::FixedSavings, 0)];
        let after = [account(AccountFamily::Cash, 50_000), account(AccountFamily::FixedSavings, 50_000)];
        assert_eq!(net_worth(&before).total, net_worth(&after).total);
    }

    #[test]
    fn r_tdb_1_net_worth_separates_debts_and_excludes_non_eur() {
        let accounts = [
            AccountValue { family: AccountFamily::Cash, include_in_net_worth: true, is_eur: true, value: m(200_000) },
            AccountValue {
                family: AccountFamily::Liability,
                include_in_net_worth: true,
                is_eur: true,
                value: m(-150_000),
            },
            AccountValue { family: AccountFamily::Cash, include_in_net_worth: true, is_eur: false, value: m(999) },
            AccountValue { family: AccountFamily::Cash, include_in_net_worth: false, is_eur: true, value: m(777) },
        ];
        assert_eq!(net_worth(&accounts), NetWorth { assets: m(200_000), liabilities: m(-150_000), total: m(50_000) });
    }

    #[test]
    fn r_tdb_5_uncategorized_split_in_and_out() {
        let result = flows(&[split(1_000, None, false), split(-400, None, false)], d("2026-03-01"), d("2026-03-31"));
        assert_eq!(result.uncategorized_in, m(1_000));
        assert_eq!(result.uncategorized_out, m(400));
    }

    #[test]
    fn r_tdb_7_rate_undefined_without_income() {
        assert_eq!(savings_rate(m(-100), Money::ZERO), None);
    }

    #[test]
    fn r_tdb_13_compares_with_previous_month_at_same_date() {
        let (current, previous) = month_to_date(d("2026-03-31"));
        assert_eq!(current, Period { from: d("2026-03-01"), to: d("2026-03-31") });
        assert_eq!(previous, Period { from: d("2026-02-01"), to: d("2026-02-28") });
        let (_, previous) = month_to_date(d("2026-03-12"));
        assert_eq!(previous, Period { from: d("2026-02-01"), to: d("2026-02-12") });
        assert_eq!(end_of_previous_month(d("2026-03-12")), d("2026-02-28"));
    }
}
