use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use ipnet::IpNet;
use serde::Serialize;
use uuid::Uuid;

use crate::{AppState, error::AppError};

#[derive(Serialize)]
pub struct Subnet {
    pub id: Uuid,
    pub cidr: IpNet,
    pub name: String,
    pub description: String,
    pub vlan_id: Option<i32>,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subnets", get(list_subnets))
        .route("/subnets/{id}", get(get_subnet))
}

async fn list_subnets(State(state): State<AppState>) -> Result<Json<Vec<Subnet>>, AppError> {
    let subnets = sqlx::query_as!(
        Subnet,
        r#"
        SELECT id, cidr AS "cidr: IpNet", name, description, vlan_id, parent_id, created_at, updated_at
        FROM subnet
        ORDER BY cidr
        "#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(subnets))
}

async fn get_subnet(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Subnet>, AppError> {
    let subnet = sqlx::query_as!(
        Subnet,
        r#"
        SELECT id, cidr AS "cidr: IpNet", name, description, vlan_id, parent_id, created_at, updated_at
        FROM subnet
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(subnet))
}
