use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::import::rules::Conditions;
use cashflow_domain::shared::ids::CategorizationRuleId;

use crate::error::{AppResult, Problem};
use crate::services::rules::{self, ApplyInput, ApplyResult, PreviewQuery, RuleInput, RuleMatch, RuleView};
use crate::state::AppState;

/// Règles de catégorisation, par priorité (IMP-08).
#[utoipa::path(get, operation_id = "listRules", path = "/rules", tag = "rules", responses((status = 200, body = Vec<RuleView>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<RuleView>>> {
    Ok(Json(rules::list(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createRule", path = "/rules", tag = "rules", request_body = RuleInput,
    responses((status = 201, body = RuleView), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<RuleInput>,
) -> AppResult<(StatusCode, Json<RuleView>)> {
    Ok((StatusCode::CREATED, Json(rules::save(&state.db, None, &input).await?)))
}

#[utoipa::path(
    put, operation_id = "updateRule", path = "/rules/{id}", tag = "rules", params(("id" = CategorizationRuleId, Path)),
    request_body = RuleInput, responses((status = 200, body = RuleView), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<CategorizationRuleId>,
    Json(input): Json<RuleInput>,
) -> AppResult<Json<RuleView>> {
    Ok(Json(rules::save(&state.db, Some(id), &input).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteRule", path = "/rules/{id}", tag = "rules", params(("id" = CategorizationRuleId, Path)),
    responses((status = 204))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<CategorizationRuleId>) -> AppResult<StatusCode> {
    rules::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Transactions non catégorisées (ou sans tiers, R-TIE-19) concernées par ces conditions (aperçu, R-IMP-19).
#[utoipa::path(
    post, operation_id = "previewRule", path = "/rules/preview", tag = "rules", params(PreviewQuery),
    request_body = Conditions, responses((status = 200, body = Vec<RuleMatch>), (status = 422, body = Problem))
)]
pub async fn preview(
    State(state): State<AppState>,
    Query(query): Query<PreviewQuery>,
    Json(conditions): Json<Conditions>,
) -> AppResult<Json<Vec<RuleMatch>>> {
    Ok(Json(rules::preview(&state.db, &conditions, &query).await?))
}

/// Applique une règle aux transactions choisies dans l'aperçu (R-IMP-19).
#[utoipa::path(
    post, operation_id = "applyRule", path = "/rules/{id}/apply", tag = "rules", params(("id" = CategorizationRuleId, Path)),
    request_body = ApplyInput, responses((status = 200, body = ApplyResult), (status = 422, body = Problem))
)]
pub async fn apply(
    State(state): State<AppState>,
    Path(id): Path<CategorizationRuleId>,
    Json(input): Json<ApplyInput>,
) -> AppResult<Json<ApplyResult>> {
    Ok(Json(rules::apply(&state.db, id, &input).await?))
}
