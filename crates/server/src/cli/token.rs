//! `cashflow token` : jetons d'accès en lecture seule d'une application tierce (ADR-0023).

use clap::Subcommand;

use crate::db::Db;
use crate::services;

#[derive(Subcommand)]
pub enum TokenAction {
    /// Crée un jeton et l'écrit sur la sortie standard : il ne sera plus jamais affiché.
    Create { name: String },
    /// Liste les jetons : nom, création, dernier usage.
    List,
    /// Révoque un jeton.
    Revoke { name: String },
}

impl TokenAction {
    pub async fn run(self, db: &Db) -> anyhow::Result<()> {
        match self {
            Self::Create { name } => {
                let token = services::access_tokens::create(db, &name).await?;
                eprintln!("Jeton « {} » créé. Il ne sera plus affiché :", name.trim());
                println!("{token}");
            }
            Self::List => {
                for token in services::access_tokens::list(db).await? {
                    let used = token.last_used_at.as_deref().unwrap_or("jamais utilisé");
                    println!("{}\tcréé le {}\tdernier usage : {used}", token.name, token.created_at);
                }
            }
            Self::Revoke { name } => {
                services::access_tokens::revoke(db, &name).await?;
                eprintln!("Jeton « {} » révoqué.", name.trim());
            }
        }
        Ok(())
    }
}
