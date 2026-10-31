use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::MemberId;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::members::{self, MemberInput, MemberReorderInput, MemberView};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct MemberCreated {
    pub id: MemberId,
}

/// Les membres du foyer, dans leur ordre (MBR-01).
#[utoipa::path(get, operation_id = "listMembers", path = "/members", tag = "household", responses((status = 200, body = Vec<MemberView>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<MemberView>>> {
    Ok(Json(members::list(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createMember", path = "/members", tag = "household", request_body = MemberInput,
    responses((status = 201, body = MemberCreated), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<MemberInput>,
) -> AppResult<(StatusCode, Json<MemberCreated>)> {
    let id = members::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(MemberCreated { id })))
}

#[utoipa::path(
    put, operation_id = "updateMember", path = "/members/{id}", tag = "household", params(("id" = MemberId, Path)),
    request_body = MemberInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<MemberId>,
    Json(input): Json<MemberInput>,
) -> AppResult<StatusCode> {
    members::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un membre ; ce qui lui était affecté revient au foyer (R-MBR-2).
#[utoipa::path(
    delete, operation_id = "deleteMember", path = "/members/{id}", tag = "household", params(("id" = MemberId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<MemberId>) -> AppResult<StatusCode> {
    members::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    put, operation_id = "reorderMembers", path = "/members/order", tag = "household", request_body = MemberReorderInput,
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn reorder(State(state): State<AppState>, Json(input): Json<MemberReorderInput>) -> AppResult<StatusCode> {
    members::reorder(&state.db, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}
