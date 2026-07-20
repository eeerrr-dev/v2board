//! Docker-only bootstrap for the browser-to-runtime E2E gate.
//!
//! The gate starts empty, dedicated PostgreSQL and Redis containers, then
//! calls this module through a feature-gated lifecycle example. The important
//! property is reuse: PostgreSQL grants and Redis ACLs come from the exact
//! importer code used for a real installation, while the production contract
//! binary remains free of the lifecycle/MySQL dependency graph.

use std::{env, path::PathBuf};

use serde_json::{Map, Value, json};
use sqlx::postgres::PgPoolOptions;
use v2board_analytics::{
    AnalyticsAdmissionPolicy, install_analytics_admission_policy, refresh_analytics_admission,
};
use v2board_config::{
    AppConfig, BOOT_ONLY_RUNTIME_KEYS_V1, RuntimePaths, RuntimeRole, save_config_atomic,
};

use super::{
    postgres_grants::install_postgres_runtime_grants,
    postgres_target::{
        PostgresIdentity, bootstrap_postgres, preflight_postgres_absent,
        retire_postgres_migration_role, verify_postgres_target_contract,
    },
    redis_target::{bootstrap_redis_runtime, preflight_redis_target},
};

const POSTGRES_BOOTSTRAP_URL_ENV: &str = "REAL_STACK_E2E_POSTGRES_BOOTSTRAP_URL";
const POSTGRES_MIGRATION_URL_ENV: &str = "REAL_STACK_E2E_POSTGRES_MIGRATION_URL";
const POSTGRES_API_URL_ENV: &str = "REAL_STACK_E2E_POSTGRES_API_URL";
const POSTGRES_WORKER_URL_ENV: &str = "REAL_STACK_E2E_POSTGRES_WORKER_URL";
const REDIS_BOOTSTRAP_URL_ENV: &str = "REAL_STACK_E2E_REDIS_BOOTSTRAP_URL";
const OUTPUT_ROOT_ENV: &str = "REAL_STACK_E2E_RUNTIME_ROOT";
const APP_KEY_ENV: &str = "REAL_STACK_E2E_APP_KEY";
const ADMIN_PATH_ENV: &str = "REAL_STACK_E2E_ADMIN_PATH";

pub(crate) async fn prepare_from_env() -> anyhow::Result<()> {
    let postgres_bootstrap_url = required_env(POSTGRES_BOOTSTRAP_URL_ENV)?;
    let postgres_migration_url = required_env(POSTGRES_MIGRATION_URL_ENV)?;
    let postgres_api_url = required_env(POSTGRES_API_URL_ENV)?;
    let postgres_worker_url = required_env(POSTGRES_WORKER_URL_ENV)?;
    let redis_bootstrap_url = required_env(REDIS_BOOTSTRAP_URL_ENV)?;
    let output_root = PathBuf::from(required_env(OUTPUT_ROOT_ENV)?);
    let app_key = required_env(APP_KEY_ENV)?;
    let admin_path = required_env(ADMIN_PATH_ENV)?;

    anyhow::ensure!(
        output_root.is_absolute(),
        "{OUTPUT_ROOT_ENV} must be absolute"
    );
    anyhow::ensure!(
        output_root.components().all(|component| !matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )),
        "{OUTPUT_ROOT_ENV} must be normalized"
    );

    let postgres = PostgresIdentity::from_urls(
        &postgres_bootstrap_url,
        &postgres_migration_url,
        &postgres_api_url,
        &postgres_worker_url,
    )?;
    preflight_postgres_absent(&postgres).await?;
    preflight_redis_target(&redis_bootstrap_url).await?;
    bootstrap_postgres(&postgres).await?;

    let migration = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(4)
        .connect(&postgres_migration_url)
        .await?;
    verify_postgres_target_contract(&migration, &postgres.database).await?;
    // V2BOARD_SEED_LOCAL=1 is set only on the disposable bootstrap service.
    // That uses the product's normal migration and local seed path, including
    // the browser credential admin@example.com / 12345678.
    v2board_db::migrate_postgres(&migration, false).await?;
    let installation_id = v2board_db::installation_id(&migration).await?;
    install_test_admission_policy(&migration, installation_id).await?;

    let redis = bootstrap_redis_runtime(&redis_bootstrap_url, installation_id).await?;
    let api_path = output_root.join("api/config.json");
    let worker_path = output_root.join("worker/config.json");
    let runtime_paths = RuntimePaths {
        config: api_path.clone(),
        frontend: PathBuf::from("/app/frontend-deploy"),
        rules: output_root.join("rules"),
    };

    // Parse the candidate through AppConfig before committing it as authority.
    // This map is intentionally not the emitted file-only document: the admin
    // path and app URL are operator-owned values and therefore belong only in
    // PostgreSQL after the initial seed.
    let candidate = AppConfig::try_from_api_boot_config_map(
        Map::from_iter([
            ("environment".to_string(), json!("testing")),
            ("bind_addr".to_string(), json!("0.0.0.0:8080")),
            ("app_key".to_string(), json!(&app_key)),
            ("database_url".to_string(), json!(&postgres_api_url)),
            (
                "peer_database_principal".to_string(),
                json!(&postgres.worker_role),
            ),
            ("redis_url".to_string(), json!(&redis.api_url)),
            (
                "app_url".to_string(),
                json!("http://rust-real-stack-api:8080"),
            ),
            ("frontend_admin_path".to_string(), json!(&admin_path)),
            ("secure_path".to_string(), json!(&admin_path)),
            ("server_require_idempotency_key".to_string(), json!(true)),
        ]),
        runtime_paths,
    )?;
    v2board_domain::operator_config::seed_initial_authority(
        &migration,
        installation_id,
        &candidate.app_key,
        &candidate.operator_config_map(),
        "real-stack-e2e",
    )
    .await?;

    install_postgres_runtime_grants(&migration, &postgres).await?;

    let api_config = boot_config(
        RuntimeRole::Api,
        &candidate.app_key,
        "0.0.0.0:8080",
        &postgres_api_url,
        &postgres.worker_role,
        &redis.api_url,
    );
    let worker_config = boot_config(
        RuntimeRole::Worker,
        &candidate.app_key,
        "0.0.0.0:8080",
        &postgres_worker_url,
        &postgres.api_role,
        &redis.worker_url,
    );
    // Parse both emitted documents before they become service inputs. This is
    // also the guard that keeps the E2E topology honest when boot-only config
    // keys change.
    AppConfig::try_from_api_boot_config_map(
        api_config.clone(),
        RuntimePaths {
            config: api_path.clone(),
            frontend: PathBuf::from("/app/frontend-deploy"),
            rules: output_root.join("rules"),
        },
    )?;
    AppConfig::try_from_worker_boot_config_map(
        worker_config.clone(),
        RuntimePaths {
            config: worker_path.clone(),
            frontend: PathBuf::from("/app/frontend-deploy"),
            rules: output_root.join("rules"),
        },
    )?;
    save_config_atomic(&api_path, &api_config)?;
    // The browser lane starts only the API. Validate the worker candidate to
    // keep the bootstrap contract covered, but never persist its credential
    // beside a process that has no reason to read it.

    migration.close().await;
    retire_postgres_migration_role(&postgres).await?;
    println!(
        "real-stack E2E runtime prepared: installation_id={installation_id}, postgres_api_role={}, postgres_worker_role={}",
        postgres.api_role, postgres.worker_role
    );
    Ok(())
}

