use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::salaries;
use crate::state::AppState;

/// Salaires (spec 10).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(salaries::overview))
        .routes(routes!(salaries::create))
        .routes(routes!(salaries::get, salaries::update, salaries::delete))
        .routes(routes!(salaries::create_salary))
        .routes(routes!(salaries::update_salary, salaries::delete_salary))
        .routes(routes!(salaries::create_bonus))
        .routes(routes!(salaries::update_bonus, salaries::delete_bonus))
}
