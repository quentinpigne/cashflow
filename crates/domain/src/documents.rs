//! Bibliothèque de documents (spec 12, ADR-0024) : fichier, description, rattachements.

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::{Date, DomainError};

/// Taille maximale d'un document, en octets (R-DOC-1).
pub const MAX_SIZE: usize = 10 * 1024 * 1024;

/// Longueur maximale d'un titre, en caractères (R-DOC-4).
pub const MAX_TITLE: usize = 120;

/// Type de contenu d'un document, reconnu à la signature de son contenu (R-DOC-1).
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

/// Vérifie un fichier envoyé : non vide, au plus 10 Mo, PDF, JPEG ou PNG (R-DOC-1).
pub fn check(content: &[u8]) -> Result<MediaType, DomainError> {
    if content.is_empty() {
        return Err(DomainError::EmptyDocument);
    }
    if content.len() > MAX_SIZE {
        return Err(DomainError::DocumentTooLarge);
    }
    MediaType::sniff(content).ok_or(DomainError::UnsupportedDocumentType)
}

/// Type d'un document (R-DOC-5). Un document sans type est « à classer ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocumentKind {
    BankStatement,
    Payslip,
    Contract,
    Receipt,
    Invoice,
    Tax,
    Certificate,
    Letter,
    Other,
}

impl DocumentKind {
    pub const ALL: [Self; 9] = [
        Self::BankStatement,
        Self::Payslip,
        Self::Contract,
        Self::Receipt,
        Self::Invoice,
        Self::Tax,
        Self::Certificate,
        Self::Letter,
        Self::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::BankStatement => "BANK_STATEMENT",
            Self::Payslip => "PAYSLIP",
            Self::Contract => "CONTRACT",
            Self::Receipt => "RECEIPT",
            Self::Invoice => "INVOICE",
            Self::Tax => "TAX",
            Self::Certificate => "CERTIFICATE",
            Self::Letter => "LETTER",
            Self::Other => "OTHER",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == value)
    }

    /// Objet auquel un document de ce type est rattaché d'office, et une seule fois : le compte d'un relevé,
    /// l'emploi d'une fiche de paie (R-DOC-10). Ces types ont aussi un mois (R-DOC-7).
    pub fn owner(self) -> Option<LinkKind> {
        match self {
            Self::BankStatement => Some(LinkKind::Account),
            Self::Payslip => Some(LinkKind::Employment),
            _ => None,
        }
    }
}

/// Nature de l'objet auquel un document est rattaché (R-DOC-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LinkKind {
    Account,
    Institution,
    Employment,
    Transaction,
    Asset,
    Loan,
    Payee,
    Project,
}

impl LinkKind {
    pub const ALL: [Self; 8] = [
        Self::Account,
        Self::Institution,
        Self::Employment,
        Self::Transaction,
        Self::Asset,
        Self::Loan,
        Self::Payee,
        Self::Project,
    ];
}

/// Objet auquel un document est rattaché : sa nature et son identifiant (R-DOC-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct LinkTarget {
    pub kind: LinkKind,
    #[cfg_attr(feature = "openapi", schema(value_type = String, format = Uuid))]
    pub id: uuid::Uuid,
}

/// Description d'un document : tout ce qui se modifie, le fichier étant immuable (R-DOC-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    pub kind: Option<DocumentKind>,
    pub title: String,
    pub document_date: Option<Date>,
    pub period_start: Option<Date>,
    pub period_end: Option<Date>,
    pub month: Option<Date>,
    pub note: Option<String>,
}

