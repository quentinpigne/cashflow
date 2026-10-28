//! Budgets (spec 05) : lignes, surcharges, projets, faits budgétaires et notifications.

use cashflow_domain::budgeting::{BudgetKind, Month, Periodicity};
use cashflow_domain::shared::ids::{AccountId, BudgetLineId, CategoryId, NotificationId, ProjectId};
use cashflow_domain::{Date, Money};
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

fn thresholds_from_db(value: Option<&str>) -> anyhow::Result<Option<Vec<u32>>> {
    Ok(value.map(serde_json::from_str).transpose()?)
}

fn thresholds_to_db(value: Option<&Vec<u32>>) -> anyhow::Result<Option<String>> {
    Ok(value.map(serde_json::to_string).transpose()?)
}

fn month_from_db(value: &str) -> anyhow::Result<Month> {
    Ok(value.parse()?)
}

// ---------------------------------------------------------------- Lignes

#[derive(Debug, Clone)]
pub struct LineRow {
    pub id: BudgetLineId,
    pub kind: BudgetKind,
    /// Nom donné à la ligne ; absent, elle porte le nom de sa catégorie.
    pub name: Option<String>,
    /// Catégories d'une ligne de dépense ou de revenu (R-BUD-10) ; aucune pour un objectif d'épargne.
    pub category_ids: Vec<CategoryId>,
    pub target_account_id: Option<AccountId>,
    pub periodicity: Periodicity,
    pub year_start_month: u32,
    pub amount: Money,
    pub rollover: bool,
    pub effective_from: Month,
    pub effective_to: Option<Month>,
    pub alert_thresholds: Option<Vec<u32>>,
    pub previous_version_id: Option<BudgetLineId>,
}

pub async fn lines(conn: &mut SqliteConnection) -> anyhow::Result<Vec<LineRow>> {
    let links = sqlx::query!("SELECT budget_line_id, category_id FROM budget_line_category ORDER BY rowid")
        .fetch_all(&mut *conn)
        .await?;
    let mut categories: std::collections::HashMap<BudgetLineId, Vec<CategoryId>> = std::collections::HashMap::new();
    for link in links {
        categories.entry(id_from_db(&link.budget_line_id)?).or_default().push(id_from_db(&link.category_id)?);
    }
    let rows = sqlx::query!(
        r#"SELECT id, kind, name, target_account_id, periodicity, year_start_month, amount,
                  rollover AS "rollover: bool", effective_from, effective_to, alert_thresholds, previous_version_id
           FROM budget_line ORDER BY effective_from, id"#
    )
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|r| {
            let id = id_from_db(&r.id)?;
            Ok(LineRow {
                id,
                kind: enum_from_db(&r.kind)?,
                name: r.name,
                category_ids: categories.remove(&id).unwrap_or_default(),
                target_account_id: opt_id_from_db(r.target_account_id.as_deref())?,
                periodicity: enum_from_db(&r.periodicity)?,
                year_start_month: u32::try_from(r.year_start_month)?,
                amount: Money::from_minor(r.amount),
                rollover: r.rollover,
                effective_from: month_from_db(&r.effective_from)?,
                effective_to: r.effective_to.as_deref().map(month_from_db).transpose()?,
                alert_thresholds: thresholds_from_db(r.alert_thresholds.as_deref())?,
                previous_version_id: opt_id_from_db(r.previous_version_id.as_deref())?,
            })
        })
        .collect()
}

