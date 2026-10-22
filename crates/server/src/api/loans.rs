use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use cashflow_domain::Date;
use cashflow_domain::shared::ids::{LoanId, TransactionId};

use crate::error::{AppResult, Problem};
use crate::services::loans::{
    self, BalanceInput, LoanInput, LoanPaymentInput, LoanPaymentProposal, LoanPaymentResult, LoanPreview,
    LoanPreviewInput, LoanSummary, LoanView,
};
use crate::state::AppState;

/// Prêts (E27, PRE-01).
#[utoipa::path(get, operation_id = "listLoans", path = "/loans", tag = "loans", responses((status = 200, body = Vec<LoanSummary>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<LoanSummary>>> {
    Ok(Json(loans::list(&state.db, state.clock.today()).await?))
}

#[utoipa::path(
    post, operation_id = "createLoan", path = "/loans", tag = "loans", request_body = LoanInput,
    responses((status = 201, body = LoanView), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<LoanInput>,
) -> AppResult<(StatusCode, Json<LoanView>)> {
    Ok((StatusCode::CREATED, Json(loans::create(&state.db, &input, state.clock.today()).await?)))
}

/// Fiche d'un prêt : tableau d'amortissement ou capitaux restants dus saisis (E28).
#[utoipa::path(
    get, operation_id = "getLoan", path = "/loans/{id}", tag = "loans", params(("id" = LoanId, Path)),
    responses((status = 200, body = LoanView), (status = 404, body = Problem))
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<LoanId>) -> AppResult<Json<LoanView>> {
    Ok(Json(loans::get(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateLoan", path = "/loans/{id}", tag = "loans", params(("id" = LoanId, Path)),
    request_body = LoanInput, responses((status = 200, body = LoanView), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<LoanId>,
    Json(input): Json<LoanInput>,
) -> AppResult<Json<LoanView>> {
    Ok(Json(loans::update(&state.db, id, &input, state.clock.today()).await?))
}

/// Supprime un prêt ; ses mensualités ventilées redeviennent non catégorisées (R-PRE-11).
#[utoipa::path(
    delete, operation_id = "deleteLoan", path = "/loans/{id}", tag = "loans", params(("id" = LoanId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<LoanId>) -> AppResult<StatusCode> {
    loans::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Saisit un capital restant dû (suivi simple, R-PRE-5).
#[utoipa::path(
    post, operation_id = "saveLoanBalance", path = "/loans/{id}/balances", tag = "loans",
    params(("id" = LoanId, Path)), request_body = BalanceInput,
    responses((status = 200, body = LoanView), (status = 422, body = Problem))
)]
pub async fn save_balance(
    State(state): State<AppState>,
    Path(id): Path<LoanId>,
    Json(input): Json<BalanceInput>,
) -> AppResult<Json<LoanView>> {
    Ok(Json(loans::save_balance(&state.db, id, &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteLoanBalance", path = "/loans/{id}/balances/{date}", tag = "loans",
    params(("id" = LoanId, Path), ("date" = Date, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_balance(
    State(state): State<AppState>,
    Path((id, date)): Path<(LoanId, Date)>,
) -> AppResult<StatusCode> {
    loans::delete_balance(&state.db, id, date).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Mensualité calculée pendant la saisie des conditions (R-PRE-3).
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

/// Mensualités déjà enregistrées qui correspondent à des échéances non payées (R-PRE-6).
#[utoipa::path(
    get, operation_id = "loanPaymentProposals", path = "/loans/{id}/proposals", tag = "loans",
    params(("id" = LoanId, Path)),
    responses((status = 200, body = Vec<LoanPaymentProposal>), (status = 404, body = Problem))
)]
pub async fn proposals(
    State(state): State<AppState>,
    Path(id): Path<LoanId>,
) -> AppResult<Json<Vec<LoanPaymentProposal>>> {
    Ok(Json(loans::proposals(&state.db, id, state.clock.today()).await?))
}

/// Ventile des mensualités selon leurs échéances (PRE-03, R-PRE-6).
#[utoipa::path(
    post, operation_id = "payLoanInstallments", path = "/loans/{id}/payments", tag = "loans",
    params(("id" = LoanId, Path)), request_body = Vec<LoanPaymentInput>,
    responses((status = 200, body = LoanPaymentResult), (status = 422, body = Problem))
)]
pub async fn pay(
    State(state): State<AppState>,
    Path(id): Path<LoanId>,
    Json(input): Json<Vec<LoanPaymentInput>>,
) -> AppResult<Json<LoanPaymentResult>> {
    Ok(Json(loans::pay(&state.db, id, &input).await?))
}

/// Annule la ventilation d'une échéance (R-PRE-8).
#[utoipa::path(
    delete, operation_id = "unpayLoanInstallment", path = "/loans/{id}/payments/{number}", tag = "loans",
    params(("id" = LoanId, Path), ("number" = u32, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn unpay(State(state): State<AppState>, Path((id, number)): Path<(LoanId, u32)>) -> AppResult<StatusCode> {
    loans::unpay(&state.db, id, number).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// L'échéance de prêt à laquelle correspond une transaction, s'il y en a une (R-PRE-6).
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
