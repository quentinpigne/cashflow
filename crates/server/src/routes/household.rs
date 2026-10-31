use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::members;
use crate::state::AppState;

/// Membres du foyer (spec 14).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(members::list, members::create))
        .routes(routes!(members::update, members::delete))
        .routes(routes!(members::reorder))
}
