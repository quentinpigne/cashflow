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
    use crate::geocoding::Geocoder;
    use crate::services::{access_tokens, auth};

    /// Géocodeur qui ne joint rien : aucun test n'appelle la BAN.
    fn test_geocoder() -> Geocoder {
        Geocoder::new("http://127.0.0.1:9").unwrap()
    }

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
            geocoder: test_geocoder(),
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

    /// Ouvre une session et renvoie son cookie.
    async fn login(app: &Router) -> String {
        let request = Request::post("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 4000))))
            .body(Body::from(r#"{"password":"motdepasse"}"#))
            .expect("requête");
        let response = app.clone().oneshot(request).await.expect("réponse");
        let cookie = response.headers().get(header::SET_COOKIE).expect("cookie").to_str().expect("texte");
        cookie.split(';').next().expect("cookie").to_owned()
    }

    fn multipart(month: &str, content: &[u8]) -> (String, Vec<u8>) {
        let boundary = "frontiere";
        let mut body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"pay_month\"\r\n\r\n{month}\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"paie août.pdf\"\r\n\
             Content-Type: application/pdf\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(content);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={boundary}"), body)
    }

    #[tokio::test]
    async fn payslip_is_uploaded_then_served_sandboxed_sal_09_adr_0024() {
        let (db, dir) = crate::db::test_db().await;
        crate::services::seed::run(&db).await.unwrap();
        auth::set_password(&db, "Moi", "motdepasse").await.unwrap();
        let data = DataDir::new(dir.path());
        data.ensure().unwrap();
        let category = crate::db::categories::list(&db.read).await.unwrap().into_iter().find(|c| c.name == "Salaires");
        let input = crate::services::salaries::EmploymentInput {
            employer: "Acme".into(),
            job_title: None,
            start_date: "2025-01-01".parse().unwrap(),
            end_date: None,
            income_category_id: category.expect("catégorie Salaires").id,
            raise_month: None,
            paid_next_month: false,
            notes: None,
        };
        let today = "2026-10-22".parse().unwrap();
        let employment = crate::services::salaries::create(&db, &input, today).await.unwrap().summary.id;
        let state = AppState {
            db,
            clock: Clock::Fixed(today),
            data,
            limiter: Arc::default(),
            secure_cookies: false,
            geocoder: test_geocoder(),
        };
        let app = router(state, dir.path().join("web"));
        let cookie = login(&app).await;
        let uri = format!("/api/v1/employments/{employment}/payslips");

        let send = |content: Vec<u8>| {
            let (content_type, body) = multipart("2026-08-01", &content);
            Request::post(&uri)
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(body))
                .expect("requête")
        };
        // Au-delà de 2 Mo (limite par défaut d'axum), jusqu'à 10 Mo.
        let mut pdf = b"%PDF-1.7\n".to_vec();
        pdf.resize(3 * 1024 * 1024, b' ');
        let response = app.clone().oneshot(send(pdf.clone())).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let view: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let payslip = view["payslips"][0]["id"].as_str().expect("fiche").to_owned();
        assert_eq!(view["payslips"][0]["file_name"], "paie août.pdf");

        let too_big = vec![b'%'; 11 * 1024 * 1024];
        let response = app.clone().oneshot(send(too_big)).await.unwrap();
        assert!(response.status().is_client_error());

        let request = Request::get(format!("{uri}/{payslip}/file?download=true"))
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
        assert_eq!(headers[header::CONTENT_SECURITY_POLICY], "sandbox");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(
            headers[header::CONTENT_DISPOSITION],
            "attachment; filename=\"paie aot.pdf\"; filename*=UTF-8''paie%20ao%C3%BBt.pdf"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.len(), pdf.len());
    }
}
