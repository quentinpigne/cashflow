//! Rubriques activables : Biens et Prêts (R-BIE-7, R-PRE-12).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

const ASSETS: &str = "features.assets";
const LOANS: &str = "features.loans";

#[derive(Debug, Serialize, ToSchema)]
pub struct Features {
    pub assets: bool,
    pub loans: bool,
    /// Nombre de biens : la rubrique ne se désactive que vide.
    pub asset_count: i64,
    pub loan_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FeaturesInput {
    pub assets: Option<bool>,
    pub loans: Option<bool>,
}

/// Une nouvelle installation n'a aucune rubrique activée.
pub async fn get(db: &Db) -> AppResult<Features> {
    Ok(Features {
        assets: db::settings::get::<bool>(&db.read, ASSETS).await?.unwrap_or(false),
        loans: db::settings::get::<bool>(&db.read, LOANS).await?.unwrap_or(false),
        asset_count: db::assets::count(&db.read).await?,
        loan_count: db::loans::count(&db.read).await?,
    })
}

pub async fn update(db: &Db, input: &FeaturesInput) -> AppResult<Features> {
    let current = get(db).await?;
    if input.assets == Some(false) && current.asset_count > 0 {
        return Err(AppError::invalid(
            "FEATURE_IN_USE",
            "la rubrique Biens ne se désactive qu'une fois vide : supprimer d'abord les biens (R-BIE-7)",
        ));
    }
    if input.loans == Some(false) && current.loan_count > 0 {
        return Err(AppError::invalid(
            "FEATURE_IN_USE",
            "la rubrique Prêts ne se désactive qu'une fois vide : supprimer d'abord les prêts (R-PRE-12)",
        ));
    }
    if let Some(on) = input.assets {
        db::settings::set(&db.write, ASSETS, &on).await?;
    }
    if let Some(on) = input.loans {
        db::settings::set(&db.write, LOANS, &on).await?;
    }
    get(db).await
}
