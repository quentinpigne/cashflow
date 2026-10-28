//! Jeu de données de démonstration, entièrement fictif (ADR-0018).
//!
//! Généré de façon déterministe à partir de la date du jour, pour que la démonstration paraisse
//! toujours à jour. Il passe par les services, donc par toutes les règles. Il sert au développement
//! local, aux tests de bout en bout et à l'instance jetable, jamais à la production.

use std::collections::HashMap;

use anyhow::{Context, bail};
use cashflow_domain::accounts::AccountKind;
use cashflow_domain::assets::AssetKind;
use cashflow_domain::loans::{Insurance, LoanKind, LoanTerms};
use cashflow_domain::shared::ids::{AccountId, CategoryId, InstitutionId, LoanId, PayeeId};
use cashflow_domain::{Date, Money, Rate};
use chrono::{Datelike, Days, Months};

use super::transactions::{SplitInput, TransactionInput};
use super::{
    accounts, auth, budgets, categories, imports, institutions, payees, reconciliation, recurring, rules, savings,
    seed, transactions, transfers,
};
use crate::config::DataDir;
use crate::db::Db;

/// Mot de passe du compte de démonstration.
pub const DEMO_PASSWORD: &str = "demo-cashflow";
const MONTHS: u32 = 12;
const SALARY: i64 = 340_000;
/// Échéance du prêt de démonstration : mensualité de 1 109,20 € et assurance de 50 € (CPT-09).
const LOAN_INSTALLMENT: i64 = 115_920;
/// 13e mois net, versé en décembre.
const THIRTEENTH_MONTH: i64 = 320_000;

/// PDF d'une page, fictif, pour les fiches de paie (SAL-09) et les relevés (CPT-11) de démonstration.
fn fictitious_pdf(lines: &[String]) -> Vec<u8> {
    let text: String = lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("BT /F1 14 Tf 72 {} Td ({line}) Tj ET\n", 760 - 28 * i))
        .collect();
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_owned(),
        format!("<< /Length {} >>\nstream\n{text}endstream", text.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1));
    pdf.into_bytes()
}

