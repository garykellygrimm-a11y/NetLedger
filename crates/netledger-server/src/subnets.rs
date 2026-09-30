use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use chrono::{DateTime, Utc};
use ipnet::IpNet;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    AppState,
    error::AppError,
    extractors::{AppJson, AppPath},
};

const DEADLOCK_DETECTED: &str = "40P01";
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

        validate_name(&self.name)?;
        validate_description(&self.description)?;
        validate_vlan_id(self.vlan_id)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateSubnet {
    #[serde(default, deserialize_with = "present")]
    pub name: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    pub vlan_id: Option<Option<i32>>,
}

impl UpdateSubnet {
    fn validate(&self) -> Result<(), AppError> {
        if self.name.is_none() && self.description.is_none() && self.vlan_id.is_none() {
            return Err(AppError::BadRequest(
                "request must change at least one field".to_string(),
            ));
        }

        match &self.name {
            Some(None) => {
                return Err(AppError::BadRequest("name must not be null".to_string()));
            }
            Some(Some(name)) => validate_name(name)?,
            None => {}
        }

        match &self.description {
            Some(None) => {
                return Err(AppError::BadRequest(
                    "description must not be null; send an empty string to clear it".to_string(),
                ));
            }
            Some(Some(description)) => validate_description(description)?,
            None => {}
        }

        if let Some(vlan_id) = self.vlan_id {
            validate_vlan_id(vlan_id)?;
        }

        Ok(())
    }
}

fn present<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn validate_name(name: &str) -> Result<(), AppError> {
    let name_len = name.trim().chars().count();
    if name_len == 0 {
        return Err(AppError::BadRequest("name must not be empty".to_string()));
    }
    if name_len > NAME_MAX_CHARS {
        return Err(AppError::BadRequest(format!(
            "name must be at most {NAME_MAX_CHARS} characters"
        )));
    }
    Ok(())
}

fn validate_description(description: &str) -> Result<(), AppError> {
    if description.chars().count() > DESCRIPTION_MAX_CHARS {
        return Err(AppError::BadRequest(format!(
            "description must be at most {DESCRIPTION_MAX_CHARS} characters"
        )));
    }
    Ok(())
}

fn validate_vlan_id(vlan_id: Option<i32>) -> Result<(), AppError> {
    if vlan_id.is_some_and(|vlan| !(1..=4094).contains(&vlan)) {
        return Err(AppError::BadRequest(
            "vlan_id must be between 1 and 4094".to_string(),
        ));
    }
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subnets", get(list_subnets).post(create_subnet))
        .route(
            "/subnets/{id}",
            get(get_subnet).patch(update_subnet).delete(delete_subnet),
        )
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
    check_placement(&state.db, &input).await?;

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

async fn check_placement(db: &PgPool, input: &CreateSubnet) -> Result<(), AppError> {
    if let Some(parent_id) = input.parent_id {
        let parent = sqlx::query!(
            r#"
            SELECT cidr AS "cidr: IpNet", name, $2 << cidr AS "contains_child!"
            FROM subnet
            WHERE id = $1
            "#,
            parent_id,
            input.cidr,
        )
        .fetch_optional(db)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest("parent_id does not refer to an existing subnet".to_string())
        })?;

        if !parent.contains_child {
            return Err(AppError::BadRequest(format!(
                "cidr {} is not inside the parent subnet {} ({})",
                input.cidr, parent.cidr, parent.name
            )));
        }
    }

    let overlap = sqlx::query!(
        r#"
        SELECT cidr AS "cidr: IpNet", name, cidr >> $1 AS "contains_new!"
        FROM subnet
        WHERE parent_id IS NOT DISTINCT FROM $2 AND cidr && $1
        LIMIT 1
        "#,
        input.cidr,
        input.parent_id,
    )
    .fetch_optional(db)
    .await?;

    if let Some(existing) = overlap {
        let message = if existing.cidr == input.cidr {
            "a subnet with this cidr already exists".to_string()
        } else if existing.contains_new {
            format!(
                "cidr {} is inside the existing subnet {} ({}); choose it as the parent",
                input.cidr, existing.cidr, existing.name
            )
        } else {
            format!(
                "cidr {} contains the existing subnet {} ({}) at the same level; create larger subnets before the subnets inside them",
                input.cidr, existing.cidr, existing.name
            )
        };
        return Err(AppError::Conflict(message));
    }

    Ok(())
}

