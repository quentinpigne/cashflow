use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{assets, loans};
use crate::state::AppState;

/// Biens et prêts (spec 09).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(loans::list, loans::create))
        .routes(routes!(loans::get, loans::update, loans::delete))
        .routes(routes!(loans::save_balance))
        .routes(routes!(loans::delete_balance))
        .routes(routes!(loans::preview))
        .routes(routes!(loans::proposals))
        .routes(routes!(loans::pay))
        .routes(routes!(loans::unpay))
        .routes(routes!(loans::transaction_proposal))
        .routes(routes!(assets::list, assets::create))
        .routes(routes!(assets::get, assets::update, assets::delete))
        .routes(routes!(assets::save_valuation))
        .routes(routes!(assets::delete_valuation))
        .routes(routes!(assets::get_features, assets::update_features))
}
