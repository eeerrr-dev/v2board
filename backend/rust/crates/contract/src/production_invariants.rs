use std::{
    collections::{BTreeMap, HashMap},
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use redis::AsyncCommands;
use sqlx::{AssertSqlSafe, MySqlPool, mysql::MySqlPoolOptions};
use tokio::task::JoinSet;
use url::Url;
use uuid::Uuid;
use v2board_config::{AppConfig, RuntimeEnvironment};
use v2board_db::{DbPoolConfig, migrations_current};
use v2board_domain::{
    admin::{AdminOutput, AdminService},
    auth::{AuthService, PasswordKdf, RegisterInput, hash_password},
    order::{OrderService, PaymentNotifyInput},
    server_credentials::{derive_node_token, verify_node_token},
    smtp::SmtpTransportCache,
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

const MIGRATION_SQL: &[&str] = &[
    include_str!("../../../migrations/0001_initial.sql"),
    include_str!("../../../migrations/0002_auth_hardening.sql"),
    include_str!("../../../migrations/0003_worker_idempotency.sql"),
    include_str!("../../../migrations/0004_traffic_report_sha256.sql"),
    include_str!("../../../migrations/0005_normalize_giftcard_redemptions.sql"),
    include_str!("../../../migrations/0006_durable_mail_outbox.sql"),
    include_str!("../../../migrations/0007_index_payment_pending_orders.sql"),
    include_str!("../../../migrations/0008_drop_retired_redis_traffic_ledger.sql"),
    include_str!("../../../migrations/0009_traffic_quota_epoch.sql"),
    include_str!("../../../migrations/0010_business_invariants.sql"),
    include_str!("../../../migrations/0011_payment_reconciliation.sql"),
    include_str!("../../../migrations/0012_relational_integrity.sql"),
    include_str!("../../../migrations/0013_traffic_report_epoch.sql"),
    include_str!("../../../migrations/0014_coupon_code_unique.sql"),
    include_str!("../../../migrations/0015_giftcard_code_unique.sql"),
    include_str!("../../../migrations/0016_invite_code_invariants.sql"),
    include_str!("../../../migrations/0017_payment_driver_uuid_unique.sql"),
    include_str!("../../../migrations/0018_ticket_open_invariants.sql"),
    include_str!("../../../migrations/0019_ticket_message_index.sql"),
    include_str!("../../../migrations/0020_node_credentials.sql"),
    include_str!("../../../migrations/0021_seed_node_credentials.sql"),
    include_str!("../../../migrations/0022_order_relational_integrity.sql"),
    include_str!("../../../migrations/0023_plan_group_integrity.sql"),
    include_str!("../../../migrations/0024_user_relational_integrity.sql"),
    include_str!("../../../migrations/0025_giftcard_plan_integrity.sql"),
    include_str!("../../../migrations/0026_invite_user_integrity.sql"),
    include_str!("../../../migrations/0027_ticket_user_integrity.sql"),
    include_str!("../../../migrations/0028_ticket_message_integrity.sql"),
    include_str!("../../../migrations/0029_giftcard_redemption_user_integrity.sql"),
    include_str!("../../../migrations/0030_traffic_report_user_integrity.sql"),
    include_str!("../../../migrations/0031_stat_user_index.sql"),
    include_str!("../../../migrations/0032_shadowsocks_group_json.sql"),
    include_str!("../../../migrations/0033_vmess_group_json.sql"),
    include_str!("../../../migrations/0034_trojan_group_json.sql"),
    include_str!("../../../migrations/0035_tuic_group_json.sql"),
    include_str!("../../../migrations/0036_hysteria_group_json.sql"),
    include_str!("../../../migrations/0037_vless_group_json.sql"),
    include_str!("../../../migrations/0038_anytls_group_json.sql"),
    include_str!("../../../migrations/0039_v2node_group_json.sql"),
    include_str!("../../../migrations/0040_archive_payment_methods.sql"),
    include_str!("../../../migrations/0041_reconciliation_payment_integrity.sql"),
    include_str!("../../../migrations/0042_order_callback_identity.sql"),
    include_str!("../../../migrations/0043_legacy_callback_identity_bridge.sql"),
];

const DEFAULT_ROOT_DATABASE_URL: &str = "mysql://root:v2board@mysql:3306/mysql";
const DEFAULT_RUNTIME_REDIS_URL: &str = "redis://redis:6379/1";
const DEFAULT_INTEGRATION_REDIS_URL: &str = "redis://redis:6379/15";
const DEFAULT_WORKER_BIN: &str = "/app/target/debug/v2board-workers";

pub async fn run() -> Result<()> {
    let root_database_url = env_or(
        "RUST_INTEGRATION_DATABASE_ROOT_URL",
        DEFAULT_ROOT_DATABASE_URL,
    );
    let runtime_redis_url = env_or("REDIS_URL", DEFAULT_RUNTIME_REDIS_URL);
    let integration_redis_url = env_or("RUST_INTEGRATION_REDIS_URL", DEFAULT_INTEGRATION_REDIS_URL);
    ensure!(
        runtime_redis_url != integration_redis_url,
        "RUST_INTEGRATION_REDIS_URL must select a Redis database isolated from REDIS_URL"
    );

    let database_name = format!(
        "v2board_contract_{}",
        &Uuid::new_v4().simple().to_string()[..16]
    );
    let database_url = database_url_for(&root_database_url, &database_name)?;
    let root = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&root_database_url)
        .await
        .context("connect to the disposable-database administrator")?;
    migration_preflight_failure_modes(&root, &root_database_url).await?;
    pass("migration preflights reject duplicate, orphan, and malformed legacy state");
    create_database(&root, &database_name).await?;

    let pool_config = DbPoolConfig {
        min_connections: 1,
        max_connections: 40,
        acquire_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
    };
    let pool = v2board_db::connect_mysql_with_config(&database_url, &pool_config)
        .await
        .context("connect to the disposable integration database")?;
    let result =
        run_isolated_checks(&pool, &database_url, &database_name, &integration_redis_url).await;

    pool.close().await;
    let drop_result = drop_database(&root, &database_name).await;
    root.close().await;

    match (result, drop_result) {
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "also failed to drop disposable database {database_name}: {cleanup:#}"
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup)) => Err(cleanup),
        (Ok(()), Ok(())) => {
            println!("Production invariant gate passed; disposable state was removed.");
            Ok(())
        }
    }
}