async fn update_subnet(
    State(state): State<AppState>,
    AppPath(id): AppPath<Uuid>,
    AppJson(input): AppJson<UpdateSubnet>,
) -> Result<Json<Subnet>, AppError> {
    input.validate()?;

    let name = input.name.flatten().map(|name| name.trim().to_string());
    let description = input.description.flatten();
    let set_vlan_id = input.vlan_id.is_some();
    let vlan_id = input.vlan_id.flatten();

    let subnet = sqlx::query_as!(
        Subnet,
        r#"
        UPDATE subnet
        SET name = COALESCE($2, name),
            description = COALESCE($3, description),
            vlan_id = CASE WHEN $4::boolean THEN $5::integer ELSE vlan_id END,
            updated_at = now()
        WHERE id = $1
        RETURNING id, cidr AS "cidr: IpNet", name, description, vlan_id, parent_id, created_at, updated_at
        "#,
        id,
        name,
        description,
        set_vlan_id,
        vlan_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(subnet))
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
        if db_err.code().as_deref() == Some(DEADLOCK_DETECTED) {
            return AppError::Conflict(
                "cidr conflicts with another subnet being created at the same time; retry the request"
                    .to_string(),
            );
        }

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
    use uuid::Uuid;

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
            "cidr 192.168.50.0/24 is not inside the parent subnet 10.0.0.0/16 (Parent)"
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
            "cidr 10.0.5.0/24 is inside the existing subnet 10.0.0.0/16 (Datacenter); choose it as the parent"
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

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn larger_subnet_after_a_smaller_one_returns_409(pool: PgPool) {
        let app = crate::app(pool);
        create(&app, json!({ "cidr": "10.0.0.0/16", "name": "Datacenter" })).await;

        let (status, body) =
            create(&app, json!({ "cidr": "10.0.0.0/8", "name": "Supernet" })).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("contains the existing subnet 10.0.0.0/16 (Datacenter)")
        );
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn database_rejects_a_child_outside_its_parent_directly(pool: PgPool) {
        let parent_id: Uuid = sqlx::query_scalar(
            "INSERT INTO subnet (cidr, name) VALUES ('10.0.0.0/16', 'Parent') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let err = sqlx::query(
            "INSERT INTO subnet (cidr, name, parent_id) VALUES ('192.168.50.0/24', 'Outside', $1)",
        )
        .bind(parent_id)
        .execute(&pool)
        .await
        .unwrap_err();

        let db_err = err.as_database_error().unwrap();
        assert_eq!(db_err.constraint(), Some("subnet_within_parent"));
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn concurrent_overlapping_creates_admit_exactly_one(pool: PgPool) {
        let app = crate::app(pool);

        for round in 0..20 {
            let larger = format!("10.{round}.0.0/16");
            let smaller = format!("10.{round}.0.0/17");

            let (first, second) = tokio::join!(
                create(&app, json!({ "cidr": larger, "name": "Larger" })),
                create(&app, json!({ "cidr": smaller, "name": "Smaller" })),
            );

            let mut statuses = [first.0.as_u16(), second.0.as_u16()];
            statuses.sort();
            assert_eq!(statuses, [201, 409], "round {round}");
        }
    }

    async fn patch(app: &Router, id: &str, body: Value) -> (StatusCode, Value) {
        send(app, "PATCH", &format!("/api/subnets/{id}"), Some(body)).await
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn patch_updates_only_the_fields_sent(pool: PgPool) {
        let app = crate::app(pool);
        let (_, created) = create(
            &app,
            json!({
                "cidr": "10.8.0.0/24",
                "name": "Old name",
                "description": "Keep me",
                "vlan_id": 100
            }),
        )
        .await;

        let (status, body) = patch(
            &app,
            created["id"].as_str().unwrap(),
            json!({ "name": "  New name  " }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "New name");
        assert_eq!(body["description"], "Keep me");
        assert_eq!(body["vlan_id"], 100);
        assert_eq!(body["cidr"], "10.8.0.0/24");
        assert_ne!(body["updated_at"], created["updated_at"]);
        assert_eq!(body["created_at"], created["created_at"]);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn patch_with_null_vlan_id_clears_it(pool: PgPool) {
        let app = crate::app(pool);
        let (_, created) = create(
            &app,
            json!({ "cidr": "10.8.0.0/24", "name": "Voice", "vlan_id": 100 }),
        )
        .await;

        let (status, body) = patch(
            &app,
            created["id"].as_str().unwrap(),
            json!({ "vlan_id": null }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["vlan_id"], Value::Null);
        assert_eq!(body["name"], "Voice");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn patch_with_no_fields_returns_400(pool: PgPool) {
        let app = crate::app(pool);
        let (_, created) = create(&app, json!({ "cidr": "10.8.0.0/24", "name": "Voice" })).await;

        let (status, body) = patch(&app, created["id"].as_str().unwrap(), json!({})).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "request must change at least one field");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn patch_rejects_invalid_values(pool: PgPool) {
        let app = crate::app(pool);
        let (_, created) = create(&app, json!({ "cidr": "10.8.0.0/24", "name": "Voice" })).await;
        let id = created["id"].as_str().unwrap();

        let (status, body) = patch(&app, id, json!({ "name": "   " })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "name must not be empty");

        let (status, body) = patch(&app, id, json!({ "name": null })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "name must not be null");

        let (status, _) = patch(&app, id, json!({ "vlan_id": 5000 })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn patch_unknown_subnet_returns_404(pool: PgPool) {
        let app = crate::app(pool);

        let (status, body) = patch(
            &app,
            "00000000-0000-0000-0000-000000000000",
            json!({ "name": "Nobody" }),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not found");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn patch_cannot_change_cidr_yet(pool: PgPool) {
        let app = crate::app(pool);
        let (_, created) = create(&app, json!({ "cidr": "10.8.0.0/24", "name": "Voice" })).await;

        let (status, _) = patch(
            &app,
            created["id"].as_str().unwrap(),
            json!({ "cidr": "10.9.0.0/24" }),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
