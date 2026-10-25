//! Documents envoyés par l'utilisateur : fiches de paie, plus tard pièces jointes (ADR-0024).

use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Taille maximale d'un document, en octets (R-SAL-15).
pub const MAX_SIZE: usize = 10 * 1024 * 1024;

/// Type d'un document, reconnu à la signature de son contenu (ADR-0024).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum MediaType {
    #[serde(rename = "application/pdf")]
    Pdf,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    Png,
}

impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Pdf, Self::Jpeg, Self::Png].into_iter().find(|t| t.as_str() == value)
    }

    /// Reconnaît le type à la signature du contenu, pas à l'extension ni au type annoncé.
    pub fn sniff(content: &[u8]) -> Option<Self> {
        if content.starts_with(b"%PDF-") {
            Some(Self::Pdf)
        } else if content.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if content.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n']) {
            Some(Self::Png)
        } else {
            None
        }
    }
}

/// Vérifie un document envoyé : non vide, au plus 10 Mo, PDF, JPEG ou PNG (R-SAL-15).
pub fn check(content: &[u8]) -> Result<MediaType, DomainError> {
    if content.is_empty() {
        return Err(DomainError::EmptyDocument);
    }
    if content.len() > MAX_SIZE {
        return Err(DomainError::DocumentTooLarge);
    }
    MediaType::sniff(content).ok_or(DomainError::UnsupportedDocumentType)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_sal_15_type_is_recognized_from_content() {
        assert_eq!(check(b"%PDF-1.7\n..."), Ok(MediaType::Pdf));
        assert_eq!(check(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]), Ok(MediaType::Jpeg));
        assert_eq!(check(b"\x89PNG\r\n\x1a\n\0\0"), Ok(MediaType::Png));
        // Un document Word renommé en `.pdf` reste un zip.
        assert_eq!(check(b"PK\x03\x04word/document.xml"), Err(DomainError::UnsupportedDocumentType));
    }

    #[test]
    fn r_sal_15_document_is_not_empty_and_at_most_10_mb() {
        assert_eq!(check(b""), Err(DomainError::EmptyDocument));
        let mut big = b"%PDF-".to_vec();
        big.resize(MAX_SIZE, b' ');
        assert_eq!(check(&big), Ok(MediaType::Pdf));
        big.push(b' ');
        assert_eq!(check(&big), Err(DomainError::DocumentTooLarge));
    }

    #[test]
    fn media_type_round_trips() {
        for t in [MediaType::Pdf, MediaType::Jpeg, MediaType::Png] {
            assert_eq!(MediaType::parse(t.as_str()), Some(t));
        }
        assert_eq!(MediaType::parse("text/html"), None);
    }
}
