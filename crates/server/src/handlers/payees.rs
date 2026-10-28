use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::{PayeeGroupId, PayeeId};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::{AppResult, Problem};
use crate::geocoding::AddressSuggestion;
use crate::services::payees::{
    self, LocationReport, LocationReportQuery, MergePayeeInput, MergePayeeResult, PayeeGroupInput, PayeeInput,
    PayeeList, PayeeReport, PayeeReportQuery, PayeeSummary, RegroupInput,
};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct PayeeCreated {
    pub id: PayeeId,
}

#[derive(Serialize, ToSchema)]
pub struct PayeeGroupCreated {
    pub id: PayeeGroupId,
}

/// Tous les tiers et groupes de tiers (TIE-01).
#[utoipa::path(get, operation_id = "listPayees", path = "/payees", tag = "payees", responses((status = 200, body = PayeeList)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<PayeeList>> {
    Ok(Json(payees::list(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createPayee", path = "/payees", tag = "payees", request_body = PayeeInput,
    responses((status = 201, body = PayeeCreated), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<PayeeInput>,
) -> AppResult<(StatusCode, Json<PayeeCreated>)> {
    let id = payees::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(PayeeCreated { id })))
}

/// Renomme, change de groupe, archive ou désarchive un tiers (R-TIE-3, R-TIE-6).
#[utoipa::path(
    put, operation_id = "updatePayee", path = "/payees/{id}", tag = "payees", params(("id" = PayeeId, Path)),
    request_body = PayeeInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<PayeeId>,
    Json(input): Json<PayeeInput>,
) -> AppResult<StatusCode> {
    payees::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un tiers jamais utilisé (R-TIE-5).
#[utoipa::path(
    delete, operation_id = "deletePayee", path = "/payees/{id}", tag = "payees", params(("id" = PayeeId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<PayeeId>) -> AppResult<StatusCode> {
    payees::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fusionne le tiers dans un autre (R-TIE-7).
#[utoipa::path(
    post, operation_id = "mergePayee", path = "/payees/{id}/merge", tag = "payees", params(("id" = PayeeId, Path)),
    request_body = MergePayeeInput, responses((status = 200, body = MergePayeeResult), (status = 422, body = Problem))
)]
pub async fn merge(
    State(state): State<AppState>,
    Path(id): Path<PayeeId>,
    Json(input): Json<MergePayeeInput>,
) -> AppResult<Json<MergePayeeResult>> {
    Ok(Json(payees::merge(&state.db, id, &input).await?))
}

/// Change le groupe de plusieurs tiers (E31).
#[utoipa::path(
    post, operation_id = "regroupPayees", path = "/payees/regroup", tag = "payees", request_body = RegroupInput,
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn regroup(State(state): State<AppState>, Json(input): Json<RegroupInput>) -> AppResult<StatusCode> {
    payees::regroup(&state.db, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fiche d'un tiers (E32).
#[utoipa::path(
    get, operation_id = "payeeSummary", path = "/payees/{id}/summary", tag = "payees", params(("id" = PayeeId, Path)),
    responses((status = 200, body = PayeeSummary), (status = 404, body = Problem))
)]
pub async fn summary(State(state): State<AppState>, Path(id): Path<PayeeId>) -> AppResult<Json<PayeeSummary>> {
    Ok(Json(payees::payee_summary(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    post, operation_id = "createPayeeGroup", path = "/payee-groups", tag = "payees", request_body = PayeeGroupInput,
    responses((status = 201, body = PayeeGroupCreated), (status = 422, body = Problem))
)]
pub async fn create_group(
    State(state): State<AppState>,
    Json(input): Json<PayeeGroupInput>,
) -> AppResult<(StatusCode, Json<PayeeGroupCreated>)> {
    let id = payees::create_group(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(PayeeGroupCreated { id })))
}

#[utoipa::path(
    put, operation_id = "updatePayeeGroup", path = "/payee-groups/{id}", tag = "payees", params(("id" = PayeeGroupId, Path)),
    request_body = PayeeGroupInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update_group(
    State(state): State<AppState>,
    Path(id): Path<PayeeGroupId>,
    Json(input): Json<PayeeGroupInput>,
) -> AppResult<StatusCode> {
    payees::update_group(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un groupe ; ses tiers restent, sans groupe (R-TIE-4).
#[utoipa::path(
    delete, operation_id = "deletePayeeGroup", path = "/payee-groups/{id}", tag = "payees", params(("id" = PayeeGroupId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_group(State(state): State<AppState>, Path(id): Path<PayeeGroupId>) -> AppResult<StatusCode> {
    payees::delete_group(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fiche d'un groupe de tiers (E32).
#[utoipa::path(
    get, operation_id = "payeeGroupSummary", path = "/payee-groups/{id}/summary", tag = "payees",
    params(("id" = PayeeGroupId, Path)), responses((status = 200, body = PayeeSummary), (status = 404, body = Problem))
)]
pub async fn group_summary(
    State(state): State<AppState>,
    Path(id): Path<PayeeGroupId>,
) -> AppResult<Json<PayeeSummary>> {
    Ok(Json(payees::group_summary(&state.db, id, state.clock.today()).await?))
}

/// Dépenses ou revenus par tiers ou par groupe sur une période (TIE-04).
#[utoipa::path(
    get, operation_id = "payeeReport", path = "/reports/payees", tag = "payees", params(PayeeReportQuery),
    responses((status = 200, body = PayeeReport), (status = 422, body = Problem))
)]
pub async fn report(
    State(state): State<AppState>,
    Query(query): Query<PayeeReportQuery>,
) -> AppResult<Json<PayeeReport>> {
    Ok(Json(payees::report(&state.db, &query, state.clock.today()).await?))
}

/// Dépenses ou revenus par pays, département ou ville, et carte (TIE-07).
#[utoipa::path(
    get, operation_id = "payeeLocationReport", path = "/reports/payee-locations", tag = "payees",
    params(LocationReportQuery), responses((status = 200, body = LocationReport), (status = 422, body = Problem))
)]
pub async fn location_report(
    State(state): State<AppState>,
    Query(query): Query<LocationReportQuery>,
) -> AppResult<Json<LocationReport>> {
    Ok(Json(payees::location_report(&state.db, &query, state.clock.today()).await?))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AddressSearchQuery {
    /// Texte saisi, envoyé à la Base Adresse Nationale ; moins de 3 caractères : aucune proposition.
    pub q: String,
}

/// Recherche d'adresse par la Base Adresse Nationale, si elle est activée dans les réglages (R-TIE-26).
#[utoipa::path(
    get, operation_id = "searchAddress", path = "/geocoding/search", tag = "payees", params(AddressSearchQuery),
    responses(
        (status = 200, body = Vec<AddressSuggestion>), (status = 422, body = Problem), (status = 502, body = Problem)
    )
)]
pub async fn search_address(
    State(state): State<AppState>,
    Query(query): Query<AddressSearchQuery>,
) -> AppResult<Json<Vec<AddressSuggestion>>> {
    Ok(Json(payees::search_address(&state.db, &state.geocoder, &query.q).await?))
}
