//! Remboursements de frais (TRX-12, ADR-0026) : une ventilation positive rembourse une dépense, en tout ou
//! partie ; une dépense peut porter le remboursement qu'on en attend.

use crate::ledger::category::CategoryKind;
use crate::shared::{Date, DomainError, Money};

/// La dépense visée par un remboursement, vue par la validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefundTarget {
    pub amount: Money,
    pub is_transfer_leg: bool,
    /// La dépense est la transaction qui porte la ventilation.
    pub is_same_transaction: bool,
}

/// Une ventilation qui rembourse : positive, sur une catégorie de dépense, visant une autre transaction de
/// montant négatif qui n'est pas une jambe de virement (R-TRX-50).
pub fn check_refund(amount: Money, category: Option<CategoryKind>, target: RefundTarget) -> Result<(), DomainError> {
    let valid = amount.is_positive()
        && category == Some(CategoryKind::Expense)
        && target.amount.is_negative()
        && !target.is_transfer_leg
        && !target.is_same_transaction;
    if valid { Ok(()) } else { Err(DomainError::InvalidRefund) }
}

/// Le remboursé d'une dépense ne dépasse jamais son montant (R-TRX-52).
pub fn check_refunded(expense: Money, refunded: Money) -> Result<(), DomainError> {
    if refunded > -expense { Err(DomainError::RefundExceedsExpense) } else { Ok(()) }
}

/// Le remboursement attendu d'une dépense est positif et au plus égal à son montant (R-TRX-53).
pub fn check_expected(expense: Money, expected: Money) -> Result<(), DomainError> {
    if expense.is_negative() && expected.is_positive() && expected <= -expense {
        Ok(())
    } else {
        Err(DomainError::InvalidExpectedRefund)
    }
}

/// Reste à recevoir : attendu − remboursé, zéro au minimum (R-TRX-53).
pub fn pending(expected: Option<Money>, refunded: Money) -> Money {
    expected.map_or(Money::ZERO, |e| (e - refunded).max(Money::ZERO))
}

/// Une dépense qui pourrait être remboursée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<T, P> {
    pub id: T,
    pub date: Date,
    /// Montant de la dépense (négatif).
    pub amount: Money,
    pub payee: Option<P>,
    pub expected: Option<Money>,
    /// Déjà remboursé, par d'autres entrées que celle qu'on lie.
    pub refunded: Money,
}

impl<T, P> Candidate<T, P> {
    /// Ce qui reste à rembourser de la dépense.
    pub fn refundable(&self) -> Money {
        (-self.amount - self.refunded).max(Money::ZERO)
    }

    pub fn pending(&self) -> Money {
        pending(self.expected, self.refunded)
    }
}

/// Une proposition : la dépense et le montant proposé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal<T, P> {
    pub candidate: Candidate<T, P>,
    pub proposed: Money,
}

/// Dépenses proposées pour une entrée de `date`, de `payee`, dont `available` n'est encore affecté à aucune
/// dépense (R-TRX-54) : toutes celles qui ne sont pas entièrement remboursées, quelle que soit leur date ; reste à
/// recevoir non nul d'abord, puis même tiers, puis date la plus proche, avant ou après. Le montant proposé est le
/// reste à recevoir, sinon ce qui reste à rembourser, dans la limite de `available`.
pub fn proposals<T, P: PartialEq>(
    candidates: Vec<Candidate<T, P>>,
    date: Date,
    payee: Option<&P>,
    available: Money,
) -> Vec<Proposal<T, P>> {
    let mut kept: Vec<Candidate<T, P>> = candidates.into_iter().filter(|c| c.refundable().is_positive()).collect();
    kept.sort_by_key(|c| {
        (!c.pending().is_positive(), !(payee.is_some() && c.payee.as_ref() == payee), (date - c.date).num_days().abs())
    });
    kept.into_iter()
        .map(|c| {
            let wanted = if c.pending().is_positive() { c.pending() } else { c.refundable() };
            let proposed = wanted.min(c.refundable()).min(available.max(Money::ZERO));
            Proposal { candidate: c, proposed }
        })
        .collect()
}

