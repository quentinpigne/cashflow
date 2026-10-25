use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{dashboard, reports};
use crate::state::AppState;

/// Tableau de bord et analyses (spec 08).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(dashboard::get))
        .routes(routes!(dashboard::recent))
        .routes(routes!(reports::categories))
        .routes(routes!(reports::evolution))
}
