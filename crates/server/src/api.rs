//! API REST sous `/api/v1`, décrite par utoipa (ADR-0016).

mod accounts;
mod auth;
mod categories;
mod dashboard;
mod institutions;
mod system;
mod transactions;

use axum::Router;
use axum::extract::{Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum_extra::extract::CookieJar;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::{AppError, Problem};
use crate::services;
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(title = "Cashflow", description = "API de gestion de finances personnelles (ADR-0016)."),
    components(schemas(Problem)),
    tags(
        (name = "auth", description = "Session (ADR-0015)"),
        (name = "accounts", description = "Comptes (spec 01)"),
        (name = "institutions", description = "Établissements (spec 01)"),
        (name = "categories", description = "Catégories (spec 02)"),
        (name = "transactions", description = "Transactions, ventilations et virements (spec 02)"),
        (name = "dashboard", description = "Tableau de bord (spec 08)"),
        (name = "system", description = "Export, sauvegardes, version"),
    )
)]
struct ApiDoc;

fn open_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(auth::status)).routes(routes!(auth::login)).routes(routes!(system::health))
}

fn protected_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(auth::logout))
        .routes(routes!(auth::logout_everywhere))
        .routes(routes!(accounts::list, accounts::create))
        .routes(routes!(accounts::get, accounts::update, accounts::delete))
        .routes(routes!(accounts::reveal_iban))
        .routes(routes!(accounts::close))
        .routes(routes!(accounts::reopen))
        .routes(routes!(institutions::list, institutions::create))
        .routes(routes!(institutions::update))
        .routes(routes!(categories::tree, categories::create))
        .routes(routes!(categories::update, categories::delete))
        .routes(routes!(categories::reorder))
        .routes(routes!(categories::merge))
        .routes(routes!(transactions::list))
        .routes(routes!(transactions::create))
        .routes(routes!(transactions::get, transactions::update, transactions::delete))
        .routes(routes!(transactions::categorize_split))
        .routes(routes!(transactions::create_transfer))
        .routes(routes!(transactions::transfer_candidates))
        .routes(routes!(transactions::mark_as_transfer))
        .routes(routes!(transactions::unlink_transfer))
        .routes(routes!(dashboard::get))
        .routes(routes!(system::info))
        .routes(routes!(system::export))
}

/// Routes de l'API. Sans état (génération du contrat), le middleware de session n'est pas branché.
fn api_router(state: Option<AppState>) -> OpenApiRouter<AppState> {
    let protected = match state {
        Some(state) => protected_routes().route_layer(middleware::from_fn_with_state(state, require_session)),
        None => protected_routes(),
    };
    OpenApiRouter::with_openapi(ApiDoc::openapi()).nest("/api/v1", open_routes().merge(protected))
}

pub fn router(state: AppState) -> Router {
    let (router, _) = api_router(Some(state.clone())).split_for_parts();
    router.with_state(state)
}

/// Contrat OpenAPI au format JSON, tel qu'il est versionné dans `openapi.json`.
pub fn openapi_json() -> serde_json::Result<String> {
    let (_, api) = api_router(None).split_for_parts();
    api.to_pretty_json()
}

/// Exige une session valide (ADR-0015).
async fn require_session(
    State(state): State<AppState>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = jar.get(services::auth::COOKIE_NAME).map(|c| c.value().to_owned()).ok_or(AppError::Unauthorized)?;
    services::auth::authenticate(&state.db, &token).await?;
    Ok(next.run(request).await)
}
