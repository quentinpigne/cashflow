use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::{AccountId, CheckpointId, TransactionId};

use crate::error::{AppResult, Problem};
use crate::services::reconciliation::{
    self, AccountHealth, Checkpoint, CheckpointInput, ClearInput, GapHintsView, ReconcilePreview,
};
use crate::state::AppState;

/// Points de contrôle d'un compte, avec leur écart (RAP-01, R-RAP-3).
#[utoipa::path(
    get, operation_id = "listCheckpoints", path = "/accounts/{id}/checkpoints", tag = "reconciliation",
    params(("id" = AccountId, Path)), responses((status = 200, body = Vec<Checkpoint>))
)]
pub async fn list(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<Json<Vec<Checkpoint>>> {
    Ok(Json(reconciliation::list(&state.db, id).await?))
}

/// Saisit un point de contrôle manuel (RAP-01).
#[utoipa::path(
    post, operation_id = "createCheckpoint", path = "/accounts/{id}/checkpoints", tag = "reconciliation",
    params(("id" = AccountId, Path)), request_body = CheckpointInput,
    responses((status = 201, body = Checkpoint), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Json(input): Json<CheckpointInput>,
) -> AppResult<(StatusCode, Json<Checkpoint>)> {
    Ok((StatusCode::CREATED, Json(reconciliation::create(&state.db, id, &input).await?)))
}

#[utoipa::path(
    delete, operation_id = "deleteCheckpoint", path = "/checkpoints/{id}", tag = "reconciliation",
    params(("id" = CheckpointId, Path)), responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<CheckpointId>) -> AppResult<StatusCode> {
    reconciliation::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ce que le rapprochement va faire : points figés, transactions en attente (R-RAP-4, R-RAP-5).
#[utoipa::path(
    get, operation_id = "reconcilePreview", path = "/checkpoints/{id}/reconcile", tag = "reconciliation",
    params(("id" = CheckpointId, Path)), responses((status = 200, body = ReconcilePreview), (status = 422, body = Problem))
)]
pub async fn preview(State(state): State<AppState>, Path(id): Path<CheckpointId>) -> AppResult<Json<ReconcilePreview>> {
    Ok(Json(reconciliation::reconcile_preview(&state.db, id).await?))
}

/// Rapproche un point de contrôle à écart nul (RAP-03).
#[utoipa::path(
    post, operation_id = "reconcile", path = "/checkpoints/{id}/reconcile", tag = "reconciliation",
    params(("id" = CheckpointId, Path)), responses((status = 204), (status = 422, body = Problem))
)]
pub async fn reconcile(State(state): State<AppState>, Path(id): Path<CheckpointId>) -> AppResult<StatusCode> {
    reconciliation::reconcile(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Déverrouille un rapprochement et les suivants (RAP-04, R-RAP-6).
#[utoipa::path(
    post, operation_id = "unlockCheckpoint", path = "/checkpoints/{id}/unlock", tag = "reconciliation",
    params(("id" = CheckpointId, Path)), responses((status = 204), (status = 422, body = Problem))
)]
pub async fn unlock(State(state): State<AppState>, Path(id): Path<CheckpointId>) -> AppResult<StatusCode> {
    reconciliation::unlock(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Pistes pour expliquer un écart (RAP-05, R-RAP-9).
#[utoipa::path(
    get, operation_id = "gapHints", path = "/checkpoints/{id}/gap-hints", tag = "reconciliation",
    params(("id" = CheckpointId, Path)), responses((status = 200, body = GapHintsView))
)]
pub async fn gap_hints(State(state): State<AppState>, Path(id): Path<CheckpointId>) -> AppResult<Json<GapHintsView>> {
    Ok(Json(reconciliation::gap_hints(&state.db, id).await?))
}

/// Pointe ou dépointe une transaction à la main (RAP-07).
#[utoipa::path(
    put, operation_id = "setCleared", path = "/transactions/{id}/cleared", tag = "reconciliation",
    params(("id" = TransactionId, Path)), request_body = ClearInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn set_cleared(
    State(state): State<AppState>,
    Path(id): Path<TransactionId>,
    Json(input): Json<ClearInput>,
) -> AppResult<StatusCode> {
    reconciliation::set_cleared(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Santé des comptes : dernier rapprochement, écart courant, transactions en attente (RAP-06, R-TDB-10).
#[utoipa::path(get, operation_id = "accountsHealth", path = "/reconciliation/health", tag = "reconciliation", responses((status = 200, body = Vec<AccountHealth>)))]
pub async fn health(State(state): State<AppState>) -> AppResult<Json<Vec<AccountHealth>>> {
    Ok(Json(reconciliation::health(&state.db, state.clock.today()).await?))
}
