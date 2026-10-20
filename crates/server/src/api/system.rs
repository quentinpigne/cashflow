use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::AppResult;
use crate::services::backup::{self, LastBackup};
use crate::services::export;
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct Health {
    pub status: &'static str,
}

#[derive(Serialize, ToSchema)]
pub struct SystemInfo {
    pub version: &'static str,
    pub schema_version: i64,
    pub last_backup: Option<LastBackup>,
}

/// Le serveur répond (sans authentification).
#[utoipa::path(get, path = "/health", tag = "system", responses((status = 200, body = Health)))]
pub async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

/// Version et dernière sauvegarde (`ecrans.md` E9, « À propos »).
#[utoipa::path(get, path = "/system/info", tag = "system", responses((status = 200, body = SystemInfo)))]
pub async fn info(State(state): State<AppState>) -> AppResult<Json<SystemInfo>> {
    Ok(Json(SystemInfo {
        version: env!("CARGO_PKG_VERSION"),
        schema_version: export::schema_version(),
        last_backup: backup::last(&state.db).await?,
    }))
}

/// Export complet, au format JSON (ADR-0008).
#[utoipa::path(
    get, path = "/export", tag = "system",
    responses((status = 200, description = "Fichier JSON (format décrit dans docs/export.md)", content_type = "application/json"))
)]
pub async fn export(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let data = export::export(&state.db).await?;
    let name = format!("cashflow-export-{}.json", state.clock.today());
    let body = serde_json::to_vec_pretty(&data).map_err(anyhow::Error::from)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
        ],
        body,
    ))
}
