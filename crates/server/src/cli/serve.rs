//! `cashflow serve` : sert l'API et l'interface en HTTP (ADR-0015).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::Args;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::clock::Clock;
use crate::config::DataDir;
use crate::state::AppState;
use crate::{routes, services};

#[derive(Args)]
pub struct Serve {
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
}

impl Serve {
    pub async fn run(self, data: DataDir) -> anyhow::Result<()> {
        let db = super::open(&data).await?;
        services::seed::run(&db).await?;
        let clock = self.today.map_or(Clock::System, Clock::Fixed);
        let state = AppState { db, clock, data, limiter: Arc::default(), secure_cookies: self.secure_cookies };

        tokio::spawn(services::backup::run_daily(state.db.clone(), state.data.clone()));
        let app = routes::router(state, self.web_dir).layer(CompressionLayer::new()).layer(TraceLayer::new_for_http());
        let addr = self.addr;
        let listener = TcpListener::bind(addr).await.with_context(|| format!("écoute sur {addr}"))?;
        tracing::info!("serveur démarré sur http://{addr}");
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    }
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "écoute du signal d'arrêt impossible");
    }
}
