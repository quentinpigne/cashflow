//! Propositions de tiers à partir des libellés bancaires (TIE-05, R-TIE-30 à R-TIE-35).

use std::collections::{BTreeMap, HashSet};
use std::hash::Hash;

use crate::import::learning::{OPERATION_WORDS, learning_key};
use crate::shared::Date;

/// Longueur maximale d'un nom de tiers (R-TIE-1).
const NAME_MAX: usize = 80;

/// Libellés montrés par proposition (R-TIE-31).
const SAMPLE_LABELS: usize = 3;

/// Clé de tiers : la clé d'apprentissage sans les mots d'opération (R-TIE-30). `None` sans clé d'apprentissage.
pub fn payee_key(label: &str) -> Option<String> {
    let key = learning_key(label)?;
    let words: Vec<&str> = key.split(' ').filter(|w| !OPERATION_WORDS.contains(w)).collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// Nom suggéré : la clé, chaque mot avec une majuscule initiale, tronquée à 80 caractères (R-TIE-32).
pub fn suggested_name(key: &str) -> String {
    let name = key
        .split(' ')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map(|first| first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect())
        })
        .collect::<Option<Vec<String>>>()
        .unwrap_or_default()
        .join(" ");
    name.chars().take(NAME_MAX).collect::<String>().trim_end().to_owned()
}

/// Motif de la règle créée à la validation : les mots de la clé, dans l'ordre, entourés de `*` (R-TIE-34).
pub fn rule_pattern(key: &str) -> String {
    format!("*{}*", key.split(' ').collect::<Vec<_>>().join("*"))
}

/// Transaction sans tiers, hors jambe de virement (R-TIE-31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unassigned<T> {
    pub id: T,
    pub date: Date,
    pub bank_label: String,
}

/// Transaction qui a un tiers non archivé, la plus récente d'abord (R-TIE-32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assigned<P> {
    pub bank_label: String,
    pub payee: P,
}

/// Tiers non archivé, pour la suggestion par le nom (R-TIE-32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedPayee<P> {
    pub id: P,
    pub name: String,
}

/// Proposition de tiers (R-TIE-31, R-TIE-32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayeeSuggestion<T, P> {
    pub key: String,
    pub name: String,
    /// Tiers existant suggéré à la place d'une création.
    pub payee: Option<P>,
    /// Transactions concernées, la plus récente d'abord.
    pub transactions: Vec<T>,
    pub first_date: Date,
    pub last_date: Date,
    /// Libellés bancaires distincts, les plus récents d'abord.
    pub labels: Vec<String>,
}

/// Propositions de tiers : transactions sans tiers de même clé, au moins deux, clé non ignorée ; rangées par
/// nombre de transactions décroissant, puis par clé (R-TIE-31, R-TIE-35).
pub fn suggestions<T: Copy + Ord, P: Copy + Eq + Hash>(
    unassigned: &[Unassigned<T>],
    assigned: &[Assigned<P>],
    payees: &[NamedPayee<P>],
    ignored: &HashSet<String>,
) -> Vec<PayeeSuggestion<T, P>> {
    let mut by_key: BTreeMap<String, Vec<&Unassigned<T>>> = BTreeMap::new();
    for transaction in unassigned {
        if let Some(key) = payee_key(&transaction.bank_label)
            && !ignored.contains(&key)
        {
            by_key.entry(key).or_default().push(transaction);
        }
    }
    let mut result: Vec<PayeeSuggestion<T, P>> = by_key
        .into_iter()
        .filter(|(_, transactions)| transactions.len() >= 2)
        .filter_map(|(key, mut transactions)| {
            transactions.sort_by_key(|t| std::cmp::Reverse((t.date, t.id)));
            let first_date = transactions.iter().map(|t| t.date).min()?;
            let last_date = transactions.iter().map(|t| t.date).max()?;
            let mut labels: Vec<String> = Vec::new();
            for t in &transactions {
                let label = t.bank_label.trim().to_owned();
                if labels.len() < SAMPLE_LABELS && !labels.contains(&label) {
                    labels.push(label);
                }
            }
            let name = suggested_name(&key);
            let payee = existing_payee(&key, &name, assigned, payees);
            Some(PayeeSuggestion {
                transactions: transactions.iter().map(|t| t.id).collect(),
                key,
                name,
                payee,
                first_date,
                last_date,
                labels,
            })
        })
        .collect();
    result.sort_by(|a, b| b.transactions.len().cmp(&a.transactions.len()).then_with(|| a.key.cmp(&b.key)));
    result
}

