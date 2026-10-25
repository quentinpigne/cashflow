//! Binaire `cashflow` : sert l'API et l'interface, et porte les sous-commandes d'administration (ADR-0015).

#![forbid(unsafe_code)]

use std::io::Read;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod clock;
mod config;
mod db;
mod error;
mod handlers;
mod middleware;
mod readers;
mod routes;
mod services;
mod state;

use clock::Clock;
use config::DataDir;
use db::Db;
use state::AppState;

#[derive(Parser)]
#[command(name = "cashflow", version, about = "Gestion de finances personnelles")]
struct Cli {
    /// Dossier des données : base, sauvegardes, fichiers importés.
    #[arg(long, env = "CASHFLOW_DATA_DIR", default_value = "data", global = true)]
    data_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lance le serveur HTTP (API et interface).
    Serve {
        #[arg(long, env = "CASHFLOW_ADDR", default_value = "127.0.0.1:3000")]
        addr: SocketAddr,
        /// Dossier de l'interface compilée (`ng build`).
        #[arg(long, env = "CASHFLOW_WEB_DIR", default_value = "web/dist/cashflow/browser")]
        web_dir: PathBuf,
        /// Cookie de session `Secure` : à activer quand l'application est servie en HTTPS (ADR-0015).
        #[arg(long, env = "CASHFLOW_SECURE_COOKIES", default_value_t = false)]
        secure_cookies: bool,
        /// Date du jour figée (AAAA-MM-JJ), pour les tests de bout en bout.
        #[arg(long, env = "CASHFLOW_TODAY", hide = true)]
        today: Option<chrono::NaiveDate>,
    },
    /// Applique les migrations en attente, après une sauvegarde.
    Migrate,
    /// Charge les données initiales (barèmes, établissements, catégories).
    Seed,
    /// Remplit une base vide avec le jeu de démonstration fictif (développement, tests).
    Demo {
        #[arg(long, env = "CASHFLOW_TODAY")]
        today: Option<chrono::NaiveDate>,
    },
    /// Écrit l'export complet (JSON) sur la sortie standard ou dans un fichier (ADR-0008).
    Export {
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Restaure un export dans une base neuve.
    ImportExport { file: PathBuf },
    /// Fait une sauvegarde de la base.
    Backup {
        /// Motif, repris dans le nom du fichier : `manual`, `pre-deploy`…
        #[arg(long, default_value = "manual")]
        reason: String,
    },
    /// Définit le mot de passe de l'utilisateur unique (ferme toutes les sessions).
    SetPassword {
        #[arg(long, default_value = "Moi")]
        display_name: String,
        /// Lit le mot de passe sur l'entrée standard au lieu de le demander.
        #[arg(long)]
        password_stdin: bool,
    },
    /// Gère les jetons d'accès en lecture seule d'une application tierce, comme Foodflow (ADR-0023).
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
    /// Écrit le contrat OpenAPI sur la sortie standard (ADR-0016).
    Openapi,
}

#[derive(Subcommand)]
enum TokenAction {
    /// Crée un jeton et l'écrit sur la sortie standard : il ne sera plus jamais affiché.
    Create { name: String },
    /// Liste les jetons : nom, création, dernier usage.
    List,
    /// Révoque un jeton.
    Revoke { name: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let data = DataDir::new(&cli.data_dir);
    match cli.command {
        Command::Openapi => {
            println!("{}", routes::openapi_json().context("sérialisation du contrat OpenAPI")?);
            Ok(())
        }
        Command::Serve { addr, web_dir, secure_cookies, today } => {
            let db = open(&data).await?;
            services::seed::run(&db).await?;
            let clock = today.map_or(Clock::System, Clock::Fixed);
            serve(AppState { db, clock, data, limiter: Arc::default(), secure_cookies }, addr, web_dir).await
        }
        Command::Migrate => {
            open(&data).await?;
            Ok(())
        }
        Command::Seed => {
            let db = open(&data).await?;
            services::seed::run(&db).await
        }
        Command::Demo { today } => {
            let db = open(&data).await?;
            let today = today.map_or(Clock::System, Clock::Fixed).today();
            services::demo::load(&db, today).await?;
            eprintln!("Jeu de démonstration chargé. Mot de passe : {}", services::demo::DEMO_PASSWORD);
            Ok(())
        }
        Command::Export { output } => {
            let db = open(&data).await?;
            let json = serde_json::to_string_pretty(&services::export::export(&db).await?)?;
            match output {
                Some(path) => std::fs::write(&path, json).with_context(|| format!("écriture de {}", path.display()))?,
                None => println!("{json}"),
            }
            Ok(())
        }
        Command::ImportExport { file } => {
            let db = open(&data).await?;
            let content = std::fs::read_to_string(&file).with_context(|| format!("lecture de {}", file.display()))?;
            services::export::restore(&db, &serde_json::from_str(&content)?).await?;
            eprintln!("Export restauré. Définir le mot de passe avec `cashflow set-password`.");
            Ok(())
        }
        Command::Backup { reason } => {
            let db = open(&data).await?;
            let path = services::backup::backup(&db, &data, &reason).await?;
            eprintln!("Sauvegarde : {}", path.display());
            Ok(())
        }
        Command::SetPassword { display_name, password_stdin } => {
            let db = open(&data).await?;
            let password = if password_stdin {
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
            services::auth::set_password(&db, &display_name, &password).await?;
            eprintln!("Mot de passe défini ; toutes les sessions sont fermées.");
            Ok(())
        }
        Command::Token { action } => {
            let db = open(&data).await?;
            match action {
                TokenAction::Create { name } => {
                    let token = services::access_tokens::create(&db, &name).await?;
                    eprintln!("Jeton « {} » créé. Il ne sera plus affiché :", name.trim());
                    println!("{token}");
                }
                TokenAction::List => {
                    for token in services::access_tokens::list(&db).await? {
                        let used = token.last_used_at.as_deref().unwrap_or("jamais utilisé");
                        println!("{}\tcréé le {}\tdernier usage : {used}", token.name, token.created_at);
                    }
                }
                TokenAction::Revoke { name } => {
                    services::access_tokens::revoke(&db, &name).await?;
                    eprintln!("Jeton « {} » révoqué.", name.trim());
                }
            }
            Ok(())
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

async fn serve(state: AppState, addr: SocketAddr, web_dir: PathBuf) -> anyhow::Result<()> {
    tokio::spawn(services::backup::run_daily(state.db.clone(), state.data.clone()));
    let app = routes::router(state, web_dir).layer(CompressionLayer::new()).layer(TraceLayer::new_for_http());
    let listener = TcpListener::bind(addr).await.with_context(|| format!("écoute sur {addr}"))?;
    tracing::info!("serveur démarré sur http://{addr}");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "écoute du signal d'arrêt impossible");
    }
}
