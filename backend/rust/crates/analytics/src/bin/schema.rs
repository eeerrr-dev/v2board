use uuid::Uuid;
use v2board_analytics::{
    bind_clickhouse_installation, clickhouse_client, configure_clickhouse_retention,
    migrate_clickhouse,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let environment_name = required("V2BOARD_ENV")?;
    let environment = v2board_config::RuntimeEnvironment::parse(Some(&environment_name))
        .map_err(anyhow::Error::msg)?;
    let url = required("V2BOARD_CLICKHOUSE_SCHEMA_URL")?;
    let database = required("V2BOARD_CLICKHOUSE_SCHEMA_DATABASE")?;
    let username = required("V2BOARD_CLICKHOUSE_SCHEMA_USERNAME")?;
    let password = v2board_config::one_shot_secret(
        "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD",
        "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD_FILE",
        "v2board-clickhouse-schema-password",
    )?;
    v2board_config::validate_clickhouse_schema_connection(
        environment,
        &url,
        &database,
        &username,
        password.as_deref(),
    )?;
    let client = clickhouse_client(&url, &database, &username, password.as_deref());
    let now_unix = chrono::Utc::now().timestamp();
    migrate_clickhouse(&client, now_unix).await?;
    if let Some(postgres_url) = optional("V2BOARD_CLICKHOUSE_BIND_POSTGRES_URL") {
        if environment != v2board_config::RuntimeEnvironment::Local {
            anyhow::bail!(
                "V2BOARD_CLICKHOUSE_BIND_POSTGRES_URL is a local Docker bootstrap convenience only"
            );
        }
        let postgres = sqlx::PgPool::connect(&postgres_url).await?;
        let installations = sqlx::query_scalar::<_, Uuid>(
            "SELECT installation_id FROM v2_system_installation WHERE state = 'active'",
        )
        .fetch_all(&postgres)
        .await?;
        let [installation_id] = installations.as_slice() else {
            anyhow::bail!("local PostgreSQL must contain exactly one active installation")
        };
        let raw_retention_days = required_u32("V2BOARD_CLICKHOUSE_RAW_RETENTION_DAYS")?;
        let aggregate_retention_days = required_u32("V2BOARD_CLICKHOUSE_AGGREGATE_RETENTION_DAYS")?;
        bind_clickhouse_installation(&client, *installation_id, now_unix).await?;
        configure_clickhouse_retention(
            &client,
            *installation_id,
            raw_retention_days,
            aggregate_retention_days,
            now_unix,
        )
        .await?;
    }
    println!("ClickHouse analytics migrations applied");
    Ok(())
}

fn optional(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn required_u32(name: &'static str) -> anyhow::Result<u32> {
    required(name)?
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("{name} must be an unsigned integer"))
}

fn required(name: &'static str) -> anyhow::Result<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} must be explicitly configured"))
}
