use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::savings;
use crate::state::AppState;

/// Épargne : intérêts, valorisations, performance (spec 06).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(savings::get_terms, savings::update_terms))
        .routes(routes!(savings::schedules))
        .routes(routes!(savings::set_rate))
        .routes(routes!(savings::delete_rate))
        .routes(routes!(savings::interest))
        .routes(routes!(savings::record_interest))
        .routes(routes!(savings::valuations, savings::save_valuation))
        .routes(routes!(savings::delete_valuation))
        .routes(routes!(savings::performance))
        .routes(routes!(savings::overview))
}
