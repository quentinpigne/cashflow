//! Établissements (CPT-02).

use cashflow_domain::shared::ids::InstitutionId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{clean, required};
use crate::db::institutions::{InstitutionFields, InstitutionRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct Institution {
    pub id: InstitutionId,
    pub name: String,
    pub bank_code: Option<String>,
    pub bic: Option<String>,
    pub archived: bool,
}

impl From<InstitutionRow> for Institution {
    fn from(r: InstitutionRow) -> Self {
        Institution { id: r.id, name: r.name, bank_code: r.bank_code, bic: r.bic, archived: r.archived }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InstitutionInput {
    pub name: String,
    pub bank_code: Option<String>,
    pub bic: Option<String>,
    #[serde(default)]
    pub archived: bool,
}

pub async fn list(db: &Db) -> AppResult<Vec<Institution>> {
    Ok(db::institutions::list(&db.read).await?.into_iter().map(Into::into).collect())
}

struct Validated {
    name: String,
    bank_code: Option<String>,
    bic: Option<String>,
}

fn validate(input: &InstitutionInput) -> AppResult<Validated> {
    let name = required(&input.name, "name", 80)?;
    let bank_code = clean(input.bank_code.as_deref());
    if bank_code.as_ref().is_some_and(|c| c.len() != 5 || !c.chars().all(|ch| ch.is_ascii_digit())) {
        return Err(AppError::invalid_field("INVALID_BANK_CODE", "bank_code", "le code banque compte 5 chiffres"));
    }
    let bic = clean(input.bic.as_deref()).map(|b| b.to_uppercase());
    if bic.as_ref().is_some_and(|b| !matches!(b.len(), 8 | 11) || !b.chars().all(|ch| ch.is_ascii_alphanumeric())) {
        return Err(AppError::invalid_field("INVALID_BIC", "bic", "le BIC compte 8 ou 11 caractères"));
    }
    Ok(Validated { name, bank_code, bic })
}

pub async fn create(db: &Db, input: &InstitutionInput) -> AppResult<Institution> {
    let v = validate(input)?;
    if db::institutions::name_taken(&db.read, &v.name, None).await? {
        return Err(AppError::invalid_field("NAME_TAKEN", "name", "un établissement porte déjà ce nom"));
    }
    let id = InstitutionId::new();
    let fields = InstitutionFields {
        name: &v.name,
        bank_code: v.bank_code.as_deref(),
        bic: v.bic.as_deref(),
        archived: input.archived,
    };
    db::institutions::insert(&db.write, id, &fields).await?;
    get(db, id).await
}

pub async fn update(db: &Db, id: InstitutionId, input: &InstitutionInput) -> AppResult<Institution> {
    let v = validate(input)?;
    if db::institutions::name_taken(&db.read, &v.name, Some(id)).await? {
        return Err(AppError::invalid_field("NAME_TAKEN", "name", "un établissement porte déjà ce nom"));
    }
    let fields = InstitutionFields {
        name: &v.name,
        bank_code: v.bank_code.as_deref(),
        bic: v.bic.as_deref(),
        archived: input.archived,
    };
    if !db::institutions::update(&db.write, id, &fields).await? {
        return Err(AppError::NotFound("établissement"));
    }
    get(db, id).await
}

pub async fn get(db: &Db, id: InstitutionId) -> AppResult<Institution> {
    db::institutions::find(&db.write, id).await?.map(Into::into).ok_or(AppError::NotFound("établissement"))
}
