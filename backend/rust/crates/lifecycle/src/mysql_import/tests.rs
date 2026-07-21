use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};
use sqlx::{Executor, mysql::MySqlPoolOptions, postgres::PgPoolOptions};
use url::Url;
use v2board_analytics::clickhouse_client;
use v2board_provision::{
    MysqlImportExecutionPlan, MysqlImportSpec, inspect_mysql_import,
    mysql_import_converter::{
        CanonicalJson, CanonicalRow, CanonicalValue, DERIVED_MAPPINGS, DISCARDED_SOURCE_TABLES,
        MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256, SourceRow, SourceValue, TABLE_MAPPINGS,
        audit_registry,
    },
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
        "GRANT SELECT ON `v2board_legacy`.* TO `legacy_reader`@`%` WITH GRANT OPTION".to_string(),
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
        "../../../../../../docs/examples/mysql-import.v1.example.json"
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
        serde_json::json!(["127.0.0.1/32"]),
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
    anyhow::ensure!(report.imported_tables.len() == TABLE_MAPPINGS.len() + DERIVED_MAPPINGS.len());
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
    let migration_state: (bool, bool) =
        sqlx::query_as("SELECT rolcanlogin, rolpassword IS NULL FROM pg_authid WHERE rolname = $1")
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
    let Some(postgres_root_url) = std::env::var("RUST_INTEGRATION_EXECUTE_DATABASE_ROOT_URL").ok()
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
        std::env::var("RUST_INTEGRATION_IMPORT_POSTGRES_URL"),
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
    POSTGRES_MIGRATOR.run_to(1, &target).await.unwrap();
    let import_app_key = "ApiApplicationSecretMaterial00000001";
    let reports = copy_business_data(&mut source, &target, import_app_key)
        .await
        .unwrap();
    commit_mysql_read_snapshot(&mut source).await.unwrap();
    POSTGRES_MIGRATOR.run(&target).await.unwrap();
    finalize_and_verify_business_data(&target, &reports)
        .await
        .unwrap();

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
    let plan_prices = reports
        .iter()
        .find(|report| report.target == "plan_price")
        .unwrap();
    assert_eq!(plan_prices.retained_rows, 8);
    let prices = sqlx::query_as::<_, (String, i32)>(
        "SELECT period::text, amount_minor FROM plan_price ORDER BY plan_price.period",
    )
    .fetch_all(&target)
    .await
    .unwrap();
    assert_eq!(
        prices,
        [
            ("month".to_string(), 1000),
            ("quarter".to_string(), 2700),
            ("half_year".to_string(), 5000),
            ("year".to_string(), 9000),
            ("two_year".to_string(), 16000),
            ("three_year".to_string(), 21000),
            ("one_time".to_string(), 30000),
            ("reset".to_string(), -500),
        ]
    );
    assert!(
        sqlx::query(
            "INSERT INTO gift_card_redemption \
             (giftcard_id, user_id, created_at, created_at_provenance) \
             VALUES (1, 9999999, 0, 'legacy_unknown')",
        )
        .execute(&target)
        .await
        .is_err(),
        "the finalized foreign key must reject a missing user"
    );

    let payment_drivers =
        sqlx::query_scalar::<_, String>("SELECT payment FROM payment_method ORDER BY id")
            .fetch_all(&target)
            .await
            .unwrap();
    assert_eq!(payment_drivers, ["Manual"]);
    // The stored column is the at-rest envelope: opaque raw text that decrypts
    // (bound to the imported driver and uuid) back to the converter's exact
    // canonical form of the legacy config, where number spellings such as
    // 9007199254740993.25 and 1.2300e3 canonicalize by exact base-10 value.
    let stored_config =
        sqlx::query_scalar::<_, String>("SELECT config::text FROM payment_method WHERE id = 1")
            .fetch_one(&target)
            .await
            .unwrap();
    assert!(!stored_config.contains("migration-test"));
    assert!(stored_config.contains("ciphertext"));
    let decrypted = v2board_payment_adapters::payment_secrets::decrypt_payment_config_canonical(
        import_app_key,
        "Manual",
        "11111111111111111111111111111111",
        &stored_config,
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(decrypted).unwrap(),
        r#"{"account":"migration-test","exact":900719925474099325e-2,"scientific":123e1}"#
    );
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
    let notice_content: String = sqlx::query_scalar("SELECT content FROM notice WHERE id = 1")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(notice_content, "\\N, \"quoted\"\r\nline\\tail");
    let member_remarks: Option<String> =
        sqlx::query_scalar("SELECT remarks FROM users WHERE id = 2")
            .fetch_one(&target)
            .await
            .unwrap();
    assert_eq!(member_remarks.as_deref(), Some(""));
    let owner_telegram_id: Option<i64> =
        sqlx::query_scalar("SELECT telegram_id FROM users WHERE id = 1")
            .fetch_one(&target)
            .await
            .unwrap();
    assert_eq!(owner_telegram_id, Some(10001));
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
fn source_vendor_and_version_contract_is_pinned() {
    for version in ["8.0.42", "8.3.0", "8.4.6"] {
        assert!(is_supported_oracle_mysql_version(
            version,
            "MySQL Community Server - GPL"
        ));
    }
    for version in ["5.7.44", "8.1.0", "8.2.0", "8.5.0", "9.0.0"] {
        assert!(!is_supported_oracle_mysql_version(
            version,
            "MySQL Community Server - GPL"
        ));
    }
    assert!(!is_supported_oracle_mysql_version(
        "8.3.0-MariaDB",
        "MariaDB Server"
    ));
    assert!(!is_supported_oracle_mysql_version(
        "8.3.0-1",
        "Percona Server for MySQL"
    ));
    assert!(!is_supported_oracle_mysql_version(
        "8.3.0",
        "Compatible Database Server"
    ));
}

#[test]
fn target_version_and_copy_memory_contracts_are_pinned() {
    assert_eq!(clickhouse_major_minor("26.3.17.4"), Some((26, 3)));
    assert_eq!(clickhouse_major_minor("26.4.1.1"), Some((26, 4)));
    assert_eq!(clickhouse_major_minor("invalid"), None);
    assert_eq!(REQUIRED_POSTGRES_MAJOR, 18);
    assert_eq!(REQUIRED_POSTGRES_LOCALE_PROVIDER, "b");
    assert_eq!(REQUIRED_POSTGRES_BUILTIN_LOCALE, "C.UTF-8");
    assert_eq!(COPY_SEND_BUFFER_BYTES, 4 * 1024 * 1024);
    assert_eq!(MAX_COPY_ROW_BYTES, 16 * 1024 * 1024);
    assert_eq!(MAX_LEGACY_PAYMENT_METHODS, 4_096);
}

#[test]
fn legacy_payment_classifier_has_a_fixed_memory_bound() {
    let mapping = TABLE_MAPPINGS
        .iter()
        .find(|mapping| mapping.source == "v2_payment")
        .expect("payment mapping");
    let mut known = BTreeSet::new();
    let mut stripe = BTreeSet::new();
    for id in 1..=MAX_LEGACY_PAYMENT_METHODS {
        let row = SourceRow::from([
            ("id".to_string(), SourceValue::I64(id as i64)),
            (
                "payment".to_string(),
                SourceValue::Text("Manual".to_string()),
            ),
        ]);
        index_legacy_payment(mapping, &row, &mut known, &mut stripe).unwrap();
    }
    let overflow = SourceRow::from([
        (
            "id".to_string(),
            SourceValue::I64(MAX_LEGACY_PAYMENT_METHODS as i64 + 1),
        ),
        (
            "payment".to_string(),
            SourceValue::Text("StripeCheckout".to_string()),
        ),
    ]);
    let error = index_legacy_payment(mapping, &overflow, &mut known, &mut stripe)
        .expect_err("the payment index must fail closed at its fixed bound");
    assert!(error.to_string().contains("classification safety bound"));
    assert_eq!(known.len(), MAX_LEGACY_PAYMENT_METHODS);
    assert!(stripe.is_empty());
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
fn csv_copy_encoding_distinguishes_null_empty_and_hostile_text() {
    let columns = ["null_value", "empty", "text", "bytes", "json"];
    let row = CanonicalRow::from([
        ("null_value".to_string(), CanonicalValue::Null),
        ("empty".to_string(), CanonicalValue::Text(String::new())),
        (
            "text".to_string(),
            CanonicalValue::Text("\\N,\"quote\"\r\nline\\tail".to_string()),
        ),
        ("bytes".to_string(), CanonicalValue::Bytes(vec![0, 255])),
        (
            "json".to_string(),
            CanonicalValue::Json(CanonicalJson::parse(r#"{"quote":"a\"b"}"#).unwrap()),
        ),
    ]);
    let encoded = String::from_utf8(encode_copy_row("fixture", &columns, &row).unwrap())
        .expect("COPY CSV is UTF-8");
    assert_eq!(
        encoded,
        concat!(
            "\\N,\"\",\"\\N,\"\"quote\"\"\r\nline\\tail\",",
            "\"\\x00ff\",\"{\"\"quote\"\":\"\"a\\",
            "\"\"",
            "b\"\"}\"\n",
        )
    );
}

#[test]
fn csv_copy_encoding_rejects_nul_and_unrepresentable_unsigned_integer() {
    for value in [
        CanonicalValue::Text("bad\0text".to_string()),
        CanonicalValue::Json(CanonicalJson::parse(r#"{"bad":"\u0000"}"#).unwrap()),
        CanonicalValue::Json(CanonicalJson::parse(r#"{"\u0000":true}"#).unwrap()),
        CanonicalValue::U64(u64::MAX),
    ] {
        let row = CanonicalRow::from([("value".to_string(), value)]);
        assert!(encode_copy_row("fixture", &["value"], &row).is_err());
    }
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
