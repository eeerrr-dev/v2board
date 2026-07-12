use v2board_analytics::{clickhouse_client, migrate_clickhouse};

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
    migrate_clickhouse(&client, chrono::Utc::now().timestamp()).await?;
    println!("ClickHouse analytics migrations applied");
    Ok(())
}

fn required(name: &'static str) -> anyhow::Result<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} must be explicitly configured"))
}
