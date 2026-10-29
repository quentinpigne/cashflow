//! Erreurs de l'application, traduites en `application/problem+json` (RFC 9457, ADR-0016).

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use cashflow_domain::DomainError;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("{0} introuvable")]
    NotFound(&'static str),
    #[error("{message}")]
    Invalid { code: &'static str, message: String, field: Option<&'static str> },
    #[error("authentification requise")]
    Unauthorized,
    #[error("route interdite à un jeton d'accès")]
    Forbidden,
    #[error("mot de passe incorrect")]
    WrongPassword,
    #[error("trop de tentatives : réessayer dans {retry_after_secs} s")]
    TooManyAttempts { retry_after_secs: u64 },
    #[error("aucun mot de passe défini : lancer `cashflow set-password` sur le serveur")]
    NoPassword,
    /// Un service externe (la Base Adresse Nationale) ne répond pas ou répond en erreur.
    #[error("{0} ne répond pas")]
    Upstream(&'static str),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn invalid(code: &'static str, message: impl Into<String>) -> Self {
        AppError::Invalid { code, message: message.into(), field: None }
    }

    pub fn invalid_field(code: &'static str, field: &'static str, message: impl Into<String>) -> Self {
        AppError::Invalid { code, message: message.into(), field: Some(field) }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        AppError::Internal(error.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Corps d'une réponse d'erreur (`application/problem+json`).
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    /// Toujours `about:blank` : le champ `code` identifie l'erreur.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Message lisible, en français.
    pub title: String,
    pub status: u16,
    /// Code stable de l'erreur (ex. `SPLIT_SUM_MISMATCH`).
    pub code: &'static str,
    /// Règle métier concernée (ex. `R-TRX-10`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<&'static str>,
    /// Champ de la requête concerné, si l'erreur en vise un.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<&'static str>,
    /// Pour `TOO_MANY_ATTEMPTS` : délai d'attente en secondes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
    /// Pour `DOCUMENT_ALREADY_ARCHIVED` : le document qui a déjà ce fichier (R-DOC-3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, rule, field, retry_after) = match &self {
            AppError::Domain(error) => (StatusCode::UNPROCESSABLE_ENTITY, error.code(), error.rule(), None, None),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND", None, None, None),
            AppError::Invalid { code, field, .. } => (StatusCode::UNPROCESSABLE_ENTITY, *code, None, *field, None),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", None, None, None),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN", None, None, None),
            AppError::WrongPassword => (StatusCode::UNAUTHORIZED, "WRONG_PASSWORD", None, None, None),
            AppError::TooManyAttempts { retry_after_secs } => {
                (StatusCode::TOO_MANY_REQUESTS, "TOO_MANY_ATTEMPTS", None, None, Some(*retry_after_secs))
            }
            AppError::NoPassword => (StatusCode::CONFLICT, "NO_PASSWORD", None, None, None),
            AppError::Upstream(_) => (StatusCode::BAD_GATEWAY, "UPSTREAM_UNAVAILABLE", None, None, None),
            AppError::Internal(error) => {
                tracing::error!(error = format!("{error:#}"), "erreur interne");
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", None, None, None)
            }
        };
        let title = match &self {
            AppError::Internal(_) => "erreur interne du serveur".to_owned(),
            other => other.to_string(),
        };
        let document_id = match &self {
            AppError::Domain(DomainError::DocumentAlreadyArchived { id, .. }) => Some(id.to_string()),
            _ => None,
        };
        let problem = Problem {
            kind: "about:blank",
            title,
            status: status.as_u16(),
            code,
            rule,
            field,
            retry_after,
            document_id,
        };
        let mut response = (status, Json(problem)).into_response();
        response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/problem+json"));
        response
    }
}
