//! Tests d'intégration des biens et des prêts (spec 09), sur une vraie base SQLite.

use cashflow_domain::accounts::AccountKind;
use cashflow_domain::assets::AssetKind;
use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::loans::{Insurance, LoanKind};
use cashflow_domain::shared::ids::{AccountId, AssetId, CategoryId, LoanId, TransactionId};

use super::assets::{self, AssetInput, AssetValuationInput};
use super::features::{self, FeaturesInput};
use super::loans::{self, BalanceInput, LoanInput, LoanPaymentInput, LoanPreviewInput, ScheduleInput};
use super::tests::{account, category, d, setup, split, tx};
use super::{accounts, transactions};
use crate::db::Db;
use crate::error::AppError;

struct Fixture {
    checking: AccountId,
    interest: CategoryId,
    insurance: CategoryId,
}

async fn fixture(db: &Db) -> Fixture {
    let parent = category(db, "Prêt maison", CategoryKind::Expense, None).await;
    Fixture {
        checking: account(db, "Courant", AccountKind::Checking, 100_000).await,
        interest: category(db, "Intérêts du prêt", CategoryKind::Expense, Some(parent)).await,
        insurance: category(db, "Assurance du prêt", CategoryKind::Expense, Some(parent)).await,
    }
}

/// Prêt de la spec (PRE-02) : 200 000 € à 3 % sur 240 mois, assurance 0,30 % du capital.
fn input(f: &Fixture) -> LoanInput {
    LoanInput {
        name: "Prêt immobilier".into(),
        kind: LoanKind::Mortgage,
        institution_id: None,
        start_date: d("2025-12-15"),
        principal_minor: 20_000_000,
        schedule: Some(ScheduleInput {
            annual_rate_ppm: 30_000,
            duration_months: 240,
            first_installment_date: d("2026-01-05"),
            insurance: Insurance::RateOnPrincipal { rate_ppm: 3_000 },
            payer_account_id: f.checking,
            interest_category_id: f.interest,
            insurance_category_id: Some(f.insurance),
        }),
        asset_id: None,
        closed_on: None,
        include_in_net_worth: None,
        notes: None,
    }
}

async fn create(db: &Db, f: &Fixture) -> LoanId {
    loans::create(db, &input(f), d("2026-01-31")).await.unwrap().summary.id
}

fn flat() -> AssetInput {
    AssetInput {
        name: "Appartement".into(),
        kind: AssetKind::RealEstate,
        purchase_date: d("2025-12-15"),
        purchase_price_minor: 25_000_000,
        sold_on: None,
        sale_price_minor: None,
        include_in_net_worth: None,
        notes: None,
    }
}

fn code(error: AppError) -> String {
    match error {
        AppError::Invalid { code, .. } => code.to_owned(),
        AppError::Domain(e) => e.code().to_owned(),
        other => format!("{other:?}"),
    }
}

async fn spend(db: &Db, account: AccountId, date: &str, amount: i64) -> TransactionId {
    transactions::create(db, &tx(account, date, amount, None)).await.unwrap()
}

// ---------------------------------------------------------------- Prêts

#[tokio::test]
async fn pre_02_r_pre_4_schedule_and_outstanding_principal() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let id = create(&db, &f).await;
    let view = loans::get(&db, id, d("2026-03-31")).await.unwrap();
    let schedule = view.schedule.unwrap();
    assert_eq!((schedule.monthly_payment_minor, schedule.monthly_insurance_minor), (110_920, 5_000));
    assert_eq!(schedule.installments.len(), 240);
    assert_eq!(schedule.end_date, d("2045-12-05"));
    // Le capital restant dû suit le tableau, sans mensualité enregistrée (R-PRE-4).
    assert_eq!(view.summary.outstanding_minor, Some(schedule.installments[2].remaining_minor));
    assert_eq!(view.summary.next_installment.unwrap().number, 4);
    let before = loans::get(&db, id, d("2025-12-01")).await.unwrap();
    assert_eq!(before.summary.outstanding_minor, None, "pas encore débloqué");
}

