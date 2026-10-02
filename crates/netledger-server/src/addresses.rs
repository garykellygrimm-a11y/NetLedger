use std::net::IpAddr;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    error::AppError,
    extractors::{AppJson, AppPath},
    subnets::PLACEMENT_LOCK_KEY,
};

const HOSTNAME_MAX_CHARS: usize = 253;
const DESCRIPTION_MAX_CHARS: usize = 1000;

#[derive(Serialize)]
pub struct Address {
    pub id: Uuid,
    pub address: IpAddr,
    pub subnet_id: Uuid,
    pub hostname: String,
    pub description: String,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateAddress {
    pub address: IpAddr,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub description: String,
}

impl CreateAddress {
    fn validate(&self) -> Result<(), AppError> {
        if self.hostname.trim().chars().count() > HOSTNAME_MAX_CHARS {
            return Err(AppError::BadRequest(format!(
                "hostname must be at most {HOSTNAME_MAX_CHARS} characters"
            )));
        }
        if self.description.chars().count() > DESCRIPTION_MAX_CHARS {
            return Err(AppError::BadRequest(format!(
                "description must be at most {DESCRIPTION_MAX_CHARS} characters"
            )));
        }
        Ok(())
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/addresses", post(create_address))
        .route("/addresses/{id}", get(get_address).delete(delete_address))
        .route("/subnets/{id}/addresses", get(list_subnet_addresses))
}

fn is_reserved_in(subnet: IpNet, address: IpAddr) -> bool {
    match subnet {
        IpNet::V4(net) if net.prefix_len() < 31 => {
            address == IpAddr::V4(net.network()) || address == IpAddr::V4(net.broadcast())
        }
        _ => false,
    }
}

async fn create_address(
    State(state): State<AppState>,
    AppJson(input): AppJson<CreateAddress>,
) -> Result<(StatusCode, Json<Address>), AppError> {
    input.validate()?;
    let host = IpNet::from(input.address);

    let mut tx = state.db.begin().await?;

    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(PLACEMENT_LOCK_KEY)
        .execute(&mut *tx)
        .await?;

    let subnet = sqlx::query!(
        r#"
        SELECT id, cidr AS "cidr: IpNet"
        FROM subnet
        WHERE cidr >>= $1
        ORDER BY masklen(cidr) DESC
        LIMIT 1
        "#,
        host,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(format!(
            "address {} is not inside any subnet; create its subnet first",
            input.address
        ))
    })?;

    if is_reserved_in(subnet.cidr, input.address) {
        return Err(AppError::BadRequest(format!(
            "address {} is the network or broadcast address of subnet {}",
            input.address, subnet.cidr
        )));
    }

    let address = sqlx::query_as!(
        Address,
        r#"
        INSERT INTO address (address, subnet_id, hostname, description, source)
        VALUES ($1, $2, $3, $4, 'manual')
        RETURNING id, address AS "address: IpAddr", subnet_id, hostname, description, source,
                  created_at, updated_at
        "#,
        host,
        subnet.id,
        input.hostname.trim(),
        input.description,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(map_address_error)?;

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(address)))
}

async fn get_address(
    State(state): State<AppState>,
    AppPath(id): AppPath<Uuid>,
) -> Result<Json<Address>, AppError> {
    let address = sqlx::query_as!(
        Address,
        r#"
        SELECT id, address AS "address: IpAddr", subnet_id, hostname, description, source,
               created_at, updated_at
        FROM address
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(address))
}

async fn list_subnet_addresses(
    State(state): State<AppState>,
    AppPath(subnet_id): AppPath<Uuid>,
) -> Result<Json<Vec<Address>>, AppError> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM subnet WHERE id = $1) AS "exists!""#,
        subnet_id
    )
    .fetch_one(&state.db)
    .await?;

    if !exists {
        return Err(AppError::NotFound);
    }

    let addresses = sqlx::query_as!(
        Address,
        r#"
        SELECT id, address AS "address: IpAddr", subnet_id, hostname, description, source,
               created_at, updated_at
        FROM address
        WHERE subnet_id = $1
        ORDER BY address
        "#,
        subnet_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(addresses))
}

