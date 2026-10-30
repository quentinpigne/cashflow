//! Budgets (spec 05, ADR-0014) : lignes versionnées, surcharges, reports, objectifs d'épargne, projets,
//! synthèse du mois et seuils d'alerte.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::shared::{Date, DomainError, Money};

/// Mois civil `AAAA-MM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Month {
    pub year: i32,
    pub month: u32,
}

impl Month {
    pub fn of(date: Date) -> Self {
        Month { year: date.year(), month: date.month() }
    }

    pub fn first_day(self) -> Date {
        NaiveDate::from_ymd_opt(self.year, self.month, 1).unwrap_or_default()
    }

    pub fn last_day(self) -> Date {
        self.next().first_day().pred_opt().unwrap_or_default()
    }

    pub fn next(self) -> Self {
        if self.month == 12 {
            Month { year: self.year + 1, month: 1 }
        } else {
            Month { year: self.year, month: self.month + 1 }
        }
    }

    pub fn previous(self) -> Self {
        if self.month == 1 {
            Month { year: self.year - 1, month: 12 }
        } else {
            Month { year: self.year, month: self.month - 1 }
        }
    }

    pub fn contains(self, date: Date) -> bool {
        Month::of(date) == self
    }

    /// Mois successifs de `self` à `last`, inclus.
    pub fn until(self, last: Month) -> Vec<Month> {
        let mut months = Vec::new();
        let mut month = self;
        while month <= last {
            months.push(month);
            month = month.next();
        }
        months
    }
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

impl FromStr for Month {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (year, month) = s.split_once('-').ok_or(DomainError::InvalidMonth)?;
        let year: i32 = year.parse().map_err(|_| DomainError::InvalidMonth)?;
        let month: u32 = month.parse().map_err(|_| DomainError::InvalidMonth)?;
        if year.to_string().len() != 4 || !(1..=12).contains(&month) {
            return Err(DomainError::InvalidMonth);
        }
        Ok(Month { year, month })
    }
}

/// Statut d'un projet (R-BUD-20) ; seul un projet en cours reçoit de nouvelles ventilations (R-BUD-24).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectStatus {
    /// En projet : envisagé, rien n'est encore dépensé.
    Planned,
    /// En cours.
    #[default]
    Active,
    /// Terminé : bilan figé.
    Closed,
}

/// Nature d'une ligne de budget.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BudgetKind {
    Expense,
    Income,
    Savings,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Periodicity {
    Monthly,
    Yearly,
}

/// Une version de ligne de budget, valable de `from` à `to` (R-BUD-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineVersion<L, C, A> {
    pub id: L,
    pub kind: BudgetKind,
    /// Catégories d'une ligne de dépense ou de revenu (R-BUD-10) ; aucune pour un objectif d'épargne.
    pub categories: Vec<C>,
    pub target_account: Option<A>,
    pub periodicity: Periodicity,
    /// Premier mois de l'année budgétaire d'une ligne annuelle (R-BUD-15).
    pub year_start_month: u32,
    pub amount: Money,
    pub rollover: bool,
    pub from: Month,
    pub to: Option<Month>,
}

impl<L, C, A> LineVersion<L, C, A> {
    pub fn in_force(&self, month: Month) -> bool {
        self.from <= month && self.to.is_none_or(|to| month <= to)
    }

    /// Premier mois de l'année budgétaire qui contient `month` (R-BUD-15).
    pub fn budget_year_start(&self, month: Month) -> Month {
        budget_year_start(self.year_start_month, month)
    }

    /// Mois de l'année budgétaire qui contient `month`.
    pub fn budget_year(&self, month: Month) -> Vec<Month> {
        let start = self.budget_year_start(month);
        let mut months = vec![start];
        while months.len() < 12 {
            let next = months[months.len() - 1].next();
            months.push(next);
        }
        months
    }
}

/// Premier mois de l'année budgétaire commençant au mois `start` qui contient `month` (R-BUD-15).
pub fn budget_year_start(start: u32, month: Month) -> Month {
    let start = start.clamp(1, 12);
    let year = if month.month >= start { month.year } else { month.year - 1 };
    Month { year, month: start }
}

/// Surcharge d'une ligne pour un mois, ou pour l'année budgétaire qui commence ce mois-là (R-BUD-51).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Override<L> {
    pub line: L,
    /// Mois surchargé ; pour une ligne annuelle, premier mois de l'année budgétaire surchargée.
    pub period: Month,
    pub amount: Money,
}

/// Valide les champs d'une version de ligne (modèle de données, R-BUD-10, R-BUD-19, R-BUD-30).
pub fn check_line<L, C: Eq + Hash, A>(line: &LineVersion<L, C, A>) -> Result<(), DomainError> {
    if !line.amount.is_positive() {
        return Err(DomainError::BudgetAmountNotPositive);
    }
    if line.to.is_some_and(|to| to < line.from) {
        return Err(DomainError::BudgetPeriodInverted);
    }
    match line.kind {
        BudgetKind::Savings => {
            if line.target_account.is_none() || !line.categories.is_empty() || line.periodicity != Periodicity::Monthly
            {
                return Err(DomainError::InvalidBudgetLine);
            }
        }
        BudgetKind::Expense | BudgetKind::Income => {
            let distinct: std::collections::HashSet<&C> = line.categories.iter().collect();
            if line.categories.is_empty() || distinct.len() != line.categories.len() || line.target_account.is_some() {
                return Err(DomainError::InvalidBudgetLine);
            }
            if line.kind == BudgetKind::Income && line.rollover {
                return Err(DomainError::IncomeRollover);
            }
        }
    }
    Ok(())
}

