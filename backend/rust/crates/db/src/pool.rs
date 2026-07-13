use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use chrono::Utc;
use sqlx::{PgConnection, PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use std::time::Duration;
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

// Advisory locks are scoped to one PostgreSQL cluster.  A fixed, audited key is
// preferable to a server-version-dependent text hash for the local-only seed.
const LOCAL_SEED_LOCK: i64 = 0x0056_3242_4f41_5244;
const LOCAL_SEED_ADMIN_EMAIL: &str = "admin@example.com";
const LOCAL_SEED_ADMIN_PASSWORD: &str = "12345678";
const RETIRED_LOCAL_SEED_ADMIN_EMAIL: &str = "admin@local";
const REQUIRED_POSTGRES_MAJOR: i32 = 18;

pub type DbPool = PgPool;
pub type DbTransaction<'a> = Transaction<'a, Postgres>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbPoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl Default for DbPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 20,
            acquire_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(10 * 60),
            max_lifetime: Duration::from_secs(30 * 60),
        }
    }
}

impl DbPoolConfig {
    pub fn from_env() -> Result<Self, DbInitError> {
        let defaults = Self::default();
        let config = Self {
            min_connections: env_u32(
                "V2BOARD_DATABASE_MIN_CONNECTIONS",
                defaults.min_connections,
                true,
            )?,
            max_connections: env_u32(
                "V2BOARD_DATABASE_MAX_CONNECTIONS",
                defaults.max_connections,
                false,
            )?,
            acquire_timeout: Duration::from_secs(env_u64(
                "V2BOARD_DATABASE_ACQUIRE_TIMEOUT_SECONDS",
                defaults.acquire_timeout.as_secs(),
            )?),
            idle_timeout: Duration::from_secs(env_u64(
                "V2BOARD_DATABASE_IDLE_TIMEOUT_SECONDS",
                defaults.idle_timeout.as_secs(),
            )?),
            max_lifetime: Duration::from_secs(env_u64(
                "V2BOARD_DATABASE_MAX_LIFETIME_SECONDS",
                defaults.max_lifetime.as_secs(),
            )?),
        };
        if config.min_connections > config.max_connections {
            return Err(DbInitError::Configuration(format!(
                "V2BOARD_DATABASE_MIN_CONNECTIONS ({}) exceeds V2BOARD_DATABASE_MAX_CONNECTIONS ({})",
                config.min_connections, config.max_connections
            )));
        }
        Ok(config)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DbInitError {
    #[error("database configuration error: {0}")]
    Configuration(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("password hash error: {0}")]
    Password(String),
    #[error("timed out acquiring the local seed lock")]
    SeedLockUnavailable,
    #[error("lost the local seed lock before it could be released")]
    SeedLockLost,
    #[error(
        "database contains tables but no native PostgreSQL SQLx migration lineage; ordinary migrate cannot adopt a legacy or unknown schema—use the reviewed lifecycle workflow"
    )]
    UnboundMigrationTarget,
    #[error(
        "production PostgreSQL schema changes require the reviewed lifecycle apply workflow; ordinary migrate can only verify an already-current bound native database"
    )]
    ProductionLifecycleRequired,
    #[error("PostgreSQL SQLx migration ledger is not an exact valid prefix of this binary")]
    InvalidMigrationLineage,
    #[error("PostgreSQL installation binding is missing or is not uniquely active")]
    InstallationBindingMissing,
    #[error("native runtime requires PostgreSQL 18.x, but the server reports version_num {0}")]
    UnsupportedPostgresVersion(i32),
}

pub async fn connect_postgres(database_url: &str) -> Result<DbPool, DbInitError> {
    let pool = connect_postgres_with_config(database_url, &DbPoolConfig::from_env()?).await?;
    let version_num: i32 =
        sqlx::query_scalar("SELECT current_setting('server_version_num')::INTEGER")
            .fetch_one(&pool)
            .await?;
    if postgres_major(version_num) != REQUIRED_POSTGRES_MAJOR {
        pool.close().await;
        return Err(DbInitError::UnsupportedPostgresVersion(version_num));
    }
    Ok(pool)
}

