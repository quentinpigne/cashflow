use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::{AccountId, SplitId, TransactionId, TransferId};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::transactions::{
    self, CategorizeInput, TransactionDetail, TransactionFilter, TransactionInput, TransactionPage,
    TransactionUpdateInput,
};
use crate::services::transfers::{
    self, Candidate, CandidatesQuery, MarkAsTransferInput, TransferCreated, TransferInput,
};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct TransactionCreated {
    pub id: TransactionId,
}

#[derive(Serialize, ToSchema)]
pub struct TransferLinked {
    pub transfer_id: TransferId,
}

/// Transactions d'un compte, les plus récentes d'abord, avec le solde après opération (TRX-02).
#[utoipa::path(
    get, operation_id = "listTransactions", path = "/accounts/{id}/transactions", tag = "transactions",
    params(("id" = AccountId, Path), TransactionFilter),
    responses((status = 200, body = TransactionPage), (status = 404, body = Problem))
)]
pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Query(filter): Query<TransactionFilter>,
) -> AppResult<Json<TransactionPage>> {
    Ok(Json(transactions::list(&state.db, id, &filter).await?))
}

/// Saisit une transaction, `PENDING` (TRX-01, TRX-03).
#[utoipa::path(
    post, operation_id = "createTransaction", path = "/transactions", tag = "transactions", request_body = TransactionInput,
    responses((status = 201, body = TransactionCreated), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<TransactionInput>,
) -> AppResult<(StatusCode, Json<TransactionCreated>)> {
    let id = transactions::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(TransactionCreated { id })))
}

#[utoipa::path(
    get, operation_id = "getTransaction", path = "/transactions/{id}", tag = "transactions", params(("id" = TransactionId, Path)),
    responses((status = 200, body = TransactionDetail), (status = 404, body = Problem))
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<TransactionId>) -> AppResult<Json<TransactionDetail>> {
    Ok(Json(transactions::get(&state.db, id).await?))
}

/// Modifie une transaction (R-TRX-3, R-TRX-4).
#[utoipa::path(
    put, operation_id = "updateTransaction", path = "/transactions/{id}", tag = "transactions", params(("id" = TransactionId, Path)),
    request_body = TransactionUpdateInput,
    responses((status = 200, body = TransactionDetail), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<TransactionId>,
    Json(input): Json<TransactionUpdateInput>,
) -> AppResult<Json<TransactionDetail>> {
    transactions::update(&state.db, id, &input).await?;
    Ok(Json(transactions::get(&state.db, id).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteTransaction", path = "/transactions/{id}", tag = "transactions", params(("id" = TransactionId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<TransactionId>) -> AppResult<StatusCode> {
    transactions::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Catégorise une ventilation sur place (`ecrans.md` E1).
#[utoipa::path(
    put, operation_id = "categorizeSplit", path = "/splits/{id}/category", tag = "transactions", params(("id" = SplitId, Path)),
    request_body = CategorizeInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn categorize_split(
    State(state): State<AppState>,
    Path(id): Path<SplitId>,
    Json(input): Json<CategorizeInput>,
) -> AppResult<StatusCode> {
    transactions::categorize_split(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Saisit un virement interne : les deux jambes d'un coup (TRX-05, R-TRX-33).
#[utoipa::path(
    post, operation_id = "createTransfer", path = "/transfers", tag = "transactions", request_body = TransferInput,
    responses((status = 201, body = TransferCreated), (status = 422, body = Problem))
)]
pub async fn create_transfer(
    State(state): State<AppState>,
    Json(input): Json<TransferInput>,
) -> AppResult<(StatusCode, Json<TransferCreated>)> {
    Ok((StatusCode::CREATED, Json(transfers::create(&state.db, &input).await?)))
}

/// Transactions candidates pour former un virement (R-TRX-35).
#[utoipa::path(
    get, operation_id = "transferCandidates", path = "/transactions/{id}/transfer-candidates", tag = "transactions",
    params(("id" = TransactionId, Path), CandidatesQuery), responses((status = 200, body = Vec<Candidate>))
)]
pub async fn transfer_candidates(
    State(state): State<AppState>,
    Path(id): Path<TransactionId>,
    Query(query): Query<CandidatesQuery>,
) -> AppResult<Json<Vec<Candidate>>> {
    Ok(Json(transfers::candidates(&state.db, id, &query).await?))
}

/// Marque une transaction comme virement (R-TRX-36).
#[utoipa::path(
    post, operation_id = "markAsTransfer", path = "/transactions/{id}/transfer", tag = "transactions", params(("id" = TransactionId, Path)),
    request_body = MarkAsTransferInput, responses((status = 201, body = TransferLinked), (status = 422, body = Problem))
)]
pub async fn mark_as_transfer(
    State(state): State<AppState>,
    Path(id): Path<TransactionId>,
    Json(input): Json<MarkAsTransferInput>,
) -> AppResult<(StatusCode, Json<TransferLinked>)> {
    let transfer_id = transfers::mark_as_transfer(&state.db, id, &input).await?;
    Ok((StatusCode::CREATED, Json(TransferLinked { transfer_id })))
}

/// Délie un virement (R-TRX-34).
#[utoipa::path(delete, operation_id = "unlinkTransfer", path = "/transfers/{id}", tag = "transactions", params(("id" = TransferId, Path)), responses((status = 204)))]
pub async fn unlink_transfer(State(state): State<AppState>, Path(id): Path<TransferId>) -> AppResult<StatusCode> {
    transfers::unlink(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
