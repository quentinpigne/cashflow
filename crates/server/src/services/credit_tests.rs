//! Tests d'intégration des simulations et demandes de crédit (spec 13), sur une vraie base SQLite.

use std::io::Read;

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::credit::pieces::{Piece, PieceStatus};
use cashflow_domain::credit::{ApplicationStatus, LenderRequestStatus, ProjectKind};
use cashflow_domain::documents::{DocumentKind, LinkKind, LinkTarget};
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::loans::{Insurance, LoanKind};
use cashflow_domain::shared::ids::{AccountId, CategoryId, EmploymentId, LoanApplicationId};

use super::credit::simulate::{
    self, CapacityRequest, CreditConditions, FinancingRequest, LoanCostRequest, SimulateQuestion,
};
use super::credit::{self, ApplicationInput, ComputeInput, LenderRequestInput, SimulationGoal, SimulationInput};
use super::documents::{self, Upload};
use super::loans::{self, LoanInput, ScheduleInput};
use super::salaries::{self, EmploymentInput, SalaryInput};
use super::tests::{account, category, d, setup};
use crate::config::DataDir;
use crate::db::Db;
use crate::error::AppError;

const TODAY: &str = "2026-10-29";

fn code(error: AppError) -> String {
    match error {
        AppError::Invalid { code, .. } => code.to_owned(),
        AppError::Domain(e) => e.code().to_owned(),
        other => format!("{other:?}"),
    }
}

fn simulation(name: &str, application: Option<LoanApplicationId>) -> SimulationInput {
    SimulationInput {
        name: name.into(),
        kind: LoanKind::Mortgage,
        principal_minor: 20_000_000,
        annual_rate_ppm: 30_000,
        duration_months: 240,
        insurance: Insurance::RateOnPrincipal { rate_ppm: 3_000 },
        fees_minor: 100_000,
        guarantee_fees_minor: 200_000,
        institution_id: None,
        application_id: application,
        note: None,
    }
}

struct Fixture {
    checking: AccountId,
    interest: CategoryId,
    employment: EmploymentId,
}

/// Un compte courant à 60 000 €, un emploi à 2 700 € net depuis 2025, un prêt auto en cours.
async fn fixture(db: &Db) -> Fixture {
    let today = d(TODAY);
    let checking = account(db, "Courant", AccountKind::Checking, 6_000_000).await;
    let pay = category(db, "Paie", CategoryKind::Income, None).await;
    let interest = category(db, "Intérêts", CategoryKind::Expense, None).await;
    let job = EmploymentInput {
        employer: "Acme".into(),
        job_title: None,
        start_date: d("2025-01-01"),
        end_date: None,
        income_category_id: pay,
        raise_month: None,
        paid_next_month: false,
        notes: None,
    };
    let employment = salaries::create(db, &job, today).await.unwrap().summary.id;
    let salary = SalaryInput {
        effective_month: d("2025-01-01"),
        annual_gross_minor: 4_200_000,
        monthly_net_minor: 270_000,
        reason: cashflow_domain::salaries::SalaryChangeReason::Hire,
        note: None,
    };
    salaries::save_salary(db, employment, None, &salary, today).await.unwrap();
    Fixture { checking, interest, employment }
}

fn car_loan(f: &Fixture) -> LoanInput {
    LoanInput {
        name: "Auto".into(),
        kind: LoanKind::Consumer,
        institution_id: None,
        start_date: d("2025-06-01"),
        principal_minor: 1_200_000,
        schedule: Some(ScheduleInput {
            annual_rate_ppm: 0,
            duration_months: 60,
            first_installment_date: d("2025-07-05"),
            insurance: Insurance::None,
            payer_account_id: f.checking,
            interest_category_id: f.interest,
            insurance_category_id: None,
        }),
        asset_id: None,
        closed_on: None,
        include_in_net_worth: None,
        notes: None,
    }
}

fn application(name: &str) -> ApplicationInput {
    ApplicationInput {
        name: name.into(),
        project_kind: ProjectKind::ExistingHome,
        asset_id: None,
        price_minor: 28_000_000,
        notary_fees_minor: None,
        works_minor: 1_000_000,
        other_costs_minor: 0,
        down_payment_minor: 4_100_000,
        other_income_minor: 0,
        other_charges_minor: 0,
        co_borrower_name: None,
        co_borrower_income_minor: 0,
        co_borrower_charges_minor: 0,
        excluded_loan_ids: vec![],
        selected_simulation_id: None,
        status: None,
        note: None,
    }
}

