use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::AccountId;

use crate::error::{AppResult, Problem};
use crate::services::loans::{self, LoanPreview, LoanPreviewInput, LoanTermsInput, LoanView};
use crate::state::AppState;

/// Conditions et tableau d'amortissement d'un prêt (CPT-09).
#[utoipa::path(
    get, operation_id = "getLoan", path = "/accounts/{id}/loan", tag = "loans", params(("id" = AccountId, Path)),
    responses((status = 200, body = LoanView), (status = 422, body = Problem))
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<Json<LoanView>> {
    Ok(Json(loans::get(&state.db, id, state.clock.today()).await?))
}

/// Crée ou remplace les conditions d'un prêt (R-CPT-15).
#[utoipa::path(
    put, operation_id = "saveLoanTerms", path = "/accounts/{id}/loan", tag = "loans",
    params(("id" = AccountId, Path)), request_body = LoanTermsInput,
    responses((status = 200, body = LoanView), (status = 422, body = Problem))
)]
pub async fn save(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Json(input): Json<LoanTermsInput>,
) -> AppResult<Json<LoanView>> {
    Ok(Json(loans::save_terms(&state.db, id, &input, state.clock.today()).await?))
}

/// Retire les conditions d'un prêt : il revient au suivi par valorisations (R-CPT-12).
#[utoipa::path(
    delete, operation_id = "deleteLoanTerms", path = "/accounts/{id}/loan", tag = "loans",
    params(("id" = AccountId, Path)), responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<StatusCode> {
    loans::delete_terms(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Mensualité calculée pendant la saisie des conditions (R-CPT-16).
#[utoipa::path(
    post, operation_id = "previewLoan", path = "/loans/preview", tag = "loans", request_body = LoanPreviewInput,
    responses((status = 200, body = LoanPreview), (status = 422, body = Problem))
)]
pub async fn preview(
    State(state): State<AppState>,
    Json(input): Json<LoanPreviewInput>,
) -> AppResult<Json<LoanPreview>> {
    Ok(Json(loans::preview(&input, state.clock.today())?))
}
