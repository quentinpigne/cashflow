//! Middlewares HTTP.

use axum::extract::{MatchedPath, Request, State};
use axum::http::{HeaderMap, Method, header};
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::CookieJar;

use crate::error::AppError;
use crate::services;
use crate::state::AppState;

/// Exige une session valide (ADR-0015) ou un jeton d'accès autorisé sur la route (ADR-0023). Quand l'en-tête
/// `Authorization` est présent, le cookie est ignoré.
pub async fn require_authentication(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    if let Some(token) = bearer(request.headers())? {
        services::access_tokens::authenticate(&state.db, &token).await?;
        let route = request.extensions().get::<MatchedPath>().map(MatchedPath::as_str).unwrap_or_default();
        if !services::access_tokens::allows(request.method(), route) {
            return Err(AppError::Forbidden);
        }
        return Ok(next.run(request).await);
    }
    let token = jar.get(services::auth::COOKIE_NAME).map(|c| c.value().to_owned()).ok_or(AppError::Unauthorized)?;
    services::auth::authenticate(&state.db, &token).await?;
    Ok(next.run(request).await)
}

/// Jeton de l'en-tête `Authorization: Bearer <jeton>` ; tout autre schéma est refusé.
fn bearer(headers: &HeaderMap) -> Result<Option<String>, AppError> {
    let Some(value) = headers.get(header::AUTHORIZATION) else { return Ok(None) };
    let token = value.to_str().ok().and_then(|v| v.strip_prefix("Bearer ")).map(str::trim);
    match token {
        Some(token) if !token.is_empty() => Ok(Some(token.to_owned())),
        _ => Err(AppError::Unauthorized),
    }
}

/// Après chaque écriture réussie, évalue les seuils d'alerte du budget (R-BUD-63). Une erreur ici ne fait
/// pas échouer l'écriture, déjà validée.
pub async fn evaluate_alerts(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let writes = request.method() != Method::GET && request.method() != Method::HEAD;
    let notifications = request.uri().path().ends_with("/notifications/read");
    let response = next.run(request).await;
    if writes
        && !notifications
        && response.status().is_success()
        && let Err(error) = services::budgets::evaluate_alerts(&state.db, state.clock.today()).await
    {
        tracing::warn!(%error, "évaluation des alertes de budget impossible");
    }
    // Les relances de crédit dues (R-CRE-26).
    if writes
        && !notifications
        && response.status().is_success()
        && let Err(error) = services::credit::notify_follow_ups(&state.db, state.clock.today()).await
    {
        tracing::warn!(%error, "annonce des relances de crédit impossible");
    }
    response
}
