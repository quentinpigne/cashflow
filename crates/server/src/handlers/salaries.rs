use axum::Json;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cashflow_domain::Date;
use cashflow_domain::shared::ids::{EmploymentId, PayslipId, SalaryBonusId, SalaryId};
use utoipa::ToSchema;

use super::{FileQuery, document_response};
use crate::error::{AppError, AppResult, Problem};
use crate::services::salaries::{
    self, BonusInput, EmploymentInput, EmploymentView, PayslipInput, PayslipUpload, SalariesOverview, SalaryInput,
};
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

/// Supprime un emploi, ses rémunérations, ses primes et ses fiches de paie (R-SAL-6, R-SAL-17).
#[utoipa::path(
    delete, operation_id = "deleteEmployment", path = "/employments/{id}", tag = "salaries",
    params(("id" = EmploymentId, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<EmploymentId>) -> AppResult<StatusCode> {
    salaries::delete(&state.db, &state.data, id, state.clock.today()).await?;
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

/// Formulaire d'envoi d'une fiche de paie (`multipart/form-data`).
#[derive(ToSchema)]
#[allow(dead_code)]
pub struct PayslipForm {
    /// PDF, JPEG ou PNG, 10 Mo au plus (R-SAL-15).
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
    /// Premier jour du mois de paie.
    pay_month: Date,
    note: Option<String>,
}

async fn payslip_upload(mut multipart: Multipart) -> AppResult<PayslipUpload> {
    let bad = |message: &str| AppError::invalid("INVALID_UPLOAD", message.to_owned());
    let (mut file_name, mut content, mut pay_month, mut note) = (None, None, None, None);
    while let Some(field) = multipart.next_field().await.map_err(|e| bad(&e.to_string()))? {
        match field.name().unwrap_or_default() {
            "file" => {
                file_name = field.file_name().map(str::to_owned);
                content = Some(field.bytes().await.map_err(|e| bad(&e.to_string()))?.to_vec());
            }
            "pay_month" => {
                let text = field.text().await.map_err(|e| bad(&e.to_string()))?;
                pay_month = Some(text.parse::<Date>().map_err(|_| bad("mois de paie invalide"))?);
            }
            "note" => note = Some(field.text().await.map_err(|e| bad(&e.to_string()))?),
            _ => {}
        }
    }
    Ok(PayslipUpload {
        file_name: file_name.unwrap_or_default(),
        content: content.ok_or_else(|| bad("fichier manquant"))?,
        pay_month: pay_month.ok_or_else(|| bad("mois de paie manquant"))?,
        note,
    })
}

/// Archive une fiche de paie (R-SAL-15, R-SAL-16).
#[utoipa::path(
    post, operation_id = "createPayslip", path = "/employments/{id}/payslips", tag = "salaries",
    params(("id" = EmploymentId, Path)), request_body(content = PayslipForm, content_type = "multipart/form-data"),
    responses((status = 200, body = EmploymentView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn create_payslip(
    State(state): State<AppState>,
    Path(id): Path<EmploymentId>,
    multipart: Multipart,
) -> AppResult<Json<EmploymentView>> {
    let upload = payslip_upload(multipart).await?;
    Ok(Json(salaries::add_payslip(&state.db, &state.data, id, upload, state.clock.today()).await?))
}

/// Modifie le mois de paie et la note d'une fiche (R-SAL-17).
#[utoipa::path(
    put, operation_id = "updatePayslip", path = "/employments/{id}/payslips/{payslip_id}", tag = "salaries",
    params(("id" = EmploymentId, Path), ("payslip_id" = PayslipId, Path)), request_body = PayslipInput,
    responses((status = 200, body = EmploymentView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update_payslip(
    State(state): State<AppState>,
    Path((id, payslip)): Path<(EmploymentId, PayslipId)>,
    Json(input): Json<PayslipInput>,
) -> AppResult<Json<EmploymentView>> {
    Ok(Json(salaries::update_payslip(&state.db, id, payslip, &input, state.clock.today()).await?))
}

/// Supprime une fiche et son document (R-SAL-17).
#[utoipa::path(
    delete, operation_id = "deletePayslip", path = "/employments/{id}/payslips/{payslip_id}", tag = "salaries",
    params(("id" = EmploymentId, Path), ("payslip_id" = PayslipId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_payslip(
    State(state): State<AppState>,
    Path((id, payslip)): Path<(EmploymentId, PayslipId)>,
) -> AppResult<StatusCode> {
    salaries::delete_payslip(&state.db, &state.data, id, payslip, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fichier d'une fiche de paie, affiché ou téléchargé (R-SAL-18, ADR-0024).
#[utoipa::path(
    get, operation_id = "downloadPayslip", path = "/employments/{id}/payslips/{payslip_id}/file", tag = "salaries",
    params(("id" = EmploymentId, Path), ("payslip_id" = PayslipId, Path), FileQuery),
    responses((status = 200, description = "Fichier de la fiche (PDF, JPEG ou PNG)", content_type = "application/octet-stream"), (status = 404, body = Problem))
)]
pub async fn download_payslip(
    State(state): State<AppState>,
    Path((id, payslip)): Path<(EmploymentId, PayslipId)>,
    Query(query): Query<FileQuery>,
) -> AppResult<impl IntoResponse> {
    let (row, content) = salaries::payslip_file(&state.db, &state.data, id, payslip).await?;
    Ok(document_response(&row.file_name, row.media_type, content, query.download))
}
