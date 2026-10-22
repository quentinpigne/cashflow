//! Salaires : emplois, rémunérations et primes ; évolution, comparaison au reçu (spec 10, ADR-0022).

use std::collections::HashMap;

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::salaries::{self, Bonus, BonusKind, Expected, Job, Pay, Projector, SalaryChangeReason};
use cashflow_domain::shared::ids::{CategoryId, EmploymentId, SalaryBonusId, SalaryId};
use cashflow_domain::{Date, Money, Rate};
use chrono::{Datelike, Months};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::clean;
use crate::db::salaries::{BonusRow, EmploymentRow, SalaryRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Mois de comparaison affichés sur la fiche d'un emploi (E30).
const CHECKED_MONTHS: u32 = 12;

// ---------------------------------------------------------------- Registre

/// Tous les emplois, leurs rémunérations et leurs primes, chargés une fois.
pub struct SalaryBook {
    pub employments: Vec<EmploymentRow>,
    salaries: HashMap<EmploymentId, Vec<SalaryRow>>,
    bonuses: HashMap<EmploymentId, Vec<BonusRow>>,
}

/// Ce que le domaine sait d'un emploi.
pub struct Payroll {
    pub job: Job,
    pub pays: Vec<Pay>,
    pub bonuses: Vec<Bonus>,
}

fn job(e: &EmploymentRow) -> Job {
    Job {
        start_date: e.start_date,
        end_date: e.end_date,
        raise_month: e.raise_month,
        paid_next_month: e.paid_next_month,
    }
}

fn pay(s: &SalaryRow) -> Pay {
    Pay { effective_month: s.effective_month, annual_gross: s.annual_gross, monthly_net: s.monthly_net }
}

fn bonus(b: &BonusRow) -> Bonus {
    Bonus {
        label: b.label.clone(),
        paid_on: b.paid_on,
        net: b.net_amount,
        gross: b.gross_amount,
        recurring: b.recurring,
    }
}

impl SalaryBook {
    pub async fn load(db: &Db) -> AppResult<Self> {
        let mut salaries: HashMap<EmploymentId, Vec<SalaryRow>> = HashMap::new();
        for s in db::salaries::salaries(&db.read).await? {
            salaries.entry(s.employment_id).or_default().push(s);
        }
        let mut bonuses: HashMap<EmploymentId, Vec<BonusRow>> = HashMap::new();
        for b in db::salaries::bonuses(&db.read).await? {
            bonuses.entry(b.employment_id).or_default().push(b);
        }
        Ok(Self { employments: db::salaries::list(&db.read).await?, salaries, bonuses })
    }

    pub fn salaries(&self, id: EmploymentId) -> &[SalaryRow] {
        self.salaries.get(&id).map(Vec::as_slice).unwrap_or_default()
    }

    pub fn bonuses(&self, id: EmploymentId) -> &[BonusRow] {
        self.bonuses.get(&id).map(Vec::as_slice).unwrap_or_default()
    }

    pub fn payroll(&self, e: &EmploymentRow) -> Payroll {
        Payroll {
            job: job(e),
            pays: self.salaries(e.id).iter().map(pay).collect(),
            bonuses: self.bonuses(e.id).iter().map(bonus).collect(),
        }
    }

    /// Catégories reliées à un emploi (R-PRJ-43).
    pub fn categories(&self) -> Vec<CategoryId> {
        self.employments.iter().map(|e| e.income_category_id).collect()
    }

    /// Salaires projetés de tous les emplois, pour chacun des `months` mois qui suivent le mois en cours
    /// (R-SAL-10, R-SAL-11).
    pub fn projected(&self, growth: Rate, today: Date, months: u32) -> Vec<Money> {
        let mut total = vec![Money::ZERO; months as usize];
        for e in &self.employments {
            let p = self.payroll(e);
            let monthly = Projector::new(&p.job, &p.pays, &p.bonuses, growth, today).monthly(months);
            for (sum, value) in total.iter_mut().zip(monthly) {
                *sum += value;
            }
        }
        total
    }

    /// Net annuel de tous les emplois, de l'année en cours à `last_year` (R-SAL-13).
    pub fn projected_years(&self, growth: Rate, today: Date, last_year: i32) -> Vec<(i32, Money)> {
        let mut years: Vec<(i32, Money)> = (today.year()..=last_year).map(|y| (y, Money::ZERO)).collect();
        for e in &self.employments {
            let p = self.payroll(e);
            let projected = Projector::new(&p.job, &p.pays, &p.bonuses, growth, today).years(last_year);
            for (total, (_, value)) in years.iter_mut().zip(projected) {
                total.1 += value;
            }
        }
        years
    }
}

// ---------------------------------------------------------------- Vues

#[derive(Debug, Deserialize, ToSchema)]
pub struct EmploymentInput {
    pub employer: String,
    pub job_title: Option<String>,
    pub start_date: Date,
    pub end_date: Option<Date>,
    /// Catégorie de revenu comparée au net attendu (R-SAL-9).
    pub income_category_id: CategoryId,
    /// Mois des augmentations, de 1 à 12 (janvier par défaut).
    pub raise_month: Option<u32>,
    #[serde(default)]
    pub paid_next_month: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SalaryInput {
    /// Premier jour du mois d'effet.
    pub effective_month: Date,
    /// Brut annuel de base, hors primes.
    pub annual_gross_minor: i64,
    /// Net mensuel versé, après prélèvement à la source.
    pub monthly_net_minor: i64,
    pub reason: SalaryChangeReason,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BonusInput {
    pub label: String,
    pub kind: BonusKind,
    pub paid_on: Date,
    pub net_amount_minor: i64,
    pub gross_amount_minor: Option<i64>,
    /// Oui par défaut pour un 13e mois, non sinon (R-SAL-5).
    pub recurring: Option<bool>,
    pub note: Option<String>,
}

/// Un emploi dans la liste (E29).
#[derive(Debug, Serialize, ToSchema)]
pub struct EmploymentSummary {
    pub id: EmploymentId,
    pub employer: String,
    pub job_title: Option<String>,
    pub start_date: Date,
    pub end_date: Option<Date>,
    pub income_category_id: CategoryId,
    /// Net mensuel en vigueur ce mois-ci (R-SAL-3).
    pub current_net_minor: Option<i64>,
    /// L'emploi est terminé.
    pub ended: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SalaryView {
    pub id: SalaryId,
    pub effective_month: Date,
    pub annual_gross_minor: i64,
    pub monthly_net_minor: i64,
    pub reason: SalaryChangeReason,
    pub note: Option<String>,
    /// Mois d'effet futur (R-SAL-4).
    pub planned: bool,
    /// Variation du net sur la rémunération précédente, en millionièmes.
    pub net_change_ppm: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BonusView {
    pub id: SalaryBonusId,
    pub label: String,
    pub kind: BonusKind,
    pub paid_on: Date,
    pub net_amount_minor: i64,
    pub gross_amount_minor: Option<i64>,
    pub recurring: bool,
    pub note: Option<String>,
    pub planned: bool,
}

/// Revenu annuel (R-SAL-7, R-SAL-8).
#[derive(Debug, Serialize, ToSchema)]
pub struct AnnualView {
    pub year: i32,
    pub net_minor: i64,
    pub gross_minor: i64,
    pub months: u32,
    pub incomplete: bool,
    pub gross_incomplete: bool,
    pub provisional: bool,
    pub net_change_ppm: Option<i64>,
    pub gross_change_ppm: Option<i64>,
}

/// Attendu et reçu d'un mois (R-SAL-9).
#[derive(Debug, Serialize, ToSchema)]
pub struct MonthCheck {
    /// Premier jour du mois.
    pub month: Date,
    pub expected_minor: i64,
    pub received_minor: i64,
    pub gap_minor: i64,
    /// Écart au-delà de la tolérance, sur un mois complet et renseigné.
    pub flagged: bool,
    pub partial: bool,
    pub unknown: bool,
    pub in_progress: bool,
}

/// Fiche d'un emploi (E30).
#[derive(Debug, Serialize, ToSchema)]
pub struct EmploymentView {
    pub summary: EmploymentSummary,
    pub raise_month: u32,
    pub paid_next_month: bool,
    pub notes: Option<String>,
    pub income_category_name: String,
    /// Rémunérations, la plus récente d'abord.
    pub salaries: Vec<SalaryView>,
    /// Primes, la plus récente d'abord.
    pub bonuses: Vec<BonusView>,
    /// Revenu annuel, la plus récente année d'abord.
    pub years: Vec<AnnualView>,
    /// Les 12 derniers mois, mois en cours compris, le plus récent d'abord. L'attendu cumule les emplois de la
    /// même catégorie.
    pub checks: Vec<MonthCheck>,
}

/// Net d'un emploi pour une année de l'évolution (E29).
#[derive(Debug, Serialize, ToSchema)]
pub struct EmploymentYear {
    pub employment_id: EmploymentId,
    pub net_minor: i64,
}

/// Une année, tous emplois (R-SAL-8).
#[derive(Debug, Serialize, ToSchema)]
pub struct YearTotal {
    pub year: i32,
    pub net_minor: i64,
    pub gross_minor: i64,
    pub incomplete: bool,
    pub gross_incomplete: bool,
    pub provisional: bool,
    pub net_change_ppm: Option<i64>,
    pub gross_change_ppm: Option<i64>,
    pub employments: Vec<EmploymentYear>,
}

/// Rubrique Salaires (E29).
#[derive(Debug, Serialize, ToSchema)]
pub struct SalariesOverview {
    pub employments: Vec<EmploymentSummary>,
    /// Années, de la plus ancienne à l'année en cours (et aux années de rémunérations prévues).
    pub years: Vec<YearTotal>,
}

fn summary(e: &EmploymentRow, book: &SalaryBook, today: Date) -> EmploymentSummary {
    let pays: Vec<Pay> = book.salaries(e.id).iter().map(pay).collect();
    let current = salaries::month_of(today);
    EmploymentSummary {
        id: e.id,
        employer: e.employer.clone(),
        job_title: e.job_title.clone(),
        start_date: e.start_date,
        end_date: e.end_date,
        income_category_id: e.income_category_id,
        current_net_minor: job(e)
            .covers(current)
            .then(|| salaries::in_force(&pays, current).map(|p| p.monthly_net.minor()))
            .flatten(),
        ended: e.end_date.is_some_and(|end| end < today),
    }
}

/// Années touchées par un emploi : de son début à la dernière date connue (fin, rémunération ou prime prévues).
fn years_of(p: &Payroll, today: Date) -> std::ops::RangeInclusive<i32> {
    let last = [
        Some(today.year()),
        p.pays.iter().map(|x| x.effective_month.year()).max(),
        p.bonuses.iter().map(|b| b.paid_on.year()).max(),
    ]
    .into_iter()
    .flatten()
    .max()
    .unwrap_or(today.year());
    let last = p.job.end_date.map_or(last, |end| end.year().min(last));
    p.job.start_date.year()..=last
}

fn ppm(rate: Option<Rate>) -> Option<i64> {
    rate.map(Rate::ppm)
}

/// Revenu annuel d'un emploi, année par année, avec la variation sur l'année précédente (R-SAL-7, R-SAL-8).
fn annual_views(p: &Payroll, today: Date) -> Vec<AnnualView> {
    let years: Vec<salaries::AnnualIncome> =
        years_of(p, today).filter_map(|year| salaries::annual(&p.job, &p.pays, &p.bonuses, year, today)).collect();
    years
        .iter()
        .enumerate()
        .map(|(i, y)| {
            let previous = i.checked_sub(1).and_then(|j| years.get(j)).filter(|prev| prev.year == y.year - 1);
            let comparable = previous.filter(|prev| prev.comparable() && y.comparable());
            AnnualView {
                year: y.year,
                net_minor: y.net.minor(),
                gross_minor: y.gross.minor(),
                months: y.months,
                incomplete: y.incomplete,
                gross_incomplete: y.gross_incomplete,
                provisional: y.provisional,
                net_change_ppm: ppm(comparable.and_then(|prev| salaries::change(prev.net, y.net))),
                gross_change_ppm: ppm(comparable
                    .filter(|prev| !prev.gross_incomplete && !y.gross_incomplete)
                    .and_then(|prev| salaries::change(prev.gross, y.gross))),
            }
        })
        .collect()
}

/// Tolérance de l'écart entre attendu et reçu (R-SAL-9), 5 % par défaut.
pub async fn tolerance(db: &Db) -> AppResult<Rate> {
    Ok(Rate::from_ppm(db::settings::get::<i64>(&db.read, TOLERANCE).await?.unwrap_or(DEFAULT_TOLERANCE_PPM)))
}

pub const TOLERANCE: &str = "salaries.gap_tolerance";
pub const DEFAULT_TOLERANCE_PPM: i64 = 50_000;

fn month_back(month: Date, count: u32) -> Date {
    month.checked_sub_months(Months::new(count)).unwrap_or(month)
}

/// Attendu et reçu d'une catégorie de revenu sur les `count` derniers mois, mois en cours compris, le plus récent
/// d'abord (R-SAL-9). L'attendu cumule les emplois reliés à la catégorie ; le reçu compte ses sous-catégories.
async fn checks(
    db: &Db,
    book: &SalaryBook,
    category: CategoryId,
    count: u32,
    today: Date,
) -> AppResult<Vec<MonthCheck>> {
    let current = salaries::month_of(today);
    let first = month_back(current, count.saturating_sub(1));
    let mut received: HashMap<String, Money> = HashMap::new();
    for r in db::salaries::received(&db.read, first, cashflow_domain::recurrence::end_of_month(current)).await? {
        if r.category_id == category || r.parent_id == Some(category) {
            *received.entry(r.month).or_default() += r.amount;
        }
    }
    let payrolls: Vec<Payroll> =
        book.employments.iter().filter(|e| e.income_category_id == category).map(|e| book.payroll(e)).collect();
    let tolerance = tolerance(db).await?;
    Ok((0..count)
        .map(|back| {
            let month = month_back(current, back);
            let expected = payrolls
                .iter()
                .map(|p| salaries::expected(&p.job, &p.pays, &p.bonuses, month))
                .fold(Expected::default(), Expected::merge);
            let got = received.get(&month.format("%Y-%m").to_string()).copied().unwrap_or(Money::ZERO);
            let in_progress = month == current;
            MonthCheck {
                month,
                expected_minor: expected.amount.minor(),
                received_minor: got.minor(),
                gap_minor: (got - expected.amount).minor(),
                flagged: expected.active
                    && !in_progress
                    && !expected.partial
                    && !expected.unknown
                    && salaries::gap_exceeds(expected.amount, got, tolerance),
                partial: expected.partial,
                unknown: expected.unknown,
                in_progress,
            }
        })
        .filter(|c| c.expected_minor != 0 || c.received_minor != 0 || c.unknown)
        .collect())
}

/// Écart signalé du dernier mois complet, pour les alertes de l'accueil (R-SAL-9).
#[derive(Debug, Serialize, ToSchema)]
pub struct SalaryGapAlert {
    pub employment_id: EmploymentId,
    pub employer: String,
    pub month: Date,
    pub expected_minor: i64,
    pub received_minor: i64,
}

pub async fn alerts(db: &Db, today: Date) -> AppResult<Vec<SalaryGapAlert>> {
    let book = SalaryBook::load(db).await?;
    let last = month_back(salaries::month_of(today), 1);
    let mut seen: Vec<CategoryId> = Vec::new();
    let mut result = Vec::new();
    for e in &book.employments {
        if seen.contains(&e.income_category_id) {
            continue;
        }
        seen.push(e.income_category_id);
        let checks = checks(db, &book, e.income_category_id, 2, today).await?;
        if let Some(c) = checks.into_iter().find(|c| c.month == last && c.flagged) {
            result.push(SalaryGapAlert {
                employment_id: e.id,
                employer: e.employer.clone(),
                month: c.month,
                expected_minor: c.expected_minor,
                received_minor: c.received_minor,
            });
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------- Lecture

pub async fn overview(db: &Db, today: Date) -> AppResult<SalariesOverview> {
    let book = SalaryBook::load(db).await?;
    let mut by_year: std::collections::BTreeMap<i32, Vec<(EmploymentId, salaries::AnnualIncome)>> =
        std::collections::BTreeMap::new();
    for e in &book.employments {
        let p = book.payroll(e);
        for year in years_of(&p, today) {
            if let Some(income) = salaries::annual(&p.job, &p.pays, &p.bonuses, year, today) {
                by_year.entry(year).or_default().push((e.id, income));
            }
        }
    }
    let mut years: Vec<YearTotal> = Vec::new();
    let mut previous: Option<(i32, Money, Money, bool, bool)> = None;
    for (year, incomes) in by_year {
        let net: Money = incomes.iter().map(|(_, i)| i.net).sum();
        let gross: Money = incomes.iter().map(|(_, i)| i.gross).sum();
        let incomplete = incomes.iter().any(|(_, i)| i.incomplete);
        let gross_incomplete = incomes.iter().any(|(_, i)| i.gross_incomplete);
        // Tous emplois : une année se compare si chaque emploi la couvre en entier ou n'y figure pas.
        let full = incomes.iter().all(|(_, i)| i.comparable());
        let comparable = previous.filter(|p| p.0 == year - 1 && p.3 && full && !incomplete);
        years.push(YearTotal {
            year,
            net_minor: net.minor(),
            gross_minor: gross.minor(),
            incomplete,
            gross_incomplete,
            provisional: year >= today.year(),
            net_change_ppm: ppm(comparable.and_then(|p| salaries::change(p.1, net))),
            gross_change_ppm: ppm(comparable
                .filter(|p| !p.4 && !gross_incomplete)
                .and_then(|p| salaries::change(p.2, gross))),
            employments: incomes
                .iter()
                .map(|(id, i)| EmploymentYear { employment_id: *id, net_minor: i.net.minor() })
                .collect(),
        });
        previous = Some((year, net, gross, full && !incomplete, gross_incomplete));
    }
    Ok(SalariesOverview { employments: book.employments.iter().map(|e| summary(e, &book, today)).collect(), years })
}

pub async fn get(db: &Db, id: EmploymentId, today: Date) -> AppResult<EmploymentView> {
    let book = SalaryBook::load(db).await?;
    let e = book.employments.iter().find(|e| e.id == id).ok_or(AppError::NotFound("emploi"))?;
    let p = book.payroll(e);
    let category = db::categories::find(&db.read, e.income_category_id).await?;
    let current = salaries::month_of(today);
    let rows = book.salaries(id);
    let salaries = rows
        .iter()
        .enumerate()
        .map(|(i, s)| SalaryView {
            id: s.id,
            effective_month: s.effective_month,
            annual_gross_minor: s.annual_gross.minor(),
            monthly_net_minor: s.monthly_net.minor(),
            reason: s.reason,
            note: s.note.clone(),
            planned: s.effective_month > current,
            net_change_ppm: ppm(i
                .checked_sub(1)
                .and_then(|j| rows.get(j))
                .and_then(|prev| salaries::change(prev.monthly_net, s.monthly_net))),
        })
        .rev()
        .collect();
    let bonuses = book
        .bonuses(id)
        .iter()
        .rev()
        .map(|b| BonusView {
            id: b.id,
            label: b.label.clone(),
            kind: b.kind,
            paid_on: b.paid_on,
            net_amount_minor: b.net_amount.minor(),
            gross_amount_minor: b.gross_amount.map(Money::minor),
            recurring: b.recurring,
            note: b.note.clone(),
            planned: b.paid_on > today,
        })
        .collect();
    let mut years = annual_views(&p, today);
    years.reverse();
    Ok(EmploymentView {
        summary: summary(e, &book, today),
        raise_month: e.raise_month,
        paid_next_month: e.paid_next_month,
        notes: e.notes.clone(),
        income_category_name: category.map(|c| c.name).unwrap_or_default(),
        salaries,
        bonuses,
        years,
        checks: checks(db, &book, e.income_category_id, CHECKED_MONTHS, today).await?,
    })
}

// ---------------------------------------------------------------- Écriture

async fn row_of(db: &Db, id: EmploymentId, input: &EmploymentInput) -> AppResult<EmploymentRow> {
    let category =
        db::categories::find(&db.read, input.income_category_id).await?.ok_or(AppError::NotFound("catégorie"))?;
    if category.kind != CategoryKind::Income {
        return Err(AppError::invalid_field(
            "NOT_INCOME_CATEGORY",
            "income_category_id",
            "le salaire se reçoit sur une catégorie de revenu",
        ));
    }
    let row = EmploymentRow {
        id,
        employer: super::required(&input.employer, "employer", 80)?,
        job_title: clean(input.job_title.as_deref()),
        start_date: input.start_date,
        end_date: input.end_date,
        income_category_id: input.income_category_id,
        raise_month: input.raise_month.unwrap_or(1),
        paid_next_month: input.paid_next_month,
        notes: clean(input.notes.as_deref()),
    };
    job(&row).validate()?;
    Ok(row)
}

pub async fn create(db: &Db, input: &EmploymentInput, today: Date) -> AppResult<EmploymentView> {
    let id = EmploymentId::new();
    db::salaries::upsert(&db.write, &row_of(db, id, input).await?).await?;
    get(db, id, today).await
}

/// Modifie un emploi ; ses rémunérations et ses primes doivent rester dans ses nouvelles dates.
pub async fn update(db: &Db, id: EmploymentId, input: &EmploymentInput, today: Date) -> AppResult<EmploymentView> {
    let book = SalaryBook::load(db).await?;
    if !book.employments.iter().any(|e| e.id == id) {
        return Err(AppError::NotFound("emploi"));
    }
    let row = row_of(db, id, input).await?;
    let new = job(&row);
    if book.salaries(id).iter().any(|s| new.check_pay(&pay(s)).is_err())
        || book.bonuses(id).iter().any(|b| new.check_bonus(&bonus(b)).is_err())
    {
        return Err(AppError::invalid(
            "EMPLOYMENT_PERIOD_EXCLUDES_PAY",
            "des rémunérations ou des primes tombent hors de ces dates : les modifier d'abord",
        ));
    }
    db::salaries::upsert(&db.write, &row).await?;
    get(db, id, today).await
}

/// Supprime un emploi, ses rémunérations et ses primes (R-SAL-6).
pub async fn delete(db: &Db, id: EmploymentId) -> AppResult<()> {
    if db::salaries::delete(&db.write, id).await? == 0 {
        return Err(AppError::NotFound("emploi"));
    }
    Ok(())
}

async fn employment(db: &Db, id: EmploymentId) -> AppResult<EmploymentRow> {
    db::salaries::find(&db.read, id).await?.ok_or(AppError::NotFound("emploi"))
}

/// Ajoute (`salary` absent) ou modifie une rémunération (R-SAL-2).
pub async fn save_salary(
    db: &Db,
    id: EmploymentId,
    salary: Option<SalaryId>,
    input: &SalaryInput,
    today: Date,
) -> AppResult<EmploymentView> {
    let e = employment(db, id).await?;
    let row = SalaryRow {
        id: salary.unwrap_or_else(SalaryId::new),
        employment_id: id,
        effective_month: input.effective_month,
        annual_gross: Money::from_minor(input.annual_gross_minor),
        monthly_net: Money::from_minor(input.monthly_net_minor),
        reason: input.reason,
        note: clean(input.note.as_deref()),
    };
    job(&e).check_pay(&pay(&row))?;
    let existing = db::salaries::salaries(&db.read).await?;
    if let Some(salary) = salary
        && !existing.iter().any(|s| s.id == salary && s.employment_id == id)
    {
        return Err(AppError::NotFound("rémunération"));
    }
    if existing.iter().any(|s| s.employment_id == id && s.id != row.id && s.effective_month == row.effective_month) {
        return Err(AppError::invalid_field(
            "SALARY_MONTH_TAKEN",
            "effective_month",
            "une rémunération prend déjà effet ce mois-là",
        ));
    }
    db::salaries::upsert_salary(&db.write, &row).await?;
    get(db, id, today).await
}

pub async fn delete_salary(db: &Db, id: EmploymentId, salary: SalaryId) -> AppResult<()> {
    if db::salaries::delete_salary(&db.write, id, salary).await? == 0 {
        return Err(AppError::NotFound("rémunération"));
    }
    Ok(())
}

/// Ajoute (`bonus` absent) ou modifie une prime (R-SAL-5).
pub async fn save_bonus(
    db: &Db,
    id: EmploymentId,
    bonus_id: Option<SalaryBonusId>,
    input: &BonusInput,
    today: Date,
) -> AppResult<EmploymentView> {
    let e = employment(db, id).await?;
    if let Some(b) = bonus_id
        && !db::salaries::bonuses(&db.read).await?.iter().any(|x| x.id == b && x.employment_id == id)
    {
        return Err(AppError::NotFound("prime"));
    }
    let row = BonusRow {
        id: bonus_id.unwrap_or_else(SalaryBonusId::new),
        employment_id: id,
        label: super::required(&input.label, "label", 80)?,
        kind: input.kind,
        paid_on: input.paid_on,
        net_amount: Money::from_minor(input.net_amount_minor),
        gross_amount: input.gross_amount_minor.map(Money::from_minor),
        recurring: input.recurring.unwrap_or(input.kind == BonusKind::ThirteenthMonth),
        note: clean(input.note.as_deref()),
    };
    job(&e).check_bonus(&bonus(&row))?;
    db::salaries::upsert_bonus(&db.write, &row).await?;
    get(db, id, today).await
}

pub async fn delete_bonus(db: &Db, id: EmploymentId, bonus: SalaryBonusId) -> AppResult<()> {
    if db::salaries::delete_bonus(&db.write, id, bonus).await? == 0 {
        return Err(AppError::NotFound("prime"));
    }
    Ok(())
}
