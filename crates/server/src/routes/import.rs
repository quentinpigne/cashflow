use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{imports, rules};
use crate::state::AppState;

/// Import de relevés et règles de catégorisation (spec 03).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(imports::preview))
        .routes(routes!(imports::list, imports::start))
        .routes(routes!(imports::get, imports::cancel))
        .routes(routes!(imports::review_line))
        .routes(routes!(imports::bulk))
        .routes(routes!(imports::commit))
        .routes(routes!(imports::revert))
        .routes(routes!(imports::download_file, imports::delete_file))
        .routes(routes!(imports::list_profiles, imports::create_profile))
        .routes(routes!(imports::update_profile, imports::delete_profile))
        .routes(routes!(rules::list, rules::create))
        .routes(routes!(rules::update, rules::delete))
        .routes(routes!(rules::preview))
        .routes(routes!(rules::apply))
}