#[tokio::test]
async fn cre_01_cre_02_r_cre_1_to_r_cre_5_simulations() {
    let (db, _dir) = setup().await;
    let today = d(TODAY);
    let saved = credit::save_simulation(&db, None, &simulation("25 ans", None), today).await.unwrap();
    assert_eq!(saved.result.payment_minor, 110_920);
    assert_eq!(saved.result.total_payment_minor, 115_920);
    assert!(saved.result.apr_ppm > 30_416);
    let schedule = credit::simulation_schedule(&db, saved.id, today).await.unwrap();
    assert_eq!(schedule.len(), 240);

    let invalid = SimulationInput { duration_months: 0, ..simulation("x", None) };
    assert_eq!(code(credit::save_simulation(&db, None, &invalid, today).await.unwrap_err()), "INVALID_SIMULATION");
    let empty = SimulationInput { name: " ".into(), ..simulation("x", None) };
    assert_eq!(code(credit::save_simulation(&db, None, &empty, today).await.unwrap_err()), "REQUIRED");

    // Calculs inverses (R-CRE-5).
    let input = |goal, principal, duration, target| ComputeInput {
        goal,
        kind: LoanKind::Mortgage,
        principal_minor: principal,
        duration_months: duration,
        target_payment_minor: target,
        annual_rate_ppm: 30_000,
        insurance: Insurance::None,
        fees_minor: 0,
        guarantee_fees_minor: 0,
    };
    let duration = credit::compute(&input(SimulationGoal::Duration, Some(20_000_000), None, Some(140_000)), today);
    assert_eq!(duration.unwrap().duration_months, 177, "exemple CRE-02");
    let capacity = credit::compute(&input(SimulationGoal::Principal, None, Some(240), Some(110_920)), today).unwrap();
    // 200 001 € donnent encore 1 109,20 € de mensualité, arrondie au centime : c'est le plus grand capital.
    assert_eq!(capacity.principal_minor, 20_000_100);
    assert!(capacity.result.total_payment_minor <= 110_920);
    let missing = credit::compute(&input(SimulationGoal::Principal, None, Some(240), None), today).unwrap_err();
    assert_eq!(code(missing), "REQUIRED");
    let too_low = credit::compute(&input(SimulationGoal::Duration, Some(20_000_000), None, Some(40_000)), today);
    assert_eq!(code(too_low.unwrap_err()), "TARGET_PAYMENT_TOO_LOW");
}

#[tokio::test]
async fn cre_03_r_cre_7_simulation_becomes_a_loan_once() {
    let (db, _dir) = setup().await;
    let today = d(TODAY);
    let f = fixture(&db).await;
    let sim = credit::save_simulation(&db, None, &simulation("Offre LCL", None), today).await.unwrap();
    let mut input = car_loan(&f);
    input.name = "Appartement".into();
    input.principal_minor = sim.principal_minor;
    let loan = credit::convert(&db, sim.id, &input, today).await.unwrap();
    assert_eq!(credit::simulation(&db, sim.id, today).await.unwrap().loan_id, Some(loan));
    let again = credit::convert(&db, sim.id, &input, today).await.unwrap_err();
    assert_eq!(code(again), "SIMULATION_ALREADY_CONVERTED");
    // Supprimer le prêt retire le lien.
    loans::delete(&db, &DataDir::new("inutilisé"), loan, false, today).await.unwrap();
    assert_eq!(credit::simulation(&db, sim.id, today).await.unwrap().loan_id, None);
}

