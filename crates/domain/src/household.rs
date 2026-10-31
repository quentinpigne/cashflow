//! Foyer (spec 14) : membres, affectations et choix des membres affichés.

use std::collections::HashSet;

use crate::shared::ids::MemberId;
use crate::shared::{DomainError, Money};

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

/// Quote-part entière d'un bien, en centièmes de pour cent (R-MBR-11).
pub const FULL_SHARE: u32 = 10_000;

/// Quotes-parts d'un bien : chacune de 0,01 % à 100 %, un membre au plus une fois, 100 % au plus en tout ; le
/// reste est au foyer (R-MBR-11).
pub fn check_shares<M: Eq + std::hash::Hash>(shares: &[(M, u32)]) -> Result<(), DomainError> {
    let distinct: HashSet<&M> = shares.iter().map(|(m, _)| m).collect();
    let total: u32 = shares.iter().map(|(_, bp)| *bp).sum();
    let valid = distinct.len() == shares.len()
        && shares.iter().all(|(_, bp)| (1..=FULL_SHARE).contains(bp))
        && total <= FULL_SHARE;
    if valid { Ok(()) } else { Err(DomainError::InvalidAssetShares) }
}

/// Quote-part équivalente à un montant saisi, d'après la valeur du bien, arrondie au centième de pour cent le
/// plus proche (R-MBR-11) ; `None` si la valeur est nulle ou le montant hors de (0, valeur].
pub fn share_of_amount(amount: Money, value: Money) -> Option<u32> {
    let (amount, value) = (i128::from(amount.minor()), i128::from(value.minor()));
    if value <= 0 || amount <= 0 || amount > value {
        return None;
    }
    let bp = (amount * i128::from(FULL_SHARE) * 2 + value) / (value * 2);
    u32::try_from(bp.max(1)).ok()
}

/// Parts d'une valeur selon des quotes-parts, puis la part du foyer (le reste), arrondies au centime au plus fort
/// reste, à reste égal dans l'ordre (le foyer en dernier) : leur somme égale la valeur (R-MBR-12, ADR-0001).
pub fn split_value(value: Money, shares: &[u32]) -> (Vec<Money>, Money) {
    let household = FULL_SHARE.saturating_sub(shares.iter().sum());
    let all: Vec<u32> = shares.iter().copied().chain([household]).collect();
    let total = i128::from(value.minor());
    let full = i128::from(FULL_SHARE);
    let exact: Vec<i128> = all.iter().map(|bp| total * i128::from(*bp)).collect();
    let mut parts: Vec<i128> = exact.iter().map(|e| e.div_euclid(full)).collect();
    let mut left = total - parts.iter().sum::<i128>();
    let mut order: Vec<usize> = (0..all.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(exact[i].rem_euclid(full)));
    for i in order {
        if left <= 0 {
            break;
        }
        parts[i] += 1;
        left -= 1;
    }
    let mut money: Vec<Money> =
        parts.into_iter().map(|p| Money::from_minor(i64::try_from(p).unwrap_or_default())).collect();
    let household = money.pop().unwrap_or(Money::ZERO);
    (money, household)
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
    fn r_mbr_11_shares_are_checked_and_amounts_become_shares() {
        assert!(check_shares(&[(1, 6_000), (2, 4_000)]).is_ok());
        assert!(check_shares::<i32>(&[]).is_ok());
        assert_eq!(check_shares(&[(1, 7_000), (2, 4_000)]), Err(DomainError::InvalidAssetShares));
        assert_eq!(check_shares(&[(1, 0)]), Err(DomainError::InvalidAssetShares));
        assert_eq!(check_shares(&[(1, 10), (1, 10)]), Err(DomainError::InvalidAssetShares));
        let house = Money::from_minor(42_000_000);
        assert_eq!(share_of_amount(Money::from_minor(10_500_000), house), Some(2_500));
        assert_eq!(share_of_amount(Money::from_minor(1), house), Some(1), "au moins 0,01 %");
        assert_eq!(share_of_amount(Money::from_minor(42_000_001), house), None);
    }

    #[test]
    fn r_mbr_12_parts_follow_shares_with_the_rest_to_the_household() {
        let (parts, household) = split_value(Money::from_minor(42_000_000), &[6_000, 3_000]);
        assert_eq!(parts, vec![Money::from_minor(25_200_000), Money::from_minor(12_600_000)]);
        assert_eq!(household, Money::from_minor(4_200_000));
        let (parts, household) = split_value(Money::from_minor(10_000_001), &[5_000, 5_000]);
        assert_eq!(parts, vec![Money::from_minor(5_000_001), Money::from_minor(5_000_000)]);
        assert_eq!(household, Money::ZERO);
        let (parts, household) = split_value(Money::from_minor(100), &[3_333, 3_333]);
        assert_eq!(parts.iter().copied().sum::<Money>() + household, Money::from_minor(100));
        assert_eq!(split_value(Money::from_minor(500), &[]), (vec![], Money::from_minor(500)));
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
