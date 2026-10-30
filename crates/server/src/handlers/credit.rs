use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use cashflow_domain::credit::ProjectKind;
use cashflow_domain::credit::pieces::Piece;
use cashflow_domain::shared::ids::{
    DocumentId, LenderEventId, LenderRequestId, LoanApplicationId, LoanId, LoanSimulationId,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::credit::simulate::{
    self as simulate, BorrowerSituation, CapacityRequest, FinancingRequest, FinancingView, ProjectDefaults,
    ProjectDefaultsInput, PurchaseCapacity, SimulateState, SituationView,
};
use crate::services::credit::{
    self, ApplicationInput, ApplicationSummary, ApplicationView, ComputeInput, ComputeResult, FollowUpInput,
    LenderEventInput, LenderRequestInput, SimulationInput, SimulationView,
};
use crate::services::loans::{InstallmentView, LoanInput};
use crate::state::AppState;

// ---------------------------------------------------------------- Simulations

/// Simulations enregistrées, les plus récentes d'abord (CRE-01).
#[utoipa::path(
    get, operation_id = "listLoanSimulations", path = "/loan-simulations", tag = "credit",
    responses((status = 200, body = Vec<SimulationView>))
)]
pub async fn list_simulations(State(state): State<AppState>) -> AppResult<Json<Vec<SimulationView>>> {
    Ok(Json(credit::simulations(&state.db, state.clock.today()).await?))
}

/// Calcule une simulation sans l'enregistrer, ou cherche son capital ou sa durée (CRE-01, CRE-02).
#[utoipa::path(
    post, operation_id = "computeLoanSimulation", path = "/loan-simulations/compute", tag = "credit",
    request_body = ComputeInput,
    responses((status = 200, body = ComputeResult), (status = 422, body = Problem))
)]
pub async fn compute(State(state): State<AppState>, Json(input): Json<ComputeInput>) -> AppResult<Json<ComputeResult>> {
    Ok(Json(credit::compute(&input, state.clock.today())?))
}

