use axum::Json;
use axum::extract::{Query, State};

use crate::error::AppResult;
use crate::services::dashboard::{self, Dashboard, RecentQuery, RecentTransaction};
use crate::state::AppState;

/// Accueil (TDB-01).
#[utoipa::path(get, operation_id = "getDashboard", path = "/dashboard", tag = "dashboard", responses((status = 200, body = Dashboard)))]
pub async fn get(State(state): State<AppState>) -> AppResult<Json<Dashboard>> {
    Ok(Json(dashboard::get(&state.db, state.clock.today()).await?))
}

/// Les dernières transactions saisies (TDB-01).
#[utoipa::path(
    get, operation_id = "recentTransactions", path = "/dashboard/recent-transactions", tag = "dashboard",
    params(RecentQuery), responses((status = 200, body = Vec<RecentTransaction>))
)]
pub async fn recent(
    State(state): State<AppState>,
    Query(query): Query<RecentQuery>,
) -> AppResult<Json<Vec<RecentTransaction>>> {
    Ok(Json(dashboard::recent(&state.db, &query).await?))
}
