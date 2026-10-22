use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::{AccountId, RecurringRuleId, ScenarioId};

use crate::error::{AppResult, Problem};
use crate::services::projections::{
    self, AccountProjectionView, CashProjectionView, CashQuery, HistoryQuery, NetWorthPoint, NetWorthProjection,
    NetWorthQuery, ProjectionQuery, SalaryProjection, SalaryProjectionQuery, ScenarioInput, ScenarioView,
};
use crate::services::recurring::{
    self, MaterializeInput, Materialized, OccurrenceView, RecurringRuleInput, RecurringRuleView, ScheduleQuery,
};
use crate::state::AppState;

// ---------------------------------------------------------------- Récurrences (TRX-07)

#[utoipa::path(get, operation_id = "listRecurringRules", path = "/recurring-rules", tag = "projections", responses((status = 200, body = Vec<RecurringRuleView>)))]
pub async fn list_rules(State(state): State<AppState>) -> AppResult<Json<Vec<RecurringRuleView>>> {
    Ok(Json(recurring::list(&state.db, state.clock.today()).await?))
}

#[utoipa::path(
    post, operation_id = "createRecurringRule", path = "/recurring-rules", tag = "projections", request_body = RecurringRuleInput,
    responses((status = 201, body = RecurringRuleView), (status = 422, body = Problem))
)]
pub async fn create_rule(
    State(state): State<AppState>,
    Json(input): Json<RecurringRuleInput>,
) -> AppResult<(StatusCode, Json<RecurringRuleView>)> {
    Ok((StatusCode::CREATED, Json(recurring::save(&state.db, None, &input, state.clock.today()).await?)))
}

#[utoipa::path(
    put, operation_id = "updateRecurringRule", path = "/recurring-rules/{id}", tag = "projections",
    params(("id" = RecurringRuleId, Path)), request_body = RecurringRuleInput,
    responses((status = 200, body = RecurringRuleView), (status = 422, body = Problem))
)]
pub async fn update_rule(
    State(state): State<AppState>,
    Path(id): Path<RecurringRuleId>,
    Json(input): Json<RecurringRuleInput>,
) -> AppResult<Json<RecurringRuleView>> {
    Ok(Json(recurring::save(&state.db, Some(id), &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteRecurringRule", path = "/recurring-rules/{id}", tag = "projections",
    params(("id" = RecurringRuleId, Path)), responses((status = 204))
)]
pub async fn delete_rule(State(state): State<AppState>, Path(id): Path<RecurringRuleId>) -> AppResult<StatusCode> {
    recurring::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Échéancier : échéances récentes et à venir, honorées ou non (R-TRX-40, R-TRX-42).
#[utoipa::path(
    get, operation_id = "listOccurrences", path = "/recurring-rules/occurrences", tag = "projections", params(ScheduleQuery),
    responses((status = 200, body = Vec<OccurrenceView>))
)]
pub async fn occurrences(
    State(state): State<AppState>,
    Query(query): Query<ScheduleQuery>,
) -> AppResult<Json<Vec<OccurrenceView>>> {
    Ok(Json(recurring::upcoming(&state.db, &query, state.clock.today()).await?))
}

/// Matérialise une échéance en transaction en attente (R-TRX-43).
#[utoipa::path(
    post, operation_id = "materializeOccurrence", path = "/recurring-rules/{id}/materialize", tag = "projections",
    params(("id" = RecurringRuleId, Path)), request_body = MaterializeInput,
    responses((status = 201, body = Materialized), (status = 422, body = Problem))
)]
pub async fn materialize(
    State(state): State<AppState>,
    Path(id): Path<RecurringRuleId>,
    Json(input): Json<MaterializeInput>,
) -> AppResult<(StatusCode, Json<Materialized>)> {
    Ok((StatusCode::CREATED, Json(recurring::materialize(&state.db, id, &input).await?)))
}

// ---------------------------------------------------------------- Scénarios (PRJ-03)

