use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use chrono::{DateTime, Utc};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    error::AppError,
    extractors::{AppJson, AppPath},
};

const NAME_MAX_CHARS: usize = 100;
const DESCRIPTION_MAX_CHARS: usize = 1000;

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSubnet {
    pub cidr: IpNet,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub vlan_id: Option<i32>,
    pub parent_id: Option<Uuid>,
}

impl CreateSubnet {
    fn validate(&self) -> Result<(), AppError> {
        let network = self.cidr.trunc();
        if self.cidr != network {
            return Err(AppError::BadRequest(format!(
                "cidr {} has host bits set; the network address is {network}",
                self.cidr
            )));
        }

        let name_len = self.name.trim().chars().count();
        if name_len == 0 {
            return Err(AppError::BadRequest("name must not be empty".to_string()));
        }
        if name_len > NAME_MAX_CHARS {
            return Err(AppError::BadRequest(format!(
                "name must be at most {NAME_MAX_CHARS} characters"
            )));
        }

        if self.description.chars().count() > DESCRIPTION_MAX_CHARS {
            return Err(AppError::BadRequest(format!(
                "description must be at most {DESCRIPTION_MAX_CHARS} characters"
            )));
        }

        if self.vlan_id.is_some_and(|vlan| !(1..=4094).contains(&vlan)) {
            return Err(AppError::BadRequest(
                "vlan_id must be between 1 and 4094".to_string(),
            ));
        }

        Ok(())
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subnets", get(list_subnets).post(create_subnet))
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
    AppPath(id): AppPath<Uuid>,
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

async fn create_subnet(
    State(state): State<AppState>,
    AppJson(input): AppJson<CreateSubnet>,
) -> Result<(StatusCode, Json<Subnet>), AppError> {
    input.validate()?;

    let subnet = sqlx::query_as!(
        Subnet,
        r#"
        INSERT INTO subnet (cidr, name, description, vlan_id, parent_id)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, cidr AS "cidr: IpNet", name, description, vlan_id, parent_id, created_at, updated_at
        "#,
        input.cidr,
        input.name.trim(),
        input.description,
        input.vlan_id,
        input.parent_id,
    )
    .fetch_one(&state.db)
    .await
    .map_err(map_create_error)?;

    Ok((StatusCode::CREATED, Json(subnet)))
}

fn map_create_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err {
        match db_err.constraint() {
            Some("subnet_cidr_key") => {
                return AppError::Conflict("a subnet with this cidr already exists".to_string());
            }
            Some("subnet_parent_id_fkey") => {
                return AppError::BadRequest(
                    "parent_id does not refer to an existing subnet".to_string(),
                );
            }
            _ => {}
        }
    }

    AppError::Database(err)
}