/// Vérifie une description (R-DOC-4 à R-DOC-7) ; un relevé est un PDF (R-DOC-1).
pub fn check_description(description: &Description, media_type: MediaType) -> Result<(), DomainError> {
    let length = description.title.trim().chars().count();
    if length == 0 || length > MAX_TITLE {
        return Err(DomainError::InvalidDocumentTitle);
    }
    if let (Some(start), Some(end)) = (description.period_start, description.period_end)
        && start > end
    {
        return Err(DomainError::InvalidDocumentPeriod);
    }
    let has_month = description.kind.and_then(DocumentKind::owner).is_some();
    match description.month {
        Some(month) if !has_month || month.day() != 1 => return Err(DomainError::InvalidDocumentMonth),
        None if has_month => return Err(DomainError::InvalidDocumentMonth),
        _ => {}
    }
    if description.kind == Some(DocumentKind::BankStatement) && media_type != MediaType::Pdf {
        return Err(DomainError::StatementNotPdf);
    }
    Ok(())
}

/// Vérifie le rattachement d'office d'un relevé ou d'une fiche de paie (R-DOC-10) : jamais plus d'un ; exactement
/// un quand `required`, à la création du document ou au changement de son type. Un relevé dont le compte a été
/// supprimé garde son type sans compte.
pub fn check_owner(kind: Option<DocumentKind>, links: &[LinkKind], required: bool) -> Result<(), DomainError> {
    let Some(owner) = kind.and_then(DocumentKind::owner) else {
        return Ok(());
    };
    match links.iter().filter(|&&l| l == owner).count() {
        0 if required => Err(DomainError::DocumentOwnerRequired),
        0 | 1 => Ok(()),
        _ => Err(DomainError::DocumentOwnerNotUnique),
    }
}

/// Date de référence (R-DOC-8) : la date du document, à défaut son mois, à défaut le début de sa période, à défaut
/// sa date d'envoi.
pub fn reference_date(description: &Description, sent_on: Date) -> Date {
    description.document_date.or(description.month).or(description.period_start).unwrap_or(sent_on)
}

/// Titre par défaut : le nom du fichier sans son extension (R-DOC-4).
pub fn default_title(file_name: &str) -> String {
    let stem = match file_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.trim().is_empty() && extension.len() <= 5 => stem,
        _ => file_name,
    };
    let title: String = stem.trim().chars().take(MAX_TITLE).collect();
    if title.trim().is_empty() { "Document".to_owned() } else { title.trim().to_owned() }
}

const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// Mois en toutes lettres : « septembre 2026 ».
pub fn month_label(month: Date) -> String {
    let name = MONTHS.get(month.month0() as usize).copied().unwrap_or_default();
    format!("{name} {}", month.year())
}

/// Titre d'un relevé ou d'une fiche de paie, d'après son compte ou son employeur et son mois (R-DOC-15) :
/// « Relevé Compte courant septembre 2026 », « Fiche de paie Acme août 2026 ».
pub fn owned_title(kind: DocumentKind, owner_name: &str, month: Date) -> String {
    let prefix = match kind {
        DocumentKind::Payslip => "Fiche de paie",
        _ => "Relevé",
    };
    format!("{prefix} {owner_name} {}", month_label(month)).chars().take(MAX_TITLE).collect()
}

// ---------------------------------------------------------------- Recherche (R-DOC-18)

/// Texte sans casse ni accents, pour comparer comme l'index (tokeniseur `unicode61 remove_diacritics 2`).
pub fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => folded.push('a'),
            'ç' => folded.push('c'),
            'è' | 'é' | 'ê' | 'ë' => folded.push('e'),
            'ì' | 'í' | 'î' | 'ï' => folded.push('i'),
            'ñ' => folded.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => folded.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => folded.push('u'),
            'ý' | 'ÿ' => folded.push('y'),
            'œ' => folded.push_str("oe"),
            'æ' => folded.push_str("ae"),
            _ => folded.push(c),
        }
    }
    folded
}

