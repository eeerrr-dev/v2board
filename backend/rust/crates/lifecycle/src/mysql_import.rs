use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
    str::FromStr,
    time::Duration,
};

use chrono::Utc;
use percent_encoding::percent_decode_str;
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{
    AssertSqlSafe, Column, Executor, MySql, MySqlConnection, PgPool, Postgres, QueryBuilder, Row,
    SqlSafeStr, TypeInfo, ValueRef,
    mysql::{MySqlPoolOptions, MySqlRow},
    postgres::PgPoolOptions,
    types::Json,
};
use url::Url;
use uuid::Uuid;
use v2board_analytics::{
    AnalyticsAdmissionPolicy, bind_clickhouse_installation, clickhouse_client,
    configure_clickhouse_retention, install_analytics_admission_policy, migrate_clickhouse,
    verify_clickhouse_bound_contract,
};
use v2board_config::{AppConfig, RuntimePaths};
use v2board_provision::{
    MysqlImportExecutionPlan, MysqlImportSpec, inspect_mysql_import,
    mysql_import_converter::{
        CanonicalRow, CanonicalValue, DEFAULT_BATCH_SIZE, DISCARDED_SOURCE_TABLES,
        INITIAL_SOURCE_ID_CURSOR, IdentityWidth, LegacyGiftcardRedemptionRow,
        MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256, MysqlImportRowDisposition,
        POSTGRES_MAX_BIND_PARAMETERS, SCALAR_REFERENCES, ScalarReferenceRule, SourceRow,
        SourceValue, TABLE_MAPPINGS, TableMapping, audit_registry, canonical_rows_sha256,
        copied_table_mappings, discarded_target_tables, expand_giftcard_redemptions,
        registry_sha256, sequence_reset_sql, source_batch_sql, target_batch_insert_sql,
        transform_mysql_import_row, validate_batch_ids,
    },
    mysql_import_policy::is_legacy_stripe_payment_driver,
};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

const API_CONFIG_FILE: &str = "api.config.json";
const WORKER_CONFIG_FILE: &str = "worker.config.json";
const REPORT_FILE: &str = "import-report.json";
const REQUIRED_POSTGRES_MAJOR: i32 = 18;
const REQUIRED_POSTGRES_ENCODING: &str = "UTF8";
const REQUIRED_POSTGRES_LOCALE_PROVIDER: &str = "b";
const REQUIRED_POSTGRES_BUILTIN_LOCALE: &str = "C.UTF-8";
const REQUIRED_CLICKHOUSE_MAJOR: u64 = 26;
const REQUIRED_CLICKHOUSE_MINOR: u64 = 3;
const REQUIRED_REDIS_MAJOR: u64 = 8;
const REQUIRED_REDIS_MINOR: u64 = 8;
const GIFTCARD_REDEMPTION_BIND_PARAMETERS: usize = 4;
const MAX_GIFTCARD_REDEMPTIONS_PER_INSERT: usize =
    POSTGRES_MAX_BIND_PARAMETERS / GIFTCARD_REDEMPTION_BIND_PARAMETERS;
const MAX_GIFTCARD_REFERENCE_IDS_PER_QUERY: usize = 10_000;
const API_REDIS_RW_KEY_PATTERNS: &[&str] = &[
    "AUTH_SESSION_*",
    "USER_SESSIONS_*",
    "AUTH_USER_SESSION_KEYS_*",
    "AUTH_STEP_UP_*",
    "TEMP_TOKEN_*",
    "PASSWORD_ERROR_LIMIT_ACCOUNT_*",
    "PASSWORD_ERROR_LIMIT_IP_*",
    "REGISTER_IP_RATE_LIMIT_V2_*",
    "SEND_EMAIL_VERIFY_LIMIT_*",
    "LAST_SEND_EMAIL_VERIFY_TIMESTAMP_*",
    "EMAIL_VERIFY_CODE_*",
    "FORGET_REQUEST_LIMIT_*",
    "otp_*",
    "otpn_*",
    "totp_*",
    "TELEGRAM_UPDATE_*",
    "ticket_sendEmailNotify_*",
    "ALIVE_IP_USER_*",
    "SERVER_*",
];
const API_REDIS_RO_KEY_PATTERNS: &[&str] = &[
    "SCHEDULE_LAST_CHECK_AT_",
    "RUST_WORKER_JOBS_TOTAL",
    "RUST_WORKER_JOBS_FAILED",
    "RUST_WORKER_LAST_RUN_AT",
    "RUST_WORKER_LAST_SUCCESS_AT",
    "RUST_WORKER_LAST_FAILURE_AT",
];
const WORKER_REDIS_RW_KEY_PATTERNS: &[&str] = &[
    "RUST_SCHEDULER_LOCK_*",
    "traffic_reset_lock",
    "SCHEDULE_LAST_CHECK_AT_",
    "RUST_WORKER_LOOP_HEARTBEAT_AT",
    "RUST_WORKER_JOBS_TOTAL",
    "RUST_WORKER_LAST_RUN_AT",
    "RUST_WORKER_LAST_SUCCESS_AT",
    "RUST_WORKER_JOBS_FAILED",
    "RUST_WORKER_LAST_FAILURE_AT",
    "RUST_ANALYTICS_ADMISSION",
];
const API_REDIS_COMMANDS: &[&str] = &[
    "+ping",
    "+info",
    "+get",
    "+mget",
    "+set",
    "+setex",
    "+getdel",
    "+del",
    "+hgetall",
    "+incr",
    "+decr",
    "+expire",
    "+expireat",
    "+ttl",
    "+exists",
    "+sadd",
    "+srem",
    "+smembers",
    "+zadd",
    "+zcard",
    "+zrem",
    "+zremrangebyscore",
    "+evalsha",
    "+script|load",
];
const WORKER_REDIS_COMMANDS: &[&str] = &[
    "+ping",
    "+info",
    "+get",
    "+set",
    "+del",
    "+exists",
    "+expire",
    "+hset",
    "+hincrby",
    "+hdel",
    "+evalsha",
    "+script|load",
];

#[derive(Debug, Serialize)]
pub(crate) struct MysqlImportExecutionReport {
    status: &'static str,
    schema_version: u32,
    manifest_sha256: String,
    inspected_dump_sha256: String,
    imported_source_schema_sha256: String,
    converter_registry_sha256: String,
    converted_snapshot_sha256: String,
    installation_id: String,
    imported_tables: Vec<ImportedTableReport>,
    discarded_tables: Vec<DiscardedTableReport>,
    api_config_sha256: String,
    worker_config_sha256: String,
    output_directory: String,
    clickhouse_started_empty: bool,
    redis_started_empty: bool,
    redis_acl_persisted: bool,
    redis_runtime_acl_isolated: bool,
    redis_bootstrap_credential_emitted: bool,
    old_mysql_contacted: bool,
    old_mysql_mutated: bool,
    old_redis_contacted: bool,
    stripe_provider_contacted: bool,
}

#[derive(Debug, Serialize)]
struct ImportedTableReport {
    source: String,
    target: String,
    source_rows: u64,
    retained_rows: u64,
    discarded_rows: u64,
    retained_sha256: String,
}

#[derive(Debug, Serialize)]
struct DiscardedTableReport {
    source: String,
    present: bool,
    rows_scanned: bool,
    policy: &'static str,
}

struct SourceSchemaInspection {
    imported_schema_sha256: String,
    discarded_tables: Vec<DiscardedTableReport>,
}

fn converted_snapshot_sha256(
    imported_tables: &[ImportedTableReport],
    discarded_tables: &[DiscardedTableReport],
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.converted-snapshot.v1\0");
    for table in imported_tables {
        digest_import_report_field(&mut digest, b"imported");
        digest_import_report_field(&mut digest, table.source.as_bytes());
        digest_import_report_field(&mut digest, table.target.as_bytes());
        digest_import_report_field(&mut digest, table.source_rows.to_string().as_bytes());
        digest_import_report_field(&mut digest, table.retained_rows.to_string().as_bytes());
        digest_import_report_field(&mut digest, table.discarded_rows.to_string().as_bytes());
        digest_import_report_field(&mut digest, table.retained_sha256.as_bytes());
    }
    for table in discarded_tables {
        digest_import_report_field(&mut digest, b"discarded");
        digest_import_report_field(&mut digest, table.source.as_bytes());
        digest_import_report_field(&mut digest, table.present.to_string().as_bytes());
        digest_import_report_field(&mut digest, table.rows_scanned.to_string().as_bytes());
        digest_import_report_field(&mut digest, table.policy.as_bytes());
    }
    hex::encode(digest.finalize())
}

fn digest_import_report_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_be_bytes());
    digest.update(field);
}

fn finalize_users_retained_sha256(base_sha256: &str, inviter_sha256: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.users-final.v1\0");
    digest_import_report_field(&mut digest, base_sha256.as_bytes());
    digest_import_report_field(&mut digest, inviter_sha256.as_bytes());
    hex::encode(digest.finalize())
}

fn digest_user_inviter_row(digest: &mut Sha256, user_id: i64, invite_user_id: Option<i64>) {
    digest_import_report_field(digest, user_id.to_string().as_bytes());
    match invite_user_id {
        Some(inviter) => {
            digest_import_report_field(digest, b"some");
            digest_import_report_field(digest, inviter.to_string().as_bytes());
        }
        None => digest_import_report_field(digest, b"none"),
    }
}

struct PostgresIdentity {
    bootstrap: Url,
    migration: Url,
    api: Url,
    worker: Url,
    database: String,
    bootstrap_role: String,
    migration_role: String,
    api_role: String,
    worker_role: String,
    migration_password: String,
    api_password: String,
    worker_password: String,
}

struct RedisRuntimeIdentity {
    api_url: String,
    worker_url: String,
}

pub(crate) async fn execute(spec: &MysqlImportSpec) -> anyhow::Result<MysqlImportExecutionReport> {
    audit_registry()?;
    let inspection = inspect_mysql_import(spec)?;
    let plan = spec.execution_plan()?;
    execute_validated(spec, inspection, plan).await
}

async fn execute_validated(
    spec: &MysqlImportSpec,
    inspection: v2board_provision::MysqlImportInspection,
    mut plan: MysqlImportExecutionPlan,
) -> anyhow::Result<MysqlImportExecutionReport> {
    require_absent_output_directory(&plan.config_output_directory)?;

    let source_pool = MySqlPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .connect(&spec.source.database_url)
        .await?;
    let mut source = source_pool.acquire().await?;
    begin_mysql_read_snapshot(&mut source).await?;
    verify_mysql_vendor_and_version(&mut source).await?;
    verify_mysql_source_principal(&mut source, &spec.source.database_url).await?;
    let source_schema = inspect_source_schema(&mut source).await?;
    if source_schema.imported_schema_sha256 != MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256 {
        anyhow::bail!(
            "imported legacy MySQL schema does not match the pinned source profile: expected {}, observed {}",
            MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256,
            source_schema.imported_schema_sha256
        );
    }
    validate_source_relationships(&mut source).await?;
    let SourceSchemaInspection {
        imported_schema_sha256,
        discarded_tables,
    } = source_schema;

    let postgres = PostgresIdentity::from_plan(&plan)?;
    preflight_postgres_absent(&postgres).await?;
    preflight_clickhouse_absent(&plan).await?;
    preflight_redis_target(&plan.redis_bootstrap_url).await?;

    bootstrap_postgres(&postgres).await?;
    let target = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(4)
        .connect(plan.postgres.migration_database_url.as_str())
        .await?;
    verify_postgres_target_contract(&target, &postgres.database).await?;
    POSTGRES_MIGRATOR.run(&target).await?;

    let installation_id = Uuid::new_v4();
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO system_installation (singleton, installation_id, created_at) VALUES (1, $1, $2)",
    )
    .bind(installation_id)
    .bind(now)
    .execute(&target)
    .await?;

    let imported_tables = copy_business_data(&mut source, &target).await?;
    let converted_snapshot_sha256 = converted_snapshot_sha256(&imported_tables, &discarded_tables);
    commit_mysql_read_snapshot(&mut source).await?;
    drop(source);
    source_pool.close().await;
    install_admission(&target, installation_id, now, &plan).await?;
    v2board_domain::operator_config::seed_initial_authority(
        &target,
        installation_id,
        &plan.app_key,
        &plan.operator_config,
        "mysql-import.v1",
    )
    .await?;
    install_postgres_runtime_grants(&target, &postgres).await?;

    bootstrap_clickhouse(&plan, installation_id, now).await?;
    verify_empty_redis(&plan.redis_bootstrap_url).await?;
    let redis = bootstrap_redis_runtime(&plan.redis_bootstrap_url, installation_id).await?;
    plan.api_boot_config
        .insert("redis_url".to_string(), Value::String(redis.api_url));
    plan.worker_boot_config
        .insert("redis_url".to_string(), Value::String(redis.worker_url));
    validate_emitted_runtime_configs(&plan.api_boot_config, &plan.worker_boot_config)?;
    verify_empty_redis(&plan.redis_bootstrap_url).await?;
    target.close().await;
    retire_postgres_migration_role(&postgres).await?;

    let api_config = pretty_json_bytes(&plan.api_boot_config)?;
    let worker_config = pretty_json_bytes(&plan.worker_boot_config)?;
    let api_config_sha256 = sha256_hex(&api_config);
    let worker_config_sha256 = sha256_hex(&worker_config);
    let report = MysqlImportExecutionReport {
        status: "complete",
        schema_version: spec.schema_version,
        manifest_sha256: spec.manifest_sha256().to_string(),
        inspected_dump_sha256: inspection.mysql_dump.sha256,
        imported_source_schema_sha256: imported_schema_sha256,
        converter_registry_sha256: registry_sha256()?,
        converted_snapshot_sha256,
        installation_id: installation_id.to_string(),
        imported_tables,
        discarded_tables,
        api_config_sha256,
        worker_config_sha256,
        output_directory: plan.config_output_directory.display().to_string(),
        clickhouse_started_empty: true,
        redis_started_empty: true,
        redis_acl_persisted: true,
        redis_runtime_acl_isolated: true,
        redis_bootstrap_credential_emitted: false,
        old_mysql_contacted: true,
        old_mysql_mutated: false,
        old_redis_contacted: false,
        stripe_provider_contacted: false,
    };
    emit_output_bundle(
        &plan.config_output_directory,
        &api_config,
        &worker_config,
        &report,
    )?;
    Ok(report)
}

fn require_absent_output_directory(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            anyhow::bail!(
                "config output path already exists; refuse to overwrite {}",
                path.display()
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("config output directory has no parent"))?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o777 != 0o700
        || metadata.uid() != 0
    {
        anyhow::bail!(
            "config output parent must be an existing root-owned owner-only non-symlink directory: {}",
            parent.display()
        );
    }
    Ok(())
}

