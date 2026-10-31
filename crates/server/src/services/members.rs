//! Membres du foyer (MBR-01, R-MBR-1 à R-MBR-3).

use std::collections::HashSet;

use cashflow_domain::DomainError;
use cashflow_domain::household::{self, MemberSelection, NAME_MAX};
use cashflow_domain::shared::ids::MemberId;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::{clean, required};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct MemberView {
    pub id: MemberId,
    pub name: String,
    /// `#RRGGBB`.
    pub color: Option<String>,
    pub display_order: i64,
    /// Ce qui lui est affecté, et revient au foyer à sa suppression (R-MBR-2).
    pub account_count: i64,
    pub split_count: i64,
    pub employment_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MemberInput {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MemberReorderInput {
    /// Tous les membres, dans l'ordre voulu.
    pub ids: Vec<MemberId>,
}

/// Les membres, dans leur ordre (R-MBR-1).
pub async fn list(db: &Db) -> AppResult<Vec<MemberView>> {
    Ok(db::members::list(&db.read)
        .await?
        .into_iter()
        .map(|r| MemberView {
            id: r.id,
            name: r.name,
            color: r.color,
            display_order: r.display_order,
            account_count: r.account_count,
            split_count: r.split_count,
            employment_count: r.employment_count,
        })
        .collect())
}

/// Nom et couleur contrôlés (R-MBR-1).
async fn checked(db: &Db, input: &MemberInput, except: Option<MemberId>) -> AppResult<(String, Option<String>)> {
    let name = required(&input.name, "name", NAME_MAX)?;
    let color = clean(input.color.as_deref());
    if let Some(color) = &color {
        household::check_color(color)?;
    }
    if db::members::name_taken(&db.write, &name, except).await? {
        return Err(DomainError::MemberNameTaken.into());
    }
    Ok((name, color))
}

pub async fn create(db: &Db, input: &MemberInput) -> AppResult<MemberId> {
    let (name, color) = checked(db, input, None).await?;
    let id = MemberId::new();
    let order = db::members::next_display_order(&db.write).await?;
    db::members::insert(&db.write, id, &name, color.as_deref(), order).await?;
    Ok(id)
}

pub async fn update(db: &Db, id: MemberId, input: &MemberInput) -> AppResult<()> {
    let (name, color) = checked(db, input, Some(id)).await?;
    if !db::members::update(&db.write, id, &name, color.as_deref()).await? {
        return Err(AppError::NotFound("membre"));
    }
    Ok(())
}

pub async fn reorder(db: &Db, input: &MemberReorderInput) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    for (position, id) in input.ids.iter().enumerate() {
        let order = i64::try_from(position).map_err(anyhow::Error::from)?;
        if !db::members::set_display_order(&mut *tx, *id, order).await? {
            return Err(AppError::NotFound("membre"));
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Supprime un membre : ses comptes, ventilations et emplois reviennent au foyer (R-MBR-2).
pub async fn delete(db: &Db, id: MemberId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    if !db::members::delete(&mut tx, id).await? {
        return Err(AppError::NotFound("membre"));
    }
    tx.commit().await?;
    Ok(())
}

/// Le membre cité existe.
pub(crate) async fn check_exists(conn: &mut SqliteConnection, member: Option<MemberId>) -> AppResult<()> {
    if let Some(member) = member
        && !db::members::exists(&mut *conn, member).await?
    {
        return Err(AppError::NotFound("membre"));
    }
    Ok(())
}

/// Case « Foyer » du paramètre `members` (R-MBR-8).
pub const HOUSEHOLD: &str = "household";

/// Choix des membres lu dans le paramètre `members` : `household` et des identifiants de membres, séparés par des
/// virgules (R-MBR-8). `None` quand il est absent ou que tout est coché : les chiffres sont alors ceux du foyer
/// (R-MBR-10). Un membre inconnu (supprimé entre-temps) est ignoré.
pub async fn selection(db: &Db, raw: Option<&str>) -> AppResult<Option<MemberSelection>> {
    let Some(raw) = raw.map(str::trim).filter(|r| !r.is_empty()) else { return Ok(None) };
    let mut selection = MemberSelection { household: false, members: HashSet::new() };
    for token in raw.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if token == HOUSEHOLD {
            selection.household = true;
        } else {
            let id = token.parse::<MemberId>().map_err(|_| {
                AppError::invalid_field("INVALID_MEMBERS", "members", "household ou identifiants de membres attendus")
            })?;
            selection.members.insert(id);
        }
    }
    let existing: Vec<MemberId> = db::members::list(&db.read).await?.into_iter().map(|m| m.id).collect();
    Ok((!selection.is_all(&existing)).then_some(selection))
}

/// Ce qui est affecté à `member` (ou au foyer pour `None`) compte avec ce choix ; sans choix, tout compte (R-MBR-9).
pub fn includes(selection: Option<&MemberSelection>, member: Option<MemberId>) -> bool {
    selection.is_none_or(|s| s.includes(member))
}
