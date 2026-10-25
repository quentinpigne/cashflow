//! `cashflow set-password` : définit le mot de passe de l'utilisateur unique (ADR-0015).

use std::io::Read;

use anyhow::bail;
use clap::Args;

use crate::db::Db;
use crate::services;

#[derive(Args)]
pub struct SetPassword {
    #[arg(long, default_value = "Moi")]
    display_name: String,
    /// Lit le mot de passe sur l'entrée standard au lieu de le demander.
    #[arg(long)]
    password_stdin: bool,
}

impl SetPassword {
    pub async fn run(self, db: &Db) -> anyhow::Result<()> {
        let password = if self.password_stdin {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            input.trim_end_matches(['\n', '\r']).to_owned()
        } else {
            let first = rpassword::prompt_password("Nouveau mot de passe : ")?;
            let second = rpassword::prompt_password("Confirmation : ")?;
            if first != second {
                bail!("les deux saisies diffèrent");
            }
            first
        };
        services::auth::set_password(db, &self.display_name, &password).await?;
        eprintln!("Mot de passe défini ; toutes les sessions sont fermées.");
        Ok(())
    }
}
