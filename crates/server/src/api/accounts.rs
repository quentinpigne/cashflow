use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::Date;
use cashflow_domain::shared::ids::AccountId;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::error::{AppResult, Problem};
use crate::services::accounts::{self, Account, AccountInput, CloseInput, IbanView};
use crate::state::AppState;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AtQuery {
    /// Date du solde (CPT-03) ; aujourd'hui par défaut.
    pub at: Option<Date>,
}

/// Tous les comptes, avec leur solde ou leur valeur (CPT-01, CPT-03).
#[utoipa::path(get, operation_id = "listAccounts", path = "/accounts", tag = "accounts", params(AtQuery), responses((status = 200, body = Vec<Account>)))]
pub async fn list(State(state): State<AppState>, Query(q): Query<AtQuery>) -> AppResult<Json<Vec<Account>>> {
    Ok(Json(accounts::list(&state.db, q.at.unwrap_or_else(|| state.clock.today())).await?))
}

/// Crée un compte (CPT-01). Un compte à taux déterminé reçoit ses conditions préremplies (R-CPT-9).
#[utoipa::path(
    post, operation_id = "createAccount", path = "/accounts", tag = "accounts", request_body = AccountInput,
    responses((status = 201, body = Account), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<AccountInput>,
) -> AppResult<(StatusCode, Json<Account>)> {
    Ok((StatusCode::CREATED, Json(accounts::create(&state.db, &input, state.clock.today()).await?)))
}

#[utoipa::path(
    get, operation_id = "getAccount", path = "/accounts/{id}", tag = "accounts", params(("id" = AccountId, Path), AtQuery),
    responses((status = 200, body = Account), (status = 404, body = Problem))
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Query(q): Query<AtQuery>,
) -> AppResult<Json<Account>> {
    Ok(Json(accounts::get(&state.db, id, q.at.unwrap_or_else(|| state.clock.today())).await?))
}

#[utoipa::path(
    put, operation_id = "updateAccount", path = "/accounts/{id}", tag = "accounts", params(("id" = AccountId, Path)), request_body = AccountInput,
    responses((status = 200, body = Account), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Json(input): Json<AccountInput>,
) -> AppResult<Json<Account>> {
    Ok(Json(accounts::update(&state.db, id, &input, state.clock.today()).await?))
}

/// Supprime un compte sans transaction (R-CPT-7).
#[utoipa::path(
    delete, operation_id = "deleteAccount", path = "/accounts/{id}", tag = "accounts", params(("id" = AccountId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<StatusCode> {
    accounts::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// IBAN complet, affiché à la demande (R-CPT-8).
#[utoipa::path(
    get, operation_id = "revealIban", path = "/accounts/{id}/iban", tag = "accounts", params(("id" = AccountId, Path)),
    responses((status = 200, body = IbanView), (status = 404, body = Problem))
)]
pub async fn reveal_iban(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<Json<IbanView>> {
    Ok(Json(accounts::reveal_iban(&state.db, id).await?))
}

/// Clôture un compte (CPT-04, R-CPT-5).
#[utoipa::path(
    post, operation_id = "closeAccount", path = "/accounts/{id}/close", tag = "accounts", params(("id" = AccountId, Path)), request_body = CloseInput,
    responses((status = 200, body = Account), (status = 422, body = Problem))
)]
pub async fn close(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Json(input): Json<CloseInput>,
) -> AppResult<Json<Account>> {
    Ok(Json(accounts::close(&state.db, id, &input, state.clock.today()).await?))
}

/// Rouvre un compte clôturé.
#[utoipa::path(
    post, operation_id = "reopenAccount", path = "/accounts/{id}/reopen", tag = "accounts", params(("id" = AccountId, Path)),
    responses((status = 200, body = Account))
)]
pub async fn reopen(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<Json<Account>> {
    Ok(Json(accounts::reopen(&state.db, id, state.clock.today()).await?))
}