#[tokio::test]
async fn cre_04_r_cre_11_to_r_cre_16_financing_plan_and_capacity() {
    let (db, _dir) = setup().await;
    let today = d(TODAY);
    let f = fixture(&db).await;
    let car = loans::create(&db, &car_loan(&f), today).await.unwrap().summary.id;

    let view = credit::save_application(&db, None, &application("Appartement"), today).await.unwrap();
    assert_eq!(view.notary_fees_minor, 2_100_000, "R-CRE-12 : 7,5 % dans l'ancien");
    assert_eq!(view.project_cost_minor, 31_100_000);
    assert_eq!(view.to_borrow_minor, 27_000_000);
    assert_eq!(view.status, ApplicationStatus::Preparing);

    let c = &view.capacity;
    assert_eq!(c.income_minor, 270_000);
    assert_eq!(c.loans.len(), 1);
    assert_eq!(c.charges_minor, 20_000, "prêt auto : 12 000 € sur 60 mois");
    assert_eq!(c.savings_minor, 6_000_000);
    assert_eq!(c.savings_after_minor, 6_000_000 - 4_100_000);
    assert_eq!(c.payment_minor, None);

    // Simulation retenue, co-emprunteur, prêt auto remboursé par le nouveau crédit.
    let sim = credit::save_simulation(&db, None, &simulation("25 ans", Some(view.id)), today).await.unwrap();
    let mut input = application("Appartement");
    input.selected_simulation_id = Some(sim.id);
    let view = credit::save_application(&db, Some(view.id), &input, today).await.unwrap();
    assert_eq!(view.capacity.debt_ratio_ppm, Some(503_407), "(200 + 1 159,20) / 2 700");
    assert!(view.capacity.debt_ratio_exceeded);
    input.co_borrower_name = Some("Camille".into());
    input.co_borrower_income_minor = 250_000;
    input.excluded_loan_ids = vec![car];
    let view = credit::save_application(&db, Some(view.id), &input, today).await.unwrap();
    assert_eq!(view.capacity.charges_minor, 0);
    assert_eq!(view.capacity.income_minor, 520_000);
    assert!(!view.capacity.debt_ratio_exceeded);
    assert!(view.capacity.borrowing_capacity_minor.is_some());

    // Une simulation d'une autre demande ne se retient pas ; un apport trop grand est refusé.
    let other = credit::save_simulation(&db, None, &simulation("Ailleurs", None), today).await.unwrap();
    input.selected_simulation_id = Some(other.id);
    let error = credit::save_application(&db, Some(view.id), &input, today).await.unwrap_err();
    assert_eq!(code(error), "SIMULATION_OF_ANOTHER_APPLICATION");
    input.selected_simulation_id = None;
    input.down_payment_minor = 40_000_000;
    let error = credit::save_application(&db, Some(view.id), &input, today).await.unwrap_err();
    assert_eq!(code(error), "INVALID_FINANCING_PLAN");

    // Supprimer la demande garde ses simulations, sans demande (R-CRE-23).
    credit::delete_application(&db, view.id).await.unwrap();
    assert_eq!(credit::simulation(&db, sim.id, today).await.unwrap().application_id, None);
}

#[tokio::test]
async fn cre_05_r_cre_17_to_r_cre_19_pieces_and_export() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path().join("data"));
    data.ensure().unwrap();
    let today = d(TODAY);
    let f = fixture(&db).await;
    for (month, content) in [("2026-08-01", "aout"), ("2026-09-01", "septembre"), ("2026-10-01", "octobre")] {
        let upload = Upload {
            file_name: format!("paie-{month}.pdf"),
            content: format!("%PDF-1.4 {content}").into_bytes(),
            kind: Some(DocumentKind::Payslip),
            month: Some(d(month)),
            target: Some(LinkTarget { kind: LinkKind::Employment, id: *f.employment.as_uuid() }),
            ..Default::default()
        };
        documents::create(&db, &data, upload, today).await.unwrap();
    }
    let view = credit::save_application(&db, None, &application("Appartement"), today).await.unwrap();
    let piece = |view: &credit::ApplicationView, piece: Piece| {
        view.pieces.iter().find(|p| p.piece == piece).map(|p| (p.status, p.documents.len()))
    };
    assert_eq!(piece(&view, Piece::Payslips), Some((PieceStatus::Provided, 3)), "trouvées dans la bibliothèque");
    assert_eq!(piece(&view, Piece::TaxNotices), Some((PieceStatus::Missing, 0)));
    assert_eq!(piece(&view, Piece::BankStatements).map(|p| p.0), Some(PieceStatus::Missing));
    assert_eq!(piece(&view, Piece::Quotes), Some((PieceStatus::Missing, 0)), "travaux : devis demandé");
    assert_eq!(piece(&view, Piece::CoBorrowerIdentity), None);

    // Attribution à la main (R-CRE-18).
    let agreement = Upload {
        file_name: "compromis.pdf".into(),
        content: b"%PDF-1.4 compromis".to_vec(),
        kind: Some(DocumentKind::SaleAgreement),
        ..Default::default()
    };
    let agreement = documents::create(&db, &data, agreement, today).await.unwrap();
    let view = credit::assign(&db, view.id, Piece::SaleAgreement, agreement.id, today).await.unwrap();
    assert_eq!(piece(&view, Piece::SaleAgreement), Some((PieceStatus::Provided, 1)));
    let view = credit::set_required(&db, view.id, Piece::TaxNotices, true, today).await.unwrap();
    assert_eq!(piece(&view, Piece::TaxNotices).map(|p| p.0), Some(PieceStatus::NotRequired));
    let summary = credit::applications(&db, today).await.unwrap();
    assert!(summary[0].missing_pieces > 0);

    // Export : un dossier par pièce, et la synthèse (R-CRE-19).
    let (name, zip) = credit::export(&db, &data, view.id, today).await.unwrap();
    assert!(name.ends_with("Appartement.zip"), "{name}");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
    let names: Vec<String> = archive.file_names().map(|n| n.unwrap().into_owned()).collect();
    assert!(names.contains(&"03 3 dernières fiches de paie/paie-2026-10-01.pdf".to_owned()), "{names:?}");
    assert!(names.iter().any(|n| n.ends_with("Compromis de vente/compromis.pdf")), "{names:?}");
    let mut html = String::new();
    archive.by_name("Synthèse.html").unwrap().read_to_string(&mut html).unwrap();
    assert!(html.contains("270\u{202f}000,00\u{a0}€"), "montant à emprunter");
    assert!(html.contains("manquante"));
}