#[utoipa::path(
    post, operation_id = "createLoanSimulation", path = "/loan-simulations", tag = "credit",
    request_body = SimulationInput,
    responses((status = 200, body = SimulationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn create_simulation(
    State(state): State<AppState>,
    Json(input): Json<SimulationInput>,
) -> AppResult<Json<SimulationView>> {
    Ok(Json(credit::save_simulation(&state.db, None, &input, state.clock.today()).await?))
}

#[utoipa::path(
    get, operation_id = "getLoanSimulation", path = "/loan-simulations/{id}", tag = "credit",
    params(("id" = LoanSimulationId, Path)),
    responses((status = 200, body = SimulationView), (status = 404, body = Problem))
)]
pub async fn get_simulation(
    State(state): State<AppState>,
    Path(id): Path<LoanSimulationId>,
) -> AppResult<Json<SimulationView>> {
    Ok(Json(credit::simulation(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateLoanSimulation", path = "/loan-simulations/{id}", tag = "credit",
    params(("id" = LoanSimulationId, Path)), request_body = SimulationInput,
    responses((status = 200, body = SimulationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update_simulation(
    State(state): State<AppState>,
    Path(id): Path<LoanSimulationId>,
    Json(input): Json<SimulationInput>,
) -> AppResult<Json<SimulationView>> {
    Ok(Json(credit::save_simulation(&state.db, Some(id), &input, state.clock.today()).await?))
}

/// Supprime une simulation ; démarches et demande perdent le lien (R-CRE-23).
#[utoipa::path(
    delete, operation_id = "deleteLoanSimulation", path = "/loan-simulations/{id}", tag = "credit",
    params(("id" = LoanSimulationId, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_simulation(
    State(state): State<AppState>,
    Path(id): Path<LoanSimulationId>,
) -> AppResult<StatusCode> {
    credit::delete_simulation(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Tableau d'amortissement d'une simulation (R-CRE-2).
#[utoipa::path(
    get, operation_id = "loanSimulationSchedule", path = "/loan-simulations/{id}/schedule", tag = "credit",
    params(("id" = LoanSimulationId, Path)),
    responses((status = 200, body = Vec<InstallmentView>), (status = 404, body = Problem))
)]
pub async fn simulation_schedule(
    State(state): State<AppState>,
    Path(id): Path<LoanSimulationId>,
) -> AppResult<Json<Vec<InstallmentView>>> {
    Ok(Json(credit::simulation_schedule(&state.db, id, state.clock.today()).await?))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoanCreated {
    pub id: LoanId,
}

/// Transforme une simulation en prêt (CRE-03, R-CRE-7).
#[utoipa::path(
    post, operation_id = "convertLoanSimulation", path = "/loan-simulations/{id}/loan", tag = "credit",
    params(("id" = LoanSimulationId, Path)), request_body = LoanInput,
    responses((status = 200, body = LoanCreated), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn convert(
    State(state): State<AppState>,
    Path(id): Path<LoanSimulationId>,
    Json(input): Json<LoanInput>,
) -> AppResult<Json<LoanCreated>> {
    Ok(Json(LoanCreated { id: credit::convert(&state.db, id, &input, state.clock.today()).await? }))
}

// ---------------------------------------------------------------- Demandes

#[utoipa::path(
    get, operation_id = "listLoanApplications", path = "/loan-applications", tag = "credit",
    responses((status = 200, body = Vec<ApplicationSummary>))
)]
pub async fn list_applications(State(state): State<AppState>) -> AppResult<Json<Vec<ApplicationSummary>>> {
    Ok(Json(credit::applications(&state.db, state.clock.today()).await?))
}

#[utoipa::path(
    post, operation_id = "createLoanApplication", path = "/loan-applications", tag = "credit",
    request_body = ApplicationInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn create_application(
    State(state): State<AppState>,
    Json(input): Json<ApplicationInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::save_application(&state.db, None, &input, state.clock.today()).await?))
}

/// Fiche d'une demande : plan, capacité, simulations, pièces, démarches (CRE-04 à CRE-06).
#[utoipa::path(
    get, operation_id = "getLoanApplication", path = "/loan-applications/{id}", tag = "credit",
    params(("id" = LoanApplicationId, Path)),
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn get_application(
    State(state): State<AppState>,
    Path(id): Path<LoanApplicationId>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::application(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateLoanApplication", path = "/loan-applications/{id}", tag = "credit",
    params(("id" = LoanApplicationId, Path)), request_body = ApplicationInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update_application(
    State(state): State<AppState>,
    Path(id): Path<LoanApplicationId>,
    Json(input): Json<ApplicationInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::save_application(&state.db, Some(id), &input, state.clock.today()).await?))
}

/// Supprime une demande ; ses simulations restent (R-CRE-23).
#[utoipa::path(
    delete, operation_id = "deleteLoanApplication", path = "/loan-applications/{id}", tag = "credit",
    params(("id" = LoanApplicationId, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_application(
    State(state): State<AppState>,
    Path(id): Path<LoanApplicationId>,
) -> AppResult<StatusCode> {
    credit::delete_application(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignInput {
    pub document_id: DocumentId,
}

/// Attribue un document à une pièce (R-CRE-18).
#[utoipa::path(
    post, operation_id = "assignPiece", path = "/loan-applications/{id}/pieces/{piece}/documents", tag = "credit",
    params(("id" = LoanApplicationId, Path), ("piece" = Piece, Path)), request_body = AssignInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn assign(
    State(state): State<AppState>,
    Path((id, piece)): Path<(LoanApplicationId, Piece)>,
    Json(input): Json<AssignInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::assign(&state.db, id, piece, input.document_id, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "unassignPiece", path = "/loan-applications/{id}/pieces/{piece}/documents/{document_id}",
    tag = "credit",
    params(("id" = LoanApplicationId, Path), ("piece" = Piece, Path), ("document_id" = DocumentId, Path)),
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn unassign(
    State(state): State<AppState>,
    Path((id, piece, document)): Path<(LoanApplicationId, Piece, DocumentId)>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::unassign(&state.db, id, piece, document, state.clock.today()).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RequiredInput {
    pub not_required: bool,
}

/// Marque une pièce non requise, ou de nouveau requise (R-CRE-18).
#[utoipa::path(
    put, operation_id = "setPieceRequired", path = "/loan-applications/{id}/pieces/{piece}/required", tag = "credit",
    params(("id" = LoanApplicationId, Path), ("piece" = Piece, Path)), request_body = RequiredInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn set_required(
    State(state): State<AppState>,
    Path((id, piece)): Path<(LoanApplicationId, Piece)>,
    Json(input): Json<RequiredInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::set_required(&state.db, id, piece, input.not_required, state.clock.today()).await?))
}

#[utoipa::path(
    post, operation_id = "createLenderRequest", path = "/loan-applications/{id}/lender-requests", tag = "credit",
    params(("id" = LoanApplicationId, Path)), request_body = LenderRequestInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn create_lender_request(
    State(state): State<AppState>,
    Path(id): Path<LoanApplicationId>,
    Json(input): Json<LenderRequestInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::save_lender_request(&state.db, id, None, &input, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateLenderRequest", path = "/loan-applications/{id}/lender-requests/{request_id}",
    tag = "credit", params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path)),
    request_body = LenderRequestInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn update_lender_request(
    State(state): State<AppState>,
    Path((id, request)): Path<(LoanApplicationId, LenderRequestId)>,
    Json(input): Json<LenderRequestInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::save_lender_request(&state.db, id, Some(request), &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteLenderRequest", path = "/loan-applications/{id}/lender-requests/{request_id}",
    tag = "credit", params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path)),
    responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_lender_request(
    State(state): State<AppState>,
    Path((id, request)): Path<(LoanApplicationId, LenderRequestId)>,
) -> AppResult<StatusCode> {
    credit::delete_lender_request(&state.db, id, request).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Enregistre l'offre reçue d'une démarche, comme une simulation de la demande (R-CRE-21).
#[utoipa::path(
    post, operation_id = "recordOffer", path = "/loan-applications/{id}/lender-requests/{request_id}/offer",
    tag = "credit", params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path)),
    request_body = SimulationInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn record_offer(
    State(state): State<AppState>,
    Path((id, request)): Path<(LoanApplicationId, LenderRequestId)>,
    Json(input): Json<SimulationInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::record_offer(&state.db, id, request, &input, state.clock.today()).await?))
}

/// Accepte l'offre d'une démarche (R-CRE-22).
#[utoipa::path(
    post, operation_id = "acceptOffer", path = "/loan-applications/{id}/lender-requests/{request_id}/accept",
    tag = "credit", params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path)),
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn accept_offer(
    State(state): State<AppState>,
    Path((id, request)): Path<(LoanApplicationId, LenderRequestId)>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::accept_offer(&state.db, id, request, state.clock.today()).await?))
}

/// Archive ZIP du dossier : les pièces fournies et la synthèse (R-CRE-19).
#[utoipa::path(
    get, operation_id = "exportLoanApplication", path = "/loan-applications/{id}/export", tag = "credit",
    params(("id" = LoanApplicationId, Path)),
    responses((status = 200, description = "Archive ZIP du dossier", content_type = "application/zip"), (status = 404, body = Problem))
)]
pub async fn export(State(state): State<AppState>, Path(id): Path<LoanApplicationId>) -> AppResult<impl IntoResponse> {
    let (name, content) = credit::export(&state.db, &state.data, id, state.clock.today()).await?;
    let ascii: String =
        name.chars().map(|c| if c.is_ascii_alphanumeric() || " .-_".contains(c) { c } else { '_' }).collect();
    let encoded: String = name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")),
            (header::CACHE_CONTROL, "private, no-store".to_owned()),
        ],
        content,
    ))
}