fn postgres_major(version_num: i32) -> i32 {
    version_num / 10_000
}

pub async fn connect_postgres_with_config(
    database_url: &str,
    config: &DbPoolConfig,
) -> Result<DbPool, DbInitError> {
    let pool = PgPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .max_lifetime(Some(config.max_lifetime))
        .test_before_acquire(true)
        .after_connect(|connection, _metadata| {
            Box::pin(async move {
                // PostgreSQL READ COMMITTED plus explicit row locks and database
                // constraints most closely matches the application's intended
                // current-row semantics.  PostgreSQL REPEATABLE READ is snapshot
                // isolation and would require whole-transaction 40001 retries.
                sqlx::query("SET TIME ZONE 'UTC'")
                    .execute(&mut *connection)
                    .await?;
                // Native migrations own exactly the public schema. Keeping
                // pg_catalog implicit makes PostgreSQL search it before public,
                // while preventing role- or database-level search_path drift
                // from redirecting unqualified application tables.
                sqlx::query("SET search_path TO public")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query(
                    "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL READ COMMITTED",
                )
                .execute(&mut *connection)
                .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn migrate_postgres(pool: &DbPool, production: bool) -> Result<(), DbInitError> {
    // Validate before touching schema state. A misspelled migration target must
    // never become a native installation or receive the well-known local admin.
    let should_seed_local = local_seed_enabled()?;
    validate_migration_target(pool, production).await?;
    MIGRATOR.run(pool).await?;
    if should_seed_local {
        seed_local(pool).await?;
    }
    Ok(())
}

async fn validate_migration_target(pool: &DbPool, production: bool) -> Result<(), DbInitError> {
    let table_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'",
    )
    .fetch_one(pool)
    .await?;
    if table_count == 0 {
        if production {
            return Err(DbInitError::ProductionLifecycleRequired);
        }
        return Ok(());
    }
    if !table_exists(pool, "_sqlx_migrations").await? {
        return Err(DbInitError::UnboundMigrationTarget);
    }
    let applied = sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
        "SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await?;
    let embedded = MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| (migration.version, migration.checksum.as_ref()))
        .collect::<Vec<_>>();
    if !migration_records_valid_prefix(&applied, &embedded) {
        return Err(DbInitError::InvalidMigrationLineage);
    }
    if production && !migration_records_current(&applied, &embedded) {
        return Err(DbInitError::ProductionLifecycleRequired);
    }
    let installation_active = if table_exists(pool, "v2_system_installation").await? {
        sqlx::query_scalar::<_, bool>(
            "SELECT COUNT(*) = 1 FROM v2_system_installation \
             WHERE singleton = 1 AND state = 'active' AND activated_at IS NOT NULL",
        )
        .fetch_one(pool)
        .await?
    } else {
        false
    };
    if !installation_active {
        return Err(DbInitError::InstallationBindingMissing);
    }
    Ok(())
}

async fn table_exists(pool: &DbPool, table_name: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = current_schema() AND table_name = $1
        )
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await
}

/// Reports whether every successful database migration exactly matches the ordered versions and
/// SHA-384 checksums embedded in this binary. A failed row or missing/unreadable SQLx migration
/// table fails closed rather than declaring an incompatible schema ready.
pub async fn migrations_current(pool: &DbPool) -> Result<bool, sqlx::Error> {
    let applied = sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
        "SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await?;
    let embedded = MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| (migration.version, migration.checksum.as_ref()))
        .collect::<Vec<_>>();
    Ok(migration_records_current(&applied, &embedded))
}

/// Returns the immutable identity of the one active installation.
///
/// `singleton = 1` is the table primary key, so `fetch_one` means exactly one
/// active row: pending, retired, missing, or unbootstrapped installations fail
/// closed with `RowNotFound`.
pub async fn installation_id(pool: &DbPool) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar("SELECT installation_id FROM v2_system_installation WHERE state = 'active'")
        .fetch_one(pool)
        .await
}

