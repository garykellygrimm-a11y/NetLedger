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
        .route("/subnets/{id}", get(get_subnet).delete(delete_subnet))
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

async fn delete_subnet(
    State(state): State<AppState>,
    AppPath(id): AppPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query!("DELETE FROM subnet WHERE id = $1", id)
        .execute(&state.db)
        .await
        .map_err(map_delete_error)?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
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
            Some("subnet_within_parent") => {
                return AppError::BadRequest(
                    "cidr must be inside the parent subnet's network".to_string(),
                );
            }
            Some("subnet_no_overlapping_siblings") => {
                return AppError::Conflict(
                    "cidr overlaps another subnet at the same level".to_string(),
                );
            }
            _ => {}
        }
    }

    AppError::Database(err)
}

fn map_delete_error(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.constraint() == Some("subnet_parent_id_fkey")
    {
        return AppError::Conflict("subnet has child subnets; delete them first".to_string());
    }

    AppError::Database(err)
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

    async fn send(
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

    async fn create(app: &Router, body: Value) -> (StatusCode, Value) {
        send(app, "POST", "/api/subnets", Some(body)).await
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn create_returns_201_and_the_new_subnet(pool: PgPool) {
        let app = crate::app(pool);

        let (status, body) = create(
            &app,
            json!({ "cidr": "10.2.0.0/24", "name": "  Management  ", "vlan_id": 200 }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["cidr"], "10.2.0.0/24");
        assert_eq!(body["name"], "Management");
        assert_eq!(body["vlan_id"], 200);
        assert_eq!(body["description"], "");
        assert!(body["id"].is_string());
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn duplicate_cidr_returns_409(pool: PgPool) {
        let app = crate::app(pool);
        let subnet = json!({ "cidr": "10.2.0.0/24", "name": "First" });

        create(&app, subnet.clone()).await;
        let (status, body) = create(&app, subnet).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "a subnet with this cidr already exists");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn host_bits_return_400_with_the_network_address(pool: PgPool) {
        let app = crate::app(pool);

        let (status, body) = create(&app, json!({ "cidr": "10.2.0.5/24", "name": "Bad" })).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("10.2.0.0/24"));
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn out_of_range_vlan_returns_400(pool: PgPool) {
        let app = crate::app(pool);

        let (status, _) = create(
            &app,
            json!({ "cidr": "10.3.0.0/24", "name": "Bad VLAN", "vlan_id": 5000 }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn blank_name_returns_400(pool: PgPool) {
        let app = crate::app(pool);

        let (status, body) = create(&app, json!({ "cidr": "10.5.0.0/24", "name": "   " })).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "name must not be empty");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn unknown_field_returns_422(pool: PgPool) {
        let app = crate::app(pool);

        let (status, body) = create(
            &app,
            json!({ "cidr": "10.4.0.0/24", "name": "Typo", "vlan": 200 }),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["error"].as_str().unwrap().contains("vlan"));
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn nonexistent_parent_returns_400(pool: PgPool) {
        let app = crate::app(pool);

        let (status, _) = create(
            &app,
            json!({
                "cidr": "10.6.0.0/24",
                "name": "Orphan",
                "parent_id": "00000000-0000-0000-0000-000000000000"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn list_is_ordered_by_network_address(pool: PgPool) {
        let app = crate::app(pool);
        for cidr in ["192.168.1.0/24", "10.0.0.0/8", "9.0.0.0/8"] {
            create(&app, json!({ "cidr": cidr, "name": cidr })).await;
        }

        let (status, body) = send(&app, "GET", "/api/subnets", None).await;

        assert_eq!(status, StatusCode::OK);
        let cidrs: Vec<&str> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|subnet| subnet["cidr"].as_str().unwrap())
            .collect();
        assert_eq!(cidrs, ["9.0.0.0/8", "10.0.0.0/8", "192.168.1.0/24"]);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn get_unknown_subnet_returns_404(pool: PgPool) {
        let app = crate::app(pool);

        let (status, body) = send(
            &app,
            "GET",
            "/api/subnets/00000000-0000-0000-0000-000000000000",
            None,
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not found");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn invalid_id_returns_400_as_json(pool: PgPool) {
        let app = crate::app(pool);

        let (status, body) = send(&app, "GET", "/api/subnets/not-a-uuid", None).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].is_string());
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn delete_returns_204_then_the_subnet_is_gone(pool: PgPool) {
        let app = crate::app(pool);
        let (_, created) = create(&app, json!({ "cidr": "10.7.0.0/24", "name": "Temp" })).await;
        let uri = format!("/api/subnets/{}", created["id"].as_str().unwrap());

        let (status, body) = send(&app, "DELETE", &uri, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null);

        let (status, _) = send(&app, "GET", &uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = send(&app, "DELETE", &uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn deleting_a_parent_with_children_returns_409(pool: PgPool) {
        let app = crate::app(pool);
        let (_, parent) = create(&app, json!({ "cidr": "10.0.0.0/16", "name": "Parent" })).await;
        let parent_id = parent["id"].as_str().unwrap();
        create(
            &app,
            json!({ "cidr": "10.0.1.0/24", "name": "Child", "parent_id": parent_id }),
        )
        .await;

        let (status, body) = send(&app, "DELETE", &format!("/api/subnets/{parent_id}"), None).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert!(body["error"].as_str().unwrap().contains("child subnets"));
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn child_outside_its_parent_returns_400(pool: PgPool) {
        let app = crate::app(pool);
        let (_, parent) = create(&app, json!({ "cidr": "10.0.0.0/16", "name": "Parent" })).await;

        let (status, body) = create(
            &app,
            json!({
                "cidr": "192.168.50.0/24",
                "name": "Wrong parent",
                "parent_id": parent["id"]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body["error"],
            "cidr must be inside the parent subnet's network"
        );
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn overlapping_top_level_subnets_return_409(pool: PgPool) {
        let app = crate::app(pool);
        create(&app, json!({ "cidr": "10.0.0.0/16", "name": "Datacenter" })).await;

        let (status, body) = create(&app, json!({ "cidr": "10.0.5.0/24", "name": "Inside" })).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body["error"],
            "cidr overlaps another subnet at the same level"
        );
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn overlapping_siblings_return_409(pool: PgPool) {
        let app = crate::app(pool);
        let (_, parent) = create(&app, json!({ "cidr": "10.0.0.0/16", "name": "Parent" })).await;
        create(
            &app,
            json!({ "cidr": "10.0.1.0/24", "name": "First", "parent_id": parent["id"] }),
        )
        .await;

        let (status, _) = create(
            &app,
            json!({ "cidr": "10.0.1.128/25", "name": "Second", "parent_id": parent["id"] }),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn a_valid_hierarchy_is_accepted(pool: PgPool) {
        let app = crate::app(pool);
        let (status, root) = create(&app, json!({ "cidr": "10.0.0.0/16", "name": "Root" })).await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, child) = create(
            &app,
            json!({ "cidr": "10.0.1.0/24", "name": "Child", "parent_id": root["id"] }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, _) = create(
            &app,
            json!({ "cidr": "10.0.2.0/24", "name": "Sibling", "parent_id": root["id"] }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, _) = create(
            &app,
            json!({ "cidr": "10.0.1.0/26", "name": "Grandchild", "parent_id": child["id"] }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn ipv4_and_ipv6_networks_never_overlap(pool: PgPool) {
        let app = crate::app(pool);

        let (v4, _) = create(&app, json!({ "cidr": "10.0.0.0/8", "name": "IPv4" })).await;
        let (v6, _) = create(&app, json!({ "cidr": "fd00::/8", "name": "IPv6" })).await;

        assert_eq!(v4, StatusCode::CREATED);
        assert_eq!(v6, StatusCode::CREATED);
    }
}
