//! Jeu de données de démonstration, entièrement fictif (ADR-0018).
//!
//! Généré de façon déterministe à partir de la date du jour, pour que la démonstration paraisse
//! toujours à jour. Il passe par les services, donc par toutes les règles. Il sert au développement
//! local, aux tests de bout en bout et à l'instance jetable, jamais à la production.

use std::collections::HashMap;

use anyhow::{Context, bail};
use cashflow_domain::Date;
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::shared::ids::{AccountId, CategoryId, InstitutionId};
use chrono::{Datelike, Days, Months};

use super::transactions::{SplitInput, TransactionInput};
use super::{
    accounts, auth, budgets, categories, imports, institutions, reconciliation, recurring, rules, savings, seed,
    transactions, transfers,
};
use crate::db::Db;

/// Mot de passe du compte de démonstration.
pub const DEMO_PASSWORD: &str = "demo-cashflow";
const MONTHS: u32 = 12;

/// Générateur pseudo-aléatoire minimal, pour un jeu reproductible.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// Entier dans [min, max].
    fn between(&mut self, min: i64, max: i64) -> i64 {
        let span = u64::try_from(max - min + 1).unwrap_or(1);
        min + i64::try_from(self.next() % span).unwrap_or(0)
    }
}

struct Categories(HashMap<(String, String), CategoryId>);

impl Categories {
    async fn load(db: &Db) -> anyhow::Result<Self> {
        let mut map = HashMap::new();
        for root in categories::tree(db).await? {
            map.insert((root.name.clone(), String::new()), root.id);
            for child in &root.children {
                map.insert((root.name.clone(), child.name.clone()), child.id);
            }
        }
        Ok(Self(map))
    }

    fn get(&self, parent: &str, child: &str) -> Option<CategoryId> {
        self.0.get(&(parent.to_owned(), child.to_owned())).copied()
    }
}

async fn institution(db: &Db, name: &str) -> anyhow::Result<Option<InstitutionId>> {
    Ok(institutions::list(db).await?.into_iter().find(|i| i.name == name).map(|i| i.id))
}

async fn account(
    db: &Db,
    name: &str,
    kind: AccountKind,
    bank: Option<&str>,
    opening: Date,
    balance: i64,
    today: Date,
) -> anyhow::Result<AccountId> {
    let institution_id = match bank {
        Some(bank) => institution(db, bank).await?,
        None => None,
    };
    let input = accounts::AccountInput {
        name: name.into(),
        kind,
        institution_id,
        currency: None,
        opening_date: opening,
        opening_balance_minor: balance,
        iban: None,
        account_number: None,
        include_in_net_worth: None,
        include_in_emergency_fund: None,
        include_in_budget: None,
        display_order: None,
        color: None,
        notes: None,
    };
    Ok(accounts::create(db, &input, today).await.map_err(|e| anyhow::anyhow!("{e}"))?.id)
}

async fn spend(
    db: &Db,
    account: AccountId,
    date: Date,
    amount: i64,
    label: &str,
    category: Option<CategoryId>,
) -> anyhow::Result<()> {
    let input = TransactionInput {
        account_id: account,
        date,
        amount_minor: amount,
        label: label.into(),
        note: None,
        category_id: category,
        splits: None,
    };
    transactions::create(db, &input).await.map_err(|e| anyhow::anyhow!("{label} le {date} : {e}"))?;
    Ok(())
}