/// Quote-part mensuelle exacte d'un montant annuel (R-BUD-16) : 11 parts arrondies à l'inférieur, la
/// dernière absorbe le reste. `index` est le rang du mois dans l'année budgétaire (0 à 11).
pub fn monthly_share(annual: Money, index: usize) -> Money {
    let base = annual.minor().div_euclid(12);
    if index == 11 { Money::from_minor(annual.minor() - base * 11) } else { Money::from_minor(base) }
}

/// Une ventilation vue par le budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetSplit<C, P> {
    /// Date de la transaction (R-BUD-3).
    pub date: Date,
    pub amount: Money,
    pub category: Option<C>,
    /// Projet, et si ses dépenses sont retirées des lignes de période (R-BUD-21).
    pub project: Option<(P, bool)>,
    /// Le compte de la transaction est dans le périmètre du budget (R-BUD-1, R-BUD-2).
    pub in_budget: bool,
    /// Jambe de virement interne, remboursement, acquisition ou catégorie hors analyses (R-TRX-24) : n'alimente
    /// aucune ligne de dépense ou de revenu (R-BUD-2).
    pub is_movement: bool,
}

/// Un virement interne vu par les objectifs d'épargne (R-BUD-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavingsMove<A> {
    pub date: Date,
    /// Compte d'épargne concerné.
    pub account: A,
    /// Positif : versement depuis un compte du budget ; négatif : retrait vers un compte du budget.
    pub amount: Money,
}

/// Où va une ventilation dans le budget d'un mois.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination<L> {
    Line(L),
    /// Dépense ou revenu catégorisé sans ligne (R-BUD-12).
    OffBudget,
    Uncategorized,
    /// Dépense d'un projet exclu des lignes de période (R-BUD-21, R-BUD-41).
    ExcludedProject,
    /// Hors périmètre : compte hors budget ou jambe de virement.
    Ignored,
}

/// Ligne la plus spécifique d'une catégorie : celle qui la porte, sinon celle qui porte sa parente (R-BUD-12).
pub fn most_specific<L: Copy, C: Copy + Eq + Hash, A>(
    lines: &[&LineVersion<L, C, A>],
    category: C,
    parent: Option<C>,
) -> Option<L> {
    let find = |c: C| lines.iter().find(|l| l.kind != BudgetKind::Savings && l.categories.contains(&c)).map(|l| l.id);
    find(category).or_else(|| parent.and_then(find))
}

/// Destination d'une ventilation parmi les lignes en vigueur.
pub fn destination<L: Copy, C: Copy + Eq + Hash, A, P>(
    split: &BudgetSplit<C, P>,
    lines: &[&LineVersion<L, C, A>],
    parents: &HashMap<C, C>,
) -> Destination<L> {
    if !split.in_budget || split.is_movement {
        return Destination::Ignored;
    }
    if split.project.as_ref().is_some_and(|(_, excluded)| *excluded) {
        return Destination::ExcludedProject;
    }
    match split.category {
        None => Destination::Uncategorized,
        Some(category) => match most_specific(lines, category, parents.get(&category).copied()) {
            Some(line) => Destination::Line(line),
            None => Destination::OffBudget,
        },
    }
}

/// Montant d'une ligne sur une période : dépense en positif (remboursements déduits), revenu tel quel (R-BUD-13).
fn signed(kind: BudgetKind, amount: Money) -> Money {
    if kind == BudgetKind::Expense { -amount } else { amount }
}

/// État d'une ligne pour un mois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMonth<L> {
    pub line: L,
    pub kind: BudgetKind,
    pub periodicity: Periodicity,
    /// Prévu du mois : montant, surcharge, ou quote-part annuelle (R-BUD-16).
    pub planned: Money,
    /// Réel du mois ; pour une ligne annuelle, cumul depuis le début de l'année budgétaire (R-BUD-15).
    pub actual: Money,
    /// Report cumulé des périodes précédentes de la version (R-BUD-17).
    pub carried: Money,
    /// Disponible = prévu + report ; pour une ligne annuelle, montant de l'année (surcharge comprise).
    pub available: Money,
}

impl<L> LineMonth<L> {
    pub fn remaining(&self) -> Money {
        self.available - self.actual
    }
}

/// Tout ce qu'il faut pour calculer le budget.
pub struct BudgetData<'a, L, C, A, P> {
    pub lines: &'a [LineVersion<L, C, A>],
    pub overrides: &'a [Override<L>],
    pub splits: &'a [BudgetSplit<C, P>],
    pub moves: &'a [SavingsMove<A>],
    pub parents: &'a HashMap<C, C>,
}

