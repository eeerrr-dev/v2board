use uuid::Uuid;
use v2board_analytics::{
    bind_clickhouse_installation, clickhouse_client, configure_clickhouse_retention,
    migrate_clickhouse, verify_clickhouse_bound_contract,
};
use v2board_provision::MysqlImportExecutionPlan;

const REQUIRED_CLICKHOUSE_MAJOR: u64 = 26;
const REQUIRED_CLICKHOUSE_MINOR: u64 = 3;

#[derive(clickhouse::Row, serde::Deserialize)]
pub(crate) struct ClickHouseCount {
    pub(crate) value: u64,
}

#[derive(clickhouse::Row, serde::Deserialize)]
struct ClickHouseString {
    value: String,
}

#[derive(clickhouse::Row, serde::Deserialize)]
struct ClickHouseGrantCheck {
    result: u8,
}

#[derive(Clone, Copy)]
enum ClickHousePrincipalKind {
    Schema,
    Writer,
}

const CLICKHOUSE_MANAGED_TABLES: &[&str] = &[
    "schema_migration",
    "installation_binding",
    "retention_binding",
    "traffic_reported",
    "traffic_accounted",
    "traffic_reported_daily",
    "traffic_accounted_daily",
];
const CLICKHOUSE_SCHEMA_INSERT_TABLES: &[&str] = &[
    "schema_migration",
    "installation_binding",
    "retention_binding",
];
const CLICKHOUSE_WRITER_INSERT_TABLES: &[&str] = &[
    "traffic_reported",
    "traffic_accounted",
    "traffic_reported_daily",
    "traffic_accounted_daily",
];
const CLICKHOUSE_RUNTIME_SYSTEM_TABLES: &[&str] = &["tables", "columns", "data_skipping_indices"];

pub(crate) async fn preflight_clickhouse_absent(
    plan: &MysqlImportExecutionPlan,
) -> anyhow::Result<()> {
    let clickhouse = &plan.clickhouse;
    let client = clickhouse_client(
        &clickhouse.endpoint,
        "default",
        &clickhouse.bootstrap_username,
        Some(&clickhouse.bootstrap_password),
    );
    let version = client
        .query("SELECT version() AS value")
        .fetch_one::<ClickHouseString>()
        .await?
        .value;
    if clickhouse_major_minor(&version)
        != Some((REQUIRED_CLICKHOUSE_MAJOR, REQUIRED_CLICKHOUSE_MINOR))
    {
        anyhow::bail!(
            "target ClickHouse must be {REQUIRED_CLICKHOUSE_MAJOR}.{REQUIRED_CLICKHOUSE_MINOR}.x, observed {version}"
        );
    }
    let database_count = client
        .query("SELECT count() AS value FROM system.databases WHERE name = ?")
        .bind(&clickhouse.database)
        .fetch_one::<ClickHouseCount>()
        .await?
        .value;
    if database_count != 0 {
        anyhow::bail!(
            "target ClickHouse database {} already exists",
            clickhouse.database
        );
    }
    for principal in [&clickhouse.schema_username, &clickhouse.writer_username] {
        let count = client
            .query("SELECT count() AS value FROM system.users WHERE name = ?")
            .bind(principal)
            .fetch_one::<ClickHouseCount>()
            .await?
            .value;
        if count != 0 {
            anyhow::bail!("target ClickHouse principal {principal} already exists");
        }
    }
    Ok(())
}

pub(crate) fn clickhouse_major_minor(version: &str) -> Option<(u64, u64)> {
    let mut components = version.split('.');
    Some((
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
    ))
}

