use axum::Json;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cashflow_domain::Date;
use cashflow_domain::documents::{DocumentKind, LinkKind, LinkTarget};
use cashflow_domain::shared::ids::DocumentId;
use utoipa::ToSchema;

use super::{FileQuery, document_response};
use crate::error::{AppError, AppResult, Problem};
use crate::services::documents::{self, AttachedDocuments, DocumentFilter, DocumentInput, DocumentView, Upload};
use crate::state::AppState;

/// Documents de la bibliothèque, cherchés et filtrés (DOC-03, R-DOC-18 à R-DOC-20).
#[utoipa::path(
    get, operation_id = "listDocuments", path = "/documents", tag = "documents", params(DocumentFilter),
    responses((status = 200, body = Vec<DocumentView>))
)]
pub async fn list(
    State(state): State<AppState>,
    Query(filter): Query<DocumentFilter>,
) -> AppResult<Json<Vec<DocumentView>>> {
    Ok(Json(documents::list(&state.db, &filter, state.clock.today()).await?))
}

/// Formulaire d'envoi d'un document (`multipart/form-data`) : seul le fichier est requis (R-DOC-14).
#[derive(ToSchema)]
#[allow(dead_code)]
pub struct DocumentForm {
    /// PDF, JPEG ou PNG, 10 Mo au plus (R-DOC-1).
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
    kind: Option<DocumentKind>,
    /// Par défaut, le nom du fichier sans son extension, ou « Relevé <compte> <mois> » (R-DOC-4, R-DOC-15).
    title: Option<String>,
    document_date: Option<Date>,
    /// Premier jour du mois, pour un relevé ou une fiche de paie (R-DOC-7).
    month: Option<Date>,
    note: Option<String>,
    /// Objet depuis lequel le document est envoyé, auquel il est rattaché d'office (R-DOC-15).
    link_kind: Option<LinkKind>,
    #[schema(value_type = Option<String>, format = Uuid)]
    link_id: Option<uuid::Uuid>,
}

async fn upload(mut multipart: Multipart) -> AppResult<Upload> {
    let bad = |message: &str| AppError::invalid("INVALID_UPLOAD", message.to_owned());
    let mut upload = Upload::default();
    let (mut content, mut link_kind, mut link_id) = (None, None, None);
    while let Some(field) = multipart.next_field().await.map_err(|e| bad(&e.to_string()))? {
        let name = field.name().unwrap_or_default().to_owned();
        if name == "file" {
            upload.file_name = field.file_name().unwrap_or_default().to_owned();
            content = Some(field.bytes().await.map_err(|e| bad(&e.to_string()))?.to_vec());
            continue;
        }
        let text = field.text().await.map_err(|e| bad(&e.to_string()))?;
        if text.is_empty() {
            continue;
        }
        let date = |label: &str| text.parse::<Date>().map_err(|_| bad(&format!("{label} invalide")));
        match name.as_str() {
            "kind" => upload.kind = Some(DocumentKind::parse(&text).ok_or_else(|| bad("type de document inconnu"))?),
            "title" => upload.title = Some(text.clone()),
            "document_date" => upload.document_date = Some(date("date du document")?),
            "month" => upload.month = Some(date("mois")?),
            "note" => upload.note = Some(text.clone()),
            "link_kind" => {
                let kind = serde_json::from_value(serde_json::Value::String(text.clone()));
                link_kind = Some(kind.map_err(|_| bad("nature d'objet inconnue"))?);
            }
            "link_id" => link_id = Some(text.parse::<uuid::Uuid>().map_err(|_| bad("objet invalide"))?),
            _ => {}
        }
    }
    upload.content = content.ok_or_else(|| bad("fichier manquant"))?;
    upload.target = match (link_kind, link_id) {
        (Some(kind), Some(id)) => Some(LinkTarget { kind, id }),
        (None, None) => None,
        _ => return Err(bad("nature et identifiant de l'objet vont ensemble")),
    };
    Ok(upload)
}

