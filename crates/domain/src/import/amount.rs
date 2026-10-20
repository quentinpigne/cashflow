//! Lecture des montants depuis le texte, sans flottant (R-IMP-2).

use crate::shared::{Currency, Money};

/// Séparateurs d'un format de montant (profil d'import).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmountFormat {
    pub decimal: char,
    /// `None` : aucun séparateur de milliers.
    pub thousands: Option<char>,
}

impl AmountFormat {
    pub const FRENCH: AmountFormat = AmountFormat { decimal: ',', thousands: Some(' ') };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AmountError {
    #[error("montant vide")]
    Empty,
    #[error("montant illisible : « {0} »")]
    Unreadable(String),
    #[error("trop de décimales : « {0} »")]
    TooManyDecimals(String),
}

fn is_space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{a0}' | '\u{202f}' | '\u{2009}')
}

/// Convertit un montant écrit (« 1 234,56 », « -0,5 », « (12,00) », « 12,00- », « +3 € ») en unités
/// mineures de `currency`, sans passer par un flottant (R-IMP-2).
pub fn parse_amount(raw: &str, format: AmountFormat, currency: Currency) -> Result<Money, AmountError> {
    let digits = usize::try_from(currency.minor_digits()).unwrap_or(2);
    let mut text: String = raw.chars().filter(|c| !is_space(*c)).collect();
    for symbol in ["€", "EUR", "eur", currency.code()] {
        text = text.replace(symbol, "");
    }
    if text.is_empty() {
        return Err(AmountError::Empty);
    }
    let mut negative = false;
    if text.starts_with('(') && text.ends_with(')') {
        negative = true;
        text = text[1..text.len() - 1].to_owned();
    }
    for minus in ['-', '−'] {
        if let Some(rest) = text.strip_prefix(minus) {
            negative = !negative;
            text = rest.to_owned();
        } else if let Some(rest) = text.strip_suffix(minus) {
            negative = !negative;
            text = rest.to_owned();
        }
    }
    if let Some(rest) = text.strip_prefix('+') {
        text = rest.to_owned();
    }
    if let Some(thousands) = format.thousands.filter(|t| !is_space(*t)) {
        text = text.replace(thousands, "");
    }
    let (integer, fraction) = match text.split_once(format.decimal) {
        Some((i, f)) => (i, f),
        None => (text.as_str(), ""),
    };
    let valid = !integer.is_empty() || !fraction.is_empty();
    if !valid || !integer.chars().all(|c| c.is_ascii_digit()) || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return Err(AmountError::Unreadable(raw.to_owned()));
    }
    if fraction.len() > digits {
        return Err(AmountError::TooManyDecimals(raw.to_owned()));
    }
    let padded = format!("{integer}{fraction:0<digits$}");
    let minor: i64 = padded.parse().map_err(|_| AmountError::Unreadable(raw.to_owned()))?;
    Ok(Money::from_minor(if negative { -minor } else { minor }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Result<i64, AmountError> {
        parse_amount(raw, AmountFormat::FRENCH, Currency::EUR).map(Money::minor)
    }

    #[test]
    fn r_imp_2_examples_from_spec() {
        assert_eq!(parse("1 234,56"), Ok(123_456));
        assert_eq!(parse("-0,5"), Ok(-50));
        assert_eq!(parse("(12,00)"), Ok(-1_200));
    }

    #[test]
    fn r_imp_2_other_writings() {
        assert_eq!(parse("1\u{a0}234,56 €"), Ok(123_456));
        assert_eq!(parse("12,00-"), Ok(-1_200));
        assert_eq!(parse("+3"), Ok(300));
        assert_eq!(parse(",5"), Ok(50));
        let dot = AmountFormat { decimal: '.', thousands: Some(',') };
        assert_eq!(parse_amount("1,234.5", dot, Currency::EUR).map(Money::minor), Ok(123_450));
        let apostrophe = AmountFormat { decimal: '.', thousands: Some('\'') };
        assert_eq!(parse_amount("1'234.50", apostrophe, Currency::EUR).map(Money::minor), Ok(123_450));
    }

    #[test]
    fn r_imp_2_errors() {
        assert_eq!(parse(""), Err(AmountError::Empty));
        assert!(matches!(parse("abc"), Err(AmountError::Unreadable(_))));
        assert!(matches!(parse("1,234"), Err(AmountError::TooManyDecimals(_))));
        assert!(matches!(parse("1,2,3"), Err(AmountError::Unreadable(_))));
    }
}