fn boot_config(
    role: RuntimeRole,
    app_key: &str,
    bind_addr: &str,
    database_url: &str,
    peer_database_principal: &str,
    redis_url: &str,
) -> Map<String, Value> {
    let mut config = BOOT_ONLY_RUNTIME_KEYS_V1
        .iter()
        .map(|key| ((*key).to_string(), Value::Null))
        .collect::<Map<_, _>>();
    config.insert("configuration_source".to_string(), json!("file_only"));
    config.insert("configuration_scope".to_string(), json!("boot_only"));
    config.insert(
        "runtime_role".to_string(),
        json!(match role {
            RuntimeRole::Api => "api",
            RuntimeRole::Worker => "worker",
        }),
    );
    config.insert("environment".to_string(), json!("testing"));
    config.insert("bind_addr".to_string(), json!(bind_addr));
    config.insert("cors_allowed_origins".to_string(), json!([]));
    config.insert("trusted_proxy_cidrs".to_string(), json!([]));
    config.insert("http_connect_timeout_seconds".to_string(), json!(5));
    config.insert("http_request_timeout_seconds".to_string(), json!(10));
    config.insert("api_request_timeout_seconds".to_string(), json!(15));
    config.insert("password_kdf_max_parallel".to_string(), json!(2));
    config.insert("app_key".to_string(), json!(app_key));
    config.insert("database_url".to_string(), json!(database_url));
    config.insert(
        "peer_database_principal".to_string(),
        json!(peer_database_principal),
    );
    config.insert("redis_url".to_string(), json!(redis_url));
    if role == RuntimeRole::Worker {
        config.insert(
            "clickhouse_url".to_string(),
            json!("http://clickhouse:8123"),
        );
        config.insert(
            "clickhouse_database".to_string(),
            json!("v2board_analytics"),
        );
        config.insert(
            "clickhouse_writer_username".to_string(),
            json!("v2board_analytics"),
        );
        config.insert("clickhouse_writer_password".to_string(), json!("e2e-only"));
    }
    config
}

async fn install_test_admission_policy(
    pool: &sqlx::PgPool,
    installation_id: uuid::Uuid,
) -> anyhow::Result<()> {
    let gib = 1024_u64 * 1024 * 1024;
    let policy = AnalyticsAdmissionPolicy {
        recovery_pending_rows: 750_000,
        soft_pending_rows: 1_000_000,
        hard_pending_rows: 2_000_000,
        recovery_relation_bytes: 3 * gib,
        soft_relation_bytes: 4 * gib,
        hard_relation_bytes: 8 * gib,
        recovery_oldest_age_seconds: 120,
        soft_oldest_age_seconds: 300,
        hard_oldest_age_seconds: 1_800,
        database_capacity_bytes: 64 * gib,
        hard_min_headroom_bytes: 8 * gib,
        soft_min_headroom_bytes: 16 * gib,
        recovery_min_headroom_bytes: 20 * gib,
        event_reservation_bytes: 4_096,
        soft_max_new_rows_per_second: 100_000,
        sample_interval_seconds: 1,
        stale_after_seconds: 10,
        capacity_evidence: "disposable real-stack browser E2E target".to_string(),
    };
    let now = chrono::Utc::now().timestamp();
    install_analytics_admission_policy(pool, installation_id, &policy, now).await?;
    refresh_analytics_admission(pool).await?;
    Ok(())
}

fn required_env(key: &str) -> anyhow::Result<String> {
    let value = env::var(key).map_err(|_| anyhow::anyhow!("{key} is required"))?;
    let value = value.trim();
    anyhow::ensure!(!value.is_empty(), "{key} must not be empty");
    Ok(value.to_string())
}
