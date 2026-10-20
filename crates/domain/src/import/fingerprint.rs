//! Libellés normalisés et empreintes des lignes (R-IMP-5).

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::shared::ids::AccountId;
use crate::shared::{Date, Money};

/// Majuscules, sans accents, espaces multiples réduits, bords rognés (R-IMP-5).
pub fn normalize_label(label: &str) -> String {
    let upper: String = label.chars().flat_map(fold).collect::<String>().to_uppercase();
    upper.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lettre sans accent ; les ligatures deviennent deux lettres.
fn fold(c: char) -> Vec<char> {
    match c {
        'œ' => return vec!['o', 'e'],
        'Œ' => return vec!['O', 'E'],
        'æ' => return vec!['a', 'e'],
        'Æ' => return vec!['A', 'E'],
        _ => {}
    }
    vec![strip_accent(c)]
}

fn strip_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'ç' => 'c',
        'Ç' => 'C',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        other => other,
    }
}

fn sha256_hex(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// Ligne lue, vue par le calcul d'empreinte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineKey {
    pub date: Date,
    pub amount: Money,
    pub bank_label: String,
}

/// Empreintes des lignes valides d'un fichier, dans l'ordre (R-IMP-5) : le rang d'occurrence départage
/// les lignes identiques d'un même fichier, et réimporter le fichier redonne les mêmes empreintes.
pub fn fingerprints(account: AccountId, lines: &[LineKey]) -> Vec<String> {
    let mut seen: HashMap<(Date, Money, String), u32> = HashMap::new();
    lines
        .iter()
        .map(|line| {
            let label = normalize_label(&line.bank_label);
            let occurrence = seen.entry((line.date, line.amount, label.clone())).or_insert(0);
            let print = sha256_hex(&format!("{account}|{}|{}|{label}|{occurrence}", line.date, line.amount.minor()));
            *occurrence += 1;
            print
        })
        .collect()
}

/// Empreinte d'une ligne OFX : l'identifiant de transaction fourni par la banque (R-IMP-5).
pub fn ofx_fingerprint(account: AccountId, fitid: &str) -> String {
    sha256_hex(&format!("{account}|{fitid}"))
}

/// Empreinte d'un fichier, pour l'avertissement de réimport (R-IMP-7).
pub fn file_sha256(content: &[u8]) -> String {
    hex::encode(Sha256::digest(content))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(date: &str, minor: i64, label: &str) -> LineKey {
        LineKey { date: date.parse().unwrap(), amount: Money::from_minor(minor), bank_label: label.into() }
    }

    #[test]
    fn r_imp_5_normalization() {
        assert_eq!(normalize_label("  Café   de la  Gare "), "CAFE DE LA GARE");
        assert_eq!(normalize_label("prélèvement Œuvre"), "PRELEVEMENT OEUVRE");
    }

    #[test]
    fn r_imp_5_identical_lines_get_distinct_but_stable_prints() {
        let account = AccountId::new();
        let lines =
            [key("2026-03-10", -250, "CAFE"), key("2026-03-10", -250, "café "), key("2026-03-11", -250, "CAFE")];
        let first = fingerprints(account, &lines);
        assert_ne!(first[0], first[1]);
        assert_eq!(first, fingerprints(account, &lines), "réimporter redonne les mêmes empreintes");
        assert_ne!(first, fingerprints(AccountId::new(), &lines), "l'empreinte dépend du compte");
    }
}