impl<'a, L, C, A, P> BudgetData<'a, L, C, A, P>
where
    L: Copy + Eq + Hash,
    C: Copy + Eq + Hash,
    A: Copy + Eq + Hash,
{
    /// Range chaque ventilation et chaque virement d'épargne dans sa ligne, une fois pour toutes.
    pub fn build(self) -> Budget<'a, L, C, A, P> {
        let mut actuals: HashMap<(Month, L), Money> = HashMap::new();
        let mut in_force: HashMap<Month, Vec<&LineVersion<L, C, A>>> = HashMap::new();
        for split in self.splits {
            let month = Month::of(split.date);
            let lines =
                in_force.entry(month).or_insert_with(|| self.lines.iter().filter(|l| l.in_force(month)).collect());
            if let Destination::Line(id) = destination(split, lines, self.parents)
                && let Some(line) = lines.iter().find(|l| l.id == id)
            {
                *actuals.entry((month, id)).or_insert(Money::ZERO) += signed(line.kind, split.amount);
            }
        }
        for movement in self.moves {
            let month = Month::of(movement.date);
            for line in self.lines.iter().filter(|l| {
                l.kind == BudgetKind::Savings && l.target_account == Some(movement.account) && l.in_force(month)
            }) {
                *actuals.entry((month, line.id)).or_insert(Money::ZERO) += movement.amount;
            }
        }
        Budget { data: self, actuals }
    }
}

/// Le budget calculé : les réels de chaque ligne, mois par mois.
pub struct Budget<'a, L, C, A, P> {
    data: BudgetData<'a, L, C, A, P>,
    actuals: HashMap<(Month, L), Money>,
}

impl<L, C, A, P> Budget<'_, L, C, A, P>
where
    L: Copy + Eq + Hash,
    C: Copy + Eq + Hash,
    A: Copy + Eq + Hash,
{
    fn in_force(&self, month: Month) -> Vec<&LineVersion<L, C, A>> {
        self.data.lines.iter().filter(|l| l.in_force(month)).collect()
    }

    fn override_for(&self, line: L, period: Month) -> Option<Money> {
        self.data.overrides.iter().find(|o| o.line == line && o.period == period).map(|o| o.amount)
    }

    /// Montant prévu d'une période : le mois pour une ligne mensuelle, l'année pour une ligne annuelle.
    fn period_amount(&self, line: &LineVersion<L, C, A>, month: Month) -> Money {
        let period = match line.periodicity {
            Periodicity::Monthly => month,
            Periodicity::Yearly => line.budget_year_start(month),
        };
        self.override_for(line.id, period).unwrap_or(line.amount)
    }

    /// Réel d'une ligne sur des mois donnés (R-BUD-13, R-BUD-31).
    fn actual(&self, line: &LineVersion<L, C, A>, months: &[Month]) -> Money {
        months.iter().filter_map(|m| self.actuals.get(&(*m, line.id))).copied().sum()
    }

    /// État d'une ligne pour le mois `month` (R-BUD-15 à R-BUD-17, R-BUD-31).
    pub fn line_month(&self, line: &LineVersion<L, C, A>, month: Month) -> LineMonth<L> {
        match line.periodicity {
            Periodicity::Monthly => {
                let planned = self.period_amount(line, month);
                let actual = self.actual(line, &[month]);
                let carried = if line.rollover && line.kind != BudgetKind::Income {
                    line.from
                        .until(month.previous())
                        .into_iter()
                        .map(|p| self.period_amount(line, p) - self.actual(line, &[p]))
                        .sum()
                } else {
                    Money::ZERO
                };
                LineMonth {
                    line: line.id,
                    kind: line.kind,
                    periodicity: line.periodicity,
                    planned,
                    actual,
                    carried,
                    available: planned + carried,
                }
            }
            Periodicity::Yearly => {
                let year = line.budget_year(month);
                let annual = self.period_amount(line, month);
                let index = year.iter().position(|m| *m == month).unwrap_or(0);
                let elapsed: Vec<Month> = year.iter().copied().filter(|m| *m <= month).collect();
                let carried = if line.rollover && line.kind != BudgetKind::Income {
                    let mut total = Money::ZERO;
                    let mut start = line.budget_year_start(line.from);
                    let current = line.budget_year_start(month);
                    while start < current {
                        let months: Vec<Month> = line.budget_year(start);
                        total += self.period_amount(line, start) - self.actual(line, &months);
                        start = Month { year: start.year + 1, month: start.month };
                    }
                    total
                } else {
                    Money::ZERO
                };
                LineMonth {
                    line: line.id,
                    kind: line.kind,
                    periodicity: line.periodicity,
                    planned: monthly_share(annual, index),
                    actual: self.actual(line, &elapsed),
                    carried,
                    available: annual + carried,
                }
            }
        }
    }

    /// Le budget d'un mois : lignes en vigueur et totaux (BUD-03).
    pub fn month(&self, month: Month) -> MonthBudget<L, C> {
        let lines: Vec<LineMonth<L>> = self.in_force(month).into_iter().map(|l| self.line_month(l, month)).collect();
        let in_force = self.in_force(month);
        let mut off_budget: HashMap<Option<C>, Money> = HashMap::new();
        let mut off_budget_income = Money::ZERO;
        let mut uncategorized_out = Money::ZERO;
        let mut uncategorized_in = Money::ZERO;
        let mut excluded_projects = Money::ZERO;
        for split in self.data.splits.iter().filter(|s| month.contains(s.date)) {
            match destination(split, &in_force, self.data.parents) {
                Destination::OffBudget if split.amount.is_negative() => {
                    *off_budget.entry(split.category).or_insert(Money::ZERO) += -split.amount;
                }
                Destination::OffBudget => off_budget_income += split.amount,
                Destination::Uncategorized if split.amount.is_negative() => uncategorized_out += -split.amount,
                Destination::Uncategorized => uncategorized_in += split.amount,
                Destination::ExcludedProject => excluded_projects += -split.amount,
                _ => {}
            }
        }
        // Une catégorie de dépense remboursée sans ligne compte en déduction (R-TRX-13) : on ne garde que les
        // catégories dont le solde est une dépense.
        off_budget.retain(|_, amount| !amount.is_zero());
        let off_budget_total: Money = off_budget.values().sum();
        let sum = |kind: BudgetKind, f: fn(&LineMonth<L>) -> Money| {
            lines.iter().filter(|l| l.kind == kind).map(f).sum::<Money>()
        };
        let planned_income = sum(BudgetKind::Income, |l| l.planned);
        let planned_expenses = sum(BudgetKind::Expense, |l| l.planned);
        let planned_savings = sum(BudgetKind::Savings, |l| l.planned);
        let actual_income = sum(BudgetKind::Income, |l| l.actual);
        // Pour le solde du mois, le réel d'une ligne annuelle est celui du seul mois (R-BUD-41).
        let monthly_expenses: Money = self
            .in_force(month)
            .into_iter()
            .filter(|l| l.kind == BudgetKind::Expense)
            .map(|l| self.actual(l, &[month]))
            .sum();
        let actual_savings = sum(BudgetKind::Savings, |l| l.actual);
        let income = actual_income + off_budget_income + uncategorized_in;
        let expenses = monthly_expenses + off_budget_total + uncategorized_out + excluded_projects;
        MonthBudget {
            month,
            lines,
            off_budget,
            off_budget_income,
            uncategorized_out,
            uncategorized_in,
            excluded_projects,
            // R-BUD-40 : les quotes-parts annuelles sont déjà dans le prévu des lignes annuelles.
            to_budget: planned_income - planned_expenses - planned_savings,
            // R-BUD-41.
            balance: income - expenses - actual_savings,
            income,
            expenses,
            savings: actual_savings,
        }
    }
}

