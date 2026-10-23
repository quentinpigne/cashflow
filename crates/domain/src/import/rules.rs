//! Règles de catégorisation (R-IMP-12 à R-IMP-14, R-IMP-19).

use serde::{Deserialize, Serialize};

use super::fingerprint::normalize_label;
use crate::shared::ids::{AccountId, CategoryId, PayeeId, ProjectId};
use crate::shared::{DomainError, Money};

/// Conditions d'une règle : toutes doivent être vraies (spec 03).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<LabelCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<AmountCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign: Option<Sign>,
    /// Vide : tous les comptes.
    #[serde(default)]
    pub accounts: Vec<AccountId>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelCondition {
    pub op: LabelOp,
    pub value: String,
    #[serde(default)]
    pub field: LabelField,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOp {
    Contains,
    StartsWith,
    Equals,
    /// Expression simplifiée : `*` remplace n'importe quelle suite de caractères, `?` un caractère.
    Regex,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelField {
    #[default]
    BankLabel,
    Label,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmountCondition {
    pub op: AmountOp,
    /// En unités mineures, signé.
    pub value: i64,
    #[serde(default)]
    pub max: Option<i64>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmountOp {
    Eq,
    Between,
    Lt,
    Gt,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sign {
    Debit,
    Credit,
}

/// Actions d'une règle (spec 03).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_label: Option<String>,
    /// Ventilation proposée : parts en pourcentage (poids), l'arrondi allant sur la dernière.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set_splits: Vec<SplitShare>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_project: Option<ProjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_transfer_to: Option<AccountId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_note: Option<String>,
    /// Tiers existant de la transaction (R-TIE-16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_payee: Option<PayeeId>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitShare {
    pub category_id: Option<CategoryId>,
    /// Poids de la part (par exemple un pourcentage en centièmes de point : 10000 = 100 %).
    pub weight: u64,
}

/// Une règle, telle que l'évalue le moteur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule<Id> {
    pub id: Id,
    pub conditions: Conditions,
    pub actions: Actions,
    pub stop_processing: bool,
}

/// Ce que le moteur examine d'une ligne ou d'une transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject<'a> {
    pub account: AccountId,
    pub bank_label: &'a str,
    pub label: &'a str,
    pub amount: Money,
}

/// Suggestion produite par les règles applicables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suggestion<Id> {
    pub label: Option<String>,
    pub splits: Vec<SplitShare>,
    pub project: Option<ProjectId>,
    pub transfer_to: Option<AccountId>,
    pub note: Option<String>,
    pub payee: Option<PayeeId>,
    /// Règles appliquées, dans l'ordre.
    pub rules: Vec<Id>,
}

impl<Id> Suggestion<Id> {
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

fn wildcard_match(pattern: &[char], text: &[char]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some('*'), _) => {
            wildcard_match(&pattern[1..], text) || (!text.is_empty() && wildcard_match(pattern, &text[1..]))
        }
        (Some('?'), Some(_)) => wildcard_match(&pattern[1..], &text[1..]),
        (Some(p), Some(t)) if p == t => wildcard_match(&pattern[1..], &text[1..]),
        _ => false,
    }
}

impl Conditions {
    pub fn matches(&self, subject: &Subject<'_>) -> bool {
        if !self.accounts.is_empty() && !self.accounts.contains(&subject.account) {
            return false;
        }
        if let Some(sign) = self.sign {
            let ok = match sign {
                Sign::Debit => subject.amount.is_negative(),
                Sign::Credit => subject.amount.is_positive(),
            };
            if !ok {
                return false;
            }
        }
        if let Some(amount) = &self.amount {
            let value = subject.amount.minor();
            let ok = match amount.op {
                AmountOp::Eq => value == amount.value,
                AmountOp::Lt => value < amount.value,
                AmountOp::Gt => value > amount.value,
                AmountOp::Between => {
                    let max = amount.max.unwrap_or(amount.value);
                    value >= amount.value.min(max) && value <= amount.value.max(max)
                }
            };
            if !ok {
                return false;
            }
        }
        if let Some(label) = &self.label {
            let field = match label.field {
                LabelField::BankLabel if !subject.bank_label.is_empty() => subject.bank_label,
                _ => subject.label,
            };
            let haystack = normalize_label(field);
            let needle = normalize_label(&label.value);
            let ok = match label.op {
                LabelOp::Contains => haystack.contains(&needle),
                LabelOp::StartsWith => haystack.starts_with(&needle),
                LabelOp::Equals => haystack == needle,
                LabelOp::Regex => {
                    let pattern: Vec<char> = needle.chars().collect();
                    let text: Vec<char> = haystack.chars().collect();
                    wildcard_match(&pattern, &text)
                }
            };
            if !ok {
                return false;
            }
        }
        true
    }

