//! Tests d'intégration de l'épargne (spec 06), sur une vraie base SQLite.

use cashflow_domain::Date;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::shared::ids::AccountId;

use super::accounts::{self, AccountInput};
use super::savings::{self, InterestProposal, PeriodQuery, RatePeriodInput, TermsInput, ValuationInput, YearQuery};
use super::tests::{d, setup, tx};
use super::transactions;
use crate::db::Db;
use crate::error::AppError;

async fn open(db: &Db, name: &str, kind: AccountKind, opening: &str, opening_minor: i64) -> AccountId {
    let input = AccountInput {
        name: name.into(),
        kind,
        institution_id: None,
        currency: None,
        opening_date: d(opening),
        opening_balance_minor: opening_minor,
        iban: None,
        account_number: None,
        include_in_net_worth: None,
        include_in_emergency_fund: None,
        include_in_budget: None,
        display_order: None,
        color: None,
        notes: None,
    };
    accounts::create(db, &input, d(opening)).await.unwrap().id
}

/// Un livret à barème propre, à 3 % depuis 2020.
async fn livret(db: &Db, opening: &str, opening_minor: i64) -> AccountId {
    let account = open(db, "Livret maison", AccountKind::SavingsOther, opening, opening_minor).await;
    let terms = savings::terms(db, account, d(opening)).await.unwrap();
    savings::set_rate(
        db,
        terms.rate_schedule_id,
        &RatePeriodInput { effective_from: d("2020-01-01"), rate_ppm: 30_000 },
    )
    .await
    .unwrap();
    account
}

fn year(y: i32) -> YearQuery {
    YearQuery { year: Some(y) }
}

#[tokio::test]
async fn epa_02_accrued_and_expected_interest() {
    let (db, _dir) = setup().await;
    let account = livret(&db, "2026-01-01", 1_000_000).await;
    let view = savings::interest(&db, account, &year(2026), d("2026-03-31")).await.unwrap();
    assert!(view.supported);
    // Le solde d'ouverture porte intérêt à partir du 16 janvier (R-EPA-2) : 23 quinzaines.
    assert_eq!(view.fortnights.len(), 23);
    assert_eq!(view.expected_minor, 28_750);
    assert_eq!(view.accrued_minor, 6_250);
    assert_eq!(view.proposal, InterestProposal::NotYet);
    assert_eq!(savings::terms(&db, account, d("2026-03-31")).await.unwrap().current_rate_ppm, 30_000);
}

#[tokio::test]
async fn epa_03_record_the_yearly_interest_once() {
    let (db, _dir) = setup().await;
    let account = livret(&db, "2026-01-01", 1_000_000).await;
    let today = d("2027-01-05");
    assert_eq!(
        savings::interest(&db, account, &year(2026), today).await.unwrap().proposal,
        InterestProposal::Available
    );
    let recorded = savings::record_interest(&db, account, 2026, today).await.unwrap();
    assert_eq!(recorded.amount_minor, 28_750);
    let after = savings::interest(&db, account, &year(2026), today).await.unwrap();
    assert_eq!(after.proposal, InterestProposal::Recorded);
    assert_eq!(after.booked_minor, Some(28_750));
    assert!(matches!(
        savings::record_interest(&db, account, 2026, today).await,
        Err(AppError::Invalid { code: "INTEREST_ALREADY_RECORDED", .. })
    ));
    // Les intérêts capitalisés portent intérêt dès le 1er janvier suivant (R-EPA-5).
    let next = savings::interest(&db, account, &year(2027), today).await.unwrap();
    assert_eq!(next.fortnights[0].balance_minor, 1_028_750);
    // Et ne comptent pas dans le capital versé (R-EPA-9).
    assert_eq!(next.capital_minor, 1_000_000);
}

#[tokio::test]
async fn epa_04_cap_warning_and_terms_update() {
    let (db, _dir) = setup().await;
    let account = livret(&db, "2026-01-01", 1_000_000).await;
    let terms = savings::terms(&db, account, d("2026-03-31")).await.unwrap();
    let input = TermsInput {
        rate_schedule_id: terms.rate_schedule_id,
        interest_method: terms.interest_method,
        capitalization: terms.capitalization,
        deposit_cap_minor: Some(1_050_000),
        maturity_date: None,
        interest_category_id: terms.interest_category_id,
        round_each_fortnight: false,
    };
    savings::update_terms(&db, account, &input, d("2026-03-31")).await.unwrap();
    transactions::create(&db, &tx(account, "2026-03-10", 100_000, None)).await.unwrap();
    let view = savings::interest(&db, account, &year(2026), d("2026-03-31")).await.unwrap();
    assert_eq!(view.capital_minor, 1_100_000);
    assert_eq!(view.cap_exceeded_on, Some(d("2026-03-10")));
}

#[tokio::test]
async fn epa_06_valuations_and_performance() {
    let (db, _dir) = setup().await;
    let life = open(&db, "Assurance vie", AccountKind::LifeInsurance, "2025-01-01", 1_000_000).await;
    transactions::create(&db, &tx(life, "2025-07-01", 200_000, None)).await.unwrap();
    let valuation = ValuationInput { date: d("2025-12-31"), value_minor: 1_260_000, note: None, replace: false };
    savings::save_valuation(&db, life, &valuation).await.unwrap();
    // Une seconde valorisation à la même date demande confirmation (CPT-06).
    assert!(matches!(
        savings::save_valuation(&db, life, &valuation).await,
        Err(AppError::Invalid { code: "VALUATION_EXISTS", .. })
    ));
    let today: Date = d("2026-01-10");
    let p = savings::performance(&db, life, &PeriodQuery { from: None, to: None }, today).await.unwrap();
    assert_eq!(p.value_minor, 1_260_000);
    assert_eq!(p.invested_minor, 1_200_000);
    assert_eq!(p.gain_minor, 60_000);
    assert_eq!(p.gain_ppm, Some(50_000));
    let irr = p.irr_since_opening_ppm.unwrap();
    assert!((54_000..=55_500).contains(&irr), "TRI : {irr}");
    assert_eq!(p.years[0].gain_minor, Some(60_000));
    assert!(!p.estimated);

    // Un compte à solde calculé ne se valorise pas.
    let checking = open(&db, "Courant", AccountKind::Checking, "2025-01-01", 0).await;
    assert!(savings::valuations(&db, checking).await.is_err());

    let overview = savings::overview(&db, today).await.unwrap();
    assert_eq!(overview.variable.len(), 1);
    assert_eq!(overview.variable_invested_minor, 1_200_000);
}

#[tokio::test]
async fn r_cpt_12_loan_valuations_are_negative() {
    let (db, _dir) = setup().await;
    let loan = open(&db, "Prêt", AccountKind::Mortgage, "2025-01-01", -10_000_000).await;
    let positive = ValuationInput { date: d("2025-06-30"), value_minor: 9_500_000, note: None, replace: false };
    assert!(savings::save_valuation(&db, loan, &positive).await.is_err());
    let negative = ValuationInput { value_minor: -9_500_000, ..positive };
    savings::save_valuation(&db, loan, &negative).await.unwrap();
    assert_eq!(accounts::get(&db, loan, d("2025-07-01")).await.unwrap().balance_minor, -9_500_000);
}
