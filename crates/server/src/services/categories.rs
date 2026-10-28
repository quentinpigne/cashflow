//! Catégories (TRX-04, R-TRX-20 à R-TRX-23).

use cashflow_domain::DomainError;
use cashflow_domain::ledger::category::{self, CategoryKind, ParentInfo};
use cashflow_domain::shared::ids::CategoryId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{clean, required};
use crate::db::categories::{CategoryFields, CategoryRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<CategoryId>,
    pub archived: bool,
    /// Ventilations exclues des revenus, dépenses, budgets et analyses (R-TRX-24) ; vaut aussi pour les
    /// sous-catégories.
    pub excluded_from_analyses: bool,
    pub display_order: i64,
    pub color: Option<String>,
    pub icon: Option<String>,
    /// Nombre de ventilations qui l'utilisent.
    pub usage: i64,
    /// Sous-catégories (une seule profondeur, R-TRX-20).
    #[schema(no_recursion)]
    pub children: Vec<Category>,
}

fn node(row: CategoryRow, children: Vec<Category>) -> Category {
    Category {
        id: row.id,
        name: row.name,
        kind: row.kind,
        parent_id: row.parent_id,
        archived: row.archived,
        excluded_from_analyses: row.excluded_from_analyses,
        display_order: row.display_order,
        color: row.color,
        icon: row.icon,
        usage: row.usage,
        children,
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CategoryInput {
    pub name: String,
    pub kind: CategoryKind,
    pub parent_id: Option<CategoryId>,
    #[serde(default)]
    pub archived: bool,
    /// R-TRX-24.
    #[serde(default)]
    pub excluded_from_analyses: bool,
    pub display_order: Option<i64>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderInput {
    /// Catégories d'un même niveau, dans l'ordre voulu.
    pub ids: Vec<CategoryId>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MergeInput {
    pub target_id: CategoryId,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MergeResult {
    /// Nombre de ventilations réaffectées.
    pub moved_splits: u64,
}

/// L'arbre complet : catégories racines et leurs sous-catégories, dans l'ordre d'affichage.
pub async fn tree(db: &Db) -> AppResult<Vec<Category>> {
    let rows = db::categories::list(&db.read).await?;
    let (roots, children): (Vec<_>, Vec<_>) = rows.into_iter().partition(|r| r.parent_id.is_none());
    Ok(roots
        .into_iter()
        .map(|root| {
            let kids = children
                .iter()
                .filter(|c| c.parent_id == Some(root.id))
                .cloned()
                .map(|c| node(c, Vec::new()))
                .collect();
            node(root, kids)
        })
        .collect())
}

async fn parent_info(db: &Db, parent: Option<CategoryId>) -> AppResult<Option<ParentInfo>> {
    let Some(parent) = parent else { return Ok(None) };
    let row = db::categories::find(&db.read, parent).await?.ok_or(AppError::NotFound("catégorie parente"))?;
    Ok(Some(ParentInfo { kind: row.kind, has_parent: row.parent_id.is_some() }))
}

pub async fn create(db: &Db, input: &CategoryInput) -> AppResult<CategoryId> {
    let name = required(&input.name, "name", 60)?;
    category::check_hierarchy(input.kind, parent_info(db, input.parent_id).await?, false)?;
    if db::categories::name_taken(&db.read, &name, input.kind, input.parent_id, None).await? {
        return Err(DomainError::CategoryNameTaken.into());
    }
    let id = CategoryId::new();
    let color = clean(input.color.as_deref());
    let icon = clean(input.icon.as_deref());
    let fields = CategoryFields {
        name: &name,
        kind: input.kind,
        parent_id: input.parent_id,
        archived: input.archived,
        excluded_from_analyses: input.excluded_from_analyses,
        display_order: input.display_order.unwrap_or(i64::MAX / 2),
        color: color.as_deref(),
        icon: icon.as_deref(),
    };
    db::categories::insert(&db.write, id, &fields).await?;
    Ok(id)
}

/// Renomme, déplace, archive ou désarchive une catégorie. Sa nature ne change pas.
pub async fn update(db: &Db, id: CategoryId, input: &CategoryInput) -> AppResult<()> {
    let current = db::categories::find(&db.read, id).await?.ok_or(AppError::NotFound("catégorie"))?;
    if input.kind != current.kind {
        return Err(DomainError::CategoryKindMismatch.into());
    }
    if input.parent_id == Some(id) {
        return Err(DomainError::CategoryDepthExceeded.into());
    }
    let name = required(&input.name, "name", 60)?;
    let has_children = db::categories::has_children(&db.read, id).await?;
    category::check_hierarchy(current.kind, parent_info(db, input.parent_id).await?, has_children)?;
    if db::categories::name_taken(&db.read, &name, current.kind, input.parent_id, Some(id)).await? {
        return Err(DomainError::CategoryNameTaken.into());
    }
    let color = clean(input.color.as_deref());
    let icon = clean(input.icon.as_deref());
    let fields = CategoryFields {
        name: &name,
        kind: current.kind,
        parent_id: input.parent_id,
        archived: input.archived,
        excluded_from_analyses: input.excluded_from_analyses,
        display_order: input.display_order.unwrap_or(current.display_order),
        color: color.as_deref(),
        icon: icon.as_deref(),
    };
    db::categories::update(&db.write, id, &fields).await?;
    Ok(())
}

/// Fixe l'ordre d'affichage d'un niveau de l'arbre (glisser-déposer, `ecrans.md` E8).
pub async fn reorder(db: &Db, input: &ReorderInput) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    for (position, id) in input.ids.iter().enumerate() {
        let mut row = db::categories::find(&mut *tx, *id).await?.ok_or(AppError::NotFound("catégorie"))?;
        row.display_order = i64::try_from(position).map_err(anyhow::Error::from)?;
        let fields = CategoryFields {
            name: &row.name,
            kind: row.kind,
            parent_id: row.parent_id,
            archived: row.archived,
            excluded_from_analyses: row.excluded_from_analyses,
            display_order: row.display_order,
            color: row.color.as_deref(),
            icon: row.icon.as_deref(),
        };
        db::categories::update(&mut *tx, *id, &fields).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Fusionne une catégorie dans une autre : toutes ses ventilations sont réaffectées (R-TRX-22).
pub async fn merge(db: &Db, source: CategoryId, input: &MergeInput) -> AppResult<MergeResult> {
    let from = db::categories::find(&db.read, source).await?.ok_or(AppError::NotFound("catégorie"))?;
    let into = db::categories::find(&db.read, input.target_id).await?.ok_or(AppError::NotFound("catégorie cible"))?;
    category::check_merge(from.kind, into.kind, from.id == into.id, into.parent_id == Some(from.id))?;
    let mut tx = db.write.begin().await?;
    let moved_splits = db::categories::merge(&mut tx, source, input.target_id).await?;
    tx.commit().await?;
    Ok(MergeResult { moved_splits })
}

/// Supprime une catégorie jamais utilisée, avec ses sous-catégories inutilisées (R-TRX-22).
pub async fn delete(db: &Db, id: CategoryId) -> AppResult<()> {
    let rows = db::categories::list(&db.read).await?;
    let target = rows.iter().find(|r| r.id == id).ok_or(AppError::NotFound("catégorie"))?;
    let family: Vec<&CategoryRow> = rows.iter().filter(|r| r.id == id || r.parent_id == Some(id)).collect();
    if family.iter().any(|r| r.usage > 0) {
        return Err(DomainError::CategoryInUse.into());
    }
    let mut tx = db.write.begin().await?;
    for child in family.iter().filter(|r| r.id != target.id) {
        db::categories::delete(&mut *tx, child.id).await?;
    }
    db::categories::delete(&mut *tx, id).await.map_err(|_| AppError::Domain(DomainError::CategoryInUse))?;
    tx.commit().await?;
    Ok(())
}