async fn delete_address(
    State(state): State<AppState>,
    AppPath(id): AppPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query!("DELETE FROM address WHERE id = $1", id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

fn map_address_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.constraint() == Some("address_address_key")
    {
        return AppError::Conflict("this address is already recorded".to_string());
    }

    AppError::Database(err)
}

#[cfg(test)]
mod tests {
    use axum::{Router, http::StatusCode};
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::test_support::send;

    async fn create_subnet(app: &Router, cidr: &str) -> Value {
        let (status, body) = send(
            app,
            "POST",
            "/api/subnets",
            Some(json!({ "cidr": cidr, "name": cidr })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        body
    }

    async fn create_address(app: &Router, body: Value) -> (StatusCode, Value) {
        send(app, "POST", "/api/addresses", Some(body)).await
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn an_address_is_placed_in_the_most_specific_subnet(pool: PgPool) {
        let app = crate::app(pool);
        create_subnet(&app, "10.0.0.0/16").await;
        let servers = create_subnet(&app, "10.0.1.0/24").await;

        let (status, body) = create_address(
            &app,
            json!({ "address": "10.0.1.5", "hostname": "  web01  " }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["address"], "10.0.1.5");
        assert_eq!(body["subnet_id"], servers["id"]);
        assert_eq!(body["hostname"], "web01");
        assert_eq!(body["source"], "manual");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn an_address_outside_every_subnet_returns_400(pool: PgPool) {
        let app = crate::app(pool);
        create_subnet(&app, "10.0.0.0/16").await;

        let (status, body) = create_address(&app, json!({ "address": "192.168.1.10" })).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body["error"],
            "address 192.168.1.10 is not inside any subnet; create its subnet first"
        );
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn network_and_broadcast_addresses_return_400(pool: PgPool) {
        let app = crate::app(pool);
        create_subnet(&app, "10.0.1.0/24").await;

        let (network, _) = create_address(&app, json!({ "address": "10.0.1.0" })).await;
        let (broadcast, _) = create_address(&app, json!({ "address": "10.0.1.255" })).await;

        assert_eq!(network, StatusCode::BAD_REQUEST);
        assert_eq!(broadcast, StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_duplicate_address_returns_409(pool: PgPool) {
        let app = crate::app(pool);
        create_subnet(&app, "10.0.1.0/24").await;
        create_address(&app, json!({ "address": "10.0.1.5" })).await;

        let (status, body) = create_address(&app, json!({ "address": "10.0.1.5" })).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "this address is already recorded");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn ipv6_addresses_are_supported(pool: PgPool) {
        let app = crate::app(pool);
        let subnet = create_subnet(&app, "fd00:10::/64").await;

        let (status, body) = create_address(&app, json!({ "address": "fd00:10::5" })).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["address"], "fd00:10::5");
        assert_eq!(body["subnet_id"], subnet["id"]);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_subnets_addresses_are_listed_in_numeric_order(pool: PgPool) {
        let app = crate::app(pool);
        let subnet = create_subnet(&app, "10.0.1.0/24").await;
        for address in ["10.0.1.20", "10.0.1.3", "10.0.1.100"] {
            create_address(&app, json!({ "address": address })).await;
        }

        let uri = format!("/api/subnets/{}/addresses", subnet["id"].as_str().unwrap());
        let (status, body) = send(&app, "GET", &uri, None).await;

        assert_eq!(status, StatusCode::OK);
        let addresses: Vec<&str> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|address| address["address"].as_str().unwrap())
            .collect();
        assert_eq!(addresses, ["10.0.1.3", "10.0.1.20", "10.0.1.100"]);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn listing_addresses_of_an_unknown_subnet_returns_404(pool: PgPool) {
        let app = crate::app(pool);

        let (status, _) = send(
            &app,
            "GET",
            "/api/subnets/00000000-0000-0000-0000-000000000000/addresses",
            None,
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn deleting_an_address_removes_it(pool: PgPool) {
        let app = crate::app(pool);
        create_subnet(&app, "10.0.1.0/24").await;
        let (_, address) = create_address(&app, json!({ "address": "10.0.1.5" })).await;
        let uri = format!("/api/addresses/{}", address["id"].as_str().unwrap());

        let (status, _) = send(&app, "DELETE", &uri, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, _) = send(&app, "GET", &uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn database_rejects_an_address_outside_its_subnet_directly(pool: PgPool) {
        let subnet_id: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO subnet (cidr, name) VALUES ('10.0.1.0/24', 'Servers') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let err = sqlx::query(
            "INSERT INTO address (address, subnet_id, source) VALUES ('192.168.1.5', $1, 'manual')",
        )
        .bind(subnet_id)
        .execute(&pool)
        .await
        .unwrap_err();

        assert_eq!(
            err.as_database_error().unwrap().constraint(),
            Some("address_within_subnet")
        );
    }
}
