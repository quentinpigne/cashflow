//! Clé d'apprentissage d'un libellé bancaire (R-IMP-13, R-TIE-17).

use super::fingerprint::normalize_label;

/// Mots qui décrivent le type d'opération, pas sa contrepartie : à eux seuls, ils ne désignent rien.
const OPERATION_WORDS: &[&str] = &[
    "CARTE",
    "CB",
    "DAB",
    "ECH",
    "EMIS",
    "EUR",
    "FACT",
    "FACTURE",
    "INST",
    "PAIEMENT",
    "PRELEVEMENT",
    "PRLV",
    "RECU",
    "RETRAIT",
    "SEPA",
    "VIR",
    "VIREMENT",
];

/// Libellé normalisé (R-IMP-5) sans ses parties variables : découpé en mots (tout caractère non
/// alphanumérique sépare), sans les mots qui contiennent un chiffre (dates, numéros, montants,
/// références). `None` s'il ne reste que des mots d'opération : la clé ne désignerait rien.
pub fn learning_key(label: &str) -> Option<String> {
    let normalized = normalize_label(label);
    let words: Vec<&str> = normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !w.chars().any(|c| c.is_ascii_digit()))
        .collect();
    words.iter().any(|w| !OPERATION_WORDS.contains(w)).then(|| words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(label: &str) -> Option<String> {
        learning_key(label)
    }

    #[test]
    fn r_imp_13_variable_parts_are_ignored() {
        assert_eq!(key("CB CARREFOUR MEYLAN 14/03"), key("CB Carrefour Meylan 02/04"));
        assert_eq!(key("CB CARREFOUR MEYLAN 14/03").as_deref(), Some("CB CARREFOUR MEYLAN"));
        assert_eq!(key("PRLV SEPA LIDL 05/09/26").as_deref(), Some("PRLV SEPA LIDL"));
        assert_eq!(key("CB AMAZON.FR*AB12CD34").as_deref(), Some("CB AMAZON FR"));
        assert_eq!(
            key("CB Carte Ticket Res 04/12/25\nMontrouge      EUR         3,72").as_deref(),
            Some("CB CARTE TICKET RES MONTROUGE EUR")
        );
    }

    #[test]
    fn r_imp_13_distinct_counterparts_stay_distinct() {
        assert_ne!(key("CB CARREFOUR MEYLAN 14/03"), key("CB CARREFOUR GRENOBLE 14/03"));
        assert_ne!(key("PRLV EDF"), key("PRLV SEPA EDF"));
    }

    #[test]
    fn r_tie_17_operation_words_alone_give_no_key() {
        assert_eq!(key("CB 14/03 1234"), None);
        assert_eq!(key("VIR SEPA 0012345"), None);
        assert_eq!(key("12/03"), None);
        assert_eq!(key(""), None);
    }
}
