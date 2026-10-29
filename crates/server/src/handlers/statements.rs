use axum::Json;
use axum::extract::{Path, State};
use cashflow_domain::shared::ids::AccountId;

use crate::error::{AppResult, Problem};
use crate::services::statements::{self, AccountStatements};
use crate::state::AppState;

/// Relevés archivés d'un compte et mois sans relevé (CPT-11, R-CPT-21). Envoi, modification et suppression :
/// `/documents` (spec 12).
#[utoipa::path(
    get, operation_id = "listStatements", path = "/accounts/{id}/statements", tag = "accounts",
    params(("id" = AccountId, Path)),
    responses((status = 200, body = AccountStatements), (status = 404, body = Problem))
)]
pub async fn list(State(state): State<AppState>, Path(id): Path<AccountId>) -> AppResult<Json<AccountStatements>> {
    Ok(Json(statements::list(&state.db, id, state.clock.today()).await?))
}
