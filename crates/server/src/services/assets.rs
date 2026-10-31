//! Biens : fiche, estimations, valeur nette, acquisition (spec 09, ADR-0019).

use std::collections::HashMap;

use cashflow_domain::assets::{AssetKind, Holding};
use cashflow_domain::household::{self, MemberSelection};
use cashflow_domain::shared::ids::{AssetId, AssetValuationId, LoanId, MemberId, TransactionId};
use cashflow_domain::{Date, Money};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::clean;
use super::loans::LoanBook;
use crate::config::DataDir;
use crate::db::assets::{AssetRow, ValuationRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------- Registre

/// Tous les biens et leurs estimations, chargés une fois.
pub struct AssetBook {
    pub assets: Vec<AssetRow>,
    valuations: HashMap<AssetId, Vec<(Date, Money)>>,
    /// Quotes-parts des membres, dans leur ordre (R-MBR-11).
    shares: HashMap<AssetId, Vec<(MemberId, u32)>>,
}

fn holding(asset: &AssetRow) -> Holding {
    Holding { purchase_date: asset.purchase_date, purchase_price: asset.purchase_price, sold_on: asset.sold_on }
}

impl AssetBook {
    pub async fn load(db: &Db) -> AppResult<Self> {
        let mut valuations: HashMap<AssetId, Vec<(Date, Money)>> = HashMap::new();
        for v in db::assets::valuations(&db.read).await? {
            valuations.entry(v.asset_id).or_default().push((v.date, v.value));
        }
        let mut shares: HashMap<AssetId, Vec<(MemberId, u32)>> = HashMap::new();
        for s in db::assets::shares(&db.read).await? {
            shares.entry(s.asset_id).or_default().push((s.member_id, s.share_bp));
        }
        Ok(Self { assets: db::assets::list(&db.read).await?, valuations, shares })
    }

    /// Quotes-parts d'un bien, dans l'ordre des membres (R-MBR-11).
    pub fn shares(&self, asset: AssetId) -> &[(MemberId, u32)] {
        self.shares.get(&asset).map(Vec::as_slice).unwrap_or_default()
    }

    /// Parts de la valeur d'un bien : celle de chaque membre, puis celle du foyer (R-MBR-12).
    pub fn parts(&self, asset: &AssetRow, date: Date) -> Option<(Vec<(MemberId, Money)>, Money)> {
        let value = self.value(asset, date)?;
        let shares = self.shares(asset.id);
        let (parts, household) = household::split_value(value, &shares.iter().map(|(_, bp)| *bp).collect::<Vec<_>>());
        Some((shares.iter().map(|(m, _)| *m).zip(parts).collect(), household))
    }

    /// Valeur d'un bien comptée pour un choix de membres : leurs parts, et le reste avec « Foyer » ; la valeur
    /// entière sans choix (R-MBR-9, R-MBR-12).
    pub fn value_for(&self, asset: &AssetRow, date: Date, selection: Option<&MemberSelection>) -> Option<Money> {
        let Some(selection) = selection else { return self.value(asset, date) };
        let (parts, household) = self.parts(asset, date)?;
        let members: Money = parts.iter().filter(|(m, _)| selection.includes(Some(*m))).map(|(_, v)| *v).sum();
        Some(members + if selection.includes(None) { household } else { Money::ZERO })
    }

    /// Valeur à une date (R-BIE-2) ; rien si le bien n'est pas possédé.
    pub fn value(&self, asset: &AssetRow, date: Date) -> Option<Money> {
        holding(asset).value_at(self.valuations.get(&asset.id).map(Vec::as_slice).unwrap_or_default(), date)
    }
}

// ---------------------------------------------------------------- Vues

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssetInput {
    pub name: String,
    pub kind: AssetKind,
    pub purchase_date: Date,
    /// Prix d'achat, positif : apport et capital emprunté, hors frais (R-BIE-1).
    pub purchase_price_minor: i64,
    pub sold_on: Option<Date>,
    pub sale_price_minor: Option<i64>,
    pub include_in_net_worth: Option<bool>,
    pub notes: Option<String>,
}

/// Un bien dans la liste (E25) et sur l'accueil.
#[derive(Debug, Serialize, ToSchema)]
pub struct AssetSummary {
    pub id: AssetId,
    pub name: String,
    pub kind: AssetKind,
    pub purchase_date: Date,
    pub purchase_price_minor: i64,
    /// Valeur aujourd'hui (R-BIE-2) ; absente une fois vendu.
    pub value_minor: Option<i64>,
    pub sold_on: Option<Date>,
    pub include_in_net_worth: bool,
    /// Quotes-parts des membres, dans leur ordre (R-MBR-11).
    pub shares: Vec<AssetShareView>,
    /// Quote-part du foyer : le reste, en centièmes de pour cent.
    pub household_share_bp: u32,
    /// Part du foyer dans la valeur d'aujourd'hui (R-MBR-12).
    pub household_value_minor: Option<i64>,
}

/// Quote-part d'un membre et sa part de la valeur d'aujourd'hui (R-MBR-11, R-MBR-12).
#[derive(Debug, Serialize, ToSchema)]
pub struct AssetShareView {
    pub member_id: MemberId,
    pub share_bp: u32,
    pub value_minor: Option<i64>,
}

/// Une quote-part saisie, en pourcentage (`share_bp`) ou en montant (`amount_minor`, converti d'après la valeur
/// d'aujourd'hui) (R-MBR-11).
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssetShareInput {
    pub member_id: MemberId,
    pub share_bp: Option<u32>,
    pub amount_minor: Option<i64>,
}

/// Répartition d'un bien entre membres ; vide, le bien est entièrement au foyer (R-MBR-11).
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssetSharesInput {
    pub shares: Vec<AssetShareInput>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssetValuationView {
    pub id: AssetValuationId,
    pub date: Date,
    pub value_minor: i64,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FinancingLoan {
    pub id: LoanId,
    pub name: String,
    /// Capital restant dû aujourd'hui (R-PRE-4).
    pub outstanding_minor: Option<i64>,
}

/// Une transaction ventilée en acquisition de ce bien (R-BIE-8).
#[derive(Debug, Serialize, ToSchema)]
pub struct AcquisitionView {
    pub transaction_id: TransactionId,
    pub date: Date,
    pub label: String,
    /// Part payée, positive.
    pub amount_minor: i64,
}

/// Fiche d'un bien (E26).
#[derive(Debug, Serialize, ToSchema)]
pub struct AssetView {
    pub summary: AssetSummary,
    pub sale_price_minor: Option<i64>,
    pub notes: Option<String>,
    /// Estimations, la plus récente d'abord.
    pub valuations: Vec<AssetValuationView>,
    pub loans: Vec<FinancingLoan>,
    /// Valeur − capital restant dû des prêts qui le financent (R-PRE-10).
    pub net_value_minor: Option<i64>,
    pub acquisitions: Vec<AcquisitionView>,
    pub acquisitions_total_minor: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssetValuationInput {
    pub date: Date,
    pub value_minor: i64,
    pub note: Option<String>,
}

fn summary(asset: &AssetRow, book: &AssetBook, today: Date) -> AssetSummary {
    let parts = book.parts(asset, today);
    let shares = book
        .shares(asset.id)
        .iter()
        .map(|(member, bp)| AssetShareView {
            member_id: *member,
            share_bp: *bp,
            value_minor: parts.as_ref().and_then(|(p, _)| p.iter().find(|(m, _)| m == member)).map(|(_, v)| v.minor()),
        })
        .collect();
    AssetSummary {
        shares,
        household_share_bp: household::FULL_SHARE.saturating_sub(book.shares(asset.id).iter().map(|(_, bp)| bp).sum()),
        household_value_minor: parts.map(|(_, h)| h.minor()),
        id: asset.id,
        name: asset.name.clone(),
        kind: asset.kind,
        purchase_date: asset.purchase_date,
        purchase_price_minor: asset.purchase_price.minor(),
        value_minor: book.value(asset, today).map(Money::minor),
        sold_on: asset.sold_on,
        include_in_net_worth: asset.include_in_net_worth,
    }
}

// ---------------------------------------------------------------- Lecture

pub async fn list(db: &Db, today: Date) -> AppResult<Vec<AssetSummary>> {
    let book = AssetBook::load(db).await?;
    Ok(book.assets.iter().map(|a| summary(a, &book, today)).collect())
}

pub async fn get(db: &Db, id: AssetId, today: Date) -> AppResult<AssetView> {
    let book = AssetBook::load(db).await?;
    let asset = book.assets.iter().find(|a| a.id == id).ok_or(AppError::NotFound("bien"))?;
    let loan_book = LoanBook::load(db).await?;
    let loans: Vec<FinancingLoan> = loan_book
        .loans
        .iter()
        .filter(|l| l.asset_id == Some(id))
        .map(|l| FinancingLoan {
            id: l.id,
            name: l.name.clone(),
            outstanding_minor: loan_book.outstanding(l, today).map(Money::minor),
        })
        .collect();
    let value = book.value(asset, today);
    let debt: Money = loans.iter().filter_map(|l| l.outstanding_minor).map(Money::from_minor).sum();
    let acquisitions: Vec<AcquisitionView> = db::transactions::attached_splits(&db.read)
        .await?
        .into_iter()
        .filter(|s| s.asset_id == Some(id))
        .map(|s| AcquisitionView {
            transaction_id: s.transaction_id,
            date: s.date,
            label: s.label,
            amount_minor: -s.amount.minor(),
        })
        .collect();
    let acquisitions_total: Money = acquisitions.iter().map(|a| Money::from_minor(a.amount_minor)).sum();
    let valuations = db::assets::valuations(&db.read)
        .await?
        .into_iter()
        .rev()
        .filter(|v| v.asset_id == id)
        .map(|v| AssetValuationView { id: v.id, date: v.date, value_minor: v.value.minor(), note: v.note })
        .collect();
    Ok(AssetView {
        summary: summary(asset, &book, today),
        sale_price_minor: asset.sale_price.map(Money::minor),
        notes: asset.notes.clone(),
        valuations,
        net_value_minor: value.map(|v| (v - debt).minor()),
        loans,
        acquisitions,
        acquisitions_total_minor: acquisitions_total.minor(),
    })
}

// ---------------------------------------------------------------- Écriture

fn row_of(id: AssetId, input: &AssetInput) -> AppResult<AssetRow> {
    let name = super::required(&input.name, "name", 80)?;
    let row = AssetRow {
        id,
        name,
        kind: input.kind,
        purchase_date: input.purchase_date,
        purchase_price: Money::from_minor(input.purchase_price_minor),
        sold_on: input.sold_on,
        sale_price: input.sale_price_minor.map(Money::from_minor),
        include_in_net_worth: input.include_in_net_worth.unwrap_or(true),
        notes: clean(input.notes.as_deref()),
    };
    holding(&row).validate(row.sale_price)?;
    Ok(row)
}

pub async fn create(db: &Db, input: &AssetInput, today: Date) -> AppResult<AssetView> {
    let id = AssetId::new();
    db::assets::upsert(&db.write, &row_of(id, input)?).await?;
    get(db, id, today).await
}

pub async fn update(db: &Db, id: AssetId, input: &AssetInput, today: Date) -> AppResult<AssetView> {
    db::assets::find(&db.read, id).await?.ok_or(AppError::NotFound("bien"))?;
    db::assets::upsert(&db.write, &row_of(id, input)?).await?;
    get(db, id, today).await
}

/// Supprime un bien et ses estimations ; ses prêts perdent le lien, ses acquisitions redeviennent non catégorisées
/// (R-BIE-6). Ses documents restent, sauf ceux rattachés seulement à lui si `delete_documents` (R-DOC-12).
pub async fn delete(db: &Db, data: &DataDir, id: AssetId, delete_documents: bool, today: Date) -> AppResult<()> {
    db::assets::find(&db.read, id).await?.ok_or(AppError::NotFound("bien"))?;
    let mut tx = db.write.begin().await?;
    let documents =
        if delete_documents { super::documents::delete_exclusive(&mut tx, *id.as_uuid()).await? } else { Vec::new() };
    db::transactions::detach_asset(&mut *tx, id).await?;
    db::assets::delete(&mut *tx, id).await?;
    tx.commit().await?;
    super::documents::trash(data, &documents, today);
    Ok(())
}

pub async fn save_valuation(db: &Db, id: AssetId, input: &AssetValuationInput, today: Date) -> AppResult<AssetView> {
    let asset = db::assets::find(&db.read, id).await?.ok_or(AppError::NotFound("bien"))?;
    holding(&asset).check_valuation(input.date, Money::from_minor(input.value_minor))?;
    let row = ValuationRow {
        id: AssetValuationId::new(),
        asset_id: id,
        date: input.date,
        value: Money::from_minor(input.value_minor),
        note: clean(input.note.as_deref()),
    };
    db::assets::upsert_valuation(&db.write, &row).await?;
    get(db, id, today).await
}

/// Répartit un bien entre membres (R-MBR-11) : un montant devient une quote-part d'après la valeur d'aujourd'hui.
pub async fn set_shares(db: &Db, id: AssetId, input: &AssetSharesInput, today: Date) -> AppResult<AssetView> {
    let book = AssetBook::load(db).await?;
    let asset = book.assets.iter().find(|a| a.id == id).ok_or(AppError::NotFound("bien"))?;
    let mut shares: Vec<(MemberId, u32)> = Vec::new();
    for line in &input.shares {
        let bp = match (line.share_bp, line.amount_minor) {
            (Some(bp), None) => Some(bp),
            (None, Some(amount)) => {
                book.value(asset, today).and_then(|value| household::share_of_amount(Money::from_minor(amount), value))
            }
            _ => None,
        };
        shares.push((line.member_id, bp.ok_or(cashflow_domain::DomainError::InvalidAssetShares)?));
    }
    household::check_shares(&shares)?;
    let mut tx = db.write.begin().await?;
    for (member, _) in &shares {
        super::members::check_exists(&mut tx, Some(*member)).await?;
    }
    db::assets::replace_shares(&mut tx, id, &shares).await?;
    tx.commit().await?;
    get(db, id, today).await
}

pub async fn delete_valuation(db: &Db, id: AssetId, valuation: AssetValuationId) -> AppResult<()> {
    if db::assets::delete_valuation(&db.write, id, valuation).await? == 0 {
        return Err(AppError::NotFound("estimation"));
    }
    Ok(())
}