#[tokio::test]
async fn cre_06_r_cre_20_to_r_cre_22_lender_requests_and_offers() {
    let (db, _dir) = setup().await;
    let today = d(TODAY);
    let view = credit::save_application(&db, None, &application("Appartement"), today).await.unwrap();
    let request = |name: &str| LenderRequestInput {
        institution_id: None,
        lender_name: Some(name.into()),
        status: LenderRequestStatus::Sent,
        sent_on: d("2026-10-20"),
        answered_on: None,
        simulation_id: None,
        follow_up_on: None,
        note: None,
    };
    let view = credit::save_lender_request(&db, view.id, None, &request("LCL"), today).await.unwrap();
    assert_eq!(view.status, ApplicationStatus::Submitted, "R-CRE-20");
    let view = credit::save_lender_request(&db, view.id, None, &request("Crédit Agricole"), today).await.unwrap();
    let (lcl, ca) = (view.lender_requests[0].id, view.lender_requests[1].id);

    // Accepter sans offre est refusé.
    assert_eq!(code(credit::accept_offer(&db, view.id, lcl, today).await.unwrap_err()), "OFFER_NOT_ACCEPTABLE");

    let offer = SimulationInput { annual_rate_ppm: 33_500, duration_months: 300, ..simulation("Offre LCL", None) };
    let view = credit::record_offer(&db, view.id, lcl, &offer, today).await.unwrap();
    let lcl_request = view.lender_requests.iter().find(|r| r.id == lcl).unwrap();
    assert_eq!(lcl_request.status, LenderRequestStatus::Offer);
    assert_eq!(lcl_request.answered_on, Some(today));
    assert_eq!(view.simulations.len(), 1, "l'offre est une simulation de la demande");

    let view = credit::accept_offer(&db, view.id, lcl, today).await.unwrap();
    assert_eq!(view.status, ApplicationStatus::Accepted);
    assert_eq!(view.selected_simulation_id, lcl_request.simulation_id);
    let statuses: Vec<_> = view.lender_requests.iter().map(|r| (r.id, r.status)).collect();
    assert!(statuses.contains(&(lcl, LenderRequestStatus::Accepted)));
    assert!(statuses.contains(&(ca, LenderRequestStatus::Withdrawn)));

    // Une démarche sans établissement ni nom est refusée.
    let anonymous = LenderRequestInput { lender_name: None, ..request("x") };
    let error = credit::save_lender_request(&db, view.id, None, &anonymous, today).await.unwrap_err();
    assert_eq!(code(error), "REQUIRED");
    credit::delete_lender_request(&db, view.id, ca).await.unwrap();
}

fn home(conditions: CreditConditions) -> CapacityRequest {
    CapacityRequest { project_kind: ProjectKind::ExistingHome, conditions }
}

