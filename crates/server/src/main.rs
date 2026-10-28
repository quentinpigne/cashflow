//! Binaire `cashflow` : sert l'API et l'interface, et porte les sous-commandes d'administration (ADR-0015).

#![forbid(unsafe_code)]

use clap::Parser;
use tracing_subscriber::EnvFilter;

mod cli;
mod clock;
mod config;
mod db;
mod error;
mod geocoding;
mod handlers;
mod middleware;
mod readers;
mod routes;
mod services;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();

    cli::Cli::parse().run().await
}
