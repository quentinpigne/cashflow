use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{accounts, institutions};
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
}
