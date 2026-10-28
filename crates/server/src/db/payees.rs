//! Tiers et groupes de tiers (spec 11).

use cashflow_domain::ledger::category::CategoryKind;
use cashflow_domain::payees::location::{GeoPoint, PayeeAddress};
use cashflow_domain::shared::ids::{CategoryId, PayeeGroupId, PayeeId, TransactionId};
use cashflow_domain::{Date, Money};
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{enum_from_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct PayeeRow {
    pub id: PayeeId,
    pub name: String,
    pub group_id: Option<PayeeGroupId>,
    pub archived: bool,
    pub note: Option<String>,
    pub address: Option<PayeeAddress>,
    pub transaction_count: i64,
    /// Transactions, récurrences et règles qui le désignent (R-TIE-5).
    pub usage: i64,
}

#[derive(Debug, Clone)]
pub struct GroupRow {
    pub id: PayeeGroupId,
    pub name: String,
    pub note: Option<String>,
}

pub struct PayeeFields<'a> {
    pub name: &'a str,
    pub group_id: Option<PayeeGroupId>,
    pub archived: bool,
    pub note: Option<&'a str>,
    pub address: Option<&'a PayeeAddress>,
}

/// Adresse lue en base, déjà contrôlée à l'écriture (R-TIE-23, R-TIE-25 ; `CHECK` de la table).
fn address_from_db(
    line: Option<String>,
    postal_code: Option<String>,
    city: Option<String>,
    country_code: Option<String>,
    latitude_e6: Option<i64>,
    longitude_e6: Option<i64>,
) -> anyhow::Result<Option<PayeeAddress>> {
    let (Some(city), Some(country_code)) = (city, country_code) else { return Ok(None) };
    let point = match (latitude_e6, longitude_e6) {
        (Some(lat), Some(lon)) => Some(GeoPoint::new(i32::try_from(lat)?, i32::try_from(lon)?)?),
        _ => None,
    };
    Ok(Some(PayeeAddress { line, postal_code, city, country_code, point }))
}

/// Colonnes d'une adresse, à écrire.
struct AddressColumns<'a> {
    line: Option<&'a str>,
    postal_code: Option<&'a str>,
    city: Option<&'a str>,
    country_code: Option<&'a str>,
    latitude_e6: Option<i32>,
    longitude_e6: Option<i32>,
}

fn address_columns(address: Option<&PayeeAddress>) -> AddressColumns<'_> {
    let point = address.and_then(|a| a.point);
    AddressColumns {
        line: address.and_then(|a| a.line.as_deref()),
        postal_code: address.and_then(|a| a.postal_code.as_deref()),
        city: address.map(|a| a.city.as_str()),
        country_code: address.map(|a| a.country_code.as_str()),
        latitude_e6: point.map(|p| p.latitude_e6),
        longitude_e6: point.map(|p| p.longitude_e6),
    }
}

macro_rules! payee_row {
    ($r:expr) => {{
        let r = $r;
        anyhow::Ok(PayeeRow {
            id: id_from_db(&r.id)?,
            name: r.name,
            group_id: opt_id_from_db(r.group_id.as_deref())?,
            archived: r.archived,
            note: r.note,
            address: address_from_db(
                r.address_line,
                r.postal_code,
                r.city,
                r.country_code,
                r.latitude_e6,
                r.longitude_e6,
            )?,
            transaction_count: r.transaction_count,
            usage: r.usage,
        })
    }};
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<PayeeRow>> {
    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.group_id, p.archived AS "archived: bool", p.note,
                  p.address_line, p.postal_code, p.city, p.country_code, p.latitude_e6, p.longitude_e6,
                  (SELECT count(*) FROM ledger_transaction t WHERE t.payee_id = p.id) AS "transaction_count!: i64",
                  (SELECT count(*) FROM ledger_transaction t WHERE t.payee_id = p.id)
                  + (SELECT count(*) FROM recurring_rule r WHERE r.payee_id = p.id)
                  + (SELECT count(*) FROM categorization_rule c WHERE json_extract(c.actions, '$.set_payee') = p.id)
                      AS "usage!: i64"
           FROM payee p ORDER BY p.name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| payee_row!(r)).collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: PayeeId) -> anyhow::Result<Option<PayeeRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT p.id, p.name, p.group_id, p.archived AS "archived: bool", p.note,
                  p.address_line, p.postal_code, p.city, p.country_code, p.latitude_e6, p.longitude_e6,
                  (SELECT count(*) FROM ledger_transaction t WHERE t.payee_id = p.id) AS "transaction_count!: i64",
                  (SELECT count(*) FROM ledger_transaction t WHERE t.payee_id = p.id)
                  + (SELECT count(*) FROM recurring_rule r WHERE r.payee_id = p.id)
                  + (SELECT count(*) FROM categorization_rule c WHERE json_extract(c.actions, '$.set_payee') = p.id)
                      AS "usage!: i64"
           FROM payee p WHERE p.id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| payee_row!(r)).transpose()
}

