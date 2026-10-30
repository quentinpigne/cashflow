use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::credit;
use crate::state::AppState;

/// Simulations et demandes de crédit (spec 13).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(credit::list_simulations, credit::create_simulation))
        .routes(routes!(credit::compute))
        .routes(routes!(credit::get_simulation, credit::update_simulation, credit::delete_simulation))
        .routes(routes!(credit::simulation_schedule))
        .routes(routes!(credit::convert))
        .routes(routes!(credit::list_applications, credit::create_application))
        .routes(routes!(credit::get_application, credit::update_application, credit::delete_application))
        .routes(routes!(credit::assign))
        .routes(routes!(credit::unassign))
        .routes(routes!(credit::set_required))
        .routes(routes!(credit::create_lender_request))
        .routes(routes!(credit::update_lender_request, credit::delete_lender_request))
        .routes(routes!(credit::record_offer))
        .routes(routes!(credit::accept_offer))
        .routes(routes!(credit::export))
        .routes(routes!(credit::summary))
        .routes(routes!(credit::borrower_situation, credit::save_borrower_situation))
        .routes(routes!(credit::project_defaults, credit::save_project_defaults))
        .routes(routes!(credit::simulate_state, credit::save_simulate_state))
        .routes(routes!(credit::purchase_capacity))
        .routes(routes!(credit::financing))
        .routes(routes!(credit::application_from_capacity))
        .routes(routes!(credit::application_from_financing))
        .routes(routes!(credit::application_from_simulation))
        .routes(routes!(credit::follow_up))
        .routes(routes!(credit::create_event))
        .routes(routes!(credit::update_event, credit::delete_event))
}