#[utoipa::path(get, operation_id = "listScenarios", path = "/scenarios", tag = "projections", responses((status = 200, body = Vec<ScenarioView>)))]
pub async fn list_scenarios(State(state): State<AppState>) -> AppResult<Json<Vec<ScenarioView>>> {
    Ok(Json(projections::scenarios(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createScenario", path = "/scenarios", tag = "projections", request_body = ScenarioInput,
    responses((status = 201, body = ScenarioView), (status = 422, body = Problem))
)]
pub async fn create_scenario(
    State(state): State<AppState>,
    Json(input): Json<ScenarioInput>,
) -> AppResult<(StatusCode, Json<ScenarioView>)> {
    Ok((StatusCode::CREATED, Json(projections::save_scenario(&state.db, None, &input).await?)))
}

#[utoipa::path(
    put, operation_id = "updateScenario", path = "/scenarios/{id}", tag = "projections", params(("id" = ScenarioId, Path)),
    request_body = ScenarioInput, responses((status = 200, body = ScenarioView), (status = 422, body = Problem))
)]
pub async fn update_scenario(
    State(state): State<AppState>,
    Path(id): Path<ScenarioId>,
    Json(input): Json<ScenarioInput>,
) -> AppResult<Json<ScenarioView>> {
    Ok(Json(projections::save_scenario(&state.db, Some(id), &input).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteScenario", path = "/scenarios/{id}", tag = "projections", params(("id" = ScenarioId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete_scenario(State(state): State<AppState>, Path(id): Path<ScenarioId>) -> AppResult<StatusCode> {
    projections::delete_scenario(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- Projections

/// Trésorerie d'un compte de liquidités jour par jour (PRJ-01).
#[utoipa::path(
    get, operation_id = "cashProjection", path = "/accounts/{id}/projection/cash", tag = "projections",
    params(("id" = AccountId, Path), CashQuery), responses((status = 200, body = CashProjectionView), (status = 422, body = Problem))
)]
pub async fn cash(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Query(query): Query<CashQuery>,
) -> AppResult<Json<CashProjectionView>> {
    Ok(Json(projections::cash(&state.db, id, &query, state.clock.today()).await?))
}

/// Projection mensuelle d'un compte sous un scénario (PRJ-02, PRJ-04).
#[utoipa::path(
    get, operation_id = "accountProjection", path = "/accounts/{id}/projection", tag = "projections",
    params(("id" = AccountId, Path), ProjectionQuery), responses((status = 200, body = AccountProjectionView), (status = 404, body = Problem))
)]
pub async fn account(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Query(query): Query<ProjectionQuery>,
) -> AppResult<Json<AccountProjectionView>> {
    Ok(Json(projections::account(&state.db, id, &query, state.clock.today()).await?))
}

/// Patrimoine projeté sous chaque scénario (PRJ-05).
#[utoipa::path(
    get, operation_id = "netWorthProjection", path = "/projections/net-worth", tag = "projections", params(NetWorthQuery),
    responses((status = 200, body = NetWorthProjection))
)]
pub async fn net_worth(
    State(state): State<AppState>,
    Query(query): Query<NetWorthQuery>,
) -> AppResult<Json<NetWorthProjection>> {
    Ok(Json(projections::net_worth(&state.db, &query, state.clock.today()).await?))
}

/// Net annuel réel puis projeté par scénario (SAL-04).
#[utoipa::path(
    get, operation_id = "salaryProjection", path = "/projections/salary", tag = "projections",
    params(SalaryProjectionQuery), responses((status = 200, body = SalaryProjection))
)]
pub async fn salary(
    State(state): State<AppState>,
    Query(query): Query<SalaryProjectionQuery>,
) -> AppResult<Json<SalaryProjection>> {
    Ok(Json(projections::salary(&state.db, &query, state.clock.today()).await?))
}

/// Historique du patrimoine par famille de comptes (TDB-04).
#[utoipa::path(
    get, operation_id = "netWorthHistory", path = "/reports/net-worth", tag = "dashboard", params(HistoryQuery),
    responses((status = 200, body = Vec<NetWorthPoint>))
)]
pub async fn history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> AppResult<Json<Vec<NetWorthPoint>>> {
    Ok(Json(projections::history(&state.db, &query, state.clock.today()).await?))
}