/// Synthèse HTML de la demande (R-CRE-19).
#[utoipa::path(
    get, operation_id = "loanApplicationSummary", path = "/loan-applications/{id}/summary", tag = "credit",
    params(("id" = LoanApplicationId, Path)),
    responses((status = 200, description = "Synthèse HTML", content_type = "text/html"), (status = 404, body = Problem))
)]
pub async fn summary(State(state): State<AppState>, Path(id): Path<LoanApplicationId>) -> AppResult<impl IntoResponse> {
    let html = credit::summary(&state.db, id, state.clock.today()).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8".to_owned()),
            (header::CONTENT_SECURITY_POLICY, "default-src 'none'; style-src 'unsafe-inline'".to_owned()),
        ],
        html,
    ))
}

// ---------------------------------------------------------------- Onglet Simuler

/// Situation d'emprunteur et son résumé (R-CRE-30).
#[utoipa::path(
    get, operation_id = "borrowerSituation", path = "/credit/situation", tag = "credit",
    responses((status = 200, body = SituationView))
)]
pub async fn borrower_situation(State(state): State<AppState>) -> AppResult<Json<SituationView>> {
    Ok(Json(simulate::borrower_situation(&state.db, state.clock.today()).await?))
}

/// Garde la situation d'emprunteur (R-CRE-30).
#[utoipa::path(
    put, operation_id = "saveBorrowerSituation", path = "/credit/situation", tag = "credit",
    request_body = BorrowerSituation,
    responses((status = 200, body = SituationView), (status = 422, body = Problem))
)]
pub async fn save_borrower_situation(
    State(state): State<AppState>,
    Json(input): Json<BorrowerSituation>,
) -> AppResult<Json<SituationView>> {
    Ok(Json(simulate::save_borrower_situation(&state.db, &input, state.clock.today()).await?))
}

