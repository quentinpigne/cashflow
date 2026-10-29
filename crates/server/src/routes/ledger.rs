use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{categories, transactions};
use crate::state::AppState;

/// Catégories, transactions, ventilations et virements (spec 02).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(categories::tree, categories::create))
        .routes(routes!(categories::update, categories::delete))
        .routes(routes!(categories::reorder))
        .routes(routes!(categories::merge))
        .routes(routes!(transactions::list))
        .routes(routes!(transactions::create))
        .routes(routes!(transactions::get, transactions::update, transactions::delete))
        .routes(routes!(transactions::categorize_split))
        .routes(routes!(transactions::create_transfer))
        .routes(routes!(transactions::transfer_candidates))
        .routes(routes!(transactions::mark_as_transfer))
        .routes(routes!(transactions::unlink_transfer))
        .routes(routes!(transactions::set_expected_refund))
        .routes(routes!(transactions::refund_candidates))
        .routes(routes!(transactions::pending_refunds))
        .routes(routes!(transactions::list_all))
        .routes(routes!(transactions::categorize_many))
        .routes(routes!(transactions::set_payee_many))
}