#[tokio::test]
async fn r_pre_1_r_pre_2_a_loan_is_checked() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let today = d("2026-03-31");
    let mut bad = input(&f);
    bad.schedule.as_mut().unwrap().duration_months = 0;
    assert_eq!(code(loans::create(&db, &bad, today).await.unwrap_err()), "INVALID_LOAN_TERMS");
    let mut bad = input(&f);
    bad.start_date = d("2026-02-01");
    assert_eq!(code(loans::create(&db, &bad, today).await.unwrap_err()), "INVALID_LOAN");
    let livret = account(&db, "Livret", AccountKind::LivretA, 0).await;
    let mut bad = input(&f);
    bad.schedule.as_mut().unwrap().payer_account_id = livret;
    assert_eq!(code(loans::create(&db, &bad, today).await.unwrap_err()), "INVALID_PAYER");
    let mut bad = input(&f);
    bad.schedule.as_mut().unwrap().insurance_category_id = None;
    assert_eq!(code(loans::create(&db, &bad, today).await.unwrap_err()), "INSURANCE_CATEGORY_REQUIRED");
    let income = category(&db, "Revenu test", CategoryKind::Income, None).await;
    let mut bad = input(&f);
    bad.schedule.as_mut().unwrap().interest_category_id = income;
    assert_eq!(code(loans::create(&db, &bad, today).await.unwrap_err()), "NOT_AN_EXPENSE_CATEGORY");
}

#[tokio::test]
async fn r_pre_5_manual_loan_follows_its_balances() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let manual = LoanInput { name: "Prêt familial".into(), schedule: None, ..input(&f) };
    let id = loans::create(&db, &manual, d("2026-03-31")).await.unwrap().summary.id;
    let balance = BalanceInput { date: d("2026-03-01"), outstanding_minor: 15_000_000, note: None };
    let view = loans::save_balance(&db, id, &balance, d("2026-03-31")).await.unwrap();
    assert_eq!(view.summary.outstanding_minor, Some(15_000_000));
    assert_eq!(view.balances.len(), 1);
    loans::delete_balance(&db, id, d("2026-03-01")).await.unwrap();
    assert_eq!(loans::get(&db, id, d("2026-03-31")).await.unwrap().summary.outstanding_minor, Some(20_000_000));
    // Un prêt avec tableau ne prend pas de saisie.
    let scheduled = create(&db, &f).await;
    assert_eq!(
        code(loans::save_balance(&db, scheduled, &balance, d("2026-03-31")).await.unwrap_err()),
        "LOAN_FOLLOWS_SCHEDULE"
    );
}

#[tokio::test]
async fn r_pre_3_preview_while_typing() {
    let preview = loans::preview(
        &LoanPreviewInput {
            principal_minor: 20_000_000,
            annual_rate_ppm: 30_000,
            duration_months: 240,
            insurance: Insurance::Fixed { amount_minor: 4_500 },
        },
        d("2026-03-31"),
    )
    .unwrap();
    assert_eq!((preview.monthly_payment_minor, preview.monthly_total_minor), (110_920, 115_420));
}

