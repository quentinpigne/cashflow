//! Devises ISO 4217 (ADR-0001).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::DomainError;

/// Code de devise ISO 4217 sur trois lettres majuscules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Currency([u8; 3]);

impl Currency {
    pub const EUR: Currency = Currency(*b"EUR");

    /// Nombre de décimales de la devise selon ISO 4217. Aucun code ne suppose « 2 » en dur.
    pub fn minor_digits(self) -> u32 {
        match &self.0 {
            b"BIF" | b"CLP" | b"DJF" | b"GNF" | b"ISK" | b"JPY" | b"KMF" | b"KRW" | b"PYG" | b"RWF" | b"UGX"
            | b"VND" | b"VUV" | b"XAF" | b"XOF" | b"XPF" => 0,
            b"BHD" | b"IQD" | b"JOD" | b"KWD" | b"LYD" | b"OMR" | b"TND" => 3,
            _ => 2,
        }
    }

    pub fn code(&self) -> &str {
        // Construit uniquement à partir de lettres ASCII majuscules.
        std::str::from_utf8(&self.0).unwrap_or("???")
    }
}

impl FromStr for Currency {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        match bytes {
            [a, b, c] if bytes.iter().all(u8::is_ascii_uppercase) => Ok(Currency([*a, *b, *c])),
            _ => Err(DomainError::InvalidCurrency(s.to_owned())),
        }
    }
}

impl TryFrom<String> for Currency {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Currency> for String {
    fn from(value: Currency) -> Self {
        value.code().to_owned()
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minor_digits_follow_iso_4217() {
        assert_eq!(Currency::EUR.minor_digits(), 2);
        assert_eq!("JPY".parse::<Currency>().unwrap().minor_digits(), 0);
        assert_eq!("TND".parse::<Currency>().unwrap().minor_digits(), 3);
    }

    #[test]
    fn rejects_invalid_codes() {
        assert!("eur".parse::<Currency>().is_err());
        assert!("EURO".parse::<Currency>().is_err());
    }
}