/// Conditions par défaut de chaque type de projet (R-CRE-38).
#[utoipa::path(
    get, operation_id = "projectDefaults", path = "/credit/project-defaults", tag = "credit",
    responses((status = 200, body = Vec<ProjectDefaults>))
)]
pub async fn project_defaults(State(state): State<AppState>) -> AppResult<Json<Vec<ProjectDefaults>>> {
    Ok(Json(simulate::project_defaults(&state.db).await?))
}

/// Garde les conditions par défaut (R-CRE-38).
#[utoipa::path(
    put, operation_id = "saveProjectDefaults", path = "/credit/project-defaults", tag = "credit",
    request_body = Vec<ProjectDefaultsInput>,
    responses((status = 200, body = Vec<ProjectDefaults>), (status = 422, body = Problem))
)]
pub async fn save_project_defaults(
    State(state): State<AppState>,
    Json(input): Json<Vec<ProjectDefaultsInput>>,
) -> AppResult<Json<Vec<ProjectDefaults>>> {
    Ok(Json(simulate::save_project_defaults(&state.db, &input).await?))
}

/// Dernier état de l'onglet Simuler (R-CRE-39).
#[utoipa::path(
    get, operation_id = "simulateState", path = "/credit/simulate-state", tag = "credit",
    responses((status = 200, body = SimulateState))
)]
pub async fn simulate_state(State(state): State<AppState>) -> AppResult<Json<SimulateState>> {
    Ok(Json(simulate::simulate_state(&state.db).await?))
}

#[utoipa::path(
    put, operation_id = "saveSimulateState", path = "/credit/simulate-state", tag = "credit",
    request_body = SimulateState,
    responses((status = 200, body = SimulateState), (status = 422, body = Problem))
)]
pub async fn save_simulate_state(
    State(state): State<AppState>,
    Json(input): Json<SimulateState>,
) -> AppResult<Json<SimulateState>> {
    Ok(Json(simulate::save_simulate_state(&state.db, &input).await?))
}

/// Que puis-je acheter ? (R-CRE-31 à R-CRE-35)
#[utoipa::path(
    post, operation_id = "purchaseCapacity", path = "/credit/purchase-capacity", tag = "credit",
    request_body = CapacityRequest,
    responses((status = 200, body = PurchaseCapacity), (status = 422, body = Problem))
)]
pub async fn purchase_capacity(
    State(state): State<AppState>,
    Json(input): Json<CapacityRequest>,
) -> AppResult<Json<PurchaseCapacity>> {
    Ok(Json(simulate::purchase_capacity(&state.db, &input, state.clock.today()).await?))
}