#[tokio::test]
async fn pre_03_r_pre_6_monthly_payments_are_split_into_repayment_interest_and_insurance() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let id = create(&db, &f).await;
    let first = spend(&db, f.checking, "2026-01-05", -115_920).await;
    // 40 centimes d'assurance en plus, payé deux jours plus tard : toujours reconnu (±1 €, ±5 jours).
    let second = spend(&db, f.checking, "2026-02-07", -115_960).await;
    spend(&db, f.checking, "2026-01-10", -5_000).await;

    let proposals = loans::proposals(&db, id, d("2026-02-28")).await.unwrap();
    assert_eq!(
        proposals.iter().map(|p| (p.transaction_id, p.installment_number)).collect::<Vec<_>>(),
        [(first, 1), (second, 2)]
    );
    let inputs: Vec<LoanPaymentInput> = proposals
        .iter()
        .map(|p| LoanPaymentInput { transaction_id: p.transaction_id, installment_number: p.installment_number })
        .collect();
    assert_eq!(loans::pay(&db, id, &inputs).await.unwrap().paid, 2);

    let detail = transactions::get(&db, first).await.unwrap();
    let mut parts: Vec<(i64, bool, bool)> =
        detail.splits.iter().map(|s| (s.amount_minor, s.category_id.is_some(), s.loan_id == Some(id))).collect();
    parts.sort_unstable();
    assert_eq!(parts, [(-60_920, false, true), (-50_000, true, false), (-5_000, true, false)]);
    assert_eq!(detail.loan_payment.as_ref().map(|p| p.installment_number), Some(1));
    // Seuls intérêts et assurance sont des dépenses (R-PRE-7).
    let january = super::dashboard::get(&db, d("2026-01-31")).await.unwrap();
    assert_eq!(january.month.expenses_minor, 55_000);
    assert_eq!(january.uncategorized.total, 1, "seule la dépense de 50 € reste à catégoriser");
    let view = loans::get(&db, id, d("2026-02-28")).await.unwrap();
    assert_eq!(view.schedule.unwrap().installments[0].paid_transaction_id, Some(first));
    assert_eq!(view.repayments.len(), 2);
    assert!(loans::proposals(&db, id, d("2026-02-28")).await.unwrap().is_empty());
    assert_eq!(code(loans::pay(&db, id, &inputs[..1]).await.unwrap_err()), "INSTALLMENT_ALREADY_PAID");
}

#[tokio::test]
async fn r_pre_8_a_paid_installment_is_locked_until_its_split_is_undone() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let id = create(&db, &f).await;
    let payment = spend(&db, f.checking, "2026-01-05", -115_920).await;
    let pay = [LoanPaymentInput { transaction_id: payment, installment_number: 1 }];
    loans::pay(&db, id, &pay).await.unwrap();

    let update = |amount: i64, label: &str| transactions::TransactionUpdateInput {
        account_id: f.checking,
        date: d("2026-01-05"),
        amount_minor: amount,
        label: label.into(),
        note: None,
        splits: None,
    };
    assert_eq!(
        code(transactions::update(&db, payment, &update(-120_000, "Échéance")).await.unwrap_err()),
        "INSTALLMENT_PAYMENT_LOCKED"
    );
    transactions::update(&db, payment, &update(-115_920, "Échéance de janvier")).await.unwrap();
    assert_eq!(transactions::get(&db, payment).await.unwrap().label, "Échéance de janvier");

    // Annuler la ventilation : la mensualité redevient une sortie non catégorisée.
    loans::unpay(&db, id, 1).await.unwrap();
    let detail = transactions::get(&db, payment).await.unwrap();
    assert_eq!(detail.splits.len(), 1);
    assert!(detail.loan_payment.is_none());

    // Supprimer une mensualité ventilée libère l'échéance.
    loans::pay(&db, id, &pay).await.unwrap();
    transactions::delete(&db, payment).await.unwrap();
    let view = loans::get(&db, id, d("2026-01-31")).await.unwrap();
    assert!(view.schedule.unwrap().installments[0].paid_transaction_id.is_none());
}

#[tokio::test]
async fn r_pre_11_deleting_a_loan_releases_its_payments_and_payer() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let id = create(&db, &f).await;
    let payment = spend(&db, f.checking, "2026-01-05", -115_920).await;
    loans::pay(&db, id, &[LoanPaymentInput { transaction_id: payment, installment_number: 1 }]).await.unwrap();
    // Une part ventilée à la main en remboursement (R-PRE-7).
    let manual = transactions::create(
        &db,
        &transactions::TransactionInput {
            splits: Some(vec![
                transactions::SplitInput { amount_minor: -1_000, loan_id: Some(id), ..Default::default() },
                split(-500, Some(f.interest)),
            ]),
            ..tx(f.checking, "2026-01-20", -1_500, None)
        },
    )
    .await
    .unwrap();
    assert_eq!(code(accounts::delete(&db, f.checking).await.unwrap_err()), "ACCOUNT_NOT_EMPTY");
    loans::delete(&db, id).await.unwrap();
    let detail = transactions::get(&db, payment).await.unwrap();
    assert_eq!((detail.splits.len(), detail.splits[0].category_id), (1, None));
    let detail = transactions::get(&db, manual).await.unwrap();
    assert!(detail.splits.iter().all(|s| s.loan_id.is_none()));
}

