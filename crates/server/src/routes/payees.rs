use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::payees;
use crate::state::AppState;

/// Tiers et groupes de tiers (spec 11).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(payees::list, payees::create))
        .routes(routes!(payees::update, payees::delete))
        .routes(routes!(payees::merge))
        .routes(routes!(payees::regroup))
        .routes(routes!(payees::summary))
        .routes(routes!(payees::create_group))
        .routes(routes!(payees::update_group, payees::delete_group))
        .routes(routes!(payees::group_summary))
        .routes(routes!(payees::report))
}
