use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};

use chrono::Utc;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, mysql::MySqlPoolOptions, postgres::PgPoolOptions};
use uuid::Uuid;
use v2board_analytics::{AnalyticsAdmissionPolicy, install_analytics_admission_policy};
use v2board_config::{AppConfig, RuntimePaths};
use v2board_provision::{
    MysqlImportExecutionPlan, MysqlImportSpec, inspect_mysql_import,
    mysql_import_converter::{
        MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256, audit_registry, registry_sha256,
    },
};

use super::{
    clickhouse_target::{bootstrap_clickhouse, preflight_clickhouse_absent},
    copy_stream::copy_business_data,
    mysql_source::{
        SourceSchemaInspection, begin_mysql_read_snapshot, commit_mysql_read_snapshot,
        inspect_source_schema, validate_source_relationships, verify_mysql_source_principal,
        verify_mysql_vendor_and_version,
    },
    postgres_grants::install_postgres_runtime_grants,
    postgres_target::{
        POSTGRES_MIGRATOR, PostgresIdentity, bootstrap_postgres, preflight_postgres_absent,
        retire_postgres_migration_role, verify_postgres_target_contract,
    },
    redis_target::{bootstrap_redis_runtime, preflight_redis_target, verify_empty_redis},
    target_verify::finalize_and_verify_business_data,
};

pub(crate) const API_CONFIG_FILE: &str = "api.config.json";
pub(crate) const WORKER_CONFIG_FILE: &str = "worker.config.json";
pub(crate) const REPORT_FILE: &str = "import-report.json";

#[derive(Debug, Serialize)]
pub struct MysqlImportExecutionReport {
    pub(crate) status: &'static str,
    pub(crate) schema_version: u32,
    pub(crate) manifest_sha256: String,
    pub(crate) inspected_dump_sha256: String,
    pub(crate) imported_source_schema_sha256: String,
    pub(crate) converter_registry_sha256: String,
    pub(crate) converted_snapshot_sha256: String,
    pub(crate) installation_id: String,
    pub(crate) imported_tables: Vec<ImportedTableReport>,
    pub(crate) discarded_tables: Vec<DiscardedTableReport>,
    pub(crate) api_config_sha256: String,
    pub(crate) worker_config_sha256: String,
    pub(crate) output_directory: String,
    pub(crate) clickhouse_started_empty: bool,
    pub(crate) redis_started_empty: bool,
    pub(crate) redis_acl_persisted: bool,
    pub(crate) redis_runtime_acl_isolated: bool,
    pub(crate) redis_bootstrap_credential_emitted: bool,
    pub(crate) old_mysql_contacted: bool,
    pub(crate) old_mysql_mutated: bool,
    pub(crate) old_redis_contacted: bool,
    pub(crate) stripe_provider_contacted: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ImportedTableReport {
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) source_rows: u64,
    pub(crate) retained_rows: u64,
    pub(crate) discarded_rows: u64,
    pub(crate) retained_sha256: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DiscardedTableReport {
    pub(crate) source: String,
    pub(crate) present: bool,
    pub(crate) rows_scanned: bool,
    pub(crate) policy: &'static str,
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

pub async fn execute(spec: &MysqlImportSpec) -> anyhow::Result<MysqlImportExecutionReport> {
    audit_registry()?;
    let inspection = inspect_mysql_import(spec)?;
    let plan = spec.execution_plan()?;
    execute_validated(spec, inspection, plan).await
}

pub(crate) async fn execute_validated(
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
    POSTGRES_MIGRATOR.run_to(1, &target).await?;

    let installation_id = Uuid::new_v4();
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO system_installation (singleton, installation_id, created_at) VALUES (1, $1, $2)",
    )
    .bind(installation_id)
    .bind(now)
    .execute(&target)
    .await?;

    let imported_tables = copy_business_data(&mut source, &target, &plan.app_key).await?;
    commit_mysql_read_snapshot(&mut source).await?;
    drop(source);
    source_pool.close().await;
    eprintln!("mysql-import PostgreSQL finalize phase started");
    POSTGRES_MIGRATOR.run(&target).await?;
    finalize_and_verify_business_data(&target, &imported_tables).await?;
    eprintln!("mysql-import PostgreSQL finalize phase completed");
    let converted_snapshot_sha256 = converted_snapshot_sha256(&imported_tables, &discarded_tables);
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

pub(crate) fn require_absent_output_directory(path: &Path) -> anyhow::Result<()> {
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

pub(crate) fn validate_emitted_runtime_configs(
    api: &Map<String, Value>,
    worker: &Map<String, Value>,
) -> anyhow::Result<()> {
    let paths = |config: &str| RuntimePaths {
        config: config.into(),
        frontend: "/opt/v2board/current/frontend".into(),
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

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
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

pub(crate) fn write_new_secret_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
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
