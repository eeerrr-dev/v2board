use std::time::Duration;

use percent_encoding::percent_decode_str;
use sqlx::{AssertSqlSafe, Executor, PgPool, Postgres, SqlSafeStr, postgres::PgPoolOptions};
use url::Url;
use v2board_provision::MysqlImportExecutionPlan;

pub(crate) static POSTGRES_MIGRATOR: sqlx::migrate::Migrator =
    sqlx::migrate!("../../migrations-postgres");

pub(crate) const REQUIRED_POSTGRES_MAJOR: i32 = 18;
const REQUIRED_POSTGRES_ENCODING: &str = "UTF8";
pub(crate) const REQUIRED_POSTGRES_LOCALE_PROVIDER: &str = "b";
pub(crate) const REQUIRED_POSTGRES_BUILTIN_LOCALE: &str = "C.UTF-8";

pub(crate) struct PostgresIdentity {
    pub(crate) bootstrap: Url,
    pub(crate) migration: Url,
    pub(crate) api: Url,
    pub(crate) worker: Url,
    pub(crate) database: String,
    pub(crate) bootstrap_role: String,
    pub(crate) migration_role: String,
    pub(crate) api_role: String,
    pub(crate) worker_role: String,
    pub(crate) migration_password: String,
    pub(crate) api_password: String,
    pub(crate) worker_password: String,
}

impl PostgresIdentity {
    pub(crate) fn from_plan(plan: &MysqlImportExecutionPlan) -> anyhow::Result<Self> {
        let bootstrap = Url::parse(&plan.postgres.bootstrap_database_url)?;
        let migration = Url::parse(&plan.postgres.migration_database_url)?;
        let api = Url::parse(&plan.postgres.api_database_url)?;
        let worker = Url::parse(&plan.postgres.worker_database_url)?;
        Ok(Self {
            database: decoded_database(&migration)?,
            bootstrap_role: decoded_username(&bootstrap)?,
            migration_role: decoded_username(&migration)?,
            api_role: decoded_username(&api)?,
            worker_role: decoded_username(&worker)?,
            migration_password: decoded_password(&migration)?,
            api_password: decoded_password(&api)?,
            worker_password: decoded_password(&worker)?,
            bootstrap,
            migration,
            api,
            worker,
        })
    }
}

fn decoded_username(url: &Url) -> anyhow::Result<String> {
    Ok(percent_decode_str(url.username())
        .decode_utf8()?
        .into_owned())
}

fn decoded_password(url: &Url) -> anyhow::Result<String> {
    Ok(percent_decode_str(
        url.password()
            .ok_or_else(|| anyhow::anyhow!("validated PostgreSQL URL lost its password"))?,
    )
    .decode_utf8()?
    .into_owned())
}

fn decoded_database(url: &Url) -> anyhow::Result<String> {
    Ok(
        percent_decode_str(url.path().strip_prefix('/').unwrap_or_default())
            .decode_utf8()?
            .into_owned(),
    )
}

pub(crate) async fn preflight_postgres_absent(identity: &PostgresIdentity) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .connect(identity.bootstrap.as_str())
        .await?;
    let version_num: i32 =
        sqlx::query_scalar("SELECT current_setting('server_version_num')::INTEGER")
            .fetch_one(&pool)
            .await?;
    if version_num / 10_000 != REQUIRED_POSTGRES_MAJOR {
        anyhow::bail!(
            "target PostgreSQL must be major {REQUIRED_POSTGRES_MAJOR}, observed server_version_num {version_num}"
        );
    }
    let databases = sqlx::query_scalar::<_, String>(
        "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY datname",
    )
    .fetch_all(&pool)
    .await?;
    if databases != ["postgres"] {
        anyhow::bail!(
            "target PostgreSQL must be a dedicated empty cluster whose only non-template database is postgres; observed {databases:?}"
        );
    }
    let database_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&identity.database)
            .fetch_one(&pool)
            .await?;
    if database_exists {
        anyhow::bail!(
            "target PostgreSQL database {} already exists",
            identity.database
        );
    }
    for role in [
        &identity.migration_role,
        &identity.api_role,
        &identity.worker_role,
    ] {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = $1)")
                .bind(role)
                .fetch_one(&pool)
                .await?;
        if exists {
            anyhow::bail!("target PostgreSQL role {role} already exists");
        }
    }
    Ok(())
}