#[tokio::test]
async fn cre_07_r_cre_30_to_r_cre_36_purchase_capacity() {
    let (db, _dir) = setup().await;
    let today = d(TODAY);
    let f = fixture(&db).await;
    loans::create(&db, &car_loan(&f), today).await.unwrap();

    // Situation par défaut : le compte courant fait partie de l'épargne de précaution (R-CPT-13, R-CRE-30).
    let situation = simulate::borrower_situation(&db, today).await.unwrap();
    assert_eq!(situation.income_minor, 270_000);
    assert_eq!(situation.charges_minor, 20_000);
    assert!(situation.accounts.iter().all(|a| a.selected));
    assert_eq!(situation.savings_minor, 6_000_000);

    let defaults = simulate::project_defaults(&db).await.unwrap();
    let conditions = defaults[0].conditions;
    let capacity = simulate::purchase_capacity(&db, &home(conditions), today).await.unwrap();
    assert_eq!(capacity.today.max_payment_minor, 94_500 - 20_000, "35 % de 2 700 € − 200 €");
    assert!(capacity.today.capital_minor > 0);
    assert_eq!(capacity.sensitivity.len(), 9);
    // Les 5 prochaines années, avec les salaires projetés du scénario par défaut (R-CRE-33).
    assert_eq!(capacity.years.len(), 6);
    assert!(capacity.years[5].income_minor > capacity.years[0].income_minor, "hausse des salaires");

    // Le compte courant mobilisé, sans réserve ; à 3 % sur 20 ans, sans assurance ni frais.
    let mut s = situation.situation.clone();
    s.account_ids = Some(vec![f.checking]);
    s.reserve_months = 0;
    let situation = simulate::save_borrower_situation(&db, &s, today).await.unwrap();
    assert_eq!(situation.possible_down_payment_minor, 6_000_000);
    let bare = CreditConditions {
        annual_rate_ppm: 30_000,
        duration_months: 240,
        insurance: Insurance::None,
        fees_minor: 0,
        guarantee_fees_minor: 0,
        ..conditions
    };
    let capacity = simulate::purchase_capacity(&db, &home(bare), today).await.unwrap();
    assert_eq!(capacity.today.down_payment_minor, 6_000_000, "R-CRE-31");
    assert!(capacity.today.max_price_minor > capacity.today.capital_minor);
    assert!(
        capacity.today.max_price_minor + capacity.today.notary_fees_minor
            <= capacity.today.capital_minor + capacity.today.down_payment_minor
    );
    assert_eq!(capacity.savings_after_minor, 0);
    assert_eq!(capacity.affordability.remaining_income_minor, 270_000 - 20_000 - capacity.today.payment_minor);
    assert_eq!(simulate::borrower_situation(&db, today).await.unwrap().situation.reserve_months, 0, "gardée");

    // Patrimoine projeté : l'achat coûte les frais de notaire, puis les intérêts au fil des mois (R-CRE-35).
    let gap = |p: &simulate::NetWorthComparison| p.without_minor - p.with_minor;
    assert_eq!(capacity.net_worth.len(), 120);
    assert!(gap(&capacity.net_worth[0]) > capacity.today.notary_fees_minor);
    assert!(gap(&capacity.net_worth[119]) > gap(&capacity.net_worth[0]));

    // Pour un véhicule : sans frais de notaire, sensibilité autour de la durée, sans patrimoine (R-CRE-34, R-CRE-35).
    let vehicle = CapacityRequest {
        project_kind: ProjectKind::Vehicle,
        conditions: defaults.iter().find(|d| d.project_kind == ProjectKind::Vehicle).unwrap().conditions,
    };
    let capacity = simulate::purchase_capacity(&db, &vehicle, today).await.unwrap();
    assert_eq!(capacity.today.notary_fees_minor, 0);
    assert!(capacity.net_worth.is_empty());
    let durations: Vec<u32> = capacity.sensitivity.iter().map(|c| c.duration_months).collect();
    assert_eq!(durations, [48, 48, 48, 60, 60, 60, 72, 72, 72]);

    // Situation et conditions invalides refusées.
    let invalid = simulate::BorrowerSituation { max_debt_ratio_ppm: 0, ..s.clone() };
    assert_eq!(code(simulate::save_borrower_situation(&db, &invalid, today).await.unwrap_err()), "INVALID_SITUATION");
    let zero = CreditConditions { duration_months: 0, ..bare };
    assert_eq!(code(simulate::purchase_capacity(&db, &home(zero), today).await.unwrap_err()), "INVALID_SIMULATION");

    // Une demande créée depuis la capacité (R-CRE-36).
    let capacity = simulate::purchase_capacity(&db, &home(bare), today).await.unwrap();
    let view = simulate::application_from_capacity(&db, &home(bare), "Projet", today).await.unwrap();
    assert_eq!(view.price_minor, capacity.today.max_price_minor);
    assert!(view.down_payment_minor <= 6_000_000, "au plus l'apport possible");
    assert_eq!(view.to_borrow_minor, capacity.today.capital_minor, "l'apport complète le capital empruntable");
    let selected = view.simulations.iter().find(|s| Some(s.id) == view.selected_simulation_id).unwrap();
    assert_eq!(selected.principal_minor, view.to_borrow_minor);
}

