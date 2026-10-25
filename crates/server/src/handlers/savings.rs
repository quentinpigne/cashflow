use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use cashflow_domain::Date;
use cashflow_domain::shared::ids::{AccountId, RateScheduleId, ValuationId};

use crate::error::{AppResult, Problem};
use crate::services::savings::{
    self, InterestRecorded, InterestView, PerformanceView, PeriodQuery, RatePeriodInput, SavingsOverview, ScheduleView,
    TermsInput, TermsView, ValuationInput, ValuationView, YearQuery,
};
use crate::state::AppState;

/// Conditions d'épargne d'un compte à taux déterminé (CPT-05).
#[utoipa::path(
    get, operation_id = "getSavingsTerms", path = "/accounts/{id}/terms", tag = "savings", params(("id" = AccountId, Path)),
    responses((status = 200, body = TermsView), (status = 422, body = Problem))
)]
pub async fn get_terms(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<Json<TermsView>> {
    Ok(Json(savings::terms(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateSavingsTerms", path = "/accounts/{id}/terms", tag = "savings",
    params(("id" = AccountId, Path)), request_body = TermsInput,
    responses((status = 200, body = TermsView), (status = 422, body = Problem))
)]
pub async fn update_terms(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Json(input): Json<TermsInput>,
) -> AppResult<Json<TermsView>> {
    Ok(Json(savings::update_terms(&state.db, id, &input, state.clock.today()).await?))
}

/// Barèmes de taux et leur historique (EPA-01).
#[utoipa::path(get, operation_id = "listRateSchedules", path = "/rate-schedules", tag = "savings", responses((status = 200, body = Vec<ScheduleView>)))]
pub async fn schedules(State(state): State<AppState>) -> AppResult<Json<Vec<ScheduleView>>> {
    Ok(Json(savings::schedules(&state.db).await?))
}

/// Ajoute un taux daté à un barème, ou corrige celui de la même date (R-EPA-11).
#[utoipa::path(
    post, operation_id = "setRate", path = "/rate-schedules/{id}/rates", tag = "savings",
    params(("id" = RateScheduleId, Path)), request_body = RatePeriodInput,
    responses((status = 200, body = Vec<ScheduleView>), (status = 422, body = Problem))
)]
pub async fn set_rate(
    State(state): State<AppState>,
    Path(id): Path<RateScheduleId>,
    Json(input): Json<RatePeriodInput>,
) -> AppResult<Json<Vec<ScheduleView>>> {
    Ok(Json(savings::set_rate(&state.db, id, &input).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteRate", path = "/rate-schedules/{id}/rates/{from}", tag = "savings",
    params(("id" = RateScheduleId, Path), ("from" = Date, Path)),
    responses((status = 200, body = Vec<ScheduleView>), (status = 404, body = Problem))
)]
pub async fn delete_rate(
    State(state): State<AppState>,
    Path((id, from)): Path<(RateScheduleId, Date)>,
) -> AppResult<Json<Vec<ScheduleView>>> {
    Ok(Json(savings::delete_rate(&state.db, id, from).await?))
}

/// Intérêts d'une année : quinzaines, courus, attendus, versés, plafond (EPA-02 à EPA-04).
#[utoipa::path(
    get, operation_id = "getInterest", path = "/accounts/{id}/interest", tag = "savings",
    params(("id" = AccountId, Path), YearQuery), responses((status = 200, body = InterestView), (status = 422, body = Problem))
)]
pub async fn interest(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Query(query): Query<YearQuery>,
) -> AppResult<Json<InterestView>> {
    Ok(Json(savings::interest(&state.db, id, &query, state.clock.today()).await?))
}

/// Inscrit les intérêts calculés d'une année finie (R-EPA-10).
#[utoipa::path(
    post, operation_id = "recordInterest", path = "/accounts/{id}/interest/{year}", tag = "savings",
    params(("id" = AccountId, Path), ("year" = i32, Path)),
    responses((status = 201, body = InterestRecorded), (status = 422, body = Problem))
)]
pub async fn record_interest(
    State(state): State<AppState>,
    Path((id, year)): Path<(AccountId, i32)>,
) -> AppResult<(StatusCode, Json<InterestRecorded>)> {
    Ok((StatusCode::CREATED, Json(savings::record_interest(&state.db, id, year, state.clock.today()).await?)))
}

/// Valorisations d'un compte à rendement variable ou d'un prêt (CPT-06).
#[utoipa::path(
    get, operation_id = "listValuations", path = "/accounts/{id}/valuations", tag = "savings",
    params(("id" = AccountId, Path)), responses((status = 200, body = Vec<ValuationView>), (status = 422, body = Problem))
)]
pub async fn valuations(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
) -> AppResult<Json<Vec<ValuationView>>> {
    Ok(Json(savings::valuations(&state.db, id).await?))
}

#[utoipa::path(
    post, operation_id = "saveValuation", path = "/accounts/{id}/valuations", tag = "savings",
    params(("id" = AccountId, Path)), request_body = ValuationInput,
    responses((status = 200, body = Vec<ValuationView>), (status = 422, body = Problem))
)]
pub async fn save_valuation(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Json(input): Json<ValuationInput>,
) -> AppResult<Json<Vec<ValuationView>>> {
    Ok(Json(savings::save_valuation(&state.db, id, &input).await?))
}

#[utoipa::path(
    delete, operation_id = "deleteValuation", path = "/accounts/{id}/valuations/{valuation_id}", tag = "savings",
    params(("id" = AccountId, Path), ("valuation_id" = ValuationId, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_valuation(
    State(state): State<AppState>,
    Path((id, valuation)): Path<(AccountId, ValuationId)>,
) -> AppResult<StatusCode> {
    savings::delete_valuation(&state.db, id, valuation).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Valeur, capital investi, plus-value, TRI et TWR (EPA-05, EPA-06).
#[utoipa::path(
    get, operation_id = "getPerformance", path = "/accounts/{id}/performance", tag = "savings",
    params(("id" = AccountId, Path), PeriodQuery), responses((status = 200, body = PerformanceView), (status = 422, body = Problem))
)]
pub async fn performance(
    State(state): State<AppState>,
    Path(id): Path<AccountId>,
    Query(query): Query<PeriodQuery>,
) -> AppResult<Json<PerformanceView>> {
    Ok(Json(savings::performance(&state.db, id, &query, state.clock.today()).await?))
}

/// Synthèse de l'épargne pour l'accueil (TDB-05).
#[utoipa::path(get, operation_id = "savingsOverview", path = "/savings/overview", tag = "savings", responses((status = 200, body = SavingsOverview)))]
pub async fn overview(State(state): State<AppState>) -> AppResult<Json<SavingsOverview>> {
    Ok(Json(savings::overview(&state.db, state.clock.today()).await?))
}
