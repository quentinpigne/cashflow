//! Validation et masquage des IBAN (R-CPT-8).

use crate::shared::DomainError;

/// IBAN normalisé : majuscules, sans espaces, clé de contrôle vérifiée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iban(String);

impl Iban {
    pub fn parse(input: &str) -> Result<Self, DomainError> {
        let iban: String = input.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
        let bytes = iban.as_bytes();
        let well_formed = bytes.len() >= 15
            && bytes.len() <= 34
            && bytes[..2].iter().all(u8::is_ascii_uppercase)
            && bytes[2..4].iter().all(u8::is_ascii_digit)
            && bytes[4..].iter().all(u8::is_ascii_alphanumeric);
        if !well_formed {
            return Err(DomainError::InvalidIban);
        }
        if expected_length(&iban[..2]).is_some_and(|len| len != iban.len()) {
            return Err(DomainError::InvalidIban);
        }
        if mod97(&iban) != 1 {
            return Err(DomainError::InvalidIban);
        }
        Ok(Iban(iban))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Forme affichée par défaut : `FR76 •••• •••• •••• •••• •••4 21`.
    pub fn masked(&self) -> String {
        let len = self.0.len();
        let chars = self.0.chars().enumerate().map(|(i, c)| if i < 4 || i >= len - 3 { c } else { '•' });
        group_by_four(chars)
    }

    /// Forme lisible complète, par groupes de quatre.
    pub fn formatted(&self) -> String {
        group_by_four(self.0.chars())
    }
}

fn group_by_four(chars: impl Iterator<Item = char>) -> String {
    let mut out = String::new();
    for (i, c) in chars.enumerate() {
        if i > 0 && i % 4 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// Reste modulo 97 de l'IBAN réarrangé (les 4 premiers caractères à la fin, lettres → 10..35).
fn mod97(iban: &str) -> u32 {
    let rearranged = iban[4..].chars().chain(iban[..4].chars());
    let mut remainder: u32 = 0;
    for c in rearranged {
        let value = c.to_digit(36).unwrap_or(0);
        remainder = if value >= 10 { (remainder * 100 + value) % 97 } else { (remainder * 10 + value) % 97 };
    }
    remainder
}

/// Longueur imposée par pays, pour les pays courants. Les autres sont contrôlés par la seule clé.
fn expected_length(country: &str) -> Option<usize> {
    Some(match country {
        "FR" | "MC" | "IT" | "SM" => 27,
        "DE" => 22,
        "BE" => 16,
        "ES" => 24,
        "LU" => 20,
        "CH" | "LI" => 21,
        "NL" => 18,
        "PT" => 25,
        "GB" | "IE" => 22,
        "AT" => 20,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = "FR76 3000 6000 0112 3456 7890 189";

    #[test]
    fn r_cpt_8_accepts_valid_iban() {
        let iban = Iban::parse(VALID).unwrap();
        assert_eq!(iban.as_str(), "FR7630006000011234567890189");
        assert_eq!(iban.formatted(), VALID);
    }

    #[test]
    fn r_cpt_8_rejects_bad_checksum_or_length() {
        assert_eq!(Iban::parse("FR76 3000 6000 0112 3456 7890 188"), Err(DomainError::InvalidIban));
        assert_eq!(Iban::parse("FR76 3000 6000 0112 3456 7890 18"), Err(DomainError::InvalidIban));
        assert_eq!(Iban::parse("not an iban"), Err(DomainError::InvalidIban));
    }

    #[test]
    fn r_cpt_8_masks_all_but_country_key_and_last_three() {
        assert_eq!(Iban::parse(VALID).unwrap().masked(), "FR76 •••• •••• •••• •••• •••• 189");
    }
}