#[tokio::test]
async fn cre_12_r_cre_38_r_cre_39_project_defaults_and_state() {
    let (db, _dir) = setup().await;
    let defaults = simulate::project_defaults(&db).await.unwrap();
    assert_eq!(defaults.len(), 5);
    let vehicle = defaults.iter().find(|d| d.project_kind == ProjectKind::Vehicle).unwrap();
    assert_eq!((vehicle.conditions.annual_rate_ppm, vehicle.conditions.duration_months), (50_000, 60));

    // Je règle le véhicule à 4,5 % sur 48 mois ; les valeurs initiales restent connues.
    let mine = CreditConditions { annual_rate_ppm: 45_000, duration_months: 48, ..vehicle.conditions };
    let input = [simulate::ProjectDefaultsInput { project_kind: ProjectKind::Vehicle, conditions: mine }];
    let saved = simulate::save_project_defaults(&db, &input).await.unwrap();
    let vehicle = saved.iter().find(|d| d.project_kind == ProjectKind::Vehicle).unwrap();
    assert_eq!(vehicle.conditions, mine);
    assert_eq!(vehicle.initial.annual_rate_ppm, 50_000);
    assert_eq!(saved[0].conditions, saved[0].initial, "les autres projets gardent leurs valeurs initiales");
    let invalid = [simulate::ProjectDefaultsInput {
        project_kind: ProjectKind::Other,
        conditions: CreditConditions { annual_rate_ppm: -1, ..mine },
    }];
    assert!(simulate::save_project_defaults(&db, &invalid).await.is_err());

    // Le dernier état de l'onglet : par défaut, l'ancien et ses conditions ; puis ce qui a été gardé (R-CRE-39).
    let state = simulate::simulate_state(&db).await.unwrap();
    assert_eq!((state.project_kind, state.question), (ProjectKind::ExistingHome, SimulateQuestion::MaxPrice));
    assert_eq!(state.conditions, saved[0].conditions);
    let mine = simulate::SimulateState {
        project_kind: ProjectKind::Vehicle,
        question: SimulateQuestion::FinancePurchase,
        conditions: mine,
        price_minor: Some(2_500_000),
        ..state
    };
    simulate::save_simulate_state(&db, &mine).await.unwrap();
    let state = simulate::simulate_state(&db).await.unwrap();
    assert_eq!((state.project_kind, state.price_minor), (ProjectKind::Vehicle, Some(2_500_000)));
}

