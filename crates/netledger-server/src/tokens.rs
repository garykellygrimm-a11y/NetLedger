use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    AppState,
    error::AppError,
    extractors::{AppJson, AppPath},
    sessions::{ClientAddr, Credential, CurrentUser, Role},
};

pub const TOKEN_PREFIX: &str = "nlt_";
const NAME_MAX_CHARS: usize = 100;
const MAX_LIFETIME_DAYS: i64 = 365;
const LAST_USED_WRITE_INTERVAL: Duration = Duration::seconds(60);

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tokens", get(list_tokens).post(create_token))
        .route("/tokens/{id}", delete(revoke_token))
}

#[derive(Serialize)]
pub struct ApiToken {
    pub id: Uuid,
    pub name: String,
    pub hint: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct CreatedToken {
    #[serde(flatten)]
    token: ApiToken,
    secret: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateToken {
    name: String,
    expires_in_days: Option<i64>,
}

async fn list_tokens(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<ApiToken>>, AppError> {
    let tokens = sqlx::query_as!(
        ApiToken,
        r#"
        SELECT id, name, hint, created_at, expires_at, last_used_at, revoked_at
        FROM api_token
        WHERE account_id = $1
        ORDER BY created_at DESC
        "#,
        user.account_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(tokens))
}

async fn create_token(
    State(state): State<AppState>,
    ClientAddr(source): ClientAddr,
    user: CurrentUser,
    AppJson(input): AppJson<CreateToken>,
) -> Result<(StatusCode, Json<CreatedToken>), AppError> {
    let session_id = user.credential.session_id()?;

    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be blank".to_string()));
    }
    if name.chars().count() > NAME_MAX_CHARS {
        return Err(AppError::BadRequest(format!(
            "name must be at most {NAME_MAX_CHARS} characters"
        )));
    }
    let expires_at = match input.expires_in_days {
        None => None,
        Some(days) if (1..=MAX_LIFETIME_DAYS).contains(&days) => {
            Some(Utc::now() + Duration::days(days))
        }
        Some(_) => {
            return Err(AppError::BadRequest(format!(
                "expires_in_days must be between 1 and {MAX_LIFETIME_DAYS}"
            )));
        }
    };

    let secret = new_secret();
    let hint = secret[secret.len() - 4..].to_string();

    let mut tx = state.db.begin().await?;

    let token = sqlx::query_as!(
        ApiToken,
        r#"
        INSERT INTO api_token (account_id, name, token_hash, hint, expires_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, name, hint, created_at, expires_at, last_used_at, revoked_at
        "#,
        user.account_id,
        name,
        secret_hash(&secret),
        hint,
        expires_at
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO audit_log
            (actor_id, actor_username, credential_id, source_addr, action, outcome, target_type, target_id, details)
        VALUES ($1, $2, $3, $4, 'token.create', 'success', 'api_token', $5,
                jsonb_build_object('name', $6::text, 'expires_at', $7::timestamptz))
        "#,
        user.account_id,
        user.username,
        session_id,
        source.map(ipnet::IpNet::from),
        token.id,
        name,
        expires_at
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(CreatedToken { token, secret })))
}

async fn revoke_token(
    State(state): State<AppState>,
    ClientAddr(source): ClientAddr,
    user: CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let session_id = user.credential.session_id()?;
    let mut tx = state.db.begin().await?;

    let revoked = sqlx::query_scalar!(
        r#"
        UPDATE api_token
        SET revoked_at = now()
        WHERE id = $1 AND account_id = $2 AND revoked_at IS NULL
        RETURNING name
        "#,
        id,
        user.account_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(name) = revoked else {
        return Err(AppError::NotFound);
    };

    sqlx::query!(
        r#"
        INSERT INTO audit_log
            (actor_id, actor_username, credential_id, source_addr, action, outcome, target_type, target_id, details)
        VALUES ($1, $2, $3, $4, 'token.revoke', 'success', 'api_token', $5, jsonb_build_object('name', $6::text))
        "#,
        user.account_id,
        user.username,
        session_id,
        source.map(ipnet::IpNet::from),
        id,
        name
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn resolve_token(
    db: &PgPool,
    secret: &str,
) -> Result<Option<CurrentUser>, AppError> {
    if !secret.starts_with(TOKEN_PREFIX) {
        return Ok(None);
    }

    let found = sqlx::query!(
        r#"
        SELECT t.id AS token_id, t.expires_at, t.revoked_at, t.last_used_at,
               a.id AS account_id, a.username, a.role, a.disabled_at
        FROM api_token t
        JOIN account a ON a.id = t.account_id
        WHERE t.token_hash = $1
        "#,
        secret_hash(secret)
    )
    .fetch_optional(db)
    .await?;

    let Some(row) = found else {
        return Ok(None);
    };

    let now = Utc::now();
    let unusable = row.revoked_at.is_some()
        || row.disabled_at.is_some()
        || row.expires_at.is_some_and(|expires_at| expires_at <= now);
    if unusable {
        return Ok(None);
    }

    let stale = row
        .last_used_at
        .is_none_or(|last_used_at| last_used_at + LAST_USED_WRITE_INTERVAL <= now);
    if stale {
        sqlx::query!(
            "UPDATE api_token SET last_used_at = now() WHERE id = $1",
            row.token_id
        )
        .execute(db)
        .await?;
    }

    Ok(Some(CurrentUser {
        account_id: row.account_id,
        username: row.username,
        role: Role::from_db(&row.role),
        credential: Credential::Token(row.token_id),
    }))
}

fn new_secret() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system random source is unavailable");
    format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn secret_hash(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    use super::*;
    use crate::test_support::{app_as, send};

    async fn create(app: &Router, body: Value) -> (StatusCode, Value) {
        send(app, "POST", "/api/tokens", Some(body)).await
    }

    async fn with_bearer(
        app: &Router,
        method: &str,
        uri: &str,
        bearer: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(match body {
                Some(body) => Body::from(body.to_string()),
                None => Body::empty(),
            })
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, value)
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_token_is_shown_once_with_a_prefix_and_a_hint(pool: PgPool) {
        let app = app_as(pool, Role::Editor).await;

        let (status, body) = create(&app, json!({ "name": "pipeline" })).await;

        assert_eq!(status, StatusCode::CREATED);
        let secret = body["secret"].as_str().unwrap();
        assert!(secret.starts_with(TOKEN_PREFIX));
        assert_eq!(secret.len(), TOKEN_PREFIX.len() + 43);
        assert_eq!(body["hint"], secret[secret.len() - 4..]);
        assert_eq!(body["name"], "pipeline");
        assert!(body["expires_at"].is_null());

        let (_, listed) = send(&app, "GET", "/api/tokens", None).await;
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert!(listed[0].get("secret").is_none());
        assert_eq!(listed[0]["hint"], body["hint"]);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_bearer_token_acts_with_its_accounts_role(pool: PgPool) {
        let editor = app_as(pool.clone(), Role::Editor).await;
        let (_, created) = create(&editor, json!({ "name": "pipeline" })).await;
        let secret = created["secret"].as_str().unwrap();
        let bare = crate::app(pool.clone());

        let (status, body) = with_bearer(&bare, "GET", "/api/session", secret, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["username"], "test-editor");

        let (status, subnet) = with_bearer(
            &bare,
            "POST",
            "/api/subnets",
            secret,
            Some(json!({ "cidr": "10.0.0.0/24", "name": "Via token" })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let credential_id = sqlx::query_scalar!(
            "SELECT credential_id FROM audit_log WHERE action = 'subnet.create' AND target_id = $1",
            Uuid::parse_str(subnet["id"].as_str().unwrap()).unwrap()
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            credential_id.unwrap().to_string(),
            created["id"].as_str().unwrap()
        );
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_viewers_token_cannot_write(pool: PgPool) {
        let viewer = app_as(pool.clone(), Role::Viewer).await;
        let (_, created) = create(&viewer, json!({ "name": "read only" })).await;
        let secret = created["secret"].as_str().unwrap();
        let bare = crate::app(pool);

        let (status, _) = with_bearer(&bare, "GET", "/api/subnets", secret, None).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = with_bearer(
            &bare,
            "POST",
            "/api/subnets",
            secret,
            Some(json!({ "cidr": "10.0.0.0/24", "name": "Nope" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn tokens_cannot_manage_tokens(pool: PgPool) {
        let editor = app_as(pool.clone(), Role::Editor).await;
        let (_, created) = create(&editor, json!({ "name": "first" })).await;
        let secret = created["secret"].as_str().unwrap();
        let bare = crate::app(pool);

        let (status, body) = with_bearer(
            &bare,
            "POST",
            "/api/tokens",
            secret,
            Some(json!({ "name": "second" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(body["error"].as_str().unwrap().contains("API token"));

        let uri = format!("/api/tokens/{}", created["id"].as_str().unwrap());
        let (status, _) = with_bearer(&bare, "DELETE", &uri, secret, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, _) = with_bearer(&bare, "DELETE", "/api/session", secret, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn revoked_expired_and_unknown_tokens_are_rejected(pool: PgPool) {
        let editor = app_as(pool.clone(), Role::Editor).await;
        let (_, revoked) = create(&editor, json!({ "name": "revoked" })).await;
        let (_, expired) =
            create(&editor, json!({ "name": "expired", "expires_in_days": 1 })).await;
        let bare = crate::app(pool.clone());

        let uri = format!("/api/tokens/{}", revoked["id"].as_str().unwrap());
        assert_eq!(
            send(&editor, "DELETE", &uri, None).await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            send(&editor, "DELETE", &uri, None).await.0,
            StatusCode::NOT_FOUND
        );

        sqlx::query!(
            "UPDATE api_token SET expires_at = now() - interval '1 second' WHERE id = $1",
            Uuid::parse_str(expired["id"].as_str().unwrap()).unwrap()
        )
        .execute(&pool)
        .await
        .unwrap();

        for secret in [
            revoked["secret"].as_str().unwrap(),
            expired["secret"].as_str().unwrap(),
            &new_secret(),
            "not even a token",
        ] {
            let (status, _) = with_bearer(&bare, "GET", "/api/session", secret, None).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{secret}");
        }

        let (_, listed) = send(&editor, "GET", "/api/tokens", None).await;
        let revoked_entry = listed
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "revoked")
            .unwrap();
        assert!(!revoked_entry["revoked_at"].is_null());
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_token_records_when_it_was_last_used(pool: PgPool) {
        let editor = app_as(pool.clone(), Role::Editor).await;
        let (_, created) = create(&editor, json!({ "name": "pipeline" })).await;
        let secret = created["secret"].as_str().unwrap();
        assert!(created["last_used_at"].is_null());

        with_bearer(&crate::app(pool), "GET", "/api/session", secret, None).await;

        let (_, listed) = send(&editor, "GET", "/api/tokens", None).await;
        assert!(!listed[0]["last_used_at"].is_null());
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn token_input_is_validated(pool: PgPool) {
        let app = app_as(pool, Role::Editor).await;

        let (status, body) = create(&app, json!({ "name": "   " })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "name must not be blank");

        let (status, _) = create(&app, json!({ "name": "x".repeat(101) })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = create(&app, json!({ "name": "ok", "expires_in_days": 0 })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = create(&app, json!({ "name": "ok", "expires_in_days": 366 })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _) = create(&app, json!({ "name": "ok", "unknown": 1 })).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

        let (status, body) = create(&app, json!({ "name": "ok", "expires_in_days": 30 })).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(!body["expires_at"].is_null());
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn tokens_belong_to_their_owner(pool: PgPool) {
        let editor = app_as(pool.clone(), Role::Editor).await;
        let (_, created) = create(&editor, json!({ "name": "mine" })).await;

        let admin = app_as(pool, Role::Administrator).await;
        let (_, listed) = send(&admin, "GET", "/api/tokens", None).await;
        assert_eq!(listed.as_array().unwrap().len(), 0);

        let uri = format!("/api/tokens/{}", created["id"].as_str().unwrap());
        assert_eq!(
            send(&admin, "DELETE", &uri, None).await.0,
            StatusCode::NOT_FOUND
        );
    }
}