    /// Une règle sans aucune condition s'appliquerait à tout : refusée.
    pub fn check(&self) -> Result<(), DomainError> {
        if self.label.is_none() && self.amount.is_none() && self.sign.is_none() && self.accounts.is_empty() {
            return Err(DomainError::RuleWithoutCondition);
        }
        Ok(())
    }
}

/// Évalue les règles par priorité croissante (`rules` déjà triées). La première qui correspond
/// s'applique ; si elle ne stoppe pas, les suivantes complètent sans écraser (R-IMP-12).
pub fn evaluate<Id: Copy>(rules: &[Rule<Id>], subject: &Subject<'_>) -> Suggestion<Id> {
    let mut suggestion = Suggestion {
        label: None,
        splits: Vec::new(),
        project: None,
        transfer_to: None,
        note: None,
        payee: None,
        rules: Vec::new(),
    };
    for rule in rules {
        if !rule.conditions.matches(subject) {
            continue;
        }
        let a = &rule.actions;
        if suggestion.label.is_none() {
            suggestion.label.clone_from(&a.set_label);
        }
        if suggestion.splits.is_empty() {
            suggestion.splits.clone_from(&a.set_splits);
        }
        if suggestion.project.is_none() {
            suggestion.project = a.set_project;
        }
        if suggestion.transfer_to.is_none() {
            suggestion.transfer_to = a.mark_transfer_to;
        }
        if suggestion.note.is_none() {
            suggestion.note.clone_from(&a.set_note);
        }
        if suggestion.payee.is_none() {
            suggestion.payee = a.set_payee;
        }
        suggestion.rules.push(rule.id);
        if rule.stop_processing {
            break;
        }
    }
    suggestion
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(label: &str, minor: i64) -> Subject<'_> {
        Subject {
            account: AccountId::from_uuid(uuid::Uuid::nil()),
            bank_label: label,
            label,
            amount: Money::from_minor(minor),
        }
    }

    fn rule(id: u32, contains: &str, category: Option<CategoryId>, stop: bool, note: Option<&str>) -> Rule<u32> {
        Rule {
            id,
            conditions: Conditions {
                label: Some(LabelCondition {
                    op: LabelOp::Contains,
                    value: contains.into(),
                    field: LabelField::BankLabel,
                }),
                ..Default::default()
            },
            actions: Actions {
                set_splits: category
                    .map(|c| vec![SplitShare { category_id: Some(c), weight: 10_000 }])
                    .unwrap_or_default(),
                set_note: note.map(Into::into),
                ..Default::default()
            },
            stop_processing: stop,
        }
    }

    #[test]
    fn r_imp_12_first_matching_rule_wins_when_it_stops() {
        let (netflix, other) = (CategoryId::new(), CategoryId::new());
        let rules = [rule(1, "netflix", Some(netflix), true, None), rule(2, "net", Some(other), true, None)];
        let s = evaluate(&rules, &subject("PRLV NETFLIX.COM", -1_399));
        assert_eq!(s.splits[0].category_id, Some(netflix));
        assert_eq!(s.rules, vec![1]);
    }

    #[test]
    fn r_imp_12_non_stopping_rules_complete_without_overwriting() {
        let (first, second) = (CategoryId::new(), CategoryId::new());
        let rules =
            [rule(1, "netflix", Some(first), false, None), rule(2, "netflix", Some(second), true, Some("streaming"))];
        let s = evaluate(&rules, &subject("NETFLIX", -1_399));
        assert_eq!(s.splits[0].category_id, Some(first));
        assert_eq!(s.note.as_deref(), Some("streaming"));
        assert_eq!(s.rules, vec![1, 2]);
    }

    /// R-TIE-16 : une règle suivante complète le tiers sans écraser celui d'une règle précédente.
    #[test]
    fn r_tie_16_set_payee_follows_r_imp_12() {
        let (carrefour, other) = (PayeeId::new(), PayeeId::new());
        let mut first = rule(1, "carrefour", None, false, None);
        first.actions.set_payee = Some(carrefour);
        let mut second = rule(2, "carrefour", Some(CategoryId::new()), true, None);
        second.actions.set_payee = Some(other);
        let s = evaluate(&[first, second], &subject("CB CARREFOUR MEYLAN 14/03", -4_250));
        assert_eq!(s.payee, Some(carrefour));
        assert_eq!(s.splits.len(), 1);
    }

    #[test]
    fn conditions_on_amount_sign_and_wildcards() {
        let c = Conditions {
            amount: Some(AmountCondition { op: AmountOp::Between, value: -2_000, max: Some(-1_000) }),
            sign: Some(Sign::Debit),
            label: Some(LabelCondition {
                op: LabelOp::Regex,
                value: "PRLV * FREE*".into(),
                field: LabelField::BankLabel,
            }),
            accounts: Vec::new(),
        };
        assert!(c.matches(&subject("prlv sepa free mobile", -1_299)));
        assert!(!c.matches(&subject("prlv sepa free mobile", -2_299)));
        assert!(!c.matches(&subject("vir free", -1_299)));
        assert!(Conditions::default().check().is_err());
    }
}
