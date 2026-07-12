use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use std::time::Duration;

use chrono::Utc;
use sqlx::{MySqlConnection, MySqlPool, mysql::MySqlPoolOptions};
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

const LOCAL_SEED_LOCK: &str = "v2board-native-local-seed";
const LOCAL_SEED_ADMIN_EMAIL: &str = "admin@example.com";
const LOCAL_SEED_ADMIN_PASSWORD: &str = "12345678";
const RETIRED_LOCAL_SEED_ADMIN_EMAIL: &str = "admin@local";

pub type DbPool = MySqlPool;

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
    #[error(
        "migration 3 preflight failed: {duplicate_user_count} users have multiple unfinished orders (status 0/1); sample user ids: {sample_user_ids:?}. Resolve those orders explicitly before migrating"
    )]
    UnfinishedOrderDuplicates {
        duplicate_user_count: i64,
        sample_user_ids: Vec<i32>,
    },
    #[error(
        "migration 5 preflight failed: {malformed_giftcard_count} gift cards have malformed used_user_ids JSON; sample giftcard ids: {sample_giftcard_ids:?}. Repair each value to a JSON array of integer user ids before migrating"
    )]
    MalformedGiftcardRedemptions {
        malformed_giftcard_count: usize,
        sample_giftcard_ids: Vec<i64>,
    },
    #[error("password hash error: {0}")]
    Password(String),
    #[error("timed out acquiring the local seed lock")]
    SeedLockUnavailable,
    #[error("lost the local seed lock before it could be released")]
    SeedLockLost,
}

pub async fn connect_mysql(database_url: &str) -> Result<DbPool, DbInitError> {
    connect_mysql_with_config(database_url, &DbPoolConfig::from_env()?).await
}

pub async fn connect_mysql_with_config(
    database_url: &str,
    config: &DbPoolConfig,
) -> Result<DbPool, DbInitError> {
    let pool = MySqlPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .max_lifetime(Some(config.max_lifetime))
        .test_before_acquire(true)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn migrate_mysql(pool: &DbPool) -> Result<(), DbInitError> {
    preflight_unfinished_order_uniqueness(pool).await?;
    preflight_giftcard_redemptions(pool).await?;
    MIGRATOR.run(pool).await?;
    if local_seed_enabled() {
        seed_local(pool).await?;
    }
    Ok(())
}

async fn preflight_giftcard_redemptions(pool: &DbPool) -> Result<(), DbInitError> {
    if !table_exists(pool, "v2_giftcard").await? {
        return Ok(());
    }
    let migration_5_applied = if table_exists(pool, "_sqlx_migrations").await? {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = 5 AND success = TRUE)",
        )
        .fetch_one(pool)
        .await?
            != 0
    } else {
        false
    };
    if migration_5_applied {
        return Ok(());
    }

    let legacy_rows = sqlx::query_as::<_, (i64, Option<String>)>(
        "SELECT id, used_user_ids FROM v2_giftcard ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let malformed = malformed_giftcard_redemption_ids(&legacy_rows);
    if malformed.is_empty() {
        return Ok(());
    }

    Err(DbInitError::MalformedGiftcardRedemptions {
        malformed_giftcard_count: malformed.len(),
        sample_giftcard_ids: malformed.into_iter().take(10).collect(),
    })
}

fn malformed_giftcard_redemption_ids(rows: &[(i64, Option<String>)]) -> Vec<i64> {
    rows.iter()
        .filter_map(|(id, raw)| {
            raw.as_deref()
                .and_then(|raw| serde_json::from_str::<Vec<i64>>(raw).err())
                .map(|_| *id)
        })
        .collect()
}

async fn preflight_unfinished_order_uniqueness(pool: &DbPool) -> Result<(), DbInitError> {
    if !table_exists(pool, "v2_order").await? {
        return Ok(());
    }
    let migration_3_applied = if table_exists(pool, "_sqlx_migrations").await? {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = 3 AND success = TRUE)",
        )
        .fetch_one(pool)
        .await?
            != 0
    } else {
        false
    };
    if migration_3_applied {
        return Ok(());
    }

    let duplicate_user_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM (
            SELECT user_id
            FROM v2_order
            WHERE status IN (0, 1)
            GROUP BY user_id
            HAVING COUNT(*) > 1
        ) AS duplicate_users
        "#,
    )
    .fetch_one(pool)
    .await?;
    if duplicate_user_count == 0 {
        return Ok(());
    }

    let sample_user_ids = sqlx::query_scalar::<_, i32>(
        r#"
        SELECT user_id
        FROM v2_order
        WHERE status IN (0, 1)
        GROUP BY user_id
        HAVING COUNT(*) > 1
        ORDER BY user_id
        LIMIT 10
        "#,
    )
    .fetch_all(pool)
    .await?;
    Err(DbInitError::UnfinishedOrderDuplicates {
        duplicate_user_count,
        sample_user_ids,
    })
}

