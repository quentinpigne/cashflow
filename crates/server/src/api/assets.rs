use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::{AssetId, AssetValuationId};

use crate::error::{AppResult, Problem};
use crate::services::assets::{self, AssetInput, AssetSummary, AssetValuationInput, AssetView};
use crate::services::features::{self, Features, FeaturesInput};
use crate::state::AppState;

/// Biens (E25, BIE-01).
#[utoipa::path(get, operation_id = "listAssets", path = "/assets", tag = "assets", responses((status = 200, body = Vec<AssetSummary>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<AssetSummary>>> {
    Ok(Json(assets::list(&state.db, state.clock.today()).await?))
}

#[utoipa::path(
    post, operation_id = "createAsset", path = "/assets", tag = "assets", request_body = AssetInput,
    responses((status = 201, body = AssetView), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<AssetInput>,
) -> AppResult<(StatusCode, Json<AssetView>)> {
    Ok((StatusCode::CREATED, Json(assets::create(&state.db, &input, state.clock.today()).await?)))
}

/// Fiche d'un bien : estimations, prêts, valeur nette, acquisition (E26).
#[utoipa::path(
    get, operation_id = "getAsset", path = "/assets/{id}", tag = "assets", params(("id" = AssetId, Path)),
    responses((status = 200, body = AssetView), (status = 404, body = Problem))
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<AssetId>) -> AppResult<Json<AssetView>> {
    Ok(Json(assets::get(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateAsset", path = "/assets/{id}", tag = "assets", params(("id" = AssetId, Path)),
    request_body = AssetInput, responses((status = 200, body = AssetView), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<AssetId>,
    Json(input): Json<AssetInput>,
) -> AppResult<Json<AssetView>> {
    Ok(Json(assets::update(&state.db, id, &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteAsset", path = "/assets/{id}", tag = "assets", params(("id" = AssetId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<AssetId>) -> AppResult<StatusCode> {
    assets::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ajoute ou remplace l'estimation d'une date (R-BIE-3).
#[utoipa::path(
    post, operation_id = "saveAssetValuation", path = "/assets/{id}/valuations", tag = "assets",
    params(("id" = AssetId, Path)), request_body = AssetValuationInput,
    responses((status = 200, body = AssetView), (status = 422, body = Problem))
)]
pub async fn save_valuation(
    State(state): State<AppState>,
    Path(id): Path<AssetId>,
    Json(input): Json<AssetValuationInput>,
) -> AppResult<Json<AssetView>> {
    Ok(Json(assets::save_valuation(&state.db, id, &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteAssetValuation", path = "/assets/{id}/valuations/{valuation_id}", tag = "assets",
    params(("id" = AssetId, Path), ("valuation_id" = AssetValuationId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_valuation(
    State(state): State<AppState>,
    Path((id, valuation)): Path<(AssetId, AssetValuationId)>,
) -> AppResult<StatusCode> {
    assets::delete_valuation(&state.db, id, valuation).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Rubriques activées (R-BIE-7, R-PRE-12).
#[utoipa::path(get, operation_id = "getFeatures", path = "/settings/features", tag = "system", responses((status = 200, body = Features)))]
pub async fn get_features(State(state): State<AppState>) -> AppResult<Json<Features>> {
    Ok(Json(features::get(&state.db).await?))
}

/// Active ou désactive une rubrique ; une rubrique ne se désactive que vide.
#[utoipa::path(
    put, operation_id = "updateFeatures", path = "/settings/features", tag = "system", request_body = FeaturesInput,
    responses((status = 200, body = Features), (status = 422, body = Problem))
)]
pub async fn update_features(
    State(state): State<AppState>,
    Json(input): Json<FeaturesInput>,
) -> AppResult<Json<Features>> {
    Ok(Json(features::update(&state.db, &input).await?))
}