/// Archive un document envoyé, rattaché d'office à l'objet depuis lequel il l'est (DOC-01, R-DOC-14, R-DOC-15).
#[utoipa::path(
    post, operation_id = "createDocument", path = "/documents", tag = "documents",
    request_body(content = DocumentForm, content_type = "multipart/form-data"),
    responses((status = 200, body = DocumentView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn create(State(state): State<AppState>, multipart: Multipart) -> AppResult<Json<DocumentView>> {
    let upload = upload(multipart).await?;
    Ok(Json(documents::create(&state.db, &state.data, upload, state.clock.today()).await?))
}

#[utoipa::path(
    get, operation_id = "getDocument", path = "/documents/{id}", tag = "documents", params(("id" = DocumentId, Path)),
    responses((status = 200, body = DocumentView), (status = 404, body = Problem))
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<DocumentId>) -> AppResult<Json<DocumentView>> {
    Ok(Json(documents::get(&state.db, id, state.clock.today()).await?))
}

/// Modifie la description d'un document ; le fichier ne change pas (R-DOC-2).
#[utoipa::path(
    put, operation_id = "updateDocument", path = "/documents/{id}", tag = "documents",
    params(("id" = DocumentId, Path)), request_body = DocumentInput,
    responses((status = 200, body = DocumentView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<DocumentId>,
    Json(input): Json<DocumentInput>,
) -> AppResult<Json<DocumentView>> {
    Ok(Json(documents::update(&state.db, id, &input, state.clock.today()).await?))
}

/// Supprime un document et ses rattachements (R-DOC-22).
#[utoipa::path(
    delete, operation_id = "deleteDocument", path = "/documents/{id}", tag = "documents",
    params(("id" = DocumentId, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<DocumentId>) -> AppResult<StatusCode> {
    documents::delete(&state.db, &state.data, id, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fichier d'un document, affiché ou téléchargé (R-DOC-21, ADR-0024).
#[utoipa::path(
    get, operation_id = "downloadDocument", path = "/documents/{id}/content", tag = "documents",
    params(("id" = DocumentId, Path), FileQuery),
    responses((status = 200, description = "Fichier PDF, JPEG ou PNG", content_type = "application/octet-stream"), (status = 404, body = Problem))
)]
pub async fn content(
    State(state): State<AppState>,
    Path(id): Path<DocumentId>,
    Query(query): Query<FileQuery>,
) -> AppResult<impl IntoResponse> {
    let (row, content) = documents::file(&state.db, &state.data, id).await?;
    Ok(document_response(&row.file_name, row.media_type, content, query.download))
}

/// Rattache un document à un objet (DOC-02, R-DOC-9 à R-DOC-11).
#[utoipa::path(
    post, operation_id = "linkDocument", path = "/documents/{id}/links", tag = "documents",
    params(("id" = DocumentId, Path)), request_body = LinkTarget,
    responses((status = 200, body = DocumentView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn link(
    State(state): State<AppState>,
    Path(id): Path<DocumentId>,
    Json(target): Json<LinkTarget>,
) -> AppResult<Json<DocumentView>> {
    Ok(Json(documents::link(&state.db, id, target, state.clock.today()).await?))
}

/// Détache un document d'un objet (DOC-02, R-DOC-9).
#[utoipa::path(
    delete, operation_id = "unlinkDocument", path = "/documents/{id}/links/{kind}/{target_id}", tag = "documents",
    params(("id" = DocumentId, Path), ("kind" = LinkKind, Path), ("target_id" = String, Path, format = Uuid)),
    responses((status = 200, body = DocumentView), (status = 404, body = Problem))
)]
pub async fn unlink(
    State(state): State<AppState>,
    Path((id, kind, target)): Path<(DocumentId, LinkKind, uuid::Uuid)>,
) -> AppResult<Json<DocumentView>> {
    Ok(Json(documents::unlink(&state.db, id, LinkTarget { kind, id: target }, state.clock.today()).await?))
}

/// Documents rattachés à un objet, pour la confirmation de sa suppression (R-DOC-12).
#[utoipa::path(
    get, operation_id = "attachedDocuments", path = "/documents/attached/{target_id}", tag = "documents",
    params(("target_id" = String, Path, format = Uuid)),
    responses((status = 200, body = AttachedDocuments))
)]
pub async fn attached(
    State(state): State<AppState>,
    Path(target): Path<uuid::Uuid>,
) -> AppResult<Json<AttachedDocuments>> {
    Ok(Json(documents::attached(&state.db, target).await?))
}