/// Enregistre une version de ligne et ses catégories.
pub async fn upsert_line(conn: &mut SqliteConnection, l: &LineRow) -> anyhow::Result<()> {
    let id = l.id.to_string();
    let kind = enum_to_db(&l.kind);
    let target = l.target_account_id.map(|a| a.to_string());
    let periodicity = enum_to_db(&l.periodicity);
    let amount = l.amount.minor();
    let from = l.effective_from.to_string();
    let to = l.effective_to.map(|m| m.to_string());
    let thresholds = thresholds_to_db(l.alert_thresholds.as_ref())?;
    let previous = l.previous_version_id.map(|p| p.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO budget_line (id, kind, name, target_account_id, periodicity, year_start_month, amount, rollover,
                                  effective_from, effective_to, alert_thresholds, previous_version_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, year_start_month = excluded.year_start_month,
                                        amount = excluded.amount, rollover = excluded.rollover,
                                        effective_from = excluded.effective_from, effective_to = excluded.effective_to,
                                        alert_thresholds = excluded.alert_thresholds, updated_at = excluded.updated_at",
        id,
        kind,
        l.name,
        target,
        periodicity,
        l.year_start_month,
        amount,
        l.rollover,
        from,
        to,
        thresholds,
        previous,
        now
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM budget_line_category WHERE budget_line_id = ?", id).execute(&mut *conn).await?;
    for category in &l.category_ids {
        let category = category.to_string();
        sqlx::query!("INSERT INTO budget_line_category (budget_line_id, category_id) VALUES (?, ?)", id, category)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Supprime une version qui n'a jamais été en vigueur ; ses surcharges suivent (cascade).
pub async fn delete_line(conn: &mut SqliteConnection, id: BudgetLineId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("UPDATE budget_line SET previous_version_id = NULL WHERE previous_version_id = ?", id)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("DELETE FROM budget_line WHERE id = ?", id).execute(&mut *conn).await?;
    Ok(())
}

// ---------------------------------------------------------------- Surcharges

#[derive(Debug, Clone)]
pub struct OverrideRow {
    pub line_id: BudgetLineId,
    pub period: Month,
    pub amount: Money,
}

pub async fn overrides(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<OverrideRow>> {
    let rows = sqlx::query!("SELECT budget_line_id, period, amount FROM budget_override ORDER BY period")
        .fetch_all(db)
        .await?;
    rows.into_iter()
        .map(|r| {
            Ok(OverrideRow {
                line_id: id_from_db(&r.budget_line_id)?,
                period: month_from_db(&r.period)?,
                amount: Money::from_minor(r.amount),
            })
        })
        .collect()
}

pub async fn set_override(db: impl SqliteExecutor<'_>, o: &OverrideRow) -> anyhow::Result<()> {
    let id = cashflow_domain::shared::ids::BudgetOverrideId::new().to_string();
    let line = o.line_id.to_string();
    let period = o.period.to_string();
    let amount = o.amount.minor();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO budget_override (id, budget_line_id, period, amount, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT (budget_line_id, period) DO UPDATE SET amount = excluded.amount, updated_at = excluded.updated_at",
        id,
        line,
        period,
        amount,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_override(db: impl SqliteExecutor<'_>, line: BudgetLineId, period: Month) -> anyhow::Result<()> {
    let line = line.to_string();
    let period = period.to_string();
    sqlx::query!("DELETE FROM budget_override WHERE budget_line_id = ? AND period = ?", line, period)
        .execute(db)
        .await?;
    Ok(())
}

/// Transfère à une nouvelle version les surcharges de l'ancienne à partir du mois `from` (R-BUD-51).
pub async fn move_overrides(
    db: impl SqliteExecutor<'_>,
    from_line: BudgetLineId,
    to_line: BudgetLineId,
    from: Month,
) -> anyhow::Result<()> {
    let from_line = from_line.to_string();
    let to_line = to_line.to_string();
    let from = from.to_string();
    sqlx::query!(
        "UPDATE budget_override SET budget_line_id = ? WHERE budget_line_id = ? AND period >= ?",
        to_line,
        from_line,
        from
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Supprime les surcharges d'une version postérieures à sa fin (R-BUD-51, « à partir de ce mois »).
pub async fn delete_overrides_after(
    db: impl SqliteExecutor<'_>,
    line: BudgetLineId,
    last: Month,
) -> anyhow::Result<()> {
    let line = line.to_string();
    let last = last.to_string();
    sqlx::query!("DELETE FROM budget_override WHERE budget_line_id = ? AND period > ?", line, last).execute(db).await?;
    Ok(())
}

// ---------------------------------------------------------------- Projets

#[derive(Debug, Clone)]
pub struct ProjectRow {
    pub id: ProjectId,
    pub name: String,
    pub start_date: Date,
    pub end_date: Option<Date>,
    pub amount: Money,
    pub exclude_from_period_budgets: bool,
    pub active: bool,
    pub alert_thresholds: Option<Vec<u32>>,
    pub note: Option<String>,
}

pub async fn projects(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ProjectRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, start_date AS "start_date: Date", end_date AS "end_date: Date", amount,
                  exclude_from_period_budgets AS "exclude: bool", status, alert_thresholds, note
           FROM project ORDER BY status, start_date DESC, name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ProjectRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                start_date: r.start_date,
                end_date: r.end_date,
                amount: Money::from_minor(r.amount),
                exclude_from_period_budgets: r.exclude,
                active: r.status == "ACTIVE",
                alert_thresholds: thresholds_from_db(r.alert_thresholds.as_deref())?,
                note: r.note,
            })
        })
        .collect()
}

pub async fn upsert_project(db: impl SqliteExecutor<'_>, p: &ProjectRow) -> anyhow::Result<()> {
    let id = p.id.to_string();
    let amount = p.amount.minor();
    let status = if p.active { "ACTIVE" } else { "CLOSED" };
    let thresholds = thresholds_to_db(p.alert_thresholds.as_ref())?;
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO project (id, name, start_date, end_date, amount, exclude_from_period_budgets, status, alert_thresholds,
                              note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, start_date = excluded.start_date, end_date = excluded.end_date,
                                        amount = excluded.amount,
                                        exclude_from_period_budgets = excluded.exclude_from_period_budgets,
                                        status = excluded.status, alert_thresholds = excluded.alert_thresholds,
                                        note = excluded.note, updated_at = excluded.updated_at",
        id,
        p.name,
        p.start_date,
        p.end_date,
        amount,
        p.exclude_from_period_budgets,
        status,
        thresholds,
        p.note,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn project_usage(db: impl SqliteExecutor<'_>, id: ProjectId) -> anyhow::Result<i64> {
    let id = id.to_string();
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "n!: i64" FROM split WHERE project_id = ?"#, id).fetch_one(db).await?)
}

pub async fn delete_project(db: impl SqliteExecutor<'_>, id: ProjectId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM project WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Rattache (ou détache) des ventilations à un projet.
pub async fn set_split_project(
    db: impl SqliteExecutor<'_>,
    split: cashflow_domain::shared::ids::SplitId,
    project: Option<ProjectId>,
) -> anyhow::Result<()> {
    let split = split.to_string();
    let project = project.map(|p| p.to_string());
    let now = timestamp();
    sqlx::query!("UPDATE split SET project_id = ?, updated_at = ? WHERE id = ?", project, now, split)
        .execute(db)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- Faits

/// Une ventilation, vue par le budget et les projets.
pub struct BudgetFactRow {
    pub date: Date,
    pub amount: Money,
    pub category_id: Option<CategoryId>,
    pub project_id: Option<ProjectId>,
    pub project_excluded: bool,
    pub in_budget: bool,
    pub is_movement: bool,
    pub currency: String,
}

/// Ventilations datées de `from` à `to` ; celles d'un projet, quelle que soit leur date si `project` est donné.
pub async fn facts(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<BudgetFactRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.date AS "date!: Date", s.amount, s.category_id, s.project_id,
                  coalesce(p.exclude_from_period_budgets, 0) AS "project_excluded!: bool",
                  a.include_in_budget AS "in_budget!: bool", a.currency,
                  (s.loan_id IS NOT NULL OR s.asset_id IS NOT NULL
                   OR EXISTS (SELECT 1 FROM transfer tr WHERE s.id IN (tr.outflow_split_id, tr.inflow_split_id))
                   OR EXISTS (SELECT 1 FROM excluded_category x WHERE x.id = s.category_id))
                      AS "is_movement!: bool"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN account a ON a.id = t.account_id
           LEFT JOIN project p ON p.id = s.project_id
           WHERE t.date BETWEEN ? AND ?"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(BudgetFactRow {
                date: r.date,
                amount: Money::from_minor(r.amount),
                category_id: opt_id_from_db(r.category_id.as_deref())?,
                project_id: opt_id_from_db(r.project_id.as_deref())?,
                project_excluded: r.project_excluded,
                in_budget: r.in_budget,
                is_movement: r.is_movement,
                currency: r.currency,
            })
        })
        .collect()
}

/// Ventilations rattachées à un projet, toutes dates et tous comptes (R-BUD-23, R-BUD-25).
pub struct ProjectSplitRow {
    pub project_id: ProjectId,
    pub transaction_id: cashflow_domain::shared::ids::TransactionId,
    pub date: Date,
    pub label: String,
    pub account_name: String,
    pub amount: Money,
    pub category_id: Option<CategoryId>,
}

pub async fn project_splits(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ProjectSplitRow>> {
    let rows = sqlx::query!(
        r#"SELECT s.project_id AS "project_id!", t.id AS "transaction_id!", t.date AS "date!: Date", t.label AS "label!",
                  a.name AS account_name, s.amount, s.category_id
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN account a ON a.id = t.account_id
           WHERE s.project_id IS NOT NULL
           ORDER BY t.date DESC"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ProjectSplitRow {
                project_id: id_from_db(&r.project_id)?,
                transaction_id: id_from_db(&r.transaction_id)?,
                date: r.date,
                label: r.label,
                account_name: r.account_name,
                amount: Money::from_minor(r.amount),
                category_id: opt_id_from_db(r.category_id.as_deref())?,
            })
        })
        .collect()
}

/// Virement interne vu du compte d'épargne : versement depuis un compte du budget (positif) ou retrait vers
/// un compte du budget (négatif) (R-BUD-31, R-BUD-33).
pub struct SavingsMoveRow {
    pub date: Date,
    pub account_id: AccountId,
    pub amount: Money,
}

pub async fn savings_moves(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<SavingsMoveRow>> {
    // Le montant d'une jambe est celui de sa ventilation de virement (ADR-0021).
    let rows = sqlx::query!(
        r#"SELECT t_in.date AS "date!: Date", t_in.account_id AS "account_id!", s_in.amount AS "amount!: i64"
           FROM transfer tr
           JOIN split s_in ON s_in.id = tr.inflow_split_id
           JOIN split s_out ON s_out.id = tr.outflow_split_id
           JOIN ledger_transaction t_in ON t_in.id = s_in.transaction_id
           JOIN ledger_transaction t_out ON t_out.id = s_out.transaction_id
           JOIN account a_out ON a_out.id = t_out.account_id
           WHERE a_out.include_in_budget = 1 AND t_in.date BETWEEN ?1 AND ?2
           UNION ALL
           SELECT t_out.date, t_out.account_id, s_out.amount
           FROM transfer tr
           JOIN split s_in ON s_in.id = tr.inflow_split_id
           JOIN split s_out ON s_out.id = tr.outflow_split_id
           JOIN ledger_transaction t_in ON t_in.id = s_in.transaction_id
           JOIN ledger_transaction t_out ON t_out.id = s_out.transaction_id
           JOIN account a_in ON a_in.id = t_in.account_id
           WHERE a_in.include_in_budget = 1 AND t_out.date BETWEEN ?1 AND ?2"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(SavingsMoveRow {
                date: r.date,
                account_id: id_from_db(&r.account_id)?,
                amount: Money::from_minor(r.amount),
            })
        })
        .collect()
}

/// Mois de la plus ancienne transaction, pour l'assistant (BUD-10).
pub async fn first_transaction_date(db: impl SqliteExecutor<'_>) -> anyhow::Result<Option<Date>> {
    Ok(sqlx::query_scalar!(r#"SELECT min(date) AS "d: Date" FROM ledger_transaction"#).fetch_one(db).await?)
}

// ---------------------------------------------------------------- Notifications

#[derive(Debug, Clone)]
pub struct NotificationRow {
    pub id: NotificationId,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
    pub read: bool,
    pub created_at: String,
}

/// Insère une notification, sauf si sa clé de dédoublonnage existe déjà (R-BUD-62). Renvoie `true` si elle
/// est nouvelle.
pub async fn notify(
    db: impl SqliteExecutor<'_>,
    kind: &str,
    title: &str,
    body: &str,
    link: Option<&str>,
    dedupe_key: Option<&str>,
) -> anyhow::Result<bool> {
    let id = NotificationId::new().to_string();
    let now = timestamp();
    let result = sqlx::query!(
        "INSERT INTO notification (id, kind, title, body, link, dedupe_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7) ON CONFLICT (dedupe_key) DO NOTHING",
        id,
        kind,
        title,
        body,
        link,
        dedupe_key,
        now
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Clés de dédoublonnage déjà émises ; une notification groupée en porte plusieurs, séparées par une espace.
pub async fn known_keys(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<String>> {
    Ok(sqlx::query_scalar!(r#"SELECT dedupe_key AS "k!" FROM notification WHERE dedupe_key IS NOT NULL"#)
        .fetch_all(db)
        .await?)
}

pub async fn notifications(db: impl SqliteExecutor<'_>, limit: i64) -> anyhow::Result<Vec<NotificationRow>> {
    let rows = sqlx::query!(
        "SELECT id, kind, title, body, link, read_at, created_at FROM notification ORDER BY created_at DESC, id DESC LIMIT ?",
        limit
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(NotificationRow {
                id: id_from_db(&r.id)?,
                kind: r.kind,
                title: r.title,
                body: r.body,
                link: r.link,
                read: r.read_at.is_some(),
                created_at: r.created_at,
            })
        })
        .collect()
}

pub async fn unread_count(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "n!: i64" FROM notification WHERE read_at IS NULL"#)
        .fetch_one(db)
        .await?)
}

/// Marque lues une notification, ou toutes.
pub async fn mark_read(db: impl SqliteExecutor<'_>, id: Option<NotificationId>) -> anyhow::Result<()> {
    let id = id.map(|i| i.to_string());
    let now = timestamp();
    sqlx::query!(
        "UPDATE notification SET read_at = ?1, updated_at = ?1 WHERE read_at IS NULL AND (?2 IS NULL OR id = ?2)",
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}