/// Un nom de tiers est unique sans distinction de casse (R-TIE-1).
pub async fn name_taken(db: impl SqliteExecutor<'_>, name: &str, except: Option<PayeeId>) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string()).unwrap_or_default();
    let count =
        sqlx::query_scalar!("SELECT count(*) FROM payee WHERE name = ? COLLATE NOCASE AND id <> ?", name, except)
            .fetch_one(db)
            .await?;
    Ok(count > 0)
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: PayeeId, f: &PayeeFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let group = f.group_id.map(|g| g.to_string());
    let a = address_columns(f.address);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO payee (id, name, group_id, archived, note, address_line, postal_code, city, country_code,
                            latitude_e6, longitude_e6, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        f.name,
        group,
        f.archived,
        f.note,
        a.line,
        a.postal_code,
        a.city,
        a.country_code,
        a.latitude_e6,
        a.longitude_e6,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: PayeeId, f: &PayeeFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let group = f.group_id.map(|g| g.to_string());
    let a = address_columns(f.address);
    let now = timestamp();
    sqlx::query!(
        "UPDATE payee SET name = ?, group_id = ?, archived = ?, note = ?, address_line = ?, postal_code = ?, city = ?,
                          country_code = ?, latitude_e6 = ?, longitude_e6 = ?, updated_at = ?
         WHERE id = ?",
        f.name,
        group,
        f.archived,
        f.note,
        a.line,
        a.postal_code,
        a.city,
        a.country_code,
        a.latitude_e6,
        a.longitude_e6,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_group(db: impl SqliteExecutor<'_>, id: PayeeId, group: Option<PayeeGroupId>) -> anyhow::Result<()> {
    let id = id.to_string();
    let group = group.map(|g| g.to_string());
    let now = timestamp();
    sqlx::query!("UPDATE payee SET group_id = ?, updated_at = ? WHERE id = ?", group, now, id).execute(db).await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: PayeeId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM payee WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Réaffecte transactions, récurrences et actions de règle d'un tiers à un autre, puis le supprime (R-TIE-7).
pub async fn merge(conn: &mut SqliteConnection, source: PayeeId, target: PayeeId) -> anyhow::Result<u64> {
    let source = source.to_string();
    let target = target.to_string();
    let now = timestamp();
    let moved = sqlx::query!(
        "UPDATE ledger_transaction SET payee_id = ?, updated_at = ? WHERE payee_id = ?",
        target,
        now,
        source
    )
    .execute(&mut *conn)
    .await?
    .rows_affected();
    sqlx::query!("UPDATE recurring_rule SET payee_id = ?, updated_at = ? WHERE payee_id = ?", target, now, source)
        .execute(&mut *conn)
        .await?;
    sqlx::query!(
        "UPDATE categorization_rule SET actions = json_set(actions, '$.set_payee', ?1), updated_at = ?2
         WHERE json_extract(actions, '$.set_payee') = ?3",
        target,
        now,
        source
    )
    .execute(&mut *conn)
    .await?;
    // Le tiers cible garde son adresse ; sans adresse, il reprend celle du tiers fusionné (R-TIE-27).
    sqlx::query!(
        "UPDATE payee SET (address_line, postal_code, city, country_code, latitude_e6, longitude_e6, updated_at) =
             (SELECT s.address_line, s.postal_code, s.city, s.country_code, s.latitude_e6, s.longitude_e6, ?1
              FROM payee s WHERE s.id = ?2)
         WHERE id = ?3 AND city IS NULL AND EXISTS (SELECT 1 FROM payee s WHERE s.id = ?2 AND s.city IS NOT NULL)",
        now,
        source,
        target
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM payee WHERE id = ?", source).execute(&mut *conn).await?;
    Ok(moved)
}

// ---------------------------------------------------------------- Groupes

pub async fn groups(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<GroupRow>> {
    let rows =
        sqlx::query!("SELECT id, name, note FROM payee_group ORDER BY name COLLATE NOCASE").fetch_all(db).await?;
    rows.into_iter().map(|r| Ok(GroupRow { id: id_from_db(&r.id)?, name: r.name, note: r.note })).collect()
}

pub async fn find_group(db: impl SqliteExecutor<'_>, id: PayeeGroupId) -> anyhow::Result<Option<GroupRow>> {
    let id = id.to_string();
    let row = sqlx::query!("SELECT id, name, note FROM payee_group WHERE id = ?", id).fetch_optional(db).await?;
    row.map(|r| Ok(GroupRow { id: id_from_db(&r.id)?, name: r.name, note: r.note })).transpose()
}

/// Un nom de groupe est unique sans distinction de casse (R-TIE-2).
pub async fn group_name_taken(
    db: impl SqliteExecutor<'_>,
    name: &str,
    except: Option<PayeeGroupId>,
) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string()).unwrap_or_default();
    let count =
        sqlx::query_scalar!("SELECT count(*) FROM payee_group WHERE name = ? COLLATE NOCASE AND id <> ?", name, except)
            .fetch_one(db)
            .await?;
    Ok(count > 0)
}

pub async fn upsert_group(
    db: impl SqliteExecutor<'_>,
    id: PayeeGroupId,
    name: &str,
    note: Option<&str>,
) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO payee_group (id, name, note, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, note = excluded.note, updated_at = excluded.updated_at",
        id,
        name,
        note,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Supprime un groupe : ses tiers restent, sans groupe (R-TIE-4, `ON DELETE SET NULL`).
pub async fn delete_group(db: impl SqliteExecutor<'_>, id: PayeeGroupId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM payee_group WHERE id = ?", id).execute(db).await?;
    Ok(())
}

// ---------------------------------------------------------------- Lectures pour les suggestions et les analyses

/// Transaction d'un tiers, pour la suggestion de catégorie (R-TIE-14).
pub struct PayeeHistoryRow {
    pub payee_id: PayeeId,
    pub transaction_id: TransactionId,
    pub date: Date,
    pub split_count: i64,
    pub category_id: Option<CategoryId>,
}

pub async fn history(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<PayeeHistoryRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.payee_id AS "payee_id!", t.id AS "id!", t.date AS "date!: Date",
                  (SELECT count(*) FROM split x WHERE x.transaction_id = t.id) AS "split_count!: i64",
                  (SELECT x.category_id FROM split x WHERE x.transaction_id = t.id ORDER BY x.position LIMIT 1)
                      AS "category_id?"
           FROM ledger_transaction t WHERE t.payee_id IS NOT NULL"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PayeeHistoryRow {
                payee_id: id_from_db(&r.payee_id)?,
                transaction_id: id_from_db(&r.id)?,
                date: r.date,
                split_count: r.split_count,
                category_id: opt_id_from_db(r.category_id.as_deref())?,
            })
        })
        .collect()
}

/// Libellés bancaires des transactions qui ont un tiers non archivé, tous comptes, la plus récente d'abord
/// (R-TIE-17).
pub async fn labelled_payees(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<(String, PayeeId)>> {
    let rows = sqlx::query!(
        r#"SELECT coalesce(t.bank_label, t.label) AS "label!: String", t.payee_id AS "payee_id!"
           FROM ledger_transaction t JOIN payee p ON p.id = t.payee_id
           WHERE p.archived = 0
           ORDER BY t.date DESC, t.id DESC"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| Ok((r.label, id_from_db(&r.payee_id)?))).collect()
}

/// Ventilation d'une transaction qui a un tiers, vue par les analyses (R-TIE-20).
pub struct PayeeFactRow {
    pub payee_id: PayeeId,
    pub transaction_id: TransactionId,
    pub date: Date,
    pub amount: Money,
    pub category_id: Option<CategoryId>,
    pub category_kind: Option<CategoryKind>,
    pub is_movement: bool,
    pub currency: String,
}

pub async fn facts(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<PayeeFactRow>> {
    let rows = sqlx::query!(
        r#"SELECT t.payee_id AS "payee_id!", t.id AS "transaction_id!", t.date AS "date!: Date", s.amount,
                  s.category_id, c.kind AS "category_kind?", a.currency,
                  (s.loan_id IS NOT NULL OR s.asset_id IS NOT NULL
                   OR EXISTS (SELECT 1 FROM transfer tr WHERE s.id IN (tr.outflow_split_id, tr.inflow_split_id)))
                      AS "is_movement!: bool"
           FROM split s
           JOIN ledger_transaction t ON t.id = s.transaction_id
           JOIN account a ON a.id = t.account_id
           LEFT JOIN category c ON c.id = s.category_id
           WHERE t.payee_id IS NOT NULL AND t.date BETWEEN ? AND ?"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PayeeFactRow {
                payee_id: id_from_db(&r.payee_id)?,
                transaction_id: id_from_db(&r.transaction_id)?,
                date: r.date,
                amount: Money::from_minor(r.amount),
                category_id: opt_id_from_db(r.category_id.as_deref())?,
                category_kind: r.category_kind.as_deref().map(enum_from_db).transpose()?,
                is_movement: r.is_movement,
                currency: r.currency,
            })
        })
        .collect()
}
