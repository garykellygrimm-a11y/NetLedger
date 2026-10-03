use std::{
    net::{IpAddr, SocketAddr},
    sync::LazyLock,
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, FromRequestParts, State},
    http::{StatusCode, request::Parts},
    routing::get,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    AppState,
    error::AppError,
    extractors::AppJson,
    passwords::{self, HashAlgorithm},
    setup,
};

pub const SESSION_COOKIE: &str = "netledger_session";
const FAILURE_WINDOW_MINUTES: i64 = 15;
const MAX_FAILURES_PER_ACCOUNT: i64 = 10;
const MAX_FAILURES_PER_ADDRESS: i64 = 50;
const SESSION_LIFETIME: Duration = Duration::hours(12);
const SESSION_IDLE_TIMEOUT: Duration = Duration::minutes(30);
const LAST_SEEN_WRITE_INTERVAL: Duration = Duration::seconds(60);

static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    passwords::hash_password(
        "no such account, keep timing uniform",
        HashAlgorithm::Argon2id,
    )
    .expect("hashing a constant cannot fail")
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Editor,
    Administrator,
}

impl Role {
    fn from_db(value: &str) -> Self {
        match value {
            "viewer" => Role::Viewer,
            "editor" => Role::Editor,
            "administrator" => Role::Administrator,
            other => unreachable!("account.role is constrained by the database, got {other}"),
        }
    }
}

impl Role {
    pub fn can_edit(self) -> bool {
        matches!(self, Role::Editor | Role::Administrator)
    }

    #[expect(dead_code, reason = "used by user management in 0.6.4")]
    pub fn is_administrator(self) -> bool {
        matches!(self, Role::Administrator)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CurrentUser {
    pub account_id: Uuid,
    pub username: String,
    pub role: Role,
    #[serde(skip)]
    pub session_id: Uuid,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE)
            .ok_or(AppError::Unauthorized)?
            .value();

        resolve_session(&state.db, token)
            .await?
            .ok_or(AppError::Unauthorized)
    }
}

pub struct Editor(pub CurrentUser);

impl FromRequestParts<AppState> for Editor {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if user.role.can_edit() {
            Ok(Editor(user))
        } else {
            Err(AppError::Forbidden)
        }
    }
}

#[expect(dead_code, reason = "used by user management in 0.6.4")]
pub struct Administrator(pub CurrentUser);

impl FromRequestParts<AppState> for Administrator {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if user.role.is_administrator() {
            Ok(Administrator(user))
        } else {
            Err(AppError::Forbidden)
        }
    }
}

pub struct ClientAddr(pub Option<IpAddr>);

impl FromRequestParts<AppState> for ClientAddr {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .ok()
            .map(|ConnectInfo(addr)| addr.ip());

        Ok(ClientAddr(client_address(
            peer,
            parts
                .headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok()),
            &state.trusted_proxies,
        )))
    }
}

fn client_address(
    peer: Option<IpAddr>,
    forwarded_for: Option<&str>,
    trusted: &[IpNet],
) -> Option<IpAddr> {
    let is_trusted = |ip: IpAddr| trusted.iter().any(|net| net.contains(&ip));

    let peer = peer?;
    if !is_trusted(peer) {
        return Some(peer);
    }

    let Some(forwarded_for) = forwarded_for else {
        return Some(peer);
    };

    forwarded_for
        .split(',')
        .rev()
        .filter_map(|entry| entry.trim().parse::<IpAddr>().ok())
        .find(|ip| !is_trusted(*ip))
        .or(Some(peer))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/session",
        get(current_session).post(sign_in).delete(sign_out),
    )
}

#[derive(Deserialize)]
struct SignIn {
    username: String,
    password: String,
}