/// Transactions sans tiers d'une clé, que la validation d'une proposition affecte (R-TIE-33).
pub fn with_key<T: Copy>(unassigned: &[Unassigned<T>], key: &str) -> Vec<T> {
    unassigned.iter().filter(|t| payee_key(&t.bank_label).as_deref() == Some(key)).map(|t| t.id).collect()
}

/// Tiers de la dernière transaction de même clé qui en a un ; à défaut, le tiers qui porte le nom suggéré, sans
/// distinction de casse (R-TIE-32). `assigned` est rangé de la plus récente à la plus ancienne.
fn existing_payee<P: Copy>(key: &str, name: &str, assigned: &[Assigned<P>], payees: &[NamedPayee<P>]) -> Option<P> {
    assigned
        .iter()
        .find(|a| payee_key(&a.bank_label).as_deref() == Some(key))
        .map(|a| a.payee)
        .or_else(|| payees.iter().find(|p| p.name.trim().to_lowercase() == name.to_lowercase()).map(|p| p.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::rules::{Conditions, LabelCondition, LabelField, LabelOp, Subject};
    use crate::shared::Money;
    use crate::shared::ids::AccountId;

    fn date(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn line(id: u32, day: &str, label: &str) -> Unassigned<u32> {
        Unassigned { id, date: date(day), bank_label: label.to_owned() }
    }

    fn run(unassigned: &[Unassigned<u32>]) -> Vec<PayeeSuggestion<u32, u8>> {
        suggestions(unassigned, &[], &[], &HashSet::new())
    }

    #[test]
    fn r_tie_30_operation_words_and_variable_parts_are_ignored() {
        assert_eq!(payee_key("CB CARREFOUR MEYLAN 14/03").as_deref(), Some("CARREFOUR MEYLAN"));
        assert_eq!(payee_key("PRLV SEPA Carrefour Meylan").as_deref(), Some("CARREFOUR MEYLAN"));
        assert_eq!(payee_key("CB AMAZON.FR*AB12CD34").as_deref(), Some("AMAZON FR"));
        assert_eq!(payee_key("CB 14/03 1234"), None);
        assert_eq!(payee_key(""), None);
    }

    #[test]
    fn r_tie_31_same_key_across_accounts_and_operation_types_is_grouped() {
        let result = run(&[
            line(1, "2026-03-14", "CB CARREFOUR MEYLAN 14/03"),
            line(2, "2026-04-02", "CB CARREFOUR MEYLAN 02/04"),
            line(3, "2026-03-20", "PRLV SEPA CARREFOUR MEYLAN"),
        ]);
        assert_eq!(result.len(), 1);
        let s = &result[0];
        assert_eq!(s.key, "CARREFOUR MEYLAN");
        assert_eq!(s.name, "Carrefour Meylan");
        assert_eq!(s.transactions, vec![2, 3, 1]);
        assert_eq!((s.first_date, s.last_date), (date("2026-03-14"), date("2026-04-02")));
        assert_eq!(
            s.labels,
            vec!["CB CARREFOUR MEYLAN 02/04", "PRLV SEPA CARREFOUR MEYLAN", "CB CARREFOUR MEYLAN 14/03"]
        );
    }

    #[test]
    fn r_tie_31_a_single_transaction_is_not_a_suggestion() {
        assert!(run(&[line(1, "2026-03-02", "CB BOULANGERIE DUPONT 02/03")]).is_empty());
    }

    #[test]
    fn r_tie_31_sorted_by_count_then_key_with_three_distinct_labels_at_most() {
        let result = run(&[
            line(1, "2026-01-01", "CB ZARA 01/01"),
            line(2, "2026-01-02", "CB ZARA 02/01"),
            line(3, "2026-01-03", "CB ZARA 03/01"),
            line(4, "2026-01-04", "CB ZARA 04/01"),
            line(5, "2026-01-05", "PRLV EDF"),
            line(6, "2026-02-05", "PRLV EDF"),
            line(7, "2026-01-05", "CB ALDI 05/01"),
            line(8, "2026-01-06", "CB ALDI 06/01"),
        ]);
        let keys: Vec<&str> = result.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, vec!["ZARA", "ALDI", "EDF"]);
        assert_eq!(result[0].labels.len(), 3);
        assert_eq!(result[2].labels, vec!["PRLV EDF"]);
    }

    #[test]
    fn r_tie_32_suggests_the_payee_of_the_latest_transaction_with_the_same_key() {
        let assigned = [
            Assigned { bank_label: "CB EDF 05/01".to_owned(), payee: 7_u8 },
            Assigned { bank_label: "PRLV SEPA EDF".to_owned(), payee: 9 },
        ];
        let lines = [line(1, "2026-02-05", "PRLV SEPA EDF"), line(2, "2026-03-05", "PRLV SEPA EDF")];
        let result = suggestions(&lines, &assigned, &[], &HashSet::new());
        assert_eq!(result[0].name, "Edf");
        assert_eq!(result[0].payee, Some(7));
    }

    #[test]
    fn r_tie_32_falls_back_on_a_payee_with_the_same_name() {
        let payees = [NamedPayee { id: 3_u8, name: "Carrefour MEYLAN".to_owned() }];
        let lines = [line(1, "2026-02-05", "CB CARREFOUR MEYLAN"), line(2, "2026-03-05", "CB CARREFOUR MEYLAN")];
        assert_eq!(suggestions(&lines, &[], &payees, &HashSet::new())[0].payee, Some(3));
        let other = [NamedPayee { id: 3_u8, name: "Carrefour".to_owned() }];
        assert_eq!(suggestions(&lines, &[], &other, &HashSet::new())[0].payee, None);
    }

    #[test]
    fn r_tie_32_name_is_capitalized_and_truncated() {
        assert_eq!(suggested_name("AMAZON FR"), "Amazon Fr");
        let long = ["ABCDEFGHIJ"; 10].join(" ");
        assert_eq!(suggested_name(&long).chars().count(), 80);
        // Une coupure juste après un mot ne laisse pas d'espace final.
        let words = ["ABCDEFGHI"; 9].join(" ");
        assert_eq!(suggested_name(&words).chars().count(), 79);
    }

    #[test]
    fn r_tie_34_rule_pattern_matches_the_words_in_order() {
        assert_eq!(rule_pattern("CARREFOUR MEYLAN"), "*CARREFOUR*MEYLAN*");
        assert_eq!(rule_pattern("EDF"), "*EDF*");
        let conditions = Conditions {
            label: Some(LabelCondition {
                op: LabelOp::Regex,
                value: rule_pattern(&payee_key("CB AMAZON.FR*AB12CD34").unwrap()),
                field: LabelField::BankLabel,
            }),
            ..Conditions::default()
        };
        let subject = |label| Subject {
            account: AccountId::from_uuid(uuid::Uuid::nil()),
            bank_label: label,
            label,
            amount: Money::from_minor(-1200),
        };
        assert!(conditions.matches(&subject("CB AMAZON.FR*ZZ99XX 03/05")));
        assert!(conditions.matches(&subject("PRLV SEPA Amazon Fr")));
        assert!(!conditions.matches(&subject("CB AMAZON PRIME")));
    }

    #[test]
    fn r_tie_33_validation_targets_every_unassigned_transaction_of_the_key() {
        let lines = [
            line(1, "2026-03-14", "CB CARREFOUR MEYLAN 14/03"),
            line(2, "2026-03-20", "PRLV SEPA CARREFOUR MEYLAN"),
            line(3, "2026-03-21", "CB CARREFOUR GRENOBLE 21/03"),
        ];
        assert_eq!(with_key(&lines, "CARREFOUR MEYLAN"), vec![1, 2]);
        assert!(with_key(&lines, "LIDL").is_empty());
    }

    #[test]
    fn r_tie_35_ignored_keys_are_not_suggested() {
        let ignored: HashSet<String> = ["FRAIS TENUE COMPTE".to_owned()].into();
        let lines = [line(1, "2026-01-31", "FRAIS TENUE COMPTE"), line(2, "2026-02-28", "FRAIS TENUE COMPTE")];
        assert!(suggestions::<u32, u8>(&lines, &[], &[], &ignored).is_empty());
    }
}
