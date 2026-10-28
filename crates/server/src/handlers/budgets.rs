use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::{BudgetLineId, ProjectId};

use crate::error::{AppResult, Problem};
use crate::services::budgets::{
    self, AmountChange, AttachInput, AttachResult, BudgetLineInput, BudgetLineVersion, CategoriesChange, LineSettings,
    MonthBudgetView, MonthInput, MonthQuery, Notifications, ProjectInput, ProjectView, ReadInput, Suggestions,
    Thresholds,
};
use crate::state::AppState;

/// Le budget d'un mois : prévu, réel, reste, hors budget, reste à budgéter et solde (BUD-03).
#[utoipa::path(
    get, operation_id = "monthBudget", path = "/budget", tag = "budgets", params(MonthQuery),
    responses((status = 200, body = MonthBudgetView), (status = 422, body = Problem))
)]
pub async fn month(State(state): State<AppState>, Query(query): Query<MonthQuery>) -> AppResult<Json<MonthBudgetView>> {
    Ok(Json(budgets::month(&state.db, &query, state.clock.today()).await?))
}

/// Toutes les versions de lignes et leurs surcharges (R-BUD-50).
#[utoipa::path(get, operation_id = "listBudgetLines", path = "/budget/lines", tag = "budgets", responses((status = 200, body = Vec<BudgetLineVersion>)))]
pub async fn list_lines(State(state): State<AppState>) -> AppResult<Json<Vec<BudgetLineVersion>>> {
    Ok(Json(budgets::versions(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createBudgetLine", path = "/budget/lines", tag = "budgets", request_body = BudgetLineInput,
    responses((status = 201, body = BudgetLineVersion), (status = 422, body = Problem))
)]
pub async fn create_line(
    State(state): State<AppState>,
    Json(input): Json<BudgetLineInput>,
) -> AppResult<(StatusCode, Json<BudgetLineVersion>)> {
    Ok((StatusCode::CREATED, Json(budgets::create_line(&state.db, &input).await?)))
}

/// Report et seuils d'alerte d'une version.
#[utoipa::path(
    put, operation_id = "updateBudgetLine", path = "/budget/lines/{id}", tag = "budgets", params(("id" = BudgetLineId, Path)),
    request_body = LineSettings, responses((status = 200, body = BudgetLineVersion), (status = 422, body = Problem))
)]
pub async fn update_line(
    State(state): State<AppState>,
    Path(id): Path<BudgetLineId>,
    Json(input): Json<LineSettings>,
) -> AppResult<Json<BudgetLineVersion>> {
    Ok(Json(budgets::update_settings(&state.db, id, &input).await?))
}

/// Nouveau montant à partir d'un mois, ou pour ce mois seulement (R-BUD-51).
#[utoipa::path(
    post, operation_id = "changeBudgetAmount", path = "/budget/lines/{id}/amount", tag = "budgets",
    params(("id" = BudgetLineId, Path)), request_body = AmountChange,
    responses((status = 200, body = BudgetLineVersion), (status = 422, body = Problem))
)]
pub async fn change_amount(
    State(state): State<AppState>,
    Path(id): Path<BudgetLineId>,
    Json(input): Json<AmountChange>,
) -> AppResult<Json<BudgetLineVersion>> {
    Ok(Json(budgets::change_amount(&state.db, id, &input).await?))
}

/// Nouveau nom et nouvelles catégories à partir d'un mois : nouvelle version (R-BUD-10, R-BUD-51).
#[utoipa::path(
    post, operation_id = "changeBudgetCategories", path = "/budget/lines/{id}/categories", tag = "budgets",
    params(("id" = BudgetLineId, Path)), request_body = CategoriesChange,
    responses((status = 200, body = BudgetLineVersion), (status = 422, body = Problem))
)]
pub async fn change_categories(
    State(state): State<AppState>,
    Path(id): Path<BudgetLineId>,
    Json(input): Json<CategoriesChange>,
) -> AppResult<Json<BudgetLineVersion>> {
    Ok(Json(budgets::change_categories(&state.db, id, &input).await?))
}

/// Remet le report à zéro à partir d'un mois (R-BUD-18).
#[utoipa::path(
    post, operation_id = "resetBudgetRollover", path = "/budget/lines/{id}/reset-rollover", tag = "budgets",
    params(("id" = BudgetLineId, Path)), request_body = MonthInput,
    responses((status = 200, body = BudgetLineVersion), (status = 422, body = Problem))
)]
pub async fn reset_rollover(
    State(state): State<AppState>,
    Path(id): Path<BudgetLineId>,
    Json(input): Json<MonthInput>,
) -> AppResult<Json<BudgetLineVersion>> {
    Ok(Json(budgets::reset_rollover(&state.db, id, &input).await?))
}

