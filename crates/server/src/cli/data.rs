//! Sous-commandes sur les données : démonstration, export et restauration (ADR-0008), sauvegarde.

use std::path::PathBuf;

use anyhow::Context;
use clap::Args;

use crate::clock::Clock;
use crate::config::DataDir;
use crate::db::Db;
use crate::services;

#[derive(Args)]
pub struct Demo {
    #[arg(long, env = "CASHFLOW_TODAY")]
    today: Option<chrono::NaiveDate>,
}

impl Demo {
    pub async fn run(self, db: &Db, data: &DataDir) -> anyhow::Result<()> {
        let today = self.today.map_or(Clock::System, Clock::Fixed).today();
        services::demo::load(db, data, today).await?;
        eprintln!("Jeu de démonstration chargé. Mot de passe : {}", services::demo::DEMO_PASSWORD);
        Ok(())
    }
}

#[derive(Args)]
pub struct Export {
    #[arg(long, short)]
    output: Option<PathBuf>,
}

impl Export {
    pub async fn run(self, db: &Db) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&services::export::export(db).await?)?;
        match self.output {
            Some(path) => std::fs::write(&path, json).with_context(|| format!("écriture de {}", path.display()))?,
            None => println!("{json}"),
        }
        Ok(())
    }
}

#[derive(Args)]
pub struct ImportExport {
    file: PathBuf,
}

impl ImportExport {
    pub async fn run(self, db: &Db) -> anyhow::Result<()> {
        let file = self.file;
        let content = std::fs::read_to_string(&file).with_context(|| format!("lecture de {}", file.display()))?;
        services::export::restore(db, &serde_json::from_str(&content)?).await?;
        eprintln!("Export restauré. Définir le mot de passe avec `cashflow set-password`.");
        Ok(())
    }
}

#[derive(Args)]
pub struct Backup {
    /// Motif, repris dans le nom du fichier : `manual`, `pre-deploy`…
    #[arg(long, default_value = "manual")]
    reason: String,
}

impl Backup {
    pub async fn run(self, db: &Db, data: &DataDir) -> anyhow::Result<()> {
        let path = services::backup::backup(db, data, &self.reason).await?;
        eprintln!("Sauvegarde : {}", path.display());
        Ok(())
    }
}
