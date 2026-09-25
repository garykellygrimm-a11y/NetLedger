use axum::{
    Router,
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;
use tower_http::set_header::SetResponseHeaderLayer;

#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../web/dist"]
#[allow_missing = true]
struct Assets;

const INDEX: &str = "index.html";

const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; \
    img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; \
    base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(response) = asset(path) {
        return response;
    }

    if path.contains('.') {
        return StatusCode::NOT_FOUND.into_response();
    }

    asset(INDEX).unwrap_or_else(not_built)
}

fn asset(path: &str) -> Option<Response> {
    let file = Assets::get(path)?;
    let content_type = file.metadata.mimetype().to_owned();
    let cache_control = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };

    Some(
        (
            [
                (header::CONTENT_TYPE, content_type.as_str()),
                (header::CACHE_CONTROL, cache_control),
            ],
            file.data,
        )
            .into_response(),
    )
}

fn not_built() -> Response {
    (
        StatusCode::NOT_FOUND,
        "The web front end is not included in this build. Run `npm run build` in web/, then rebuild the server.",
    )
        .into_response()
}

pub fn with_security_headers(router: Router) -> Router {
    router
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode, header},
        response::Response,
    };
    use http_body_util::BodyExt;
    use sqlx::PgPool;
    use tower::ServiceExt;

    async fn get(app: Router, uri: &str) -> Response {
        app.oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn unknown_api_route_returns_json_404(pool: PgPool) {
        let response = get(crate::app(pool), "/api/does-not-exist").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"], "not found");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn missing_asset_file_returns_404(pool: PgPool) {
        let response = get(crate::app(pool), "/assets/does-not-exist.js").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn responses_carry_security_headers(pool: PgPool) {
        let response = get(crate::app(pool), "/health").await;
        let headers = response.headers();

        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
        assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
        assert!(
            headers[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .contains("frame-ancestors 'none'")
        );
    }
}
