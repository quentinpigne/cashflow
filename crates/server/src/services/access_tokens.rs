//! Jetons d'accès en lecture seule pour une application tierce, Foodflow (ADR-0023).

use axum::http::Method;
use chrono::{SecondsFormat, Utc};

use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::services::auth::hash_token;

/// Préfixe des jetons, pour les reconnaître dans une configuration.
const PREFIX: &str = "cf_";

/// Routes lisibles avec un jeton : celles dont Foodflow a besoin (ADR-0023). Toute autre route répond 403.
const READABLE_ROUTES: &[&str] = &[
    "/api/v1/categories",
    "/api/v1/budget",
    "/api/v1/transactions",
    "/api/v1/transactions/{id}",
    "/api/v1/payees",
    "/api/v1/accounts",
];

pub struct AccessTokenSummary {
    pub name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// Un jeton n'autorise que la lecture des routes de `READABLE_ROUTES` (`route` : chemin déclaré, pas l'URL).
pub fn allows(method: &Method, route: &str) -> bool {
    (method == Method::GET || method == Method::HEAD) && READABLE_ROUTES.contains(&route)
}

/// Crée un jeton et le renvoie en clair : il ne sera plus jamais affiché.
pub async fn create(db: &Db, name: &str) -> anyhow::Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        anyhow::bail!("le nom du jeton doit faire de 1 à 80 caractères");
    }
    let mut tx = db.write.begin().await?;
    let user = db::auth::find_user(&mut *tx).await?.ok_or(AppError::NoPassword)?;
    if db::auth::list_access_tokens(&mut *tx).await?.iter().any(|t| t.name == name) {
        anyhow::bail!("un jeton nommé « {name} » existe déjà : le révoquer d'abord");
    }
    let token = format!("{PREFIX}{}", hex::encode(rand::random::<[u8; 32]>()));
    db::auth::insert_access_token(&mut *tx, user.id, name, &hash_token(&token)).await?;
    tx.commit().await?;
    Ok(token)
}

pub async fn list(db: &Db) -> anyhow::Result<Vec<AccessTokenSummary>> {
    Ok(db::auth::list_access_tokens(&db.read)
        .await?
        .into_iter()
        .map(|t| AccessTokenSummary { name: t.name, created_at: t.created_at, last_used_at: t.last_used_at })
        .collect())
}

pub async fn revoke(db: &Db, name: &str) -> anyhow::Result<()> {
    if !db::auth::delete_access_token(&db.write, name.trim()).await? {
        anyhow::bail!("aucun jeton nommé « {} »", name.trim());
    }
    Ok(())
}

/// Vérifie un jeton et note son dernier usage, au plus une fois par heure.
pub async fn authenticate(db: &Db, token: &str) -> AppResult<()> {
    let found = db::auth::find_access_token(&db.read, &hash_token(token)).await?.ok_or(AppError::Unauthorized)?;
    let an_hour_ago = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Micros, true);
    if found.last_used_at.is_none_or(|used| used < an_hour_ago) {
        db::auth::touch_access_token(&db.write, found.id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::auth::set_password;

    #[test]
    fn token_reads_only_listed_routes() {
        assert!(allows(&Method::GET, "/api/v1/budget"));
        assert!(allows(&Method::GET, "/api/v1/transactions/{id}"));
        assert!(!allows(&Method::POST, "/api/v1/categories"));
        assert!(!allows(&Method::DELETE, "/api/v1/transactions/{id}"));
        assert!(!allows(&Method::GET, "/api/v1/accounts/{id}/iban"));
        assert!(!allows(&Method::GET, "/api/v1/export"));
    }

    #[tokio::test]
    async fn token_lifecycle() {
        let (db, _dir) = db::test_db().await;
        assert!(create(&db, "foodflow").await.is_err(), "sans utilisateur, pas de jeton");
        set_password(&db, "Moi", "motdepasse").await.unwrap();

        let token = create(&db, " foodflow ").await.unwrap();
        assert!(token.starts_with(PREFIX));
        assert!(create(&db, "foodflow").await.is_err(), "nom unique");
        authenticate(&db, &token).await.unwrap();
        assert!(matches!(authenticate(&db, "cf_inconnu").await, Err(AppError::Unauthorized)));

        let tokens = list(&db).await.unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].name, "foodflow");
        assert!(tokens[0].last_used_at.is_some());

        // Changer le mot de passe ferme les sessions, pas les jetons.
        set_password(&db, "Moi", "autremotdepasse").await.unwrap();
        authenticate(&db, &token).await.unwrap();

        revoke(&db, "foodflow").await.unwrap();
        assert!(matches!(authenticate(&db, &token).await, Err(AppError::Unauthorized)));
        assert!(revoke(&db, "foodflow").await.is_err());
    }
}
