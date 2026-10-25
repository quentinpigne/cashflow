//! Routes : assemblage de l'API REST sous `/api/v1`, décrite par utoipa (ADR-0016), et des fichiers de
//! l'interface. Chaque sous-module branche les handlers d'un domaine.

mod accounts;
mod assets;
mod auth;
mod budgeting;
mod import;
mod ledger;
mod payees;
mod projection;
mod reconciliation;
mod reporting;
mod salaries;
mod savings;
mod static_files;
mod system;

use std::path::PathBuf;

use axum::Router;
use axum::middleware as layer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::error::Problem;
use crate::middleware::{evaluate_alerts, require_session};
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
        (name = "payees", description = "Tiers et groupes de tiers (spec 11)"),
        (name = "transactions", description = "Transactions, ventilations et virements (spec 02)"),
        (name = "imports", description = "Import de relevés (spec 03)"),
        (name = "reconciliation", description = "Rapprochement (spec 04)"),
        (name = "rules", description = "Règles de catégorisation (spec 03)"),
        (name = "budgets", description = "Budgets, projets et notifications (spec 05)"),
        (name = "assets", description = "Biens (spec 09)"),
        (name = "loans", description = "Prêts : suivi, tableau d'amortissement, mensualités (spec 09)"),
        (name = "salaries", description = "Salaires : emplois, rémunérations, primes (spec 10)"),
        (name = "savings", description = "Épargne : intérêts, valorisations, performance (spec 06)"),
        (name = "projections", description = "Récurrences, scénarios et projections (spec 02, spec 07)"),
        (name = "dashboard", description = "Tableau de bord (spec 08)"),
        (name = "system", description = "Export, sauvegardes, version"),
    )
)]
struct ApiDoc;

fn protected_routes() -> OpenApiRouter<AppState> {
    auth::protected()
        .merge(accounts::router())
        .merge(ledger::router())
        .merge(payees::router())
        .merge(import::router())
        .merge(reconciliation::router())
        .merge(budgeting::router())
        .merge(savings::router())
        .merge(projection::router())
        .merge(reporting::router())
        .merge(assets::router())
        .merge(salaries::router())
        .merge(system::protected())
}

/// Routes de l'API. Sans état (génération du contrat), le middleware de session n'est pas branché.
fn api_router(state: Option<AppState>) -> OpenApiRouter<AppState> {
    let protected = match state {
        Some(state) => protected_routes()
            .route_layer(layer::from_fn_with_state(state.clone(), evaluate_alerts))
            .route_layer(layer::from_fn_with_state(state, require_session)),
        None => protected_routes(),
    };
    let open = auth::open().merge(system::open());
    OpenApiRouter::with_openapi(ApiDoc::openapi()).nest("/api/v1", open.merge(protected))
}

/// Application complète : API et interface compilée.
pub fn router(state: AppState, web_dir: PathBuf) -> Router {
    let (api, _) = api_router(Some(state.clone())).split_for_parts();
    api.with_state(state).merge(static_files::router(web_dir))
}

/// Contrat OpenAPI au format JSON, tel qu'il est versionné dans `openapi.json`.
pub fn openapi_json() -> serde_json::Result<String> {
    let (_, api) = api_router(None).split_for_parts();
    api.to_pretty_json()
}