/// Remplit une base vide avec le jeu de démonstration.
pub async fn load(db: &Db, today: Date) -> anyhow::Result<()> {
    if !accounts::list(db, today).await.map_err(|e| anyhow::anyhow!("{e}"))?.is_empty() {
        bail!("la base contient déjà des comptes : le jeu de démonstration se charge dans une base vide");
    }
    seed::run(db).await?;
    if !auth::password_is_set(db).await? {
        auth::set_password(db, "Démo", DEMO_PASSWORD).await?;
    }

    let start = (today - Months::new(MONTHS)).with_day(1).context("date de début")?;
    let checking = account(db, "Compte courant", AccountKind::Checking, Some("LCL"), start, 185_000, today).await?;
    let wallet = account(db, "Espèces", AccountKind::CashWallet, None, start, 6_000, today).await?;
    let livret_a =
        account(db, "Livret A", AccountKind::LivretA, Some("La Banque Postale"), start, 450_000, today).await?;
    account(db, "LDDS", AccountKind::Ldds, Some("LCL"), start, 200_000, today).await?;
    let life =
        account(db, "Assurance vie", AccountKind::LifeInsurance, Some("Boursorama Banque"), start, 1_200_000, today)
            .await?;
    let pea = account(db, "PEA", AccountKind::Pea, Some("Boursorama Banque"), start, 650_000, today).await?;
    let loan = account(db, "Prêt immobilier", AccountKind::Mortgage, Some("LCL"), start, -14_500_000, today).await?;

    let c = Categories::load(db).await?;
    let mut rng = Lcg(20_261_020);
    let mut month = start;
    while month <= today {
        let day = |d: u32| month.with_day(d.min(28)).filter(|date| *date <= today);
        if let Some(d) = day(3) {
            spend(db, checking, d, -89_000, "LOYER APPARTEMENT", c.get("Logement", "Loyer")).await?;
        }
        if let Some(d) = day(5) {
            spend(db, checking, d, -72_000, "ECHEANCE PRET IMMO", c.get("Prêts", "")).await?;
        }
        if let Some(d) = day(6) {
            spend(db, checking, d, -2_999, "BOX INTERNET", c.get("Abonnements", "Internet")).await?;
        }
        if let Some(d) = day(10) {
            spend(
                db,
                checking,
                d,
                -rng.between(6_500, 9_800),
                "ENERGIE ELEC GAZ",
                c.get("Logement", "Gaz, électricité, eau"),
            )
            .await?;
        }
        if let Some(d) = day(15) {
            spend(db, checking, d, -1_299, "FORFAIT MOBILE", c.get("Abonnements", "Téléphone")).await?;
        }
        if let Some(d) = day(27) {
            spend(db, checking, d, 285_000, "VIREMENT SALAIRE", c.get("Salaires", "")).await?;
        }
        // Courses, restaurants, carburant : fréquences et montants variables.
        for week in 0..4 {
            if let Some(d) = day(2 + week * 7) {
                spend(
                    db,
                    checking,
                    d,
                    -rng.between(4_500, 12_000),
                    "CARTE SUPERMARCHE",
                    c.get("Alimentation & restauration", "Supermarché / épicerie"),
                )
                .await?;
            }
        }
        for _ in 0..rng.between(1, 3) {
            if let Some(d) = day(u32::try_from(rng.between(8, 26)).unwrap_or(8)) {
                spend(
                    db,
                    checking,
                    d,
                    -rng.between(1_800, 6_500),
                    "CARTE RESTAURANT",
                    c.get("Alimentation & restauration", "Restaurants"),
                )
                .await?;
            }
        }
        if let Some(d) = day(u32::try_from(rng.between(12, 22)).unwrap_or(12)) {
            spend(
                db,
                checking,
                d,
                -rng.between(5_500, 8_000),
                "CARTE STATION SERVICE",
                c.get("Auto & transports", "Carburant"),
            )
            .await?;
        }
        if let Some(d) = day(20) {
            spend(db, wallet, d, -rng.between(500, 3_000), "Marché", c.get("Alimentation & restauration", "Autres"))
                .await?;
        }
        // Une transaction ventilée par mois (spec 02, exemple de la ventilation).
        if let Some(d) = day(18) {
            let input = TransactionInput {
                account_id: checking,
                date: d,
                amount_minor: -12_000,
                label: "CARREFOUR MARKET".into(),
                note: None,
                category_id: None,
                splits: Some(vec![
                    SplitInput {
                        amount_minor: -9_500,
                        category_id: c.get("Alimentation & restauration", "Supermarché / épicerie"),
                        note: None,
                        project_id: None,
                    },
                    SplitInput {
                        amount_minor: -1_500,
                        category_id: c.get("Logement", "Autres"),
                        note: Some("Entretien".into()),
                        project_id: None,
                    },
                    SplitInput {
                        amount_minor: -1_000,
                        category_id: c.get("Achats & shopping", "Autres"),
                        note: Some("Cadeau".into()),
                        project_id: None,
                    },
                ]),
            };
            transactions::create(db, &input).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        // L'assurance auto, payée une fois par an : suivie par une ligne annuelle (BUD-05).
        if month.month() == 3
            && let Some(d) = day(14)
        {
            spend(db, checking, d, -68_000, "ASSURANCE AUTO ECHEANCE", c.get("Assurances", "Auto")).await?;
        }
        // Remboursement de santé : vient en déduction des dépenses (R-TRX-13).
        if month.month() % 3 == 0
            && let Some(d) = day(22)
        {
            spend(db, checking, d, -5_000, "MEDECIN GENERALISTE", c.get("Santé", "Médecin")).await?;
            if let Some(refund) = day(25) {
                spend(db, checking, refund, 3_500, "REMBOURSEMENT CPAM", c.get("Santé", "Médecin")).await?;
            }
        }
        // Virements internes : un vers le livret, un vers l'assurance vie (jambe CLEARED, R-TRX-36).
        if let Some(d) = day(2) {
            let input = transfers::TransferInput {
                from_account_id: checking,
                to_account_id: livret_a,
                amount_minor: 20_000,
                date: d,
                label: "VIR EPARGNE LIVRET A".into(),
                note: None,
            };
            transfers::create(db, &input).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        if let Some(d) = day(16) {
            spend(db, checking, d, -10_000, "VERSEMENT ASSURANCE VIE", None).await?;
        }
        // Retrait au distributeur : un virement vers le compte espèces, qui l'alimente.
        if let Some(d) = day(12) {
            let input = transfers::TransferInput {
                from_account_id: checking,
                to_account_id: wallet,
                amount_minor: 4_000,
                date: d,
                label: "RETRAIT DAB".into(),
                note: None,
            };
            transfers::create(db, &input).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        month = month.checked_add_months(Months::new(1)).context("mois suivant")?;
    }

    // Les versements sur l'assurance vie deviennent des virements avec jambe opposée créée.
    let page = transactions::list(
        db,
        checking,
        &transactions::TransactionFilter {
            text: Some("VERSEMENT ASSURANCE VIE".into()),
            limit: Some(1000),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    for item in page.items {
        let input = transfers::MarkAsTransferInput {
            counterpart_account_id: life,
            counterpart_transaction_id: None,
            confirm: true,
        };
        transfers::mark_as_transfer(db, item.id, &input).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    // Les transactions de plus de 10 jours sont pointées, comme après des imports ; les récentes
    // restent en attente (RAP-07).
    let pointed_until = today - Days::new(10);
    for account in accounts::list(db, today).await.map_err(|e| anyhow::anyhow!("{e}"))? {
        let page = transactions::list(
            db,
            account.id,
            &transactions::TransactionFilter { limit: Some(1000), ..Default::default() },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        for item in page.items.iter().filter(|i| i.balance_date <= pointed_until) {
            reconciliation::set_cleared(db, item.id, &reconciliation::ClearInput { cleared: true })
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }

    // Un rapprochement à la fin de l'avant-dernier mois (spec 04).
    let reconciled_at = today
        .checked_sub_months(Months::new(1))
        .and_then(|d| d.with_day(1))
        .and_then(|d| d.pred_opt())
        .context("date de rapprochement")?;
    if reconciled_at >= start {
        let balance =
            accounts::get(db, checking, reconciled_at).await.map_err(|e| anyhow::anyhow!("{e}"))?.balance_minor;
        let point = reconciliation::create(
            db,
            checking,
            &reconciliation::CheckpointInput { date: reconciled_at, balance_minor: balance, replace: false },
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        reconciliation::reconcile(db, point.id).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    // Une règle et un profil d'import, pour le relevé fictif de `fixtures/demo` (IMP-02, IMP-08).
    if let Some(streaming) = c.get("Abonnements", "Autres") {
        let rule = rules::RuleInput {
            name: "Netflix".into(),
            priority: 0,
            conditions: cashflow_domain::import::rules::Conditions {
                label: Some(cashflow_domain::import::rules::LabelCondition {
                    op: cashflow_domain::import::rules::LabelOp::Contains,
                    value: "NETFLIX".into(),
                    field: cashflow_domain::import::rules::LabelField::BankLabel,
                }),
                ..Default::default()
            },
            actions: cashflow_domain::import::rules::Actions {
                set_label: Some("Netflix".into()),
                set_splits: vec![cashflow_domain::import::rules::SplitShare {
                    category_id: Some(streaming),
                    weight: 1,
                }],
                ..Default::default()
            },
            stop_processing: true,
            active: true,
        };
        rules::save(db, None, &rule).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    let settings = serde_json::json!({
        "header_row": 1,
        "date": { "column": "Date", "format": "dd/MM/yyyy" },
        "label": { "columns": ["Libellé"] },
        "amount": { "mode": "debit_credit", "debit_column": "Débit", "credit_column": "Crédit", "debit_is_positive": true }
    });
    let profile = imports::ProfileInput {
        name: "Banque démo (CSV)".into(),
        format: crate::readers::FileFormat::Csv,
        settings: Some(serde_json::from_value(settings)?),
        default_account_id: Some(checking),
    };
    imports::save_profile(db, None, &profile).await.map_err(|e| anyhow::anyhow!("{e}"))?;

    budget(db, &c, start, today, livret_a, life, checking).await?;
    valuations(db, start, today, life, pea, loan).await?;
    recurrences(db, &c, start, today, checking, livret_a, life).await?;

    // Quelques dépenses récentes à catégoriser, pour l'accueil (R-TDB-5).
    for (offset, amount, label) in
        [(1, -2_390, "CB AMAZON MARKETPLACE"), (2, -1_450, "CB BOULANGERIE"), (4, -6_800, "CB DECATHLON")]
    {
        let date = today.checked_sub_days(Days::new(offset)).filter(|d| *d >= start);
        if let Some(date) = date {
            spend(db, checking, date, amount, label, None).await?;
        }
    }
    Ok(())
}

/// Les échéances régulières du compte courant (TRX-07), qui alimentent la projection de trésorerie.
async fn recurrences(
    db: &Db,
    c: &Categories,
    start: Date,
    today: Date,
    checking: AccountId,
    livret_a: AccountId,
    life: AccountId,
) -> anyhow::Result<()> {
    use cashflow_domain::recurrence::Frequency;
    let rules = [
        ("Loyer", 3, -89_000, c.get("Logement", "Loyer"), None, 0),
        ("Échéance prêt immobilier", 5, -72_000, c.get("Prêts", ""), None, 0),
        ("Box internet", 6, -2_999, c.get("Abonnements", "Internet"), None, 0),
        ("Forfait mobile", 15, -1_299, c.get("Abonnements", "Téléphone"), None, 0),
        ("Électricité et gaz", 10, -8_000, c.get("Logement", "Gaz, électricité, eau"), None, 1_700),
        ("Salaire", 27, 285_000, c.get("Salaires", ""), None, 0),
        ("Épargne Livret A", 2, -20_000, None, Some(livret_a), 0),
        ("Versement assurance vie", 16, -10_000, None, Some(life), 0),
    ];
    for (label, day, amount, category, target, tolerance) in rules {
        let input = recurring::RecurringRuleInput {
            account_id: checking,
            target_account_id: target,
            label: label.into(),
            amount_minor: amount,
            amount_tolerance_minor: tolerance,
            frequency: Frequency::Monthly,
            interval: 1,
            anchor_date: start.with_day(day).context("jour d'échéance")?,
            end_date: None,
            split_template: if target.is_some() {
                Vec::new()
            } else {
                vec![crate::db::recurring::TemplateSplit { category_id: category, amount_minor: amount }]
            },
            active: true,
        };
        recurring::save(db, None, &input, today).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

/// Valorisations trimestrielles (CPT-06) : l'assurance vie progresse d'environ 3 % par an, le PEA de 6 % avec
/// un trimestre de baisse, le prêt s'amortit (R-CPT-12).
async fn valuations(
    db: &Db,
    start: Date,
    today: Date,
    life: AccountId,
    pea: AccountId,
    loan: AccountId,
) -> anyhow::Result<()> {
    let mut quarter = 0_i64;
    let pea_moves = [15, 12, -20, 18];
    let mut pea_value = 650_000_i64;
    loop {
        quarter += 1;
        let Some(next) = start.checked_add_months(Months::new(u32::try_from(quarter * 3).unwrap_or(3))) else {
            break;
        };
        let date = next.pred_opt().context("fin de trimestre")?;
        if date > today {
            break;
        }
        // Valeur de l'assurance vie : capital versé à date, plus 0,75 % par trimestre écoulé.
        let invested = accounts::get(db, life, date).await.map_err(|e| anyhow::anyhow!("{e}"))?.balance_minor;
        let life_value = invested + invested * 75 * quarter / 10_000;
        pea_value += pea_value * pea_moves[usize::try_from((quarter - 1) % 4).unwrap_or(0)] / 1_000;
        for (account, value) in [(life, life_value), (pea, pea_value), (loan, -14_500_000 + 150_000 * quarter)] {
            let input = savings::ValuationInput { date, value_minor: value, note: None, replace: true };
            savings::save_valuation(db, account, &input).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }
    Ok(())
}

/// Un budget complet (spec 05) : lignes mensuelles, une ligne annuelle, deux objectifs d'épargne et un projet.
async fn budget(
    db: &Db,
    c: &Categories,
    start: Date,
    today: Date,
    livret_a: AccountId,
    life: AccountId,
    checking: AccountId,
) -> anyhow::Result<()> {
    use cashflow_domain::budgeting::{BudgetKind, Month, Periodicity};
    let from = Month::of(start).to_string();
    let line = |kind, category: Option<CategoryId>, amount_minor, rollover| budgets::BudgetLineInput {
        kind,
        category_id: category,
        target_account_id: None,
        periodicity: Periodicity::Monthly,
        year_start_month: 1,
        amount_minor,
        rollover,
        from: from.clone(),
        alert_thresholds: None,
    };
    let mut lines = vec![
        line(BudgetKind::Income, c.get("Salaires", ""), 285_000, false),
        line(BudgetKind::Expense, c.get("Logement", "Loyer"), 89_000, false),
        line(BudgetKind::Expense, c.get("Logement", "Gaz, électricité, eau"), 9_000, false),
        line(BudgetKind::Expense, c.get("Alimentation & restauration", "Supermarché / épicerie"), 41_000, true),
        line(BudgetKind::Expense, c.get("Alimentation & restauration", "Restaurants"), 8_000, false),
        line(BudgetKind::Expense, c.get("Auto & transports", ""), 8_000, false),
        line(BudgetKind::Expense, c.get("Abonnements", ""), 5_000, false),
        line(BudgetKind::Expense, c.get("Prêts", ""), 72_000, false),
        budgets::BudgetLineInput {
            periodicity: Periodicity::Yearly,
            ..line(BudgetKind::Expense, c.get("Assurances", "Auto"), 72_000, false)
        },
    ];
    for (account, amount) in [(livret_a, 20_000), (life, 15_000)] {
        lines.push(budgets::BudgetLineInput {
            target_account_id: Some(account),
            ..line(BudgetKind::Savings, None, amount, false)
        });
    }
    for input in lines.iter().filter(|l| l.category_id.is_some() || l.target_account_id.is_some()) {
        budgets::create_line(db, input).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    // Un week-end récent, suivi comme projet et retiré des lignes du mois (R-BUD-21).
    let (Some(departure), Some(back)) = (today.checked_sub_days(Days::new(20)), today.checked_sub_days(Days::new(17)))
    else {
        return Ok(());
    };
    if departure < start {
        return Ok(());
    }
    let project = budgets::ProjectInput {
        name: "Week-end à Lyon".into(),
        start_date: departure,
        end_date: Some(back),
        amount_minor: 60_000,
        exclude_from_period_budgets: true,
        active: true,
        alert_thresholds: None,
        note: None,
    };
    let project = budgets::save_project(db, None, &project).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    for (date, amount, label, category) in [
        (departure, -9_800, "SNCF CONNECT", c.get("Voyages", "Transport")),
        (departure, -24_000, "HOTEL LYON PRESQU ILE", c.get("Voyages", "Hébergement")),
        (back, -6_400, "BOUCHON LYONNAIS", c.get("Alimentation & restauration", "Restaurants")),
    ] {
        let input = TransactionInput {
            account_id: checking,
            date,
            amount_minor: amount,
            label: label.into(),
            note: None,
            category_id: None,
            splits: Some(vec![SplitInput {
                amount_minor: amount,
                category_id: category,
                note: None,
                project_id: Some(project.id),
            }]),
        };
        transactions::create(db, &input).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    budgets::evaluate_alerts(db, today).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn demo_loads_and_balances_are_consistent() {
        let (db, _dir) = crate::db::test_db().await;
        let today: Date = "2026-10-20".parse().unwrap();
        load(&db, today).await.unwrap();
        let accounts = accounts::list(&db, today).await.unwrap();
        assert_eq!(accounts.len(), 7);
        // Les comptes d'actif restent positifs : le jeu reste vraisemblable.
        for account in accounts.iter().filter(|a| a.kind != AccountKind::Mortgage) {
            assert!(account.balance_minor > 0, "{} : {}", account.name, account.balance_minor);
        }
        let dashboard = super::super::dashboard::get(&db, today).await.unwrap();
        assert!(dashboard.uncategorized.total >= 3);
        assert!(dashboard.month.income_minor >= 0);
        // Les virements ne changent pas le patrimoine : il ne dépend que des flux catégorisés et non catégorisés.
        let budget = budgets::month(&db, &budgets::MonthQuery { month: None }, today).await.unwrap();
        assert!(budget.lines.len() >= 10);
        assert_eq!(budgets::projects(&db).await.unwrap()[0].actual_minor, 40_200);
        assert!(load(&db, today).await.is_err(), "le jeu ne se charge que dans une base vide");
    }
}
