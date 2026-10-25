//! Emplacements des données sur disque (ADR-0015 : un volume qui contient tout l'état).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("cashflow.db")
    }

    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn imports(&self) -> PathBuf {
        self.root.join("imports")
    }

    /// Documents archivés : un fichier par document, nommé par son identifiant (ADR-0024).
    pub fn documents(&self) -> PathBuf {
        self.root.join("documents")
    }

    /// Documents supprimés, effacés après la rétention des sauvegardes (ADR-0024).
    pub fn documents_trash(&self) -> PathBuf {
        self.documents().join("trash")
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.backups())?;
        std::fs::create_dir_all(self.imports())?;
        std::fs::create_dir_all(self.documents_trash())
    }
}
