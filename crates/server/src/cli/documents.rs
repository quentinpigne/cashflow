//! `cashflow documents` : bibliothèque de documents (spec 12).

use clap::Subcommand;

use crate::config::DataDir;
use crate::db::Db;
use crate::services;

#[derive(Subcommand)]
pub enum DocumentsAction {
    /// Reconstruit l'index de recherche à partir des fichiers : texte des PDF, description (R-DOC-19).
    Reindex,
}

impl DocumentsAction {
    pub async fn run(self, db: &Db, data: &DataDir) -> anyhow::Result<()> {
        match self {
            Self::Reindex => {
                let indexed = services::documents::reindex(db, data, true).await.map_err(|e| anyhow::anyhow!("{e}"))?;
                eprintln!("{indexed} document(s) indexé(s).");
            }
        }
        Ok(())
    }
}