async fn sign_in(
    State(state): State<AppState>,
    ClientAddr(source): ClientAddr,
    jar: CookieJar,
    AppJson(body): AppJson<SignIn>,
) -> Result<(StatusCode, CookieJar, Json<CurrentUser>), AppError> {
    let username = setup::normalize_username(&body.username).unwrap_or_default();

    if let Some(retry_after_secs) = failures_exceeded(&state.db, &username, source).await? {
        record_failure(&state.db, &username, source, "rate limited").await?;
        return Err(AppError::TooManyRequests { retry_after_secs });
    }

    let found = sqlx::query!(
        r#"
        SELECT a.id, a.username, a.role, a.disabled_at, i.id AS identity_id, i.secret_hash
        FROM account a
        JOIN identity i ON i.account_id = a.id AND i.kind = 'password'
        WHERE a.username = $1
        "#,
        username
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(account) = found else {
        let _ = passwords::verify_password(&body.password, DUMMY_HASH.as_str());
        record_failure(&state.db, &username, source, "unknown account").await?;
        return Err(AppError::Unauthorized);
    };

    let stored = account
        .secret_hash
        .as_deref()
        .unwrap_or(DUMMY_HASH.as_str());
    if !passwords::verify_password(&body.password, stored) {
        record_failure(&state.db, &account.username, source, "wrong password").await?;
        return Err(AppError::Unauthorized);
    }

    if account.disabled_at.is_some() {
        record_failure(&state.db, &account.username, source, "account disabled").await?;
        return Err(AppError::Unauthorized);
    }

    let mut tx = state.db.begin().await?;

    if passwords::needs_rehash(stored, state.password_hash) {
        let rehashed = passwords::hash_password(&body.password, state.password_hash)
            .map_err(|_| AppError::Unauthorized)?;
        sqlx::query!(
            "UPDATE identity SET secret_hash = $2 WHERE id = $1",
            account.identity_id,
            rehashed
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        "UPDATE identity SET last_used_at = now() WHERE id = $1",
        account.identity_id
    )
    .execute(&mut *tx)
    .await?;

    let token = new_token();
    let session_id = sqlx::query_scalar!(
        "INSERT INTO session (token_hash, account_id, expires_at) VALUES ($1, $2, $3) RETURNING id",
        token_hash(&token),
        account.id,
        Utc::now() + SESSION_LIFETIME
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO audit_log
            (actor_id, actor_username, credential_id, source_addr, action, outcome, target_type, target_id)
        VALUES ($1, $2, $3, $4, 'session.create', 'success', 'session', $3)
        "#,
        account.id,
        account.username,
        session_id,
        source.map(IpNet::from)
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let user = CurrentUser {
        account_id: account.id,
        username: account.username,
        role: Role::from_db(&account.role),
        session_id,
    };
    let jar = jar.add(session_cookie(&token, state.cookie_secure));

    Ok((StatusCode::CREATED, jar, Json(user)))
}

async fn current_session(user: CurrentUser) -> Json<CurrentUser> {
    Json(user)
}

async fn sign_out(
    State(state): State<AppState>,
    ClientAddr(source): ClientAddr,
    user: CurrentUser,
    jar: CookieJar,
) -> Result<(StatusCode, CookieJar), AppError> {
    let mut tx = state.db.begin().await?;

    sqlx::query!("DELETE FROM session WHERE id = $1", user.session_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        r#"
        INSERT INTO audit_log
            (actor_id, actor_username, credential_id, source_addr, action, outcome, target_type, target_id)
        VALUES ($1, $2, $3, $4, 'session.delete', 'success', 'session', $3)
        "#,
        user.account_id,
        user.username,
        user.session_id,
        source.map(IpNet::from)
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/").removal());

    Ok((StatusCode::NO_CONTENT, jar))
}

async fn resolve_session(db: &PgPool, token: &str) -> Result<Option<CurrentUser>, AppError> {
    let found = sqlx::query!(
        r#"
        SELECT s.id AS session_id, s.last_seen_at, s.expires_at,
               a.id AS account_id, a.username, a.role, a.disabled_at
        FROM session s
        JOIN account a ON a.id = s.account_id
        WHERE s.token_hash = $1
        "#,
        token_hash(token)
    )
    .fetch_optional(db)
    .await?;

    let Some(row) = found else {
        return Ok(None);
    };

    let now = Utc::now();
    let expired = row.expires_at <= now || row.last_seen_at + SESSION_IDLE_TIMEOUT <= now;
    if expired || row.disabled_at.is_some() {
        sqlx::query!("DELETE FROM session WHERE id = $1", row.session_id)
            .execute(db)
            .await?;
        return Ok(None);
    }

    if row.last_seen_at + LAST_SEEN_WRITE_INTERVAL <= now {
        sqlx::query!(
            "UPDATE session SET last_seen_at = now() WHERE id = $1",
            row.session_id
        )
        .execute(db)
        .await?;
    }

    Ok(Some(CurrentUser {
        account_id: row.account_id,
        username: row.username,
        role: Role::from_db(&row.role),
        session_id: row.session_id,
    }))
}

async fn failures_exceeded(
    db: &PgPool,
    username: &str,
    source: Option<IpAddr>,
) -> Result<Option<u64>, AppError> {
    let counts = sqlx::query!(
        r#"
        SELECT
            count(*) FILTER (WHERE actor_username = $1) AS "by_account!",
            count(*) FILTER (WHERE $2::inet IS NOT NULL AND source_addr = $2) AS "by_address!",
            min(occurred_at) AS oldest
        FROM audit_log
        WHERE action = 'session.create'
          AND outcome = 'failure'
          AND occurred_at > now() - ($3::int * interval '1 minute')
        "#,
        username,
        source.map(IpNet::from),
        FAILURE_WINDOW_MINUTES as i32
    )
    .fetch_one(db)
    .await?;

    if counts.by_account < MAX_FAILURES_PER_ACCOUNT && counts.by_address < MAX_FAILURES_PER_ADDRESS
    {
        return Ok(None);
    }

    let window = Duration::minutes(FAILURE_WINDOW_MINUTES);
    let retry_after = counts
        .oldest
        .map(|oldest| (oldest + window - Utc::now()).num_seconds().max(1))
        .unwrap_or(window.num_seconds());

    Ok(Some(retry_after as u64))
}

async fn record_failure(
    db: &PgPool,
    username: &str,
    source: Option<IpAddr>,
    reason: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO audit_log (actor_id, actor_username, source_addr, action, outcome, target_type, details)
        SELECT a.id, $1, $2, 'session.create', 'failure', 'account', jsonb_build_object('reason', $3::text)
        FROM (SELECT 1) AS one
        LEFT JOIN account a ON a.username = $1
        "#,
        username,
        source.map(IpNet::from),
        reason
    )
    .execute(db)
    .await?;

    Ok(())
}

pub(crate) fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system random source is unavailable");
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn session_cookie(token: &str, secure: bool) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token.to_owned()))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(SameSite::Strict)
        .build()
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
    use crate::test_support::send;

    const PASSWORD: &str = "correct horse battery staple";

    async fn seed(pool: &PgPool) {
        setup::create_administrator(pool, HashAlgorithm::Argon2id, "gary", PASSWORD)
            .await
            .unwrap();
    }

    async fn sign_in_as(
        app: &Router,
        username: &str,
        password: &str,
    ) -> (StatusCode, Option<String>, Value) {
        let body = json!({ "username": username, "password": password }).to_string();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = response.status();
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };

        (status, cookie, value)
    }

    async fn with_cookie(app: &Router, method: &str, uri: &str, cookie: &str) -> StatusCode {
        let token = cookie.split(';').next().unwrap();
        app.clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header(header::COOKIE, token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn sign_in_sets_a_hardened_cookie_and_returns_the_user(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool);

        let (status, cookie, body) = sign_in_as(&app, "Gary", PASSWORD).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["username"], "gary");
        assert_eq!(body["role"], "administrator");
        assert!(body.get("session_id").is_none());
        let cookie = cookie.unwrap();
        assert!(cookie.starts_with("netledger_session="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Path=/"));
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn wrong_password_and_unknown_user_look_identical(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool.clone());

        let (wrong_status, wrong_cookie, wrong_body) =
            sign_in_as(&app, "gary", "not the password at all").await;
        let (unknown_status, unknown_cookie, unknown_body) =
            sign_in_as(&app, "nobody", PASSWORD).await;

        assert_eq!(wrong_status, StatusCode::UNAUTHORIZED);
        assert_eq!(unknown_status, StatusCode::UNAUTHORIZED);
        assert!(wrong_cookie.is_none() && unknown_cookie.is_none());
        assert_eq!(wrong_body, unknown_body);

        let failures = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM audit_log WHERE action = 'session.create' AND outcome = 'failure'"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(failures, 2);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn the_session_cookie_identifies_the_user(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool);

        let (_, cookie, _) = sign_in_as(&app, "gary", PASSWORD).await;
        let status = with_cookie(&app, "GET", "/api/session", &cookie.unwrap()).await;

        assert_eq!(status, StatusCode::OK);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn no_cookie_or_a_forged_cookie_is_unauthorized(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool);

        let (status, body) = send(&app, "GET", "/api/session", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "authentication required");

        let forged = format!("{SESSION_COOKIE}={}", new_token());
        let status = with_cookie(&app, "GET", "/api/session", &forged).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn sign_out_ends_the_session_immediately(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool.clone());

        let (_, cookie, _) = sign_in_as(&app, "gary", PASSWORD).await;
        let cookie = cookie.unwrap();

        assert_eq!(
            with_cookie(&app, "DELETE", "/api/session", &cookie).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            with_cookie(&app, "GET", "/api/session", &cookie).await,
            StatusCode::UNAUTHORIZED
        );

        let sessions = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM session"#)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(sessions, 0);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn disabling_an_account_ends_its_sessions(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool.clone());

        let (_, cookie, _) = sign_in_as(&app, "gary", PASSWORD).await;
        let cookie = cookie.unwrap();
        sqlx::query!("UPDATE account SET disabled_at = now() WHERE username = 'gary'")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            with_cookie(&app, "GET", "/api/session", &cookie).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            sign_in_as(&app, "gary", PASSWORD).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn expired_and_idle_sessions_are_rejected_and_removed(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool.clone());

        let (_, idle, _) = sign_in_as(&app, "gary", PASSWORD).await;
        let (_, expired, _) = sign_in_as(&app, "gary", PASSWORD).await;
        let idle = idle.unwrap();
        let expired = expired.unwrap();

        sqlx::query!(
            "UPDATE session SET last_seen_at = now() - interval '31 minutes' WHERE token_hash = $1",
            cookie_hash(&idle)
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query!(
            "UPDATE session SET expires_at = now() - interval '1 second' WHERE token_hash = $1",
            cookie_hash(&expired)
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            with_cookie(&app, "GET", "/api/session", &idle).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            with_cookie(&app, "GET", "/api/session", &expired).await,
            StatusCode::UNAUTHORIZED
        );

        let sessions = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM session"#)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(sessions, 0);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn the_database_holds_only_a_hash_of_the_token(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool.clone());

        let (_, cookie, _) = sign_in_as(&app, "gary", PASSWORD).await;
        let token = cookie_token(&cookie.unwrap());

        let stored = sqlx::query_scalar!("SELECT token_hash FROM session")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_ne!(stored, token.as_bytes());
        assert_eq!(stored, token_hash(&token));
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_password_is_rehashed_when_the_configured_algorithm_changes(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app_with(pool.clone(), HashAlgorithm::Pbkdf2Sha256, true, Vec::new());

        assert_eq!(
            sign_in_as(&app, "gary", PASSWORD).await.0,
            StatusCode::CREATED
        );

        let hash = sqlx::query_scalar!("SELECT secret_hash FROM identity WHERE kind = 'password'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .unwrap();
        assert!(hash.starts_with("$pbkdf2-sha256$"));
        assert_eq!(
            sign_in_as(&app, "gary", PASSWORD).await.0,
            StatusCode::CREATED
        );
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn repeated_failures_lock_the_account_out_for_a_while(pool: PgPool) {
        seed(&pool).await;
        let app = crate::app(pool);

        for _ in 0..MAX_FAILURES_PER_ACCOUNT {
            assert_eq!(
                sign_in_as(&app, "gary", "wrong password every time")
                    .await
                    .0,
                StatusCode::UNAUTHORIZED
            );
        }

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({ "username": "gary", "password": PASSWORD }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after: u64 = response.headers()[header::RETRY_AFTER]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=900).contains(&retry_after));
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn api_responses_are_never_cached(pool: PgPool) {
        let app = crate::app(pool);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[test]
    fn forwarded_addresses_are_trusted_only_from_configured_proxies() {
        let f5: IpAddr = "10.0.0.5".parse().unwrap();
        let client: IpAddr = "192.0.2.10".parse().unwrap();
        let trusted = vec!["10.0.0.0/24".parse::<IpNet>().unwrap()];

        assert_eq!(
            client_address(Some(f5), Some("192.0.2.10"), &trusted),
            Some(client)
        );
        assert_eq!(
            client_address(Some(f5), Some("192.0.2.10, 10.0.0.7"), &trusted),
            Some(client)
        );
        assert_eq!(
            client_address(Some(client), Some("203.0.113.9"), &trusted),
            Some(client)
        );
        assert_eq!(client_address(Some(f5), None, &trusted), Some(f5));
        assert_eq!(
            client_address(Some(f5), Some("garbage"), &trusted),
            Some(f5)
        );
        assert_eq!(client_address(None, Some("192.0.2.10"), &trusted), None);
    }

    fn cookie_token(cookie: &str) -> String {
        cookie
            .split(';')
            .next()
            .unwrap()
            .trim_start_matches(&format!("{SESSION_COOKIE}="))
            .to_owned()
    }

    fn cookie_hash(cookie: &str) -> Vec<u8> {
        token_hash(&cookie_token(cookie))
    }
}
