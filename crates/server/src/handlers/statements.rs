use axum::Json;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cashflow_domain::Date;
use cashflow_domain::shared::ids::{AccountId, BankStatementId};
use utoipa::ToSchema;

use super::{FileQuery, document_response};
use crate::error::{AppError, AppResult, Problem};
use crate::services::statements::{self, AccountStatements, StatementInput, StatementUpload};
use crate::state::AppState;

/// Relevés archivés d'un compte et mois sans relevé (CPT-11, R-CPT-21).
#[utoipa::path(
    get, operation_id = "listStatements", path = "/accounts/{id}/statements", tag = "accounts",
    params(("id" = AccountId, Path)),
    responses((status = 200, body = AccountStatements), (status = 404, body = Problem))
)]
pub async fn list(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<Json<AccountStatements>> {
    Ok(Json(statements::list(&state.db, id, state.clock.today()).await?))
}

/// Formulaire d'envoi d'un relevé (`multipart/form-data`).
#[derive(ToSchema)]
#[allow(dead_code)]
pub struct StatementForm {
    /// PDF, 10 Mo au plus (R-CPT-18).
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
    /// Premier jour du mois du relevé.
    statement_month: Date,
    note: Option<String>,
}

async fn statement_upload(mut multipart: Multipart) -> AppResult<StatementUpload> {
    let bad = |message: &str| AppError::invalid("INVALID_UPLOAD", message.to_owned());
    let (mut file_name, mut content, mut statement_month, mut note) = (None, None, None, None);
    while let Some(field) = multipart.next_field().await.map_err(|e| bad(&e.to_string()))? {
        match field.name().unwrap_or_default() {
            "file" => {
                file_name = field.file_name().map(str::to_owned);
                content = Some(field.bytes().await.map_err(|e| bad(&e.to_string()))?.to_vec());
            }
            "statement_month" => {
                let text = field.text().await.map_err(|e| bad(&e.to_string()))?;
                statement_month = Some(text.parse::<Date>().map_err(|_| bad("mois du relevé invalide"))?);
            }
            "note" => note = Some(field.text().await.map_err(|e| bad(&e.to_string()))?),
            _ => {}
        }
    }
    Ok(StatementUpload {
        file_name: file_name.unwrap_or_default(),
        content: content.ok_or_else(|| bad("fichier manquant"))?,
        statement_month: statement_month.ok_or_else(|| bad("mois du relevé manquant"))?,
        note,
    })
}

/// Archive un relevé envoyé depuis le compte (R-CPT-18, R-CPT-19).
#[utoipa::path(
    post, operation_id = "createStatement", path = "/accounts/{id}/statements", tag = "accounts",
    params(("id" = AccountId, Path)), request_body(content = StatementForm, content_type = "multipart/form-data"),
    responses((status = 200, body = AccountStatements), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    multipart: Multipart,
) -> AppResult<Json<AccountStatements>> {
    let upload = statement_upload(multipart).await?;
    Ok(Json(statements::add(&state.db, &state.data, id, upload, state.clock.today()).await?))
}

/// Modifie le mois et la note d'un relevé (R-CPT-20).
#[utoipa::path(
    put, operation_id = "updateStatement", path = "/accounts/{id}/statements/{statement_id}", tag = "accounts",
    params(("id" = AccountId, Path), ("statement_id" = BankStatementId, Path)), request_body = StatementInput,
    responses((status = 200, body = AccountStatements), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path((id, statement)): Path<(AccountId, BankStatementId)>,
    Json(input): Json<StatementInput>,
) -> AppResult<Json<AccountStatements>> {
    Ok(Json(statements::update(&state.db, id, statement, &input, state.clock.today()).await?))
}

/// Supprime un relevé et son document (R-CPT-20).
#[utoipa::path(
    delete, operation_id = "deleteStatement", path = "/accounts/{id}/statements/{statement_id}", tag = "accounts",
    params(("id" = AccountId, Path), ("statement_id" = BankStatementId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete(
    State(state): State<AppState>,
    Path((id, statement)): Path<(AccountId, BankStatementId)>,
) -> AppResult<StatusCode> {
    statements::delete(&state.db, &state.data, id, statement, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fichier d'un relevé, affiché ou téléchargé (R-CPT-21, ADR-0024).
#[utoipa::path(
    get, operation_id = "downloadStatement", path = "/accounts/{id}/statements/{statement_id}/file", tag = "accounts",
    params(("id" = AccountId, Path), ("statement_id" = BankStatementId, Path), FileQuery),
    responses((status = 200, description = "Fichier PDF du relevé", content_type = "application/pdf"), (status = 404, body = Problem))
)]
pub async fn download(
    State(state): State<AppState>,
    Path((id, statement)): Path<(AccountId, BankStatementId)>,
    Query(query): Query<FileQuery>,
) -> AppResult<impl IntoResponse> {
    let (row, content) = statements::file(&state.db, &state.data, id, statement).await?;
    Ok(document_response(&row.file_name, row.media_type, content, query.download))
}
