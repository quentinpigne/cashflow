//! Points de contrôle et rapprochement (spec 04).

use crate::shared::{Date, DomainError, Money};

/// Écart d'un point de contrôle : solde annoncé − solde calculé (R-RAP-3, jamais stocké).
pub fn gap(declared: Money, computed: Money) -> Money {
    declared - computed
}

/// Un point de contrôle tel que l'examine le rapprochement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointState<Id> {
    pub id: Id,
    pub date: Date,
    pub gap: Money,
    pub reconciled: bool,
}

/// Effet d'un rapprochement au point `target` (R-RAP-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcilePlan<Id> {
    /// Points rapprochés : la cible et les points antérieurs à écart nul.
    pub reconcile: Vec<Id>,
    /// Points antérieurs dont l'écart sera figé (signalés avant validation).
    pub frozen: Vec<Id>,
}

/// Prépare le rapprochement de `target` : seul un point à écart nul se rapproche (R-RAP-4).
pub fn plan_reconcile<Id: Copy + PartialEq>(
    checkpoints: &[CheckpointState<Id>],
    target: Id,
) -> Result<ReconcilePlan<Id>, DomainError> {
    let point = checkpoints.iter().find(|c| c.id == target).ok_or(DomainError::CheckpointNotFound)?;
    if point.reconciled {
        return Err(DomainError::CheckpointAlreadyReconciled);
    }
    if !point.gap.is_zero() {
        return Err(DomainError::CheckpointGapNotZero { gap: point.gap });
    }
    let mut plan = ReconcilePlan { reconcile: vec![target], frozen: Vec::new() };
    for earlier in checkpoints.iter().filter(|c| c.date < point.date && !c.reconciled) {
        if earlier.gap.is_zero() {
            plan.reconcile.push(earlier.id);
        } else {
            plan.frozen.push(earlier.id);
        }
    }
    Ok(plan)
}

/// Déverrouiller un point rapproché déverrouille aussi tous les points postérieurs (R-RAP-6).
pub fn plan_unlock<Id: Copy>(checkpoints: &[CheckpointState<Id>], from: Date) -> Vec<Id> {
    checkpoints.iter().filter(|c| c.reconciled && c.date >= from).map(|c| c.id).collect()
}

/// Période verrouillée d'un compte : jusqu'à la date du dernier point rapproché, incluse (R-RAP-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lock {
    pub until: Option<Date>,
}

impl Lock {
    pub fn covers(&self, balance_date: Date) -> bool {
        self.until.is_some_and(|until| balance_date <= until)
    }

    /// Refuse une écriture qui modifierait le solde à une date couverte (R-RAP-7).
    pub fn check(&self, balance_date: Date) -> Result<(), DomainError> {
        match self.until {
            Some(until) if balance_date <= until => Err(DomainError::PeriodReconciled { until }),
            _ => Ok(()),
        }
    }
}

/// Une transaction vue par l'aide à la recherche d'écart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapTransaction<Id> {
    pub id: Id,
    pub amount: Money,
}

/// Pistes pour expliquer un écart (R-RAP-9) entre le dernier point rapproché et le point en écart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapHints<Id> {
    /// Une transaction dont le montant égale l'écart ou son opposé.
    pub singles: Vec<Id>,
    /// Deux transactions dont la somme égale l'écart ou son opposé.
    pub pairs: Vec<(Id, Id)>,
    /// Une transaction dont le montant égale le double de l'écart : erreur de signe probable.
    pub sign_errors: Vec<Id>,
}

/// Cherche les transactions qui expliqueraient l'écart (R-RAP-9). Les paires sont limitées aux
/// 50 premières trouvées.
pub fn gap_hints<Id: Copy>(gap: Money, transactions: &[GapTransaction<Id>]) -> GapHints<Id> {
    let target = gap.minor().abs();
    let mut hints = GapHints { singles: Vec::new(), pairs: Vec::new(), sign_errors: Vec::new() };
    if target == 0 {
        return hints;
    }
    for t in transactions {
        if t.amount.minor().abs() == target {
            hints.singles.push(t.id);
        }
        if t.amount.minor().abs() == target * 2 {
            hints.sign_errors.push(t.id);
        }
    }
    'outer: for (i, a) in transactions.iter().enumerate() {
        for b in &transactions[i + 1..] {
            if (a.amount + b.amount).minor().abs() == target {
                hints.pairs.push((a.id, b.id));
                if hints.pairs.len() == 50 {
                    break 'outer;
                }
            }
        }
    }
    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn cp(id: u32, date: &str, gap: i64, reconciled: bool) -> CheckpointState<u32> {
        CheckpointState { id, date: d(date), gap: Money::from_minor(gap), reconciled }
    }

    /// Critère d'acceptation de RAP-01 / RAP-03.
    #[test]
    fn rap_01_gap_and_validation() {
        let computed = Money::from_minor(100_000 - 25_000);
        assert_eq!(gap(Money::from_minor(75_000), computed), Money::ZERO);
        assert_eq!(gap(Money::from_minor(74_000), computed), Money::from_minor(-1_000));
        let checkpoints = [cp(1, "2026-01-31", -1_000, false)];
        assert_eq!(
            plan_reconcile(&checkpoints, 1),
            Err(DomainError::CheckpointGapNotZero { gap: Money::from_minor(-1_000) })
        );
    }

    #[test]
    fn r_rap_4_earlier_points_are_reconciled_or_frozen() {
        let checkpoints =
            [cp(1, "2026-01-31", 0, false), cp(2, "2026-02-28", -1_250, false), cp(3, "2026-03-31", 0, false)];
        let plan = plan_reconcile(&checkpoints, 3).unwrap();
        assert_eq!(plan.reconcile, vec![3, 1]);
        assert_eq!(plan.frozen, vec![2]);
    }

    #[test]
    fn r_rap_6_unlock_cascades_to_later_points() {
        let checkpoints = [cp(1, "2026-01-31", 0, true), cp(2, "2026-02-28", 0, true), cp(3, "2026-03-31", 0, true)];
        assert_eq!(plan_unlock(&checkpoints, d("2026-02-28")), vec![2, 3]);
    }

    #[test]
    fn r_rap_7_lock_refuses_covered_dates() {
        let lock = Lock { until: Some(d("2026-01-31")) };
        assert_eq!(lock.check(d("2026-01-15")), Err(DomainError::PeriodReconciled { until: d("2026-01-31") }));
        assert!(lock.check(d("2026-02-01")).is_ok());
        assert!(Lock { until: None }.check(d("2026-01-15")).is_ok());
    }

    #[test]
    fn r_rap_9_gap_hints() {
        let t = |id: u32, minor: i64| GapTransaction { id, amount: Money::from_minor(minor) };
        let transactions = [t(1, -1_250), t(2, -500), t(3, -750), t(4, 2_500)];
        let hints = gap_hints(Money::from_minor(1_250), &transactions);
        assert_eq!(hints.singles, vec![1]);
        assert_eq!(hints.pairs, vec![(1, 4), (2, 3)]);
        assert_eq!(hints.sign_errors, vec![4]);
    }
}
