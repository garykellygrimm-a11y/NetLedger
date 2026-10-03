use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::{ServiceExt, util::MapRequestLayer};

use crate::{
    passwords::HashAlgorithm,
    sessions::{self, Role},
    setup,
};

pub async fn app_as(pool: PgPool, role: Role) -> Router {
    let username = match role {
        Role::Viewer => "test-viewer",
        Role::Editor => "test-editor",
        Role::Administrator => "test-admin",
    };
    let account_id = setup::create_administrator(
        &pool,
        HashAlgorithm::Argon2id,
        username,
        "test fixture password, long enough",
    )
    .await
    .unwrap();
    let role_name = match role {
        Role::Viewer => "viewer",
        Role::Editor => "editor",
        Role::Administrator => "administrator",
    };
    sqlx::query!(
        "UPDATE account SET role = $2 WHERE id = $1",
        account_id,
        role_name
    )
    .execute(&pool)
    .await
    .unwrap();

    let token = sessions::new_token();
    sqlx::query!(
        "INSERT INTO session (token_hash, account_id, expires_at) VALUES ($1, $2, now() + interval '1 hour')",
        sessions::token_hash(&token),
        account_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let cookie = HeaderValue::from_str(&format!("{}={token}", sessions::SESSION_COOKIE)).unwrap();
    crate::app(pool).layer(MapRequestLayer::new(move |mut request: Request<Body>| {
        request.headers_mut().insert(header::COOKIE, cookie.clone());
        request
    }))
}

pub async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(json) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };

    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };

    (status, value)
}
