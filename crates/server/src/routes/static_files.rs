//! Fichiers statiques de l'interface, avec repli sur `index.html` pour le routage côté client.
//!
//! Après un déploiement, les fichiers JS changent de nom (empreinte du contenu) : `index.html` doit
//! donc toujours être revalidé, et un fichier absent répondre 404, jamais `index.html` (le navigateur
//! refuserait ce HTML comme module JS et la navigation échouerait sans message).

use std::path::PathBuf;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower_http::services::{ServeDir, ServeFile};

pub fn router(web_dir: PathBuf) -> Router {
    let files = ServeDir::new(&web_dir);
    let index = ServeFile::new(web_dir.join("index.html"));
    Router::new().fallback(move |request: Request| {
        let (mut files, mut index) = (files.clone(), index.clone());
        async move {
            if is_file(request.uri().path()) {
                return into_response(files.try_call(request).await);
            }
            let mut response = into_response(index.try_call(request).await);
            response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            response
        }
    })
}

/// Un chemin dont le dernier segment a une extension désigne un fichier ; sinon, une route de l'interface.
fn is_file(path: &str) -> bool {
    path.rsplit('/').next().is_some_and(|segment| segment.contains('.'))
}

fn into_response<B>(result: std::io::Result<axum::http::Response<B>>) -> Response
where
    B: axum::body::HttpBody<Data = axum::body::Bytes> + Send + 'static,
    B::Error: Into<axum::BoxError>,
{
    match result {
        Ok(response) => response.map(axum::body::Body::new),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, "lecture d'un fichier de l'interface");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use tower::ServiceExt;

    use super::*;

    async fn get(app: &Router, uri: &str) -> Response {
        let request = Request::get(uri).body(Body::empty()).expect("requête");
        app.clone().oneshot(request).await.expect("réponse")
    }

    #[tokio::test]
    async fn missing_asset_is_404_and_routes_get_fresh_index() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html></html>").unwrap();
        std::fs::write(dir.path().join("main-ABC.js"), "export {};").unwrap();
        let app = router(dir.path().to_path_buf());

        assert_eq!(get(&app, "/main-ABC.js").await.status(), StatusCode::OK);
        assert_eq!(get(&app, "/chunk-ANCIEN.js").await.status(), StatusCode::NOT_FOUND);
        for route in ["/", "/reglages", "/tiers/01900000-0000-7000-8000-000000000000"] {
            let response = get(&app, route).await;
            assert_eq!(response.status(), StatusCode::OK, "{route}");
            assert_eq!(response.headers()[header::CONTENT_TYPE], "text/html", "{route}");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache", "{route}");
        }
    }
}
