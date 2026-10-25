//! Ligne de commande du binaire `cashflow` : un fichier par sous-commande dans `cli/` (ADR-0015).

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

use crate::config::DataDir;
use crate::db::Db;
use crate::{routes, services};

mod data;
mod password;
mod serve;
mod token;

#[derive(Parser)]
#[command(name = "cashflow", version, about = "Gestion de finances personnelles")]
pub struct Cli {
    /// Dossier des données : base, sauvegardes, fichiers importés.
    #[arg(long, env = "CASHFLOW_DATA_DIR", default_value = "data", global = true)]
    data_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lance le serveur HTTP (API et interface).
    Serve(serve::Serve),
    /// Applique les migrations en attente, après une sauvegarde.
    Migrate,
    /// Charge les données initiales (barèmes, établissements, catégories).
    Seed,
    /// Remplit une base vide avec le jeu de démonstration fictif (développement, tests).
    Demo(data::Demo),
    /// Écrit l'export complet (JSON) sur la sortie standard ou dans un fichier (ADR-0008).
    Export(data::Export),
    /// Restaure un export dans une base neuve.
    ImportExport(data::ImportExport),
    /// Fait une sauvegarde de la base.
    Backup(data::Backup),
    /// Définit le mot de passe de l'utilisateur unique (ferme toutes les sessions).
    SetPassword(password::SetPassword),
    /// Gère les jetons d'accès en lecture seule d'une application tierce, comme Foodflow (ADR-0023).
    Token {
        #[command(subcommand)]
        action: token::TokenAction,
    },
    /// Écrit le contrat OpenAPI sur la sortie standard (ADR-0016).
    Openapi,
}

impl Cli {
    pub async fn run(self) -> anyhow::Result<()> {
        let data = DataDir::new(&self.data_dir);
        match self.command {
            Command::Openapi => {
                println!("{}", routes::openapi_json().context("sérialisation du contrat OpenAPI")?);
                Ok(())
            }
            Command::Serve(args) => args.run(data).await,
            Command::Migrate => open(&data).await.map(drop),
            Command::Seed => services::seed::run(&open(&data).await?).await,
            Command::Demo(args) => args.run(&open(&data).await?).await,
            Command::Export(args) => args.run(&open(&data).await?).await,
            Command::ImportExport(args) => args.run(&open(&data).await?).await,
            Command::Backup(args) => args.run(&open(&data).await?, &data).await,
            Command::SetPassword(args) => args.run(&open(&data).await?).await,
            Command::Token { action } => action.run(&open(&data).await?).await,
        }
    }
}

/// Ouvre la base et applique les migrations, après une sauvegarde si la base contient des données.
async fn open(data: &DataDir) -> anyhow::Result<Db> {
    data.ensure().with_context(|| format!("création de {}", data.root().display()))?;
    let db = Db::connect(&data.database()).await?;
    if !db.pending_migrations().await?.is_empty() {
        if db.has_data().await? {
            services::backup::backup(&db, data, "pre-migration").await?;
        }
        db.migrate().await?;
    }
    Ok(db)
}
