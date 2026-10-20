use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::InstitutionId;

use crate::error::{AppResult, Problem};
use crate::services::institutions::{self, Institution, InstitutionInput};
use crate::state::AppState;

/// Établissements, archivés compris (CPT-02).
#[utoipa::path(get, path = "/institutions", tag = "institutions", responses((status = 200, body = Vec<Institution>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Institution>>> {
    Ok(Json(institutions::list(&state.db).await?))
}

#[utoipa::path(
    post, path = "/institutions", tag = "institutions", request_body = InstitutionInput,
    responses((status = 201, body = Institution), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<InstitutionInput>,
) -> AppResult<(StatusCode, Json<Institution>)> {
    Ok((StatusCode::CREATED, Json(institutions::create(&state.db, &input).await?)))
}

/// Modifie ou archive un établissement (R-CPT-10).
#[utoipa::path(
    put, path = "/institutions/{id}", tag = "institutions", params(("id" = InstitutionId, Path)),
    request_body = InstitutionInput, responses((status = 200, body = Institution), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<InstitutionId>,
    Json(input): Json<InstitutionInput>,
) -> AppResult<Json<Institution>> {
    Ok(Json(institutions::update(&state.db, id, &input).await?))
}
