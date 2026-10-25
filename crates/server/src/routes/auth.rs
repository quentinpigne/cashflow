use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::auth;
use crate::state::AppState;

/// Routes accessibles sans session.
pub fn open() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(auth::status)).routes(routes!(auth::login))
}

/// Routes de session (ADR-0015).
pub fn protected() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(auth::logout)).routes(routes!(auth::logout_everywhere))
}