/// Recherche dans les dépenses proposées (R-TRX-54) : le texte, sans distinction de casse, dans le libellé ou le
/// nom du tiers ; un montant (« 68,40 », « 68.4 », « 68 ») dans le montant de la dépense écrit en euros, sans
/// séparateur de milliers (« 1268,00 »). Une recherche vide garde tout.
pub fn matches_search(query: &str, label: &str, payee: Option<&str>, amount: Money) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    if label.to_lowercase().contains(&query) || payee.is_some_and(|p| p.to_lowercase().contains(&query)) {
        return true;
    }
    let number: String =
        query.chars().filter(|c| !c.is_whitespace() && *c != '€').map(|c| if c == '.' { ',' } else { c }).collect();
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit() || c == ',') {
        return false;
    }
    let minor = amount.minor().unsigned_abs();
    let written = format!("{},{:02}", minor / 100, minor % 100);
    // « 68,4 » cherche « 68,40 » : une décimale tapée est complétée.
    let prefix = match number.split_once(',') {
        Some((_, decimals)) if decimals.len() == 1 => format!("{number}0"),
        _ => number,
    };
    written.contains(&prefix)
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

    const OTHER: RefundTarget =
        RefundTarget { amount: Money::from_minor(-2_500), is_transfer_leg: false, is_same_transaction: false };

    #[test]
    fn r_trx_50_a_refund_is_a_positive_expense_split_towards_another_expense() {
        assert_eq!(check_refund(m(1_750), Some(CategoryKind::Expense), OTHER), Ok(()));
        let invalid = [
            (m(-1_750), Some(CategoryKind::Expense), OTHER),
            (m(1_750), Some(CategoryKind::Income), OTHER),
            (m(1_750), None, OTHER),
            (m(1_750), Some(CategoryKind::Expense), RefundTarget { amount: m(2_500), ..OTHER }),
            (m(1_750), Some(CategoryKind::Expense), RefundTarget { is_transfer_leg: true, ..OTHER }),
            (m(1_750), Some(CategoryKind::Expense), RefundTarget { is_same_transaction: true, ..OTHER }),
        ];
        for (amount, kind, target) in invalid {
            assert_eq!(check_refund(amount, kind, target), Err(DomainError::InvalidRefund));
        }
    }

    #[test]
    fn r_trx_52_refunded_never_exceeds_the_expense() {
        assert_eq!(check_refunded(m(-2_500), m(2_500)), Ok(()));
        assert_eq!(check_refunded(m(-2_500), m(2_501)), Err(DomainError::RefundExceedsExpense));
    }

    #[test]
    fn r_trx_53_expected_refund_and_pending() {
        assert_eq!(check_expected(m(-2_500), m(1_750)), Ok(()));
        assert_eq!(check_expected(m(-2_500), m(2_600)), Err(DomainError::InvalidExpectedRefund));
        assert_eq!(check_expected(m(-2_500), m(0)), Err(DomainError::InvalidExpectedRefund));
        assert_eq!(check_expected(m(2_500), m(100)), Err(DomainError::InvalidExpectedRefund));
        assert_eq!(pending(Some(m(1_750)), m(1_000)), m(750));
        assert_eq!(pending(Some(m(1_750)), m(2_000)), Money::ZERO);
        assert_eq!(pending(None, m(0)), Money::ZERO);
    }

    #[test]
    fn r_trx_54_search_by_label_payee_or_amount() {
        let search = |q: &str| matches_search(q, "CABINET DENTAIRE", Some("Dr Martin"), m(-126_840));
        assert!(search(""));
        assert!(search("dentaire"));
        assert!(search("martin"));
        assert!(search("1268,40"));
        assert!(search("1268.4"));
        assert!(search("1 268"));
        assert!(search("68,4 €"));
        assert!(!search("68,5"));
        assert!(!search("pharmacie"));
    }

    #[test]
    fn r_trx_54_proposals_order_and_amounts() {
        let candidate = |id: u32, date: &str, amount: i64, payee: Option<u32>, expected: Option<i64>, refunded: i64| {
            Candidate { id, date: d(date), amount: m(amount), payee, expected: expected.map(m), refunded: m(refunded) }
        };
        let candidates = vec![
            candidate(1, "2026-09-28", -6_840, None, None, 0),
            candidate(2, "2026-09-02", -2_500, Some(9), Some(1_750), 0),
            candidate(3, "2026-09-25", -3_999, Some(7), None, 0),
            candidate(4, "2026-09-26", -1_000, None, None, 1_000),
            candidate(5, "2025-09-01", -1_000, None, None, 0),
            candidate(6, "2026-10-02", -1_000, None, None, 0),
        ];
        let result = proposals(candidates, d("2026-09-30"), Some(&7), m(6_200));
        let ids: Vec<u32> = result.iter().map(|p| p.candidate.id).collect();
        // Attendu d'abord, puis même tiers, puis date la plus proche, avant ou après ; ni remboursé en entier, ni
        // limité dans le temps : toute la liste.
        assert_eq!(ids, [2, 3, 1, 6, 5]);
        assert_eq!(result[0].proposed, m(1_750));
        assert_eq!(result[1].proposed, m(3_999));
        // Limité à ce que l'entrée n'a pas encore affecté.
        assert_eq!(result[2].proposed, m(6_200));
    }
}
