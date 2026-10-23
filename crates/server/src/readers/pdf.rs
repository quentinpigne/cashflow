//! Extraction locale du texte d'un PDF, avec la position de chaque élément (ADR-0007, R-IMP-23).
//!
//! Aucune donnée ne quitte le serveur. `pdf-extract` peut paniquer sur un PDF mal formé : la panique est
//! confinée ici et le fichier est refusé.

use std::panic::{self, AssertUnwindSafe};

use pdf_extract::{Document, MediaBox, OutputDev, OutputError, Transform};

use super::ReadError;

/// Un texte affiché d'un seul tenant sur une page, avec la position de ses caractères visibles.
#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    /// Page, à partir de 1.
    pub page: u32,
    /// Ordonnée de la ligne de base, en points depuis le bas de la page.
    pub y: f64,
    /// Abscisse du premier et du dernier caractère visible : un montant aligné à droite garde le même
    /// `last_x` quelle que soit sa longueur.
    pub first_x: f64,
    pub last_x: f64,
    /// Texte, espaces de bord retirés.
    pub text: String,
}

#[derive(Default)]
struct Collector {
    page: u32,
    items: Vec<TextItem>,
    text: String,
    first_x: Option<f64>,
    last_x: f64,
    y: f64,
}

impl Collector {
    fn flush(&mut self) {
        let text = std::mem::take(&mut self.text);
        if let Some(first_x) = self.first_x.take() {
            self.items.push(TextItem {
                page: self.page,
                y: self.y,
                first_x,
                last_x: self.last_x,
                text: text.trim().to_owned(),
            });
        }
    }
}

impl OutputDev for Collector {
    fn begin_page(&mut self, page: u32, _: &MediaBox, _: Option<(f64, f64, f64, f64)>) -> Result<(), OutputError> {
        self.page = page;
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        self.flush();
        Ok(())
    }

    fn output_character(&mut self, trm: &Transform, _: f64, _: f64, _: f64, char: &str) -> Result<(), OutputError> {
        if !char.trim().is_empty() {
            if self.first_x.is_none() {
                self.first_x = Some(trm.m31);
                self.y = trm.m32;
            }
            self.last_x = trm.m31;
        }
        self.text.push_str(char);
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn end_word(&mut self) -> Result<(), OutputError> {
        self.flush();
        Ok(())
    }

    fn end_line(&mut self) -> Result<(), OutputError> {
        self.flush();
        Ok(())
    }
}

/// Textes positionnés de toutes les pages. Un PDF illisible, chiffré ou sans texte (scanné) est refusé.
pub fn text_items(content: &[u8]) -> Result<Vec<TextItem>, ReadError> {
    let extracted = panic::catch_unwind(AssertUnwindSafe(|| {
        let document = Document::load_mem(content).map_err(|e| e.to_string())?;
        if document.is_encrypted() {
            return Err("PDF protégé par un mot de passe".to_owned());
        }
        let mut collector = Collector::default();
        pdf_extract::output_doc(&document, &mut collector).map_err(|e| e.to_string())?;
        collector.flush();
        Ok(collector.items)
    }));
    let items = match extracted {
        Ok(result) => result.map_err(|e| ReadError::Unreadable(format!("PDF illisible ({e})")))?,
        Err(_) => return Err(ReadError::Unreadable("PDF illisible".into())),
    };
    if items.is_empty() {
        return Err(ReadError::Unreadable(
            "aucun texte dans ce PDF : un relevé scanné demande l'OCR, pas encore disponible (IMP-12)".into(),
        ));
    }
    Ok(items)
}
