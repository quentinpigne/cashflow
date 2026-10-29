//! Handlers HTTP : extraient la requête, appellent un service et mettent en forme la réponse. Aucun
//! calcul ni aucune requête SQL ici. Chacun est décrit pour le contrat OpenAPI (ADR-0016).

pub mod accounts;
pub mod assets;
pub mod auth;
pub mod budgets;
pub mod categories;
pub mod dashboard;
pub mod documents;
pub mod imports;
pub mod institutions;
pub mod loans;
pub mod payees;
pub mod projections;
pub mod reconciliation;
pub mod reports;
pub mod rules;
pub mod salaries;
pub mod savings;
pub mod statements;
pub mod system;
pub mod transactions;

use axum::http::header;
use axum::response::IntoResponse;
use cashflow_domain::documents::MediaType;
use serde::Deserialize;
use utoipa::IntoParams;

/// Suppression d'un objet qui a des documents rattachés (R-DOC-12).
#[derive(Deserialize, IntoParams)]
pub struct DeleteQuery {
    /// Supprimer aussi les documents rattachés seulement à cet objet ; sinon ils restent dans la bibliothèque.
    #[serde(default)]
    delete_documents: bool,
}

#[derive(Deserialize, IntoParams)]
pub struct FileQuery {
    /// Télécharger plutôt qu'afficher dans le navigateur.
    #[serde(default)]
    download: bool,
}

/// Fichier d'un document archivé, affiché ou téléchargé, sans pouvoir rien exécuter dans l'origine de
/// l'application (ADR-0024).
fn document_response(
    file_name: &str,
    media_type: MediaType,
    content: Vec<u8>,
    download: bool,
) -> impl IntoResponse + use<> {
    let disposition = if download { "attachment" } else { "inline" };
    // Nom ASCII de repli, et nom exact encodé (RFC 6266) pour les accents.
    let ascii: String =
        file_name.chars().filter(|c| (c.is_ascii_graphic() || *c == ' ') && !matches!(c, '"' | '\\')).collect();
    let encoded: String = file_name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    (
        [
            (header::CONTENT_TYPE, media_type.as_str().to_owned()),
            (header::CONTENT_DISPOSITION, format!("{disposition}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
            (header::CONTENT_SECURITY_POLICY, "sandbox".to_owned()),
            (header::CACHE_CONTROL, "private, no-store".to_owned()),
        ],
        content,
    )
}
