//! Correspondance entre lignes importées et transactions existantes (R-IMP-9, R-TRX-35).

use std::collections::HashSet;

use super::fingerprint::normalize_label;
use crate::shared::{Date, Money};

/// Score minimal pour proposer `MATCH` par défaut (R-IMP-9).
pub const AUTO_MATCH_SCORE: u8 = 80;

/// Une transaction existante susceptible de correspondre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<Id> {
    pub id: Id,
    /// Date de solde de la transaction.
    pub date: Date,
    pub amount: Money,
    pub label: String,
}

/// Une ligne importée à rapprocher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineToMatch {
    pub date: Date,
    pub amount: Money,
    pub bank_label: String,
}

/// Ratio de mots communs entre deux libellés normalisés, en pour cent.
pub fn label_similarity(a: &str, b: &str) -> u8 {
    let tokens = |s: &str| -> HashSet<String> {
        normalize_label(s).split(|c: char| !c.is_alphanumeric()).filter(|t| t.len() > 1).map(str::to_owned).collect()
    };
    let (left, right) = (tokens(a), tokens(b));
    let smaller = left.len().min(right.len());
    if smaller == 0 {
        return 0;
    }
    let common = left.intersection(&right).count();
    u8::try_from(common * 100 / smaller).unwrap_or(100)
}

/// Score de 0 à 100 (R-IMP-9) : 60 pour le montant exact, jusqu'à 25 pour la proximité de date (25 le
/// jour même, −5 par jour d'écart), jusqu'à 15 pour la similarité des libellés.
pub fn score<Id>(line: &LineToMatch, candidate: &Candidate<Id>, tolerance_days: i64) -> Option<u8> {
    let gap = (candidate.date - line.date).num_days().abs();
    if candidate.amount != line.amount || gap > tolerance_days {
        return None;
    }
    let date_points = u8::try_from((25 - 5 * gap).max(0)).unwrap_or(0);
    let label_points =
        u8::try_from(u16::from(label_similarity(&line.bank_label, &candidate.label)) * 15 / 100).unwrap_or(15);
    Some(60 + date_points + label_points)
}

/// Meilleur candidat pour chaque ligne, sans attribuer deux fois la même transaction (R-IMP-9).
///
/// Les meilleurs couples sont servis d'abord, pour qu'une transaction aille à la ligne qui lui
/// correspond le mieux.
pub fn assign<Id: Copy + Eq + std::hash::Hash>(
    lines: &[LineToMatch],
    candidates: &[Candidate<Id>],
    tolerance_days: i64,
) -> Vec<Option<(Id, u8)>> {
    let mut pairs: Vec<(u8, usize, usize)> = Vec::new();
    for (l, line) in lines.iter().enumerate() {
        for (c, candidate) in candidates.iter().enumerate() {
            if let Some(points) = score(line, candidate, tolerance_days) {
                pairs.push((points, l, c));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut result = vec![None; lines.len()];
    let mut used = HashSet::new();
    for (points, l, c) in pairs {
        if result[l].is_none() && used.insert(candidates[c].id) {
            result[l] = Some((candidates[c].id, points));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    /// Critère d'acceptation d'IMP-05.
    #[test]
    fn imp_05_bakery_is_matched_automatically() {
        let line = LineToMatch {
            date: d("2026-03-11"),
            amount: Money::from_minor(-420),
            bank_label: "CB BOULANGERIE DUPONT 10/03".into(),
        };
        let candidate =
            Candidate { id: 1, date: d("2026-03-10"), amount: Money::from_minor(-420), label: "Boulangerie".into() };
        let points = score(&line, &candidate, 5).unwrap();
        assert!(points >= AUTO_MATCH_SCORE, "score {points}");
    }

    #[test]
    fn r_imp_9_amount_must_match_and_date_within_tolerance() {
        let line = LineToMatch { date: d("2026-03-10"), amount: Money::from_minor(-420), bank_label: "X".into() };
        let wrong_amount =
            Candidate { id: 1, date: d("2026-03-10"), amount: Money::from_minor(-421), label: "X".into() };
        let too_far = Candidate { id: 2, date: d("2026-03-16"), amount: Money::from_minor(-420), label: "X".into() };
        assert_eq!(score(&line, &wrong_amount, 5), None);
        assert_eq!(score(&line, &too_far, 5), None);
        let same_day = Candidate { id: 3, date: d("2026-03-10"), amount: Money::from_minor(-420), label: "Y".into() };
        assert_eq!(score(&line, &same_day, 5), Some(85));
    }

    #[test]
    fn r_imp_9_a_transaction_is_assigned_once() {
        let lines = [
            LineToMatch { date: d("2026-03-12"), amount: Money::from_minor(-250), bank_label: "CAFE".into() },
            LineToMatch { date: d("2026-03-10"), amount: Money::from_minor(-250), bank_label: "CAFE".into() },
        ];
        let candidates =
            [Candidate { id: 7, date: d("2026-03-10"), amount: Money::from_minor(-250), label: "Café".into() }];
        let result = assign(&lines, &candidates, 5);
        assert_eq!(result[0], None);
        assert_eq!(result[1].map(|(id, _)| id), Some(7));
    }
}