#[tokio::test]
async fn r_trx_37_a_split_has_one_attachment_at_most() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let id = create(&db, &f).await;
    let input = transactions::TransactionInput {
        splits: Some(vec![transactions::SplitInput {
            amount_minor: -1_000,
            category_id: Some(f.interest),
            loan_id: Some(id),
            ..Default::default()
        }]),
        ..tx(f.checking, "2026-01-20", -1_000, None)
    };
    assert_eq!(code(transactions::create(&db, &input).await.unwrap_err()), "SPLIT_ATTACHMENT_CONFLICT");
}

// ---------------------------------------------------------------- Projections (R-PRJ-10, R-PRJ-40)

#[tokio::test]
async fn r_prj_10_payer_cash_projection_counts_each_installment_once() {
    use super::projections::{self, CashQuery};
    use super::recurring::{self, RecurringRuleInput};
    use cashflow_domain::recurrence::Frequency;

    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    create(&db, &f).await;
    // Une récurrence du même montant que l'échéance, souvent saisie avant le prêt : ignorée.
    let duplicate = RecurringRuleInput {
        account_id: f.checking,
        target_account_id: None,
        label: "Prêt".into(),
        amount_minor: -115_920,
        amount_tolerance_minor: 0,
        frequency: Frequency::Monthly,
        interval: 1,
        anchor_date: d("2026-01-05"),
        end_date: None,
        split_template: Vec::new(),
        active: true,
    };
    recurring::save(&db, None, &duplicate, d("2026-01-31")).await.unwrap();
    let query = CashQuery { days: Some(60), variable: Some(false), threshold_minor: None };
    let view = projections::cash(&db, f.checking, &query, d("2026-01-31")).await.unwrap();
    let installments: Vec<(cashflow_domain::Date, i64)> =
        view.events.iter().filter(|e| e.amount_minor == -115_920).map(|e| (e.date, e.amount_minor)).collect();
    assert_eq!(installments, [(d("2026-02-05"), -115_920), (d("2026-03-05"), -115_920)]);
    assert!(view.events.iter().all(|e| e.label != "Prêt"));
}

#[tokio::test]
async fn r_prj_40_net_worth_projection_includes_assets_and_amortizing_loans() {
    use super::projections::{self, NetWorthQuery};

    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let asset = assets::create(&db, &flat(), d("2026-01-31")).await.unwrap().summary.id;
    let id = create(&db, &f).await;
    let today = d("2026-01-31");
    let worth = projections::net_worth(&db, &NetWorthQuery { months: Some(3) }, today).await.unwrap();
    // Départ : 1 000 € sur le compte, 250 000 € de bien, la dette après la première échéance.
    let schedule = loans::get(&db, id, today).await.unwrap().schedule.unwrap().installments;
    assert_eq!(worth.start_minor, 100_000 + 25_000_000 - schedule[0].remaining_minor);
    let default = worth.scenarios.iter().find(|s| s.is_default).unwrap();
    // La dette baisse au fil des échéances : le patrimoine projeté monte.
    assert!(default.points[2].value_minor > default.points[0].value_minor);
    assert!(assets::get(&db, asset, today).await.unwrap().summary.value_minor.is_some());
}

// ---------------------------------------------------------------- Biens

