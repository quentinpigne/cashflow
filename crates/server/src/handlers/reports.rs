use axum::Json;
use axum::extract::{Query, State};

use crate::error::{AppResult, Problem};
use crate::services::reports::{self, CategoryReport, CategoryReportQuery, Evolution, EvolutionQuery};
use crate::state::AppState;

/// Dépenses ou revenus par catégorie d'un mois, comparés au mois précédent et à la moyenne (TDB-02).
#[utoipa::path(
    get, operation_id = "categoryReport", path = "/reports/categories", tag = "dashboard", params(CategoryReportQuery),
    responses((status = 200, body = CategoryReport), (status = 422, body = Problem))
)]
pub async fn categories(
    State(state): State<AppState>,
    Query(query): Query<CategoryReportQuery>,
) -> AppResult<Json<CategoryReport>> {
    Ok(Json(reports::categories(&state.db, &query, state.clock.today()).await?))
}

/// Revenus, dépenses, épargne et taux d'épargne mois par mois (TDB-03).
#[utoipa::path(get, operation_id = "evolutionReport", path = "/reports/evolution", tag = "dashboard", params(EvolutionQuery), responses((status = 200, body = Evolution)))]
pub async fn evolution(
    State(state): State<AppState>,
    Query(query): Query<EvolutionQuery>,
) -> AppResult<Json<Evolution>> {
    Ok(Json(reports::evolution(&state.db, &query, state.clock.today()).await?))
}
