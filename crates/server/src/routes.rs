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
use crate::middleware::{evaluate_alerts, require_authentication};
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

/// Routes de l'API. Sans état (génération du contrat), le middleware d'authentification n'est pas
/// branché.
fn api_router(state: Option<AppState>) -> OpenApiRouter<AppState> {
    let protected = match state {
        Some(state) => protected_routes()
            .route_layer(layer::from_fn_with_state(state.clone(), evaluate_alerts))
            .route_layer(layer::from_fn_with_state(state, require_authentication)),
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode, header};
    use tower::ServiceExt;

    use super::*;
    use crate::clock::Clock;
    use crate::config::DataDir;
    use crate::services::{access_tokens, auth};

    async fn call(app: &Router, method: Method, uri: &str, token: Option<&str>) -> StatusCode {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = request.body(Body::empty()).expect("requête");
        app.clone().oneshot(request).await.expect("réponse").status()
    }

    #[tokio::test]
    async fn access_token_reads_only_listed_routes_adr_0023() {
        let (db, dir) = crate::db::test_db().await;
        auth::set_password(&db, "Moi", "motdepasse").await.unwrap();
        let token = access_tokens::create(&db, "foodflow").await.unwrap();
        let state = AppState {
            db: db.clone(),
            clock: Clock::System,
            data: DataDir::new(dir.path()),
            limiter: Arc::default(),
            secure_cookies: false,
        };
        let app = router(state, dir.path().join("web"));
        let id = "01900000-0000-7000-8000-000000000000";

        assert_eq!(call(&app, Method::GET, "/api/v1/categories", None).await, StatusCode::UNAUTHORIZED);
        assert_eq!(call(&app, Method::GET, "/api/v1/categories", Some("cf_inconnu")).await, StatusCode::UNAUTHORIZED);
        assert_eq!(call(&app, Method::GET, "/api/v1/categories", Some(&token)).await, StatusCode::OK);
        assert_eq!(call(&app, Method::GET, "/api/v1/accounts", Some(&token)).await, StatusCode::OK);
        assert_eq!(call(&app, Method::GET, "/api/v1/payees", Some(&token)).await, StatusCode::OK);
        assert_eq!(
            call(&app, Method::GET, &format!("/api/v1/transactions/{id}"), Some(&token)).await,
            StatusCode::NOT_FOUND
        );
        // Hors de la liste, ou en écriture : interdit, avant même le handler.
        assert_eq!(
            call(&app, Method::GET, &format!("/api/v1/accounts/{id}/iban"), Some(&token)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(call(&app, Method::GET, "/api/v1/export", Some(&token)).await, StatusCode::FORBIDDEN);
        assert_eq!(
            call(&app, Method::POST, "/api/v1/auth/logout-everywhere", Some(&token)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(&app, Method::DELETE, &format!("/api/v1/transactions/{id}"), Some(&token)).await,
            StatusCode::FORBIDDEN
        );

        access_tokens::revoke(&db, "foodflow").await.unwrap();
        assert_eq!(call(&app, Method::GET, "/api/v1/categories", Some(&token)).await, StatusCode::UNAUTHORIZED);
    }
}
