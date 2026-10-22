use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::AccountId;

use crate::error::{AppResult, Problem};
use cashflow_domain::shared::ids::TransactionId;

use crate::services::loans::{
    self, LoanPaymentInput, LoanPaymentProposal, LoanPaymentResult, LoanPreview, LoanPreviewInput, LoanTermsInput,
    LoanView,
};
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

/// Mensualités déjà enregistrées qui correspondent à des échéances non payées (R-CPT-17).
#[utoipa::path(
    get, operation_id = "loanPaymentProposals", path = "/accounts/{id}/loan/proposals", tag = "loans",
    params(("id" = AccountId, Path)),
    responses((status = 200, body = Vec<LoanPaymentProposal>), (status = 422, body = Problem))
)]
pub async fn proposals(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
) -> AppResult<Json<Vec<LoanPaymentProposal>>> {
    Ok(Json(loans::proposals(&state.db, id, state.clock.today()).await?))
}

/// Ventile des mensualités selon leurs échéances (CPT-10, R-CPT-17).
#[utoipa::path(
    post, operation_id = "payLoanInstallments", path = "/accounts/{id}/loan/payments", tag = "loans",
    params(("id" = AccountId, Path)), request_body = Vec<LoanPaymentInput>,
    responses((status = 200, body = LoanPaymentResult), (status = 422, body = Problem))
)]
pub async fn pay(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Json(input): Json<Vec<LoanPaymentInput>>,
) -> AppResult<Json<LoanPaymentResult>> {
    Ok(Json(loans::pay(&state.db, id, &input).await?))
}

/// Annule la ventilation d'une échéance (R-CPT-17).
#[utoipa::path(
    delete, operation_id = "unpayLoanInstallment", path = "/accounts/{id}/loan/payments/{number}", tag = "loans",
    params(("id" = AccountId, Path), ("number" = u32, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn unpay(State(state): State<AppState>, Path((id, number)): Path<(AccountId, u32)>) -> AppResult<StatusCode> {
    loans::unpay(&state.db, id, number).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// L'échéance de prêt à laquelle correspond une transaction, s'il y en a une (R-CPT-17).
#[utoipa::path(
    get, operation_id = "transactionLoanProposal", path = "/transactions/{id}/loan-proposal", tag = "loans",
    params(("id" = TransactionId, Path)), responses((status = 200, body = Option<LoanPaymentProposal>))
)]
pub async fn transaction_proposal(
    State(state): State<AppState>,
    Path(id): Path<TransactionId>,
) -> AppResult<Json<Option<LoanPaymentProposal>>> {
    Ok(Json(loans::proposals_among(&state.db, &[id]).await?.into_iter().next()))
}
