use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::shared::ids::{EmploymentId, SalaryBonusId, SalaryId};

use super::DeleteQuery;
use crate::error::{AppResult, Problem};
use crate::services::salaries::{self, BonusInput, EmploymentInput, EmploymentView, SalariesOverview, SalaryInput};
use crate::state::AppState;

/// Emplois et évolution annuelle du salaire (E29, SAL-01, SAL-02).
#[utoipa::path(get, operation_id = "salariesOverview", path = "/salaries", tag = "salaries", responses((status = 200, body = SalariesOverview)))]
pub async fn overview(State(state): State<AppState>) -> AppResult<Json<SalariesOverview>> {
    Ok(Json(salaries::overview(&state.db, state.clock.today()).await?))
}

#[utoipa::path(
    post, operation_id = "createEmployment", path = "/employments", tag = "salaries", request_body = EmploymentInput,
    responses((status = 201, body = EmploymentView), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<EmploymentInput>,
) -> AppResult<(StatusCode, Json<EmploymentView>)> {
    Ok((StatusCode::CREATED, Json(salaries::create(&state.db, &input, state.clock.today()).await?)))
}

/// Fiche d'un emploi : rémunérations, primes, revenu annuel, comparaison au reçu (E30).
#[utoipa::path(
    get, operation_id = "getEmployment", path = "/employments/{id}", tag = "salaries",
    params(("id" = EmploymentId, Path)), responses((status = 200, body = EmploymentView), (status = 404, body = Problem))
)]
pub async fn get(State(state): State<AppState>, Path(id): Path<EmploymentId>) -> AppResult<Json<EmploymentView>> {
    Ok(Json(salaries::get(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateEmployment", path = "/employments/{id}", tag = "salaries",
    params(("id" = EmploymentId, Path)), request_body = EmploymentInput,
    responses((status = 200, body = EmploymentView), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<EmploymentId>,
    Json(input): Json<EmploymentInput>,
) -> AppResult<Json<EmploymentView>> {
    Ok(Json(salaries::update(&state.db, id, &input, state.clock.today()).await?))
}

/// Supprime un emploi, ses rémunérations et ses primes ; ses documents restent, sauf demande contraire (R-SAL-6,
/// R-DOC-12).
#[utoipa::path(
    delete, operation_id = "deleteEmployment", path = "/employments/{id}", tag = "salaries",
    params(("id" = EmploymentId, Path), DeleteQuery), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<EmploymentId>,
    Query(query): Query<DeleteQuery>,
) -> AppResult<StatusCode> {
    salaries::delete(&state.db, &state.data, id, query.delete_documents, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ajoute une rémunération (R-SAL-2).
#[utoipa::path(
    post, operation_id = "createSalary", path = "/employments/{id}/salaries", tag = "salaries",
    params(("id" = EmploymentId, Path)), request_body = SalaryInput,
    responses((status = 200, body = EmploymentView), (status = 422, body = Problem))
)]
pub async fn create_salary(
    State(state): State<AppState>,
    Path(id): Path<EmploymentId>,
    Json(input): Json<SalaryInput>,
) -> AppResult<Json<EmploymentView>> {
    Ok(Json(salaries::save_salary(&state.db, id, None, &input, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateSalary", path = "/employments/{id}/salaries/{salary_id}", tag = "salaries",
    params(("id" = EmploymentId, Path), ("salary_id" = SalaryId, Path)), request_body = SalaryInput,
    responses((status = 200, body = EmploymentView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update_salary(
    State(state): State<AppState>,
    Path((id, salary)): Path<(EmploymentId, SalaryId)>,
    Json(input): Json<SalaryInput>,
) -> AppResult<Json<EmploymentView>> {
    Ok(Json(salaries::save_salary(&state.db, id, Some(salary), &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteSalary", path = "/employments/{id}/salaries/{salary_id}", tag = "salaries",
    params(("id" = EmploymentId, Path), ("salary_id" = SalaryId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_salary(
    State(state): State<AppState>,
    Path((id, salary)): Path<(EmploymentId, SalaryId)>,
) -> AppResult<StatusCode> {
    salaries::delete_salary(&state.db, id, salary).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ajoute une prime (R-SAL-5).
#[utoipa::path(
    post, operation_id = "createBonus", path = "/employments/{id}/bonuses", tag = "salaries",
    params(("id" = EmploymentId, Path)), request_body = BonusInput,
    responses((status = 200, body = EmploymentView), (status = 422, body = Problem))
)]
pub async fn create_bonus(
    State(state): State<AppState>,
    Path(id): Path<EmploymentId>,
    Json(input): Json<BonusInput>,
) -> AppResult<Json<EmploymentView>> {
    Ok(Json(salaries::save_bonus(&state.db, id, None, &input, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateBonus", path = "/employments/{id}/bonuses/{bonus_id}", tag = "salaries",
    params(("id" = EmploymentId, Path), ("bonus_id" = SalaryBonusId, Path)), request_body = BonusInput,
    responses((status = 200, body = EmploymentView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update_bonus(
    State(state): State<AppState>,
    Path((id, bonus)): Path<(EmploymentId, SalaryBonusId)>,
    Json(input): Json<BonusInput>,
) -> AppResult<Json<EmploymentView>> {
    Ok(Json(salaries::save_bonus(&state.db, id, Some(bonus), &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteBonus", path = "/employments/{id}/bonuses/{bonus_id}", tag = "salaries",
    params(("id" = EmploymentId, Path), ("bonus_id" = SalaryBonusId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_bonus(
    State(state): State<AppState>,
    Path((id, bonus)): Path<(EmploymentId, SalaryBonusId)>,
) -> AppResult<StatusCode> {
    salaries::delete_bonus(&state.db, id, bonus).await?;
    Ok(StatusCode::NO_CONTENT)
}
