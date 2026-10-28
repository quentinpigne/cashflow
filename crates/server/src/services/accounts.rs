//! Comptes (spec 01 : CPT-01 à CPT-04, CPT-07).

use std::collections::HashMap;

use cashflow_domain::accounts::iban::Iban;
use cashflow_domain::accounts::{self, AccountFamily, AccountKind, AccountPeriod, ScheduleChoice};
use cashflow_domain::ledger::balance::{self, DatedAmount, Valuation};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{AccountId, InstitutionId, RateScheduleId};
use cashflow_domain::{Currency, Date, DomainError, Money};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{clean, required};
use crate::config::DataDir;
use crate::db::accounts::{AccountFields, AccountRow, TermsFields};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Un compte, avec sa valeur à la date demandée.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub kind: AccountKind,
    pub family: AccountFamily,
    pub institution_id: Option<InstitutionId>,
    pub institution_name: Option<String>,
    pub currency: String,
    pub opening_date: Date,
    pub opening_balance_minor: i64,
    /// IBAN masqué (R-CPT-8) ; l'IBAN complet s'obtient par une route dédiée.
    pub iban_masked: Option<String>,
    pub account_number: Option<String>,
    pub closed_on: Option<Date>,
    pub include_in_net_worth: bool,
    pub include_in_emergency_fund: bool,
    pub include_in_budget: bool,
    pub display_order: i64,
    pub color: Option<String>,
    pub notes: Option<String>,
    /// Solde (R-CPT-4) ou valeur (R-CPT-6) à `balance_date`.
    pub balance_minor: i64,
    /// Solde pointé : ouverture + transactions pointées ou rapprochées (CPT-03). Absent pour l'épargne
    /// variable et les prêts, suivis par valorisations.
    pub cleared_balance_minor: Option<i64>,
    pub balance_date: Date,
    /// Date de la valorisation retenue, pour un compte d'épargne variable ou un prêt.
    pub valuation_date: Option<Date>,
    /// Le compte a des transactions ou des valorisations : sa nature ne peut plus changer (R-CPT-1).
    pub has_activity: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AccountInput {
    pub name: String,
    pub kind: AccountKind,
    pub institution_id: Option<InstitutionId>,
    /// Code ISO 4217, `EUR` par défaut ; non modifiable après la création.
    pub currency: Option<String>,
    pub opening_date: Date,
    #[serde(default)]
    pub opening_balance_minor: i64,
    pub iban: Option<String>,
    pub account_number: Option<String>,
    pub include_in_net_worth: Option<bool>,
    pub include_in_emergency_fund: Option<bool>,
    pub include_in_budget: Option<bool>,
    pub display_order: Option<i64>,
    pub color: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CloseInput {
    pub closed_on: Date,
    /// Confirmation explicite d'une clôture à solde non nul (R-CPT-5).
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IbanView {
    pub iban: String,
}

/// Données nécessaires au calcul des soldes de tous les comptes.
pub struct Ledger {
    movements: HashMap<AccountId, Vec<DatedAmount>>,
    cleared: HashMap<AccountId, Vec<DatedAmount>>,
    valuations: HashMap<AccountId, Vec<Valuation>>,
}

impl Ledger {
    pub async fn load(db: &Db) -> AppResult<Self> {
        let mut movements: HashMap<AccountId, Vec<DatedAmount>> = HashMap::new();
        let mut cleared: HashMap<AccountId, Vec<DatedAmount>> = HashMap::new();
        for m in db::transactions::all_movements(&db.read).await? {
            let movement = DatedAmount { balance_date: m.balance_date, amount: m.amount };
            if m.cleared {
                cleared.entry(m.account_id).or_default().push(movement);
            }
            movements.entry(m.account_id).or_default().push(movement);
        }
        let mut valuations: HashMap<AccountId, Vec<Valuation>> = HashMap::new();
        for (account, date, value) in db::accounts::all_valuations(&db.read).await? {
            valuations.entry(account).or_default().push(Valuation { date, value });
        }
        Ok(Self { movements, cleared, valuations })
    }

    /// Solde ou valeur d'un compte à la fin de la journée `at`, selon sa famille.
    pub fn value(&self, account: &AccountRow, at: Date) -> (Money, Option<Date>) {
        let movements = self.movements.get(&account.id).map(Vec::as_slice).unwrap_or_default();
        if account.kind.family().has_computed_balance() {
            (balance::balance_at(account.opening_balance, movements, at), None)
        } else {
            let valuations = self.valuations.get(&account.id).map(Vec::as_slice).unwrap_or_default();
            balance::value_at(account.opening_balance, valuations, movements, at)
        }
    }

    /// Solde pointé à `at` (CPT-03), pour les comptes à solde calculé.
    pub fn cleared_balance(&self, account: &AccountRow, at: Date) -> Option<Money> {
        account.kind.family().has_computed_balance().then(|| {
            let cleared = self.cleared.get(&account.id).map(Vec::as_slice).unwrap_or_default();
            balance::balance_at(account.opening_balance, cleared, at)
        })
    }

    pub fn has_activity(&self, account: AccountId) -> bool {
        self.movements.get(&account).is_some_and(|m| !m.is_empty())
            || self.valuations.get(&account).is_some_and(|v| !v.is_empty())
    }
}

fn to_view(row: AccountRow, institutions: &HashMap<InstitutionId, String>, ledger: &Ledger, at: Date) -> Account {
    let (value, valuation_date) = ledger.value(&row, at);
    let cleared = ledger.cleared_balance(&row, at);
    Account {
        id: row.id,
        family: row.kind.family(),
        kind: row.kind,
        institution_name: row.institution_id.and_then(|i| institutions.get(&i).cloned()),
        institution_id: row.institution_id,
        currency: row.currency.code().to_owned(),
        opening_date: row.opening_date,
        opening_balance_minor: row.opening_balance.minor(),
        iban_masked: row.iban.as_deref().and_then(|i| Iban::parse(i).ok()).map(|i| i.masked()),
        account_number: row.account_number,
        closed_on: row.closed_on,
        include_in_net_worth: row.include_in_net_worth,
        include_in_emergency_fund: row.include_in_emergency_fund,
        include_in_budget: row.include_in_budget,
        display_order: row.display_order,
        color: row.color,
        notes: row.notes,
        balance_minor: value.minor(),
        cleared_balance_minor: cleared.map(Money::minor),
        balance_date: at,
        valuation_date,
        has_activity: ledger.has_activity(row.id),
        name: row.name,
    }
}

async fn institution_names(db: &Db) -> AppResult<HashMap<InstitutionId, String>> {
    Ok(db::institutions::list(&db.read).await?.into_iter().map(|i| (i.id, i.name)).collect())
}

/// Tous les comptes, avec leur valeur à `at` (CPT-01, CPT-03).
pub async fn list(db: &Db, at: Date) -> AppResult<Vec<Account>> {
    let institutions = institution_names(db).await?;
    let ledger = Ledger::load(db).await?;
    Ok(db::accounts::list(&db.read).await?.into_iter().map(|row| to_view(row, &institutions, &ledger, at)).collect())
}

pub async fn get(db: &Db, id: AccountId, at: Date) -> AppResult<Account> {
    let row = db::accounts::find(&db.write, id).await?.ok_or(AppError::NotFound("compte"))?;
    let institutions = institution_names(db).await?;
    let ledger = Ledger::load(db).await?;
    Ok(to_view(row, &institutions, &ledger, at))
}

pub async fn reveal_iban(db: &Db, id: AccountId) -> AppResult<IbanView> {
    let row = db::accounts::find(&db.read, id).await?.ok_or(AppError::NotFound("compte"))?;
    let iban = row.iban.ok_or(AppError::NotFound("IBAN"))?;
    let formatted = Iban::parse(&iban).map(|i| i.formatted()).unwrap_or(iban);
    Ok(IbanView { iban: formatted })
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AccountReorderInput {
    /// Comptes d'une même famille, dans l'ordre voulu.
    pub ids: Vec<AccountId>,
}

/// Ordre d'affichage des comptes d'une famille (CPT-07).
pub async fn reorder(db: &Db, input: &AccountReorderInput) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    for (position, id) in input.ids.iter().enumerate() {
        let order = i64::try_from(position).map_err(anyhow::Error::from)?;
        if !db::accounts::set_display_order(&mut *tx, *id, order).await? {
            return Err(AppError::NotFound("compte"));
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Un nouveau compte se range à la fin de sa famille (CPT-07).
async fn next_display_order(db: &Db, family: AccountFamily) -> AppResult<i64> {
    let rows = db::accounts::list(&db.read).await?;
    let last = rows.iter().filter(|r| r.kind.family() == family).map(|r| r.display_order).max();
    Ok(last.map_or(0, |order| order + 1))
}

/// Couleur d'un compte : `#RRGGBB` (CPT-07).
fn color(input: Option<&str>) -> AppResult<Option<String>> {
    let color = clean(input);
    let valid = |c: &str| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit());
    match color {
        Some(c) if !valid(&c) => Err(AppError::invalid_field("INVALID_COLOR", "color", "couleur attendue : #RRGGBB")),
        other => Ok(other.map(|c| c.to_lowercase())),
    }
}

struct Validated {
    name: String,
    currency: Currency,
    iban: Option<String>,
    account_number: Option<String>,
    color: Option<String>,
    notes: Option<String>,
}

fn validate(input: &AccountInput) -> AppResult<Validated> {
    let name = required(&input.name, "name", 80)?;
    let currency = match clean(input.currency.as_deref()) {
        Some(code) => code.to_uppercase().parse::<Currency>()?,
        None => Currency::EUR,
    };
    let iban = clean(input.iban.as_deref()).map(|i| Iban::parse(&i).map(|i| i.as_str().to_owned())).transpose()?;
    let account_number = clean(input.account_number.as_deref());
    if account_number.as_ref().is_some_and(|n| n.chars().count() > 30) {
        return Err(AppError::invalid_field("TOO_LONG", "account_number", "30 caractères au maximum"));
    }
    Ok(Validated {
        name,
        currency,
        iban,
        account_number,
        color: color(input.color.as_deref())?,
        notes: clean(input.notes.as_deref()),
    })
}

/// Drapeaux finaux : valeur saisie, sinon défaut de la nature, et toujours faux si la nature l'interdit.
fn flags(input: &AccountInput) -> (bool, bool, bool) {
    let kind = input.kind;
    let net_worth = input.include_in_net_worth.unwrap_or(true);
    let emergency =
        kind.allows_emergency_fund() && input.include_in_emergency_fund.unwrap_or(kind.default_in_emergency_fund());
    let budget = kind.allows_budget() && input.include_in_budget.unwrap_or(kind.default_in_budget());
    (net_worth, emergency, budget)
}

async fn check_institution(db: &Db, id: Option<InstitutionId>, current: Option<InstitutionId>) -> AppResult<()> {
    let Some(id) = id else { return Ok(()) };
    let institution = db::institutions::find(&db.read, id).await?.ok_or(AppError::NotFound("établissement"))?;
    // Un établissement archivé n'est plus proposé, sauf s'il est déjà celui du compte (R-CPT-10).
    if institution.archived && current != Some(id) {
        return Err(AppError::invalid_field("INSTITUTION_ARCHIVED", "institution_id", "cet établissement est archivé"));
    }
    Ok(())
}

/// Crée les conditions d'épargne préremplies d'un compte à taux déterminé (R-CPT-9).
async fn create_terms(
    conn: &mut sqlx::SqliteConnection,
    account: AccountId,
    kind: AccountKind,
    name: &str,
) -> AppResult<()> {
    let Some(defaults) = kind.fixed_savings_defaults() else { return Ok(()) };
    let schedule = match defaults.schedule {
        ScheduleChoice::Shared(shared) => match db::accounts::find_schedule_by_name(&mut *conn, shared.name()).await? {
            Some(id) => id,
            None => {
                let id = RateScheduleId::new();
                db::accounts::insert_schedule(&mut *conn, id, shared.name(), true).await?;
                id
            }
        },
        ScheduleChoice::Own => {
            let id = RateScheduleId::new();
            let base = format!("Taux — {name}");
            let mut schedule_name = base.clone();
            let mut n = 2;
            while db::accounts::schedule_name_taken(&mut *conn, &schedule_name).await? {
                schedule_name = format!("{base} ({n})");
                n += 1;
            }
            db::accounts::insert_schedule(&mut *conn, id, &schedule_name, false).await?;
            id
        }
    };
    let interest_category =
        db::categories::find_by_name(&mut *conn, super::seed::INTEREST_CATEGORY, CategoryKind::Income, None).await?;
    let terms = TermsFields {
        rate_schedule_id: schedule,
        interest_method: defaults.interest_method,
        capitalization: defaults.capitalization,
        deposit_cap: defaults.deposit_cap,
        maturity_date: None,
        interest_category_id: interest_category,
    };
    db::accounts::insert_terms(&mut *conn, account, &terms).await?;
    Ok(())
}

pub async fn create(db: &Db, input: &AccountInput, today: Date) -> AppResult<Account> {
    let v = validate(input)?;
    check_institution(db, input.institution_id, None).await?;
    let display_order = match input.display_order {
        Some(order) => order,
        None => next_display_order(db, input.kind.family()).await?,
    };
    let (net_worth, emergency, budget) = flags(input);
    let id = AccountId::new();
    let fields = AccountFields {
        name: &v.name,
        kind: input.kind,
        institution_id: input.institution_id,
        currency: v.currency,
        opening_date: input.opening_date,
        opening_balance: Money::from_minor(input.opening_balance_minor),
        iban: v.iban.as_deref(),
        account_number: v.account_number.as_deref(),
        include_in_net_worth: net_worth,
        include_in_emergency_fund: emergency,
        include_in_budget: budget,
        display_order,
        color: v.color.as_deref(),
        notes: v.notes.as_deref(),
    };
    let mut tx = db.write.begin().await?;
    db::accounts::insert(&mut *tx, id, &fields).await?;
    create_terms(&mut tx, id, input.kind, &v.name).await?;
    tx.commit().await?;
    get(db, id, today).await
}

pub async fn update(db: &Db, id: AccountId, input: &AccountInput, today: Date) -> AppResult<Account> {
    let current = db::accounts::find(&db.read, id).await?.ok_or(AppError::NotFound("compte"))?;
    let v = validate(input)?;
    if v.currency != current.currency {
        return Err(AppError::Domain(DomainError::AccountFieldLocked));
    }
    let has_activity = db::accounts::activity_count(&db.read, id).await? > 0;
    if input.kind != current.kind && has_activity {
        return Err(DomainError::AccountKindLocked.into());
    }
    check_institution(db, input.institution_id, current.institution_id).await?;
    // La nouvelle période doit contenir toutes les transactions existantes (R-CPT-3).
    if let Some((first, last)) = db::accounts::transaction_date_bounds(&db.read, id).await? {
        let period = AccountPeriod { opening: input.opening_date, closed_on: current.closed_on };
        period.check(first)?;
        period.check(last)?;
    }
    if current.closed_on.is_some_and(|closed| closed < input.opening_date) {
        return Err(DomainError::ClosingBeforeOpening.into());
    }
    // Le solde d'ouverture est un point de contrôle implicite toujours rapproché (R-RAP-8).
    let opening_changes =
        input.opening_balance_minor != current.opening_balance.minor() || input.opening_date != current.opening_date;
    if opening_changes && db::checkpoints::locked_until(&db.read, id).await?.is_some() {
        return Err(DomainError::OpeningBalanceLocked.into());
    }
    let (net_worth, emergency, budget) = flags(input);
    let fields = AccountFields {
        name: &v.name,
        kind: input.kind,
        institution_id: input.institution_id,
        currency: current.currency,
        opening_date: input.opening_date,
        opening_balance: Money::from_minor(input.opening_balance_minor),
        iban: v.iban.as_deref(),
        account_number: v.account_number.as_deref(),
        include_in_net_worth: net_worth,
        include_in_emergency_fund: emergency,
        include_in_budget: budget,
        display_order: input.display_order.unwrap_or(current.display_order),
        color: v.color.as_deref(),
        notes: v.notes.as_deref(),
    };
    let mut tx = db.write.begin().await?;
    db::accounts::update(&mut *tx, id, &fields).await?;
    if input.kind != current.kind {
        // Changement de nature sans activité : les conditions suivent la nouvelle nature (R-CPT-9).
        db::accounts::delete_terms(&mut tx, id).await?;
        create_terms(&mut tx, id, input.kind, &v.name).await?;
    } else if input.kind.family() == AccountFamily::FixedSavings && !db::accounts::has_terms(&mut *tx, id).await? {
        create_terms(&mut tx, id, input.kind, &v.name).await?;
    }
    tx.commit().await?;
    get(db, id, today).await
}

/// Clôture un compte (CPT-04, R-CPT-5).
pub async fn close(db: &Db, id: AccountId, input: &CloseInput, today: Date) -> AppResult<Account> {
    let row = db::accounts::find(&db.read, id).await?.ok_or(AppError::NotFound("compte"))?;
    if let Some((_, last)) = db::accounts::transaction_date_bounds(&db.read, id).await?
        && last > input.closed_on
    {
        return Err(AppError::invalid_field(
            "TRANSACTIONS_AFTER_CLOSING",
            "closed_on",
            format!("le compte a des transactions jusqu'au {last}"),
        ));
    }
    let ledger = Ledger::load(db).await?;
    let (balance, _) = ledger.value(&row, input.closed_on);
    accounts::check_closing(row.opening_date, input.closed_on, balance, input.confirm)?;
    db::accounts::set_closed_on(&db.write, id, Some(input.closed_on)).await?;
    get(db, id, today).await
}

pub async fn reopen(db: &Db, id: AccountId, today: Date) -> AppResult<Account> {
    db::accounts::find(&db.read, id).await?.ok_or(AppError::NotFound("compte"))?;
    db::accounts::set_closed_on(&db.write, id, None).await?;
    get(db, id, today).await
}

/// Supprime un compte vide, avec ses relevés et leurs documents (R-CPT-20) ; un compte qui a des transactions se
/// clôture (R-CPT-7).
pub async fn delete(db: &Db, data: &DataDir, id: AccountId, today: Date) -> AppResult<()> {
    db::accounts::find(&db.read, id).await?.ok_or(AppError::NotFound("compte"))?;
    // Seules les transactions l'empêchent : valorisations et points de contrôle partent avec le compte.
    if db::accounts::transaction_date_bounds(&db.read, id).await?.is_some()
        || db::imports::batches_for_account(&db.read, id).await? > 0
    {
        return Err(DomainError::AccountNotEmpty.into());
    }
    if db::loans::is_payer(&db.read, id).await? {
        return Err(AppError::invalid(
            "ACCOUNT_IS_LOAN_PAYER",
            "ce compte paie les mensualités d'un prêt : changer d'abord le compte payeur du prêt (R-CPT-15)",
        ));
    }
    let documents: Vec<_> =
        db::statements::of_account(&db.read, id).await?.into_iter().map(|s| s.document_id).collect();
    let mut tx = db.write.begin().await?;
    db::accounts::delete_terms(&mut tx, id).await?;
    db::accounts::delete(&mut *tx, id).await?;
    for document in &documents {
        db::documents::delete(&mut *tx, *document).await?;
    }
    tx.commit().await?;
    super::documents::trash(data, &documents, today);
    Ok(())
}