/// Mots cherchés : le texte sans casse ni accents, découpé sur tout ce qui n'est ni lettre ni chiffre.
pub fn search_words(text: &str) -> Vec<String> {
    fold(text).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

/// Requête FTS5 qui trouve un mot par son début (`"assur"*`), sans syntaxe venue de l'utilisateur.
pub fn prefix_query(word: &str) -> String {
    format!("\"{}\"*", word.replace('"', ""))
}

/// Un nom contient-il un mot qui commence par `word` (déjà sans casse ni accents) ?
pub fn name_matches(name: &str, word: &str) -> bool {
    search_words(name).iter().any(|w| w.starts_with(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_doc_18_search_ignores_case_and_accents_and_matches_word_starts() {
        assert_eq!(search_words("Relevé  d'ÉPARGNE, 2026"), ["releve", "d", "epargne", "2026"]);
        assert_eq!(fold("Œuvre Noël"), "oeuvre noel");
        assert!(name_matches("Garage Dupont", "dup"));
        assert!(name_matches("Crédit Agricole", "credit"));
        assert!(!name_matches("Garage Dupont", "pont"));
        assert_eq!(prefix_query("assur"), "\"assur\"*");
        assert_eq!(prefix_query("a\"b"), "\"ab\"*");
    }

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn description(kind: Option<DocumentKind>) -> Description {
        Description {
            kind,
            title: "Assurance habitation".to_owned(),
            document_date: None,
            period_start: None,
            period_end: None,
            month: None,
            note: None,
        }
    }

    #[test]
    fn r_doc_1_type_is_recognized_from_content() {
        assert_eq!(check(b"%PDF-1.7\n..."), Ok(MediaType::Pdf));
        assert_eq!(check(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]), Ok(MediaType::Jpeg));
        assert_eq!(check(b"\x89PNG\r\n\x1a\n\0\0"), Ok(MediaType::Png));
        // Un document Word renommé en `.pdf` reste un zip.
        assert_eq!(check(b"PK\x03\x04word/document.xml"), Err(DomainError::UnsupportedDocumentType));
    }

    #[test]
    fn r_doc_1_document_is_not_empty_and_at_most_10_mb() {
        assert_eq!(check(b""), Err(DomainError::EmptyDocument));
        let mut big = b"%PDF-".to_vec();
        big.resize(MAX_SIZE, b' ');
        assert_eq!(check(&big), Ok(MediaType::Pdf));
        big.push(b' ');
        assert_eq!(check(&big), Err(DomainError::DocumentTooLarge));
    }

    #[test]
    fn r_doc_1_bank_statement_is_a_pdf() {
        let mut statement = description(Some(DocumentKind::BankStatement));
        statement.month = Some(d("2026-09-01"));
        assert_eq!(check_description(&statement, MediaType::Pdf), Ok(()));
        assert_eq!(check_description(&statement, MediaType::Jpeg), Err(DomainError::StatementNotPdf));
        let mut payslip = statement.clone();
        payslip.kind = Some(DocumentKind::Payslip);
        assert_eq!(check_description(&payslip, MediaType::Png), Ok(()));
    }

    #[test]
    fn media_type_and_kind_round_trip() {
        for t in [MediaType::Pdf, MediaType::Jpeg, MediaType::Png] {
            assert_eq!(MediaType::parse(t.as_str()), Some(t));
        }
        assert_eq!(MediaType::parse("text/html"), None);
        for k in DocumentKind::ALL {
            assert_eq!(DocumentKind::parse(k.as_str()), Some(k));
        }
    }

    #[test]
    fn r_doc_4_title_has_1_to_120_characters_and_defaults_to_the_file_name() {
        let mut doc = description(None);
        assert_eq!(check_description(&doc, MediaType::Pdf), Ok(()));
        doc.title = "  ".to_owned();
        assert_eq!(check_description(&doc, MediaType::Pdf), Err(DomainError::InvalidDocumentTitle));
        doc.title = "é".repeat(120);
        assert_eq!(check_description(&doc, MediaType::Pdf), Ok(()));
        doc.title.push('é');
        assert_eq!(check_description(&doc, MediaType::Pdf), Err(DomainError::InvalidDocumentTitle));

        assert_eq!(default_title("Contrat MAIF.pdf"), "Contrat MAIF");
        assert_eq!(default_title("IMG_2041.JPEG"), "IMG_2041");
        assert_eq!(default_title("relevé.2026.09.pdf"), "relevé.2026.09");
        assert_eq!(default_title(".pdf"), ".pdf");
        assert_eq!(default_title("   "), "Document");
    }

    #[test]
    fn r_doc_6_period_bounds_are_independent_and_ordered() {
        let mut contract = description(Some(DocumentKind::Contract));
        contract.period_start = Some(d("2026-01-01"));
        assert_eq!(check_description(&contract, MediaType::Pdf), Ok(()), "un CDI n'a pas de fin");
        contract.period_end = Some(d("2025-12-31"));
        assert_eq!(check_description(&contract, MediaType::Pdf), Err(DomainError::InvalidDocumentPeriod));
    }

    #[test]
    fn r_doc_7_month_belongs_to_statements_and_payslips_only() {
        let mut payslip = description(Some(DocumentKind::Payslip));
        assert_eq!(check_description(&payslip, MediaType::Pdf), Err(DomainError::InvalidDocumentMonth));
        payslip.month = Some(d("2026-08-15"));
        assert_eq!(check_description(&payslip, MediaType::Pdf), Err(DomainError::InvalidDocumentMonth));
        payslip.month = Some(d("2026-08-01"));
        assert_eq!(check_description(&payslip, MediaType::Pdf), Ok(()));
        let mut contract = payslip.clone();
        contract.kind = Some(DocumentKind::Contract);
        assert_eq!(check_description(&contract, MediaType::Pdf), Err(DomainError::InvalidDocumentMonth));
        contract.kind = None;
        assert_eq!(check_description(&contract, MediaType::Pdf), Err(DomainError::InvalidDocumentMonth));
    }

    #[test]
    fn r_doc_8_reference_date_falls_back_from_date_to_month_to_period_to_sending() {
        let sent = d("2026-10-29");
        let mut doc = description(Some(DocumentKind::Contract));
        assert_eq!(reference_date(&doc, sent), sent);
        doc.period_start = Some(d("2026-01-01"));
        assert_eq!(reference_date(&doc, sent), d("2026-01-01"));
        doc.month = Some(d("2026-08-01"));
        assert_eq!(reference_date(&doc, sent), d("2026-08-01"));
        doc.document_date = Some(d("2025-12-19"));
        assert_eq!(reference_date(&doc, sent), d("2025-12-19"));
    }

    #[test]
    fn r_doc_10_statement_has_one_account_and_payslip_one_employment() {
        use LinkKind::*;
        let statement = Some(DocumentKind::BankStatement);
        assert_eq!(check_owner(statement, &[Account, Payee], true), Ok(()));
        assert_eq!(check_owner(statement, &[Payee], true), Err(DomainError::DocumentOwnerRequired));
        assert_eq!(check_owner(statement, &[Payee], false), Ok(()), "compte supprimé");
        assert_eq!(check_owner(statement, &[Account, Account], false), Err(DomainError::DocumentOwnerNotUnique));
        let payslip = Some(DocumentKind::Payslip);
        assert_eq!(check_owner(payslip, &[Account], true), Err(DomainError::DocumentOwnerRequired));
        assert_eq!(check_owner(payslip, &[Employment, Payee], true), Ok(()));
        assert_eq!(check_owner(Some(DocumentKind::Contract), &[Account, Account], true), Ok(()));
        assert_eq!(check_owner(None, &[], true), Ok(()));
    }

    #[test]
    fn r_doc_15_statement_and_payslip_titles() {
        assert_eq!(
            owned_title(DocumentKind::BankStatement, "Compte courant", d("2026-09-01")),
            "Relevé Compte courant septembre 2026"
        );
        assert_eq!(owned_title(DocumentKind::Payslip, "Acme", d("2026-08-01")), "Fiche de paie Acme août 2026");
    }
}
