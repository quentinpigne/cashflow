//! Foyer (spec 14) : membres, affectations et choix des membres affichés.

use std::collections::HashSet;

use crate::shared::DomainError;
use crate::shared::ids::MemberId;

/// Longueur maximale du nom d'un membre (R-MBR-1).
pub const NAME_MAX: usize = 40;

/// Une couleur de membre s'écrit `#RRGGBB` (R-MBR-1).
pub fn check_color(color: &str) -> Result<(), DomainError> {
    let valid = color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if valid { Ok(()) } else { Err(DomainError::InvalidMemberColor) }
}

/// Une ventilation de virement n'a jamais de membre (R-MBR-5).
pub fn check_split_member(has_member: bool, is_transfer_split: bool) -> Result<(), DomainError> {
    if has_member && is_transfer_split { Err(DomainError::MemberOnTransfer) } else { Ok(()) }
}

/// Membre commun à toutes les ventilations, conservé quand elles sont recalculées (R-MBR-5) : `None` s'il n'y en a
/// pas, si l'une est sans membre ou si deux membres diffèrent.
pub fn common_member(members: &[Option<MemberId>]) -> Option<MemberId> {
    let first = (*members.first()?)?;
    members.iter().all(|m| *m == Some(first)).then_some(first)
}

/// Membre retenu d'une ventilation : le sien, à défaut celui de son compte ; `None` : le foyer (R-MBR-6).
pub fn effective_member(split: Option<MemberId>, account: Option<MemberId>) -> Option<MemberId> {
    split.or(account)
}

/// Cases cochées : « Foyer » et des membres (R-MBR-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberSelection {
    /// Ce qui n'est affecté à aucun membre.
    pub household: bool,
    pub members: HashSet<MemberId>,
}

impl MemberSelection {
    /// Compte ce qui est affecté à `member`, ou au foyer pour `None` (R-MBR-9).
    pub fn includes(&self, member: Option<MemberId>) -> bool {
        match member {
            Some(m) => self.members.contains(&m),
            None => self.household,
        }
    }

    /// Toutes les cases sont cochées : les chiffres sont ceux du foyer (R-MBR-10). Un membre qui n'existe plus
    /// est ignoré.
    pub fn is_all(&self, existing: &[MemberId]) -> bool {
        self.household && existing.iter().all(|m| self.members.contains(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_mbr_1_color_is_rrggbb() {
        assert!(check_color("#1a73E8").is_ok());
        for bad in ["1a73e8", "#1a73e", "#1a73e8f", "#gggggg", "", "#ééé"] {
            assert_eq!(check_color(bad), Err(DomainError::InvalidMemberColor), "{bad}");
        }
    }

    #[test]
    fn r_mbr_5_a_transfer_split_has_no_member() {
        assert_eq!(check_split_member(true, true), Err(DomainError::MemberOnTransfer));
        assert!(check_split_member(false, true).is_ok());
        assert!(check_split_member(true, false).is_ok());
    }

    #[test]
    fn r_mbr_5_the_member_is_kept_only_when_all_splits_share_it() {
        let (leo, claire) = (MemberId::new(), MemberId::new());
        assert_eq!(common_member(&[Some(leo), Some(leo)]), Some(leo));
        assert_eq!(common_member(&[Some(leo), Some(claire)]), None);
        assert_eq!(common_member(&[Some(leo), None]), None);
        assert_eq!(common_member(&[]), None);
    }

    #[test]
    fn r_mbr_6_split_member_then_account_member_then_household() {
        let (quentin, claire) = (MemberId::new(), MemberId::new());
        assert_eq!(effective_member(Some(claire), Some(quentin)), Some(claire));
        assert_eq!(effective_member(None, Some(quentin)), Some(quentin));
        assert_eq!(effective_member(None, None), None);
    }

    #[test]
    fn r_mbr_9_r_mbr_10_selection_includes_checked_members_and_household() {
        let (quentin, claire) = (MemberId::new(), MemberId::new());
        let only_claire = MemberSelection { household: false, members: HashSet::from([claire]) };
        assert!(only_claire.includes(Some(claire)));
        assert!(!only_claire.includes(Some(quentin)));
        assert!(!only_claire.includes(None));
        assert!(!only_claire.is_all(&[quentin, claire]));
        let all = MemberSelection { household: true, members: HashSet::from([quentin, claire]) };
        assert!(all.includes(None));
        assert!(all.is_all(&[quentin, claire]));
        assert!(!all.is_all(&[quentin, claire, MemberId::new()]));
    }
}
