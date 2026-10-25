use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::reconciliation;
use crate::state::AppState;

/// Rapprochement (spec 04).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(reconciliation::list, reconciliation::create))
        .routes(routes!(reconciliation::delete))
        .routes(routes!(reconciliation::preview, reconciliation::reconcile))
        .routes(routes!(reconciliation::unlock))
        .routes(routes!(reconciliation::gap_hints))
        .routes(routes!(reconciliation::set_cleared))
        .routes(routes!(reconciliation::health))
}
