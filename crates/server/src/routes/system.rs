use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::system;
use crate::state::AppState;

/// Routes accessibles sans session.
pub fn open() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(system::health))
}

/// Version, export.
pub fn protected() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(system::info)).routes(routes!(system::export))
}
