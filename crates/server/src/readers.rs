//! Lecteurs de relevés (spec 03) : chaque format produit la même sortie, un [`Statement`].
//!
//! Ajouter un format revient à ajouter un lecteur ; le reste du pipeline ne change pas. L'OCR (IMP-12,
//! ADR-0007) s'ajoutera ici derrière le même trait.

pub mod csv;
pub mod lcl_pdf;
pub mod ofx;
pub mod pdf;
pub mod profile;
pub mod xlsx;

use cashflow_domain::{Currency, Date, Money};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Format d'un fichier de relevé (modèle de données, `ImportProfile.format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileFormat {
    Csv,
    Xlsx,
    Ofx,
    /// PDF à texte extractible, lu par un parseur dédié à la banque (IMP-10).
    Pdf,
    PdfOcr,
}

impl FileFormat {
    /// Format déduit du nom et du contenu du fichier.
    pub fn detect(file_name: &str, content: &[u8]) -> Option<Self> {
        let lower = file_name.to_lowercase();
        let head = String::from_utf8_lossy(&content[..content.len().min(512)]).to_uppercase();
        if lower.ends_with(".pdf") || content.starts_with(b"%PDF") {
            Some(FileFormat::Pdf)
        } else if lower.ends_with(".ofx")
            || lower.ends_with(".qfx")
            || head.contains("OFXHEADER")
            || head.contains("<OFX>")
        {
            Some(FileFormat::Ofx)
        } else if lower.ends_with(".xlsx")
            || lower.ends_with(".xlsm")
            || lower.ends_with(".xls")
            || content.starts_with(b"PK")
        {
            Some(FileFormat::Xlsx)
        } else if lower.ends_with(".csv") || lower.ends_with(".txt") || std::str::from_utf8(content).is_ok() {
            Some(FileFormat::Csv)
        } else {
            None
        }
    }
}

/// Une ligne lue et normalisée, avec ses éventuelles erreurs (R-IMP-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadLine {
    /// Position dans le fichier source (1 = première ligne).
    pub line_number: i64,
    /// Cellules d'origine, pour la traçabilité.
    pub raw: Vec<String>,
    pub date: Option<Date>,
    pub value_date: Option<Date>,
    pub amount: Option<Money>,
    pub bank_label: String,
    /// Identifiant fourni par la banque (OFX `FITID`) : il sert d'empreinte (R-IMP-5).
    pub fitid: Option<String>,
    /// Catégorie fournie par la banque, si le profil la lit.
    pub bank_category: Option<String>,
    /// Date d'achat d'un paiement par carte, lue dans le libellé (R-IMP-26).
    pub purchase_date: Option<Date>,
    pub errors: Vec<String>,
}

impl ReadLine {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty() && self.date.is_some() && self.amount.is_some() && !self.bank_label.is_empty()
    }
}

/// Résultat de la lecture d'un relevé.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statement {
    pub lines: Vec<ReadLine>,
    pub period: Option<(Date, Date)>,
    pub opening_balance: Option<Money>,
    pub closing_balance: Option<Money>,
    /// Date du solde de fin, si le relevé la donne (OFX `DTASOF`) ; sinon la fin de période.
    pub closing_date: Option<Date>,
}

impl Statement {
    /// Période déduite des lignes quand le fichier ne la donne pas.
    pub fn fill_period(&mut self) {
        if self.period.is_none() {
            let dates = self.lines.iter().filter_map(|l| l.date);
            let (min, max) = dates.fold((None, None), |(min, max): (Option<Date>, Option<Date>), d| {
                (Some(min.map_or(d, |m| m.min(d))), Some(max.map_or(d, |m| m.max(d))))
            });
            self.period = min.zip(max);
        }
    }
}

/// Contexte de lecture.
#[derive(Debug, Clone, Copy)]
pub struct ReadContext {
    pub today: Date,
    pub currency: Currency,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("fichier illisible : {0}")]
    Unreadable(String),
    #[error("réglage d'import invalide : {0}")]
    Settings(String),
}

/// Lecteur d'un format de relevé.
pub trait StatementReader {
    fn read(&self, content: &[u8], context: ReadContext) -> Result<Statement, ReadError>;
}

/// Cellule d'un tableau (CSV ou XLSX) avant application du profil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Text(String),
    /// Date reconnue par le tableur.
    Date(Date),
    /// Nombre du tableur, déjà écrit en décimal avec un point (« 1234.56 »).
    Number(String),
}

impl Cell {
    pub fn text(&self) -> String {
        match self {
            Cell::Text(t) | Cell::Number(t) => t.clone(),
            Cell::Date(d) => d.to_string(),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Cell::Text(t) if t.trim().is_empty())
    }
}

/// Tableau brut lu d'un fichier CSV ou XLSX.
pub type Table = Vec<Vec<Cell>>;