#[tokio::test]
async fn bie_01_assets_valuations_and_net_value() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let today = d("2026-03-31");
    let id: AssetId = assets::create(&db, &flat(), today).await.unwrap().summary.id;
    let loan = loans::create(&db, &LoanInput { asset_id: Some(id), ..input(&f) }, today).await.unwrap();
    let valuation = AssetValuationInput { date: d("2026-03-01"), value_minor: 26_000_000, note: None };
    let view = assets::save_valuation(&db, id, &valuation, today).await.unwrap();
    assert_eq!(view.summary.value_minor, Some(26_000_000));
    assert_eq!(view.loans.len(), 1);
    // Valeur nette : valeur − capital restant dû du prêt (R-PRE-10).
    assert_eq!(view.net_value_minor, Some(26_000_000 - loan.summary.outstanding_minor.unwrap()));
    let bad = AssetValuationInput { date: d("2025-01-01"), value_minor: 1, note: None };
    assert_eq!(code(assets::save_valuation(&db, id, &bad, today).await.unwrap_err()), "INVALID_ASSET_VALUATION");
    // Vendu, il ne compte plus (R-BIE-2).
    let sold = AssetInput { sold_on: Some(d("2026-03-15")), sale_price_minor: Some(27_000_000), ..flat() };
    assert_eq!(assets::update(&db, id, &sold, today).await.unwrap().summary.value_minor, None);
    assert_eq!(
        code(assets::update(&db, id, &AssetInput { purchase_price_minor: 0, ..flat() }, today).await.unwrap_err()),
        "INVALID_ASSET"
    );
}

#[tokio::test]
async fn bie_03_r_bie_8_acquisition_is_not_an_expense() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let today = d("2026-01-31");
    let id = assets::create(&db, &flat(), today).await.unwrap().summary.id;
    let before = super::dashboard::get(&db, today).await.unwrap().net_worth.total_minor;
    let apport = transactions::TransactionInput {
        splits: Some(vec![transactions::SplitInput {
            amount_minor: -50_000,
            asset_id: Some(id),
            ..Default::default()
        }]),
        ..tx(f.checking, "2026-01-10", -50_000, None)
    };
    transactions::create(&db, &apport).await.unwrap();
    let dashboard = super::dashboard::get(&db, today).await.unwrap();
    assert_eq!(dashboard.month.expenses_minor, 0);
    assert_eq!(dashboard.uncategorized.total, 0);
    // Le compte baisse de 500 €, le bien ne bouge pas : la variation n'est que la sortie d'argent.
    assert_eq!(dashboard.net_worth.total_minor, before - 50_000);
    let view = assets::get(&db, id, today).await.unwrap();
    assert_eq!(view.acquisitions_total_minor, 50_000);
    // Supprimer le bien détache ses acquisitions (R-BIE-6).
    assets::delete(&db, id).await.unwrap();
    assert_eq!(super::dashboard::get(&db, today).await.unwrap().uncategorized.total, 1);
}

// ---------------------------------------------------------------- Rubriques (R-BIE-7, R-PRE-12)

#[tokio::test]
async fn bie_02_pre_04_sections_turn_off_only_when_empty() {
    let (db, _dir) = setup().await;
    let f = fixture(&db).await;
    let initial = features::get(&db).await.unwrap();
    assert!(!initial.assets && !initial.loans, "désactivées sur une nouvelle installation");
    let on = FeaturesInput { assets: Some(true), loans: Some(true) };
    features::update(&db, &on).await.unwrap();
    let id = create(&db, &f).await;
    let off = FeaturesInput { assets: None, loans: Some(false) };
    assert_eq!(code(features::update(&db, &off).await.unwrap_err()), "FEATURE_IN_USE");
    loans::delete(&db, id).await.unwrap();
    assert!(!features::update(&db, &off).await.unwrap().loans);
    // Rubrique désactivée : aucune proposition de mensualité.
    let payment = spend(&db, f.checking, "2026-01-05", -115_920).await;
    create(&db, &f).await;
    assert!(loans::proposals_among(&db, &[payment]).await.unwrap().is_empty());
}
