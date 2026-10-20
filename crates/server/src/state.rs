//! État partagé par les routes.

use std::sync::Arc;

use crate::clock::Clock;
use crate::config::DataDir;
use crate::db::Db;
use crate::services::auth::LoginLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub clock: Clock,
    pub data: DataDir,
    pub limiter: Arc<LoginLimiter>,
    /// Drapeau `Secure` du cookie de session : actif dès que le serveur est servi en HTTPS (ADR-0015).
    pub secure_cookies: bool,
}
