//! Catégories à deux niveaux (R-TRX-20 à R-TRX-22).

use serde::{Deserialize, Serialize};

use crate::shared::DomainError;

/// Nature d'une catégorie. Elle ne contraint pas le signe des ventilations (R-TRX-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CategoryKind {
    Income,
    Expense,
}

/// Ce qu'il faut savoir du parent envisagé pour une catégorie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentInfo {
    pub kind: CategoryKind,
    pub has_parent: bool,
}

/// Vérifie la place d'une catégorie dans la hiérarchie (R-TRX-20).
///
/// `has_children` : la catégorie a déjà des sous-catégories (elle ne peut donc pas devenir enfant).
pub fn check_hierarchy(kind: CategoryKind, parent: Option<ParentInfo>, has_children: bool) -> Result<(), DomainError> {
    let Some(parent) = parent else { return Ok(()) };
    if parent.has_parent || has_children {
        return Err(DomainError::CategoryDepthExceeded);
    }
    if parent.kind != kind {
        return Err(DomainError::CategoryKindMismatch);
    }
    Ok(())
}

/// Vérifie qu'une fusion de `source` dans `target` est possible (R-TRX-22).
pub fn check_merge(
    source_kind: CategoryKind,
    target_kind: CategoryKind,
    same: bool,
    target_is_child_of_source: bool,
) -> Result<(), DomainError> {
    if same || target_is_child_of_source || source_kind != target_kind {
        return Err(DomainError::InvalidCategoryMerge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_trx_20_two_levels_max_and_same_kind() {
        let expense_root = ParentInfo { kind: CategoryKind::Expense, has_parent: false };
        let expense_child = ParentInfo { kind: CategoryKind::Expense, has_parent: true };
        assert!(check_hierarchy(CategoryKind::Expense, None, true).is_ok());
        assert!(check_hierarchy(CategoryKind::Expense, Some(expense_root), false).is_ok());
        assert_eq!(
            check_hierarchy(CategoryKind::Expense, Some(expense_child), false),
            Err(DomainError::CategoryDepthExceeded)
        );
        assert_eq!(
            check_hierarchy(CategoryKind::Expense, Some(expense_root), true),
            Err(DomainError::CategoryDepthExceeded)
        );
        assert_eq!(
            check_hierarchy(CategoryKind::Income, Some(expense_root), false),
            Err(DomainError::CategoryKindMismatch)
        );
    }

    #[test]
    fn r_trx_22_merge_requires_same_kind_and_distinct_categories() {
        use CategoryKind::*;
        assert!(check_merge(Expense, Expense, false, false).is_ok());
        assert!(check_merge(Expense, Income, false, false).is_err());
        assert!(check_merge(Expense, Expense, true, false).is_err());
        assert!(check_merge(Expense, Expense, false, true).is_err());
    }
}
