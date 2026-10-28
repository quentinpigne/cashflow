use axum::Json;
use axum::extract::{Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use cashflow_domain::shared::ids::{AccountId, ImportBatchId, ImportLineId, ImportProfileId};
use utoipa::ToSchema;

use crate::error::{AppError, AppResult, Problem};
use crate::readers::FileFormat;
use crate::readers::profile::ProfileSettings;
use crate::services::imports::{
    self, BatchCreated, BatchDetail, BatchSummary, BulkInput, CommitInput, CommitResult, ImportPreview,
    LineReviewInput, Profile, ProfileInput, Upload,
};
use crate::state::AppState;

/// Formulaire d'envoi d'un relevé (`multipart/form-data`).
#[derive(ToSchema)]
#[allow(dead_code)]
pub struct UploadForm {
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
    account_id: AccountId,
    /// Profil existant ; sinon, `settings`.
    profile_id: Option<ImportProfileId>,
    /// Imposé ; sinon déduit du nom et du contenu du fichier.
    format: Option<FileFormat>,
    /// Réglages CSV/XLSX, en JSON (schéma `ProfileSettings`).
    settings: Option<String>,
    /// Enregistre les réglages comme nouveau profil sous ce nom.
    save_profile_as: Option<String>,
}

async fn upload(mut multipart: Multipart) -> AppResult<Upload> {
    let bad = |message: &str| AppError::invalid("INVALID_UPLOAD", message.to_owned());
    let (mut file_name, mut content, mut account, mut profile, mut format, mut settings, mut save) =
        (None, None, None, None, None, None, None);
    while let Some(field) = multipart.next_field().await.map_err(|e| bad(&e.to_string()))? {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "file" => {
                file_name = field.file_name().map(str::to_owned);
                content = Some(field.bytes().await.map_err(|e| bad(&e.to_string()))?.to_vec());
            }
            other => {
                let text = field.text().await.map_err(|e| bad(&e.to_string()))?;
                if text.is_empty() || text == "null" {
                    continue;
                }
                match other {
                    "account_id" => account = Some(text.parse().map_err(|_| bad("compte invalide"))?),
                    "profile_id" => profile = Some(text.parse().map_err(|_| bad("profil invalide"))?),
                    "format" => {
                        format = Some(
                            serde_json::from_value::<FileFormat>(serde_json::Value::String(text))
                                .map_err(|_| bad("format invalide"))?,
                        )
                    }
                    "settings" => {
                        settings = Some(
                            serde_json::from_str::<ProfileSettings>(&text)
                                .map_err(|e| AppError::invalid("INVALID_SETTINGS", e.to_string()))?,
                        )
                    }
                    "save_profile_as" => save = Some(text),
                    _ => {}
                }
            }
        }
    }
    Ok(Upload {
        file_name: file_name.unwrap_or_else(|| "releve".into()),
        content: content.ok_or_else(|| bad("fichier manquant"))?,
        account_id: account.ok_or_else(|| bad("compte manquant"))?,
        profile_id: profile,
        format,
        settings,
        save_profile_as: save,
    })
}

/// Aperçu en direct d'un relevé avec un profil ou des réglages (IMP-01). N'enregistre rien.
#[utoipa::path(
    post, operation_id = "previewImport", path = "/imports/preview", tag = "imports",
    request_body(content = UploadForm, content_type = "multipart/form-data"),
    responses((status = 200, body = ImportPreview), (status = 422, body = Problem))
)]
pub async fn preview(State(state): State<AppState>, multipart: Multipart) -> AppResult<Json<ImportPreview>> {
    let upload = upload(multipart).await?;
    Ok(Json(imports::preview(&state.db, &upload, state.clock.today()).await?))
}

/// Lit le relevé et crée un lot en revue (IMP-01 à IMP-05). Aucune transaction n'est modifiée (R-IMP-1).
#[utoipa::path(
    post, operation_id = "startImport", path = "/imports", tag = "imports",
    request_body(content = UploadForm, content_type = "multipart/form-data"),
    responses((status = 201, body = BatchCreated), (status = 422, body = Problem))
)]
pub async fn start(State(state): State<AppState>, multipart: Multipart) -> AppResult<(StatusCode, Json<BatchCreated>)> {
    let upload = upload(multipart).await?;
    Ok((StatusCode::CREATED, Json(imports::start(&state.db, &state.data, &upload, state.clock.today()).await?)))
}

/// Historique des lots (IMP-06).
#[utoipa::path(get, operation_id = "listImports", path = "/imports", tag = "imports", responses((status = 200, body = Vec<BatchSummary>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<BatchSummary>>> {
    Ok(Json(imports::list(&state.db, &state.data).await?))
}

/// Lot et lignes, pour la revue (IMP-04).
#[utoipa::path(
    get, operation_id = "getImport", path = "/imports/{id}", tag = "imports", params(("id" = ImportBatchId, Path)),
    responses((status = 200, body = BatchDetail), (status = 404, body = Problem))
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<ImportBatchId>) -> AppResult<Json<BatchDetail>> {
    Ok(Json(imports::get(&state.db, &state.data, id).await?))
}

