//! Middlewares HTTP.

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::CookieJar;

use crate::error::AppError;
use crate::services;
use crate::state::AppState;

/// Exige une session valide (ADR-0015).
pub async fn require_session(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = jar.get(services::auth::COOKIE_NAME).map(|c| c.value().to_owned()).ok_or(AppError::Unauthorized)?;
    services::auth::authenticate(&state.db, &token).await?;
    Ok(next.run(request).await)
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
    response
}