fn migration_records_current(applied: &[(i64, Vec<u8>, bool)], embedded: &[(i64, &[u8])]) -> bool {
    applied.len() == embedded.len()
        && applied.iter().zip(embedded).all(
            |((applied_version, applied_checksum, success), (embedded_version, checksum))| {
                *success
                    && applied_version == embedded_version
                    && applied_checksum.as_slice() == *checksum
            },
        )
}

fn migration_records_valid_prefix(
    applied: &[(i64, Vec<u8>, bool)],
    embedded: &[(i64, &[u8])],
) -> bool {
    applied.len() <= embedded.len()
        && applied.iter().zip(embedded).all(
            |((applied_version, applied_checksum, success), (embedded_version, checksum))| {
                *success
                    && applied_version == embedded_version
                    && applied_checksum.as_slice() == *checksum
            },
        )
}

fn env_u32(key: &str, default: u32, allow_zero: bool) -> Result<u32, DbInitError> {
    let Some(raw) = std::env::var(key).ok() else {
        return Ok(default);
    };
    let value = raw.trim().parse::<u32>().map_err(|_| {
        DbInitError::Configuration(format!("{key} must be an unsigned integer, got {raw:?}"))
    })?;
    if !allow_zero && value == 0 {
        return Err(DbInitError::Configuration(format!(
            "{key} must be greater than zero"
        )));
    }
    Ok(value)
}

fn env_u64(key: &str, default: u64) -> Result<u64, DbInitError> {
    let Some(raw) = std::env::var(key).ok() else {
        return Ok(default);
    };
    let value = raw.trim().parse::<u64>().map_err(|_| {
        DbInitError::Configuration(format!("{key} must be an unsigned integer, got {raw:?}"))
    })?;
    if value == 0 {
        return Err(DbInitError::Configuration(format!(
            "{key} must be greater than zero"
        )));
    }
    Ok(value)
}

fn local_seed_enabled() -> Result<bool, DbInitError> {
    local_seed_enabled_for(
        std::env::var("V2BOARD_SEED_LOCAL").ok().as_deref(),
        std::env::var("V2BOARD_ENV").ok().as_deref(),
    )
}

fn local_seed_enabled_for(
    seed_value: Option<&str>,
    environment: Option<&str>,
) -> Result<bool, DbInitError> {
    let enabled = seed_value
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"));
    let production = environment.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "prod" | "production"
        )
    });
    if enabled && production {
        return Err(DbInitError::Configuration(
            "V2BOARD_SEED_LOCAL must not be enabled when V2BOARD_ENV is prod/production"
                .to_string(),
        ));
    }
    Ok(enabled)
}

async fn seed_local(pool: &PgPool) -> Result<(), DbInitError> {
    let mut connection = pool.acquire().await?;
    let acquired = sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
        .bind(LOCAL_SEED_LOCK)
        .fetch_one(&mut *connection)
        .await?;
    if !acquired {
        return Err(DbInitError::SeedLockUnavailable);
    }

    let seed_result = seed_local_locked(&mut connection).await;
    let released = sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
        .bind(LOCAL_SEED_LOCK)
        .fetch_one(&mut *connection)
        .await;

    seed_result?;
    if !released? {
        return Err(DbInitError::SeedLockLost);
    }
    Ok(())
}

