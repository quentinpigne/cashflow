use axum::extract::DefaultBodyLimit;
use cashflow_domain::documents::MAX_SIZE;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::documents;
use crate::state::AppState;

/// Bibliothèque de documents (spec 12) : l'envoi accepte un fichier de 10 Mo, plus l'enveloppe du formulaire.
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(documents::list, documents::create))
        .routes(routes!(documents::get, documents::update, documents::delete))
        .routes(routes!(documents::content))
        .routes(routes!(documents::link))
        .routes(routes!(documents::unlink))
        .routes(routes!(documents::attached))
        .layer(DefaultBodyLimit::max(MAX_SIZE + 64 * 1024))
}
