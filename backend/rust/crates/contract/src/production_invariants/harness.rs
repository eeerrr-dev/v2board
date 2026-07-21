use std::env;

use anyhow::{Context, Result, ensure};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, PgPool};
use url::Url;
use uuid::Uuid;
use v2board_config::{AppConfig, RuntimeEnvironment};

pub(crate) static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

pub(crate) const DEFAULT_ROOT_DATABASE_URL: &str =
    "postgresql://v2board:v2board@postgres:5432/postgres";

pub(crate) const DEFAULT_INTEGRATION_REDIS_URL: &str = "redis://redis:6379/15";

pub(super) const INTEGRATION_APP_KEY: &str =
    "integration-only-app-key-with-at-least-thirty-two-bytes";

pub(crate) fn integration_config(_pool: &PgPool, redis_url: &str) -> Result<AppConfig> {
    let mut config = AppConfig::try_from_api_env().context("load integration AppConfig")?;
    config.environment = RuntimeEnvironment::Testing;
    config.redis_url = redis_url.to_string();
    config.app_key = INTEGRATION_APP_KEY.to_string();
    config.stop_register = false;
    config.email_verify = false;
    config.recaptcha_enable = false;
    config.email_whitelist_enable = false;
    config.email_gmail_limit_enable = false;
    config.try_out_plan_id = 0;
    Ok(config)
}

pub(super) fn operator_authority_config() -> Result<AppConfig> {
    AppConfig::try_from_api_env().context("load contract AppConfig")
}

/// Payment fixtures must store the same at-rest AES-256-GCM envelope the
/// runtime writes; a plaintext `payment_method.config` row is an integrity
/// error to every reader.
pub(crate) fn encrypt_payment_fixture_config(
    payment: &str,
    uuid: &str,
    config: &serde_json::Value,
) -> Result<serde_json::Value> {
    let object = config
        .as_object()
        .context("payment fixture config must be a JSON object")?;
    v2board_payment_adapters::payment_secrets::encrypt_payment_config(
        INTEGRATION_APP_KEY,
        payment,
        uuid,
        object,
    )
    .context("encrypt payment fixture config")
}

pub(super) async fn insert_user(pool: &PgPool, label: &str, password: &str) -> Result<i64> {
    let email = format!("{label}-{}@example.test", Uuid::new_v4().simple());
    insert_user_with_email(pool, &email, password).await
}

pub(super) async fn insert_user_with_email(
    pool: &PgPool,
    email: &str,
    password: &str,
) -> Result<i64> {
    let now = Utc::now().timestamp();
    sqlx::query_scalar(
        "INSERT INTO users (email, password, uuid, token, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(email)
    .bind(password)
    .bind(Uuid::new_v4().hyphenated().to_string())
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub(crate) async fn flush_redis(redis: &redis::Client) -> Result<()> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    redis::cmd("FLUSHDB").query_async::<()>(&mut conn).await?;
    Ok(())
}

pub(crate) async fn create_database(
    root: &PgPool,
    database_name: &GeneratedDatabaseName,
) -> Result<()> {
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE {} WITH TEMPLATE template0 ENCODING 'UTF8'",
        database_name.quoted()
    )))
    .execute(root)
    .await?;
    Ok(())
}

pub(crate) async fn drop_database(
    root: &PgPool,
    database_name: &GeneratedDatabaseName,
) -> Result<()> {
    // A failed invariant may leave pooled or child-process sessions behind.
    // Terminate them by a bound value before issuing the necessarily dynamic
    // DROP DATABASE against the validated generated identifier.
    let _: Vec<bool> = sqlx::query_scalar(
        r#"
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = $1 AND pid <> pg_backend_pid()
        "#,
    )
    .bind(database_name.as_str())
    .fetch_all(root)
    .await?;
    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {} WITH (FORCE)",
        database_name.quoted()
    )))
    .execute(root)
    .await?;
    Ok(())
}

pub(crate) fn database_url_for(
    root_database_url: &str,
    database_name: &GeneratedDatabaseName,
) -> Result<String> {
    let mut url = Url::parse(root_database_url).context("parse integration root database URL")?;
    url.set_path(&format!("/{}", database_name.as_str()));
    Ok(url.to_string())
}

#[derive(Debug)]
pub(crate) struct GeneratedDatabaseName(String);

impl GeneratedDatabaseName {
    pub(crate) fn new(label: &str) -> Result<Self> {
        ensure!(
            !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()),
            "unsafe generated database label"
        );
        let value = format!(
            "v2board_{label}_{}",
            &Uuid::new_v4().simple().to_string()[..16]
        );
        validate_generated_database_name(&value)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn quoted(&self) -> String {
        // Validation excludes quotes and every other escaping case.
        format!("\"{}\"", self.0)
    }
}

pub(super) fn validate_generated_database_name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 63
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "unsafe generated SQL identifier"
    );
    Ok(())
}

pub(super) fn sha256_bytes(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

pub(super) fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn random_traffic_key() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(crate) fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
