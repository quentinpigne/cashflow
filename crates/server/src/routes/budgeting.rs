use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::budgets;
use crate::state::AppState;

/// Budgets, projets et notifications (spec 05).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(budgets::month))
        .routes(routes!(budgets::list_lines, budgets::create_line))
        .routes(routes!(budgets::update_line))
        .routes(routes!(budgets::change_amount))
        .routes(routes!(budgets::change_categories))
        .routes(routes!(budgets::reset_rollover))
        .routes(routes!(budgets::end_line))
        .routes(routes!(budgets::delete_override))
        .routes(routes!(budgets::suggestions))
        .routes(routes!(budgets::set_thresholds))
        .routes(routes!(budgets::list_projects, budgets::create_project))
        .routes(routes!(budgets::get_project, budgets::update_project, budgets::delete_project))
        .routes(routes!(budgets::attach))
        .routes(routes!(budgets::notifications))
        .routes(routes!(budgets::mark_read))
}