pub(crate) async fn bootstrap_clickhouse(
    plan: &MysqlImportExecutionPlan,
    installation_id: Uuid,
    now: i64,
) -> anyhow::Result<()> {
    let clickhouse = &plan.clickhouse;
    let bootstrap = clickhouse_client(
        &clickhouse.endpoint,
        "default",
        &clickhouse.bootstrap_username,
        Some(&clickhouse.bootstrap_password),
    );
    bootstrap
        .query(&format!(
            "CREATE DATABASE {}",
            clickhouse_identifier(&clickhouse.database)
        ))
        .execute()
        .await?;
    for (username, password) in [
        (&clickhouse.schema_username, &clickhouse.schema_password),
        (&clickhouse.writer_username, &clickhouse.writer_password),
    ] {
        bootstrap
            .query(&format!(
                "CREATE USER {} IDENTIFIED WITH sha256_password BY {}",
                clickhouse_identifier(username),
                clickhouse_literal(password)?
            ))
            .execute()
            .await?;
    }
    let database = clickhouse_identifier(&clickhouse.database);
    let schema = clickhouse_identifier(&clickhouse.schema_username);
    let writer = clickhouse_identifier(&clickhouse.writer_username);
    for grant in [
        format!("GRANT CREATE TABLE ON {database}.* TO {schema}"),
        format!("GRANT ALTER TABLE ON {database}.* TO {schema}"),
    ] {
        bootstrap.query(&grant).execute().await?;
    }
    for table in CLICKHOUSE_MANAGED_TABLES {
        grant_clickhouse_table(&bootstrap, "SELECT", &database, table, &schema).await?;
        grant_clickhouse_table(&bootstrap, "SELECT", &database, table, &writer).await?;
    }
    for table in CLICKHOUSE_SCHEMA_INSERT_TABLES {
        grant_clickhouse_table(&bootstrap, "INSERT", &database, table, &schema).await?;
    }
    for table in CLICKHOUSE_WRITER_INSERT_TABLES {
        grant_clickhouse_table(&bootstrap, "INSERT", &database, table, &writer).await?;
    }
    for table in CLICKHOUSE_RUNTIME_SYSTEM_TABLES {
        grant_clickhouse_table(&bootstrap, "SELECT", "`system`", table, &schema).await?;
        grant_clickhouse_table(&bootstrap, "SELECT", "`system`", table, &writer).await?;
    }

    let schema_client = clickhouse_client(
        &clickhouse.endpoint,
        &clickhouse.database,
        &clickhouse.schema_username,
        Some(&clickhouse.schema_password),
    );
    verify_clickhouse_principal_acl(
        &bootstrap,
        &schema_client,
        &clickhouse.database,
        &clickhouse.schema_username,
        ClickHousePrincipalKind::Schema,
    )
    .await?;
    migrate_clickhouse(&schema_client, now).await?;
    bind_clickhouse_installation(&schema_client, installation_id, now).await?;
    configure_clickhouse_retention(
        &schema_client,
        installation_id,
        clickhouse.raw_retention_days,
        clickhouse.aggregate_retention_days,
        now,
    )
    .await?;
    verify_clickhouse_bound_contract(
        &schema_client,
        installation_id,
        clickhouse.raw_retention_days,
        clickhouse.aggregate_retention_days,
    )
    .await?;
    drop(schema_client);
    bootstrap
        .query(&format!(
            "DROP USER {}",
            clickhouse_identifier(&clickhouse.schema_username)
        ))
        .execute()
        .await?;
    let schema_principal_count = bootstrap
        .query("SELECT count() AS value FROM system.users WHERE name = ?")
        .bind(&clickhouse.schema_username)
        .fetch_one::<ClickHouseCount>()
        .await?
        .value;
    if schema_principal_count != 0 {
        anyhow::bail!("temporary ClickHouse schema principal was not retired");
    }

    let writer_client = clickhouse_client(
        &clickhouse.endpoint,
        &clickhouse.database,
        &clickhouse.writer_username,
        Some(&clickhouse.writer_password),
    );
    verify_clickhouse_principal_acl(
        &bootstrap,
        &writer_client,
        &clickhouse.database,
        &clickhouse.writer_username,
        ClickHousePrincipalKind::Writer,
    )
    .await?;
    verify_clickhouse_bound_contract(
        &writer_client,
        installation_id,
        clickhouse.raw_retention_days,
        clickhouse.aggregate_retention_days,
    )
    .await?;
    Ok(())
}

