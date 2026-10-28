//! Rubriques activables : Biens, Prêts et Salaires (R-BIE-7, R-PRE-12, R-SAL-14), et recherche d'adresse des
//! tiers (R-TIE-26).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

const ASSETS: &str = "features.assets";
const LOANS: &str = "features.loans";
const SALARIES: &str = "features.salaries";
pub const GEOCODING: &str = "geocoding.enabled";

#[derive(Debug, Serialize, ToSchema)]
pub struct Features {
    pub assets: bool,
    pub loans: bool,
    pub salaries: bool,
    /// Nombre de biens : la rubrique ne se désactive que vide.
    pub asset_count: i64,
    pub loan_count: i64,
    pub employment_count: i64,
    /// Tolérance de l'écart entre salaire attendu et reçu, en millionièmes (R-SAL-9).
    pub salary_gap_tolerance_ppm: i64,
    /// Recherche d'adresse des tiers par la Base Adresse Nationale, désactivée par défaut (R-TIE-26, ADR-0007).
    pub geocoding: bool,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct FeaturesInput {
    pub assets: Option<bool>,
    pub loans: Option<bool>,
    pub salaries: Option<bool>,
    pub salary_gap_tolerance_ppm: Option<i64>,
    pub geocoding: Option<bool>,
}

/// Une nouvelle installation n'a aucune rubrique activée.
pub async fn get(db: &Db) -> AppResult<Features> {
    Ok(Features {
        assets: db::settings::get::<bool>(&db.read, ASSETS).await?.unwrap_or(false),
        loans: db::settings::get::<bool>(&db.read, LOANS).await?.unwrap_or(false),
        asset_count: db::assets::count(&db.read).await?,
        loan_count: db::loans::count(&db.read).await?,
        salaries: db::settings::get::<bool>(&db.read, SALARIES).await?.unwrap_or(false),
        employment_count: db::salaries::count(&db.read).await?,
        salary_gap_tolerance_ppm: super::salaries::tolerance(db).await?.ppm(),
        geocoding: geocoding_enabled(db).await?,
    })
}

pub async fn geocoding_enabled(db: &Db) -> AppResult<bool> {
    Ok(db::settings::get::<bool>(&db.read, GEOCODING).await?.unwrap_or(false))
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
    if input.salaries == Some(false) && current.employment_count > 0 {
        return Err(AppError::invalid(
            "FEATURE_IN_USE",
            "la rubrique Salaires ne se désactive qu'une fois vide : supprimer d'abord les emplois (R-SAL-14)",
        ));
    }
    if input.salary_gap_tolerance_ppm.is_some_and(|t| !(0..=1_000_000).contains(&t)) {
        return Err(AppError::invalid_field("INVALID_TOLERANCE", "salary_gap_tolerance_ppm", "entre 0 et 100 %"));
    }
    if let Some(on) = input.salaries {
        db::settings::set(&db.write, SALARIES, &on).await?;
    }
    if let Some(ppm) = input.salary_gap_tolerance_ppm {
        db::settings::set(&db.write, super::salaries::TOLERANCE, &ppm).await?;
    }
    if let Some(on) = input.assets {
        db::settings::set(&db.write, ASSETS, &on).await?;
    }
    if let Some(on) = input.loans {
        db::settings::set(&db.write, LOANS, &on).await?;
    }
    if let Some(on) = input.geocoding {
        db::settings::set(&db.write, GEOCODING, &on).await?;
    }
    get(db).await
}