pub(crate) async fn bootstrap_postgres(identity: &PostgresIdentity) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .connect(identity.bootstrap.as_str())
        .await?;
    execute_dynamic(
        &pool,
        format!(
            "GRANT CONNECT ON DATABASE postgres TO {}",
            postgres_identifier(&identity.bootstrap_role)
        ),
    )
    .await?;
    for database in ["postgres", "template0", "template1"] {
        execute_dynamic(
            &pool,
            format!(
                "REVOKE CONNECT, CREATE, TEMPORARY ON DATABASE {} FROM PUBLIC",
                postgres_identifier(database)
            ),
        )
        .await?;
    }
    for (role, password) in [
        (&identity.migration_role, &identity.migration_password),
        (&identity.api_role, &identity.api_password),
        (&identity.worker_role, &identity.worker_password),
    ] {
        execute_dynamic(
            &pool,
            format!(
                "CREATE ROLE {} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS PASSWORD {}",
                postgres_identifier(role),
                postgres_literal(password)?
            ),
        )
        .await?;
    }
    execute_dynamic(
        &pool,
        format!(
            "CREATE DATABASE {} OWNER {} TEMPLATE template0 ENCODING '{REQUIRED_POSTGRES_ENCODING}' LOCALE_PROVIDER builtin BUILTIN_LOCALE '{REQUIRED_POSTGRES_BUILTIN_LOCALE}'",
            postgres_identifier(&identity.database),
            postgres_identifier(&identity.migration_role)
        ),
    )
    .await?;
    execute_dynamic(
        &pool,
        format!(
            "REVOKE CONNECT, CREATE, TEMPORARY ON DATABASE {} FROM PUBLIC",
            postgres_identifier(&identity.database)
        ),
    )
    .await?;
    Ok(())
}

pub(crate) async fn verify_postgres_target_contract(
    target: &PgPool,
    expected_database: &str,
) -> anyhow::Result<()> {
    type TargetContract = (i32, String, String, String, String, Option<String>);
    let contract = sqlx::query_as::<_, TargetContract>(
        "SELECT current_setting('server_version_num')::INTEGER, \
                pg_encoding_to_char(d.encoding), d.datlocprovider::text, \
                d.datcollate, d.datctype, d.datlocale \
         FROM pg_database AS d WHERE d.datname = current_database()",
    )
    .fetch_one(target)
    .await?;
    if contract.0 / 10_000 != REQUIRED_POSTGRES_MAJOR
        || contract.1 != REQUIRED_POSTGRES_ENCODING
        || contract.2 != REQUIRED_POSTGRES_LOCALE_PROVIDER
        || contract.3 != REQUIRED_POSTGRES_BUILTIN_LOCALE
        || contract.4 != REQUIRED_POSTGRES_BUILTIN_LOCALE
        || contract.5.as_deref() != Some(REQUIRED_POSTGRES_BUILTIN_LOCALE)
    {
        anyhow::bail!(
            "target PostgreSQL database contract drifted: database={expected_database}, version_num={}, encoding={}, locale_provider={}, collate={}, ctype={}, locale={:?}",
            contract.0,
            contract.1,
            contract.2,
            contract.3,
            contract.4,
            contract.5
        );
    }
    Ok(())
}

pub(crate) async fn retire_postgres_migration_role(
    identity: &PostgresIdentity,
) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .connect(identity.bootstrap.as_str())
        .await?;
    execute_dynamic(
        &pool,
        format!(
            "ALTER ROLE {} NOLOGIN PASSWORD NULL",
            postgres_identifier(&identity.migration_role)
        ),
    )
    .await?;
    let termination_results = sqlx::query_scalar::<_, bool>(
        "SELECT pg_terminate_backend(pid) \
         FROM pg_stat_activity \
         WHERE usename = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&identity.migration_role)
    .fetch_all(&pool)
    .await?;
    if termination_results.iter().any(|terminated| !terminated) {
        anyhow::bail!("temporary PostgreSQL migration role retained an unterminated session");
    }
    let mut active_sessions = i64::MAX;
    for _ in 0..100 {
        active_sessions = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pg_stat_activity WHERE usename = $1 AND pid <> pg_backend_pid()",
        )
        .bind(&identity.migration_role)
        .fetch_one(&pool)
        .await?;
        if active_sessions == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if active_sessions != 0 {
        anyhow::bail!(
            "temporary PostgreSQL migration role retained {active_sessions} active session(s)"
        );
    }
    let state: Option<(bool, bool)> =
        sqlx::query_as("SELECT rolcanlogin, rolpassword IS NULL FROM pg_authid WHERE rolname = $1")
            .bind(&identity.migration_role)
            .fetch_optional(&pool)
            .await?;
    if state != Some((false, true)) {
        anyhow::bail!("temporary PostgreSQL migration role was not fully retired");
    }
    if let Ok(pool) = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(identity.migration.as_str())
        .await
    {
        pool.close().await;
        anyhow::bail!("retired PostgreSQL migration role can still log in");
    }
    pool.close().await;
    Ok(())
}

pub(crate) fn postgres_identifier(value: &str) -> String {
    format!("\"{value}\"")
}

pub(crate) fn postgres_literal(value: &str) -> anyhow::Result<String> {
    if value.chars().any(|character| character == '\0') {
        anyhow::bail!("PostgreSQL password contains a NUL byte");
    }
    Ok(format!("'{}'", value.replace('\'', "''")))
}

pub(crate) async fn execute_dynamic<'e, E>(executor: E, sql: String) -> anyhow::Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    executor.execute(AssertSqlSafe(sql).into_sql_str()).await?;
    Ok(())
}
