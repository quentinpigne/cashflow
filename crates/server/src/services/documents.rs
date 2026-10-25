//! Documents archivés sur le disque (ADR-0024) : écriture en deux temps, lecture, suppression différée.

use std::path::PathBuf;

use anyhow::Context;
use cashflow_domain::Date;
use cashflow_domain::documents;
use cashflow_domain::shared::ids::DocumentId;
use sha2::{Digest, Sha256};

use crate::clock::timestamp;
use crate::config::DataDir;
use crate::db::documents::DocumentRow;
use crate::error::{AppError, AppResult};

/// Durée de séjour d'un document supprimé dans la corbeille : plus que la rétention des sauvegardes (12 mois).
const TRASH_DAYS: i64 = 400;
const TRASH_DATE: &str = "%Y%m%d";

/// Nom d'origine conservé, sans chemin, borné à 255 caractères.
fn file_name(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    let name: String = name.chars().filter(|c| !c.is_control()).take(255).collect();
    if name.is_empty() { "document".to_owned() } else { name }
}

/// Document vérifié et écrit sous un nom temporaire, en attente de sa ligne en base. Abandonné (`drop`) sans
/// `publish`, il ne laisse aucun fichier.
pub struct Staged {
    pub row: DocumentRow,
    temp: PathBuf,
    target: PathBuf,
}

/// Vérifie le contenu (R-SAL-15) et l'écrit sous un nom temporaire.
pub fn stage(data: &DataDir, name: &str, content: &[u8]) -> AppResult<Staged> {
    let media_type = documents::check(content)?;
    let id = DocumentId::new();
    let row = DocumentRow {
        id,
        file_name: file_name(name),
        media_type,
        size_bytes: i64::try_from(content.len()).map_err(anyhow::Error::from)?,
        sha256: hex::encode(Sha256::digest(content)),
        created_at: timestamp(),
    };
    std::fs::create_dir_all(data.documents()).with_context(|| format!("création de {}", data.documents().display()))?;
    let temp = data.documents().join(format!(".{id}.tmp"));
    std::fs::write(&temp, content).with_context(|| format!("écriture de {}", temp.display()))?;
    Ok(Staged { row, temp, target: data.documents().join(id.to_string()) })
}

impl Staged {
    /// Rend le fichier visible, une fois sa ligne enregistrée.
    pub fn publish(self) -> anyhow::Result<()> {
        std::fs::rename(&self.temp, &self.target).with_context(|| format!("écriture de {}", self.target.display()))
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        // Après `publish`, le fichier temporaire n'existe plus : rien à faire.
        let _ = std::fs::remove_file(&self.temp);
    }
}

/// Contenu d'un document.
pub fn read(data: &DataDir, id: DocumentId) -> AppResult<Vec<u8>> {
    let path = data.documents().join(id.to_string());
    match std::fs::read(&path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(AppError::NotFound("fichier du document")),
        Err(error) => Err(anyhow::Error::from(error).context(format!("lecture de {}", path.display())).into()),
    }
}

/// Déplace les fichiers de documents supprimés en base dans la corbeille, préfixés de la date du jour. La base
/// est déjà à jour : un échec est journalisé, pas remonté.
pub fn trash(data: &DataDir, ids: &[DocumentId], today: Date) {
    for id in ids {
        let from = data.documents().join(id.to_string());
        let to = data.documents_trash().join(format!("{}-{id}", today.format(TRASH_DATE)));
        let moved = std::fs::create_dir_all(data.documents_trash()).and_then(|()| std::fs::rename(&from, &to));
        if let Err(error) = moved {
            tracing::error!(%error, file = %from.display(), "mise à la corbeille d'un document impossible");
        }
    }
}

/// Efface les documents de la corbeille depuis plus de 400 jours (ADR-0024) ; renvoie leur nombre.
pub fn purge_trash(data: &DataDir, today: Date) -> anyhow::Result<usize> {
    let Ok(entries) = std::fs::read_dir(data.documents_trash()) else {
        return Ok(0);
    };
    let mut purged = 0;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let trashed_on = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.split_once('-'))
            .and_then(|(date, _)| chrono::NaiveDate::parse_from_str(date, TRASH_DATE).ok());
        if trashed_on.is_some_and(|on| (today - on).num_days() > TRASH_DAYS) {
            std::fs::remove_file(&path).with_context(|| format!("suppression de {}", path.display()))?;
            purged += 1;
        }
    }
    Ok(purged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn file_name_keeps_only_the_name() {
        assert_eq!(file_name("C:\\Users\\moi\\paie-2026-08.pdf"), "paie-2026-08.pdf");
        assert_eq!(file_name("../../etc/passwd"), "passwd");
        assert_eq!(file_name("  "), "document");
    }

    #[test]
    fn staged_document_is_published_or_left_without_trace() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        data.ensure().unwrap();
        let staged = stage(&data, "paie.pdf", b"%PDF-1.7 fiche").unwrap();
        let id = staged.row.id;
        assert_eq!(staged.row.size_bytes, 14);
        drop(staged);
        assert_eq!(std::fs::read_dir(data.documents()).unwrap().count(), 1, "seule la corbeille reste");

        let staged = stage(&data, "paie.pdf", b"%PDF-1.7 fiche").unwrap();
        let published = staged.row.id;
        staged.publish().unwrap();
        assert_eq!(read(&data, published).unwrap(), b"%PDF-1.7 fiche");
        assert!(matches!(read(&data, id), Err(AppError::NotFound(_))));
    }

    #[test]
    fn trashed_document_is_purged_after_400_days_adr_0024() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::new(dir.path());
        data.ensure().unwrap();
        let staged = stage(&data, "paie.pdf", b"%PDF-1.7 fiche").unwrap();
        let id = staged.row.id;
        staged.publish().unwrap();
        trash(&data, &[id], d("2026-01-10"));
        assert!(matches!(read(&data, id), Err(AppError::NotFound(_))));
        assert_eq!(purge_trash(&data, d("2027-02-14")).unwrap(), 0, "400 jours tout juste");
        assert_eq!(purge_trash(&data, d("2027-02-15")).unwrap(), 1);
        assert_eq!(std::fs::read_dir(data.documents_trash()).unwrap().count(), 0);
    }
}
