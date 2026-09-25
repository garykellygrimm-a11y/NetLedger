mod config;
mod error;
mod extractors;
mod subnets;
mod web;

use std::time::Duration;

use anyhow::{Context, Result};
use axum::{Router, extract::State, http::StatusCode, routing::get};
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};
use tracing::{error, info};

use crate::{config::Config, error::AppError};

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

fn app(db: PgPool) -> Router {
    let router = Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(ready))
        .nest("/api", subnets::router().fallback(api_not_found))
        .fallback(web::serve)
        .with_state(AppState { db });

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

    let router = app(db);

    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_addr))?;
    info!("listening on {}", config.bind_addr);
    axum::serve(listener, router).await?;

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
