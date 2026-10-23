//! Tiers et groupes de tiers (spec 11) : contrôles, suggestion de catégorie et totaux par tiers.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::reporting::{self, Period, SplitFact};
use crate::shared::ids::CategoryId;
use crate::shared::{Date, DomainError, Money};

/// Une jambe de virement interne n'a pas de tiers : de l'autre côté, c'est un de mes comptes (R-TIE-11).
pub fn check_transfer_leg(has_payee: bool, is_transfer_leg: bool) -> Result<(), DomainError> {
    if has_payee && is_transfer_leg { Err(DomainError::PayeeOnTransferLeg) } else { Ok(()) }
}

/// Un tiers utilisé par une transaction, une récurrence ou une règle ne se supprime pas (R-TIE-5).
pub fn check_deletable(usage: u64) -> Result<(), DomainError> {
    if usage > 0 { Err(DomainError::PayeeInUse) } else { Ok(()) }
}

/// On ne fusionne un tiers que dans un autre tiers (R-TIE-7).
pub fn check_merge(same: bool) -> Result<(), DomainError> {
    if same { Err(DomainError::InvalidPayeeMerge) } else { Ok(()) }
}

/// Une transaction passée d'un tiers, vue par la suggestion de catégorie (R-TIE-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PastTransaction<T> {
    pub date: Date,
    /// Départage deux transactions du même jour (identifiants triables dans le temps, ADR-0011).
    pub id: T,
    pub split_count: usize,
    /// Catégorie de la première ventilation.
    pub category: Option<CategoryId>,
}

/// Catégorie de la dernière transaction (par `date`) qui n'a qu'une ventilation, catégorisée (R-TIE-14).
pub fn suggested_category<T: Ord + Copy>(history: &[PastTransaction<T>]) -> Option<CategoryId> {
    history
        .iter()
        .filter(|t| t.split_count == 1 && t.category.is_some())
        .max_by_key(|t| (t.date, t.id))
        .and_then(|t| t.category)
}

/// Une ventilation, rattachée au tiers de sa transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayeeFact<P, T> {
    pub payee: P,
    pub transaction: T,
    pub split: SplitFact,
}

/// Dépenses, revenus et non catégorisé d'un tiers ou d'un groupe sur une période (R-TIE-20, R-TIE-21).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PayeeTotals {
    /// R-TDB-3.
    pub income: Money,
    /// R-TDB-4 : positif, remboursements déduits.
    pub expenses: Money,
    /// R-TDB-5 : affichés à côté, jamais ajoutés aux dépenses ni aux revenus (R-TIE-21).
    pub uncategorized_in: Money,
    pub uncategorized_out: Money,
    /// Transactions qui ont au moins une ventilation comptée.
    pub transactions: u32,
}

impl PayeeTotals {
    /// Dépense moyenne par transaction, arrondie à l'unité mineure ; non définie sans transaction.
    pub fn average_expense(&self) -> Option<Money> {
        if self.transactions == 0 {
            return None;
        }
        let value = (Decimal::from(self.expenses.minor()) / Decimal::from(self.transactions))
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
        value.to_i64().map(Money::from_minor)
    }
}

impl std::ops::Add for PayeeTotals {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            income: self.income + other.income,
            expenses: self.expenses + other.expenses,
            uncategorized_in: self.uncategorized_in + other.uncategorized_in,
            uncategorized_out: self.uncategorized_out + other.uncategorized_out,
            transactions: self.transactions + other.transactions,
        }
    }
}

impl std::iter::Sum for PayeeTotals {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |a, b| a + b)
    }
}

