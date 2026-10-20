//! Authentification mono-utilisateur (ADR-0006, ADR-0015).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use chrono::{SecondsFormat, Utc};
use sha2::{Digest, Sha256};

use crate::clock::timestamp;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Durée de vie d'une session, prolongée à chaque usage (30 jours glissants).
pub const SESSION_DAYS: i64 = 30;
pub const COOKIE_NAME: &str = "cashflow_session";
/// Tentatives libres avant que l'attente ne s'impose.
const FREE_ATTEMPTS: u32 = 3;
const MAX_WAIT: Duration = Duration::from_secs(15 * 60);

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let hash = Argon2::default().hash_password(password.as_bytes()).map_err(|e| anyhow!("hachage : {e}"))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| anyhow!("hachage stocké illisible : {e}"))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

/// Empreinte d'un jeton de session : seul elle est stockée (modèle de données).
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn expiry() -> String {
    (Utc::now() + chrono::Duration::days(SESSION_DAYS)).to_rfc3339_opts(SecondsFormat::Micros, true)
}

/// Définit (ou remplace) le mot de passe de l'utilisateur unique, et ferme toutes les sessions.
pub async fn set_password(db: &Db, display_name: &str, password: &str) -> anyhow::Result<()> {
    if password.chars().count() < 8 {
        anyhow::bail!("le mot de passe doit faire au moins 8 caractères");
    }
    let password = password.to_owned();
    let hash = tokio::task::spawn_blocking(move || hash_password(&password)).await??;
    let mut tx = db.write.begin().await?;
    match db::auth::find_user(&mut *tx).await? {
        Some(user) => db::auth::update_password(&mut *tx, user.id, &hash).await?,
        None => db::auth::insert_user(&mut *tx, display_name, &hash).await?,
    }
    db::auth::delete_all_sessions(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn password_is_set(db: &Db) -> anyhow::Result<bool> {
    Ok(db::auth::find_user(&db.read).await?.is_some())
}

/// Ouvre une session et renvoie son jeton, à placer dans le cookie.
pub async fn login(
    db: &Db,
    limiter: &LoginLimiter,
    ip: IpAddr,
    password: &str,
    device: Option<&str>,
) -> AppResult<String> {
    limiter.check(ip)?;
    let user = db::auth::find_user(&db.read).await?.ok_or(AppError::NoPassword)?;
    let (password, hash) = (password.to_owned(), user.password_hash.clone());
    let valid =
        tokio::task::spawn_blocking(move || verify_password(&password, &hash)).await.context("vérification")??;
    if !valid {
        limiter.failure(ip);
        return Err(AppError::WrongPassword);
    }
    limiter.success(ip);
    let token = hex::encode(rand::random::<[u8; 32]>());
    let mut conn = db.write.acquire().await?;
    db::auth::delete_expired_sessions(&mut *conn, &timestamp()).await?;
    db::auth::insert_session(&mut *conn, user.id, &hash_token(&token), device, &expiry()).await?;
    Ok(token)
}

/// Vérifie un jeton de session et prolonge la session (30 jours glissants).
pub async fn authenticate(db: &Db, token: &str) -> AppResult<()> {
    let token_hash = hash_token(token);
    let session = db::auth::find_session(&db.read, &token_hash).await?.ok_or(AppError::Unauthorized)?;
    let now = timestamp();
    if session.expires_at < now {
        return Err(AppError::Unauthorized);
    }
    // Prolonger au plus une fois par heure, pour ne pas écrire à chaque requête.
    let an_hour_ago = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Micros, true);
    if session.last_seen_at < an_hour_ago {
        db::auth::touch_session(&db.write, session.id, &expiry()).await?;
    }
    Ok(())
}

pub async fn logout(db: &Db, token: &str) -> AppResult<()> {
    db::auth::delete_session(&db.write, &hash_token(token)).await?;
    Ok(())
}

/// « Se déconnecter de tous les appareils » (ADR-0015).
pub async fn logout_everywhere(db: &Db) -> AppResult<()> {
    db::auth::delete_all_sessions(&db.write).await?;
    Ok(())
}

/// Limitation des tentatives de connexion : attente exponentielle par adresse IP (ADR-0015).
#[derive(Default)]
pub struct LoginLimiter {
    attempts: Mutex<HashMap<IpAddr, Attempts>>,
}

struct Attempts {
    failures: u32,
    blocked_until: Option<Instant>,
}

impl LoginLimiter {
    pub fn check(&self, ip: IpAddr) -> AppResult<()> {
        let attempts = self.attempts.lock().map_err(|_| anyhow!("verrou empoisonné"))?;
        if let Some(until) = attempts.get(&ip).and_then(|a| a.blocked_until) {
            let now = Instant::now();
            if until > now {
                let wait = until - now;
                return Err(AppError::TooManyAttempts { retry_after_secs: wait.as_secs() + 1 });
            }
        }
        Ok(())
    }

    pub fn failure(&self, ip: IpAddr) {
        let Ok(mut attempts) = self.attempts.lock() else { return };
        let entry = attempts.entry(ip).or_insert(Attempts { failures: 0, blocked_until: None });
        entry.failures += 1;
        if entry.failures >= FREE_ATTEMPTS {
            let exponent = (entry.failures - FREE_ATTEMPTS).min(16);
            let wait = Duration::from_secs(1 << exponent).min(MAX_WAIT);
            entry.blocked_until = Some(Instant::now() + wait);
        }
    }

    pub fn success(&self, ip: IpAddr) {
        if let Ok(mut attempts) = self.attempts.lock() {
            attempts.remove(&ip);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("correct horse").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("correct horse", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn limiter_blocks_after_free_attempts() {
        let limiter = LoginLimiter::default();
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        for _ in 0..FREE_ATTEMPTS - 1 {
            limiter.failure(ip);
            assert!(limiter.check(ip).is_ok());
        }
        limiter.failure(ip);
        assert!(matches!(limiter.check(ip), Err(AppError::TooManyAttempts { .. })));
        limiter.success(ip);
        assert!(limiter.check(ip).is_ok());
    }

    #[tokio::test]
    async fn login_flow() {
        let (db, _dir) = crate::db::test_db().await;
        let limiter = LoginLimiter::default();
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert!(matches!(login(&db, &limiter, ip, "x", None).await, Err(AppError::NoPassword)));
        set_password(&db, "Moi", "motdepasse").await.unwrap();
        assert!(matches!(login(&db, &limiter, ip, "mauvais", None).await, Err(AppError::WrongPassword)));
        let token = login(&db, &limiter, ip, "motdepasse", Some("test")).await.unwrap();
        authenticate(&db, &token).await.unwrap();
        logout_everywhere(&db).await.unwrap();
        assert!(matches!(authenticate(&db, &token).await, Err(AppError::Unauthorized)));
    }
}