async fn run_isolated_checks(
    pool: &MySqlPool,
    database_url: &str,
    database_name: &str,
    integration_redis_url: &str,
) -> Result<()> {
    let integration_redis = redis::Client::open(integration_redis_url)?;
    flush_redis(&integration_redis).await?;

    let result = async {
        audit_forward_migration_ddl_boundaries()?;
        crate::sql_schema_prepare::audit_dynamic_inventory()?;
        MIGRATOR
            .run(pool)
            .await
            .context("apply every embedded migration to a fresh MySQL database")?;
        ensure!(
            migrations_current(pool).await?,
            "freshly applied migration ledger is not current"
        );
        schema_invariants(pool).await?;
        pass("fresh migrations and production schema constraints");

        crate::sql_schema_prepare::run(pool).await?;
        pass("static runtime SQL prepares against the migrated production schema");

        traffic_epoch_invariant(pool, database_url, database_name, integration_redis_url).await?;
        pass("traffic epoch rejects delayed pre-reset reports");

        invite_single_consumption(pool, integration_redis_url).await?;
        pass("single-use invite remains single-use under concurrency");
        flush_redis(&integration_redis).await?;

        ticket_state_machine(pool, database_url, database_name, integration_redis_url).await?;
        pass("one-open-ticket and reply/auto-close serialization");

        late_payment_reconciliation(pool, database_url, integration_redis_url).await?;
        pass("late authenticated payment reconciliation is durable and idempotent");

        node_identity_epoch(pool).await?;
        pass("node credentials are bound to identity and revocation epoch");

        auth_rate_limits(pool, database_url, integration_redis_url).await?;
        pass("registration and login reservations are atomic in Redis");
        flush_redis(&integration_redis).await?;

        redis_lease_ownership(&integration_redis).await?;
        pass("a stale worker lease owner cannot renew or release a replacement lease");

        worker_health_process(database_url, database_name, integration_redis_url).await?;
        pass("a live isolated worker publishes health and per-loop heartbeats");

        migration_readiness_failure_modes(pool).await?;
        pass("migration readiness fails closed for missing or corrupt ledger state");
        Ok(())
    }
    .await;

    let cleanup = flush_redis(&integration_redis).await;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "also failed to flush the isolated integration Redis database: {cleanup:#}"
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup)) => Err(cleanup),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn audit_forward_migration_ddl_boundaries() -> Result<()> {
    for (index, migration) in MIGRATION_SQL.iter().enumerate().skip(8) {
        let persistent_ddl = migration
            .lines()
            .map(str::trim_start)
            .filter(|line| {
                line.starts_with("ALTER TABLE")
                    || line.starts_with("CREATE TABLE")
                    || line.starts_with("DROP TABLE")
                    || line.starts_with("RENAME TABLE")
                    || line.starts_with("CREATE TRIGGER")
                    || line.starts_with("DROP TRIGGER")
            })
            .count();
        ensure!(
            persistent_ddl <= 1,
            "forward migration version {} contains {persistent_ddl} irreversible DDL statements; split it so MySQL failure is retryable",
            index + 1
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PreflightFailureCase {
    DuplicateBusinessCode,
    OrphanOrder,
    NonArrayNodeGroup,
}

async fn migration_preflight_failure_modes(
    root: &MySqlPool,
    root_database_url: &str,
) -> Result<()> {
    for case in [
        PreflightFailureCase::DuplicateBusinessCode,
        PreflightFailureCase::OrphanOrder,
        PreflightFailureCase::NonArrayNodeGroup,
    ] {
        run_preflight_failure_case(root, root_database_url, case).await?;
    }
    Ok(())
}

async fn run_preflight_failure_case(
    root: &MySqlPool,
    root_database_url: &str,
    case: PreflightFailureCase,
) -> Result<()> {
    let suffix = match case {
        PreflightFailureCase::DuplicateBusinessCode => "duplicate",
        PreflightFailureCase::OrphanOrder => "orphan",
        PreflightFailureCase::NonArrayNodeGroup => "nodejson",
    };
    let database_name = format!(
        "v2board_preflight_{suffix}_{}",
        &Uuid::new_v4().simple().to_string()[..10]
    );
    create_database(root, &database_name).await?;
    let database_url = database_url_for(root_database_url, &database_name)?;
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let result = exercise_preflight_failure(&pool, case).await;
    pool.close().await;
    let cleanup = drop_database(root, &database_name).await;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "also failed to drop preflight database {database_name}: {cleanup:#}"
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup)) => Err(cleanup),
        (Ok(()), Ok(())) => Ok(()),
    }
}

async fn exercise_preflight_failure(pool: &MySqlPool, case: PreflightFailureCase) -> Result<()> {
    let mut connection = pool.acquire().await?;
    let migrations_before_failure = match case {
        PreflightFailureCase::DuplicateBusinessCode => 9,
        PreflightFailureCase::OrphanOrder | PreflightFailureCase::NonArrayNodeGroup => 11,
    };
    for migration in MIGRATION_SQL.iter().take(migrations_before_failure) {
        sqlx::raw_sql(*migration).execute(&mut *connection).await?;
    }

    match case {
        PreflightFailureCase::DuplicateBusinessCode => {
            sqlx::query(
                r#"
                INSERT INTO v2_coupon (
                    code, name, type, value, `show`, started_at, ended_at, created_at, updated_at
                ) VALUES
                    ('DUPLICATE-INTEGRATION', 'first', 1, 100, 0, 0, 1, 0, 0),
                    ('DUPLICATE-INTEGRATION', 'second', 1, 100, 0, 0, 1, 0, 0)
                "#,
            )
            .execute(&mut *connection)
            .await?;
        }
        PreflightFailureCase::OrphanOrder => {
            sqlx::query(
                r#"
                INSERT INTO v2_order (
                    user_id, plan_id, type, period, trade_no, total_amount, status,
                    commission_status, commission_balance, created_at, updated_at
                ) VALUES (987654321, 0, 1, 'deposit', ?, 100, 2, 0, 0, 0, 0)
                "#,
            )
            .bind(Uuid::new_v4().hyphenated().to_string())
            .execute(&mut *connection)
            .await?;
        }
        PreflightFailureCase::NonArrayNodeGroup => {
            sqlx::query(
                r#"
                INSERT INTO v2_server_shadowsocks (
                    group_id, name, rate, host, port, server_port, cipher, created_at, updated_at
                ) VALUES ('{}', 'malformed group integration node', '1', '127.0.0.1',
                          '443', 443, 'aes-128-gcm', 0, 0)
                "#,
            )
            .execute(&mut *connection)
            .await?;
        }
    }

    let failing_migration = match case {
        PreflightFailureCase::DuplicateBusinessCode => MIGRATION_SQL[9],
        PreflightFailureCase::OrphanOrder | PreflightFailureCase::NonArrayNodeGroup => {
            MIGRATION_SQL[11]
        }
    };
    let error = sqlx::raw_sql(failing_migration)
        .execute(&mut *connection)
        .await
        .expect_err("migration preflight accepted invalid legacy state");
    let error_text = error.to_string();
    let expected_guard = match case {
        PreflightFailureCase::DuplicateBusinessCode => "business_invariant_preflight_failed",
        PreflightFailureCase::OrphanOrder | PreflightFailureCase::NonArrayNodeGroup => {
            "relational_integrity_preflight_failed"
        }
    };
    ensure!(
        error_text.contains(expected_guard) || error_text.contains("Duplicate entry"),
        "migration failed for an unexpected reason: {error_text}"
    );

    let persistent_ddl_count: i64 = match case {
        PreflightFailureCase::DuplicateBusinessCode => {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM information_schema.statistics
                WHERE table_schema = DATABASE() AND table_name = 'v2_coupon'
                  AND index_name = 'uniq_coupon_code'
                "#,
            )
            .fetch_one(&mut *connection)
            .await?
        }
        PreflightFailureCase::OrphanOrder | PreflightFailureCase::NonArrayNodeGroup => {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM information_schema.columns
                WHERE table_schema = DATABASE() AND table_name = 'v2_order'
                  AND column_name = 'referenced_plan_id'
                "#,
            )
            .fetch_one(&mut *connection)
            .await?
        }
    };
    ensure!(
        persistent_ddl_count == 0,
        "preflight failure occurred after persistent migration DDL was applied"
    );
    Ok(())
}

