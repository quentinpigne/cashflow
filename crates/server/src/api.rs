//! API REST sous `/api/v1`, décrite par utoipa (ADR-0016).

mod accounts;
mod assets;
mod auth;
mod budgets;
mod categories;
mod dashboard;
mod imports;
mod institutions;
mod loans;
mod projections;
mod reconciliation;
mod reports;
mod rules;
mod savings;
mod system;
mod transactions;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::Method;
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
        (name = "imports", description = "Import de relevés (spec 03)"),
        (name = "reconciliation", description = "Rapprochement (spec 04)"),
        (name = "rules", description = "Règles de catégorisation (spec 03)"),
        (name = "budgets", description = "Budgets, projets et notifications (spec 05)"),
        (name = "assets", description = "Biens (spec 09)"),
        (name = "loans", description = "Prêts : suivi, tableau d'amortissement, mensualités (spec 09)"),
        (name = "savings", description = "Épargne : intérêts, valorisations, performance (spec 06)"),
        (name = "projections", description = "Récurrences, scénarios et projections (spec 02, spec 07)"),
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
        .routes(routes!(imports::preview))
        .routes(routes!(imports::list, imports::start))
        .routes(routes!(imports::get, imports::cancel))
        .routes(routes!(imports::review_line))
        .routes(routes!(imports::bulk))
        .routes(routes!(imports::commit))
        .routes(routes!(imports::revert))
        .routes(routes!(imports::download_file, imports::delete_file))
        .routes(routes!(imports::list_profiles, imports::create_profile))
        .routes(routes!(imports::update_profile, imports::delete_profile))
        .routes(routes!(reconciliation::list, reconciliation::create))
        .routes(routes!(reconciliation::delete))
        .routes(routes!(reconciliation::preview, reconciliation::reconcile))
        .routes(routes!(reconciliation::unlock))
        .routes(routes!(reconciliation::gap_hints))
        .routes(routes!(reconciliation::set_cleared))
        .routes(routes!(reconciliation::health))
        .routes(routes!(rules::list, rules::create))
        .routes(routes!(rules::update, rules::delete))
        .routes(routes!(rules::preview))
        .routes(routes!(rules::apply))
        .routes(routes!(dashboard::get))
        .routes(routes!(dashboard::recent))
        .routes(routes!(reports::categories))
        .routes(routes!(reports::evolution))
        .routes(routes!(transactions::list_all))
        .routes(routes!(transactions::categorize_many))
        .routes(routes!(budgets::month))
        .routes(routes!(budgets::list_lines, budgets::create_line))
        .routes(routes!(budgets::update_line))
        .routes(routes!(budgets::change_amount))
        .routes(routes!(budgets::reset_rollover))
        .routes(routes!(budgets::end_line))
        .routes(routes!(budgets::delete_override))
        .routes(routes!(budgets::suggestions))
        .routes(routes!(budgets::set_thresholds))
        .routes(routes!(budgets::list_projects, budgets::create_project))
        .routes(routes!(budgets::get_project, budgets::update_project, budgets::delete_project))
        .routes(routes!(budgets::attach))
        .routes(routes!(budgets::notifications))
        .routes(routes!(budgets::mark_read))
        .routes(routes!(savings::get_terms, savings::update_terms))
        .routes(routes!(loans::list, loans::create))
        .routes(routes!(loans::get, loans::update, loans::delete))
        .routes(routes!(loans::save_balance))
        .routes(routes!(loans::delete_balance))
        .routes(routes!(loans::preview))
        .routes(routes!(loans::proposals))
        .routes(routes!(loans::pay))
        .routes(routes!(loans::unpay))
        .routes(routes!(loans::transaction_proposal))
        .routes(routes!(assets::list, assets::create))
        .routes(routes!(assets::get, assets::update, assets::delete))
        .routes(routes!(assets::save_valuation))
        .routes(routes!(assets::delete_valuation))
        .routes(routes!(assets::get_features, assets::update_features))
        .routes(routes!(savings::schedules))
        .routes(routes!(savings::set_rate))
        .routes(routes!(savings::delete_rate))
        .routes(routes!(savings::interest))
        .routes(routes!(savings::record_interest))
        .routes(routes!(savings::valuations, savings::save_valuation))
        .routes(routes!(savings::delete_valuation))
        .routes(routes!(savings::performance))
        .routes(routes!(savings::overview))
        .routes(routes!(projections::occurrences))
        .routes(routes!(projections::list_rules, projections::create_rule))
        .routes(routes!(projections::update_rule, projections::delete_rule))
        .routes(routes!(projections::materialize))
        .routes(routes!(projections::list_scenarios, projections::create_scenario))
        .routes(routes!(projections::update_scenario, projections::delete_scenario))
        .routes(routes!(projections::cash))
        .routes(routes!(projections::account))
        .routes(routes!(projections::net_worth))
        .routes(routes!(projections::history))
        .routes(routes!(system::info))
        .routes(routes!(system::export))
}

/// Routes de l'API. Sans état (génération du contrat), le middleware de session n'est pas branché.
fn api_router(state: Option<AppState>) -> OpenApiRouter<AppState> {
    let protected = match state {
        Some(state) => protected_routes()
            .route_layer(middleware::from_fn_with_state(state.clone(), evaluate_alerts))
            .route_layer(middleware::from_fn_with_state(state, require_session)),
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

/// Après chaque écriture réussie, évalue les seuils d'alerte du budget (R-BUD-63). Une erreur ici ne fait
/// pas échouer l'écriture, déjà validée.
async fn evaluate_alerts(State(state): State<AppState>, request: Request, next: Next) -> Response {
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