/// Relevés fictifs du compte courant (CPT-11) : les cinq derniers mois, sauf l'avant-dernier, signalé sans
/// relevé.
async fn statements(db: &Db, data: &DataDir, checking: AccountId, today: Date) -> anyhow::Result<()> {
    let current = cashflow_domain::salaries::month_of(today);
    for back in [5, 4, 3, 1] {
        let month = current.checked_sub_months(Months::new(back)).context("mois du relevé")?;
        let upload = super::statements::StatementUpload {
            file_name: format!("releve-{}.pdf", month.format("%Y-%m")),
            content: fictitious_pdf(&[
                "RELEVE DE COMPTE FICTIF - DONNEES DE DEMONSTRATION".to_owned(),
                "Compte courant".to_owned(),
                format!("Mois : {}", month.format("%m/%Y")),
            ]),
            statement_month: month,
            note: None,
        };
        super::statements::add(db, data, checking, upload, today).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

/// L'emploi de démonstration : embauche quatre ans avant le début du jeu, deux augmentations, la dernière en
/// janvier de la première année du jeu, un 13e mois récurrent (spec 10), et les fiches de paie des quatre
/// derniers mois, sauf l'avant-dernier, signalé sans fiche (SAL-09).
async fn employment(
    db: &Db,
    data: &DataDir,
    c: &Categories,
    start: Date,
    thirteenth: Option<Date>,
    today: Date,
) -> anyhow::Result<()> {
    use super::salaries::{BonusInput, EmploymentInput, SalaryInput};
    use cashflow_domain::salaries::{BonusKind, SalaryChangeReason};
    let category = c.get("Salaires", "").context("catégorie Salaires absente")?;
    let hire = start.checked_sub_months(Months::new(48)).context("date d'embauche")?;
    let input = EmploymentInput {
        employer: "Acme Industries".into(),
        job_title: Some("Ingénieure d'études".into()),
        start_date: hire,
        end_date: None,
        income_category_id: category,
        raise_month: Some(1),
        paid_next_month: false,
        notes: None,
    };
    let id = super::salaries::create(db, &input, today).await.map_err(|e| anyhow::anyhow!("{e}"))?.summary.id;
    let january = |year: i32| Date::from_ymd_opt(year, 1, 1).context("janvier");
    let pays = [
        (hire, 4_400_000, 280_000, SalaryChangeReason::Hire),
        (january(start.year() - 2)?, 4_800_000, 310_000, SalaryChangeReason::Raise),
        (january(start.year())?, 5_200_000, SALARY, SalaryChangeReason::Promotion),
    ];
    for (month, gross, net, reason) in pays {
        let pay = SalaryInput {
            effective_month: month,
            annual_gross_minor: gross,
            monthly_net_minor: net,
            reason,
            note: None,
        };
        super::salaries::save_salary(db, id, None, &pay, today).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    if let Some(paid_on) = thirteenth {
        let bonus = BonusInput {
            label: "13e mois".into(),
            kind: BonusKind::ThirteenthMonth,
            paid_on,
            net_amount_minor: THIRTEENTH_MONTH,
            gross_amount_minor: Some(4_300_000 / 10),
            recurring: None,
            note: None,
        };
        super::salaries::save_bonus(db, id, None, &bonus, today).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    let current = cashflow_domain::salaries::month_of(today);
    for back in [1, 3, 4] {
        let month = current.checked_sub_months(Months::new(back)).context("mois de paie")?;
        let upload = super::salaries::PayslipUpload {
            file_name: format!("fiche-de-paie-{}.pdf", month.format("%Y-%m")),
            content: fictitious_pdf(&[
                "FICHE DE PAIE FICTIVE - DONNEES DE DEMONSTRATION".to_owned(),
                "Acme Industries".to_owned(),
                format!("Periode : {}", month.format("%m/%Y")),
                format!("Net a payer : {},{:02} EUR", SALARY / 100, SALARY % 100),
            ]),
            pay_month: month,
            note: None,
        };
        super::salaries::add_payslip(db, data, id, upload, today).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

/// L'appartement (acheté 250 000 €) et le prêt qui l'a financé, rubriques activées (spec 09).
async fn assets_and_loan(
    db: &Db,
    c: &Categories,
    terms: &LoanTerms,
    checking: AccountId,
    today: Date,
) -> anyhow::Result<LoanId> {
    super::features::update(
        db,
        &super::features::FeaturesInput {
            assets: Some(true),
            loans: Some(true),
            salaries: Some(true),
            salary_gap_tolerance_ppm: None,
            geocoding: None,
        },
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    let purchase = terms.first_installment.checked_sub_months(Months::new(1)).context("date d'achat")?;
    let flat = super::assets::AssetInput {
        name: "Appartement".into(),
        kind: AssetKind::RealEstate,
        purchase_date: purchase,
        // Prix d'achat : apport et capital emprunté (R-BIE-1).
        purchase_price_minor: 25_000_000,
        sold_on: None,
        sale_price_minor: None,
        include_in_net_worth: None,
        notes: None,
    };
    let flat = super::assets::create(db, &flat, today).await.map_err(|e| anyhow::anyhow!("{e}"))?.summary.id;
    let (Some(interest), Some(insurance)) = (c.get("Prêts", "Intérêts"), c.get("Prêts", "Assurance emprunteur"))
    else {
        bail!("catégories de prêt absentes");
    };
    let loan = super::loans::LoanInput {
        name: "Prêt immobilier".into(),
        kind: LoanKind::Mortgage,
        institution_id: institution(db, "LCL").await?,
        start_date: purchase,
        principal_minor: terms.principal.minor(),
        schedule: Some(super::loans::ScheduleInput {
            annual_rate_ppm: terms.annual_rate.ppm(),
            duration_months: terms.duration_months,
            first_installment_date: terms.first_installment,
            insurance: terms.insurance,
            payer_account_id: checking,
            interest_category_id: interest,
            insurance_category_id: Some(insurance),
        }),
        asset_id: Some(flat),
        closed_on: None,
        include_in_net_worth: None,
        notes: None,
    };
    Ok(super::loans::create(db, &loan, today).await.map_err(|e| anyhow::anyhow!("{e}"))?.summary.id)
}

/// Prêt de démonstration : 200 000 € à 3 % sur 20 ans, assurance 0,30 % du capital, première échéance
/// 84 mois avant le début du jeu.
fn demo_loan(start: Date) -> anyhow::Result<LoanTerms> {
    let first = start.with_day(5).and_then(|d| d.checked_sub_months(Months::new(84))).context("première échéance")?;
    Ok(LoanTerms {
        principal: Money::from_minor(20_000_000),
        annual_rate: Rate::from_ppm(30_000),
        duration_months: 240,
        first_installment: first,
        insurance: Insurance::RateOnPrincipal { rate_ppm: 3_000 },
    })
}

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
        payee_id: None,
    };
    transactions::create(db, &input).await.map_err(|e| anyhow::anyhow!("{label} le {date} : {e}"))?;
    Ok(())
}

/// Remplit une base vide avec le jeu de démonstration.
pub async fn load(db: &Db, data: &DataDir, today: Date) -> anyhow::Result<()> {
    if !accounts::list(db, today).await.map_err(|e| anyhow::anyhow!("{e}"))?.is_empty() {
        bail!("la base contient déjà des comptes : le jeu de démonstration se charge dans une base vide");
    }
    seed::run(db).await?;
    if !auth::password_is_set(db).await? {
        auth::set_password(db, "Démo", DEMO_PASSWORD).await?;
    }

    let start = (today - Months::new(MONTHS)).with_day(1).context("date de début")?;
    let checking = account(db, "Compte courant", AccountKind::Checking, Some("LCL"), start, 185_000, today).await?;
    statements(db, data, checking, today).await?;
    let wallet = account(db, "Espèces", AccountKind::CashWallet, None, start, 6_000, today).await?;
    let livret_a =
        account(db, "Livret A", AccountKind::LivretA, Some("La Banque Postale"), start, 450_000, today).await?;
    account(db, "LDDS", AccountKind::Ldds, Some("LCL"), start, 200_000, today).await?;
    let life =
        account(db, "Assurance vie", AccountKind::LifeInsurance, Some("Boursorama Banque"), start, 1_200_000, today)
            .await?;
    let pea = account(db, "PEA", AccountKind::Pea, Some("Boursorama Banque"), start, 650_000, today).await?;
    let loan_terms = demo_loan(start)?;

    let c = Categories::load(db).await?;
    let mut rng = Lcg(20_261_020);
    let mut month = start;
    let mut thirteenth = None;
    while month <= today {
        let day = |d: u32| month.with_day(d.min(28)).filter(|date| *date <= today);
        if let Some(d) = day(3) {
            spend(db, checking, d, -89_000, "LOYER APPARTEMENT", c.get("Logement", "Loyer")).await?;
        }
        if let Some(d) = day(5) {
            spend(db, checking, d, -LOAN_INSTALLMENT, "ECHEANCE PRET IMMO", c.get("Prêts", "")).await?;
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
            spend(db, checking, d, SALARY, "VIREMENT SALAIRE", c.get("Salaires", "")).await?;
        }
        if month.month() == 12
            && let Some(d) = day(20)
        {
            spend(db, checking, d, THIRTEENTH_MONTH, "PRIME 13E MOIS", c.get("Salaires", "")).await?;
            thirteenth = Some(d);
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
                        ..Default::default()
                    },
                    SplitInput {
                        amount_minor: -1_500,
                        category_id: c.get("Logement", "Autres"),
                        note: Some("Entretien".into()),
                        project_id: None,
                        ..Default::default()
                    },
                    SplitInput {
                        amount_minor: -1_000,
                        category_id: c.get("Achats & shopping", "Autres"),
                        note: Some("Cadeau".into()),
                        project_id: None,
                        ..Default::default()
                    },
                ]),
                payee_id: None,
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
    if let Some(streaming) = c.get("Abonnements", "Streaming (vidéo, musique)") {
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
    let loan = assets_and_loan(db, &c, &loan_terms, checking, today).await?;
    employment(db, data, &c, start, thirteenth, today).await?;
    // Chaque mensualité est ventilée selon son échéance : la dette baisse du capital remboursé (CPT-10).
    let payments: Vec<super::loans::LoanPaymentInput> = super::loans::proposals(db, loan, today)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .iter()
        .map(|p| super::loans::LoanPaymentInput {
            transaction_id: p.transaction_id,
            installment_number: p.installment_number,
        })
        .collect();
    super::loans::pay(db, loan, &payments).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    valuations(db, start, today, life, pea).await?;
    payees(db).await?;
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

/// Tiers et groupes de tiers (spec 11), affectés aux transactions déjà saisies d'après leur libellé ; une règle
/// affecte le tiers des stations-service (R-TIE-16).
async fn payees(db: &Db) -> anyhow::Result<()> {
    use super::payees::{AddressInput, PayeeGroupInput, PayeeInput};
    use cashflow_domain::import::rules::{Actions, Conditions, LabelCondition, LabelField, LabelOp};
    let err = |e: crate::error::AppError| anyhow::anyhow!("{e}");
    let mut groups = HashMap::new();
    for name in ["Carrefour", "TotalEnergies", "Santé"] {
        let input = PayeeGroupInput { name: name.into(), note: None };
        groups.insert(name, payees::create_group(db, &input).await.map_err(err)?);
    }
    let list = [
        ("Carrefour Meylan", Some("Carrefour")),
        ("Carrefour Market Grenoble", Some("Carrefour")),
        ("Biocoop Grenoble", None),
        ("TotalEnergies Meylan", Some("TotalEnergies")),
        ("Le Bistrot du Coin", None),
        ("Pizzeria Napoli", None),
        ("Marché de Meylan", None),
        ("Propriétaire", None),
        ("Free", None),
        ("EDF", None),
        ("Acme Industries", None),
        ("LCL", None),
        ("MAIF", None),
        ("Dr Martin", Some("Santé")),
        ("CPAM", Some("Santé")),
    ];
    // Adresses fictives, au centre de la ville (R-TIE-23) ; le médecin n'a pas été localisé (R-TIE-25).
    let addresses = HashMap::from([
        ("Carrefour Meylan", ("38240", "Meylan", Some((45_231_000, 5_781_000)))),
        ("Carrefour Market Grenoble", ("38000", "Grenoble", Some((45_188_000, 5_724_000)))),
        ("Biocoop Grenoble", ("38100", "Grenoble", Some((45_176_000, 5_731_000)))),
        ("TotalEnergies Meylan", ("38240", "Meylan", Some((45_226_000, 5_792_000)))),
        ("Le Bistrot du Coin", ("69002", "Lyon", Some((45_757_000, 4_832_000)))),
        ("Pizzeria Napoli", ("38000", "Grenoble", Some((45_191_000, 5_727_000)))),
        ("Marché de Meylan", ("38240", "Meylan", Some((45_233_000, 5_779_000)))),
        ("Dr Martin", ("38240", "Meylan", None)),
    ]);
    let mut ids = HashMap::new();
    for (name, group) in list {
        let address = addresses.get(name).map(|(postal_code, city, point)| AddressInput {
            postal_code: Some((*postal_code).into()),
            city: Some((*city).into()),
            country_code: Some("FR".into()),
            latitude_e6: point.map(|p| p.0),
            longitude_e6: point.map(|p| p.1),
            ..Default::default()
        });
        let input = PayeeInput {
            name: name.into(),
            group_id: group.and_then(|g| groups.get(g).copied()),
            archived: false,
            note: None,
            address,
        };
        ids.insert(name, payees::create(db, &input).await.map_err(err)?);
    }
    // Les supermarchés et les restaurants alternent d'un mois à l'autre.
    let rotations: [(&str, &[&str]); 3] = [
        ("CARTE SUPERMARCHE", &["Carrefour Meylan", "Carrefour Market Grenoble", "Biocoop Grenoble"]),
        ("CARTE RESTAURANT", &["Le Bistrot du Coin", "Pizzeria Napoli"]),
        ("CARREFOUR MARKET", &["Carrefour Market Grenoble"]),
    ];
    let fixed = [
        ("Marché", "Marché de Meylan"),
        ("LOYER APPARTEMENT", "Propriétaire"),
        ("BOX INTERNET", "Free"),
        ("FORFAIT MOBILE", "Free"),
        ("ENERGIE ELEC GAZ", "EDF"),
        ("VIREMENT SALAIRE", "Acme Industries"),
        ("PRIME 13E MOIS", "Acme Industries"),
        ("ECHEANCE PRET IMMO", "LCL"),
        ("ASSURANCE AUTO ECHEANCE", "MAIF"),
        ("MEDECIN GENERALISTE", "Dr Martin"),
        ("REMBOURSEMENT CPAM", "CPAM"),
    ];
    let all = transactions::list_all(db, &transactions::TransactionFilter { limit: Some(1000), ..Default::default() })
        .await
        .map_err(err)?;
    let mut chosen: HashMap<PayeeId, Vec<cashflow_domain::shared::ids::TransactionId>> = HashMap::new();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for t in all.items.iter().rev() {
        let name = if let Some((label, names)) = rotations.iter().find(|(l, _)| *l == t.label) {
            let n = seen.entry(label).or_default();
            *n += 1;
            Some(names[*n % names.len()])
        } else {
            fixed.iter().find(|(l, _)| *l == t.label).map(|(_, p)| *p)
        };
        if let Some(payee) = name.and_then(|n| ids.get(n)) {
            chosen.entry(*payee).or_default().push(t.id);
        }
    }
    for (payee, transaction_ids) in chosen {
        let input = transactions::BulkPayeeInput { transaction_ids, payee_id: Some(payee) };
        transactions::set_payee_many(db, &input).await.map_err(err)?;
    }
    let station = rules::RuleInput {
        name: "Station-service".into(),
        priority: 10,
        conditions: Conditions {
            label: Some(LabelCondition {
                op: LabelOp::Contains,
                value: "STATION SERVICE".into(),
                field: LabelField::Label,
            }),
            ..Default::default()
        },
        actions: Actions { set_payee: ids.get("TotalEnergies Meylan").copied(), ..Default::default() },
        stop_processing: false,
        active: true,
    };
    let rule = rules::save(db, None, &station).await.map_err(err)?;
    let matches =
        rules::preview(db, &station.conditions, &rules::PreviewQuery { sets_payee: Some(true) }).await.map_err(err)?;
    let apply = rules::ApplyInput { transaction_ids: matches.into_iter().map(|m| m.transaction_id).collect() };
    rules::apply(db, rule.id, &apply).await.map_err(err)?;
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
    let payees: HashMap<String, PayeeId> = payees::list(db)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .payees
        .into_iter()
        .map(|p| (p.name, p.id))
        .collect();
    let payee_of = |label: &str| {
        let name = match label {
            "Loyer" => "Propriétaire",
            "Box internet" | "Forfait mobile" => "Free",
            "Électricité et gaz" => "EDF",
            "Salaire" => "Acme Industries",
            _ => return None,
        };
        payees.get(name).copied()
    };
    let rules = [
        ("Loyer", 3, -89_000, c.get("Logement", "Loyer"), None, 0),
        ("Box internet", 6, -2_999, c.get("Abonnements", "Internet"), None, 0),
        ("Forfait mobile", 15, -1_299, c.get("Abonnements", "Téléphone"), None, 0),
        ("Électricité et gaz", 10, -8_000, c.get("Logement", "Gaz, électricité, eau"), None, 1_700),
        ("Salaire", 27, SALARY, c.get("Salaires", ""), None, 0),
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
            payee_id: payee_of(label),
        };
        recurring::save(db, None, &input, today).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

/// Valorisations trimestrielles (CPT-06) : l'assurance vie progresse d'environ 3 % par an, le PEA de 6 % avec
/// un trimestre de baisse.
async fn valuations(db: &Db, start: Date, today: Date, life: AccountId, pea: AccountId) -> anyhow::Result<()> {
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
        for (account, value) in [(life, life_value), (pea, pea_value)] {
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
        line(BudgetKind::Income, c.get("Salaires", ""), SALARY, false),
        // Budget fixe : pas d'alerte (R-BUD-60).
        budgets::BudgetLineInput {
            alert_thresholds: Some(Vec::new()),
            ..line(BudgetKind::Expense, c.get("Logement", "Loyer"), 89_000, false)
        },
        line(BudgetKind::Expense, c.get("Logement", "Gaz, électricité, eau"), 9_000, false),
        line(BudgetKind::Expense, c.get("Alimentation & restauration", "Supermarché / épicerie"), 41_000, true),
        line(BudgetKind::Expense, c.get("Alimentation & restauration", "Restaurants"), 8_000, false),
        line(BudgetKind::Expense, c.get("Auto & transports", ""), 8_000, false),
        line(BudgetKind::Expense, c.get("Abonnements", ""), 5_000, false),
        // Budget fixe : pas d'alerte (R-BUD-60).
        budgets::BudgetLineInput {
            alert_thresholds: Some(Vec::new()),
            // Intérêts et assurance seulement : le capital rembourse la dette (CPT-10).
            ..line(BudgetKind::Expense, c.get("Prêts", ""), 39_000, false)
        },
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
                ..Default::default()
            }]),
            payee_id: None,
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
        let (db, dir) = crate::db::test_db().await;
        let data = DataDir::new(dir.path());
        let today: Date = "2026-10-20".parse().unwrap();
        load(&db, &data, today).await.unwrap();
        let accounts = accounts::list(&db, today).await.unwrap();
        assert_eq!(accounts.len(), 6);
        // Les comptes d'actif restent positifs : le jeu reste vraisemblable.
        for account in &accounts {
            assert!(account.balance_minor > 0, "{} : {}", account.name, account.balance_minor);
        }
        let dashboard = super::super::dashboard::get(&db, today).await.unwrap();
        assert!(dashboard.uncategorized.total >= 3);
        assert!(dashboard.month.income_minor >= 0);
        // Les virements ne changent pas le patrimoine : il ne dépend que des flux catégorisés et non catégorisés.
        let budget = budgets::month(&db, &budgets::MonthQuery { month: None }, today).await.unwrap();
        assert!(budget.lines.len() >= 10);
        assert_eq!(budgets::projects(&db).await.unwrap()[0].actual_minor, 40_200);
        // Les mensualités de la démo sont ventilées selon leurs échéances (PRE-03).
        let loan = &super::super::loans::list(&db, today).await.unwrap()[0];
        let view = super::super::loans::get(&db, loan.id, today).await.unwrap();
        let schedule = view.schedule.unwrap();
        assert!(
            schedule
                .installments
                .iter()
                .filter(|i| i.date <= today && i.number >= 85)
                .all(|i| i.paid_transaction_id.is_some())
        );
        assert!(dashboard.net_worth.total_minor > 0, "le bien compense le prêt (R-TDB-1)");
        // Fiches de paie fictives : juillet, juin et septembre ; août est sans fiche (SAL-09).
        let job = &super::super::salaries::overview(&db, today).await.unwrap().employments[0];
        let job = super::super::salaries::get(&db, job.id, today).await.unwrap();
        assert_eq!(job.payslips.len(), 3);
        assert_eq!(job.missing_payslip_months, ["2026-08-01".parse::<Date>().unwrap()]);
        // Relevés fictifs du compte courant : août est sans relevé (CPT-11).
        let checking = accounts.iter().find(|a| a.name == "Compte courant").unwrap().id;
        let statements = super::super::statements::list(&db, checking, today).await.unwrap();
        assert_eq!(statements.statements.len(), 4);
        assert_eq!(statements.missing_months, ["2026-08-01".parse::<Date>().unwrap()]);
        assert!(load(&db, &data, today).await.is_err(), "le jeu ne se charge que dans une base vide");
    }
}
