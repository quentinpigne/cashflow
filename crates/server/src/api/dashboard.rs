use axum::Json;
use axum::extract::State;

use crate::error::AppResult;
use crate::services::dashboard::{self, Dashboard};
use crate::state::AppState;

/// Accueil (TDB-01).
#[utoipa::path(get, operation_id = "getDashboard", path = "/dashboard", tag = "dashboard", responses((status = 200, body = Dashboard)))]
pub async fn get(State(state): State<AppState>) -> AppResult<Json<Dashboard>> {
    Ok(Json(dashboard::get(&state.db, state.clock.today()).await?))
}