async fn schema_invariants(pool: &MySqlPool) -> Result<()> {
    for (table, index) in [
        ("v2_coupon", "uniq_coupon_code"),
        ("v2_giftcard", "uniq_giftcard_code"),
        ("v2_invite_code", "uniq_invite_code"),
        ("v2_payment", "uniq_payment_driver_uuid"),
        ("v2_ticket", "uniq_ticket_open_user"),
        (
            "v2_payment_reconciliation",
            "uniq_payment_reconciliation_callback",
        ),
    ] {
        let unique_columns: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM information_schema.statistics
            WHERE table_schema = DATABASE()
              AND table_name = ?
              AND index_name = ?
              AND non_unique = 0
            "#,
        )
        .bind(table)
        .bind(index)
        .fetch_one(pool)
        .await?;
        ensure!(unique_columns > 0, "missing unique index {table}.{index}");
    }
    for (table, column) in [
        ("v2_user", "traffic_epoch"),
        ("v2_server_traffic_report_item", "traffic_epoch"),
        ("v2_ticket", "open_user_id"),
        ("v2_server_credential", "credential_epoch"),
        ("v2_payment", "archived_at"),
        ("v2_payment_reconciliation", "trade_no_hash"),
        ("v2_payment_reconciliation", "callback_no_hash"),
        ("v2_order", "callback_no_hash"),
    ] {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM information_schema.columns
            WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?
            "#,
        )
        .bind(table)
        .bind(column)
        .fetch_one(pool)
        .await?;
        ensure!(count == 1, "missing required column {table}.{column}");
    }
    let reconciliation_payment_fk: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.referential_constraints
        WHERE constraint_schema = DATABASE()
          AND table_name = 'v2_payment_reconciliation'
          AND constraint_name = 'fk_payment_reconciliation_payment'
          AND referenced_table_name = 'v2_payment'
          AND delete_rule = 'RESTRICT'
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        reconciliation_payment_fk == 1,
        "payment reconciliation does not retain its verification version"
    );
    let expected_amount_type: String = sqlx::query_scalar(
        r#"
        SELECT data_type
        FROM information_schema.columns
        WHERE table_schema = DATABASE()
          AND table_name = 'v2_payment_reconciliation'
          AND column_name = 'expected_amount'
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        expected_amount_type == "bigint",
        "payment reconciliation expected_amount is not bigint"
    );
    let callback_bridge_trigger: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM information_schema.triggers
        WHERE trigger_schema = DATABASE()
          AND event_object_table = 'v2_order'
          AND trigger_name = 'v2_order_callback_identity_before_update'
          AND action_timing = 'BEFORE'
          AND event_manipulation = 'UPDATE'
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        callback_bridge_trigger == 1,
        "missing legacy callback identity rolling-deploy bridge"
    );
    Ok(())
}

async fn traffic_epoch_invariant(
    pool: &MySqlPool,
    database_url: &str,
    database_name: &str,
    redis_url: &str,
) -> Result<()> {
    let user_id = insert_user(pool, "traffic", "not-used").await?;
    sqlx::query("UPDATE v2_user SET u = 11, d = 13 WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

    let stale_key = random_traffic_key();
    insert_traffic_report(pool, &stale_key, user_id, 0, 101, 103).await?;
    let reset = sqlx::query(
        "UPDATE v2_user SET u = 0, d = 0, traffic_epoch = traffic_epoch + 1 WHERE id = ?",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    ensure!(
        reset.rows_affected() == 1,
        "traffic reset did not update its user"
    );

    run_worker_once(database_url, database_name, redis_url, "traffic_update").await?;
    let (u, d, epoch): (i64, i64, i64) =
        sqlx::query_as("SELECT u, d, traffic_epoch FROM v2_user WHERE id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    ensure!(
        (u, d, epoch) == (0, 0, 1),
        "stale report crossed the reset epoch"
    );
    assert_report_consumed(pool, &stale_key).await?;

    let current_key = random_traffic_key();
    insert_traffic_report(pool, &current_key, user_id, epoch, 7, 9).await?;
    run_worker_once(database_url, database_name, redis_url, "traffic_update").await?;
    let (u, d): (i64, i64) = sqlx::query_as("SELECT u, d FROM v2_user WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    ensure!(
        (u, d) == (7, 9),
        "current-epoch report was not applied exactly once"
    );
    assert_report_consumed(pool, &current_key).await?;
    Ok(())
}

async fn insert_traffic_report(
    pool: &MySqlPool,
    report_key: &str,
    user_id: i64,
    epoch: i64,
    u: i64,
    d: i64,
) -> Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO v2_server_traffic_report \
         (report_key, payload_hash, applied_at, created_at, updated_at) \
         VALUES (?, ?, NULL, ?, ?)",
    )
    .bind(report_key)
    .bind(random_traffic_key())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO v2_server_traffic_report_item \
         (report_key, user_id, traffic_epoch, u, d) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(report_key)
    .bind(user_id)
    .bind(epoch)
    .bind(u)
    .bind(d)
    .execute(pool)
    .await?;
    Ok(())
}

async fn assert_report_consumed(pool: &MySqlPool, report_key: &str) -> Result<()> {
    let applied_at: Option<i64> =
        sqlx::query_scalar("SELECT applied_at FROM v2_server_traffic_report WHERE report_key = ?")
            .bind(report_key)
            .fetch_one(pool)
            .await?;
    let item_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_server_traffic_report_item WHERE report_key = ?",
    )
    .bind(report_key)
    .fetch_one(pool)
    .await?;
    ensure!(applied_at.is_some(), "traffic report was not acknowledged");
    ensure!(
        item_count == 0,
        "consumed traffic report retained payload rows"
    );
    Ok(())
}

