//! Biens et leurs estimations (spec 09, ADR-0019).

use cashflow_domain::assets::AssetKind;
use cashflow_domain::shared::ids::{AssetId, AssetValuationId};
use cashflow_domain::{Date, Money};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct AssetRow {
    pub id: AssetId,
    pub name: String,
    pub kind: AssetKind,
    pub purchase_date: Date,
    pub purchase_price: Money,
    pub sold_on: Option<Date>,
    pub sale_price: Option<Money>,
    pub include_in_net_worth: bool,
    pub notes: Option<String>,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<AssetRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, kind, purchase_date AS "purchase_date: Date", purchase_price,
                  sold_on AS "sold_on: Date", sale_price, include_in_net_worth AS "include_in_net_worth: bool", notes
           FROM asset ORDER BY purchase_date, name"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(AssetRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                kind: enum_from_db(&r.kind)?,
                purchase_date: r.purchase_date,
                purchase_price: Money::from_minor(r.purchase_price),
                sold_on: r.sold_on,
                sale_price: r.sale_price.map(Money::from_minor),
                include_in_net_worth: r.include_in_net_worth,
                notes: r.notes,
            })
        })
        .collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: AssetId) -> anyhow::Result<Option<AssetRow>> {
    Ok(list(db).await?.into_iter().find(|a| a.id == id))
}

pub async fn upsert(db: impl SqliteExecutor<'_>, row: &AssetRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let kind = enum_to_db(&row.kind);
    let price = row.purchase_price.minor();
    let sale = row.sale_price.map(Money::minor);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO asset (id, name, kind, purchase_date, purchase_price, sold_on, sale_price, include_in_net_worth,
                            notes, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
             name = excluded.name, kind = excluded.kind, purchase_date = excluded.purchase_date,
             purchase_price = excluded.purchase_price, sold_on = excluded.sold_on, sale_price = excluded.sale_price,
             include_in_net_worth = excluded.include_in_net_worth, notes = excluded.notes,
             updated_at = excluded.updated_at",
        id,
        row.name,
        kind,
        row.purchase_date,
        price,
        row.sold_on,
        sale,
        row.include_in_net_worth,
        row.notes,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: AssetId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM asset WHERE id = ?", id).execute(db).await?;
    Ok(())
}

pub async fn count(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "count!: i64" FROM asset"#).fetch_one(db).await?)
}

// ---------------------------------------------------------------- Estimations (R-BIE-3)

#[derive(Debug, Clone)]
pub struct ValuationRow {
    pub id: AssetValuationId,
    pub asset_id: AssetId,
    pub date: Date,
    pub value: Money,
    pub note: Option<String>,
}

pub async fn valuations(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ValuationRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, asset_id, date AS "date: Date", value, note FROM asset_valuation ORDER BY asset_id, date"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ValuationRow {
                id: id_from_db(&r.id)?,
                asset_id: id_from_db(&r.asset_id)?,
                date: r.date,
                value: Money::from_minor(r.value),
                note: r.note,
            })
        })
        .collect()
}

pub async fn upsert_valuation(db: impl SqliteExecutor<'_>, v: &ValuationRow) -> anyhow::Result<()> {
    let id = v.id.to_string();
    let asset = v.asset_id.to_string();
    let value = v.value.minor();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO asset_valuation (id, asset_id, date, value, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (asset_id, date) DO UPDATE SET value = excluded.value, note = excluded.note,
                                                    updated_at = excluded.updated_at",
        id,
        asset,
        v.date,
        value,
        v.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_valuation(
    db: impl SqliteExecutor<'_>,
    asset: AssetId,
    id: AssetValuationId,
) -> anyhow::Result<u64> {
    let asset = asset.to_string();
    let id = id.to_string();
    Ok(sqlx::query!("DELETE FROM asset_valuation WHERE asset_id = ? AND id = ?", asset, id)
        .execute(db)
        .await?
        .rows_affected())
}