#[tokio::test]
async fn cre_11_r_cre_40_r_cre_41_finance_a_purchase_and_cost_of_a_loan() {
    let (db, _dir) = setup().await;
    let today = d(TODAY);
    let f = fixture(&db).await;
    loans::create(&db, &car_loan(&f), today).await.unwrap();
    let mut s = simulate::borrower_situation(&db, today).await.unwrap().situation;
    s.reserve_months = 0;
    simulate::save_borrower_situation(&db, &s, today).await.unwrap();
    let conditions = CreditConditions::from(ProjectKind::Vehicle.initial_conditions());

    // Une voiture à 25 000 € avec 5 000 € d'apport, à 5 % sur 60 mois (R-CRE-40).
    let request = FinancingRequest {
        project_kind: ProjectKind::Vehicle,
        conditions,
        price_minor: 2_500_000,
        down_payment_minor: Some(500_000),
        notary_fees_minor: None,
        target_payment_minor: None,
    };
    let view = simulate::financing(&db, &request, today).await.unwrap();
    assert_eq!((view.notary_fees_minor, view.to_borrow_minor), (0, 2_000_000));
    let loan = view.loan.as_ref().unwrap();
    assert_eq!((loan.result.total_payment_minor, loan.duration_months), (37_742, 60));
    let a = &view.affordability;
    assert_eq!(a.debt_ratio_ppm, Some(213_859), "(200 + 377,42) / 2 700");
    assert!(!a.debt_ratio_exceeded && !view.down_payment_exceeds_possible);
    assert_eq!(view.savings_after_minor, 6_000_000 - 500_000);

    // Sans apport saisi : l'apport proposé, au plus le coût ; égal au coût, rien à emprunter.
    let proposed = FinancingRequest { down_payment_minor: None, ..request.clone() };
    let view = simulate::financing(&db, &proposed, today).await.unwrap();
    assert_eq!(view.down_payment_minor, 2_500_000);
    assert!(view.loan.is_none());
    let error = simulate::application_from_financing(&db, &proposed, "Comptant", today).await.unwrap_err();
    assert_eq!(code(error), "NOTHING_TO_BORROW");

    // Un apport au-delà de l'apport possible (aucun compte mobilisé) est signalé ; au-delà du coût, refusé.
    s.account_ids = Some(vec![]);
    simulate::save_borrower_situation(&db, &s, today).await.unwrap();
    let view = simulate::financing(&db, &request, today).await.unwrap();
    assert!(view.down_payment_exceeds_possible);
    let too_much = FinancingRequest { down_payment_minor: Some(3_000_000), ..request.clone() };
    assert_eq!(code(simulate::financing(&db, &too_much, today).await.unwrap_err()), "INVALID_FINANCING_PLAN");

    // La mensualité fixée à 400 € : on cherche la durée.
    let fixed = FinancingRequest { target_payment_minor: Some(40_000), ..request.clone() };
    let duration = simulate::financing(&db, &fixed, today).await.unwrap().loan.unwrap().duration_months;
    assert!(duration < 60);

    // Un logement : frais de notaire estimés, modifiables.
    let flat = FinancingRequest {
        project_kind: ProjectKind::ExistingHome,
        conditions: CreditConditions::from(ProjectKind::ExistingHome.initial_conditions()),
        price_minor: 28_000_000,
        down_payment_minor: Some(4_000_000),
        notary_fees_minor: None,
        target_payment_minor: None,
    };
    let view = simulate::financing(&db, &flat, today).await.unwrap();
    assert_eq!(view.notary_fees_minor, 2_100_000);
    assert_eq!(view.to_borrow_minor, 28_000_000 + 2_100_000 + 300_000 - 4_000_000);
    assert!(view.affordability.debt_ratio_exceeded, "2 700 € de revenus ne suffisent pas");

    // La demande préparée reprend le prix, les frais et l'apport saisis (R-CRE-36).
    let application = simulate::application_from_financing(&db, &request, "Voiture", today).await.unwrap();
    assert_eq!(application.project_kind, ProjectKind::Vehicle);
    assert_eq!((application.price_minor, application.down_payment_minor), (2_500_000, 500_000));
    assert_eq!(application.to_borrow_minor, 2_000_000);
    assert!(application.selected_simulation_id.is_some());

    // Que coûte ce prêt ? Le simulateur, et l'endettement qu'il donnerait (R-CRE-41).
    let cost = simulate::loan_cost(
        &db,
        &LoanCostRequest {
            project_kind: ProjectKind::Vehicle,
            loan: ComputeInput {
                goal: SimulationGoal::Payment,
                kind: LoanKind::Consumer,
                principal_minor: Some(2_000_000),
                duration_months: Some(60),
                target_payment_minor: None,
                annual_rate_ppm: 50_000,
                insurance: Insurance::None,
                fees_minor: 0,
                guarantee_fees_minor: 0,
            },
        },
        today,
    )
    .await
    .unwrap();
    assert_eq!(cost.loan.result.total_payment_minor, 37_742);
    assert_eq!(cost.affordability.remaining_income_minor, 270_000 - 20_000 - 37_742);
}

