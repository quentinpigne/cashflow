use axum::extract::DefaultBodyLimit;
use cashflow_domain::documents::MAX_SIZE;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{accounts, institutions, statements};
use crate::state::AppState;

/// Comptes et établissements (spec 01).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(accounts::list, accounts::create))
        .routes(routes!(accounts::get, accounts::update, accounts::delete))
        .routes(routes!(accounts::reveal_iban))
        .routes(routes!(accounts::close))
        .routes(routes!(accounts::reopen))
        .routes(routes!(accounts::reorder))
        .routes(routes!(institutions::list, institutions::create))
        .routes(routes!(institutions::update))
        .merge(statements())
}

/// Relevés de compte (CPT-11) : l'envoi accepte un fichier de 10 Mo, plus l'enveloppe du formulaire.
fn statements() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(statements::list, statements::create))
        .routes(routes!(statements::update, statements::delete))
        .routes(routes!(statements::download))
        .layer(DefaultBodyLimit::max(MAX_SIZE + 64 * 1024))
}