/// Le budget d'un mois (BUD-03).
#[derive(Debug, Clone)]
pub struct MonthBudget<L, C> {
    pub month: Month,
    pub lines: Vec<LineMonth<L>>,
    /// Dépenses catégorisées sans ligne, par catégorie (R-BUD-12).
    pub off_budget: HashMap<Option<C>, Money>,
    pub off_budget_income: Money,
    pub uncategorized_out: Money,
    pub uncategorized_in: Money,
    /// Dépenses des projets exclus des lignes de période (R-BUD-41).
    pub excluded_projects: Money,
    /// Reste à budgéter, prévu (R-BUD-40).
    pub to_budget: Money,
    /// Solde du mois, réel (R-BUD-41).
    pub balance: Money,
    pub income: Money,
    pub expenses: Money,
    pub savings: Money,
}

/// Réel d'un projet par catégorie, remboursements déduits (R-BUD-22), toutes ventilations confondues
/// quel que soit le compte (R-BUD-25).
pub fn project_actual<C: Copy + Eq + Hash + Ord, P: Copy + Eq>(
    splits: &[BudgetSplit<C, P>],
    project: P,
) -> Vec<(Option<C>, Money)> {
    let mut by_category: HashMap<Option<C>, Money> = HashMap::new();
    for split in splits.iter().filter(|s| s.project.is_some_and(|(p, _)| p == project)) {
        *by_category.entry(split.category).or_insert(Money::ZERO) += -split.amount;
    }
    let mut result: Vec<(Option<C>, Money)> = by_category.into_iter().collect();
    result.sort_by_key(|(c, _)| *c);
    result
}

/// Poste d'un projet (R-BUD-26) : `categories` proposent le poste d'une ventilation (R-BUD-27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLine<L, C> {
    pub id: L,
    pub name: String,
    pub amount: Money,
    pub categories: Vec<C>,
}

