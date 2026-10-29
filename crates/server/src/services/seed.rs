//! Données initiales (architecture.md § Migrations) : distinctes des migrations, idempotentes, sans
//! aucune donnée personnelle.
//!
//! - Barèmes réglementés : complétés à chaque démarrage (nouvelles périodes de taux).
//! - Établissements et catégories : proposés **au premier lancement** seulement (R-TRX-23), pour ne
//!   jamais recréer ce que l'utilisateur a supprimé ou renommé.
//! - Catégorie « Sans catégorie », hors analyses : proposée une fois, y compris sur une base déjà en service (R-TRX-25).

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::shared::ids::{CategoryId, InstitutionId, RateScheduleId};
use cashflow_domain::{Date, Rate};
use serde::Deserialize;

use crate::db::categories::CategoryFields;
use crate::db::institutions::InstitutionFields;
use crate::db::{self, Db};

/// Catégorie de revenu des intérêts générés (R-EPA-10), reliée aux conditions d'épargne.
pub const INTEREST_CATEGORY: &str = "Intérêts";

/// Catégorie hors analyses proposée, en dépense : rien n'y est compté, sa nature est sans effet (R-TRX-25).
pub const EXCLUDED_CATEGORY: &str = "Sans catégorie";

const CATEGORIES: &str = include_str!("../../seed/categories.json");
const INSTITUTIONS: &str = include_str!("../../seed/institutions.json");
const RATES: &str = include_str!("../../seed/rates.json");

#[derive(Deserialize)]
struct CategorySeed {
    expense: Vec<CategoryNode>,
    income: Vec<CategoryNode>,
}

#[derive(Deserialize)]
struct CategoryNode {
    name: String,
    children: Vec<String>,
}

#[derive(Deserialize)]
struct InstitutionSeed {
    institutions: Vec<InstitutionEntry>,
}

#[derive(Deserialize)]
struct InstitutionEntry {
    name: String,
    bank_code: Option<String>,
}

#[derive(Deserialize)]
struct RateSeed {
    schedules: Vec<ScheduleEntry>,
}

#[derive(Deserialize)]
struct ScheduleEntry {
    name: String,
    periods: Vec<PeriodEntry>,
}

#[derive(Deserialize)]
struct PeriodEntry {
    effective_from: Date,
    annual_rate_ppm: i64,
}

pub async fn run(db: &Db) -> anyhow::Result<()> {
    let mut tx = db.write.begin().await?;

    let rates: RateSeed = serde_json::from_str(RATES)?;
    for schedule in rates.schedules {
        let id = match db::accounts::find_schedule_by_name(&mut *tx, &schedule.name).await? {
            Some(id) => id,
            None => {
                let id = RateScheduleId::new();
                db::accounts::insert_schedule(&mut *tx, id, &schedule.name, true).await?;
                id
            }
        };
        for period in schedule.periods {
            db::accounts::insert_rate_period_if_missing(
                &mut *tx,
                id,
                period.effective_from,
                Rate::from_ppm(period.annual_rate_ppm),
            )
            .await?;
        }
    }

    if db::settings::get::<bool>(&mut *tx, "seed.institutions").await?.is_none() {
        let seed: InstitutionSeed = serde_json::from_str(INSTITUTIONS)?;
        for entry in seed.institutions {
            if !db::institutions::name_taken(&mut *tx, &entry.name, None).await? {
                let fields = InstitutionFields {
                    name: &entry.name,
                    bank_code: entry.bank_code.as_deref(),
                    bic: None,
                    archived: false,
                };
                db::institutions::insert(&mut *tx, InstitutionId::new(), &fields).await?;
            }
        }
        db::settings::set(&mut *tx, "seed.institutions", &true).await?;
    }

    if db::settings::get::<bool>(&mut *tx, "seed.categories").await?.is_none() {
        if db::categories::count(&mut *tx).await? == 0 {
            let seed: CategorySeed = serde_json::from_str(CATEGORIES)?;
            for (kind, nodes) in [(CategoryKind::Expense, seed.expense), (CategoryKind::Income, seed.income)] {
                for (order, node) in nodes.into_iter().enumerate() {
                    let parent = CategoryId::new();
                    insert_category(&mut tx, parent, &node.name, kind, None, order, false).await?;
                    for (child_order, child) in node.children.iter().enumerate() {
                        insert_category(&mut tx, CategoryId::new(), child, kind, Some(parent), child_order, false)
                            .await?;
                    }
                }
            }
        }
        db::settings::set(&mut *tx, "seed.categories", &true).await?;
    }

    if db::settings::get::<bool>(&mut *tx, "seed.excluded_categories").await?.is_none() {
        let kind = CategoryKind::Expense;
        if !db::categories::name_taken(&mut *tx, EXCLUDED_CATEGORY, kind, None, None).await? {
            // En dernier parmi les catégories racines.
            insert_category(&mut tx, CategoryId::new(), EXCLUDED_CATEGORY, kind, None, 1_000, true).await?;
        }
        db::settings::set(&mut *tx, "seed.excluded_categories", &true).await?;
    }

    tx.commit().await?;
    // Scénarios de projection par défaut (R-PRJ-31).
    super::projections::ensure_default_scenarios(db).await?;
    Ok(())
}

