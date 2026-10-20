//! Soldes calculés (ADR-0002, R-CPT-4, R-TRX-7).

use crate::shared::{Date, Money};

/// Un mouvement daté par sa date de solde (`balance_date`, R-TRX-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatedAmount {
    pub balance_date: Date,
    pub amount: Money,
}

/// Solde à la fin de la journée `at` : ouverture + Σ des mouvements de date de solde ≤ `at` (R-CPT-4).
pub fn balance_at(opening_balance: Money, movements: &[DatedAmount], at: Date) -> Money {
    opening_balance + movements.iter().filter(|m| m.balance_date <= at).map(|m| m.amount).sum::<Money>()
}

/// Valorisation constatée d'un compte d'épargne variable ou d'un prêt (spec 01, CPT-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Valuation {
    pub date: Date,
    pub value: Money,
}

/// Valeur d'un compte à la fin de la journée `at` (R-CPT-6) : dernière valorisation à date ≤ `at`, plus
/// les flux nets enregistrés après elle (exclue) jusqu'à `at` (inclus). Sans valorisation : ouverture + flux.
///
/// Renvoie aussi la date de la valorisation retenue, pour signaler une valeur « estimée ».
pub fn value_at(
    opening_balance: Money,
    valuations: &[Valuation],
    movements: &[DatedAmount],
    at: Date,
) -> (Money, Option<Date>) {
    let last = valuations.iter().filter(|v| v.date <= at).max_by_key(|v| v.date);
    match last {
        Some(valuation) => {
            let flows: Money = movements
                .iter()
                .filter(|m| m.balance_date > valuation.date && m.balance_date <= at)
                .map(|m| m.amount)
                .sum();
            (valuation.value + flows, Some(valuation.date))
        }
        None => (balance_at(opening_balance, movements, at), None),
    }
}

/// Solde après chaque opération, dans l'ordre fourni (TRX-02).
///
/// L'appelant trie les mouvements par date de solde, puis par ordre de création.
pub fn running_balances(opening_balance: Money, movements: &[DatedAmount]) -> Vec<Money> {
    movements
        .iter()
        .scan(opening_balance, |balance, movement| {
            *balance += movement.amount;
            Some(*balance)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn mv(date: &str, minor: i64) -> DatedAmount {
        DatedAmount { balance_date: d(date), amount: Money::from_minor(minor) }
    }

    #[test]
    fn cpt_01_opening_balance_alone() {
        assert_eq!(balance_at(Money::from_minor(136_993), &[], d("2026-01-01")), Money::from_minor(136_993));
    }

    #[test]
    fn cpt_03_balance_includes_only_movements_up_to_date() {
        let movements = [mv("2026-03-10", -5_000), mv("2026-03-31", 200_000)];
        assert_eq!(balance_at(Money::ZERO, &movements, d("2026-03-15")), Money::from_minor(-5_000));
        assert_eq!(balance_at(Money::ZERO, &movements, d("2026-03-31")), Money::from_minor(195_000));
    }

    #[test]
    fn trx_02_running_balance() {
        let movements = [mv("2026-03-01", -1_000), mv("2026-03-02", 500)];
        assert_eq!(
            running_balances(Money::from_minor(10_000), &movements),
            vec![Money::from_minor(9_000), Money::from_minor(9_500)]
        );
    }

    #[test]
    fn r_cpt_6_value_is_last_valuation_plus_later_flows() {
        let valuations = [
            Valuation { date: d("2026-01-31"), value: Money::from_minor(1_000_000) },
            Valuation { date: d("2026-02-28"), value: Money::from_minor(1_020_000) },
        ];
        let movements = [mv("2026-02-15", 50_000), mv("2026-03-05", 30_000), mv("2026-03-20", -10_000)];
        assert_eq!(
            value_at(Money::ZERO, &valuations, &movements, d("2026-03-10")),
            (Money::from_minor(1_050_000), Some(d("2026-02-28")))
        );
        assert_eq!(
            value_at(Money::ZERO, &valuations, &movements, d("2026-02-20")),
            (Money::from_minor(1_050_000), Some(d("2026-01-31")))
        );
    }

    #[test]
    fn r_cpt_6_without_valuation_falls_back_to_opening_plus_flows() {
        let movements = [mv("2026-02-15", 50_000)];
        assert_eq!(
            value_at(Money::from_minor(-15_000_000), &[], &movements, d("2026-03-01")),
            (Money::from_minor(-14_950_000), None)
        );
    }

    proptest! {
        /// Solde = ouverture + Σ (critère de sortie de la phase 1).
        #[test]
        fn r_cpt_4_balance_is_opening_plus_sum(
            opening in -1_000_000_i64..1_000_000,
            amounts in prop::collection::vec((0_u32..365, -1_000_000_i64..1_000_000), 0..50),
        ) {
            let start = d("2026-01-01");
            let movements: Vec<DatedAmount> = amounts
                .iter()
                .map(|&(offset, minor)| DatedAmount {
                    balance_date: start + chrono::Days::new(u64::from(offset)),
                    amount: Money::from_minor(minor),
                })
                .collect();
            let end = start + chrono::Days::new(365);
            let expected = Money::from_minor(opening + amounts.iter().map(|&(_, a)| a).sum::<i64>());
            prop_assert_eq!(balance_at(Money::from_minor(opening), &movements, end), expected);
            let running = running_balances(Money::from_minor(opening), &movements);
            prop_assert_eq!(running.last().copied().unwrap_or(Money::from_minor(opening)), expected);
        }
    }
}