/// Valide le prévu d'un projet : un montant global positif sans poste, aucun avec des postes ; des postes de
/// montant positif, de noms distincts sans tenir compte de la casse (R-BUD-26) ; une catégorie dans au plus un
/// poste (R-BUD-27).
pub fn check_project_plan<L, C: Eq + Hash>(
    amount: Option<Money>,
    lines: &[ProjectLine<L, C>],
) -> Result<(), DomainError> {
    match amount {
        Some(amount) if !lines.is_empty() || !amount.is_positive() => return Err(DomainError::InvalidProjectPlan),
        None if lines.is_empty() => return Err(DomainError::InvalidProjectPlan),
        _ => {}
    }
    let mut names = std::collections::HashSet::new();
    let mut categories = std::collections::HashSet::new();
    for line in lines {
        if !line.amount.is_positive() || line.name.trim().is_empty() || !names.insert(line.name.trim().to_lowercase()) {
            return Err(DomainError::InvalidProjectPlan);
        }
        if !line.categories.iter().all(|c| categories.insert(c)) {
            return Err(DomainError::ProjectLineCategoryTaken);
        }
    }
    Ok(())
}

/// Prévu d'un projet : son montant global, ou la somme de ses postes (R-BUD-26).
pub fn project_planned<L, C>(amount: Option<Money>, lines: &[ProjectLine<L, C>]) -> Money {
    if lines.is_empty() { amount.unwrap_or(Money::ZERO) } else { lines.iter().map(|l| l.amount).sum() }
}

/// Poste proposé pour une ventilation : celui qui porte sa catégorie, sinon sa catégorie parente (R-BUD-27).
pub fn proposed_line<L: Copy, C: Copy + Eq>(
    lines: &[ProjectLine<L, C>],
    category: Option<C>,
    parent: Option<C>,
) -> Option<L> {
    let find = |c: C| lines.iter().find(|l| l.categories.contains(&c)).map(|l| l.id);
    category.and_then(find).or_else(|| parent.and_then(find))
}

/// Réel de chaque poste et réel hors postes, à partir des ventilations du projet et de leur poste ; remboursements
/// déduits (R-BUD-28).
pub fn project_line_actuals<L: Copy + Eq + Hash>(splits: &[(Money, Option<L>)]) -> (HashMap<L, Money>, Money) {
    let mut by_line: HashMap<L, Money> = HashMap::new();
    let mut unassigned = Money::ZERO;
    for (amount, line) in splits {
        match line {
            Some(line) => *by_line.entry(*line).or_insert(Money::ZERO) += -*amount,
            None => unassigned += -*amount,
        }
    }
    (by_line, unassigned)
}

/// Seuils d'alerte par défaut, en pour cent du disponible (R-BUD-60).
pub const DEFAULT_THRESHOLDS: [u32; 2] = [80, 100];

/// Seuils franchis : ceux que le réel dépasse, `réel > seuil × disponible` (R-BUD-60, « au-delà », R-BUD-61) ;
/// une ligne consommée à 100 % pile ne franchit pas le seuil de 100 %. Pour un objectif d'épargne, voir
/// [`savings_goal_missed`].
pub fn crossed(actual: Money, available: Money, thresholds: &[u32]) -> BTreeSet<u32> {
    if !available.is_positive() {
        return if actual.is_positive() { thresholds.iter().copied().collect() } else { BTreeSet::new() };
    }
    thresholds
        .iter()
        .copied()
        .filter(|t| i128::from(actual.minor()) * 100 > i128::from(available.minor()) * i128::from(*t))
        .collect()
}

/// Un objectif d'épargne est signalé s'il n'est pas atteint en fin de mois (R-BUD-61).
pub fn savings_goal_missed(actual: Money, available: Money, month_over: bool) -> bool {
    month_over && actual < available
}

/// Clé de dédoublonnage d'une alerte : une seule notification par ligne, seuil et période (R-BUD-62).
pub fn alert_key(kind: &str, id: impl fmt::Display, period: impl fmt::Display, threshold: u32) -> String {
    format!("{kind}:{id}:{period}:{threshold}")
}