fn pretty_json_bytes(value: &Map<String, Value>) -> anyhow::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_emitted_runtime_configs(
    api: &Map<String, Value>,
    worker: &Map<String, Value>,
) -> anyhow::Result<()> {
    let paths = |config: &str| RuntimePaths {
        config: config.into(),
        frontend: "/opt/v2board/frontend".into(),
        rules: "/var/lib/v2board/rules".into(),
    };
    AppConfig::try_from_api_boot_config_map(
        api.clone(),
        paths("/var/lib/v2board/api/config.json"),
    )?;
    AppConfig::try_from_worker_boot_config_map(
        worker.clone(),
        paths("/var/lib/v2board/worker/config.json"),
    )?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn emit_output_bundle(
    directory: &Path,
    api: &[u8],
    worker: &[u8],
    report: &MysqlImportExecutionReport,
) -> anyhow::Result<()> {
    let parent = directory
        .parent()
        .ok_or_else(|| anyhow::anyhow!("config output directory has no parent"))?;
    let parent_before = fs::symlink_metadata(parent)?;
    let mut builder = DirBuilder::new();
    builder.mode(0o700).create(directory)?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    let parent_after = fs::symlink_metadata(parent)?;
    if !same_file_identity(&parent_before, &parent_after)
        || !parent_after.file_type().is_dir()
        || parent_after.file_type().is_symlink()
        || parent_after.permissions().mode() & 0o777 != 0o700
        || parent_after.uid() != 0
    {
        anyhow::bail!("config output parent changed while creating the output bundle");
    }
    let directory_path_metadata = fs::symlink_metadata(directory)?;
    let directory_handle = File::open(directory)?;
    let directory_handle_metadata = directory_handle.metadata()?;
    if !same_file_identity(&directory_path_metadata, &directory_handle_metadata)
        || !directory_path_metadata.file_type().is_dir()
        || directory_path_metadata.file_type().is_symlink()
        || directory_path_metadata.permissions().mode() & 0o777 != 0o700
        || directory_path_metadata.uid() != 0
        || directory_handle_metadata.uid() != 0
    {
        anyhow::bail!("config output directory identity or permissions changed during creation");
    }
    write_new_secret_file(&directory.join(API_CONFIG_FILE), api)?;
    write_new_secret_file(&directory.join(WORKER_CONFIG_FILE), worker)?;
    let mut report_bytes = serde_json::to_vec_pretty(report)?;
    report_bytes.push(b'\n');
    write_new_secret_file(&directory.join(REPORT_FILE), &report_bytes)?;
    directory_handle.sync_all()?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

fn write_new_secret_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

impl PostgresIdentity {
    fn from_plan(plan: &MysqlImportExecutionPlan) -> anyhow::Result<Self> {
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

async fn preflight_postgres_absent(identity: &PostgresIdentity) -> anyhow::Result<()> {
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

async fn bootstrap_postgres(identity: &PostgresIdentity) -> anyhow::Result<()> {
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

async fn verify_postgres_target_contract(
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

async fn retire_postgres_migration_role(identity: &PostgresIdentity) -> anyhow::Result<()> {
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

fn postgres_identifier(value: &str) -> String {
    format!("\"{value}\"")
}

fn postgres_literal(value: &str) -> anyhow::Result<String> {
    if value.chars().any(|character| character == '\0') {
        anyhow::bail!("PostgreSQL password contains a NUL byte");
    }
    Ok(format!("'{}'", value.replace('\'', "''")))
}

async fn execute_dynamic<'e, E>(executor: E, sql: String) -> anyhow::Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    executor.execute(AssertSqlSafe(sql).into_sql_str()).await?;
    Ok(())
}

const RUNTIME_TABLES: &[&str] = &[
    "_sqlx_migrations",
    "system_installation",
    "server_group",
    "plan",
    "payment_method",
    "coupon",
    "users",
    "orders",
    "commission_log",
    "invite_code",
    "gift_card",
    "gift_card_redemption",
    "payment_reconciliation",
    "knowledge",
    "notice",
    "ticket",
    "ticket_message",
    "system_log",
    "mail_log",
    "mail_outbox_batch",
    "mail_outbox",
    "stat",
    "server_traffic",
    "user_traffic",
    "analytics_delivery_batch",
    "analytics_outbox",
    "server_traffic_report",
    "server_traffic_report_item",
    "server_credential",
    "server_route",
    "server_shadowsocks",
    "server_vmess",
    "server_trojan",
    "server_tuic",
    "server_hysteria",
    "server_vless",
    "server_anytls",
    "server_v2node",
    "analytics_admission_policy",
    "analytics_admission_state",
    "operator_config_revision",
    "operator_config_state",
    "operator_config_api_ack",
    "operator_config_worker_ack",
];

const API_SELECT_TABLES: &[&str] = &[
    "_sqlx_migrations",
    "system_installation",
    "server_group",
    "plan",
    "payment_method",
    "coupon",
    "users",
    "orders",
    "commission_log",
    "invite_code",
    "gift_card",
    "gift_card_redemption",
    "payment_reconciliation",
    "knowledge",
    "notice",
    "ticket",
    "ticket_message",
    "system_log",
    "mail_outbox_batch",
    "stat",
    "server_traffic",
    "user_traffic",
    "analytics_outbox",
    "server_traffic_report",
    "server_credential",
    "server_route",
    "server_shadowsocks",
    "server_vmess",
    "server_trojan",
    "server_tuic",
    "server_hysteria",
    "server_vless",
    "server_anytls",
    "server_v2node",
    "analytics_admission_policy",
    "analytics_admission_state",
    "operator_config_revision",
    "operator_config_state",
];

const API_INSERT_TABLES: &[&str] = &[
    "server_group",
    "plan",
    "payment_method",
    "coupon",
    "users",
    "orders",
    "invite_code",
    "gift_card",
    "gift_card_redemption",
    "payment_reconciliation",
    "knowledge",
    "notice",
    "ticket",
    "ticket_message",
    "mail_outbox_batch",
    "mail_outbox",
    "server_traffic",
    "user_traffic",
    "analytics_outbox",
    "server_traffic_report",
    "server_traffic_report_item",
    "server_credential",
    "server_route",
    "server_shadowsocks",
    "server_vmess",
    "server_trojan",
    "server_tuic",
    "server_hysteria",
    "server_vless",
    "server_anytls",
    "server_v2node",
    "operator_config_revision",
];

const API_UPDATE_TABLES: &[&str] = &[
    "server_group",
    "plan",
    "payment_method",
    "coupon",
    "users",
    "orders",
    "invite_code",
    "gift_card",
    "payment_reconciliation",
    "knowledge",
    "notice",
    "ticket",
    "mail_outbox_batch",
    "server_traffic",
    "user_traffic",
    "server_credential",
    "server_route",
    "server_shadowsocks",
    "server_vmess",
    "server_trojan",
    "server_tuic",
    "server_hysteria",
    "server_vless",
    "server_anytls",
    "server_v2node",
    "analytics_admission_state",
    "operator_config_state",
];

const API_DELETE_TABLES: &[&str] = &[
    "server_group",
    "plan",
    "coupon",
    "users",
    "orders",
    "invite_code",
    "gift_card",
    "knowledge",
    "notice",
    "ticket",
    "ticket_message",
    "server_credential",
    "server_route",
    "server_shadowsocks",
    "server_vmess",
    "server_trojan",
    "server_tuic",
    "server_hysteria",
    "server_vless",
    "server_anytls",
    "server_v2node",
];

const WORKER_SELECT_TABLES: &[&str] = &[
    "_sqlx_migrations",
    "system_installation",
    "plan",
    "mail_outbox_batch",
    "mail_outbox",
    "analytics_delivery_batch",
    "analytics_outbox",
    "server_traffic_report",
    "server_traffic_report_item",
    "analytics_admission_policy",
    "analytics_admission_state",
];

const WORKER_INSERT_TABLES: &[&str] = &[
    "mail_outbox_batch",
    "mail_outbox",
    "analytics_delivery_batch",
    "analytics_outbox",
];

const WORKER_UPDATE_TABLES: &[&str] = &[
    "mail_outbox_batch",
    "mail_outbox",
    "analytics_delivery_batch",
    "analytics_outbox",
    "server_traffic_report",
    "analytics_admission_state",
];

const WORKER_DELETE_TABLES: &[&str] = &[
    "system_log",
    "mail_log",
    "mail_outbox_batch",
    "mail_outbox",
    "server_traffic",
    "user_traffic",
    "analytics_delivery_batch",
    "analytics_outbox",
    "server_traffic_report",
    "server_traffic_report_item",
];

#[derive(Clone, Copy)]
struct PostgresColumnGrant {
    privilege: &'static str,
    table: &'static str,
    columns: &'static [&'static str],
}

// Locking SELECTs require UPDATE on at least one column in PostgreSQL. The
// narrow analytics_outbox grant is only that lock capability; API code does not
// update the column.
const API_COLUMN_GRANTS: &[PostgresColumnGrant] = &[
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "analytics_outbox",
        columns: &["created_at"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "operator_config_api_ack",
        columns: &["singleton"],
    },
    PostgresColumnGrant {
        privilege: "INSERT",
        table: "operator_config_api_ack",
        columns: &[
            "singleton",
            "installation_id",
            "observed_revision",
            "applied_revision",
            "status",
            "error_code",
            "observed_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "operator_config_api_ack",
        columns: &[
            "observed_revision",
            "applied_revision",
            "status",
            "error_code",
            "observed_at",
        ],
    },
];

// Worker owns queue machinery, but sensitive business and retention tables are
// column-scoped so a compromised scheduler cannot read credentials/messages or
// rewrite identity, ownership, and money fields it never uses.
const WORKER_COLUMN_GRANTS: &[PostgresColumnGrant] = &[
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "payment_method",
        columns: &["id", "payment", "config"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "users",
        columns: &[
            "id",
            "invite_user_id",
            "email",
            "balance",
            "discount",
            "commission_type",
            "commission_rate",
            "commission_balance",
            "traffic_epoch",
            "scheduled_traffic_reset_key",
            "u",
            "d",
            "transfer_enable",
            "device_limit",
            "banned",
            "group_id",
            "plan_id",
            "speed_limit",
            "auto_renewal",
            "remind_expire",
            "remind_traffic",
            "expired_at",
            "created_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "users",
        columns: &[
            "balance",
            "commission_balance",
            "traffic_epoch",
            "scheduled_traffic_reset_key",
            "t",
            "u",
            "d",
            "transfer_enable",
            "device_limit",
            "group_id",
            "plan_id",
            "speed_limit",
            "auto_renewal",
            "expired_at",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "orders",
        columns: &[
            "id",
            "invite_user_id",
            "user_id",
            "plan_id",
            "payment_id",
            "type",
            "period",
            "trade_no",
            "callback_no",
            "total_amount",
            "balance_amount",
            "refund_amount",
            "surplus_order_ids",
            "status",
            "commission_status",
            "commission_balance",
            "actual_commission_balance",
            "paid_at",
            "created_at",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "INSERT",
        table: "orders",
        columns: &[
            "user_id",
            "plan_id",
            "type",
            "period",
            "trade_no",
            "total_amount",
            "balance_amount",
            "status",
            "created_at",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "orders",
        columns: &[
            "status",
            "commission_status",
            "actual_commission_balance",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "commission_log",
        columns: &["get_amount", "created_at"],
    },
    PostgresColumnGrant {
        privilege: "INSERT",
        table: "commission_log",
        columns: &[
            "invite_user_id",
            "user_id",
            "trade_no",
            "order_amount",
            "get_amount",
            "created_at",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "ticket",
        columns: &["id", "user_id", "status", "reply_status", "updated_at"],
    },
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "ticket",
        columns: &["status", "updated_at"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "ticket_message",
        columns: &["id", "ticket_id", "user_id"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "system_log",
        columns: &["id", "created_at"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "mail_log",
        columns: &["id", "created_at"],
    },
    PostgresColumnGrant {
        privilege: "INSERT",
        table: "mail_log",
        columns: &[
            "email",
            "subject",
            "template_name",
            "error",
            "created_at",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "stat",
        columns: &["record_at"],
    },
    PostgresColumnGrant {
        privilege: "INSERT",
        table: "stat",
        columns: &[
            "record_at",
            "record_type",
            "order_count",
            "order_total",
            "commission_count",
            "commission_total",
            "paid_count",
            "paid_total",
            "register_count",
            "invite_count",
            "transfer_used_total",
            "created_at",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "stat",
        columns: &[
            "order_count",
            "order_total",
            "commission_count",
            "commission_total",
            "paid_count",
            "paid_total",
            "register_count",
            "invite_count",
            "transfer_used_total",
            "updated_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "server_traffic",
        columns: &["id", "u", "d", "record_at", "created_at"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "user_traffic",
        columns: &["id", "record_at"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "operator_config_revision",
        columns: &[
            "revision",
            "revision_id",
            "format_version",
            "installation_id",
            "public_config",
            "secret_nonce",
            "secret_ciphertext",
            "secret_tag",
            "config_hmac_sha256",
        ],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "operator_config_state",
        columns: &["singleton", "installation_id", "active_revision"],
    },
    PostgresColumnGrant {
        privilege: "SELECT",
        table: "operator_config_worker_ack",
        columns: &["singleton"],
    },
    PostgresColumnGrant {
        privilege: "INSERT",
        table: "operator_config_worker_ack",
        columns: &[
            "singleton",
            "installation_id",
            "observed_revision",
            "applied_revision",
            "status",
            "error_code",
            "observed_at",
        ],
    },
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "operator_config_worker_ack",
        columns: &[
            "observed_revision",
            "applied_revision",
            "status",
            "error_code",
            "observed_at",
        ],
    },
    // PostgreSQL requires one UPDATE-capable column for SELECT ... FOR SHARE;
    // the renewal worker never issues a plan UPDATE.
    PostgresColumnGrant {
        privilege: "UPDATE",
        table: "plan",
        columns: &["updated_at"],
    },
];

const API_SEQUENCES: &[&str] = &[
    "server_group_id_seq",
    "plan_id_seq",
    "payment_method_id_seq",
    "coupon_id_seq",
    "users_id_seq",
    "orders_id_seq",
    "invite_code_id_seq",
    "gift_card_id_seq",
    "payment_reconciliation_id_seq",
    "knowledge_id_seq",
    "notice_id_seq",
    "ticket_id_seq",
    "ticket_message_id_seq",
    "mail_outbox_id_seq",
    "server_traffic_id_seq",
    "user_traffic_id_seq",
    "analytics_outbox_outbox_id_seq",
    "server_route_id_seq",
    "server_shadowsocks_id_seq",
    "server_vmess_id_seq",
    "server_trojan_id_seq",
    "server_tuic_id_seq",
    "server_hysteria_id_seq",
    "server_vless_id_seq",
    "server_anytls_id_seq",
    "server_v2node_id_seq",
    "operator_config_revision_revision_seq",
];

const WORKER_SEQUENCES: &[&str] = &[
    "orders_id_seq",
    "commission_log_id_seq",
    "mail_log_id_seq",
    "mail_outbox_id_seq",
    "stat_id_seq",
    "analytics_outbox_outbox_id_seq",
];

const RUNTIME_SEQUENCES: &[&str] = &[
    "server_group_id_seq",
    "plan_id_seq",
    "payment_method_id_seq",
    "coupon_id_seq",
    "users_id_seq",
    "orders_id_seq",
    "commission_log_id_seq",
    "invite_code_id_seq",
    "gift_card_id_seq",
    "payment_reconciliation_id_seq",
    "knowledge_id_seq",
    "notice_id_seq",
    "ticket_id_seq",
    "ticket_message_id_seq",
    "system_log_id_seq",
    "mail_log_id_seq",
    "mail_outbox_id_seq",
    "stat_id_seq",
    "server_traffic_id_seq",
    "user_traffic_id_seq",
    "analytics_outbox_outbox_id_seq",
    "server_route_id_seq",
    "server_shadowsocks_id_seq",
    "server_vmess_id_seq",
    "server_trojan_id_seq",
    "server_tuic_id_seq",
    "server_hysteria_id_seq",
    "server_vless_id_seq",
    "server_anytls_id_seq",
    "server_v2node_id_seq",
    "operator_config_revision_revision_seq",
];

#[derive(Clone, Copy)]
enum PostgresRuntimeRole {
    Api,
    Worker,
}

async fn install_postgres_runtime_grants(
    target: &PgPool,
    identity: &PostgresIdentity,
) -> anyhow::Result<()> {
    let api = postgres_identifier(&identity.api_role);
    let worker = postgres_identifier(&identity.worker_role);
    let database = postgres_identifier(&identity.database);
    execute_dynamic(
        target,
        format!("REVOKE ALL ON DATABASE {database} FROM PUBLIC, {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("GRANT CONNECT ON DATABASE {database} TO {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("REVOKE ALL ON SCHEMA public FROM PUBLIC, {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("GRANT USAGE ON SCHEMA public TO {api}, {worker}"),
    )
    .await?;

    execute_dynamic(
        target,
        format!("REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC, {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM PUBLIC, {api}, {worker}"),
    )
    .await?;

    for (role, privilege, tables) in [
        (&api, "SELECT", API_SELECT_TABLES),
        (&api, "INSERT", API_INSERT_TABLES),
        (&api, "UPDATE", API_UPDATE_TABLES),
        (&api, "DELETE", API_DELETE_TABLES),
        (&worker, "SELECT", WORKER_SELECT_TABLES),
        (&worker, "INSERT", WORKER_INSERT_TABLES),
        (&worker, "UPDATE", WORKER_UPDATE_TABLES),
        (&worker, "DELETE", WORKER_DELETE_TABLES),
    ] {
        grant_postgres_tables(target, role, privilege, tables).await?;
    }
    for (role, grants) in [(&api, API_COLUMN_GRANTS), (&worker, WORKER_COLUMN_GRANTS)] {
        for grant in grants {
            grant_postgres_columns(target, role, grant.privilege, grant.table, grant.columns)
                .await?;
        }
    }
    grant_postgres_sequences(target, &api, API_SEQUENCES).await?;
    grant_postgres_sequences(target, &worker, WORKER_SEQUENCES).await?;
    verify_postgres_runtime_roles(identity).await
}

async fn grant_postgres_tables(
    target: &PgPool,
    role: &str,
    privilege: &str,
    tables: &[&str],
) -> anyhow::Result<()> {
    let tables = tables
        .iter()
        .map(|table| postgres_identifier(table))
        .collect::<Vec<_>>()
        .join(", ");
    execute_dynamic(
        target,
        format!("GRANT {privilege} ON TABLE {tables} TO {role}"),
    )
    .await
}

async fn grant_postgres_sequences(
    target: &PgPool,
    role: &str,
    sequences: &[&str],
) -> anyhow::Result<()> {
    let sequences = sequences
        .iter()
        .map(|sequence| postgres_identifier(sequence))
        .collect::<Vec<_>>()
        .join(", ");
    execute_dynamic(
        target,
        format!("GRANT USAGE ON SEQUENCE {sequences} TO {role}"),
    )
    .await
}

async fn grant_postgres_columns(
    target: &PgPool,
    role: &str,
    privilege: &str,
    table: &str,
    columns: &[&str],
) -> anyhow::Result<()> {
    let columns = columns
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    execute_dynamic(
        target,
        format!(
            "GRANT {privilege} ({columns}) ON TABLE {} TO {role}",
            postgres_identifier(table)
        ),
    )
    .await
}

async fn verify_postgres_runtime_roles(identity: &PostgresIdentity) -> anyhow::Result<()> {
    for (kind, url, expected_role) in [
        (PostgresRuntimeRole::Api, &identity.api, &identity.api_role),
        (
            PostgresRuntimeRole::Worker,
            &identity.worker,
            &identity.worker_role,
        ),
    ] {
        let pool = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(url.as_str())
            .await?;
        let (role, database, installation): (String, String, Uuid) = sqlx::query_as(
            "SELECT current_user, current_database(), installation_id FROM system_installation WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await?;
        if role != **expected_role || database != identity.database {
            anyhow::bail!("PostgreSQL runtime role connected to an unexpected identity");
        }
        let (
            can_connect,
            can_create_database_objects,
            can_create_temp,
            can_use_schema,
            can_create_schema_objects,
        ): (bool, bool, bool, bool, bool) = sqlx::query_as(
            "SELECT has_database_privilege(current_user, current_database(), 'CONNECT'), \
                    has_database_privilege(current_user, current_database(), 'CREATE'), \
                    has_database_privilege(current_user, current_database(), 'TEMP'), \
                    has_schema_privilege(current_user, 'public', 'USAGE'), \
                    has_schema_privilege(current_user, 'public', 'CREATE')",
        )
        .fetch_one(&pool)
        .await?;
        if !can_connect
            || can_create_database_objects
            || can_create_temp
            || !can_use_schema
            || can_create_schema_objects
            || installation.is_nil()
        {
            anyhow::bail!(
                "PostgreSQL runtime role retained DDL/TEMP or lost its schema/installation binding"
            );
        }
        let (can_connect_postgres, can_temp_postgres, can_connect_template1, can_temp_template1): (
            bool,
            bool,
            bool,
            bool,
        ) = sqlx::query_as(
            "SELECT has_database_privilege(current_user, 'postgres', 'CONNECT'), \
                    has_database_privilege(current_user, 'postgres', 'TEMP'), \
                    has_database_privilege(current_user, 'template1', 'CONNECT'), \
                    has_database_privilege(current_user, 'template1', 'TEMP')",
        )
        .fetch_one(&pool)
        .await?;
        if can_connect_postgres || can_temp_postgres || can_connect_template1 || can_temp_template1
        {
            anyhow::bail!(
                "PostgreSQL runtime role can escape the target database through the dedicated cluster"
            );
        }
        let databases = sqlx::query_scalar::<_, String>(
            "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY datname",
        )
        .fetch_all(&pool)
        .await?;
        let mut expected_databases = vec!["postgres".to_string(), identity.database.clone()];
        expected_databases.sort();
        if databases != expected_databases {
            anyhow::bail!(
                "dedicated PostgreSQL cluster gained an unexpected non-template database: {databases:?}"
            );
        }
        verify_postgres_migration_ledger_access(&pool).await?;
        verify_postgres_table_acl(&pool, kind).await?;
        verify_postgres_column_acl(&pool, kind).await?;
        verify_postgres_sequence_acl(&pool, kind).await?;
        pool.close().await;

        let mut maintenance_url = (*url).clone();
        maintenance_url.set_path("/postgres");
        if PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(maintenance_url.as_str())
            .await
            .is_ok()
        {
            anyhow::bail!(
                "PostgreSQL runtime role unexpectedly connected to the maintenance database"
            );
        }
    }
    Ok(())
}

async fn verify_postgres_migration_ledger_access(pool: &PgPool) -> anyhow::Result<()> {
    let applied = sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
        "SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await?;
    let embedded = POSTGRES_MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .collect::<Vec<_>>();
    if applied.len() != embedded.len()
        || applied
            .iter()
            .zip(embedded)
            .any(|((version, checksum, success), migration)| {
                !success
                    || *version != migration.version
                    || checksum.as_slice() != migration.checksum.as_ref()
            })
    {
        anyhow::bail!("PostgreSQL runtime role cannot verify the exact migration ledger");
    }
    Ok(())
}

fn expected_postgres_tables(kind: PostgresRuntimeRole, privilege: &str) -> &'static [&'static str] {
    match (kind, privilege) {
        (PostgresRuntimeRole::Api, "SELECT") => API_SELECT_TABLES,
        (PostgresRuntimeRole::Api, "INSERT") => API_INSERT_TABLES,
        (PostgresRuntimeRole::Api, "UPDATE") => API_UPDATE_TABLES,
        (PostgresRuntimeRole::Api, "DELETE") => API_DELETE_TABLES,
        (PostgresRuntimeRole::Worker, "SELECT") => WORKER_SELECT_TABLES,
        (PostgresRuntimeRole::Worker, "INSERT") => WORKER_INSERT_TABLES,
        (PostgresRuntimeRole::Worker, "UPDATE") => WORKER_UPDATE_TABLES,
        (PostgresRuntimeRole::Worker, "DELETE") => WORKER_DELETE_TABLES,
        _ => &[],
    }
}

async fn verify_postgres_table_acl(pool: &PgPool, kind: PostgresRuntimeRole) -> anyhow::Result<()> {
    for table in RUNTIME_TABLES {
        let qualified = format!("public.{table}");
        for privilege in [
            "SELECT",
            "INSERT",
            "UPDATE",
            "DELETE",
            "TRUNCATE",
            "REFERENCES",
            "TRIGGER",
        ] {
            let observed: bool =
                sqlx::query_scalar("SELECT has_table_privilege(current_user, $1::text, $2::text)")
                    .bind(&qualified)
                    .bind(privilege)
                    .fetch_one(pool)
                    .await?;
            let expected = expected_postgres_tables(kind, privilege).contains(table);
            if observed != expected {
                anyhow::bail!(
                    "PostgreSQL runtime table privilege drifted: table={table}, privilege={privilege}, expected={expected}, observed={observed}"
                );
            }
        }
    }
    Ok(())
}

async fn verify_postgres_sequence_acl(
    pool: &PgPool,
    kind: PostgresRuntimeRole,
) -> anyhow::Result<()> {
    let expected_sequences = match kind {
        PostgresRuntimeRole::Api => API_SEQUENCES,
        PostgresRuntimeRole::Worker => WORKER_SEQUENCES,
    };
    for sequence in RUNTIME_SEQUENCES {
        let qualified = format!("public.{sequence}");
        for privilege in ["USAGE", "SELECT", "UPDATE"] {
            let observed: bool = sqlx::query_scalar(
                "SELECT has_sequence_privilege(current_user, $1::text, $2::text)",
            )
            .bind(&qualified)
            .bind(privilege)
            .fetch_one(pool)
            .await?;
            let expected = privilege == "USAGE" && expected_sequences.contains(sequence);
            if observed != expected {
                anyhow::bail!(
                    "PostgreSQL runtime sequence privilege drifted: sequence={sequence}, privilege={privilege}, expected={expected}, observed={observed}"
                );
            }
        }
    }
    Ok(())
}

async fn verify_postgres_column_acl(
    pool: &PgPool,
    kind: PostgresRuntimeRole,
) -> anyhow::Result<()> {
    let columns = sqlx::query_as::<_, (String, String)>(
        "SELECT c.relname, a.attname \
         FROM pg_catalog.pg_class AS c \
         JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace \
         JOIN pg_catalog.pg_attribute AS a ON a.attrelid = c.oid \
         WHERE n.nspname = 'public' AND c.relkind IN ('r', 'p') \
           AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY c.relname, a.attnum",
    )
    .fetch_all(pool)
    .await?;
    let observed_tables = columns
        .iter()
        .map(|(table, _)| table.as_str())
        .collect::<BTreeSet<_>>();
    let expected_tables = RUNTIME_TABLES.iter().copied().collect::<BTreeSet<_>>();
    if observed_tables != expected_tables {
        anyhow::bail!("PostgreSQL runtime ACL verifier table registry drifted");
    }
    for (table, column) in columns {
        for privilege in ["SELECT", "INSERT", "UPDATE"] {
            let observed: bool = sqlx::query_scalar(
                "SELECT has_column_privilege(current_user, $1::text, $2::text, $3::text)",
            )
            .bind(format!("public.{table}"))
            .bind(&column)
            .bind(privilege)
            .fetch_one(pool)
            .await?;
            let expected = expected_postgres_tables(kind, privilege).contains(&table.as_str())
                || postgres_column_grants(kind).iter().any(|grant| {
                    grant.privilege == privilege
                        && grant.table == table
                        && grant.columns.contains(&column.as_str())
                });
            if observed != expected {
                anyhow::bail!(
                    "PostgreSQL runtime column privilege drifted: table={table}, column={column}, privilege={privilege}, expected={expected}, observed={observed}"
                );
            }
        }
    }
    Ok(())
}

fn postgres_column_grants(kind: PostgresRuntimeRole) -> &'static [PostgresColumnGrant] {
    match kind {
        PostgresRuntimeRole::Api => API_COLUMN_GRANTS,
        PostgresRuntimeRole::Worker => WORKER_COLUMN_GRANTS,
    }
}

async fn verify_mysql_vendor_and_version(source: &mut MySqlConnection) -> anyhow::Result<()> {
    let (version, comment): (String, String) =
        sqlx::query_as("SELECT VERSION(), @@version_comment")
            .fetch_one(&mut *source)
            .await?;
    let lowercase = format!("{version} {comment}").to_ascii_lowercase();
    if !(version.starts_with("8.0.") || version.starts_with("8.4."))
        || lowercase.contains("mariadb")
        || lowercase.contains("percona")
        || !lowercase.contains("mysql")
    {
        anyhow::bail!(
            "legacy source engine must be Oracle MySQL 8.0.x or 8.4.x, observed {version} ({comment})"
        );
    }
    Ok(())
}

async fn verify_mysql_source_principal(
    source: &mut MySqlConnection,
    database_url: &str,
) -> anyhow::Result<()> {
    let url = Url::parse(database_url)?;
    let expected_username = percent_decode_str(url.username()).decode_utf8()?;
    let expected_database = percent_decode_str(url.path().trim_start_matches('/')).decode_utf8()?;
    let (current_user, current_database): (String, Option<String>) =
        sqlx::query_as("SELECT CURRENT_USER(), DATABASE()")
            .fetch_one(&mut *source)
            .await?;
    let authenticated_username = current_user
        .rsplit_once('@')
        .map(|(username, _)| username)
        .ok_or_else(|| anyhow::anyhow!("legacy MySQL returned an invalid CURRENT_USER identity"))?;
    if authenticated_username != expected_username
        || current_database.as_deref() != Some(&expected_database)
    {
        anyhow::bail!(
            "legacy MySQL authenticated identity or selected database differs from source.database_url"
        );
    }

    let enabled_roles = sqlx::query_as::<_, (String, String)>(
        "SELECT role_name, role_host FROM information_schema.enabled_roles",
    )
    .fetch_all(&mut *source)
    .await?;
    if !enabled_roles.is_empty() {
        anyhow::bail!("legacy MySQL source account must not have any enabled role");
    }

    let grant_rows = sqlx::query("SHOW GRANTS FOR CURRENT_USER")
        .fetch_all(&mut *source)
        .await?;
    let mut grants = Vec::with_capacity(grant_rows.len());
    for row in grant_rows {
        grants.push(row.try_get::<String, _>(0)?);
    }
    validate_mysql_source_grants(&grants, &expected_database)
}

fn validate_mysql_source_grants(grants: &[String], database: &str) -> anyhow::Result<()> {
    let usage_prefix = "GRANT USAGE ON *.* TO ";
    let select_prefix = format!("GRANT SELECT ON `{database}`.* TO ");
    let mut saw_usage = false;
    let mut saw_select = false;
    for grant in grants {
        let recipient = if let Some(recipient) = grant.strip_prefix(usage_prefix) {
            saw_usage = true;
            recipient
        } else if let Some(recipient) = grant.strip_prefix(&select_prefix) {
            saw_select = true;
            recipient
        } else {
            anyhow::bail!(
                "legacy MySQL source account must have only USAGE and database-level SELECT"
            );
        };
        if recipient.is_empty() || recipient.contains(" WITH ") {
            anyhow::bail!(
                "legacy MySQL source account must not have GRANT OPTION or account resource grants"
            );
        }
    }
    if !saw_usage || !saw_select {
        anyhow::bail!(
            "legacy MySQL source account must have exactly database-level SELECT plus implicit USAGE"
        );
    }
    Ok(())
}

async fn begin_mysql_read_snapshot(source: &mut MySqlConnection) -> anyhow::Result<()> {
    (&mut *source)
        .execute("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .await?;
    (&mut *source)
        .execute("SET SESSION TRANSACTION_READ_ONLY = 1")
        .await?;
    (&mut *source)
        .execute("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await?;
    let (isolation, read_only): (String, i64) =
        sqlx::query_as("SELECT @@transaction_isolation, @@transaction_read_only")
            .fetch_one(&mut *source)
            .await?;
    if !isolation.eq_ignore_ascii_case("REPEATABLE-READ") || read_only != 1 {
        anyhow::bail!("legacy MySQL did not enter the required read-only repeatable-read snapshot");
    }
    Ok(())
}

async fn commit_mysql_read_snapshot(source: &mut MySqlConnection) -> anyhow::Result<()> {
    (&mut *source).execute("COMMIT").await?;
    Ok(())
}

async fn inspect_source_schema(
    source: &mut MySqlConnection,
) -> anyhow::Result<SourceSchemaInspection> {
    let actual_tables = sqlx::query_scalar::<_, String>(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' ORDER BY table_name",
    )
    .fetch_all(&mut *source)
    .await?
    .into_iter()
    .collect::<BTreeSet<_>>();
    let imported_tables = validate_source_table_inventory(&actual_tables)?;
    let mut engines_query = QueryBuilder::<MySql>::new(
        "SELECT table_name, COALESCE(engine, '') FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' AND table_name IN (",
    );
    {
        let mut separated = engines_query.separated(", ");
        for table in &imported_tables {
            separated.push_bind(table);
        }
    }
    engines_query.push(") ORDER BY table_name");
    let imported_engines = engines_query
        .build_query_as::<(String, String)>()
        .fetch_all(&mut *source)
        .await?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    validate_source_table_engines(&imported_tables, &imported_engines)?;

    for mapping in TABLE_MAPPINGS {
        let actual = sqlx::query_scalar::<_, String>(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ordinal_position",
        )
        .bind(mapping.source)
        .fetch_all(&mut *source)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
        let expected = mapping_source_columns(mapping)
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        if actual != expected {
            anyhow::bail!(
                "legacy source columns drifted for {}: expected={expected:?}, observed={actual:?}",
                mapping.source
            );
        }
    }

    type ColumnDescriptor = (
        String,
        u32,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    );
    let mut columns_query = QueryBuilder::<MySql>::new(
        "SELECT table_name, ordinal_position, column_name, column_type, is_nullable, \
                COALESCE(CAST(column_default AS CHAR), '<NULL>'), extra, \
                COALESCE(character_set_name, ''), COALESCE(collation_name, ''), column_key \
         FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name IN (",
    );
    {
        let mut separated = columns_query.separated(", ");
        for table in &imported_tables {
            separated.push_bind(table);
        }
    }
    columns_query.push(") ORDER BY table_name, ordinal_position");
    let columns = columns_query
        .build_query_as::<ColumnDescriptor>()
        .fetch_all(&mut *source)
        .await?;
    type IndexDescriptor = (String, String, i32, u32, String, i64, String);
    let mut indexes_query = QueryBuilder::<MySql>::new(
        "SELECT table_name, index_name, non_unique, seq_in_index, column_name, \
                COALESCE(sub_part, 0), index_type \
         FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name IN (",
    );
    {
        let mut separated = indexes_query.separated(", ");
        for table in &imported_tables {
            separated.push_bind(table);
        }
    }
    indexes_query.push(") ORDER BY table_name, index_name, seq_in_index");
    let indexes = indexes_query
        .build_query_as::<IndexDescriptor>()
        .fetch_all(&mut *source)
        .await?;
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.source-schema.v1\0");
    digest.update(b"tables\0");
    for table in &imported_tables {
        schema_digest_fields(&mut digest, [table.clone(), "InnoDB".to_string()]);
    }
    digest.update(b"columns\0");
    for row in columns {
        schema_digest_fields(
            &mut digest,
            [
                row.0,
                row.1.to_string(),
                row.2,
                row.3,
                row.4,
                row.5,
                row.6,
                row.7,
                row.8,
                row.9,
            ],
        );
    }
    digest.update(b"indexes\0");
    for row in indexes {
        schema_digest_fields(
            &mut digest,
            [
                row.0,
                row.1,
                row.2.to_string(),
                row.3.to_string(),
                row.4,
                row.5.to_string(),
                row.6,
            ],
        );
    }
    let discarded_tables = DISCARDED_SOURCE_TABLES
        .iter()
        .map(|table| DiscardedTableReport {
            source: (*table).to_string(),
            present: actual_tables.contains(*table),
            rows_scanned: false,
            policy: "allowlisted_full_table_discard_without_row_scan",
        })
        .collect();
    Ok(SourceSchemaInspection {
        imported_schema_sha256: hex::encode(digest.finalize()),
        discarded_tables,
    })
}

fn validate_source_table_inventory(
    actual_tables: &BTreeSet<String>,
) -> anyhow::Result<BTreeSet<String>> {
    let imported_tables = TABLE_MAPPINGS
        .iter()
        .map(|mapping| mapping.source.to_string())
        .collect::<BTreeSet<_>>();
    let allowed_tables = imported_tables
        .iter()
        .cloned()
        .chain(
            DISCARDED_SOURCE_TABLES
                .iter()
                .map(|table| (*table).to_string()),
        )
        .collect::<BTreeSet<_>>();
    let missing = imported_tables
        .difference(actual_tables)
        .cloned()
        .collect::<Vec<_>>();
    let unexpected = actual_tables
        .difference(&allowed_tables)
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() || !unexpected.is_empty() {
        anyhow::bail!(
            "legacy source table inventory is unsupported; missing imported tables={missing:?}, unexpected tables={unexpected:?}"
        );
    }
    Ok(imported_tables)
}

fn validate_source_table_engines(
    imported_tables: &BTreeSet<String>,
    imported_engines: &BTreeMap<String, String>,
) -> anyhow::Result<()> {
    if imported_engines.keys().collect::<BTreeSet<_>>()
        != imported_tables.iter().collect::<BTreeSet<_>>()
    {
        anyhow::bail!("legacy source storage-engine inventory is incomplete");
    }
    for (table, engine) in imported_engines {
        if !engine.eq_ignore_ascii_case("InnoDB") {
            anyhow::bail!(
                "legacy source imported table {table} must use InnoDB for the consistent snapshot"
            );
        }
    }
    Ok(())
}

fn schema_digest_fields<const N: usize>(digest: &mut Sha256, fields: [String; N]) {
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
}

fn mapping_source_columns(mapping: &TableMapping) -> Vec<&str> {
    mapping
        .direct_columns
        .iter()
        .copied()
        .chain(
            mapping
                .transformed_columns
                .iter()
                .map(|column| column.source),
        )
        .chain(mapping.deferred_columns.iter().map(|column| column.source))
        .chain(
            mapping
                .consumed_source_columns
                .iter()
                .map(|column| column.source),
        )
        .collect()
}

async fn validate_source_relationships(source: &mut MySqlConnection) -> anyhow::Result<()> {
    for mapping in TABLE_MAPPINGS {
        let sql = format!("SELECT COUNT(*) FROM `{}` WHERE `id` <= 0", mapping.source);
        let invalid: i64 = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
            .fetch_one(&mut *source)
            .await?;
        if invalid != 0 {
            anyhow::bail!(
                "legacy source table {} has {invalid} non-positive business identity row(s); native identities must be positive",
                mapping.source
            );
        }
    }

    for reference in SCALAR_REFERENCES {
        let predicate = match reference.rule {
            ScalarReferenceRule::Required => format!(
                "s.`{column}` IS NULL OR r.`id` IS NULL",
                column = reference.column
            ),
            ScalarReferenceRule::Nullable => format!(
                "s.`{column}` IS NOT NULL AND r.`id` IS NULL",
                column = reference.column
            ),
            ScalarReferenceRule::ZeroMeansNoReference => format!(
                "s.`{column}` <> 0 AND r.`id` IS NULL",
                column = reference.column
            ),
        };
        let sql = format!(
            "SELECT COUNT(*) FROM `{source_table}` AS s \
             LEFT JOIN `{referenced_table}` AS r ON r.`id` = s.`{column}` WHERE {predicate}",
            source_table = reference.source_table,
            referenced_table = reference.source_referenced_table,
            column = reference.column,
        );
        let invalid: i64 = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
            .fetch_one(&mut *source)
            .await?;
        if invalid != 0 {
            anyhow::bail!(
                "legacy source relationship {}.{} -> {} has {invalid} invalid row(s)",
                reference.source_table,
                reference.column,
                reference.source_referenced_table
            );
        }
    }
    Ok(())
}

async fn copy_business_data(
    source: &mut MySqlConnection,
    target: &PgPool,
) -> anyhow::Result<Vec<ImportedTableReport>> {
    let payments =
        sqlx::query_as::<_, (i32, String)>("SELECT id, payment FROM v2_payment ORDER BY id")
            .fetch_all(&mut *source)
            .await?;
    let known_payment_ids = payments.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
    let stripe_payment_ids = payments
        .iter()
        .filter(|(_, driver)| is_legacy_stripe_payment_driver(driver))
        .map(|(id, _)| *id)
        .collect::<BTreeSet<_>>();

    let mut reports = Vec::with_capacity(TABLE_MAPPINGS.len() + 1);
    for mapping in copied_table_mappings() {
        reports.push(
            copy_base_table(
                &mut *source,
                target,
                mapping,
                &known_payment_ids,
                &stripe_payment_ids,
            )
            .await?,
        );
    }
    let inviter_sha256 = apply_deferred_user_inviters(&mut *source, target).await?;
    let users = reports
        .iter_mut()
        .find(|report| report.target == "users")
        .ok_or_else(|| anyhow::anyhow!("converter registry omitted the users report"))?;
    users.retained_sha256 = finalize_users_retained_sha256(&users.retained_sha256, &inviter_sha256);
    reports.push(build_giftcard_redemptions(&mut *source, target).await?);
    validate_transformed_references(target).await?;

    for mapping in TABLE_MAPPINGS {
        ensure_sequence_headroom(target, mapping).await?;
    }
    for mapping in TABLE_MAPPINGS {
        execute_dynamic(target, sequence_reset_sql(mapping)?).await?;
    }
    for table in discarded_target_tables() {
        let sql = format!("SELECT COUNT(*) FROM {}", postgres_identifier(table));
        let count: i64 = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
            .fetch_one(target)
            .await?;
        if count != 0 {
            anyhow::bail!("fixed-discard target {table} contains {count} row(s)");
        }
    }
    Ok(reports)
}

async fn ensure_sequence_headroom(target: &PgPool, mapping: &TableMapping) -> anyhow::Result<()> {
    let sql = format!(
        "SELECT MAX(id)::bigint FROM {}",
        postgres_identifier(mapping.target)
    );
    let maximum: Option<i64> = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
        .fetch_one(target)
        .await?;
    validate_identity_headroom(mapping, maximum)
}

fn validate_identity_headroom(mapping: &TableMapping, maximum: Option<i64>) -> anyhow::Result<()> {
    let exhausted = match (mapping.identity_width, maximum) {
        (_, None) => false,
        (IdentityWidth::I32, Some(maximum)) => maximum >= i64::from(i32::MAX),
        (IdentityWidth::I64, Some(maximum)) => maximum == i64::MAX,
    };
    if exhausted {
        anyhow::bail!(
            "target identity sequence for {} is exhausted at imported maximum id {}; refuse to complete an installation that cannot allocate the next id",
            mapping.target,
            maximum.expect("exhausted identities have a maximum")
        );
    }
    Ok(())
}

async fn copy_base_table(
    source: &mut MySqlConnection,
    target: &PgPool,
    mapping: &TableMapping,
    known_payment_ids: &BTreeSet<i32>,
    stripe_payment_ids: &BTreeSet<i32>,
) -> anyhow::Result<ImportedTableReport> {
    let sql = source_batch_sql(mapping)?;
    let mut cursor = INITIAL_SOURCE_ID_CURSOR;
    let mut source_rows = 0_u64;
    let mut retained_rows = 0_u64;
    let mut discarded_rows = 0_u64;
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.retained-table.v1\0");
    digest.update(mapping.target.as_bytes());

    loop {
        let rows = sqlx::query(AssertSqlSafe(sql.clone()).into_sql_str())
            .bind(cursor)
            .bind(DEFAULT_BATCH_SIZE)
            .fetch_all(&mut *source)
            .await?;
        if rows.is_empty() {
            break;
        }
        let source_batch = rows
            .iter()
            .map(|row| decode_mysql_row(mapping, row))
            .collect::<Result<Vec<_>, _>>()?;
        cursor = validate_batch_ids(mapping, cursor, &source_batch, DEFAULT_BATCH_SIZE)?;
        source_rows += source_batch.len() as u64;

        let mut retained = Vec::with_capacity(source_batch.len());
        for row in &source_batch {
            match transform_mysql_import_row(mapping, row, known_payment_ids, stripe_payment_ids)? {
                MysqlImportRowDisposition::Discard => discarded_rows += 1,
                MysqlImportRowDisposition::Retain(row) => retained.push(row),
            }
        }
        if !retained.is_empty() {
            insert_canonical_rows(target, mapping, &retained).await?;
            verify_inserted_rows(target, mapping, &retained).await?;
            let batch_hash = canonical_rows_sha256(mapping, &retained)?;
            digest.update((retained.len() as u64).to_be_bytes());
            digest.update(batch_hash.as_bytes());
            retained_rows += retained.len() as u64;
        }
    }
    let target_sql = format!(
        "SELECT COUNT(*) FROM {}",
        postgres_identifier(mapping.target)
    );
    let target_count: i64 = sqlx::query_scalar(AssertSqlSafe(target_sql).into_sql_str())
        .fetch_one(target)
        .await?;
    if u64::try_from(target_count).ok() != Some(retained_rows) {
        anyhow::bail!(
            "target row count mismatch for {}: expected {retained_rows}, observed {target_count}",
            mapping.target
        );
    }
    Ok(ImportedTableReport {
        source: mapping.source.to_string(),
        target: mapping.target.to_string(),
        source_rows,
        retained_rows,
        discarded_rows,
        retained_sha256: hex::encode(digest.finalize()),
    })
}

fn decode_mysql_row(mapping: &TableMapping, row: &MySqlRow) -> anyhow::Result<SourceRow> {
    let mut decoded = BTreeMap::new();
    let expected = mapping_source_columns(mapping)
        .into_iter()
        .collect::<BTreeSet<_>>();
    for (index, column) in row.columns().iter().enumerate() {
        if !expected.contains(column.name()) {
            anyhow::bail!(
                "source query for {} returned unexpected column {}",
                mapping.source,
                column.name()
            );
        }
        decoded.insert(column.name().to_string(), decode_mysql_value(row, index)?);
    }
    Ok(decoded)
}

fn decode_mysql_value(row: &MySqlRow, index: usize) -> anyhow::Result<SourceValue> {
    if row.try_get_raw(index)?.is_null() {
        return Ok(SourceValue::Null);
    }
    let type_name = row.column(index).type_info().name().to_ascii_uppercase();
    if type_name.contains("INT") || type_name == "YEAR" || type_name == "BOOLEAN" {
        if type_name.contains("UNSIGNED") {
            if let Ok(value) = row.try_get::<u64, _>(index) {
                return Ok(SourceValue::U64(value));
            }
            if let Ok(value) = row.try_get::<u32, _>(index) {
                return Ok(SourceValue::U64(u64::from(value)));
            }
            if let Ok(value) = row.try_get::<u16, _>(index) {
                return Ok(SourceValue::U64(u64::from(value)));
            }
            if let Ok(value) = row.try_get::<u8, _>(index) {
                return Ok(SourceValue::U64(u64::from(value)));
            }
        } else {
            if let Ok(value) = row.try_get::<i64, _>(index) {
                return Ok(SourceValue::I64(value));
            }
            if let Ok(value) = row.try_get::<i32, _>(index) {
                return Ok(SourceValue::I64(i64::from(value)));
            }
            if let Ok(value) = row.try_get::<i16, _>(index) {
                return Ok(SourceValue::I64(i64::from(value)));
            }
            if let Ok(value) = row.try_get::<i8, _>(index) {
                return Ok(SourceValue::I64(i64::from(value)));
            }
        }
    }
    if type_name.contains("DECIMAL") || type_name.contains("NUMERIC") {
        return Ok(SourceValue::Decimal(
            row.try_get::<Decimal, _>(index)?.normalize().to_string(),
        ));
    }
    if type_name.contains("BLOB") || type_name.contains("BINARY") || type_name == "BIT" {
        return Ok(SourceValue::Bytes(row.try_get(index)?));
    }
    if let Ok(value) = row.try_get::<String, _>(index) {
        return Ok(SourceValue::Text(value));
    }
    if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
        return Ok(SourceValue::Bytes(value));
    }
    anyhow::bail!(
        "unsupported legacy MySQL type {} at column {}",
        type_name,
        row.column(index).name()
    )
}

fn target_columns(mapping: &TableMapping) -> Vec<&str> {
    mapping
        .direct_columns
        .iter()
        .copied()
        .chain(
            mapping
                .transformed_columns
                .iter()
                .map(|column| column.target),
        )
        .chain(mapping.added_columns.iter().map(|column| column.target))
        .collect()
}

async fn insert_canonical_rows(
    target: &PgPool,
    mapping: &TableMapping,
    rows: &[CanonicalRow],
) -> anyhow::Result<()> {
    // Retain the converter's parameter-limit audit even though QueryBuilder
    // emits literal NULLs rather than assigning them an incorrect bind type.
    target_batch_insert_sql(mapping, rows.len())?;
    let columns = target_columns(mapping);
    let mut prepared = Vec::with_capacity(rows.len());
    for row in rows {
        prepared.push(
            columns
                .iter()
                .map(|column| prepare_target_value(row.get(*column).expect("audited row")))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }

    let mut builder = QueryBuilder::<Postgres>::new(format!(
        "INSERT INTO {} (",
        postgres_identifier(mapping.target)
    ));
    {
        let mut separated = builder.separated(", ");
        for column in &columns {
            separated.push(postgres_identifier(column));
        }
    }
    builder.push(") ");
    builder.push_values(&prepared, |mut values, row| {
        for value in row {
            match value {
                PreparedTargetValue::Null => {
                    values.push("NULL");
                }
                PreparedTargetValue::Integer(value) => {
                    values.push_bind(*value);
                }
                PreparedTargetValue::Decimal(value) => {
                    values.push_bind(*value);
                }
                PreparedTargetValue::Text(value) => {
                    values.push_bind(value);
                }
                PreparedTargetValue::Bytes(value) => {
                    values.push_bind(value);
                }
                PreparedTargetValue::Json(value) => {
                    values.push_bind(Json(value));
                }
            }
        }
    });
    builder.build().execute(target).await?;
    Ok(())
}

enum PreparedTargetValue {
    Null,
    Integer(i64),
    Decimal(Decimal),
    Text(String),
    Bytes(Vec<u8>),
    Json(Value),
}

fn prepare_target_value(value: &CanonicalValue) -> anyhow::Result<PreparedTargetValue> {
    Ok(match value {
        CanonicalValue::Null => PreparedTargetValue::Null,
        CanonicalValue::I64(value) => PreparedTargetValue::Integer(*value),
        CanonicalValue::U64(value) => PreparedTargetValue::Integer(i64::try_from(*value)?),
        CanonicalValue::Decimal(value) => PreparedTargetValue::Decimal(Decimal::from_str(value)?),
        CanonicalValue::Text(value) => PreparedTargetValue::Text(value.clone()),
        CanonicalValue::Bytes(value) => PreparedTargetValue::Bytes(value.clone()),
        CanonicalValue::Json(value) => PreparedTargetValue::Json(value.clone()),
    })
}

async fn verify_inserted_rows(
    target: &PgPool,
    mapping: &TableMapping,
    expected: &[CanonicalRow],
) -> anyhow::Result<()> {
    let columns = target_columns(mapping);
    let selected = columns
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let ids = expected
        .iter()
        .map(canonical_id)
        .collect::<Result<Vec<_>, _>>()?;
    let sql = format!(
        "SELECT {selected} FROM {} WHERE id::bigint = ANY($1) ORDER BY id",
        postgres_identifier(mapping.target)
    );
    let rows = sqlx::query(AssertSqlSafe(sql).into_sql_str())
        .bind(&ids)
        .fetch_all(target)
        .await?;
    if rows.len() != expected.len() {
        anyhow::bail!(
            "target batch verification count mismatch for {}",
            mapping.target
        );
    }
    for (row, expected) in rows.iter().zip(expected) {
        for (index, column) in columns.iter().enumerate() {
            verify_postgres_value(row, index, expected.get(*column).expect("audited row"))
                .map_err(|error| {
                    anyhow::anyhow!(
                        "target value mismatch for {}.{}: {error}",
                        mapping.target,
                        column
                    )
                })?;
        }
    }
    Ok(())
}

fn canonical_id(row: &CanonicalRow) -> anyhow::Result<i64> {
    match row.get("id") {
        Some(CanonicalValue::I64(value)) => Ok(*value),
        Some(CanonicalValue::U64(value)) => Ok(i64::try_from(*value)?),
        _ => anyhow::bail!("canonical row has no signed-compatible id"),
    }
}

fn verify_postgres_value(
    row: &sqlx::postgres::PgRow,
    index: usize,
    expected: &CanonicalValue,
) -> anyhow::Result<()> {
    if matches!(expected, CanonicalValue::Null) {
        if !row.try_get_raw(index)?.is_null() {
            anyhow::bail!("expected NULL");
        }
        return Ok(());
    }
    let matches = match expected {
        CanonicalValue::Null => unreachable!(),
        CanonicalValue::I64(expected) => postgres_integer(row, index)? == *expected,
        CanonicalValue::U64(expected) => {
            u64::try_from(postgres_integer(row, index)?).ok() == Some(*expected)
        }
        CanonicalValue::Decimal(expected) => {
            row.try_get::<Decimal, _>(index)?.normalize().to_string() == *expected
        }
        CanonicalValue::Text(expected) => row.try_get::<String, _>(index)? == *expected,
        CanonicalValue::Bytes(expected) => row.try_get::<Vec<u8>, _>(index)? == *expected,
        CanonicalValue::Json(expected) => row.try_get::<Json<Value>, _>(index)?.0 == *expected,
    };
    if !matches {
        anyhow::bail!("value differs");
    }
    Ok(())
}

fn postgres_integer(row: &sqlx::postgres::PgRow, index: usize) -> anyhow::Result<i64> {
    if let Ok(value) = row.try_get::<i64, _>(index) {
        return Ok(value);
    }
    if let Ok(value) = row.try_get::<i32, _>(index) {
        return Ok(i64::from(value));
    }
    if let Ok(value) = row.try_get::<i16, _>(index) {
        return Ok(i64::from(value));
    }
    anyhow::bail!("target integer has an unsupported PostgreSQL type")
}

async fn apply_deferred_user_inviters(
    source: &mut MySqlConnection,
    target: &PgPool,
) -> anyhow::Result<String> {
    let mut cursor = INITIAL_SOURCE_ID_CURSOR;
    let mut expected_non_null = 0_i64;
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.user-inviters.v1\0");
    loop {
        let rows = sqlx::query_as::<_, (i32, Option<i32>)>(
            "SELECT id, invite_user_id FROM v2_user WHERE id > ? ORDER BY id LIMIT ?",
        )
        .bind(cursor)
        .bind(DEFAULT_BATCH_SIZE)
        .fetch_all(&mut *source)
        .await?;
        if rows.is_empty() {
            break;
        }
        cursor = i64::from(rows.last().expect("non-empty batch").0);
        let expected = rows
            .iter()
            .map(|(id, inviter)| (i64::from(*id), inviter.map(i64::from)))
            .collect::<Vec<_>>();
        let non_null = expected
            .iter()
            .filter_map(|(id, inviter)| inviter.map(|inviter| (*id, inviter)))
            .collect::<Vec<_>>();
        expected_non_null += non_null.len() as i64;
        if !non_null.is_empty() {
            let mut builder = QueryBuilder::<Postgres>::new(
                "UPDATE users AS u SET invite_user_id = v.invite_user_id FROM (",
            );
            builder.push_values(&non_null, |mut values, (id, inviter)| {
                values.push_bind(*id).push_bind(*inviter);
            });
            builder.push(") AS v(id, invite_user_id) WHERE u.id = v.id");
            let changed = builder.build().execute(target).await?.rows_affected();
            if changed != non_null.len() as u64 {
                anyhow::bail!("deferred user inviter update did not match every source user");
            }
        }
        let ids = expected.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let observed = sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT id, invite_user_id FROM users WHERE id = ANY($1) ORDER BY id",
        )
        .bind(&ids)
        .fetch_all(target)
        .await?;
        if observed != expected {
            anyhow::bail!("deferred user inviter values differ from the source snapshot");
        }
        for (user_id, invite_user_id) in expected {
            digest_user_inviter_row(&mut digest, user_id, invite_user_id);
        }
    }
    let actual_non_null: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE invite_user_id IS NOT NULL")
            .fetch_one(target)
            .await?;
    if actual_non_null != expected_non_null {
        anyhow::bail!(
            "deferred user inviter count mismatch: expected {expected_non_null}, observed {actual_non_null}"
        );
    }
    Ok(hex::encode(digest.finalize()))
}

async fn build_giftcard_redemptions(
    source: &mut MySqlConnection,
    target: &PgPool,
) -> anyhow::Result<ImportedTableReport> {
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.giftcard-redemptions.v1\0");
    let mut cursor = INITIAL_SOURCE_ID_CURSOR;
    let mut source_rows = 0_u64;
    let mut retained_rows = 0_u64;
    loop {
        let giftcards = sqlx::query_as::<_, (i32, Option<String>)>(
            "SELECT id, used_user_ids FROM v2_giftcard \
             WHERE id > ? ORDER BY id LIMIT ?",
        )
        .bind(cursor)
        .bind(DEFAULT_BATCH_SIZE)
        .fetch_all(&mut *source)
        .await?;
        if giftcards.is_empty() {
            break;
        }
        cursor = i64::from(giftcards.last().expect("non-empty batch").0);
        source_rows = source_rows
            .checked_add(giftcards.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("gift-card source row count overflow"))?;
        for (giftcard_id, used_user_ids) in giftcards {
            let source_value = used_user_ids
                .map(SourceValue::Text)
                .unwrap_or(SourceValue::Null);
            let redemptions = expand_giftcard_redemptions(giftcard_id, &source_value)?;
            validate_giftcard_redemption_users(target, &redemptions).await?;
            for row in &redemptions {
                digest_giftcard_redemption(&mut digest, row);
            }
            for chunk in redemptions.chunks(MAX_GIFTCARD_REDEMPTIONS_PER_INSERT) {
                insert_giftcard_redemptions(target, chunk).await?;
            }
            retained_rows = retained_rows
                .checked_add(redemptions.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("gift-card redemption row count overflow"))?;
        }
    }
    let actual: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gift_card_redemption")
        .fetch_one(target)
        .await?;
    if u64::try_from(actual).ok() != Some(retained_rows) {
        anyhow::bail!("gift-card redemption target count mismatch");
    }
    Ok(ImportedTableReport {
        source: "v2_giftcard.used_user_ids".to_string(),
        target: "gift_card_redemption".to_string(),
        source_rows,
        retained_rows,
        discarded_rows: 0,
        retained_sha256: hex::encode(digest.finalize()),
    })
}

async fn validate_giftcard_redemption_users(
    target: &PgPool,
    rows: &[LegacyGiftcardRedemptionRow],
) -> anyhow::Result<()> {
    let expected = rows.iter().map(|row| row.user_id).collect::<BTreeSet<_>>();
    let ids = expected.iter().copied().collect::<Vec<_>>();
    let mut observed = BTreeSet::new();
    for chunk in ids.chunks(MAX_GIFTCARD_REFERENCE_IDS_PER_QUERY) {
        observed.extend(
            sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE id = ANY($1)")
                .bind(chunk)
                .fetch_all(target)
                .await?,
        );
    }
    if observed != expected {
        let missing = expected
            .difference(&observed)
            .next()
            .copied()
            .unwrap_or_default();
        anyhow::bail!("v2_giftcard.used_user_ids references missing v2_user id {missing}");
    }
    Ok(())
}

async fn insert_giftcard_redemptions(
    target: &PgPool,
    rows: &[LegacyGiftcardRedemptionRow],
) -> anyhow::Result<()> {
    if rows.is_empty() || rows.len() > MAX_GIFTCARD_REDEMPTIONS_PER_INSERT {
        anyhow::bail!("invalid gift-card redemption insert batch size");
    }
    let mut builder = QueryBuilder::<Postgres>::new(
        "INSERT INTO gift_card_redemption \
         (giftcard_id, user_id, created_at, created_at_provenance) ",
    );
    builder.push_values(rows, |mut values, row| {
        values
            .push_bind(row.giftcard_id)
            .push_bind(row.user_id)
            .push_bind(row.created_at)
            .push_bind(&row.created_at_provenance);
    });
    let inserted = builder.build().execute(target).await?.rows_affected();
    if inserted != rows.len() as u64 {
        anyhow::bail!("gift-card redemption insert did not write every derived row");
    }
    Ok(())
}

fn digest_giftcard_redemption(digest: &mut Sha256, row: &LegacyGiftcardRedemptionRow) {
    for field in [
        row.giftcard_id.to_string(),
        row.user_id.to_string(),
        row.created_at.to_string(),
        row.created_at_provenance.clone(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
}

async fn validate_transformed_references(target: &PgPool) -> anyhow::Result<()> {
    let missing_coupon_plans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM coupon AS c \
         CROSS JOIN LATERAL jsonb_array_elements_text(c.limit_plan_ids) AS item(plan_id) \
         LEFT JOIN plan AS p ON p.id = item.plan_id::integer \
         WHERE c.limit_plan_ids IS NOT NULL AND p.id IS NULL",
    )
    .fetch_one(target)
    .await?;
    let missing_surplus_orders: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orders AS o \
         CROSS JOIN LATERAL jsonb_array_elements_text(o.surplus_order_ids::jsonb) AS item(order_id) \
         LEFT JOIN orders AS referenced ON referenced.id = item.order_id::bigint \
         WHERE o.surplus_order_ids IS NOT NULL AND referenced.id IS NULL",
    )
    .fetch_one(target)
    .await?;
    if missing_coupon_plans != 0 || missing_surplus_orders != 0 {
        anyhow::bail!(
            "transformed array references are incomplete: coupon_plans={missing_coupon_plans}, surplus_orders={missing_surplus_orders}"
        );
    }
    Ok(())
}

async fn install_admission(
    target: &PgPool,
    installation_id: Uuid,
    now: i64,
    plan: &MysqlImportExecutionPlan,
) -> anyhow::Result<()> {
    let source = &plan.analytics_admission;
    let policy = AnalyticsAdmissionPolicy {
        recovery_pending_rows: source.recovery_pending_rows,
        soft_pending_rows: source.soft_pending_rows,
        hard_pending_rows: source.hard_pending_rows,
        recovery_relation_bytes: source.recovery_relation_bytes,
        soft_relation_bytes: source.soft_relation_bytes,
        hard_relation_bytes: source.hard_relation_bytes,
        recovery_oldest_age_seconds: source.recovery_oldest_age_seconds,
        soft_oldest_age_seconds: source.soft_oldest_age_seconds,
        hard_oldest_age_seconds: source.hard_oldest_age_seconds,
        database_capacity_bytes: source.database_capacity_bytes,
        hard_min_headroom_bytes: source.hard_min_headroom_bytes,
        soft_min_headroom_bytes: source.soft_min_headroom_bytes,
        recovery_min_headroom_bytes: source.recovery_min_headroom_bytes,
        event_reservation_bytes: source.event_reservation_bytes,
        soft_max_new_rows_per_second: source.soft_max_new_rows_per_second,
        sample_interval_seconds: source.sample_interval_seconds,
        stale_after_seconds: source.stale_after_seconds,
        capacity_evidence: source.capacity_evidence.clone(),
    };
    install_analytics_admission_policy(target, installation_id, &policy, now).await?;
    Ok(())
}

async fn preflight_redis_target(redis_url: &str) -> anyhow::Result<()> {
    verify_empty_redis(redis_url).await?;
    verify_redis_server_version(redis_url).await?;

    let bootstrap_username = redis_url_username(redis_url)?;
    anyhow::ensure!(
        !bootstrap_username.is_empty() && bootstrap_username != "default",
        "target Redis bootstrap URL must name a non-default ACL user"
    );
    let client = redis::Client::open(redis_url)?;
    let mut connection = client.get_multiplexed_async_connection().await?;
    require_external_redis_aclfile(&mut connection).await?;
    verify_redis_acl_users(
        &mut connection,
        &BTreeSet::from(["default".to_string(), bootstrap_username.clone()]),
    )
    .await?;

    let acl: Vec<String> = redis::cmd("ACL")
        .arg("LIST")
        .query_async(&mut connection)
        .await?;
    let default = acl
        .iter()
        .find(|entry| entry.split_whitespace().take(2).eq(["user", "default"]))
        .ok_or_else(|| anyhow::anyhow!("target Redis ACL has no default user"))?;
    let default_tokens = default.split_whitespace().collect::<BTreeSet<_>>();
    anyhow::ensure!(
        default_tokens.contains("off")
            && !default_tokens.contains("on")
            && !default_tokens.contains("nopass"),
        "target Redis default user must be disabled and must not be passwordless"
    );

    let mut unauthenticated = Url::parse(redis_url)?;
    unauthenticated
        .set_username("")
        .map_err(|()| anyhow::anyhow!("could not clear Redis bootstrap username"))?;
    unauthenticated
        .set_password(None)
        .map_err(|()| anyhow::anyhow!("could not clear Redis bootstrap password"))?;
    let unauthenticated = redis::Client::open(unauthenticated.as_str())?;
    let unauthenticated_ping = async {
        let mut connection = unauthenticated.get_multiplexed_async_connection().await?;
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
    }
    .await;
    anyhow::ensure!(
        unauthenticated_ping.is_err(),
        "target Redis accepted an unauthenticated default-user connection"
    );
    Ok(())
}

async fn verify_redis_server_version(redis_url: &str) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut connection = client.get_multiplexed_async_connection().await?;
    let server: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut connection)
        .await?;
    let version = server
        .lines()
        .find_map(|line| {
            let (key, value) = line.trim_end_matches('\r').split_once(':')?;
            (key == "redis_version").then_some(value)
        })
        .ok_or_else(|| anyhow::anyhow!("target Redis did not report redis_version"))?;
    let mut components = version.split('.');
    let major = components
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    let minor = components
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    anyhow::ensure!(
        major == Some(REQUIRED_REDIS_MAJOR) && minor == Some(REQUIRED_REDIS_MINOR),
        "target Redis must be {}.{}, observed {version}",
        REQUIRED_REDIS_MAJOR,
        REQUIRED_REDIS_MINOR
    );
    Ok(())
}

async fn require_external_redis_aclfile(
    connection: &mut redis::aio::MultiplexedConnection,
) -> anyhow::Result<()> {
    let values: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("aclfile")
        .query_async(connection)
        .await?;
    let path = values
        .chunks_exact(2)
        .find_map(|pair| (pair[0] == "aclfile").then_some(pair[1].as_str()))
        .filter(|path| Path::new(path).is_absolute())
        .ok_or_else(|| {
            anyhow::anyhow!("target Redis must configure an absolute writable external aclfile")
        })?;
    anyhow::ensure!(!path.trim().is_empty());
    Ok(())
}

async fn verify_redis_acl_users(
    connection: &mut redis::aio::MultiplexedConnection,
    expected: &BTreeSet<String>,
) -> anyhow::Result<()> {
    let users = redis::cmd("ACL")
        .arg("USERS")
        .query_async::<Vec<String>>(connection)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        &users == expected,
        "target Redis ACL users differ from the dedicated-instance contract: expected {expected:?}, observed {users:?}"
    );
    Ok(())
}

fn redis_url_username(redis_url: &str) -> anyhow::Result<String> {
    let url = Url::parse(redis_url)?;
    Ok(percent_decode_str(url.username())
        .decode_utf8()
        .map_err(|_| anyhow::anyhow!("Redis URL username is not valid UTF-8"))?
        .into_owned())
}

fn generated_redis_password() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn redis_runtime_url(
    bootstrap_url: &str,
    username: &str,
    password: &str,
) -> anyhow::Result<String> {
    let mut url = Url::parse(bootstrap_url)?;
    url.set_username(username)
        .map_err(|()| anyhow::anyhow!("could not encode Redis runtime username"))?;
    url.set_password(Some(password))
        .map_err(|()| anyhow::anyhow!("could not encode Redis runtime password"))?;
    Ok(url.into())
}

async fn set_redis_acl_user(
    connection: &mut redis::aio::MultiplexedConnection,
    username: &str,
    password: &str,
    prefix: &str,
    read_write_patterns: &[&str],
    read_only_patterns: &[&str],
    commands: &[&str],
) -> anyhow::Result<()> {
    let mut command = redis::cmd("ACL");
    command
        .arg("SETUSER")
        .arg(username)
        .arg("reset")
        .arg("on")
        .arg(format!(">{password}"));
    for pattern in read_write_patterns {
        command.arg(format!("%RW~{prefix}{pattern}"));
    }
    for pattern in read_only_patterns {
        command.arg(format!("%R~{prefix}{pattern}"));
    }
    for permission in commands {
        command.arg(permission);
    }
    let response: String = command.query_async(connection).await?;
    anyhow::ensure!(response == "OK", "Redis ACL SETUSER did not return OK");
    Ok(())
}

async fn bootstrap_redis_runtime(
    bootstrap_url: &str,
    installation_id: Uuid,
) -> anyhow::Result<RedisRuntimeIdentity> {
    let bootstrap_username = redis_url_username(bootstrap_url)?;
    let api_username = format!("v2board_api_{}", Uuid::new_v4().simple());
    let worker_username = format!("v2board_worker_{}", Uuid::new_v4().simple());
    let api_password = generated_redis_password();
    let worker_password = generated_redis_password();
    let api_url = redis_runtime_url(bootstrap_url, &api_username, &api_password)?;
    let worker_url = redis_runtime_url(bootstrap_url, &worker_username, &worker_password)?;
    let prefix = format!("v2board:{installation_id}:");

    let bootstrap = redis::Client::open(bootstrap_url)?;
    let mut connection = bootstrap.get_multiplexed_async_connection().await?;
    require_external_redis_aclfile(&mut connection).await?;
    verify_redis_acl_users(
        &mut connection,
        &BTreeSet::from(["default".to_string(), bootstrap_username.clone()]),
    )
    .await?;
    set_redis_acl_user(
        &mut connection,
        &api_username,
        &api_password,
        &prefix,
        API_REDIS_RW_KEY_PATTERNS,
        API_REDIS_RO_KEY_PATTERNS,
        API_REDIS_COMMANDS,
    )
    .await?;
    set_redis_acl_user(
        &mut connection,
        &worker_username,
        &worker_password,
        &prefix,
        WORKER_REDIS_RW_KEY_PATTERNS,
        &[],
        WORKER_REDIS_COMMANDS,
    )
    .await?;
    let saved: String = redis::cmd("ACL")
        .arg("SAVE")
        .query_async(&mut connection)
        .await?;
    anyhow::ensure!(saved == "OK", "Redis ACL SAVE did not return OK");
    let loaded: String = redis::cmd("ACL")
        .arg("LOAD")
        .query_async(&mut connection)
        .await?;
    anyhow::ensure!(loaded == "OK", "Redis ACL LOAD did not return OK");
    drop(connection);

    let mut connection = bootstrap.get_multiplexed_async_connection().await?;
    let expected_users = BTreeSet::from([
        "default".to_string(),
        bootstrap_username,
        api_username.clone(),
        worker_username.clone(),
    ]);
    verify_redis_acl_users(&mut connection, &expected_users).await?;
    for username in [&api_username, &worker_username] {
        let _: redis::Value = redis::cmd("ACL")
            .arg("GETUSER")
            .arg(username)
            .query_async(&mut connection)
            .await?;
    }
    drop(connection);

    verify_redis_runtime_acl(&api_url, &worker_url, &prefix).await?;
    Ok(RedisRuntimeIdentity {
        api_url,
        worker_url,
    })
}

async fn verify_redis_runtime_acl(
    api_url: &str,
    worker_url: &str,
    prefix: &str,
) -> anyhow::Result<()> {
    let api = redis::Client::open(api_url)?;
    let worker = redis::Client::open(worker_url)?;
    v2board_domain::redis_runtime::verify_redis_runtime(
        &api,
        v2board_config::RuntimeEnvironment::Production,
    )
    .await?;
    v2board_domain::redis_runtime::verify_redis_runtime(
        &worker,
        v2board_config::RuntimeEnvironment::Production,
    )
    .await?;
    let mut api_connection = api.get_multiplexed_async_connection().await?;
    let mut worker_connection = worker.get_multiplexed_async_connection().await?;

    let api_key = format!("{prefix}AUTH_SESSION_acl_probe");
    let set: String = redis::cmd("SET")
        .arg(&api_key)
        .arg("probe")
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(set == "OK");
    let value: String = redis::cmd("GET")
        .arg(&api_key)
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(value == "probe");
    let deleted: i64 = redis::cmd("DEL")
        .arg(&api_key)
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(deleted == 1);

    let lock_key = format!("{prefix}RUST_SCHEDULER_LOCK_acl_probe");
    let set: String = redis::cmd("SET")
        .arg(&lock_key)
        .arg("lease")
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(set == "OK");
    let renewed: i64 = redis::Script::new(
        "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('EXPIRE', KEYS[1], 30) end return 0",
    )
    .key(&lock_key)
    .arg("lease")
    .invoke_async(&mut worker_connection)
    .await?;
    anyhow::ensure!(renewed == 1);
    let deleted: i64 = redis::cmd("DEL")
        .arg(&lock_key)
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(deleted == 1);

    let metric_key = format!("{prefix}RUST_WORKER_JOBS_TOTAL");
    let _: i64 = redis::cmd("HSET")
        .arg(&metric_key)
        .arg("acl_probe")
        .arg(1)
        .query_async(&mut worker_connection)
        .await?;
    let incremented: i64 = redis::cmd("HINCRBY")
        .arg(&metric_key)
        .arg("acl_probe")
        .arg(1)
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(incremented == 2);
    let metrics: BTreeMap<String, i64> = redis::cmd("HGETALL")
        .arg(&metric_key)
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(metrics.get("acl_probe") == Some(&2));
    anyhow::ensure!(
        redis::cmd("SET")
            .arg(&metric_key)
            .arg("forbidden")
            .query_async::<String>(&mut api_connection)
            .await
            .is_err()
    );
    anyhow::ensure!(
        redis::cmd("DEL")
            .arg(&metric_key)
            .query_async::<i64>(&mut api_connection)
            .await
            .is_err()
    );
    let removed: i64 = redis::cmd("HDEL")
        .arg(&metric_key)
        .arg("acl_probe")
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(removed == 1);

    let sensitive_keys = [
        "AUTH_SESSION_acl_probe",
        "USER_SESSIONS_1",
        "AUTH_USER_SESSION_KEYS_1",
        "TEMP_TOKEN_acl_probe",
        "AUTH_STEP_UP_acl_probe",
        "otp_acl_probe",
        "otpn_acl_probe",
        "totp_acl_probe",
    ];
    for logical_key in sensitive_keys {
        let key = format!("{prefix}{logical_key}");
        anyhow::ensure!(
            redis::cmd("SET")
                .arg(&key)
                .arg("forbidden")
                .query_async::<String>(&mut worker_connection)
                .await
                .is_err(),
            "worker Redis ACL allowed SET for {logical_key}"
        );
        anyhow::ensure!(
            redis::cmd("GET")
                .arg(&key)
                .query_async::<Option<String>>(&mut worker_connection)
                .await
                .is_err(),
            "worker Redis ACL allowed GET for {logical_key}"
        );
        anyhow::ensure!(
            redis::cmd("DEL")
                .arg(&key)
                .query_async::<i64>(&mut worker_connection)
                .await
                .is_err(),
            "worker Redis ACL allowed DEL for {logical_key}"
        );
    }
    let dynamic_auth_key = format!("{prefix}AUTH_SESSION_dynamic_lua_probe");
    anyhow::ensure!(
        redis::Script::new("return redis.call('SET', ARGV[1], 'forbidden')")
            .arg(&dynamic_auth_key)
            .invoke_async::<String>(&mut worker_connection)
            .await
            .is_err(),
        "worker Redis ACL allowed a zero-KEY Lua script to write an auth key"
    );

    for connection in [&mut api_connection, &mut worker_connection] {
        anyhow::ensure!(
            redis::cmd("CONFIG")
                .arg("GET")
                .arg("aclfile")
                .query_async::<Vec<String>>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("DBSIZE")
                .query_async::<u64>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("FLUSHDB")
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("FLUSHALL")
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("SELECT")
                .arg(1)
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("ACL")
                .arg("USERS")
                .query_async::<Vec<String>>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("SET")
                .arg("v2board:another-installation:acl_probe")
                .arg("forbidden")
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("EVAL")
                .arg("return 1")
                .arg(0)
                .query_async::<i64>(connection)
                .await
                .is_err()
        );
    }
    Ok(())
}

async fn verify_empty_redis(redis_url: &str) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    v2board_domain::redis_runtime::verify_redis_runtime(
        &client,
        v2board_config::RuntimeEnvironment::Production,
    )
    .await?;
    let mut connection = client.get_multiplexed_async_connection().await?;
    let keyspace: String = redis::cmd("INFO")
        .arg("keyspace")
        .query_async(&mut connection)
        .await?;
    let populated_databases = keyspace
        .lines()
        .filter(|line| line.starts_with("db") && line.contains("keys="))
        .collect::<Vec<_>>();
    if !populated_databases.is_empty() {
        anyhow::bail!(
            "target Redis instance is not completely empty: {}",
            populated_databases.join(", ")
        );
    }
    let size: u64 = redis::cmd("DBSIZE").query_async(&mut connection).await?;
    if size != 0 {
        anyhow::bail!("target Redis database 0 is not empty ({size} keys)");
    }
    Ok(())
}

#[derive(clickhouse::Row, serde::Deserialize)]
struct ClickHouseCount {
    value: u64,
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

async fn preflight_clickhouse_absent(plan: &MysqlImportExecutionPlan) -> anyhow::Result<()> {
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

fn clickhouse_major_minor(version: &str) -> Option<(u64, u64)> {
    let mut components = version.split('.');
    Some((
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
    ))
}

async fn bootstrap_clickhouse(
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

fn clickhouse_identifier(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use std::{
        os::unix::fs::{MetadataExt, PermissionsExt, symlink},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn test_directory(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "v2board-mysql-import-{}-{nonce}-{name}",
            std::process::id()
        ))
    }

    const EXECUTE_DATABASE: &str = "v2board_execute_test";
    const EXECUTE_MIGRATION_ROLE: &str = "v2board_execute_migration";
    const EXECUTE_API_ROLE: &str = "v2board_execute_api";
    const EXECUTE_WORKER_ROLE: &str = "v2board_execute_worker";
    const EXECUTE_CLICKHOUSE_DATABASE: &str = "v2board_execute_test";
    const EXECUTE_CLICKHOUSE_SCHEMA_ROLE: &str = "v2board_execute_schema";
    const EXECUTE_CLICKHOUSE_WRITER_ROLE: &str = "v2board_execute_writer";

    #[test]
    fn source_grants_allow_only_database_select_without_roles_or_grant_option() {
        let expected = vec![
            "GRANT USAGE ON *.* TO `legacy_reader`@`%`".to_string(),
            "GRANT SELECT ON `v2board_legacy`.* TO `legacy_reader`@`%`".to_string(),
        ];
        assert!(validate_mysql_source_grants(&expected, "v2board_legacy").is_ok());

        for extra in [
            "GRANT INSERT ON `v2board_legacy`.* TO `legacy_reader`@`%`",
            "GRANT `legacy_writer`@`%` TO `legacy_reader`@`%`",
            "GRANT SELECT ON *.* TO `legacy_reader`@`%`",
        ] {
            let mut excessive = expected.clone();
            excessive.push(extra.to_string());
            assert!(validate_mysql_source_grants(&excessive, "v2board_legacy").is_err());
        }

        let grantable = vec![
            expected[0].clone(),
            "GRANT SELECT ON `v2board_legacy`.* TO `legacy_reader`@`%` WITH GRANT OPTION"
                .to_string(),
        ];
        assert!(validate_mysql_source_grants(&grantable, "v2board_legacy").is_err());
    }

    #[test]
    fn source_inventory_requires_imported_tables_and_only_allowlists_known_discards() {
        let imported = TABLE_MAPPINGS
            .iter()
            .map(|mapping| mapping.source.to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            validate_source_table_inventory(&imported).unwrap(),
            imported
        );

        let mut with_optional_residue = imported.clone();
        with_optional_residue.insert("v2_tutorial".to_string());
        assert!(validate_source_table_inventory(&with_optional_residue).is_ok());

        let mut unknown = imported.clone();
        unknown.insert("unreviewed_legacy_table".to_string());
        assert!(validate_source_table_inventory(&unknown).is_err());

        let mut missing = imported;
        missing.remove(TABLE_MAPPINGS[0].source);
        assert!(validate_source_table_inventory(&missing).is_err());
    }

    #[test]
    fn every_imported_source_table_must_use_innodb() {
        let imported = TABLE_MAPPINGS
            .iter()
            .map(|mapping| mapping.source.to_string())
            .collect::<BTreeSet<_>>();
        let mut engines = imported
            .iter()
            .map(|table| (table.clone(), "InnoDB".to_string()))
            .collect::<BTreeMap<_, _>>();
        assert!(validate_source_table_engines(&imported, &engines).is_ok());

        engines.insert(TABLE_MAPPINGS[0].source.to_string(), "MyISAM".to_string());
        assert!(validate_source_table_engines(&imported, &engines).is_err());
        engines.insert(TABLE_MAPPINGS[0].source.to_string(), "InnoDB".to_string());
        engines.remove(TABLE_MAPPINGS[1].source);
        assert!(validate_source_table_engines(&imported, &engines).is_err());
    }

    fn postgres_role_url(
        bootstrap_url: &str,
        role: &str,
        password: &str,
        database: &str,
    ) -> anyhow::Result<String> {
        let mut url = Url::parse(bootstrap_url)?;
        url.set_username(role)
            .map_err(|_| anyhow::anyhow!("test PostgreSQL role is not URL-safe"))?;
        url.set_password(Some(password))
            .map_err(|_| anyhow::anyhow!("test PostgreSQL password is not URL-safe"))?;
        url.set_path(&format!("/{database}"));
        url.set_query(None);
        url.set_fragment(None);
        Ok(url.to_string())
    }

    fn set_document_value(document: &mut Value, pointer: &str, value: Value) {
        *document
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("missing example manifest field {pointer}")) = value;
    }

    fn local_execute_spec_and_plan(
        root: &Path,
        mysql_url: &str,
        postgres_root_url: &str,
        clickhouse_url: &str,
        clickhouse_username: &str,
        clickhouse_password: &str,
        redis_url: &str,
    ) -> anyhow::Result<(MysqlImportSpec, MysqlImportExecutionPlan)> {
        let dump_path = root.join("legacy-mysql.sql");
        let dump_bytes = b"immutable dump provenance fixture for the execute integration test\n";
        write_new_secret_file(&dump_path, dump_bytes)?;

        let output = root.join("output");
        let mut document: Value = serde_json::from_str(include_str!(
            "../../../../../docs/examples/mysql-import.v1.example.json"
        ))?;
        set_document_value(
            &mut document,
            "/source/dump_path",
            Value::String(dump_path.display().to_string()),
        );
        set_document_value(
            &mut document,
            "/source/dump_sha256",
            Value::String(sha256_hex(dump_bytes)),
        );
        set_document_value(
            &mut document,
            "/source/database_url",
            Value::String(
                "mysql://legacy_reader:LegacySourceManifestSecret-32-bytes@127.0.0.1:3306/v2board"
                    .to_string(),
            ),
        );
        for (pointer, value) in [
            (
                "/target/postgres/bootstrap_database_url",
                "postgresql://manifest_bootstrap:ManifestBootstrapSecret01@postgres.acme.internal:5432/postgres?sslmode=verify-full",
            ),
            (
                "/target/postgres/migration_database_url",
                "postgresql://manifest_migration:ManifestMigrationSecret02@postgres.acme.internal:5432/v2board?sslmode=verify-full",
            ),
            (
                "/target/postgres/api_database_url",
                "postgresql://manifest_api:ManifestApiRuntimeSecret03@postgres.acme.internal:5432/v2board?sslmode=verify-full",
            ),
            (
                "/target/postgres/worker_database_url",
                "postgresql://manifest_worker:ManifestWorkerRuntimeSecret04@postgres.acme.internal:5432/v2board?sslmode=verify-full",
            ),
            (
                "/target/clickhouse/endpoint",
                "https://clickhouse.acme.internal",
            ),
            (
                "/target/clickhouse/bootstrap_principal/password",
                "ClickHouseBootstrapSecretMaterial0001",
            ),
            (
                "/target/clickhouse/schema_principal/password",
                "ClickHouseSchemaSecretMaterial0000002",
            ),
            (
                "/target/clickhouse/writer_principal/password",
                "ClickHouseWriterSecretMaterial0000003",
            ),
            (
                "/target/analytics_admission/capacity_evidence",
                "dedicated PostgreSQL 18 volume with a measured 64 GiB quota",
            ),
            (
                "/target/redis_bootstrap_url",
                "rediss://manifest_redis_bootstrap:RedisManifestSecretMaterial-32-bytes@redis.acme.internal:6380/0",
            ),
            ("/runtime/app_key", "ApiApplicationSecretMaterial00000001"),
            (
                "/runtime/server_token",
                "NodeServerTokenSecretMaterial00000002",
            ),
            ("/runtime/app_url", "https://panel.acme.internal"),
            ("/runtime/subscribe_url", "https://panel.acme.internal"),
            ("/runtime/server_api_url", "https://panel.acme.internal"),
            ("/runtime/secure_path", "private_admin_execute_test"),
        ] {
            set_document_value(&mut document, pointer, Value::String(value.to_string()));
        }
        set_document_value(
            &mut document,
            "/target/config_output_directory",
            Value::String(output.display().to_string()),
        );
        set_document_value(
            &mut document,
            "/runtime/cors_allowed_origins",
            serde_json::json!(["https://panel.acme.internal"]),
        );
        set_document_value(
            &mut document,
            "/runtime/trusted_proxy_cidrs",
            serde_json::json!(["10.0.0.0/8"]),
        );

        let manifest_path = root.join("mysql-import.v1.json");
        let mut manifest_bytes = serde_json::to_vec_pretty(&document)?;
        manifest_bytes.push(b'\n');
        write_new_secret_file(&manifest_path, &manifest_bytes)?;
        let mut spec = v2board_provision::load_mysql_import_spec(&manifest_path)?;
        let mut plan = spec.execution_plan()?;
        // The networked E2E uses Docker-internal plaintext transports, but the
        // same generated Redis URL shape must first pass the real Production
        // boot parsers with the manifest's verified TLS endpoints.
        let mut production_api = plan.api_boot_config.clone();
        let mut production_worker = plan.worker_boot_config.clone();
        production_api.insert(
            "redis_url".to_string(),
            Value::String(redis_runtime_url(
                &plan.redis_bootstrap_url,
                "v2board_api_production_parser_probe",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )?),
        );
        production_worker.insert(
            "redis_url".to_string(),
            Value::String(redis_runtime_url(
                &plan.redis_bootstrap_url,
                "v2board_worker_production_parser_probe",
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
            )?),
        );
        validate_emitted_runtime_configs(&production_api, &production_worker)?;
        spec.source.database_url = mysql_url.to_string();

        let migration_url = postgres_role_url(
            postgres_root_url,
            EXECUTE_MIGRATION_ROLE,
            "MigrationExecuteSecret-32-bytes",
            EXECUTE_DATABASE,
        )?;
        let api_url = postgres_role_url(
            postgres_root_url,
            EXECUTE_API_ROLE,
            "ApiExecuteRuntimeSecret-32-bytes",
            EXECUTE_DATABASE,
        )?;
        let worker_url = postgres_role_url(
            postgres_root_url,
            EXECUTE_WORKER_ROLE,
            "WorkerExecuteRuntimeSecret-32-bytes",
            EXECUTE_DATABASE,
        )?;
        plan.postgres.bootstrap_database_url = postgres_root_url.to_string();
        plan.postgres.migration_database_url = migration_url.clone();
        plan.postgres.api_database_url = api_url.clone();
        plan.postgres.worker_database_url = worker_url.clone();
        plan.clickhouse.endpoint = clickhouse_url.to_string();
        plan.clickhouse.database = EXECUTE_CLICKHOUSE_DATABASE.to_string();
        plan.clickhouse.bootstrap_username = clickhouse_username.to_string();
        plan.clickhouse.bootstrap_password = clickhouse_password.to_string();
        plan.clickhouse.schema_username = EXECUTE_CLICKHOUSE_SCHEMA_ROLE.to_string();
        plan.clickhouse.schema_password = "SchemaExecuteSecretMaterial-32-bytes".to_string();
        plan.clickhouse.writer_username = EXECUTE_CLICKHOUSE_WRITER_ROLE.to_string();
        plan.clickhouse.writer_password = "WriterExecuteSecretMaterial-32-bytes".to_string();
        plan.redis_bootstrap_url = redis_url.to_string();
        plan.config_output_directory = output;

        for (config, database_url, peer) in [
            (&mut plan.api_boot_config, &api_url, EXECUTE_WORKER_ROLE),
            (&mut plan.worker_boot_config, &worker_url, EXECUTE_API_ROLE),
        ] {
            config.insert(
                "database_url".to_string(),
                Value::String(database_url.to_string()),
            );
            config.insert(
                "peer_database_principal".to_string(),
                Value::String(peer.to_string()),
            );
            config.insert(
                "redis_url".to_string(),
                Value::String(redis_url.to_string()),
            );
            config.insert(
                "environment".to_string(),
                Value::String("testing".to_string()),
            );
        }
        for (key, value) in [
            ("clickhouse_url", clickhouse_url),
            ("clickhouse_database", EXECUTE_CLICKHOUSE_DATABASE),
            ("clickhouse_writer_username", EXECUTE_CLICKHOUSE_WRITER_ROLE),
            (
                "clickhouse_writer_password",
                "WriterExecuteSecretMaterial-32-bytes",
            ),
        ] {
            plan.worker_boot_config
                .insert(key.to_string(), Value::String(value.to_string()));
        }
        Ok((spec, plan))
    }

    async fn cleanup_execute_postgres(postgres_root_url: &str) -> anyhow::Result<()> {
        let pool = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(postgres_root_url)
            .await?;
        execute_dynamic(
            &pool,
            format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                postgres_identifier(EXECUTE_DATABASE)
            ),
        )
        .await?;
        for role in [
            EXECUTE_MIGRATION_ROLE,
            EXECUTE_API_ROLE,
            EXECUTE_WORKER_ROLE,
        ] {
            execute_dynamic(
                &pool,
                format!("DROP ROLE IF EXISTS {}", postgres_identifier(role)),
            )
            .await?;
        }
        pool.close().await;
        Ok(())
    }

    async fn cleanup_execute_clickhouse(
        endpoint: &str,
        username: &str,
        password: &str,
    ) -> anyhow::Result<()> {
        let client = clickhouse_client(endpoint, "default", username, Some(password));
        client
            .query(&format!(
                "DROP DATABASE IF EXISTS {} SYNC",
                clickhouse_identifier(EXECUTE_CLICKHOUSE_DATABASE)
            ))
            .execute()
            .await?;
        for role in [
            EXECUTE_CLICKHOUSE_SCHEMA_ROLE,
            EXECUTE_CLICKHOUSE_WRITER_ROLE,
        ] {
            client
                .query(&format!(
                    "DROP USER IF EXISTS {}",
                    clickhouse_identifier(role)
                ))
                .execute()
                .await?;
        }
        Ok(())
    }

    fn assert_secret_output(path: &Path) -> anyhow::Result<Vec<u8>> {
        let metadata = fs::symlink_metadata(path)?;
        anyhow::ensure!(metadata.file_type().is_file());
        anyhow::ensure!(!metadata.file_type().is_symlink());
        anyhow::ensure!(metadata.uid() == 0);
        anyhow::ensure!(metadata.permissions().mode() & 0o777 == 0o600);
        Ok(fs::read(path)?)
    }

    async fn run_full_execute_test(
        root: &Path,
        mysql_url: &str,
        postgres_root_url: &str,
        clickhouse_url: &str,
        clickhouse_username: &str,
        clickhouse_password: &str,
        redis_url: &str,
    ) -> anyhow::Result<()> {
        let (spec, plan) = local_execute_spec_and_plan(
            root,
            mysql_url,
            postgres_root_url,
            clickhouse_url,
            clickhouse_username,
            clickhouse_password,
            redis_url,
        )?;
        audit_registry()?;
        let inspection = inspect_mysql_import(&spec)?;
        let postgres_identity = PostgresIdentity::from_plan(&plan)?;
        let api_url = plan.postgres.api_database_url.clone();
        let worker_url = plan.postgres.worker_database_url.clone();
        let output = plan.config_output_directory.clone();
        let report = execute_validated(&spec, inspection, plan).await?;

        anyhow::ensure!(report.status == "complete");
        anyhow::ensure!(report.converted_snapshot_sha256.len() == 64);
        anyhow::ensure!(report.converter_registry_sha256.len() == 64);
        anyhow::ensure!(report.imported_tables.len() == TABLE_MAPPINGS.len() + 1);
        anyhow::ensure!(report.discarded_tables.len() == DISCARDED_SOURCE_TABLES.len());
        for source in ["v2_log", "failed_jobs", "v2_tutorial"] {
            let discarded = report
                .discarded_tables
                .iter()
                .find(|table| table.source == source)
                .ok_or_else(|| anyhow::anyhow!("missing discard report for {source}"))?;
            anyhow::ensure!(discarded.present);
            anyhow::ensure!(!discarded.rows_scanned);
            anyhow::ensure!(discarded.policy.contains("without_row_scan"));
        }
        anyhow::ensure!(report.clickhouse_started_empty);
        anyhow::ensure!(report.redis_started_empty);
        anyhow::ensure!(report.redis_acl_persisted);
        anyhow::ensure!(report.redis_runtime_acl_isolated);
        anyhow::ensure!(!report.redis_bootstrap_credential_emitted);
        anyhow::ensure!(report.old_mysql_contacted);
        anyhow::ensure!(!report.old_mysql_mutated);
        anyhow::ensure!(!report.old_redis_contacted);
        anyhow::ensure!(!report.stripe_provider_contacted);

        let api_config_bytes = assert_secret_output(&output.join(API_CONFIG_FILE))?;
        let worker_config_bytes = assert_secret_output(&output.join(WORKER_CONFIG_FILE))?;
        let persisted_report_bytes = assert_secret_output(&output.join(REPORT_FILE))?;
        let bootstrap = Url::parse(redis_url)?;
        let bootstrap_username = percent_decode_str(bootstrap.username()).decode_utf8()?;
        let bootstrap_password =
            percent_decode_str(bootstrap.password().unwrap_or_default()).decode_utf8()?;
        for bytes in [
            api_config_bytes.as_slice(),
            worker_config_bytes.as_slice(),
            persisted_report_bytes.as_slice(),
        ] {
            anyhow::ensure!(
                !bytes
                    .windows(redis_url.len())
                    .any(|part| part == redis_url.as_bytes())
            );
            anyhow::ensure!(
                !bytes
                    .windows(bootstrap_username.len())
                    .any(|part| part == bootstrap_username.as_bytes())
            );
            anyhow::ensure!(
                !bytes
                    .windows(bootstrap_password.len())
                    .any(|part| part == bootstrap_password.as_bytes())
            );
        }
        let api_config: Map<String, Value> = serde_json::from_slice(&api_config_bytes)?;
        let worker_config: Map<String, Value> = serde_json::from_slice(&worker_config_bytes)?;
        validate_emitted_runtime_configs(&api_config, &worker_config)?;
        let persisted_report: Value = serde_json::from_slice(&persisted_report_bytes)?;
        anyhow::ensure!(api_config["database_url"] == api_url);
        anyhow::ensure!(worker_config["database_url"] == worker_url);
        let api_redis_url = api_config["redis_url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("API redis_url is not a string"))?;
        let worker_redis_url = worker_config["redis_url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("worker redis_url is not a string"))?;
        anyhow::ensure!(api_redis_url != worker_redis_url);
        let api_redis = Url::parse(api_redis_url)?;
        let worker_redis = Url::parse(worker_redis_url)?;
        anyhow::ensure!(api_redis.scheme() == worker_redis.scheme());
        anyhow::ensure!(api_redis.host_str() == worker_redis.host_str());
        anyhow::ensure!(api_redis.port_or_known_default() == worker_redis.port_or_known_default());
        anyhow::ensure!(api_redis.path() == worker_redis.path());
        anyhow::ensure!(api_redis.username() != worker_redis.username());
        anyhow::ensure!(api_redis.password() != worker_redis.password());
        anyhow::ensure!(api_redis.username() != bootstrap.username());
        anyhow::ensure!(worker_redis.username() != bootstrap.username());
        anyhow::ensure!(api_redis.password() != bootstrap.password());
        anyhow::ensure!(worker_redis.password() != bootstrap.password());
        anyhow::ensure!(
            !persisted_report_bytes
                .windows(api_redis_url.len())
                .any(|part| part == api_redis_url.as_bytes())
        );
        anyhow::ensure!(
            !persisted_report_bytes
                .windows(worker_redis_url.len())
                .any(|part| part == worker_redis_url.as_bytes())
        );
        for password in [api_redis.password(), worker_redis.password()]
            .into_iter()
            .flatten()
        {
            anyhow::ensure!(
                !persisted_report_bytes
                    .windows(password.len())
                    .any(|part| part == password.as_bytes())
            );
        }
        anyhow::ensure!(worker_config["clickhouse_database"] == EXECUTE_CLICKHOUSE_DATABASE);
        anyhow::ensure!(
            persisted_report["converted_snapshot_sha256"] == report.converted_snapshot_sha256
        );

        let root_pool = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(postgres_root_url)
            .await?;
        let migration_state: (bool, bool) = sqlx::query_as(
            "SELECT rolcanlogin, rolpassword IS NULL FROM pg_authid WHERE rolname = $1",
        )
        .bind(EXECUTE_MIGRATION_ROLE)
        .fetch_one(&root_pool)
        .await?;
        anyhow::ensure!(migration_state == (false, true));

        // NOLOGIN does not terminate an already-authenticated database-owner
        // session. Re-establish one under the test-only root boundary, then
        // prove the same retirement path used by execute actively removes it.
        execute_dynamic(
            &root_pool,
            format!(
                "ALTER ROLE {} LOGIN PASSWORD {}",
                postgres_identifier(EXECUTE_MIGRATION_ROLE),
                postgres_literal(&postgres_identity.migration_password)?
            ),
        )
        .await?;
        let extra_migration_session = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(2))
            .connect(postgres_identity.migration.as_str())
            .await?;
        let extra_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&extra_migration_session)
            .await?;
        let observed_extra_session: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE pid = $1 AND usename = $2)",
        )
        .bind(extra_pid)
        .bind(EXECUTE_MIGRATION_ROLE)
        .fetch_one(&root_pool)
        .await?;
        anyhow::ensure!(observed_extra_session);
        retire_postgres_migration_role(&postgres_identity).await?;
        anyhow::ensure!(
            sqlx::query("SELECT 1")
                .execute(&extra_migration_session)
                .await
                .is_err()
        );
        extra_migration_session.close().await;
        let active_migration_sessions: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pg_stat_activity WHERE usename = $1")
                .bind(EXECUTE_MIGRATION_ROLE)
                .fetch_one(&root_pool)
                .await?;
        anyhow::ensure!(active_migration_sessions == 0);
        root_pool.close().await;

        let worker = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(&worker_url)
            .await?;
        sqlx::query("SELECT id, balance FROM users ORDER BY id LIMIT 1")
            .execute(&worker)
            .await?;
        sqlx::query("SELECT id FROM plan ORDER BY id LIMIT 1 FOR SHARE")
            .execute(&worker)
            .await?;
        for denied in [
            "SELECT password FROM users LIMIT 1",
            "UPDATE users SET is_admin = is_admin WHERE id = 1",
            "SELECT message FROM ticket_message LIMIT 1",
            "UPDATE orders SET total_amount = total_amount WHERE id = 1",
            "SELECT singleton FROM operator_config_api_ack LIMIT 1",
        ] {
            anyhow::ensure!(sqlx::query(denied).execute(&worker).await.is_err());
        }
        worker.close().await;

        let api = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(&api_url)
            .await?;
        sqlx::query("SELECT event_id FROM analytics_outbox LIMIT 1 FOR SHARE")
            .execute(&api)
            .await?;
        anyhow::ensure!(
            sqlx::query("SELECT singleton FROM operator_config_worker_ack LIMIT 1")
                .execute(&api)
                .await
                .is_err()
        );
        api.close().await;

        let clickhouse = clickhouse_client(
            clickhouse_url,
            "default",
            clickhouse_username,
            Some(clickhouse_password),
        );
        let schema_count = clickhouse
            .query("SELECT count() AS value FROM system.users WHERE name = ?")
            .bind(EXECUTE_CLICKHOUSE_SCHEMA_ROLE)
            .fetch_one::<ClickHouseCount>()
            .await?
            .value;
        let writer_count = clickhouse
            .query("SELECT count() AS value FROM system.users WHERE name = ?")
            .bind(EXECUTE_CLICKHOUSE_WRITER_ROLE)
            .fetch_one::<ClickHouseCount>()
            .await?
            .value;
        anyhow::ensure!(schema_count == 0 && writer_count == 1);
        verify_empty_redis(redis_url).await?;
        Ok(())
    }

    #[tokio::test]
    async fn full_execute_bootstraps_and_retires_every_principal() {
        let Some(postgres_root_url) =
            std::env::var("RUST_INTEGRATION_EXECUTE_DATABASE_ROOT_URL").ok()
        else {
            return;
        };
        let mysql_url = std::env::var("RUST_INTEGRATION_LEGACY_MYSQL_URL")
            .expect("full execute test requires RUST_INTEGRATION_LEGACY_MYSQL_URL");
        let clickhouse_url = std::env::var("RUST_INTEGRATION_CLICKHOUSE_URL")
            .expect("full execute test requires RUST_INTEGRATION_CLICKHOUSE_URL");
        let clickhouse_username = std::env::var("RUST_INTEGRATION_CLICKHOUSE_USERNAME")
            .expect("full execute test requires RUST_INTEGRATION_CLICKHOUSE_USERNAME");
        let clickhouse_password = std::env::var("RUST_INTEGRATION_CLICKHOUSE_PASSWORD")
            .expect("full execute test requires RUST_INTEGRATION_CLICKHOUSE_PASSWORD");
        let redis_url = std::env::var("RUST_INTEGRATION_EXECUTE_REDIS_URL")
            .expect("full execute test requires RUST_INTEGRATION_EXECUTE_REDIS_URL");

        cleanup_execute_postgres(&postgres_root_url)
            .await
            .expect("pre-clean PostgreSQL execute target");
        cleanup_execute_clickhouse(&clickhouse_url, &clickhouse_username, &clickhouse_password)
            .await
            .expect("pre-clean ClickHouse execute target");
        let root = test_directory("full-execute");
        fs::create_dir(&root).expect("create execute test root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("harden execute test root");

        let result = run_full_execute_test(
            &root,
            &mysql_url,
            &postgres_root_url,
            &clickhouse_url,
            &clickhouse_username,
            &clickhouse_password,
            &redis_url,
        )
        .await;
        let postgres_cleanup = cleanup_execute_postgres(&postgres_root_url).await;
        let clickhouse_cleanup =
            cleanup_execute_clickhouse(&clickhouse_url, &clickhouse_username, &clickhouse_password)
                .await;
        let filesystem_cleanup = fs::remove_dir_all(&root);

        result.expect("full importer execute path");
        postgres_cleanup.expect("post-clean PostgreSQL execute target");
        clickhouse_cleanup.expect("post-clean ClickHouse execute target");
        filesystem_cleanup.expect("remove execute test root");
    }

    #[tokio::test]
    async fn imported_source_schema_matches_oracle_mysql_8_legacy_fixture() {
        let Ok(database_url) = std::env::var("RUST_INTEGRATION_LEGACY_MYSQL_URL") else {
            return;
        };
        let source_pool = MySqlPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(&database_url)
            .await
            .unwrap();
        let mut source = source_pool.acquire().await.unwrap();
        verify_mysql_vendor_and_version(&mut source).await.unwrap();
        verify_mysql_source_principal(&mut source, &database_url)
            .await
            .unwrap();
        assert!(
            sqlx::query("UPDATE v2_server_group SET name = name WHERE id = -1")
                .execute(&mut *source)
                .await
                .is_err(),
            "the legacy source fixture principal must have SELECT-only grants"
        );
        begin_mysql_read_snapshot(&mut source).await.unwrap();
        let observed = inspect_source_schema(&mut source).await.unwrap();
        commit_mysql_read_snapshot(&mut source).await.unwrap();
        assert_eq!(
            observed.imported_schema_sha256,
            MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256
        );
        assert!(
            observed
                .discarded_tables
                .iter()
                .any(|table| table.source == "v2_tutorial" && table.present && !table.rows_scanned)
        );

        if let Ok(admin_url) = std::env::var("RUST_INTEGRATION_LEGACY_MYSQL_FIXTURE_ADMIN_URL") {
            let admin_pool = MySqlPoolOptions::new()
                .min_connections(1)
                .max_connections(1)
                .connect(&admin_url)
                .await
                .unwrap();
            let mut admin = admin_pool.acquire().await.unwrap();
            begin_mysql_read_snapshot(&mut admin).await.unwrap();
            let error = verify_mysql_source_principal(&mut admin, &admin_url)
                .await
                .expect_err("the fixture administrator must not pass the SELECT-only source gate");
            assert!(
                error
                    .to_string()
                    .contains("only USAGE and database-level SELECT")
            );
            commit_mysql_read_snapshot(&mut admin).await.unwrap();
        }
    }

    #[tokio::test]
    async fn representative_mysql_rows_copy_into_fresh_postgres() {
        let (Ok(mysql_url), Ok(mysql_fixture_admin_url), Ok(postgres_url)) = (
            std::env::var("RUST_INTEGRATION_LEGACY_MYSQL_URL"),
            std::env::var("RUST_INTEGRATION_LEGACY_MYSQL_FIXTURE_ADMIN_URL"),
            std::env::var("RUST_INTEGRATION_MYSQL_IMPORT_DATABASE_URL"),
        ) else {
            return;
        };
        let source_pool = MySqlPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(&mysql_url)
            .await
            .unwrap();
        let mut source = source_pool.acquire().await.unwrap();
        let target = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(2)
            .connect(&postgres_url)
            .await
            .unwrap();

        verify_mysql_vendor_and_version(&mut source).await.unwrap();
        begin_mysql_read_snapshot(&mut source).await.unwrap();
        assert_eq!(
            inspect_source_schema(&mut source)
                .await
                .unwrap()
                .imported_schema_sha256,
            MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256
        );
        validate_source_relationships(&mut source).await.unwrap();
        POSTGRES_MIGRATOR.run(&target).await.unwrap();
        let reports = copy_business_data(&mut source, &target).await.unwrap();
        commit_mysql_read_snapshot(&mut source).await.unwrap();

        let payment = reports
            .iter()
            .find(|report| report.target == "payment_method")
            .unwrap();
        assert_eq!(
            (
                payment.source_rows,
                payment.retained_rows,
                payment.discarded_rows
            ),
            (2, 1, 1)
        );
        let orders = reports
            .iter()
            .find(|report| report.target == "orders")
            .unwrap();
        assert_eq!(
            (
                orders.source_rows,
                orders.retained_rows,
                orders.discarded_rows
            ),
            (3, 2, 1)
        );
        let redemptions = reports
            .iter()
            .find(|report| report.target == "gift_card_redemption")
            .unwrap();
        assert_eq!(redemptions.retained_rows, 2);
        assert!(
            validate_giftcard_redemption_users(
                &target,
                &[LegacyGiftcardRedemptionRow {
                    giftcard_id: 1,
                    user_id: 9_999_999,
                    created_at: 0,
                    created_at_provenance: "legacy_unknown".to_string(),
                }],
            )
            .await
            .is_err()
        );

        let payment_drivers =
            sqlx::query_scalar::<_, String>("SELECT payment FROM payment_method ORDER BY id")
                .fetch_all(&target)
                .await
                .unwrap();
        assert_eq!(payment_drivers, ["Manual"]);
        let order_bindings = sqlx::query_as::<_, (String, Option<i32>, Option<String>)>(
            "SELECT trade_no, payment_id, callback_no FROM orders ORDER BY id",
        )
        .fetch_all(&target)
        .await
        .unwrap();
        assert_eq!(
            order_bindings,
            [
                (
                    "trade-manual-complete".to_string(),
                    Some(1),
                    Some("manual-callback".to_string()),
                ),
                ("trade-stripe-complete".to_string(), None, None),
            ]
        );
        let next_group_id: i32 = sqlx::query_scalar(
            "INSERT INTO server_group (name, created_at, updated_at) \
             VALUES ('sequence-check', 1, 1) RETURNING id",
        )
        .fetch_one(&target)
        .await
        .unwrap();
        assert_eq!(next_group_id, 2);

        let admin_pool = MySqlPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(&mysql_fixture_admin_url)
            .await
            .unwrap();
        let mut admin = admin_pool.acquire().await.unwrap();
        (&mut *admin).execute("START TRANSACTION").await.unwrap();
        sqlx::query(
            "INSERT INTO v2_server_group (id, name, created_at, updated_at) VALUES (-1, 'invalid identity', 1, 1)",
        )
        .execute(&mut *admin)
        .await
        .unwrap();
        let error = validate_source_relationships(&mut admin)
            .await
            .expect_err("non-positive source identities must fail before target writes");
        assert!(error.to_string().contains("non-positive business identity"));
        (&mut *admin).execute("ROLLBACK").await.unwrap();
    }

    #[test]
    fn target_version_and_redemption_batch_contracts_are_pinned() {
        assert_eq!(clickhouse_major_minor("26.3.17.4"), Some((26, 3)));
        assert_eq!(clickhouse_major_minor("26.4.1.1"), Some((26, 4)));
        assert_eq!(clickhouse_major_minor("invalid"), None);
        assert_eq!(REQUIRED_POSTGRES_MAJOR, 18);
        assert_eq!(REQUIRED_POSTGRES_LOCALE_PROVIDER, "b");
        assert_eq!(REQUIRED_POSTGRES_BUILTIN_LOCALE, "C.UTF-8");
        assert_eq!(MAX_GIFTCARD_REDEMPTIONS_PER_INSERT, 16_383);
    }

    #[test]
    fn exhausted_identity_sequence_is_rejected_before_reset() {
        let integer = TABLE_MAPPINGS
            .iter()
            .find(|mapping| mapping.target == "server_group")
            .expect("integer identity mapping");
        assert!(validate_identity_headroom(integer, None).is_ok());
        assert!(validate_identity_headroom(integer, Some(i64::from(i32::MAX) - 1)).is_ok());
        assert!(
            validate_identity_headroom(integer, Some(i64::from(i32::MAX))).is_err(),
            "an imported INTEGER maximum leaves no value for nextval"
        );

        let bigint = TABLE_MAPPINGS
            .iter()
            .find(|mapping| mapping.target == "users")
            .expect("bigint identity mapping");
        assert!(validate_identity_headroom(bigint, Some(i64::MAX - 1)).is_ok());
        assert!(validate_identity_headroom(bigint, Some(i64::MAX)).is_err());
    }

    #[test]
    fn final_users_hash_binds_the_exact_inviter_graph() {
        let base = "1".repeat(64);
        let mut left = Sha256::new();
        left.update(b"v2board.mysql-import.user-inviters.v1\0");
        digest_user_inviter_row(&mut left, 1, Some(2));
        digest_user_inviter_row(&mut left, 2, None);
        let left = finalize_users_retained_sha256(&base, &hex::encode(left.finalize()));

        let mut right = Sha256::new();
        right.update(b"v2board.mysql-import.user-inviters.v1\0");
        digest_user_inviter_row(&mut right, 1, None);
        digest_user_inviter_row(&mut right, 2, Some(1));
        let right = finalize_users_retained_sha256(&base, &hex::encode(right.finalize()));

        assert_ne!(
            left, right,
            "equal counts must not hide a different inviter graph"
        );
    }

    #[test]
    fn config_output_rejects_existing_and_symlink_directory_entries() {
        let root = test_directory("output-boundary");
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();

        let dangling_output = root.join("dangling-output");
        symlink(root.join("missing-target"), &dangling_output).unwrap();
        assert!(require_absent_output_directory(&dangling_output).is_err());
        fs::remove_file(&dangling_output).unwrap();

        let real_parent = root.join("real-parent");
        fs::create_dir(&real_parent).unwrap();
        fs::set_permissions(&real_parent, fs::Permissions::from_mode(0o700)).unwrap();
        let parent_alias = root.join("parent-alias");
        symlink(&real_parent, &parent_alias).unwrap();
        assert!(require_absent_output_directory(&parent_alias.join("output")).is_err());

        fs::remove_file(parent_alias).unwrap();
        fs::remove_dir(real_parent).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