/// Totaux par tiers sur une période : les définitions de spec 08 restreintes aux ventilations des transactions
/// de chaque tiers, hors virements, remboursements et acquisitions, comptes en euros seulement (R-TIE-20, R-TIE-22).
///
/// Un groupe somme les totaux de ses tiers : `PayeeTotals` s'additionne.
pub fn totals<P: Copy + Eq + Hash, T: Copy + Eq + Hash>(
    facts: &[PayeeFact<P, T>],
    period: Period,
) -> HashMap<P, PayeeTotals> {
    let counted = |f: &&PayeeFact<P, T>| {
        f.split.is_eur && !f.split.is_movement && f.split.date >= period.from && f.split.date <= period.to
    };
    let mut splits: HashMap<P, Vec<SplitFact>> = HashMap::new();
    let mut transactions: HashMap<P, HashSet<T>> = HashMap::new();
    for fact in facts.iter().filter(counted) {
        splits.entry(fact.payee).or_default().push(fact.split);
        transactions.entry(fact.payee).or_default().insert(fact.transaction);
    }
    splits
        .into_iter()
        .map(|(payee, splits)| {
            let flows = reporting::flows(&splits, period.from, period.to);
            let count = transactions.get(&payee).map_or(0, HashSet::len);
            (
                payee,
                PayeeTotals {
                    income: flows.income,
                    expenses: flows.expenses,
                    uncategorized_in: flows.uncategorized_in,
                    uncategorized_out: flows.uncategorized_out,
                    transactions: u32::try_from(count).unwrap_or(u32::MAX),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::category::CategoryKind;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn m(minor: i64) -> Money {
        Money::from_minor(minor)
    }

    fn fact(
        payee: &'static str,
        transaction: u32,
        amount: i64,
        kind: Option<CategoryKind>,
    ) -> PayeeFact<&'static str, u32> {
        PayeeFact {
            payee,
            transaction,
            split: SplitFact {
                date: d("2026-03-10"),
                amount: m(amount),
                category_kind: kind,
                is_movement: false,
                is_eur: true,
            },
        }
    }

    fn march() -> Period {
        Period { from: d("2026-03-01"), to: d("2026-03-31") }
    }

    /// Critère d'acceptation de TIE-04.
    #[test]
    fn r_tie_20_r_tie_21_group_totals_sum_their_payees_and_keep_uncategorized_apart() {
        use CategoryKind::Expense;
        let facts = [
            fact("meylan", 1, -8_000, Some(Expense)),
            fact("grenoble", 2, -2_000, Some(Expense)),
            fact("meylan", 3, 500, Some(Expense)),
            fact("meylan", 4, -1_500, None),
        ];
        let by_payee = totals(&facts, march());
        let meylan = by_payee["meylan"];
        assert_eq!(meylan.expenses, m(7_500));
        assert_eq!(meylan.uncategorized_out, m(1_500));
        assert_eq!(meylan.transactions, 3);
        let group: PayeeTotals = by_payee.values().copied().sum();
        assert_eq!(group.expenses, m(9_500));
        assert_eq!(group.uncategorized_out, m(1_500));
        assert_eq!(group.income, Money::ZERO);
    }

    #[test]
    fn r_tie_20_r_tie_22_movements_other_currencies_and_other_periods_are_excluded() {
        let mut transfer = fact("bank", 1, -5_000, None);
        transfer.split.is_movement = true;
        let mut dollars = fact("bank", 2, -5_000, Some(CategoryKind::Expense));
        dollars.split.is_eur = false;
        let mut april = fact("bank", 3, -5_000, Some(CategoryKind::Expense));
        april.split.date = d("2026-04-01");
        let by_payee = totals(&[transfer, dollars, april], march());
        assert!(by_payee.is_empty());
    }

    #[test]
    fn r_tie_20_split_transaction_counts_once() {
        use CategoryKind::{Expense, Income};
        let by_payee = totals(&[fact("shop", 1, -3_000, Some(Expense)), fact("shop", 1, 1_000, Some(Income))], march());
        assert_eq!(by_payee["shop"].transactions, 1);
        assert_eq!(by_payee["shop"].income, m(1_000));
        assert_eq!(by_payee["shop"].average_expense(), Some(m(3_000)));
        assert_eq!(PayeeTotals::default().average_expense(), None);
    }

    #[test]
    fn r_tie_14_suggests_the_category_of_the_last_single_split_transaction() {
        let (groceries, home) = (CategoryId::new(), CategoryId::new());
        let past =
            |date: &str, id: u32, split_count, category| PastTransaction { date: d(date), id, split_count, category };
        let history = [
            past("2026-03-01", 1, 1, Some(home)),
            past("2026-03-05", 2, 1, Some(groceries)),
            // Plus récentes mais écartées : ventilée, non catégorisée.
            past("2026-03-08", 3, 2, Some(home)),
            past("2026-03-09", 4, 1, None),
        ];
        assert_eq!(suggested_category(&history), Some(groceries));
        assert_eq!(suggested_category::<u32>(&[]), None);
    }

    #[test]
    fn r_tie_11_no_payee_on_a_transfer_leg() {
        assert_eq!(check_transfer_leg(true, true), Err(DomainError::PayeeOnTransferLeg));
        assert!(check_transfer_leg(false, true).is_ok());
        assert!(check_transfer_leg(true, false).is_ok());
    }

    #[test]
    fn r_tie_5_r_tie_7_used_payees_are_kept_and_merge_needs_another_payee() {
        assert_eq!(check_deletable(3), Err(DomainError::PayeeInUse));
        assert!(check_deletable(0).is_ok());
        assert_eq!(check_merge(true), Err(DomainError::InvalidPayeeMerge));
        assert!(check_merge(false).is_ok());
    }
}