/// Supprime une ligne à partir d'un mois : elle est close au mois précédent (R-BUD-53).
#[utoipa::path(
    post, operation_id = "endBudgetLine", path = "/budget/lines/{id}/end", tag = "budgets",
    params(("id" = BudgetLineId, Path)), request_body = MonthInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn end_line(
    State(state): State<AppState>,
    Path(id): Path<BudgetLineId>,
    Json(input): Json<MonthInput>,
) -> AppResult<StatusCode> {
    budgets::end_line(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime la surcharge d'une période (`AAAA-MM`).
#[utoipa::path(
    delete, operation_id = "deleteBudgetOverride", path = "/budget/lines/{id}/overrides/{period}", tag = "budgets",
    params(("id" = BudgetLineId, Path), ("period" = String, Path)), responses((status = 204))
)]
pub async fn delete_override(
    State(state): State<AppState>,
    Path((id, period)): Path<(BudgetLineId, String)>,
) -> AppResult<StatusCode> {
    budgets::delete_override(&state.db, id, &period).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Montants proposés d'après les derniers mois complets (BUD-10).
#[utoipa::path(get, operation_id = "budgetSuggestions", path = "/budget/suggestions", tag = "budgets", responses((status = 200, body = Suggestions)))]
pub async fn suggestions(State(state): State<AppState>) -> AppResult<Json<Suggestions>> {
    Ok(Json(budgets::suggestions(&state.db, state.clock.today()).await?))
}

/// Seuils d'alerte globaux (R-BUD-60).
#[utoipa::path(
    put, operation_id = "setBudgetThresholds", path = "/budget/thresholds", tag = "budgets", request_body = Thresholds,
    responses((status = 200, body = Thresholds), (status = 422, body = Problem))
)]
pub async fn set_thresholds(
    State(state): State<AppState>,
    Json(input): Json<Thresholds>,
) -> AppResult<Json<Thresholds>> {
    Ok(Json(budgets::set_thresholds(&state.db, &input).await?))
}

// ---------------------------------------------------------------- Projets

#[utoipa::path(get, operation_id = "listProjects", path = "/projects", tag = "budgets", responses((status = 200, body = Vec<ProjectView>)))]
pub async fn list_projects(State(state): State<AppState>) -> AppResult<Json<Vec<ProjectView>>> {
    Ok(Json(budgets::projects(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createProject", path = "/projects", tag = "budgets", request_body = ProjectInput,
    responses((status = 201, body = ProjectView), (status = 422, body = Problem))
)]
pub async fn create_project(
    State(state): State<AppState>,
    Json(input): Json<ProjectInput>,
) -> AppResult<(StatusCode, Json<ProjectView>)> {
    Ok((StatusCode::CREATED, Json(budgets::save_project(&state.db, None, &input).await?)))
}

#[utoipa::path(
    get, operation_id = "getProject", path = "/projects/{id}", tag = "budgets", params(("id" = ProjectId, Path)),
    responses((status = 200, body = ProjectView), (status = 404, body = Problem))
)]
pub async fn get_project(State(state): State<AppState>, Path(id): Path<ProjectId>) -> AppResult<Json<ProjectView>> {
    Ok(Json(budgets::project(&state.db, id).await?))
}

#[utoipa::path(
    put, operation_id = "updateProject", path = "/projects/{id}", tag = "budgets", params(("id" = ProjectId, Path)),
    request_body = ProjectInput, responses((status = 200, body = ProjectView), (status = 422, body = Problem))
)]
pub async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<ProjectId>,
    Json(input): Json<ProjectInput>,
) -> AppResult<Json<ProjectView>> {
    Ok(Json(budgets::save_project(&state.db, Some(id), &input).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteProject", path = "/projects/{id}", tag = "budgets", params(("id" = ProjectId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete_project(State(state): State<AppState>, Path(id): Path<ProjectId>) -> AppResult<StatusCode> {
    budgets::delete_project(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Rattache (ou détache) les ventilations de transactions à un projet, en masse (BUD-06).
#[utoipa::path(
    post, operation_id = "attachToProject", path = "/transactions/project", tag = "budgets", request_body = AttachInput,
    responses((status = 200, body = AttachResult), (status = 422, body = Problem))
)]
pub async fn attach(State(state): State<AppState>, Json(input): Json<AttachInput>) -> AppResult<Json<AttachResult>> {
    Ok(Json(budgets::attach(&state.db, &input).await?))
}

// ---------------------------------------------------------------- Notifications

/// Centre de notifications (BUD-08, R-BUD-62).
#[utoipa::path(get, operation_id = "listNotifications", path = "/notifications", tag = "budgets", responses((status = 200, body = Notifications)))]
pub async fn notifications(State(state): State<AppState>) -> AppResult<Json<Notifications>> {
    Ok(Json(budgets::notifications(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "markNotificationsRead", path = "/notifications/read", tag = "budgets", request_body = ReadInput,
    responses((status = 200, body = Notifications))
)]
pub async fn mark_read(State(state): State<AppState>, Json(input): Json<ReadInput>) -> AppResult<Json<Notifications>> {
    Ok(Json(budgets::mark_read(&state.db, &input).await?))
}
