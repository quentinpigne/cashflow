//! Épargne (spec 06) : conditions et barèmes (CPT-05, EPA-01), intérêts (EPA-02 à EPA-04), valorisations
//! et performance (CPT-06, EPA-05, EPA-06), synthèse de l'accueil (TDB-05).

use std::collections::HashMap;

use cashflow_domain::accounts::{AccountFamily, Capitalization, InterestMethod};
use cashflow_domain::ledger::{ClearingStatus, TransactionOrigin};
use cashflow_domain::reporting::{self, SplitFact};
use cashflow_domain::savings::{self, FortnightlyTerms, Movement, Point, RatePoint};
use cashflow_domain::shared::ids::{AccountId, CategoryId, RateScheduleId, TransactionId, ValuationId};
use cashflow_domain::{Currency, Date, Money, Rate};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{clean, reconciliation};
use crate::db::accounts::AccountRow;
use crate::db::savings::{MovementRow, PeriodRow, TermsRow};
use crate::db::transactions::{NewSplit, NewTransaction};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------- Conditions et barèmes

#[derive(Debug, Serialize, ToSchema)]
pub struct TermsView {
    pub rate_schedule_id: RateScheduleId,
    pub schedule_name: String,
    pub shared: bool,
    pub interest_method: InterestMethod,
    pub capitalization: Capitalization,
    pub deposit_cap_minor: Option<i64>,
    pub maturity_date: Option<Date>,
    pub interest_category_id: Option<CategoryId>,
    pub round_each_fortnight: bool,
    /// Taux en vigueur aujourd'hui, en millionièmes.
    pub current_rate_ppm: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TermsInput {
    pub rate_schedule_id: RateScheduleId,
    pub interest_method: InterestMethod,
    pub capitalization: Capitalization,
    pub deposit_cap_minor: Option<i64>,
    pub maturity_date: Option<Date>,
    pub interest_category_id: Option<CategoryId>,
    #[serde(default)]
    pub round_each_fortnight: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RatePeriodView {
    pub effective_from: Date,
    pub rate_ppm: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ScheduleView {
    pub id: RateScheduleId,
    pub name: String,
    /// Barème réglementé partagé entre comptes (R-EPA-11).
    pub shared: bool,
    pub periods: Vec<RatePeriodView>,
    /// Comptes qui l'utilisent.
    pub accounts: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RatePeriodInput {
    pub effective_from: Date,
    pub rate_ppm: i64,
}

async fn terms_of(db: &Db, account: AccountId) -> AppResult<TermsRow> {
    db::savings::all_terms(&db.read)
        .await?
        .into_iter()
        .find(|t| t.account_id == account)
        .ok_or_else(|| AppError::invalid("NO_SAVINGS_TERMS", "ce compte n'a pas de conditions d'épargne (R-CPT-9)"))
}

async fn rates_of(db: &Db, schedule: RateScheduleId) -> AppResult<Vec<RatePoint>> {
    Ok(db::savings::periods(&db.read)
        .await?
        .into_iter()
        .filter(|p| p.schedule_id == schedule)
        .map(|p| RatePoint { from: p.effective_from, rate: p.rate })
        .collect())
}

pub async fn terms(db: &Db, account: AccountId, today: Date) -> AppResult<TermsView> {
    let t = terms_of(db, account).await?;
    let rates = rates_of(db, t.rate_schedule_id).await?;
    Ok(TermsView {
        rate_schedule_id: t.rate_schedule_id,
        schedule_name: t.schedule_name,
        shared: t.shared,
        interest_method: t.interest_method,
        capitalization: t.capitalization,
        deposit_cap_minor: t.deposit_cap.map(Money::minor),
        maturity_date: t.maturity_date,
        interest_category_id: t.interest_category_id,
        round_each_fortnight: t.round_each_fortnight,
        current_rate_ppm: savings::rate_at(&rates, today).ppm(),
    })
}

/// Conditions d'un compte à taux déterminé (CPT-05).
pub async fn update_terms(db: &Db, account: AccountId, input: &TermsInput, today: Date) -> AppResult<TermsView> {
    let current = terms_of(db, account).await?;
    if !db::savings::schedules(&db.read).await?.iter().any(|s| s.id == input.rate_schedule_id) {
        return Err(AppError::NotFound("barème"));
    }
    if let Some(category) = input.interest_category_id
        && db::categories::find(&db.read, category).await?.is_none()
    {
        return Err(AppError::NotFound("catégorie"));
    }
    if input.deposit_cap_minor.is_some_and(|c| c <= 0) {
        return Err(AppError::invalid_field("INVALID_CAP", "deposit_cap_minor", "le plafond doit être positif"));
    }
    let row = TermsRow {
        rate_schedule_id: input.rate_schedule_id,
        interest_method: input.interest_method,
        capitalization: input.capitalization,
        deposit_cap: input.deposit_cap_minor.map(Money::from_minor),
        maturity_date: input.maturity_date,
        interest_category_id: input.interest_category_id,
        round_each_fortnight: input.round_each_fortnight,
        ..current
    };
    db::savings::update_terms(&db.write, &row).await?;
    terms(db, account, today).await
}

/// Barèmes et leur historique de taux (EPA-01).
pub async fn schedules(db: &Db) -> AppResult<Vec<ScheduleView>> {
    let periods = db::savings::periods(&db.read).await?;
    let terms = db::savings::all_terms(&db.read).await?;
    let accounts: HashMap<AccountId, String> =
        db::accounts::list(&db.read).await?.into_iter().map(|a| (a.id, a.name)).collect();
    Ok(db::savings::schedules(&db.read)
        .await?
        .into_iter()
        .map(|s| ScheduleView {
            periods: periods
                .iter()
                .filter(|p| p.schedule_id == s.id)
                .map(|p| RatePeriodView { effective_from: p.effective_from, rate_ppm: p.rate.ppm() })
                .collect(),
            accounts: terms
                .iter()
                .filter(|t| t.rate_schedule_id == s.id)
                .filter_map(|t| accounts.get(&t.account_id).cloned())
                .collect(),
            id: s.id,
            name: s.name,
            shared: s.shared,
        })
        .collect())
}

/// Ajoute (ou corrige) un taux daté. Les intérêts étant calculés à la demande, tous les comptes du
/// barème en tiennent compte aussitôt (R-EPA-11).
pub async fn set_rate(db: &Db, schedule: RateScheduleId, input: &RatePeriodInput) -> AppResult<Vec<ScheduleView>> {
    if !(0..=1_000_000).contains(&input.rate_ppm) {
        return Err(AppError::invalid_field("INVALID_RATE", "rate_ppm", "taux entre 0 et 100 %"));
    }
    if !db::savings::schedules(&db.read).await?.iter().any(|s| s.id == schedule) {
        return Err(AppError::NotFound("barème"));
    }
    let period =
        PeriodRow { schedule_id: schedule, effective_from: input.effective_from, rate: Rate::from_ppm(input.rate_ppm) };
    db::savings::set_period(&db.write, &period).await?;
    schedules(db).await
}

pub async fn delete_rate(db: &Db, schedule: RateScheduleId, from: Date) -> AppResult<Vec<ScheduleView>> {
    if db::savings::delete_period(&db.write, schedule, from).await? == 0 {
        return Err(AppError::NotFound("taux"));
    }
    schedules(db).await
}

// ---------------------------------------------------------------- Intérêts

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct YearQuery {
    /// Année civile ; l'année en cours par défaut.
    pub year: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FortnightView {
    pub start: Date,
    pub end: Date,
    pub balance_minor: i64,
    pub rate_ppm: i64,
    /// Intérêt de la quinzaine, en centimes, avec quatre décimales (affichage seulement).
    pub interest: String,
}

/// État de la transaction d'intérêts d'une année (R-EPA-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InterestProposal {
    /// L'année n'est pas finie.
    NotYet,
    /// L'année est finie : la transaction peut être proposée.
    Available,
    /// La transaction d'intérêts existe.
    Recorded,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InterestView {
    pub year: i32,
    /// Méthode prise en charge en V1 (quinzaines) ; les autres arrivent en V2 (EPA-07).
    pub supported: bool,
    pub fortnights: Vec<FortnightView>,
    /// Intérêts courus au jour (quinzaines terminées), ou de l'année entière si elle est finie.
    pub accrued_minor: i64,
    /// Intérêts attendus au 31/12 si aucun mouvement n'intervient d'ici là (EPA-02).
    pub expected_minor: i64,
    /// Intérêts effectivement inscrits pour l'année (transactions d'intérêts autour du 31/12).
    pub booked_minor: Option<i64>,
    pub proposal: InterestProposal,
    pub interest_transaction_id: Option<TransactionId>,
    /// Capital versé (hors intérêts capitalisés) et plafond (R-EPA-9).
    pub capital_minor: i64,
    pub deposit_cap_minor: Option<i64>,
    /// Premier versement qui fait dépasser le plafond, s'il y en a un.
    pub cap_exceeded_on: Option<Date>,
}

/// Date de capitalisation de l'année : le 31 décembre, ou la clôture si elle intervient avant (R-EPA-5).
fn capitalization_date(year: i32, closed: Option<Date>) -> Date {
    let end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or_default();
    closed.filter(|c| c.year() == year).map_or(end, |c| c.min(end))
}

fn is_interest(m: &MovementRow) -> bool {
    m.origin == TransactionOrigin::Interest || m.is_interest
}

struct Computed {
    account: AccountRow,
    terms: TermsRow,
    movements: Vec<MovementRow>,
    year: savings::YearInterest,
}

async fn compute(db: &Db, account: AccountId, year: i32) -> AppResult<Computed> {
    let row = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    let terms = terms_of(db, account).await?;
    let rates = rates_of(db, terms.rate_schedule_id).await?;
    let movements = db::savings::movements(&db.read, account, terms.interest_category_id).await?;
    let mut inputs: Vec<Movement> = vec![Movement { date: row.opening_date, amount: row.opening_balance }];
    inputs.extend(movements.iter().map(|m| Movement { date: m.interest_date, amount: m.amount }));
    let fortnightly =
        FortnightlyTerms { opened: row.opening_date, closed: row.closed_on, round_each: terms.round_each_fortnight };
    let year = savings::fortnightly_interest(&inputs, &rates, year, fortnightly);
    Ok(Computed { account: row, terms, movements, year })
}

/// Intérêts d'une année : détail par quinzaine, courus, attendus, versés, plafond (EPA-02 à EPA-04).
pub async fn interest(db: &Db, account: AccountId, query: &YearQuery, today: Date) -> AppResult<InterestView> {
    let year = query.year.unwrap_or(today.year());
    let c = compute(db, account, year).await?;
    let supported = c.terms.interest_method == InterestMethod::Fortnightly;
    let capitalization = capitalization_date(year, c.account.closed_on);
    // Intérêts inscrits : du jour de capitalisation à la fin janvier suivante (versement bancaire).
    let window_end = NaiveDate::from_ymd_opt(year + 1, 1, 31).unwrap_or(capitalization);
    let booked: Vec<&MovementRow> = c
        .movements
        .iter()
        .filter(|m| is_interest(m) && m.balance_date >= capitalization && m.balance_date <= window_end)
        .collect();
    let recorded = c.movements.iter().find(|m| {
        m.origin == TransactionOrigin::Interest && m.balance_date >= capitalization && m.balance_date <= window_end
    });
    let proposal = match (recorded, today > capitalization || c.account.closed_on == Some(capitalization)) {
        (Some(_), _) => InterestProposal::Recorded,
        (None, true) => InterestProposal::Available,
        (None, false) => InterestProposal::NotYet,
    };
    let deposits: Vec<Movement> =
        std::iter::once(Movement { date: c.account.opening_date, amount: c.account.opening_balance })
            .chain(
                c.movements
                    .iter()
                    .filter(|m| !is_interest(m))
                    .map(|m| Movement { date: m.balance_date, amount: m.amount }),
            )
            .collect();
    let capital: Money = deposits.iter().map(|m| m.amount).sum();
    Ok(InterestView {
        year,
        supported,
        fortnights: if supported {
            c.year
                .fortnights
                .iter()
                .map(|f| FortnightView {
                    start: f.start,
                    end: f.end,
                    balance_minor: f.balance.minor(),
                    rate_ppm: f.rate.ppm(),
                    interest: f.interest.round_dp(4).to_string(),
                })
                .collect()
        } else {
            Vec::new()
        },
        accrued_minor: if supported { c.year.accrued(today).minor() } else { 0 },
        expected_minor: if supported { c.year.total().minor() } else { 0 },
        booked_minor: (!booked.is_empty()).then(|| booked.iter().map(|m| m.amount.minor()).sum()),
        proposal,
        interest_transaction_id: recorded.map(|m| m.id),
        capital_minor: capital.minor(),
        deposit_cap_minor: c.terms.deposit_cap.map(Money::minor),
        cap_exceeded_on: c.terms.deposit_cap.and_then(|cap| savings::cap_exceeded(&deposits, cap)).map(|(d, _)| d),
    })
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InterestRecorded {
    pub transaction_id: TransactionId,
    pub amount_minor: i64,
}

/// Inscrit les intérêts calculés d'une année finie : transaction `PENDING` d'origine `INTEREST`, que
/// l'import du relevé pointera (R-EPA-10).
pub async fn record_interest(db: &Db, account: AccountId, year: i32, today: Date) -> AppResult<InterestRecorded> {
    let view = interest(db, account, &YearQuery { year: Some(year) }, today).await?;
    match view.proposal {
        InterestProposal::Recorded => {
            return Err(AppError::invalid(
                "INTEREST_ALREADY_RECORDED",
                "les intérêts de cette année sont déjà inscrits",
            ));
        }
        InterestProposal::NotYet => {
            return Err(AppError::invalid(
                "YEAR_NOT_OVER",
                "les intérêts s'inscrivent une fois l'année finie (R-EPA-5)",
            ));
        }
        InterestProposal::Available => {}
    }
    if !view.supported || view.expected_minor <= 0 {
        return Err(AppError::invalid("NO_INTEREST", "aucun intérêt calculé pour cette année"));
    }
    let c = compute(db, account, year).await?;
    let date = capitalization_date(year, c.account.closed_on);
    let amount = Money::from_minor(view.expected_minor);
    let label = format!("Intérêts {year}");
    let mut tx = db.write.begin().await?;
    reconciliation::lock(&mut tx, account).await?.check(date)?;
    let id = TransactionId::new();
    let new = NewTransaction {
        id,
        account_id: account,
        date,
        bank_date: None,
        value_date: None,
        amount,
        bank_label: None,
        label: &label,
        note: Some("Calculé par Cashflow (spec 06)"),
        clearing_status: ClearingStatus::Pending,
        origin: TransactionOrigin::Interest,
        recurring_rule_id: None,
        fingerprint: None,
        payee_id: None,
    };
    let split = NewSplit {
        amount,
        category_id: c.terms.interest_category_id,
        note: None,
        project_id: None,
        ..Default::default()
    };
    super::transactions::insert_validated(&mut tx, &new, &[split]).await?;
    tx.commit().await?;
    Ok(InterestRecorded { transaction_id: id, amount_minor: amount.minor() })
}

// ---------------------------------------------------------------- Valorisations

#[derive(Debug, Serialize, ToSchema)]
pub struct ValuationView {
    pub id: ValuationId,
    pub date: Date,
    pub value_minor: i64,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ValuationInput {
    pub date: Date,
    /// Valeur du compte ; pour un prêt, le capital restant dû en négatif (R-CPT-12).
    pub value_minor: i64,
    pub note: Option<String>,
    /// Remplacer la valorisation de la même date, après confirmation (CPT-06).
    #[serde(default)]
    pub replace: bool,
}

async fn valued_account(db: &Db, account: AccountId) -> AppResult<AccountRow> {
    let row = db::accounts::find(&db.read, account).await?.ok_or(AppError::NotFound("compte"))?;
    if row.kind.family().has_computed_balance() {
        return Err(AppError::invalid(
            "NO_VALUATION_FOR_FAMILY",
            "ce compte a un solde calculé : il ne se suit pas par valorisations (R-CPT-6)",
        ));
    }
    Ok(row)
}

pub async fn valuations(db: &Db, account: AccountId) -> AppResult<Vec<ValuationView>> {
    valued_account(db, account).await?;
    Ok(db::savings::valuations(&db.read, account)
        .await?
        .into_iter()
        .rev()
        .map(|v| ValuationView { id: v.id, date: v.date, value_minor: v.value.minor(), note: v.note })
        .collect())
}

pub async fn save_valuation(db: &Db, account: AccountId, input: &ValuationInput) -> AppResult<Vec<ValuationView>> {
    let row = valued_account(db, account).await?;
    super::transactions::period(&row).check(input.date)?;
    if input.value_minor < 0 {
        return Err(AppError::invalid_field("NEGATIVE_VALUE", "value_minor", "une valorisation est positive"));
    }
    let exists = db::savings::valuations(&db.read, account).await?.iter().any(|v| v.date == input.date);
    if exists && !input.replace {
        return Err(AppError::invalid_field(
            "VALUATION_EXISTS",
            "date",
            "une valorisation existe déjà à cette date : confirmer pour la remplacer",
        ));
    }
    let note = clean(input.note.as_deref());
    db::savings::set_valuation(&db.write, account, input.date, Money::from_minor(input.value_minor), note.as_deref())
        .await?;
    valuations(db, account).await
}

pub async fn delete_valuation(db: &Db, account: AccountId, id: ValuationId) -> AppResult<()> {
    if db::savings::delete_valuation(&db.write, account, id).await? == 0 {
        return Err(AppError::NotFound("valorisation"));
    }
    Ok(())
}

// ---------------------------------------------------------------- Performance

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PeriodQuery {
    /// Début de la période choisie ; l'ouverture par défaut.
    pub from: Option<Date>,
    /// Fin de la période choisie ; aujourd'hui par défaut.
    pub to: Option<Date>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct YearPerformanceView {
    pub year: i32,
    /// Absent : données insuffisantes (pas de valorisation à ±15 jours du 31/12, R-EPA-24).
    pub gain_minor: Option<i64>,
    pub irr_ppm: Option<i64>,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PerformanceView {
    pub value_minor: i64,
    pub value_date: Option<Date>,
    /// La dernière valorisation a plus de 90 jours (R-CPT-6).
    pub estimated: bool,
    /// Capital investi (R-EPA-25).
    pub invested_minor: i64,
    pub gain_minor: i64,
    /// Plus-value rapportée au capital investi.
    pub gain_ppm: Option<i64>,
    pub irr_since_opening_ppm: Option<i64>,
    pub irr_year_to_date_ppm: Option<i64>,
    pub twr_since_opening_ppm: Option<i64>,
    /// Période choisie.
    pub from: Date,
    pub to: Date,
    pub period_gain_minor: i64,
    pub period_irr_ppm: Option<i64>,
    pub period_twr_ppm: Option<i64>,
    pub years: Vec<YearPerformanceView>,
}

/// Seuil d'ancienneté d'une valorisation au-delà duquel la valeur est « estimée » (R-CPT-6).
const STALE_DAYS: i64 = 90;

struct Variable {
    opening: Point,
    points: Vec<Point>,
    flows: Vec<Movement>,
    movements: Vec<cashflow_domain::ledger::balance::DatedAmount>,
    valuations: Vec<cashflow_domain::ledger::balance::Valuation>,
}

impl Variable {
    async fn load(db: &Db, row: &AccountRow) -> AppResult<Self> {
        let valuations = db::savings::valuations(&db.read, row.id).await?;
        let movements = db::savings::movements(&db.read, row.id, None).await?;
        Ok(Variable {
            opening: Point { date: row.opening_date, value: row.opening_balance },
            points: valuations.iter().map(|v| Point { date: v.date, value: v.value }).collect(),
            flows: movements.iter().map(|m| Movement { date: m.balance_date, amount: m.amount }).collect(),
            movements: movements
                .iter()
                .map(|m| cashflow_domain::ledger::balance::DatedAmount {
                    balance_date: m.balance_date,
                    amount: m.amount,
                })
                .collect(),
            valuations: valuations
                .iter()
                .map(|v| cashflow_domain::ledger::balance::Valuation { date: v.date, value: v.value })
                .collect(),
        })
    }

    /// Valeur à une date (R-CPT-6) : la dernière valorisation, plus les flux depuis.
    fn point(&self, date: Date) -> Point {
        let (value, _) =
            cashflow_domain::ledger::balance::value_at(self.opening.value, &self.valuations, &self.movements, date);
        Point { date, value }
    }

    fn points_from_opening(&self) -> Vec<Point> {
        std::iter::once(self.opening).chain(self.points.iter().copied()).collect()
    }
}

fn ratio_ppm(numerator: Money, denominator: Money) -> Option<i64> {
    (denominator.is_positive())
        .then(|| i64::try_from(i128::from(numerator.minor()) * 1_000_000 / i128::from(denominator.minor())).ok())
        .flatten()
}

/// Valeur, capital investi, plus-value, TRI et TWR d'un compte à rendement variable (EPA-05, EPA-06).
pub async fn performance(db: &Db, account: AccountId, query: &PeriodQuery, today: Date) -> AppResult<PerformanceView> {
    let row = valued_account(db, account).await?;
    if row.kind.family() != AccountFamily::VariableSavings {
        return Err(AppError::invalid(
            "NOT_VARIABLE_SAVINGS",
            "la performance concerne l'épargne à rendement variable",
        ));
    }
    let v = Variable::load(db, &row).await?;
    let end_date = row.closed_on.map_or(today, |c| c.min(today));
    let current = v.point(end_date);
    let last_valuation = v.points.iter().filter(|p| p.date <= end_date).map(|p| p.date).max();
    let invested =
        savings::invested(v.opening.value, &v.flows.iter().filter(|f| f.date <= end_date).copied().collect::<Vec<_>>());
    let gain = current.value - invested;
    // Les taux s'arrêtent à la dernière valeur constatée, sauf si des flux l'ont suivie (R-EPA-22).
    let measured = match last_valuation {
        Some(date) if !v.flows.iter().any(|f| f.date > date && f.date <= end_date) => v.point(date),
        _ => current,
    };

    // Depuis le 1er janvier : départ à la valorisation la plus proche du 31/12 précédent, ou à l'ouverture.
    let year_start = NaiveDate::from_ymd_opt(today.year() - 1, 12, 31).unwrap_or(today);
    let ytd_start =
        if row.opening_date > year_start { Some(v.opening) } else { savings::nearest(&v.points, year_start) };

    let from = query.from.unwrap_or(row.opening_date).max(row.opening_date);
    let to = query.to.unwrap_or(end_date).min(end_date);
    let start = if from == row.opening_date { v.opening } else { v.point(from) };
    let finish = v.point(to);
    let mut period_points: Vec<Point> = v.points.iter().copied().filter(|p| p.date > from && p.date < to).collect();
    period_points.insert(0, start);
    period_points.push(finish);

    let years = savings::yearly(v.opening, &v.points, &v.flows, today)
        .into_iter()
        .map(|(year, p)| YearPerformanceView {
            year,
            gain_minor: p.map(|p| p.gain.minor()),
            irr_ppm: p.and_then(|p| p.irr).map(Rate::ppm),
            from: p.map(|p| p.start.date),
            to: p.map(|p| p.end.date),
        })
        .collect();

    Ok(PerformanceView {
        value_minor: current.value.minor(),
        value_date: last_valuation,
        estimated: last_valuation.is_none_or(|d| (end_date - d).num_days() > STALE_DAYS),
        invested_minor: invested.minor(),
        gain_minor: gain.minor(),
        gain_ppm: ratio_ppm(gain, invested),
        irr_since_opening_ppm: savings::irr(v.opening, &v.flows, measured).map(Rate::ppm),
        irr_year_to_date_ppm: ytd_start.and_then(|s| savings::irr(s, &v.flows, measured)).map(Rate::ppm),
        twr_since_opening_ppm: {
            let mut points = v.points_from_opening();
            points.retain(|p| p.date < measured.date);
            points.push(measured);
            savings::twr(&points, &v.flows).map(Rate::ppm)
        },
        from,
        to,
        period_gain_minor: savings::gain(start, finish, &v.flows).minor(),
        period_irr_ppm: savings::irr(start, &v.flows, finish).map(Rate::ppm),
        period_twr_ppm: savings::twr(&period_points, &v.flows).map(Rate::ppm),
        years,
    })
}

// ---------------------------------------------------------------- Synthèse (TDB-05)

#[derive(Debug, Serialize, ToSchema)]
pub struct FixedSummary {
    pub account_id: AccountId,
    pub name: String,
    pub balance_minor: i64,
    pub rate_ppm: i64,
    pub accrued_minor: i64,
    pub expected_minor: i64,
    /// Intérêts de l'an dernier à inscrire (R-EPA-10).
    pub interest_to_record: bool,
    pub cap_exceeded: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VariableSummary {
    pub account_id: AccountId,
    pub name: String,
    pub value_minor: i64,
    pub estimated: bool,
    pub invested_minor: i64,
    pub gain_minor: i64,
    pub irr_year_to_date_ppm: Option<i64>,
    pub irr_since_opening_ppm: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SavingsOverview {
    /// Σ des intérêts courus depuis le 1er janvier (R-TDB-11).
    pub interest_accrued_minor: i64,
    pub interest_expected_minor: i64,
    pub fixed: Vec<FixedSummary>,
    pub variable: Vec<VariableSummary>,
    /// Valeur et capital investi cumulés des comptes à rendement variable.
    pub variable_value_minor: i64,
    pub variable_invested_minor: i64,
    /// TRI global depuis l'ouverture des comptes variables (R-TDB-12).
    pub variable_irr_ppm: Option<i64>,
    /// Épargne de précaution (R-TDB-9) : Σ des comptes concernés, dépenses mensuelles moyennes et nombre
    /// de mois couverts (en dixièmes).
    pub emergency_fund_minor: i64,
    pub average_expenses_minor: i64,
    pub emergency_months_tenths: Option<i64>,
}

/// Nombre de mois complets des dépenses moyennes (R-TDB-8).
const AVERAGE_MONTHS: u32 = 12;

pub async fn overview(db: &Db, today: Date) -> AppResult<SavingsOverview> {
    let accounts = super::accounts::list(db, today).await?;
    let mut fixed = Vec::new();
    let mut variable = Vec::new();
    let mut global_flows: Vec<Movement> = Vec::new();
    let terms: HashMap<AccountId, TermsRow> =
        db::savings::all_terms(&db.read).await?.into_iter().map(|t| (t.account_id, t)).collect();
    for account in accounts.iter().filter(|a| a.closed_on.is_none()) {
        match account.family {
            AccountFamily::FixedSavings
                if terms.get(&account.id).is_some_and(|t| t.interest_method == InterestMethod::Fortnightly) =>
            {
                let current = interest(db, account.id, &YearQuery { year: None }, today).await?;
                let last = interest(db, account.id, &YearQuery { year: Some(today.year() - 1) }, today).await?;
                let rate = terms.get(&account.id).map(|t| t.rate_schedule_id);
                let rate_ppm = match rate {
                    Some(schedule) => savings::rate_at(&rates_of(db, schedule).await?, today).ppm(),
                    None => 0,
                };
                fixed.push(FixedSummary {
                    account_id: account.id,
                    name: account.name.clone(),
                    balance_minor: account.balance_minor,
                    rate_ppm,
                    accrued_minor: current.accrued_minor,
                    expected_minor: current.expected_minor,
                    interest_to_record: last.proposal == InterestProposal::Available && last.expected_minor > 0,
                    cap_exceeded: current.cap_exceeded_on.is_some(),
                });
            }
            AccountFamily::VariableSavings => {
                let p = performance(db, account.id, &PeriodQuery { from: None, to: None }, today).await?;
                // TRI global : chaque ouverture est un apport, la valeur de départ est nulle.
                if let Some(row) = db::accounts::find(&db.read, account.id).await? {
                    let v = Variable::load(db, &row).await?;
                    global_flows.push(Movement { date: v.opening.date, amount: v.opening.value });
                    global_flows.extend(v.flows.iter().copied());
                }
                variable.push(VariableSummary {
                    account_id: account.id,
                    name: account.name.clone(),
                    value_minor: p.value_minor,
                    estimated: p.estimated,
                    invested_minor: p.invested_minor,
                    gain_minor: p.gain_minor,
                    irr_year_to_date_ppm: p.irr_year_to_date_ppm,
                    irr_since_opening_ppm: p.irr_since_opening_ppm,
                });
            }
            _ => {}
        }
    }

    // Dépenses mensuelles moyennes des 12 derniers mois complets (R-TDB-8).
    let last_complete = reporting::first_of_month(today).pred_opt().unwrap_or(today);
    let months = reporting::months_until(last_complete, AVERAGE_MONTHS);
    let first = months.first().map_or(today, |p| p.from);
    let facts: Vec<SplitFact> = db::transactions::split_facts(&db.read, first, last_complete)
        .await?
        .into_iter()
        .map(|f| SplitFact {
            date: f.date,
            amount: f.amount,
            category_kind: f.category_kind,
            is_movement: f.is_movement,
            is_eur: f.currency == Currency::EUR.code(),
        })
        .collect();
    let first_activity = facts.iter().map(|f| f.date).min();
    let expenses: Vec<Money> = months
        .iter()
        .filter(|p| first_activity.is_some_and(|d| d <= p.to))
        .map(|p| reporting::flows(&facts, p.from, p.to).expenses)
        .collect();
    let average = reporting::average(&expenses);
    let variable_value: Money = variable.iter().map(|v| Money::from_minor(v.value_minor)).sum();
    let variable_irr = global_flows.iter().map(|f| f.date).min().and_then(|first| {
        let start = Point { date: first.pred_opt().unwrap_or(first), value: Money::ZERO };
        savings::irr(start, &global_flows, Point { date: today, value: variable_value })
    });
    let emergency: i64 =
        accounts.iter().filter(|a| a.include_in_emergency_fund && a.closed_on.is_none()).map(|a| a.balance_minor).sum();
    Ok(SavingsOverview {
        interest_accrued_minor: fixed.iter().map(|f| f.accrued_minor).sum(),
        interest_expected_minor: fixed.iter().map(|f| f.expected_minor).sum(),
        variable_value_minor: variable.iter().map(|v| v.value_minor).sum(),
        variable_invested_minor: variable.iter().map(|v| v.invested_minor).sum(),
        variable_irr_ppm: variable_irr.map(Rate::ppm),
        fixed,
        variable,
        emergency_fund_minor: emergency,
        average_expenses_minor: average.minor(),
        emergency_months_tenths: (average.is_positive()).then(|| emergency * 10 / average.minor()),
    })
}