async fn table_exists(pool: &DbPool, table_name: &str) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, i64>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = DATABASE() AND table_name = ?
        )
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?
        != 0)
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

fn local_seed_enabled() -> bool {
    std::env::var("V2BOARD_SEED_LOCAL")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

async fn seed_local(pool: &MySqlPool) -> Result<(), DbInitError> {
    let mut connection = pool.acquire().await?;
    let acquired = sqlx::query_scalar::<_, Option<i64>>("SELECT GET_LOCK(?, 30)")
        .bind(LOCAL_SEED_LOCK)
        .fetch_one(&mut *connection)
        .await?;
    if acquired != Some(1) {
        return Err(DbInitError::SeedLockUnavailable);
    }

    let seed_result = seed_local_locked(&mut connection).await;
    let released = sqlx::query_scalar::<_, Option<i64>>("SELECT RELEASE_LOCK(?)")
        .bind(LOCAL_SEED_LOCK)
        .fetch_one(&mut *connection)
        .await;

    seed_result?;
    if released? != Some(1) {
        return Err(DbInitError::SeedLockLost);
    }
    Ok(())
}

async fn seed_local_locked(connection: &mut MySqlConnection) -> Result<(), DbInitError> {
    let now = Utc::now().timestamp();
    let current_admin_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM v2_user WHERE email = ? LIMIT 1)")
            .bind(LOCAL_SEED_ADMIN_EMAIL)
            .fetch_one(&mut *connection)
            .await?;
    if !current_admin_exists {
        // Preserve the original local seed's id, password, and related test data
        // while moving it to an address accepted by the frontend email contract.
        // This is an upgrade migration, not an authentication fallback.
        sqlx::query(
            "UPDATE v2_user SET email = ?, updated_at = ? \
             WHERE email = ? AND is_admin = 1 LIMIT 1",
        )
        .bind(LOCAL_SEED_ADMIN_EMAIL)
        .bind(now)
        .bind(RETIRED_LOCAL_SEED_ADMIN_EMAIL)
        .execute(&mut *connection)
        .await?;
    }

    let admin_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM v2_user WHERE email = ? LIMIT 1)")
            .bind(LOCAL_SEED_ADMIN_EMAIL)
            .fetch_one(&mut *connection)
            .await?;
    if !admin_exists {
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
            SELECT ?, ?, ?, ?, 1, ?, ? FROM DUAL
            WHERE NOT EXISTS (
                SELECT 1 FROM v2_user WHERE email = ? LIMIT 1
            )
            "#,
        )
        .bind(LOCAL_SEED_ADMIN_EMAIL)
        .bind(password)
        .bind(uuid)
        .bind(token)
        .bind(now)
        .bind(now)
        .bind(LOCAL_SEED_ADMIN_EMAIL)
        .execute(&mut *connection)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO v2_server_group (name, created_at, updated_at)
        SELECT 'Default Group', ?, ? FROM DUAL
        WHERE NOT EXISTS (SELECT 1 FROM v2_server_group LIMIT 1)
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
            group_id, transfer_enable, name, `show`, sort, renew, content,
            month_price, quarter_price, half_year_price, year_price,
            onetime_price, created_at, updated_at
        )
        SELECT ?, 100, 'Test Plan', 1, 1, 1, 'Local Rust test plan',
               100, 280, 540, 1000, 9900, ?, ? FROM DUAL
        WHERE NOT EXISTS (SELECT 1 FROM v2_plan LIMIT 1)
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
            language, category, title, body, sort, `show`, created_at, updated_at
        )
        SELECT 'zh-CN', '使用文档', '本地开发环境快速开始',
               ?,
               1, 1, ?, ? FROM DUAL
        WHERE NOT EXISTS (SELECT 1 FROM v2_knowledge LIMIT 1)
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
    fn unfinished_order_preflight_error_is_actionable_without_mutating_orders() {
        let error = DbInitError::UnfinishedOrderDuplicates {
            duplicate_user_count: 3,
            sample_user_ids: vec![7, 11, 42],
        };
        let message = error.to_string();
        assert!(message.contains("3 users"));
        assert!(message.contains("[7, 11, 42]"));
        assert!(message.contains("Resolve those orders explicitly"));
    }

    #[test]
    fn giftcard_preflight_error_is_actionable_and_fail_closed() {
        let error = DbInitError::MalformedGiftcardRedemptions {
            malformed_giftcard_count: 2,
            sample_giftcard_ids: vec![4, 9],
        };
        let message = error.to_string();
        assert!(message.contains("2 gift cards"));
        assert!(message.contains("[4, 9]"));
        assert!(message.contains("JSON array of integer user ids"));
    }

    #[test]
    fn giftcard_preflight_accepts_only_integer_arrays_or_null() {
        let rows = vec![
            (1, None),
            (2, Some("[]".to_string())),
            (3, Some("[7,11]".to_string())),
            (4, Some(String::new())),
            (5, Some("{}".to_string())),
            (6, Some("[\"7\"]".to_string())),
            (7, Some("[1.5]".to_string())),
        ];
        assert_eq!(malformed_giftcard_redemption_ids(&rows), vec![4, 5, 6, 7]);
    }

    #[test]
    fn giftcard_migration_is_normalized_and_removes_the_legacy_column() {
        let migration = include_str!("../../../migrations/0005_normalize_giftcard_redemptions.sql");
        assert!(migration.contains("CREATE TABLE `v2_giftcard_redemption`"));
        assert!(migration.contains("PRIMARY KEY (`giftcard_id`, `user_id`)"));
        assert!(migration.contains("`created_at` bigint NOT NULL"));
        assert!(migration.contains("ERROR ON EMPTY ERROR ON ERROR"));
        assert!(migration.contains("DROP COLUMN `used_user_ids`"));
    }

    #[test]
    fn mail_outbox_migration_is_transactional_idempotent_and_leased() {
        let migration = include_str!("../../../migrations/0006_durable_mail_outbox.sql");
        assert!(migration.contains("PRIMARY KEY (`batch_key`)"));
        assert!(migration.contains("`actor` varchar(512) NOT NULL"));
        assert!(migration.contains("uniq_mail_outbox_batch_recipient"));
        assert!(migration.contains("uniq_mail_outbox_message_id"));
        assert!(migration.contains("FOREIGN KEY (`batch_key`)"));
        assert!(migration.contains("ON DELETE CASCADE"));
        assert!(migration.contains("idx_mail_outbox_claim"));
        assert!(migration.contains("`last_error` text DEFAULT NULL"));
        assert_eq!(migration.matches("`sender` varchar(512)").count(), 1);
        assert_eq!(migration.matches("`subject` mediumtext").count(), 1);
        assert_eq!(migration.matches("`body` mediumtext").count(), 1);
        assert_eq!(migration.matches("`template_name` varchar(255)").count(), 1);
        assert!(!migration.contains("`sent_at`"));
    }

    #[test]
    fn retired_redis_traffic_ledger_is_removed_by_a_forward_migration() {
        let migration =
            include_str!("../../../migrations/0008_drop_retired_redis_traffic_ledger.sql");
        assert!(migration.contains("DROP TABLE `v2_traffic_batch`"));
    }

    #[sqlx::test(migrations = "../../migrations")]
    #[ignore = "requires a disposable MySQL server via DATABASE_URL"]
    async fn migration_readiness_tracks_the_real_sqlx_table(pool: MySqlPool) {
        assert!(migrations_current(&pool).await.unwrap());

        let latest = MIGRATOR
            .iter()
            .map(|migration| migration.version)
            .max()
            .unwrap();
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(latest)
            .execute(&pool)
            .await
            .unwrap();
        assert!(!migrations_current(&pool).await.unwrap());

        sqlx::query("DROP TABLE _sqlx_migrations")
            .execute(&pool)
            .await
            .unwrap();
        assert!(migrations_current(&pool).await.is_err());
    }
}