#[tokio::test]
async fn r_cre_36_application_from_capacity_on_the_demo_data() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path().join("data"));
    data.ensure().unwrap();
    let today = d("2026-10-20");
    super::demo::load(&db, &data, today).await.unwrap();
    // Charges au-delà de 35 % : aucun capital empruntable ; la demande depuis la capacité est refusée.
    let mut s = simulate::borrower_situation(&db, today).await.unwrap().situation;
    s.other_charges_minor = 10_000_000;
    let situation = simulate::save_borrower_situation(&db, &s, today).await.unwrap();
    let request = home(CreditConditions::from(ProjectKind::ExistingHome.initial_conditions()));
    let capacity = simulate::purchase_capacity(&db, &request, today).await.unwrap();
    assert_eq!(capacity.today.capital_minor, 0);
    let error = simulate::application_from_capacity(&db, &request, "Achat", today).await.unwrap_err();
    assert_eq!(code(error), "NO_BORROWING_CAPACITY");

    // Depuis une simulation, la demande se prépare quand même, sans plus d'apport que nécessaire (R-CRE-36).
    let small = SimulationInput { principal_minor: 100_000, ..simulation("Petit crédit", None) };
    let sim = credit::save_simulation(&db, None, &small, today).await.unwrap();
    let view = simulate::application_from_simulation(&db, sim.id, ProjectKind::Vehicle, "Achat", today).await.unwrap();
    assert_eq!(view.project_kind, ProjectKind::Vehicle);
    assert!(view.down_payment_minor <= situation.possible_down_payment_minor);
    assert_eq!(view.selected_simulation_id, Some(sim.id));
    assert!(view.to_borrow_minor <= 100_000);
    assert_eq!(credit::simulation(&db, sim.id, today).await.unwrap().application_id, Some(view.id));
}

#[tokio::test]
async fn cre_09_r_cre_24_to_r_cre_26_journal_and_follow_ups() {
    let (db, _dir) = setup().await;
    let sent = d("2026-10-20");
    let view = credit::save_application(&db, None, &application("Appartement"), sent).await.unwrap();
    let input = LenderRequestInput {
        institution_id: None,
        lender_name: Some("LCL".into()),
        status: LenderRequestStatus::Sent,
        sent_on: sent,
        answered_on: None,
        simulation_id: None,
        follow_up_on: None,
        note: None,
    };
    let view = credit::save_lender_request(&db, view.id, None, &input, sent).await.unwrap();
    let request = &view.lender_requests[0];
    assert_eq!(request.follow_up_on, Some(d("2026-11-03")), "R-CRE-25 : envoi + 14 jours");
    assert_eq!(request.events.len(), 1, "R-CRE-24 : l'envoi au journal");
    assert!(!request.follow_up_due);
    let id = request.id;

    // Le 3 novembre, la démarche est à relancer ; la notification n'est émise qu'une fois (R-CRE-26).
    let due = d("2026-11-03");
    assert!(credit::application(&db, view.id, due).await.unwrap().lender_requests[0].follow_up_due);
    assert_eq!(credit::applications(&db, due).await.unwrap()[0].follow_ups_due, 1);
    assert_eq!(credit::notify_follow_ups(&db, due).await.unwrap(), 1);
    assert_eq!(credit::notify_follow_ups(&db, due).await.unwrap(), 0);

    // Relancer le 4 novembre : la relance au journal, la prochaine 7 jours plus tard (R-CRE-25).
    let next = d("2026-11-04");
    let followed = credit::FollowUpInput { note: Some("Mail au conseiller".into()), next_follow_up_on: None };
    let view = credit::follow_up(&db, view.id, id, &followed, next).await.unwrap();
    let request = &view.lender_requests[0];
    assert_eq!(request.follow_up_on, Some(d("2026-11-11")));
    assert!(!request.follow_up_due);
    assert_eq!(request.events[0].kind, cashflow_domain::credit::LenderEventKind::FollowUp);
    assert_eq!(request.events[0].note.as_deref(), Some("Mail au conseiller"));

    // Un appel ajouté à la main, puis supprimé.
    let call = credit::LenderEventInput {
        date: next,
        kind: cashflow_domain::credit::LenderEventKind::Call,
        note: Some("Rendez-vous fixé".into()),
    };
    let view = credit::save_event(&db, view.id, id, None, &call, next).await.unwrap();
    assert_eq!(view.lender_requests[0].events.len(), 3);
    let call_id =
        view.lender_requests[0].events.iter().find(|e| e.note.as_deref() == Some("Rendez-vous fixé")).unwrap().id;
    let view = credit::delete_event(&db, view.id, id, call_id, next).await.unwrap();
    assert_eq!(view.lender_requests[0].events.len(), 2);

    // Une offre reçue : réponse au journal, relance effacée.
    let view = credit::record_offer(&db, view.id, id, &simulation("Offre LCL", None), next).await.unwrap();
    let request = &view.lender_requests[0];
    assert_eq!(request.follow_up_on, None);
    assert_eq!(request.events[0].kind, cashflow_domain::credit::LenderEventKind::Answer);
}
