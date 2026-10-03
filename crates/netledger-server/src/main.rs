mod addresses;
mod config;
mod error;
mod extractors;
mod passwords;
mod sessions;
mod setup;
mod subnets;
#[cfg(test)]
mod test_support;
mod web;

use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result, bail};
use axum::{Router, extract::State, http::StatusCode, routing::get};
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};
use tracing::{error, info, warn};

use crate::{config::Config, error::AppError, passwords::HashAlgorithm};

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Clone)]
struct AppState {
    db: PgPool,
    password_hash: HashAlgorithm,
    cookie_secure: bool,
}

#[cfg(test)]
fn app(db: PgPool) -> Router {
    app_with(db, HashAlgorithm::Argon2id, true)
}

fn app_with(db: PgPool, password_hash: HashAlgorithm, cookie_secure: bool) -> Router {
    let router = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(ready))
        .nest(
            "/api",
            sessions::router()
                .merge(subnets::router())
                .merge(addresses::router())
                .fallback(api_not_found),
        )
        .fallback(web::serve)
        .with_state(AppState {
            db,
            password_hash,
            cookie_secure,
        });

    web::with_security_headers(router)
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "netledger_server=info".into()),
        )
        .init();

    let config = Config::from_env()?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await
        .context("failed to connect to the database")?;

    MIGRATOR
        .run(&db)
        .await
        .context("failed to run database migrations")?;
    info!("database migrations are up to date");

    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => serve(db, &config).await,
        Some("create-admin") => {
            let username = args
                .next()
                .context("usage: netledger-server create-admin <username>")?;
            setup::create_admin_interactively(&db, config.password_hash, &username).await
        }
        Some(other) => {
            bail!("unknown command {other}; usage: netledger-server [create-admin <username>]")
        }
    }
}

async fn serve(db: PgPool, config: &Config) -> Result<()> {
    if !config.cookie_secure {
        warn!("NETLEDGER_COOKIE_SECURE=false: session cookies may be sent over plain HTTP");
    }
    let router = app_with(db, config.password_hash, config.cookie_secure);

    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_addr))?;
    info!("listening on {}", config.bind_addr);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn ready(State(state): State<AppState>) -> (StatusCode, &'static str) {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => (StatusCode::OK, "ready"),
        Err(err) => {
            error!(error = %err, "readiness check failed");
            (StatusCode::SERVICE_UNAVAILABLE, "database unavailable")
        }
    }
}

async fn api_not_found() -> AppError {
    AppError::NotFound
}
