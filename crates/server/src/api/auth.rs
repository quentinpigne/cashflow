use std::net::SocketAddr;

use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::auth::{self, COOKIE_NAME, SESSION_DAYS};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct AuthStatus {
    pub authenticated: bool,
    /// Faux tant que `cashflow set-password` n'a pas été lancé sur le serveur.
    pub password_set: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginInput {
    pub password: String,
}

/// État de la session courante.
#[utoipa::path(get, path = "/auth/status", tag = "auth", responses((status = 200, body = AuthStatus)))]
pub async fn status(State(state): State<AppState>, jar: CookieJar) -> AppResult<Json<AuthStatus>> {
    let authenticated = match jar.get(COOKIE_NAME) {
        Some(cookie) => auth::authenticate(&state.db, cookie.value()).await.is_ok(),
        None => false,
    };
    Ok(Json(AuthStatus { authenticated, password_set: auth::password_is_set(&state.db).await? }))
}

/// Ouvre une session : pose le cookie `cashflow_session` (HttpOnly, SameSite=Strict, 30 jours glissants).
#[utoipa::path(
    post, path = "/auth/login", tag = "auth", request_body = LoginInput,
    responses(
        (status = 204, description = "Session ouverte"),
        (status = 401, body = Problem, description = "Mot de passe incorrect"),
        (status = 409, body = Problem, description = "Aucun mot de passe défini"),
        (status = 429, body = Problem, description = "Trop de tentatives"),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(input): Json<LoginInput>,
) -> AppResult<(StatusCode, CookieJar)> {
    let device =
        headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|v| v.chars().take(200).collect::<String>());
    let token = auth::login(&state.db, &state.limiter, peer.ip(), &input.password, device.as_deref()).await?;
    let cookie = Cookie::build((COOKIE_NAME, token))
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(state.secure_cookies)
        .path("/")
        .max_age(time_days(SESSION_DAYS))
        .build();
    Ok((StatusCode::NO_CONTENT, jar.add(cookie)))
}

fn time_days(days: i64) -> time::Duration {
    time::Duration::days(days)
}

fn removal() -> Cookie<'static> {
    Cookie::build(COOKIE_NAME).path("/").build()
}

/// Ferme la session courante.
#[utoipa::path(post, path = "/auth/logout", tag = "auth", responses((status = 204)))]
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<(StatusCode, CookieJar)> {
    if let Some(cookie) = jar.get(COOKIE_NAME) {
        auth::logout(&state.db, cookie.value()).await?;
    }
    Ok((StatusCode::NO_CONTENT, jar.remove(removal())))
}

/// Se déconnecter de tous les appareils (ADR-0015).
#[utoipa::path(post, path = "/auth/logout-everywhere", tag = "auth", responses((status = 204)))]
pub async fn logout_everywhere(State(state): State<AppState>, jar: CookieJar) -> AppResult<(StatusCode, CookieJar)> {
    auth::logout_everywhere(&state.db).await?;
    Ok((StatusCode::NO_CONTENT, jar.remove(removal())))
}
