//! Fichiers statiques de l'interface, avec repli sur `index.html` pour le routage côté client.

use std::path::PathBuf;

use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub fn router(web_dir: PathBuf) -> Router {
    let index = ServeFile::new(web_dir.join("index.html"));
    Router::new().fallback_service(ServeDir::new(web_dir).fallback(index))
}
