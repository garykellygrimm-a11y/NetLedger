use std::net::IpAddr;

use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{error::AppError, sessions::CurrentUser};

pub async fn record(
    conn: &mut PgConnection,
    actor: &CurrentUser,
    source: Option<IpAddr>,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    details: Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO audit_log
            (actor_id, actor_username, credential_id, source_addr, action, outcome, target_type, target_id, details)
        VALUES ($1, $2, $3, $4, $5, 'success', $6, $7, $8)
        "#,
        actor.account_id,
        actor.username,
        actor.credential.id(),
        source.map(ipnet::IpNet::from),
        action,
        target_type,
        target_id,
        details
    )
    .execute(conn)
    .await?;

    Ok(())
}
