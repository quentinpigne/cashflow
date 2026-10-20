//! Sauvegardes automatiques de la base (ADR-0008, ADR-0015).
//!
//! Une sauvegarde quotidienne, une avant chaque migration et une avant chaque déploiement, par
//! `VACUUM INTO` dans `backups/`. Rétention des quotidiennes : 7 jours, 4 semaines, 12 mois.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::DataDir;
use crate::db::{self, Db};

const LAST_BACKUP_SETTING: &str = "backup.last";
const PREFIX: &str = "cashflow-";
const FORMAT: &str = "%Y%m%dT%H%M%SZ";
/// Sauvegardes non quotidiennes (avant migration, avant déploiement, manuelles) conservées.
const KEEP_OTHERS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LastBackup {
    pub at: String,
    pub file: String,
    pub reason: String,
}

/// Fait une sauvegarde et renvoie son chemin. `reason` : `daily`, `pre-migration`, `pre-deploy`, `manual`.
pub async fn backup(db: &Db, data: &DataDir, reason: &str) -> anyhow::Result<PathBuf> {
    data.ensure()?;
    let now = Utc::now();
    let path = data.backups().join(format!("{PREFIX}{}-{reason}.db", now.format(FORMAT)));
    db.vacuum_into(&path).await.with_context(|| format!("sauvegarde vers {}", path.display()))?;
    if db.pending_migrations().await?.is_empty() {
        let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let last = LastBackup { at: crate::clock::timestamp(), file, reason: reason.to_owned() };
        db::settings::set(&db.write, LAST_BACKUP_SETTING, &last).await?;
    }
    prune(&data.backups())?;
    tracing::info!(reason, file = %path.display(), "sauvegarde faite");
    Ok(path)
}

pub async fn last(db: &Db) -> anyhow::Result<Option<LastBackup>> {
    db::settings::get(&db.read, LAST_BACKUP_SETTING).await
}

struct BackupFile {
    path: PathBuf,
    at: DateTime<Utc>,
    daily: bool,
}

fn parse(path: &Path) -> Option<BackupFile> {
    let name = path.file_name()?.to_str()?.strip_prefix(PREFIX)?.strip_suffix(".db")?;
    let (stamp, reason) = name.split_once('-')?;
    let at = NaiveDateTime::parse_from_str(stamp, FORMAT).ok()?.and_utc();
    Some(BackupFile { path: path.to_owned(), at, daily: reason == "daily" })
}

/// Applique la rétention : pour les quotidiennes, la plus récente de chacun des 7 derniers jours, des
/// 4 dernières semaines et des 12 derniers mois ; pour les autres, les 10 plus récentes.
pub fn prune(dir: &Path) -> anyhow::Result<()> {
    let mut files: Vec<BackupFile> =
        std::fs::read_dir(dir)?.filter_map(Result::ok).filter_map(|e| parse(&e.path())).collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.at));
    let (daily, others): (Vec<_>, Vec<_>) = files.into_iter().partition(|f| f.daily);

    let mut keep: HashSet<PathBuf> = HashSet::new();
    for (key, count) in [(0, 7), (1, 4), (2, 12)] {
        let mut seen = Vec::new();
        for file in &daily {
            let bucket = match key {
                0 => (file.at.year(), file.at.ordinal(), 0),
                1 => (file.at.iso_week().year(), file.at.iso_week().week(), 1),
                _ => (file.at.year(), file.at.month(), 2),
            };
            if !seen.contains(&bucket) {
                if seen.len() == count {
                    break;
                }
                seen.push(bucket);
                keep.insert(file.path.clone());
            }
        }
    }
    keep.extend(others.iter().take(KEEP_OTHERS).map(|f| f.path.clone()));
    for file in daily.iter().chain(others.iter()) {
        if !keep.contains(&file.path) {
            std::fs::remove_file(&file.path).with_context(|| format!("suppression de {}", file.path.display()))?;
        }
    }
    Ok(())
}

/// Boucle de fond de `serve` : une sauvegarde quotidienne, vérifiée toutes les heures (ADR-0015).
pub async fn run_daily(db: Db, data: DataDir) {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));
    loop {
        interval.tick().await;
        let due = match last_daily_at(&data) {
            Some(at) => Utc::now() - at > chrono::Duration::hours(23),
            None => true,
        };
        if due && let Err(error) = backup(&db, &data, "daily").await {
            tracing::error!(error = format!("{error:#}"), "échec de la sauvegarde quotidienne");
        }
    }
}

fn last_daily_at(data: &DataDir) -> Option<DateTime<Utc>> {
    std::fs::read_dir(data.backups())
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| parse(&e.path()))
        .filter(|f| f.daily)
        .map(|f| f.at)
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, stamp: &str, reason: &str) {
        std::fs::write(dir.join(format!("{PREFIX}{stamp}-{reason}.db")), b"").unwrap();
    }

    #[test]
    fn retention_keeps_7_days_4_weeks_12_months() {
        let dir = tempfile::tempdir().unwrap();
        // Une sauvegarde quotidienne par jour pendant 400 jours.
        let start = NaiveDateTime::parse_from_str("20250101T020000Z", FORMAT).unwrap();
        for day in 0..400 {
            let at = start + chrono::Duration::days(day);
            touch(dir.path(), &at.format(FORMAT).to_string(), "daily");
        }
        for i in 0..12 {
            touch(dir.path(), &format!("202601{:02}T120000Z", i + 1), "pre-migration");
        }
        prune(dir.path()).unwrap();
        let remaining: Vec<BackupFile> =
            std::fs::read_dir(dir.path()).unwrap().filter_map(Result::ok).filter_map(|e| parse(&e.path())).collect();
        let daily = remaining.iter().filter(|f| f.daily).count();
        let others = remaining.iter().filter(|f| !f.daily).count();
        // 7 jours + au plus 4 semaines + au plus 12 mois, avec recouvrements.
        assert!((12..=23).contains(&daily), "{daily} sauvegardes quotidiennes conservées");
        assert_eq!(others, KEEP_OTHERS);
    }
}