/// Puis-je financer cet achat ? (R-CRE-40)
#[utoipa::path(
    post, operation_id = "purchaseFinancing", path = "/credit/financing", tag = "credit",
    request_body = FinancingRequest,
    responses((status = 200, body = FinancingView), (status = 422, body = Problem))
)]
pub async fn financing(
    State(state): State<AppState>,
    Json(input): Json<FinancingRequest>,
) -> AppResult<Json<FinancingView>> {
    Ok(Json(simulate::financing(&state.db, &input, state.clock.today()).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FromCapacityInput {
    pub name: String,
    #[serde(flatten)]
    pub request: CapacityRequest,
}

/// Crée une demande depuis « Que puis-je acheter ? » (R-CRE-36).
#[utoipa::path(
    post, operation_id = "applicationFromCapacity", path = "/credit/purchase-capacity/application", tag = "credit",
    request_body = FromCapacityInput,
    responses((status = 200, body = ApplicationView), (status = 422, body = Problem))
)]
pub async fn application_from_capacity(
    State(state): State<AppState>,
    Json(input): Json<FromCapacityInput>,
) -> AppResult<Json<ApplicationView>> {
    let today = state.clock.today();
    Ok(Json(simulate::application_from_capacity(&state.db, &input.request, &input.name, today).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FromFinancingInput {
    pub name: String,
    #[serde(flatten)]
    pub request: FinancingRequest,
}

/// Crée une demande depuis « Puis-je financer cet achat ? » (R-CRE-36).
#[utoipa::path(
    post, operation_id = "applicationFromFinancing", path = "/credit/financing/application", tag = "credit",
    request_body = FromFinancingInput,
    responses((status = 200, body = ApplicationView), (status = 422, body = Problem))
)]
pub async fn application_from_financing(
    State(state): State<AppState>,
    Json(input): Json<FromFinancingInput>,
) -> AppResult<Json<ApplicationView>> {
    let today = state.clock.today();
    Ok(Json(simulate::application_from_financing(&state.db, &input.request, &input.name, today).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FromSimulationInput {
    pub name: String,
    pub project_kind: ProjectKind,
}

/// Prépare une demande depuis une simulation (R-CRE-36).
#[utoipa::path(
    post, operation_id = "applicationFromSimulation", path = "/loan-simulations/{id}/application", tag = "credit",
    params(("id" = LoanSimulationId, Path)), request_body = FromSimulationInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn application_from_simulation(
    State(state): State<AppState>,
    Path(id): Path<LoanSimulationId>,
    Json(input): Json<FromSimulationInput>,
) -> AppResult<Json<ApplicationView>> {
    let today = state.clock.today();
    Ok(Json(simulate::application_from_simulation(&state.db, id, input.project_kind, &input.name, today).await?))
}

// ---------------------------------------------------------------- Journal et relances

/// Relance une démarche (R-CRE-25).
#[utoipa::path(
    post, operation_id = "followUpLenderRequest",
    path = "/loan-applications/{id}/lender-requests/{request_id}/follow-up", tag = "credit",
    params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path)), request_body = FollowUpInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn follow_up(
    State(state): State<AppState>,
    Path((id, request)): Path<(LoanApplicationId, LenderRequestId)>,
    Json(input): Json<FollowUpInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::follow_up(&state.db, id, request, &input, state.clock.today()).await?))
}

/// Ajoute un échange au journal d'une démarche (R-CRE-24).
#[utoipa::path(
    post, operation_id = "createLenderEvent", path = "/loan-applications/{id}/lender-requests/{request_id}/events",
    tag = "credit", params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path)),
    request_body = LenderEventInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn create_event(
    State(state): State<AppState>,
    Path((id, request)): Path<(LoanApplicationId, LenderRequestId)>,
    Json(input): Json<LenderEventInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::save_event(&state.db, id, request, None, &input, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateLenderEvent",
    path = "/loan-applications/{id}/lender-requests/{request_id}/events/{event_id}", tag = "credit",
    params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path), ("event_id" = LenderEventId, Path)),
    request_body = LenderEventInput,
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn update_event(
    State(state): State<AppState>,
    Path((id, request, event)): Path<(LoanApplicationId, LenderRequestId, LenderEventId)>,
    Json(input): Json<LenderEventInput>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::save_event(&state.db, id, request, Some(event), &input, state.clock.today()).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteLenderEvent",
    path = "/loan-applications/{id}/lender-requests/{request_id}/events/{event_id}", tag = "credit",
    params(("id" = LoanApplicationId, Path), ("request_id" = LenderRequestId, Path), ("event_id" = LenderEventId, Path)),
    responses((status = 200, body = ApplicationView), (status = 404, body = Problem))
)]
pub async fn delete_event(
    State(state): State<AppState>,
    Path((id, request, event)): Path<(LoanApplicationId, LenderRequestId, LenderEventId)>,
) -> AppResult<Json<ApplicationView>> {
    Ok(Json(credit::delete_event(&state.db, id, request, event, state.clock.today()).await?))
}