async fn verify_clickhouse_principal_acl(
    bootstrap: &clickhouse::Client,
    principal: &clickhouse::Client,
    database_name: &str,
    username: &str,
    kind: ClickHousePrincipalKind,
) -> anyhow::Result<()> {
    let database = clickhouse_identifier(database_name);
    let expected_schema = matches!(kind, ClickHousePrincipalKind::Schema);
    for privilege in ["CREATE TABLE", "ALTER TABLE"] {
        check_clickhouse_grant(
            principal,
            privilege,
            &format!("{database}.*"),
            expected_schema,
        )
        .await?;
    }
    check_clickhouse_grant(principal, "DROP TABLE", &format!("{database}.*"), false).await?;
    for table in CLICKHOUSE_MANAGED_TABLES {
        let object = format!("{database}.{}", clickhouse_identifier(table));
        check_clickhouse_grant(principal, "SELECT", &object, true).await?;
        let insert_expected = match kind {
            ClickHousePrincipalKind::Schema => CLICKHOUSE_SCHEMA_INSERT_TABLES.contains(table),
            ClickHousePrincipalKind::Writer => CLICKHOUSE_WRITER_INSERT_TABLES.contains(table),
        };
        check_clickhouse_grant(principal, "INSERT", &object, insert_expected).await?;
    }
    for table in CLICKHOUSE_RUNTIME_SYSTEM_TABLES {
        check_clickhouse_grant(
            principal,
            "SELECT",
            &format!("`system`.{}", clickhouse_identifier(table)),
            true,
        )
        .await?;
    }
    check_clickhouse_grant(principal, "SELECT", "`system`.`query_log`", false).await?;

    let allowed_access = match kind {
        ClickHousePrincipalKind::Schema => "('CREATE TABLE', 'ALTER TABLE', 'SELECT', 'INSERT')",
        ClickHousePrincipalKind::Writer => "('SELECT', 'INSERT')",
    };
    let violation_sql = format!(
        "SELECT count() AS value FROM system.grants \
         WHERE user_name = ? AND (grant_option != 0 OR is_partial_revoke != 0 \
           OR toString(access_type) NOT IN {allowed_access} \
           OR (toString(access_type) IN ('SELECT', 'INSERT') \
               AND (database IS NULL OR table IS NULL)))"
    );
    let violations = bootstrap
        .query(&violation_sql)
        .bind(username)
        .fetch_one::<ClickHouseCount>()
        .await?
        .value;
    if violations != 0 {
        anyhow::bail!(
            "ClickHouse principal {username} retained wildcard, grant-option, partial-revoke, or unexpected privileges"
        );
    }
    Ok(())
}

async fn check_clickhouse_grant(
    client: &clickhouse::Client,
    privilege: &str,
    object: &str,
    expected: bool,
) -> anyhow::Result<()> {
    let observed = client
        .query(&format!("CHECK GRANT {privilege} ON {object}"))
        .fetch_one::<ClickHouseGrantCheck>()
        .await?
        .result
        != 0;
    if observed != expected {
        anyhow::bail!(
            "ClickHouse privilege drifted: privilege={privilege}, object={object}, expected={expected}, observed={observed}"
        );
    }
    Ok(())
}

async fn grant_clickhouse_table(
    bootstrap: &clickhouse::Client,
    privilege: &str,
    database: &str,
    table: &str,
    role: &str,
) -> anyhow::Result<()> {
    bootstrap
        .query(&format!(
            "GRANT {privilege} ON {database}.{} TO {role}",
            clickhouse_identifier(table)
        ))
        .execute()
        .await?;
    Ok(())
}

pub(crate) fn clickhouse_identifier(value: &str) -> String {
    format!("`{value}`")
}

fn clickhouse_literal(value: &str) -> anyhow::Result<String> {
    if value
        .chars()
        .any(|character| matches!(character, '\0' | '\r' | '\n'))
    {
        anyhow::bail!("ClickHouse password contains a forbidden control delimiter");
    }
    Ok(format!(
        "'{}'",
        value.replace('\\', "\\\\").replace('\'', "\\'")
    ))
}