/// Montant proposé par l'assistant (BUD-10) : moyenne des dépenses mensuelles des mois donnés, arrondie à
/// l'euro supérieur.
pub fn suggested_amount(monthly: &[Money]) -> Money {
    if monthly.is_empty() {
        return Money::ZERO;
    }
    let total: i64 = monthly.iter().map(|m| m.minor()).sum();
    let count = i64::try_from(monthly.len()).unwrap_or(1);
    let average = (total + count - 1).div_euclid(count);
    Money::from_minor((average + 99).div_euclid(100) * 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn m(s: &str) -> Month {
        s.parse().unwrap()
    }

    fn eur(minor: i64) -> Money {
        Money::from_minor(minor)
    }

    type Line = LineVersion<u32, u32, u32>;

    fn line(id: u32, category: u32, amount: i64, from: &str) -> Line {
        LineVersion {
            id,
            kind: BudgetKind::Expense,
            categories: vec![category],
            target_account: None,
            periodicity: Periodicity::Monthly,
            year_start_month: 1,
            amount: eur(amount),
            rollover: false,
            from: m(from),
            to: None,
        }
    }

    fn spend(date: &str, amount: i64, category: u32) -> BudgetSplit<u32, u32> {
        BudgetSplit {
            date: d(date),
            amount: eur(amount),
            category: Some(category),
            project: None,
            in_budget: true,
            is_movement: false,
        }
    }

    #[test]
    fn months_parse_and_iterate() {
        assert_eq!(m("2027-03").to_string(), "2027-03");
        assert!("2027-13".parse::<Month>().is_err());
        assert_eq!(m("2026-12").next(), m("2027-01"));
        assert_eq!(m("2027-02").last_day(), d("2027-02-28"));
        assert_eq!(m("2026-11").until(m("2027-01")).len(), 3);
    }

    /// Critère d'acceptation de BUD-03 / R-BUD-12.
    #[test]
    fn bud_03_most_specific_line() {
        const ALIMENTATION: u32 = 1;
        const RESTAURANTS: u32 = 2;
        const SUPERMARCHE: u32 = 3;
        let lines = [line(10, ALIMENTATION, 80_000, "2027-01"), line(20, RESTAURANTS, 15_000, "2027-01")];
        let parents = HashMap::from([(RESTAURANTS, ALIMENTATION), (SUPERMARCHE, ALIMENTATION)]);
        let splits = [spend("2027-03-05", -6_000, SUPERMARCHE), spend("2027-03-06", -4_000, RESTAURANTS)];
        let data = BudgetData { lines: &lines, overrides: &[], splits: &splits, moves: &[], parents: &parents }.build();
        let month = data.month(m("2027-03"));
        let actual = |id| month.lines.iter().find(|l| l.line == id).unwrap().actual;
        assert_eq!(actual(10), eur(6_000));
        assert_eq!(actual(20), eur(4_000));
    }

    /// Critère d'acceptation de BUD-02 / R-BUD-10 : une ligne sur plusieurs catégories.
    #[test]
    fn r_bud_10_line_over_several_categories() {
        const AUTO: u32 = 1;
        const CARBURANT: u32 = 2;
        const PEAGE: u32 = 3;
        const ENTRETIEN: u32 = 4;
        let car = LineVersion { categories: vec![CARBURANT, PEAGE], ..line(10, CARBURANT, 20_000, "2027-01") };
        let lines = [car, line(20, AUTO, 10_000, "2027-01")];
        let parents = HashMap::from([(CARBURANT, AUTO), (PEAGE, AUTO), (ENTRETIEN, AUTO)]);
        let splits = [
            spend("2027-03-05", -6_000, CARBURANT),
            spend("2027-03-06", -1_500, PEAGE),
            spend("2027-03-07", -9_000, ENTRETIEN),
        ];
        let data = BudgetData { lines: &lines, overrides: &[], splits: &splits, moves: &[], parents: &parents }.build();
        let month = data.month(m("2027-03"));
        let line = |id| month.lines.iter().find(|l| l.line == id).unwrap();
        assert_eq!((line(10).actual, line(10).remaining()), (eur(7_500), eur(12_500)));
        // Entretien n'est pas dans « Voiture » : il va à la ligne de sa parente (R-BUD-12).
        assert_eq!(line(20).actual, eur(9_000));
    }

    #[test]
    fn r_bud_10_line_categories_are_required_and_distinct() {
        let empty = LineVersion { categories: Vec::new(), ..line(10, 1, 1_000, "2027-01") };
        assert_eq!(check_line(&empty), Err(DomainError::InvalidBudgetLine));
        let twice = LineVersion { categories: vec![1, 1], ..line(10, 1, 1_000, "2027-01") };
        assert_eq!(check_line(&twice), Err(DomainError::InvalidBudgetLine));
        assert_eq!(check_line(&LineVersion { categories: vec![1, 2], ..line(10, 1, 1_000, "2027-01") }), Ok(()));
    }

    /// Critère d'acceptation de BUD-04 / R-BUD-17.
    #[test]
    fn bud_04_rollover() {
        let mut supermarket = line(10, 3, 60_000, "2027-01");
        supermarket.rollover = true;
        let splits = [spend("2027-01-10", -55_000, 3), spend("2027-02-10", -64_000, 3)];
        let parents = HashMap::new();
        let data = BudgetData {
            lines: std::slice::from_ref(&supermarket),
            overrides: &[],
            splits: &splits,
            moves: &[],
            parents: &parents,
        }
        .build();
        let march = data.line_month(&supermarket, m("2027-03"));
        assert_eq!(march.carried, eur(1_000));
        assert_eq!(march.available, eur(61_000));
    }

    /// Critère d'acceptation de BUD-05 / R-BUD-16.
    #[test]
    fn bud_05_yearly_line_share_and_cumulative_actual() {
        let mut insurance = line(10, 7, 90_000, "2027-01");
        insurance.periodicity = Periodicity::Yearly;
        let splits = [spend("2027-03-15", -87_000, 7)];
        let parents = HashMap::new();
        let data = BudgetData {
            lines: std::slice::from_ref(&insurance),
            overrides: &[],
            splits: &splits,
            moves: &[],
            parents: &parents,
        }
        .build();
        let march = data.line_month(&insurance, m("2027-03"));
        assert_eq!(march.planned, eur(7_500));
        assert_eq!(march.actual, eur(87_000));
        assert_eq!(march.available, eur(90_000));
    }

    #[test]
    fn r_bud_16_shares_sum_to_the_annual_amount() {
        let shares: Vec<Money> = (0..12).map(|i| monthly_share(eur(100_000), i)).collect();
        assert_eq!(shares[0], eur(8_333));
        assert_eq!(shares[11], eur(8_337));
        assert_eq!(shares.iter().sum::<Money>(), eur(100_000));
    }

    #[test]
    fn r_bud_15_budget_year_starting_in_march() {
        let mut insurance = line(10, 7, 90_000, "2027-01");
        insurance.year_start_month = 3;
        assert_eq!(insurance.budget_year_start(m("2027-02")), m("2026-03"));
        assert_eq!(insurance.budget_year_start(m("2027-03")), m("2027-03"));
    }

    /// Critère d'acceptation de BUD-06 / R-BUD-21.
    #[test]
    fn bud_06_excluded_project_leaves_period_lines() {
        let restaurants = line(10, 2, 15_000, "2027-01");
        let mut split = spend("2027-03-10", -8_000, 2);
        split.project = Some((99, true));
        let splits = [split];
        let parents = HashMap::new();
        let data = BudgetData {
            lines: std::slice::from_ref(&restaurants),
            overrides: &[],
            splits: &splits,
            moves: &[],
            parents: &parents,
        }
        .build();
        let month = data.month(m("2027-03"));
        assert_eq!(month.lines[0].actual, Money::ZERO);
        assert_eq!(month.excluded_projects, eur(8_000));
        assert_eq!(project_actual(&splits, 99), vec![(Some(2), eur(8_000))]);
    }

    fn post(id: u32, name: &str, amount: i64, categories: &[u32]) -> ProjectLine<u32, u32> {
        ProjectLine { id, name: name.into(), amount: eur(amount), categories: categories.to_vec() }
    }

    /// Italie 2027 : Train (sans catégorie), Logement, Restaurants (catégorie 2), Loisirs (catégorie 7).
    fn italy() -> Vec<ProjectLine<u32, u32>> {
        vec![
            post(1, "Train", 50_000, &[]),
            post(2, "Logement", 30_000, &[]),
            post(3, "Restaurants", 10_000, &[2]),
            post(4, "Loisirs", 10_000, &[7]),
        ]
    }

    /// Critère d'acceptation de BUD-11 / R-BUD-26 : le prévu d'un projet détaillé est la somme de ses postes.
    #[test]
    fn r_bud_26_planned_is_sum_of_lines() {
        let lines = italy();
        assert_eq!(check_project_plan(None, &lines), Ok(()));
        assert_eq!(project_planned(None, &lines), eur(100_000));
        assert_eq!(project_planned::<u32, u32>(Some(eur(250_000)), &[]), eur(250_000));
        assert_eq!(project_planned(None, &lines[2..]), eur(20_000));
    }

    /// R-BUD-26 : un montant global ou des postes, jamais les deux ni aucun ; des postes nommés une fois.
    #[test]
    fn r_bud_26_plan_is_either_amount_or_lines() {
        let lines = italy();
        assert_eq!(check_project_plan::<u32, u32>(Some(eur(1)), &[]), Ok(()));
        assert_eq!(check_project_plan(Some(eur(1)), &lines), Err(DomainError::InvalidProjectPlan));
        assert_eq!(check_project_plan::<u32, u32>(None, &[]), Err(DomainError::InvalidProjectPlan));
        assert_eq!(check_project_plan::<u32, u32>(Some(Money::ZERO), &[]), Err(DomainError::InvalidProjectPlan));
        let twice = [post(1, "Train", 100, &[]), post(2, " train ", 100, &[])];
        assert_eq!(check_project_plan(None, &twice), Err(DomainError::InvalidProjectPlan));
        assert_eq!(check_project_plan(None, &[post(1, "Train", 0, &[])]), Err(DomainError::InvalidProjectPlan));
    }

    /// Critère d'acceptation de BUD-11 / R-BUD-27 : une catégorie ne propose qu'un poste.
    #[test]
    fn r_bud_27_category_in_one_line_only() {
        let mut lines = italy();
        lines[1].categories.push(2);
        assert_eq!(check_project_plan(None, &lines), Err(DomainError::ProjectLineCategoryTaken));
    }

    /// Critère d'acceptation de BUD-11 / R-BUD-27 : poste de la catégorie, sinon de la parente, sinon aucun.
    #[test]
    fn r_bud_27_proposed_line_most_specific() {
        let lines = italy();
        assert_eq!(proposed_line(&lines, Some(2), Some(1)), Some(3));
        // « Culture » (8), sous-catégorie de Loisirs (7).
        assert_eq!(proposed_line(&lines, Some(8), Some(7)), Some(4));
        // « Voyages » (9) : aucun poste ne la porte, hors postes.
        assert_eq!(proposed_line(&lines, Some(9), None), None);
        assert_eq!(proposed_line(&lines, None, None), None);
    }

    /// Critère d'acceptation de BUD-11 / R-BUD-28 : réel par poste et hors postes, remboursements déduits.
    #[test]
    fn r_bud_28_actual_by_line_and_unassigned() {
        let splits = [(eur(-4_500), Some(3)), (eur(-1_200), Some(4)), (eur(-18_000), None), (eur(500), Some(3))];
        let (by_line, unassigned) = project_line_actuals(&splits);
        assert_eq!(by_line[&3], eur(4_000));
        assert_eq!(by_line[&4], eur(1_200));
        assert_eq!(unassigned, eur(18_000));
        let moved = [(eur(-18_000), Some(1))];
        let (by_line, unassigned) = project_line_actuals(&moved);
        assert_eq!(by_line[&1], eur(18_000));
        assert_eq!(unassigned, Money::ZERO);
    }

    /// Critère d'acceptation de BUD-07 / R-BUD-31.
    #[test]
    fn bud_07_savings_goal_counts_withdrawals() {
        let goal = LineVersion {
            id: 10,
            kind: BudgetKind::Savings,
            categories: Vec::new(),
            target_account: Some(5),
            periodicity: Periodicity::Monthly,
            year_start_month: 1,
            amount: eur(20_000),
            rollover: false,
            from: m("2027-01"),
            to: None,
        };
        let moves = [
            SavingsMove { date: d("2027-03-05"), account: 5, amount: eur(30_000) },
            SavingsMove { date: d("2027-03-20"), account: 5, amount: eur(-10_000) },
        ];
        let parents = HashMap::new();
        let data = BudgetData::<u32, u32, u32, u32> {
            lines: std::slice::from_ref(&goal),
            overrides: &[],
            splits: &[],
            moves: &moves,
            parents: &parents,
        }
        .build();
        assert_eq!(data.line_month(&goal, m("2027-03")).actual, eur(20_000));
        check_line(&goal).unwrap();
    }

    #[test]
    fn r_bud_51_override_and_versions() {
        let gifts = line(10, 8, 5_000, "2027-01");
        let overrides = [Override { line: 10, period: m("2027-12"), amount: eur(30_000) }];
        let parents = HashMap::new();
        let data = BudgetData::<u32, u32, u32, u32> {
            lines: std::slice::from_ref(&gifts),
            overrides: &overrides,
            splits: &[],
            moves: &[],
            parents: &parents,
        }
        .build();
        assert_eq!(data.line_month(&gifts, m("2027-12")).planned, eur(30_000));
        assert_eq!(data.line_month(&gifts, m("2027-11")).planned, eur(5_000));
        let mut closed = gifts.clone();
        closed.to = Some(m("2027-06"));
        assert!(!closed.in_force(m("2027-07")));
    }

    #[test]
    fn r_bud_40_and_41_month_synthesis() {
        let salary = LineVersion { kind: BudgetKind::Income, ..line(1, 100, 320_000, "2027-01") };
        let food = line(2, 3, 60_000, "2027-01");
        let mut uncategorized = spend("2027-03-02", -1_200, 0);
        uncategorized.category = None;
        let splits = [
            spend("2027-03-01", 320_000, 100),
            spend("2027-03-03", -54_020, 3),
            spend("2027-03-04", -3_780, 42),
            uncategorized,
        ];
        let parents = HashMap::new();
        let lines = [salary, food];
        let data = BudgetData::<u32, u32, u32, u32> {
            lines: &lines,
            overrides: &[],
            splits: &splits,
            moves: &[],
            parents: &parents,
        }
        .build();
        let month = data.month(m("2027-03"));
        assert_eq!(month.to_budget, eur(260_000));
        assert_eq!(month.off_budget.get(&Some(42)), Some(&eur(3_780)));
        assert_eq!(month.uncategorized_out, eur(1_200));
        assert_eq!(month.balance, eur(320_000 - 54_020 - 3_780 - 1_200));
    }

    /// Critère d'acceptation de BUD-08 / R-BUD-62 : les deux seuils franchis d'un coup.
    #[test]
    fn bud_08_thresholds() {
        assert_eq!(crossed(eur(11_000), eur(15_000), &DEFAULT_THRESHOLDS), BTreeSet::new());
        assert_eq!(crossed(eur(16_500), eur(15_000), &DEFAULT_THRESHOLDS), BTreeSet::from([80, 100]));
        assert_eq!(crossed(eur(12_100), eur(15_000), &DEFAULT_THRESHOLDS), BTreeSet::from([80]));
        assert_eq!(crossed(eur(15_000), eur(15_000), &DEFAULT_THRESHOLDS), BTreeSet::from([80]));
        assert!(savings_goal_missed(eur(10_000), eur(20_000), true));
        assert!(!savings_goal_missed(eur(10_000), eur(20_000), false));
        assert_eq!(alert_key("budget", 7, m("2027-03"), 80), "budget:7:2027-03:80");
    }

    #[test]
    fn bud_10_suggestion_rounds_up_to_the_euro() {
        assert_eq!(suggested_amount(&[eur(54_020), eur(61_000), eur(58_000)]), eur(57_700));
        assert_eq!(suggested_amount(&[]), Money::ZERO);
    }
}
