use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::projections;
use crate::state::AppState;

/// Récurrences, scénarios et projections (spec 02, spec 07).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(projections::occurrences))
        .routes(routes!(projections::list_rules, projections::create_rule))
        .routes(routes!(projections::update_rule, projections::delete_rule))
        .routes(routes!(projections::materialize))
        .routes(routes!(projections::list_scenarios, projections::create_scenario))
        .routes(routes!(projections::update_scenario, projections::delete_scenario))
        .routes(routes!(projections::cash))
        .routes(routes!(projections::account))
        .routes(routes!(projections::net_worth))
        .routes(routes!(projections::salary))
        .routes(routes!(projections::history))
}