async fn invite_single_consumption(pool: &MySqlPool, redis_url: &str) -> Result<()> {
    let inviter_id = insert_user(pool, "inviter", "not-used").await?;
    let invite_code = Uuid::new_v4().simple().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO v2_invite_code (user_id, code, status, pv, created_at, updated_at) \
         VALUES (?, ?, 0, 0, ?, ?)",
    )
    .bind(inviter_id)
    .bind(&invite_code)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    let mut config = integration_config(pool, redis_url)?;
    config.invite_force = true;
    config.invite_never_expire = false;
    config.register_limit_by_ip_enable = false;
    let auth = auth_service(pool, redis_url, config).await?;
    let mut attempts = JoinSet::new();
    for sequence in 0..6 {
        let auth = auth.clone();
        let invite_code = invite_code.clone();
        attempts.spawn(async move {
            auth.register(
                RegisterInput {
                    email: format!("invite-{sequence}-{}@example.test", Uuid::new_v4().simple()),
                    password: "integration-password".to_string(),
                    invite_code: Some(invite_code),
                    email_code: None,
                    recaptcha_data: None,
                },
                Some(format!("198.51.100.{}", sequence + 1)),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut success = 0;
    let mut rejected = 0;
    while let Some(joined) = attempts.join_next().await {
        match joined? {
            Ok(_) => success += 1,
            Err(error) if error.to_string().contains("Invalid invitation code") => rejected += 1,
            Err(error) => bail!("unexpected invitation registration error: {error:#}"),
        }
    }
    ensure!(
        (success, rejected) == (1, 5),
        "invite code admitted {success} registrations"
    );
    let status: i8 = sqlx::query_scalar("SELECT status FROM v2_invite_code WHERE code = ?")
        .bind(&invite_code)
        .fetch_one(pool)
        .await?;
    let invited: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE invite_user_id = ?")
        .bind(inviter_id)
        .fetch_one(pool)
        .await?;
    ensure!(
        status == 1 && invited == 1,
        "invite consumption was not atomically persisted"
    );
    Ok(())
}

async fn ticket_state_machine(
    pool: &MySqlPool,
    database_url: &str,
    database_name: &str,
    redis_url: &str,
) -> Result<()> {
    use v2board_db::ticket::{TicketCreateOutcome, UserTicketReplyOutcome};

    let user_id = insert_user(pool, "ticket", "not-used").await?;
    let now = Utc::now().timestamp();
    let mut creates = JoinSet::new();
    for sequence in 0..8 {
        let pool = pool.clone();
        creates.spawn(async move {
            v2board_db::ticket::create_ticket(
                &pool,
                user_id,
                &format!("concurrent ticket {sequence}"),
                1,
                "initial message",
                now,
                false,
            )
            .await
        });
    }
    let mut created = 0;
    let mut existed = 0;
    while let Some(joined) = creates.join_next().await {
        match joined?? {
            TicketCreateOutcome::Created => created += 1,
            TicketCreateOutcome::OpenTicketExists => existed += 1,
            outcome => bail!("unexpected concurrent ticket outcome: {outcome:?}"),
        }
    }
    ensure!(
        (created, existed) == (1, 7),
        "one-open-ticket invariant failed"
    );
    let first_ticket: i32 =
        sqlx::query_scalar("SELECT id FROM v2_ticket WHERE user_id = ? AND status = 0")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    ensure!(
        v2board_db::ticket::close_ticket(pool, user_id, first_ticket, now + 1).await?,
        "failed to close the ticket used by the uniqueness check"
    );

    ensure!(
        v2board_db::ticket::create_ticket(
            pool,
            user_id,
            "reply race",
            1,
            "user opening message",
            now - 90_000,
            false,
        )
        .await?
            == TicketCreateOutcome::Created,
        "failed to create reply-race ticket"
    );
    let race_ticket: i32 =
        sqlx::query_scalar("SELECT id FROM v2_ticket WHERE user_id = ? AND status = 0")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    insert_operator_reply(pool, race_ticket, now - 90_000).await?;

    let worker = tokio::spawn(run_worker_once(
        database_url.to_string(),
        database_name.to_string(),
        redis_url.to_string(),
        "check_ticket",
    ));
    let reply = v2board_db::ticket::reply_ticket(
        pool,
        race_ticket,
        user_id,
        "reply concurrent with auto-close",
        Utc::now().timestamp(),
    )
    .await?;
    worker.await??;
    ensure!(
        reply == UserTicketReplyOutcome::Replied,
        "fresh user reply lost its row lock"
    );
    let race_status: i8 = sqlx::query_scalar("SELECT status FROM v2_ticket WHERE id = ?")
        .bind(race_ticket)
        .fetch_one(pool)
        .await?;
    ensure!(
        race_status == 0,
        "auto-close closed a ticket after a fresh user reply"
    );
    v2board_db::ticket::close_ticket(pool, user_id, race_ticket, now + 2).await?;

    ensure!(
        v2board_db::ticket::create_ticket(
            pool,
            user_id,
            "stale answered ticket",
            1,
            "old user opening message",
            now - 90_000,
            false,
        )
        .await?
            == TicketCreateOutcome::Created,
        "failed to create stale ticket"
    );
    let stale_ticket: i32 =
        sqlx::query_scalar("SELECT id FROM v2_ticket WHERE user_id = ? AND status = 0")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    insert_operator_reply(pool, stale_ticket, now - 90_000).await?;
    run_worker_once(database_url, database_name, redis_url, "check_ticket").await?;
    let stale_status: i8 = sqlx::query_scalar("SELECT status FROM v2_ticket WHERE id = ?")
        .bind(stale_ticket)
        .fetch_one(pool)
        .await?;
    ensure!(
        stale_status == 1,
        "genuinely stale answered ticket was not closed"
    );
    Ok(())
}

async fn insert_operator_reply(pool: &MySqlPool, ticket_id: i32, timestamp: i64) -> Result<()> {
    sqlx::query(
        "INSERT INTO v2_ticket_message (user_id, ticket_id, message, created_at, updated_at) \
         VALUES (0, ?, 'operator reply', ?, ?)",
    )
    .bind(ticket_id)
    .bind(timestamp)
    .bind(timestamp)
    .execute(pool)
    .await?;
    sqlx::query("UPDATE v2_ticket SET status = 0, reply_status = 1, updated_at = ? WHERE id = ?")
        .bind(timestamp)
        .bind(ticket_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn late_payment_reconciliation(
    pool: &MySqlPool,
    database_url: &str,
    redis_url: &str,
) -> Result<()> {
    let user_id = insert_user(pool, "late-payment", "not-used").await?;
    let payment_uuid = Uuid::new_v4().simple().to_string();
    let now = Utc::now().timestamp();
    let payment = sqlx::query(
        r#"
        INSERT INTO v2_payment (
            uuid, payment, name, config, enable, created_at, updated_at
        ) VALUES (?, 'EPay', 'integration EPay', ?, 1, ?, ?)
        "#,
    )
    .bind(&payment_uuid)
    .bind(r#"{"key":"epay-secret","pid":"integration","url":"https://pay.invalid"}"#)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let payment_id = payment.last_insert_id() as i32;
    let trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        r#"
        INSERT INTO v2_order (
            user_id, plan_id, payment_id, type, period, trade_no, total_amount,
            status, commission_status, commission_balance, created_at, updated_at
        ) VALUES (?, 0, ?, 1, 'deposit', ?, 1234, 2, 0, 0, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(payment_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    // Exercise the real admin path: drop must archive rather than delete, hide
    // the version from ordinary reads, and retain it for delayed callbacks.
    let admin = AdminService::new(
        pool.clone(),
        redis::Client::open(redis_url)?,
        Arc::new(integration_config(pool, redis_url)?),
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()?,
        PasswordKdf::new(1),
        SmtpTransportCache::default(),
    );
    let archive_output = admin
        .post(
            "payment/drop",
            HashMap::from([("id".to_string(), payment_id.to_string())]),
        )
        .await?;
    ensure!(
        matches!(archive_output, AdminOutput::Data(value) if value == true),
        "admin payment drop did not report a successful soft archive"
    );
    let fetch_output = admin.get("payment/fetch", HashMap::new()).await?;
    ensure!(
        matches!(fetch_output, AdminOutput::Data(value) if value.as_array().is_some_and(|rows| rows.iter().all(|row| row["id"].as_i64() != Some(i64::from(payment_id))))),
        "archived payment remained visible in the ordinary admin list"
    );

    let mut signed = BTreeMap::from([
        ("money".to_string(), "12.34".to_string()),
        ("out_trade_no".to_string(), trade_no.clone()),
        ("trade_no".to_string(), "EPAY-LATE-INTEGRATION".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = signed
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    signed.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    signed.insert("sign_type".to_string(), "MD5".to_string());
    let input = PaymentNotifyInput {
        params: signed.into_iter().collect::<HashMap<_, _>>(),
        body: Vec::new(),
        headers: HashMap::new(),
    };

    let mut config = integration_config(pool, redis_url)?;
    config.database_url = database_url.to_string();
    let order = OrderService::new(pool.clone(), Arc::new(config));
    let mut callbacks = JoinSet::new();
    const CALLBACK_COUNT: usize = 8;
    for _ in 0..CALLBACK_COUNT {
        let order = order.clone();
        let input = input.clone();
        let payment_uuid = payment_uuid.clone();
        callbacks.spawn(async move {
            order
                .handle_payment_notify("EPay", &payment_uuid, input)
                .await
        });
    }
    let mut first_notices = 0;
    while let Some(joined) = callbacks.join_next().await {
        let response = joined??;
        ensure!(
            response.body == "success",
            "authenticated EPay callback was not acknowledged"
        );
        first_notices += usize::from(response.late_payment_notice.is_some());
        ensure!(
            response.paid_notice.is_none(),
            "cancelled order was incorrectly marked paid"
        );
    }
    ensure!(
        first_notices == 1,
        "late payment emitted {first_notices} first-seen notices"
    );
    let (rows, occurrences, order_status): (i64, i64, i8) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*),
            COALESCE(MAX(occurrence_count), 0),
            (SELECT status FROM v2_order WHERE trade_no = ?)
        FROM v2_payment_reconciliation
        WHERE payment_id = ? AND callback_no = 'EPAY-LATE-INTEGRATION'
        "#,
    )
    .bind(&trade_no)
    .bind(payment_id)
    .fetch_one(pool)
    .await?;
    ensure!(
        rows == 1 && occurrences == CALLBACK_COUNT as i64 && order_status == 2,
        "late payment reconciliation was not one-row idempotent"
    );

    let mut mismatched = BTreeMap::from([
        ("money".to_string(), "1.00".to_string()),
        ("out_trade_no".to_string(), trade_no.clone()),
        ("trade_no".to_string(), "EPAY-AMOUNT-MISMATCH".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = mismatched
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    mismatched.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    mismatched.insert("sign_type".to_string(), "MD5".to_string());
    let response = order
        .handle_payment_notify(
            "EPay",
            &payment_uuid,
            PaymentNotifyInput {
                params: mismatched.into_iter().collect(),
                body: Vec::new(),
                headers: HashMap::new(),
            },
        )
        .await?;
    let notice = response
        .late_payment_notice
        .context("authenticated amount mismatch did not produce a reconciliation notice")?;
    ensure!(
        notice.reason == "settled_amount_mismatch" && response.paid_notice.is_none(),
        "authenticated amount mismatch was not classified safely"
    );
    let (reason, expected_amount, settled_amount): (String, i64, Option<i64>) = sqlx::query_as(
        r#"
        SELECT reason, expected_amount, settled_amount
        FROM v2_payment_reconciliation
        WHERE payment_id = ? AND callback_no = 'EPAY-AMOUNT-MISMATCH'
        "#,
    )
    .bind(payment_id)
    .fetch_one(pool)
    .await?;
    ensure!(
        reason == "settled_amount_mismatch"
            && expected_amount == 1234
            && settled_amount == Some(100),
        "authenticated amount mismatch was not durably recorded with exact amounts"
    );

    let missing_trade_no = format!("UNKNOWN-{}", "界".repeat(150));
    let unknown_callback_no = format!("EPAY-🚀{}", "X".repeat(400));
    let mut unknown_order = BTreeMap::from([
        ("money".to_string(), "2.00".to_string()),
        ("out_trade_no".to_string(), missing_trade_no.clone()),
        ("trade_no".to_string(), unknown_callback_no.clone()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = unknown_order
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    unknown_order.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    unknown_order.insert("sign_type".to_string(), "MD5".to_string());
    let response = order
        .handle_payment_notify(
            "EPay",
            &payment_uuid,
            PaymentNotifyInput {
                params: unknown_order.into_iter().collect(),
                body: Vec::new(),
                headers: HashMap::new(),
            },
        )
        .await?;
    ensure!(
        response.late_payment_notice.as_ref().is_some_and(|notice| {
            notice.reason == "order_not_found"
                && notice.trade_no.len() <= 255
                && notice.callback_no.len() <= 255
                && notice.trade_no_hash.len() == 64
                && notice.callback_no_hash.len() == 64
        }),
        "authenticated payment for an unknown order was not surfaced"
    );
    let (unknown_reason, stored_trade_no, stored_callback_no): (String, String, String) =
        sqlx::query_as(
            "SELECT reason, trade_no, callback_no FROM v2_payment_reconciliation \
             WHERE payment_id = ? AND callback_no_hash = UNHEX(SHA2(?, 256))",
        )
        .bind(payment_id)
        .bind(&unknown_callback_no)
        .fetch_one(pool)
        .await?;
    ensure!(
        unknown_reason == "order_not_found",
        "authenticated payment for an unknown order was not durable"
    );
    ensure!(
        stored_trade_no.len() <= 255 && stored_callback_no.len() <= 255,
        "oversized provider identifiers were not stored as bounded UTF-8 labels"
    );
    let trade_hash_matches: bool = sqlx::query_scalar(
        "SELECT trade_no_hash = UNHEX(SHA2(?, 256)) \
         FROM v2_payment_reconciliation \
         WHERE payment_id = ? AND callback_no_hash = UNHEX(SHA2(?, 256))",
    )
    .bind(&missing_trade_no)
    .bind(payment_id)
    .bind(&unknown_callback_no)
    .fetch_one(pool)
    .await?;
    ensure!(
        trade_hash_matches,
        "bounded reconciliation label did not retain the raw trade identity hash"
    );
    let archived_state: (i8, Option<i64>) =
        sqlx::query_as("SELECT enable, archived_at FROM v2_payment WHERE id = ?")
            .bind(payment_id)
            .fetch_one(pool)
            .await?;
    ensure!(
        archived_state.0 == 0 && archived_state.1.is_some(),
        "delayed callbacks did not preserve the archived verification version"
    );
    let expected_callback_hash_hex: String = sqlx::query_scalar("SELECT UPPER(SHA2(?, 256))")
        .bind(&unknown_callback_no)
        .fetch_one(pool)
        .await?;
    let list_output = admin
        .get(
            "order/reconciliation/fetch",
            HashMap::from([("trade_no".to_string(), missing_trade_no)]),
        )
        .await?;
    ensure!(
        matches!(
            list_output,
            AdminOutput::Page { data, total }
                if total == 1
                    && data.first().is_some_and(|row| {
                        row["reason"] == "order_not_found"
                            && row["callback_no_hash"].as_str()
                                == Some(expected_callback_hash_hex.as_str())
                    })
        ),
        "unknown-order reconciliation was not discoverable through the global admin API"
    );

    let paid_trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        r#"
        INSERT INTO v2_order (
            user_id, plan_id, payment_id, type, period, trade_no, total_amount,
            status, commission_status, commission_balance, created_at, updated_at
        ) VALUES (?, 0, ?, 1, 'deposit', ?, 200, 0, 0, 0, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(payment_id)
    .bind(&paid_trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let paid_callback_no = format!("EPAY-PAID-🚀{}", "Y".repeat(400));
    let mut paid_callback = BTreeMap::from([
        ("money".to_string(), "2.00".to_string()),
        ("out_trade_no".to_string(), paid_trade_no.clone()),
        ("trade_no".to_string(), paid_callback_no.clone()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = paid_callback
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    paid_callback.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    paid_callback.insert("sign_type".to_string(), "MD5".to_string());
    let paid_input = PaymentNotifyInput {
        params: paid_callback.into_iter().collect(),
        body: Vec::new(),
        headers: HashMap::new(),
    };
    let paid_response = order
        .handle_payment_notify("EPay", &payment_uuid, paid_input.clone())
        .await?;
    ensure!(
        paid_response.paid_notice.is_some() && paid_response.late_payment_notice.is_none(),
        "oversized authenticated callback did not complete the normal paid transition"
    );
    let (paid_status, callback_label, callback_label_bytes, callback_hash_matches): (
        i8,
        String,
        i64,
        bool,
    ) = sqlx::query_as(
        "SELECT status, callback_no, OCTET_LENGTH(callback_no), \
                    callback_no_hash = UNHEX(SHA2(?, 256)) \
             FROM v2_order WHERE trade_no = ?",
    )
    .bind(&paid_callback_no)
    .bind(&paid_trade_no)
    .fetch_one(pool)
    .await?;
    ensure!(
        matches!(paid_status, 1 | 3 | 4)
            && callback_label_bytes <= 255
            && callback_label.contains("\\u{1F680}")
            && !callback_label.contains('🚀')
            && callback_hash_matches,
        "normal settlement did not retain a bounded label and complete callback identity"
    );
    let replay = order
        .handle_payment_notify("EPay", &payment_uuid, paid_input)
        .await?;
    ensure!(
        replay.paid_notice.is_none() && replay.late_payment_notice.is_none(),
        "oversized callback hash did not make an exact provider replay idempotent"
    );
    let unexpected_reconciliation: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_payment_reconciliation \
         WHERE payment_id = ? AND callback_no_hash = UNHEX(SHA2(?, 256))",
    )
    .bind(payment_id)
    .bind(&paid_callback_no)
    .fetch_one(pool)
    .await?;
    ensure!(
        unexpected_reconciliation == 0,
        "an ordinary oversized callback replay was misclassified for reconciliation"
    );

    // Simulate an old API replica in a rolling deploy: it changes callback_no
    // without mentioning the new digest column. The 0043 bridge must replace,
    // not retain, the previous digest so the new replica sees an exact replay.
    let legacy_callback_no = "EPAY-LEGACY-ROLLING";
    sqlx::query("UPDATE v2_order SET callback_no = ? WHERE trade_no = ?")
        .bind(legacy_callback_no)
        .bind(&paid_trade_no)
        .execute(pool)
        .await?;
    let legacy_hash_matches: bool = sqlx::query_scalar(
        "SELECT callback_no_hash = UNHEX(SHA2(?, 256)) \
         FROM v2_order WHERE trade_no = ?",
    )
    .bind(legacy_callback_no)
    .bind(&paid_trade_no)
    .fetch_one(pool)
    .await?;
    ensure!(
        legacy_hash_matches,
        "legacy rolling-deploy writer left a stale callback digest"
    );
    let mut legacy_replay = BTreeMap::from([
        ("money".to_string(), "2.00".to_string()),
        ("out_trade_no".to_string(), paid_trade_no),
        ("trade_no".to_string(), legacy_callback_no.to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = legacy_replay
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    legacy_replay.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    legacy_replay.insert("sign_type".to_string(), "MD5".to_string());
    let legacy_response = order
        .handle_payment_notify(
            "EPay",
            &payment_uuid,
            PaymentNotifyInput {
                params: legacy_replay.into_iter().collect(),
                body: Vec::new(),
                headers: HashMap::new(),
            },
        )
        .await?;
    ensure!(
        legacy_response.paid_notice.is_none() && legacy_response.late_payment_notice.is_none(),
        "rolling-deploy bridge did not preserve exact replay idempotency"
    );
    Ok(())
}

async fn node_identity_epoch(pool: &MySqlPool) -> Result<()> {
    let node_id = 700_000 + i32::from(Uuid::new_v4().as_bytes()[0]);
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO v2_server_credential \
         (node_type, node_id, credential_epoch, updated_at) VALUES ('v2node', ?, 0, ?)",
    )
    .bind(node_id)
    .bind(now)
    .execute(pool)
    .await?;
    let master = "integration-only-node-master-key-with-enough-entropy";
    let epoch: i64 = sqlx::query_scalar(
        "SELECT credential_epoch FROM v2_server_credential WHERE node_type = 'v2node' AND node_id = ?",
    )
    .bind(node_id)
    .fetch_one(pool)
    .await?;
    let token =
        derive_node_token(master, "v2node", node_id, epoch).context("derive current node token")?;
    ensure!(verify_node_token(master, "v2node", node_id, epoch, &token));
    ensure!(!verify_node_token(
        master,
        "v2node",
        node_id + 1,
        epoch,
        &token
    ));
    ensure!(!verify_node_token(master, "vmess", node_id, epoch, &token));

    sqlx::query(
        "UPDATE v2_server_credential SET credential_epoch = credential_epoch + 1, updated_at = ? \
         WHERE node_type = 'v2node' AND node_id = ?",
    )
    .bind(now + 1)
    .bind(node_id)
    .execute(pool)
    .await?;
    let revoked_epoch: i64 = sqlx::query_scalar(
        "SELECT credential_epoch FROM v2_server_credential WHERE node_type = 'v2node' AND node_id = ?",
    )
    .bind(node_id)
    .fetch_one(pool)
    .await?;
    ensure!(!verify_node_token(
        master,
        "v2node",
        node_id,
        revoked_epoch,
        &token
    ));
    let replacement = derive_node_token(master, "v2node", node_id, revoked_epoch)
        .context("derive rotated node token")?;
    ensure!(verify_node_token(
        master,
        "v2node",
        node_id,
        revoked_epoch,
        &replacement
    ));
    Ok(())
}

async fn auth_rate_limits(pool: &MySqlPool, database_url: &str, redis_url: &str) -> Result<()> {
    let redis = redis::Client::open(redis_url)?;
    flush_redis(&redis).await?;

    let same_email = format!("same-email-{}@example.test", Uuid::new_v4().simple());
    let mut unique_config = integration_config(pool, redis_url)?;
    unique_config.database_url = database_url.to_string();
    unique_config.register_limit_by_ip_enable = false;
    unique_config.invite_force = false;
    let unique_auth = auth_service(pool, redis_url, unique_config).await?;
    let mut duplicate_registrations = JoinSet::new();
    for sequence in 0..6 {
        let auth = unique_auth.clone();
        let email = same_email.clone();
        duplicate_registrations.spawn(async move {
            auth.register(
                RegisterInput {
                    email,
                    password: "integration-password".to_string(),
                    invite_code: None,
                    email_code: None,
                    recaptcha_data: None,
                },
                Some(format!("198.18.0.{}", sequence + 1)),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut same_email_success = 0;
    let mut same_email_rejected = 0;
    while let Some(joined) = duplicate_registrations.join_next().await {
        match joined? {
            Ok(_) => same_email_success += 1,
            Err(error) if error.to_string().contains("Email already exists") => {
                same_email_rejected += 1;
            }
            Err(error) => bail!("unexpected same-email registration error: {error:#}"),
        }
    }
    let persisted_same_email: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE email = ?")
            .bind(&same_email)
            .fetch_one(pool)
            .await?;
    ensure!(
        (
            same_email_success,
            same_email_rejected,
            persisted_same_email
        ) == (1, 5, 1),
        "same-email registration did not map the unique race to a stable business outcome"
    );
    flush_redis(&redis).await?;

    let mut register_config = integration_config(pool, redis_url)?;
    register_config.database_url = database_url.to_string();
    register_config.register_limit_by_ip_enable = true;
    register_config.register_limit_count = 3;
    register_config.register_limit_expire = 1;
    register_config.invite_force = false;
    let register_auth = auth_service(pool, redis_url, register_config).await?;
    let registration_ip = "203.0.113.44";
    let mut registrations = JoinSet::new();
    for sequence in 0..9 {
        let auth = register_auth.clone();
        registrations.spawn(async move {
            auth.register(
                RegisterInput {
                    email: format!("rate-{sequence}-{}@example.test", Uuid::new_v4().simple()),
                    password: "integration-password".to_string(),
                    invite_code: None,
                    email_code: None,
                    recaptcha_data: None,
                },
                Some(registration_ip.to_string()),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut registered = 0;
    let mut registration_limited = 0;
    while let Some(joined) = registrations.join_next().await {
        match joined? {
            Ok(_) => registered += 1,
            Err(error) if error.to_string().contains("Register frequently") => {
                registration_limited += 1;
            }
            Err(error) => bail!("unexpected registration limiter error: {error:#}"),
        }
    }
    ensure!(
        (registered, registration_limited) == (3, 6),
        "registration limiter admitted {registered} concurrent requests"
    );
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let registration_slots: i64 = conn
        .zcard(format!("REGISTER_IP_RATE_LIMIT_V2_{registration_ip}"))
        .await?;
    ensure!(
        registration_slots == 3,
        "registration reservations were not atomic"
    );

    flush_redis(&redis).await?;
    let email = format!("login-{}@example.test", Uuid::new_v4().simple());
    let password_hash = hash_password("correct-integration-password")?;
    insert_user_with_email(pool, &email, &password_hash).await?;
    let mut login_config = integration_config(pool, redis_url)?;
    login_config.database_url = database_url.to_string();
    login_config.password_limit_enable = true;
    login_config.password_limit_count = 3;
    login_config.password_limit_expire = 1;
    let login_auth = auth_service(pool, redis_url, login_config).await?;
    let mut logins = JoinSet::new();
    for sequence in 0..9 {
        let auth = login_auth.clone();
        let attempted_email = if sequence % 2 == 0 {
            email.to_ascii_uppercase()
        } else {
            email.clone()
        };
        logins.spawn(async move {
            auth.login(
                &attempted_email,
                "wrong-integration-password",
                Some("192.0.2.55".to_string()),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut incorrect = 0;
    let mut login_limited = 0;
    while let Some(joined) = logins.join_next().await {
        match joined? {
            Ok(_) => bail!("wrong password unexpectedly authenticated"),
            Err(error) if error.to_string().contains("Incorrect email or password") => {
                incorrect += 1;
            }
            Err(error) if error.to_string().contains("too many password errors") => {
                login_limited += 1;
            }
            Err(error) => bail!("unexpected login limiter error: {error:#}"),
        }
    }
    ensure!(
        (incorrect, login_limited) == (3, 6),
        "login limiter admitted {incorrect} concurrent password checks"
    );
    let keys: Vec<String> = conn.keys("PASSWORD_ERROR_LIMIT_*").await?;
    ensure!(
        keys.len() == 3,
        "login limiter did not maintain all three dimensions"
    );
    for key in keys {
        let count: i64 = conn.get(&key).await?;
        ensure!(
            count == 3,
            "login limiter key {key} stored {count}, expected 3"
        );
    }
    Ok(())
}

async fn redis_lease_ownership(redis: &redis::Client) -> Result<()> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let key = format!("RUST_SCHEDULER_LOCK_contract_{}", Uuid::new_v4().simple());
    let owner = Uuid::new_v4().to_string();
    let replacement = Uuid::new_v4().to_string();
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&owner)
        .arg("NX")
        .arg("EX")
        .arg(30)
        .query_async(&mut conn)
        .await?;
    ensure!(
        acquired.as_deref() == Some("OK"),
        "failed to acquire test lease"
    );
    let _: String = redis::cmd("SET")
        .arg(&key)
        .arg(&replacement)
        .arg("XX")
        .arg("EX")
        .arg(30)
        .query_async(&mut conn)
        .await?;
    let renewed: i64 = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("EXPIRE", KEYS[1], ARGV[2])
        end
        return 0
        "#,
    )
    .key(&key)
    .arg(&owner)
    .arg(30)
    .invoke_async(&mut conn)
    .await?;
    let released: i64 = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("DEL", KEYS[1])
        end
        return 0
        "#,
    )
    .key(&key)
    .arg(&owner)
    .invoke_async(&mut conn)
    .await?;
    let current: String = conn.get(&key).await?;
    ensure!(renewed == 0 && released == 0 && current == replacement);
    let _: i64 = conn.del(key).await?;
    Ok(())
}

async fn worker_health_process(
    database_url: &str,
    database_name: &str,
    redis_url: &str,
) -> Result<()> {
    let worker_bin = env_or("RUST_INTEGRATION_WORKER_BIN", DEFAULT_WORKER_BIN);
    let runtime_root = PathBuf::from("/tmp").join(format!("{database_name}-health-runtime"));
    let health_file = PathBuf::from("/tmp").join(format!("{database_name}-worker-health"));
    let _ = tokio::fs::remove_file(&health_file).await;
    let mut child = Command::new(&worker_bin)
        .env("DATABASE_URL", database_url)
        .env("REDIS_URL", redis_url)
        .env("V2BOARD_ENV", "testing")
        .env("V2BOARD_SEED_LOCAL", "0")
        .env("V2BOARD_RUNTIME_ROOT", runtime_root)
        .env("V2BOARD_WORKER_HEALTH_FILE", &health_file)
        .env("V2BOARD_WORKER_HEARTBEAT_INTERVAL_SECONDS", "1")
        .env(
            "APP_KEY",
            "integration-only-worker-app-key-with-at-least-thirty-two-bytes",
        )
        .env("RUST_LOG", "v2board_workers=error")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawn isolated worker process {worker_bin}"))?;

    let result = wait_for_worker_health(&mut child, &health_file, redis_url).await;
    if child.try_wait()?.is_none() {
        child
            .kill()
            .context("stop isolated worker health process")?;
    }
    let _ = child.wait();
    let _ = tokio::fs::remove_file(&health_file).await;
    result
}

async fn wait_for_worker_health(
    child: &mut std::process::Child,
    health_file: &Path,
    redis_url: &str,
) -> Result<()> {
    let redis = redis::Client::open(redis_url)?;
    let expected = [
        "traffic_update",
        "statistics",
        "check_order",
        "check_commission",
        "check_ticket",
        "check_renewal",
        "reset_traffic",
        "reset_log",
        "send_remind_mail",
        "mail_outbox",
    ];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait()? {
            bail!("isolated worker exited before becoming healthy: {status}");
        }
        let mut conn = redis.get_multiplexed_async_connection().await?;
        let heartbeats: BTreeMap<String, i64> =
            conn.hgetall("RUST_WORKER_LOOP_HEARTBEAT_AT").await?;
        let now = Utc::now().timestamp();
        let all_recent = expected.iter().all(|name| {
            heartbeats
                .get(*name)
                .is_some_and(|seen| now.saturating_sub(*seen) <= 60)
        });
        let health_timestamp = tokio::fs::read_to_string(health_file)
            .await
            .ok()
            .and_then(|value| value.trim().parse::<i64>().ok());
        let health_recent = health_timestamp.is_some_and(|seen| now.saturating_sub(seen) <= 5);
        if all_recent && health_recent {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            let missing = expected
                .iter()
                .filter(|name| {
                    heartbeats
                        .get(**name)
                        .is_none_or(|seen| now.saturating_sub(*seen) > 60)
                })
                .copied()
                .collect::<Vec<_>>();
            bail!("worker loop heartbeat is missing or stale for {missing:?}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn migration_readiness_failure_modes(pool: &MySqlPool) -> Result<()> {
    let latest = MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| migration.version)
        .max()
        .context("embedded migration list is empty")?;
    let deleted = sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
        .bind(latest)
        .execute(pool)
        .await?;
    ensure!(
        deleted.rows_affected() == 1,
        "latest migration ledger row was absent"
    );
    ensure!(
        !migrations_current(pool).await?,
        "missing migration was reported current"
    );
    sqlx::query("DROP TABLE _sqlx_migrations")
        .execute(pool)
        .await?;
    ensure!(
        migrations_current(pool).await.is_err(),
        "missing migration ledger did not fail readiness"
    );
    Ok(())
}

async fn auth_service(pool: &MySqlPool, redis_url: &str, config: AppConfig) -> Result<AuthService> {
    let redis = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(redis).await?;
    Ok(AuthService::new(
        pool.clone(),
        manager,
        Arc::new(config),
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()?,
        PasswordKdf::new(4),
        SmtpTransportCache::default(),
    ))
}

fn integration_config(_pool: &MySqlPool, redis_url: &str) -> Result<AppConfig> {
    let mut config = AppConfig::try_from_env().context("load integration AppConfig")?;
    config.environment = RuntimeEnvironment::Testing;
    config.redis_url = redis_url.to_string();
    config.app_key = "integration-only-app-key-with-at-least-thirty-two-bytes".to_string();
    config.stop_register = false;
    config.email_verify = false;
    config.recaptcha_enable = false;
    config.email_whitelist_enable = false;
    config.email_gmail_limit_enable = false;
    config.try_out_plan_id = 0;
    Ok(config)
}

async fn insert_user(pool: &MySqlPool, label: &str, password: &str) -> Result<i64> {
    let email = format!("{label}-{}@example.test", Uuid::new_v4().simple());
    insert_user_with_email(pool, &email, password).await
}

async fn insert_user_with_email(pool: &MySqlPool, email: &str, password: &str) -> Result<i64> {
    let now = Utc::now().timestamp();
    let result = sqlx::query(
        "INSERT INTO v2_user (email, password, uuid, token, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(email)
    .bind(password)
    .bind(Uuid::new_v4().hyphenated().to_string())
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(result.last_insert_id() as i64)
}

async fn run_worker_once(
    database_url: impl Into<String>,
    database_name: impl Into<String>,
    redis_url: impl Into<String>,
    job: &'static str,
) -> Result<()> {
    let database_url = database_url.into();
    let database_name = database_name.into();
    let redis_url = redis_url.into();
    let worker_bin = env_or("RUST_INTEGRATION_WORKER_BIN", DEFAULT_WORKER_BIN);
    tokio::task::spawn_blocking(move || {
        let runtime_root = PathBuf::from("/tmp").join(format!("{database_name}-worker"));
        let output = Command::new(&worker_bin)
            .args(["run-once", job])
            .env("DATABASE_URL", &database_url)
            .env("REDIS_URL", &redis_url)
            .env("V2BOARD_ENV", "testing")
            .env("V2BOARD_SEED_LOCAL", "0")
            .env("V2BOARD_RUNTIME_ROOT", runtime_root)
            .env(
                "APP_KEY",
                "integration-only-worker-app-key-with-at-least-thirty-two-bytes",
            )
            .env("RUST_LOG", "v2board_workers=error")
            .output()
            .with_context(|| format!("execute {worker_bin} run-once {job}"))?;
        ensure!(
            output.status.success(),
            "worker run-once {job} failed (status {}): stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    })
    .await??;
    Ok(())
}

async fn flush_redis(redis: &redis::Client) -> Result<()> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    redis::cmd("FLUSHDB").query_async::<()>(&mut conn).await?;
    Ok(())
}

async fn create_database(root: &MySqlPool, database_name: &str) -> Result<()> {
    ensure_safe_identifier(database_name)?;
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE `{database_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )))
    .execute(root)
    .await?;
    Ok(())
}

async fn drop_database(root: &MySqlPool, database_name: &str) -> Result<()> {
    ensure_safe_identifier(database_name)?;
    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS `{database_name}`"
    )))
    .execute(root)
    .await?;
    Ok(())
}

fn database_url_for(root_database_url: &str, database_name: &str) -> Result<String> {
    ensure_safe_identifier(database_name)?;
    let mut url = Url::parse(root_database_url).context("parse integration root database URL")?;
    url.set_path(&format!("/{database_name}"));
    Ok(url.to_string())
}

fn ensure_safe_identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "unsafe generated SQL identifier"
    );
    Ok(())
}

fn random_traffic_key() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn pass(name: &str) {
    println!("PASS {name}");
}