/// Annule une revue : le lot et ses lignes sont supprimés (R-IMP-1).
#[utoipa::path(
    delete, operation_id = "cancelImport", path = "/imports/{id}", tag = "imports", params(("id" = ImportBatchId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn cancel(State(state): State<AppState>, Path(id): Path<ImportBatchId>) -> AppResult<StatusCode> {
    imports::cancel(&state.db, &state.data, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Décision et choix d'une ligne (IMP-04).
#[utoipa::path(
    put, operation_id = "reviewImportLine", path = "/imports/{id}/lines/{line_id}", tag = "imports",
    params(("id" = ImportBatchId, Path), ("line_id" = ImportLineId, Path)), request_body = LineReviewInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn review_line(
    State(state): State<AppState>,
    Path((id, line_id)): Path<(ImportBatchId, ImportLineId)>,
    Json(input): Json<LineReviewInput>,
) -> AppResult<StatusCode> {
    imports::review_line(&state.db, id, line_id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Décision ou catégorie appliquée à plusieurs lignes (IMP-04).
#[utoipa::path(
    post, operation_id = "bulkReviewImport", path = "/imports/{id}/lines/bulk", tag = "imports",
    params(("id" = ImportBatchId, Path)), request_body = BulkInput, responses((status = 204))
)]
pub async fn bulk(
    State(state): State<AppState>,
    Path(id): Path<ImportBatchId>,
    Json(input): Json<BulkInput>,
) -> AppResult<StatusCode> {
    imports::bulk(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Valide le lot, de façon atomique (R-IMP-15, R-IMP-16).
#[utoipa::path(
    post, operation_id = "commitImport", path = "/imports/{id}/commit", tag = "imports",
    params(("id" = ImportBatchId, Path)), request_body = CommitInput,
    responses((status = 200, body = CommitResult), (status = 422, body = Problem))
)]
pub async fn commit(
    State(state): State<AppState>,
    Path(id): Path<ImportBatchId>,
    Json(input): Json<CommitInput>,
) -> AppResult<Json<CommitResult>> {
    Ok(Json(imports::commit(&state.db, &state.data, id, &input, state.clock.today()).await?))
}

/// Annule un lot validé (R-IMP-17).
#[utoipa::path(
    post, operation_id = "revertImport", path = "/imports/{id}/revert", tag = "imports",
    params(("id" = ImportBatchId, Path)), responses((status = 204), (status = 422, body = Problem))
)]
pub async fn revert(State(state): State<AppState>, Path(id): Path<ImportBatchId>) -> AppResult<StatusCode> {
    imports::revert(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Télécharge le fichier source conservé (R-IMP-21).
#[utoipa::path(
    get, operation_id = "downloadImportFile", path = "/imports/{id}/file", tag = "imports", params(("id" = ImportBatchId, Path)),
    responses((status = 200, description = "Fichier source", content_type = "application/octet-stream"), (status = 404, body = Problem))
)]
pub async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<ImportBatchId>,
) -> AppResult<impl IntoResponse> {
    let (name, content) = imports::source_file(&state.db, &state.data, id).await?;
    let disposition = format!("attachment; filename=\"{}\"", name.replace('"', ""));
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream".to_owned()), (header::CONTENT_DISPOSITION, disposition)],
        content,
    ))
}

/// Supprime le fichier source conservé ; le lot reste (R-IMP-21).
#[utoipa::path(
    delete, operation_id = "deleteImportFile", path = "/imports/{id}/file", tag = "imports", params(("id" = ImportBatchId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_file(State(state): State<AppState>, Path(id): Path<ImportBatchId>) -> AppResult<StatusCode> {
    imports::delete_file(&state.db, &state.data, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Profils d'import enregistrés (IMP-02).
#[utoipa::path(get, operation_id = "listImportProfiles", path = "/import-profiles", tag = "imports", responses((status = 200, body = Vec<Profile>)))]
pub async fn list_profiles(State(state): State<AppState>) -> AppResult<Json<Vec<Profile>>> {
    Ok(Json(imports::list_profiles(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createImportProfile", path = "/import-profiles", tag = "imports", request_body = ProfileInput,
    responses((status = 201, body = Profile), (status = 422, body = Problem))
)]
pub async fn create_profile(
    State(state): State<AppState>,
    Json(input): Json<ProfileInput>,
) -> AppResult<(StatusCode, Json<Profile>)> {
    Ok((StatusCode::CREATED, Json(imports::save_profile(&state.db, None, &input).await?)))
}

#[utoipa::path(
    put, operation_id = "updateImportProfile", path = "/import-profiles/{id}", tag = "imports",
    params(("id" = ImportProfileId, Path)), request_body = ProfileInput, responses((status = 200, body = Profile), (status = 422, body = Problem))
)]
pub async fn update_profile(
    State(state): State<AppState>,
    Path(id): Path<ImportProfileId>,
    Json(input): Json<ProfileInput>,
) -> AppResult<Json<Profile>> {
    Ok(Json(imports::save_profile(&state.db, Some(id), &input).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteImportProfile", path = "/import-profiles/{id}", tag = "imports",
    params(("id" = ImportProfileId, Path)), responses((status = 204))
)]
pub async fn delete_profile(State(state): State<AppState>, Path(id): Path<ImportProfileId>) -> AppResult<StatusCode> {
    imports::delete_profile(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
