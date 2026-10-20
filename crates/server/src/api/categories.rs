use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::CategoryId;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::categories::{self, Category, CategoryInput, MergeInput, MergeResult, ReorderInput};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct Created {
    pub id: CategoryId,
}

/// Arbre des catégories à deux niveaux (TRX-04).
#[utoipa::path(get, path = "/categories", tag = "categories", responses((status = 200, body = Vec<Category>)))]
pub async fn tree(State(state): State<AppState>) -> AppResult<Json<Vec<Category>>> {
    Ok(Json(categories::tree(&state.db).await?))
}

#[utoipa::path(
    post, path = "/categories", tag = "categories", request_body = CategoryInput,
    responses((status = 201, body = Created), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<CategoryInput>,
) -> AppResult<(StatusCode, Json<Created>)> {
    let id = categories::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Renomme, déplace, archive ou désarchive (R-TRX-20, R-TRX-22).
#[utoipa::path(
    put, path = "/categories/{id}", tag = "categories", params(("id" = CategoryId, Path)), request_body = CategoryInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<CategoryId>,
    Json(input): Json<CategoryInput>,
) -> AppResult<StatusCode> {
    categories::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime une catégorie jamais utilisée (R-TRX-22).
#[utoipa::path(
    delete, path = "/categories/{id}", tag = "categories", params(("id" = CategoryId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<CategoryId>) -> AppResult<StatusCode> {
    categories::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ordre d'affichage d'un niveau de l'arbre.
#[utoipa::path(post, path = "/categories/reorder", tag = "categories", request_body = ReorderInput, responses((status = 204)))]
pub async fn reorder(State(state): State<AppState>, Json(input): Json<ReorderInput>) -> AppResult<StatusCode> {
    categories::reorder(&state.db, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fusionne la catégorie dans une autre (R-TRX-22).
#[utoipa::path(
    post, path = "/categories/{id}/merge", tag = "categories", params(("id" = CategoryId, Path)), request_body = MergeInput,
    responses((status = 200, body = MergeResult), (status = 422, body = Problem))
)]
pub async fn merge(
    State(state): State<AppState>,
    Path(id): Path<CategoryId>,
    Json(input): Json<MergeInput>,
) -> AppResult<Json<MergeResult>> {
    Ok(Json(categories::merge(&state.db, id, &input).await?))
}