async fn seed_local_locked(connection: &mut PgConnection) -> Result<(), DbInitError> {
    let now = Utc::now().timestamp();
    let installation_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM v2_system_installation)")
            .fetch_one(&mut *connection)
            .await?;
    if !installation_exists {
        sqlx::query(
            r#"
            INSERT INTO v2_system_installation (
                singleton, installation_id, lineage, state, created_at, activated_at
            )
            VALUES (1, $1, 'native', 'active', $2, $3)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(now)
        .bind(now)
        .execute(&mut *connection)
        .await?;
    }
    let current_admin_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM v2_user WHERE lower(btrim(email)) = lower(btrim($1)))",
    )
    .bind(LOCAL_SEED_ADMIN_EMAIL)
    .fetch_one(&mut *connection)
    .await?;
    if !current_admin_exists {
        sqlx::query(
            "UPDATE v2_user SET email = $1, updated_at = $2 \
             WHERE id = (SELECT id FROM v2_user \
                         WHERE lower(btrim(email)) = lower(btrim($3)) AND is_admin = 1 LIMIT 1)",
        )
        .bind(LOCAL_SEED_ADMIN_EMAIL)
        .bind(now)
        .bind(RETIRED_LOCAL_SEED_ADMIN_EMAIL)
        .execute(&mut *connection)
        .await?;
    }

    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
        .map_err(|error| DbInitError::Password(error.to_string()))?;
    let password = Argon2::default()
        .hash_password(LOCAL_SEED_ADMIN_PASSWORD.as_bytes(), &salt)
        .map_err(|error| DbInitError::Password(error.to_string()))?
        .to_string();
    let uuid = Uuid::new_v4().hyphenated().to_string();
    let token = format!(
        "{:x}",
        md5::compute(format!("{}-{now}", Uuid::new_v4().hyphenated()))
    );
    sqlx::query(
        r#"
        INSERT INTO v2_user (
            email, password, uuid, token, is_admin, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, 1, $5, $6)
        ON CONFLICT ((lower(btrim(email)))) DO NOTHING
        "#,
    )
    .bind(LOCAL_SEED_ADMIN_EMAIL)
    .bind(password)
    .bind(uuid)
    .bind(token)
    .bind(now)
    .bind(now)
    .execute(&mut *connection)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO v2_server_group (name, created_at, updated_at)
        SELECT 'Default Group', $1, $2
        WHERE NOT EXISTS (SELECT 1 FROM v2_server_group)
        "#,
    )
    .bind(now)
    .bind(now)
    .execute(&mut *connection)
    .await?;

    let group_id: i32 = sqlx::query_scalar("SELECT id FROM v2_server_group ORDER BY id LIMIT 1")
        .fetch_one(&mut *connection)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO v2_plan (
            group_id, transfer_enable, name, show, sort, renew, content,
            month_price, quarter_price, half_year_price, year_price,
            onetime_price, created_at, updated_at
        )
        SELECT $1, 100, 'Test Plan', 1, 1, 1, 'Local Rust test plan',
               100, 280, 540, 1000, 9900, $2, $3
        WHERE NOT EXISTS (SELECT 1 FROM v2_plan)
        "#,
    )
    .bind(group_id)
    .bind(now)
    .bind(now)
    .execute(&mut *connection)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO v2_knowledge (
            language, category, title, body, sort, show, created_at, updated_at
        )
        SELECT 'zh-CN', '使用文档', '本地开发环境快速开始',
               $1, 1, 1, $2, $3
        WHERE NOT EXISTS (SELECT 1 FROM v2_knowledge)
        "#,
    )
    .bind(format!(
        "# 本地开发环境快速开始\n\n这是 Rust API 与 React 前端本地环境的默认文档。\n\n- 测试账号：{LOCAL_SEED_ADMIN_EMAIL}\n- 测试密码：{LOCAL_SEED_ADMIN_PASSWORD}"
    ))
    .bind(now)
    .bind(now)
    .execute(&mut *connection)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_readiness_requires_exact_versions_checksums_and_success() {
        let checksum_1 = b"one".as_slice();
        let checksum_2 = b"two".as_slice();
        let embedded = [(1, checksum_1), (2, checksum_2)];
        assert!(migration_records_current(
            &[
                (1, checksum_1.to_vec(), true),
                (2, checksum_2.to_vec(), true),
            ],
            &embedded,
        ));
        assert!(migration_records_valid_prefix(&[], &embedded));
        assert!(migration_records_valid_prefix(
            &[(1, checksum_1.to_vec(), true)],
            &embedded,
        ));
        assert!(migration_records_valid_prefix(
            &[
                (1, checksum_1.to_vec(), true),
                (2, checksum_2.to_vec(), true),
            ],
            &embedded,
        ));
        assert!(!migration_records_valid_prefix(
            &[(2, checksum_2.to_vec(), true)],
            &embedded,
        ));
        assert!(!migration_records_valid_prefix(
            &[
                (1, checksum_1.to_vec(), true),
                (2, checksum_2.to_vec(), true),
                (3, b"future".to_vec(), true),
            ],
            &embedded,
        ));
        assert!(!migration_records_current(
            &[(1, checksum_1.to_vec(), true)],
            &embedded,
        ));
        assert!(!migration_records_current(
            &[
                (1, checksum_1.to_vec(), true),
                (3, checksum_2.to_vec(), true),
            ],
            &embedded,
        ));
        assert!(!migration_records_current(
            &[
                (1, checksum_1.to_vec(), true),
                (2, b"changed".to_vec(), true),
            ],
            &embedded,
        ));
        assert!(!migration_records_current(
            &[
                (1, checksum_1.to_vec(), true),
                (2, checksum_2.to_vec(), false),
            ],
            &embedded,
        ));
    }

    #[test]
    fn production_environment_rejects_the_known_local_seed() {
        for environment in ["prod", "PROD", "production", "Production"] {
            let error = local_seed_enabled_for(Some("1"), Some(environment)).unwrap_err();
            assert!(error.to_string().contains("must not be enabled"));
        }
        assert!(local_seed_enabled_for(Some("true"), Some("local")).unwrap());
        assert!(!local_seed_enabled_for(Some("0"), Some("production")).unwrap());
        assert!(!local_seed_enabled_for(None, Some("production")).unwrap());
    }

    #[test]
    fn postgres_migrations_are_independent_from_the_mysql_lineage() {
        let baseline = include_str!("../../../migrations-postgres/0001_initial.sql");
        let extension = include_str!(
            "../../../migrations-postgres/0002_legacy_lifecycle_and_analytics_admission.sql"
        );
        assert!(baseline.contains("PostgreSQL 18"));
        assert!(baseline.contains("GENERATED BY DEFAULT AS IDENTITY"));
        assert!(baseline.contains("uniq_unfinished_order_per_user"));
        assert!(baseline.contains("CREATE TABLE v2_system_installation"));
        assert!(baseline.contains("CREATE TABLE v2_analytics_outbox"));
        assert!(!baseline.contains("CREATE TABLE v2_lifecycle_operation"));
        assert!(extension.contains("CREATE TABLE v2_lifecycle_operation"));
        assert!(extension.contains("CREATE TABLE v2_lifecycle_event"));
        assert!(extension.contains("lifecycle events are append-only"));
        assert!(extension.contains("created_at_provenance = 'legacy_unknown'"));
        let executable_sql = [baseline, extension]
            .join("\n")
            .lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!executable_sql.contains("gen_random_uuid"));
        assert!(!executable_sql.contains("ENGINE=InnoDB"));
        assert!(!executable_sql.contains("AUTO_INCREMENT"));
        assert!(!executable_sql.contains('`'));
    }

    #[test]
    fn native_runtime_accepts_only_postgres_18_patch_releases() {
        assert_eq!(postgres_major(180_000), REQUIRED_POSTGRES_MAJOR);
        assert_eq!(postgres_major(180_004), REQUIRED_POSTGRES_MAJOR);
        assert_ne!(postgres_major(170_009), REQUIRED_POSTGRES_MAJOR);
        assert_ne!(postgres_major(190_000), REQUIRED_POSTGRES_MAJOR);
    }
}