async fn insert_category(
    conn: &mut sqlx::SqliteConnection,
    id: CategoryId,
    name: &str,
    kind: CategoryKind,
    parent: Option<CategoryId>,
    order: usize,
    excluded_from_analyses: bool,
) -> anyhow::Result<()> {
    let fields = CategoryFields {
        name,
        kind,
        parent_id: parent,
        archived: false,
        excluded_from_analyses,
        display_order: i64::try_from(order)?,
        color: None,
        icon: None,
    };
    db::categories::insert(&mut *conn, id, &fields).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_is_idempotent_and_runs_categories_once() {
        let (db, _dir) = crate::db::test_db().await;
        run(&db).await.unwrap();
        let categories = db::categories::count(&db.read).await.unwrap();
        assert!(categories > 40);
        assert!(
            db::categories::find_by_name(&db.read, INTEREST_CATEGORY, CategoryKind::Income, None)
                .await
                .unwrap()
                .is_some()
        );

        // Une catégorie supprimée n'est pas recréée au démarrage suivant (R-TRX-23).
        let first = db::categories::list(&db.read).await.unwrap().into_iter().find(|c| c.name == "Dons").unwrap();
        db::categories::delete(&db.write, first.id).await.unwrap();
        run(&db).await.unwrap();
        assert_eq!(db::categories::count(&db.read).await.unwrap(), categories - 1);

        let livret_a = db::accounts::find_schedule_by_name(&db.read, "Livret A (réglementé)").await.unwrap();
        assert!(livret_a.is_some());
    }

    #[tokio::test]
    async fn r_trx_25_excluded_category_is_proposed_once_even_on_a_used_base() {
        let (db, _dir) = crate::db::test_db().await;
        // Base déjà en service, d'avant la catégorie hors analyses : catégories proposées, pas celle-ci.
        db::settings::set(&db.write, "seed.categories", &true).await.unwrap();
        run(&db).await.unwrap();
        let excluded = |rows: Vec<crate::db::categories::CategoryRow>| {
            rows.into_iter().filter(|c| c.name == EXCLUDED_CATEGORY).collect::<Vec<_>>()
        };
        let found = excluded(db::categories::list(&db.read).await.unwrap());
        assert_eq!(found.len(), 1);
        assert!(found[0].excluded_from_analyses && found[0].parent_id.is_none());

        // Supprimée, elle n'est pas recréée.
        db::categories::delete(&db.write, found[0].id).await.unwrap();
        run(&db).await.unwrap();
        assert!(excluded(db::categories::list(&db.read).await.unwrap()).is_empty());
    }
}
