use anyhow::{Context, Result, anyhow, bail};
use sqlx::PgPool;
use uuid::Uuid;

use crate::passwords::{self, HashAlgorithm};

const USERNAME_MAX_CHARS: usize = 64;

pub fn normalize_username(input: &str) -> Result<String, String> {
    let username = input.trim().to_lowercase();

    if username.is_empty() || username.chars().count() > USERNAME_MAX_CHARS {
        return Err(format!(
            "username must be 1 to {USERNAME_MAX_CHARS} characters"
        ));
    }

    let allowed = username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    let starts_well = username.starts_with(|c: char| c.is_ascii_alphanumeric());
    if !allowed || !starts_well {
        return Err("username may contain only letters, digits, '.', '_', and '-', and must start with a letter or digit".to_string());
    }

    Ok(username)
}

pub async fn create_admin_interactively(
    db: &PgPool,
    algorithm: HashAlgorithm,
    username: &str,
) -> Result<()> {
    let username = normalize_username(username).map_err(|message| anyhow!(message))?;

    let password = rpassword::prompt_password(format!("Password for {username}: "))
        .context("failed to read the password")?;
    let confirmation =
        rpassword::prompt_password("Confirm password: ").context("failed to read the password")?;
    if password != confirmation {
        bail!("the passwords do not match");
    }

    let id = create_administrator(db, algorithm, &username, &password).await?;
    println!("Created administrator {username} ({id}).");

    Ok(())
}

pub async fn create_administrator(
    db: &PgPool,
    algorithm: HashAlgorithm,
    username: &str,
    password: &str,
) -> Result<Uuid> {
    let username = normalize_username(username).map_err(|message| anyhow!(message))?;
    passwords::check_new_password(password, &username).map_err(|message| anyhow!(message))?;
    let secret_hash = passwords::hash_password(password, algorithm)?;

    let mut tx = db.begin().await?;

    let account_id = sqlx::query_scalar!(
        "INSERT INTO account (username, role) VALUES ($1, 'administrator') RETURNING id",
        username
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|err| {
        if let sqlx::Error::Database(db_err) = &err
            && db_err.constraint() == Some("account_username_key")
        {
            return anyhow!("an account named {username} already exists");
        }
        anyhow::Error::new(err).context("failed to create the account")
    })?;

    sqlx::query!(
        "INSERT INTO identity (account_id, kind, secret_hash) VALUES ($1, 'password', $2)",
        account_id,
        secret_hash
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO audit_log (actor_username, action, target_type, target_id, details)
        VALUES (
            'setup',
            'account.create',
            'account',
            $1,
            jsonb_build_object('username', $2::text, 'role', 'administrator', 'via', 'setup command')
        )
        "#,
        account_id,
        username
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(account_id)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    const PASSWORD: &str = "correct horse battery staple";

    #[test]
    fn usernames_are_normalized_and_validated() {
        assert_eq!(normalize_username("  Gary.K  ").unwrap(), "gary.k");
        assert!(normalize_username("").is_err());
        assert!(normalize_username("-gary").is_err());
        assert!(normalize_username("gary kelly").is_err());
        assert!(normalize_username(&"a".repeat(65)).is_err());
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn creates_an_administrator_with_a_password_and_an_audit_entry(pool: PgPool) {
        let id = create_administrator(&pool, HashAlgorithm::Argon2id, "Gary", PASSWORD)
            .await
            .unwrap();

        let account = sqlx::query!("SELECT username, role FROM account WHERE id = $1", id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(account.username, "gary");
        assert_eq!(account.role, "administrator");

        let secret_hash = sqlx::query_scalar!(
            "SELECT secret_hash FROM identity WHERE account_id = $1 AND kind = 'password'",
            id
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap();
        assert!(passwords::verify_password(PASSWORD, &secret_hash));

        let audit_entries = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM audit_log WHERE target_id = $1 AND action = 'account.create'"#,
            id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audit_entries, 1);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn usernames_are_unique_regardless_of_case(pool: PgPool) {
        create_administrator(&pool, HashAlgorithm::Argon2id, "gary", PASSWORD)
            .await
            .unwrap();

        let err = create_administrator(&pool, HashAlgorithm::Argon2id, "GARY", PASSWORD)
            .await
            .unwrap_err();

        assert_eq!(err.to_string(), "an account named gary already exists");
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn weak_passwords_are_rejected_before_anything_is_written(pool: PgPool) {
        let err = create_administrator(&pool, HashAlgorithm::Argon2id, "gary", "short")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("at least 15 characters"));

        let accounts = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM account"#)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(accounts, 0);
    }

    #[sqlx::test(migrator = "crate::MIGRATOR")]
    async fn the_audit_log_cannot_be_changed(pool: PgPool) {
        create_administrator(&pool, HashAlgorithm::Argon2id, "gary", PASSWORD)
            .await
            .unwrap();

        let update = sqlx::query("UPDATE audit_log SET action = 'tampered'")
            .execute(&pool)
            .await;
        let delete = sqlx::query("DELETE FROM audit_log").execute(&pool).await;

        assert!(update.is_err());
        assert!(delete.is_err());
    }
}
