use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, PgPool, postgres::PgPoolOptions};
use tokio::task::JoinSet;
use url::Url;
use uuid::Uuid;
use v2board_analytics::{
    AnalyticsAdmissionPolicy, AnalyticsEvent, OutboxError, claim_delivery_batch, enqueue_event,
    install_analytics_admission_policy, mark_batch_published, refresh_analytics_admission,
    release_batch_for_retry,
};
use v2board_config::{AppConfig, RedisKeyspace, RuntimeEnvironment};
use v2board_db::{DbPoolConfig, installation_id, migrations_current};
use v2board_domain::{
    admin::{AdminOutput, AdminService},
    auth::{AuthService, PasswordKdf, RegisterInput, hash_password},
    operator_config,
    order::{OrderService, PaymentNotifyInput},
    redis_runtime::verify_redis_runtime,
    server_credentials::{derive_node_token, verify_node_token},
    smtp::SmtpTransportCache,
};

pub(crate) static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

pub(crate) const DEFAULT_ROOT_DATABASE_URL: &str =
    "postgresql://v2board:v2board@postgres:5432/postgres";
const DEFAULT_RUNTIME_REDIS_URL: &str = "redis://redis:6379/1";
pub(crate) const DEFAULT_INTEGRATION_REDIS_URL: &str = "redis://redis:6379/15";
const DEFAULT_WORKER_BIN: &str = "/app/target/debug/v2board-workers";
const INTEGRATION_APP_KEY: &str = "integration-only-app-key-with-at-least-thirty-two-bytes";

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

    let database_name = GeneratedDatabaseName::new("contract")?;
    let database_url = database_url_for(&root_database_url, &database_name)?;
    let root = PgPoolOptions::new()
        .max_connections(2)
        .connect(&root_database_url)
        .await
        .context("connect to the disposable-database administrator")?;
    create_database(&root, &database_name).await?;

    let pool_config = DbPoolConfig {
        min_connections: 1,
        max_connections: 40,
        acquire_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
    };
    let pool = match v2board_db::connect_postgres_with_config(&database_url, &pool_config).await {
        Ok(pool) => pool,
        Err(error) => {
            let error =
                anyhow::Error::new(error).context("connect to the disposable integration database");
            let cleanup = drop_database(&root, &database_name).await;
            root.close().await;
            return match cleanup {
                Ok(()) => Err(error),
                Err(cleanup) => Err(error.context(format!(
                    "also failed to drop disposable database {}: {cleanup:#}",
                    database_name.as_str()
                ))),
            };
        }
    };
    let result = run_isolated_checks(
        &pool,
        &database_url,
        database_name.as_str(),
        &integration_redis_url,
    )
    .await;

    pool.close().await;
    let drop_result = drop_database(&root, &database_name).await;
    root.close().await;

    match (result, drop_result) {
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "also failed to drop disposable database {}: {cleanup:#}",
            database_name.as_str()
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
    pool: &PgPool,
    database_url: &str,
    database_name: &str,
    integration_redis_url: &str,
) -> Result<()> {
    let integration_redis = redis::Client::open(integration_redis_url)?;
    flush_redis(&integration_redis).await?;

    let result = async {
        crate::sql_schema_prepare::audit_dynamic_inventory()?;
        MIGRATOR
            .run(pool)
            .await
            .context("apply every embedded migration to a fresh PostgreSQL database")?;
        ensure!(
            migrations_current(pool).await?,
            "freshly applied migration ledger is not current"
        );
        installation_identity_invariant(pool).await?;
        pass("installation identity is explicit, unique, and immutable");
        install_contract_operator_config_authority(pool, integration_redis_url).await?;
        pass("operator configuration authority is explicit and authenticated");
        install_contract_analytics_admission(pool).await?;
        pass("analytics admission policy is installation-bound and measurable");
        schema_invariants(pool).await?;
        pass("fresh migrations and production schema constraints");

        analytics_outbox_invariant(pool).await?;
        pass("analytics outbox uniqueness, batching, and leases are durable");

        crate::sql_schema_prepare::run(pool).await?;
        pass("static runtime SQL prepares against the migrated production schema");

        traffic_epoch_invariant(pool, database_url, database_name, integration_redis_url).await?;
        pass("traffic epoch rejects delayed pre-reset reports");

        invite_single_consumption(pool, integration_redis_url).await?;
        pass("single-use invite remains single-use under concurrency");
        flush_redis(&integration_redis).await?;
        verify_redis_runtime(&integration_redis, RuntimeEnvironment::Production).await?;
        pass("production Redis policy is verifiably noeviction");

        ticket_state_machine(pool, database_url, database_name, integration_redis_url).await?;
        pass("one-open-ticket and reply/auto-close serialization");

        node_identity_epoch(pool).await?;
        pass("node credentials are bound to identity and revocation epoch");

        auth_rate_limits(pool, database_url, integration_redis_url).await?;
        pass("registration and login reservations are atomic in Redis");
        flush_redis(&integration_redis).await?;

        redis_lease_ownership(&integration_redis).await?;
        pass("a stale worker lease owner cannot renew or release a replacement lease");

        worker_health_process(pool, database_url, database_name, integration_redis_url).await?;
        pass("a live isolated worker publishes health and per-loop heartbeats");

        late_payment_reconciliation(pool, database_url, integration_redis_url).await?;
        pass("late authenticated payment reconciliation is durable and idempotent");

        admin_projection_key_sets(pool, integration_redis_url).await?;
        pass("admin projections serialize exactly their pinned key sets");

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

async fn install_contract_analytics_admission(pool: &PgPool) -> Result<()> {
    let now = Utc::now().timestamp();
    let installation_id = installation_id(pool).await?;
    let gib = 1024_u64 * 1024 * 1024;
    let policy = AnalyticsAdmissionPolicy {
        recovery_pending_rows: 750_000,
        soft_pending_rows: 1_000_000,
        hard_pending_rows: 2_000_000,
        recovery_relation_bytes: 20 * gib,
        soft_relation_bytes: 30 * gib,
        hard_relation_bytes: 40 * gib,
        recovery_oldest_age_seconds: 300,
        soft_oldest_age_seconds: 900,
        hard_oldest_age_seconds: 3_600,
        database_capacity_bytes: 128 * gib,
        hard_min_headroom_bytes: 16 * gib,
        soft_min_headroom_bytes: 32 * gib,
        recovery_min_headroom_bytes: 48 * gib,
        event_reservation_bytes: 4_096,
        soft_max_new_rows_per_second: 100_000,
        sample_interval_seconds: 1,
        stale_after_seconds: 30,
        capacity_evidence: "disposable production-invariant PostgreSQL quota".to_owned(),
    };
    install_analytics_admission_policy(pool, installation_id, &policy, now).await?;
    let snapshot = refresh_analytics_admission(pool).await?.snapshot;
    ensure!(snapshot.sample_fresh && snapshot.pending_rows == 0);
    Ok(())
}

async fn install_contract_operator_config_authority(pool: &PgPool, redis_url: &str) -> Result<()> {
    let config = integration_config(pool, redis_url)?;
    let installation_id = installation_id(pool).await?;
    let candidate = config.operator_config_map();
    let snapshot = operator_config::seed_initial_authority(
        pool,
        installation_id,
        &config.app_key,
        &candidate,
        "contract:bootstrap",
    )
    .await?;
    ensure!(
        snapshot.revision == 1 && snapshot.values == candidate,
        "fresh operator configuration authority was not seeded exactly"
    );
    let reseed = operator_config::seed_initial_authority(
        pool,
        installation_id,
        &config.app_key,
        &candidate,
        "contract:reseed",
    )
    .await;
    ensure!(
        matches!(
            reseed,
            Err(operator_config::OperatorConfigError::Integrity(
                "operator configuration authority is not empty"
            ))
        ),
        "operator configuration authority accepted a second initial seed"
    );
    let (state_rows, revision_rows) = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM operator_config_state),
            (SELECT COUNT(*) FROM operator_config_revision)
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        (state_rows, revision_rows) == (1, 1),
        "rejected operator configuration reseed changed authority rows"
    );
    Ok(())
}

async fn schema_invariants(pool: &PgPool) -> Result<()> {
    for (table, index) in [
        ("coupon", "uniq_coupon_code_canonical"),
        ("gift_card", "uniq_gift_card_code_canonical"),
        ("invite_code", "uniq_invite_code_canonical"),
        ("users", "uniq_user_email_canonical"),
        ("payment_method", "uniq_payment_method_driver_uuid"),
        ("orders", "uniq_unfinished_order_per_user"),
        ("ticket", "uniq_ticket_open_user"),
        (
            "payment_reconciliation",
            "uniq_payment_reconciliation_callback",
        ),
        ("analytics_outbox", "uniq_analytics_event_id"),
        ("analytics_outbox", "uniq_analytics_batch_row"),
    ] {
        let unique_columns: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM pg_indexes
            WHERE schemaname = current_schema()
              AND tablename = $1
              AND indexname = $2
              AND indexdef LIKE 'CREATE UNIQUE INDEX%'
            "#,
        )
        .bind(table)
        .bind(index)
        .fetch_one(pool)
        .await?;
        ensure!(unique_columns > 0, "missing unique index {table}.{index}");
    }
    for (table, column) in [
        ("users", "traffic_epoch"),
        ("server_traffic_report_item", "traffic_epoch"),
        ("server_credential", "credential_epoch"),
        ("payment_method", "archived_at"),
        ("payment_reconciliation", "trade_no_hash"),
        ("payment_reconciliation", "callback_no_hash"),
        ("orders", "callback_no_hash"),
        ("orders", "referenced_plan_id"),
        ("system_installation", "installation_id"),
        ("analytics_outbox", "delivery_batch_id"),
        ("server_traffic_report", "identity_kind"),
        ("server_traffic_report", "accounting_date"),
        ("server_traffic_report_item", "raw_u"),
        ("server_traffic_report_item", "charged_u"),
    ] {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM information_schema.columns
            WHERE table_schema = current_schema() AND table_name = $1 AND column_name = $2
            "#,
        )
        .bind(table)
        .bind(column)
        .fetch_one(pool)
        .await?;
        ensure!(count == 1, "missing required column {table}.{column}");
    }
    for (table, constraint) in [
        ("plan", "chk_plan_flags"),
        ("coupon", "chk_coupon_type_value"),
        ("users", "chk_user_role_flags"),
        ("users", "chk_user_traffic_nonnegative"),
        ("orders", "chk_order_status"),
        ("orders", "chk_order_amounts_nonnegative"),
        ("ticket", "chk_ticket_status"),
        ("stat", "chk_stat_counts_nonnegative"),
        ("server_traffic", "chk_server_traffic_nonnegative"),
        ("user_traffic", "chk_user_traffic_history_nonnegative"),
    ] {
        let validated: bool = sqlx::query_scalar(
            r#"
            SELECT COALESCE(bool_and(constraint_row.convalidated), false)
            FROM pg_constraint AS constraint_row
            JOIN pg_class AS source_table ON source_table.oid = constraint_row.conrelid
            JOIN pg_namespace AS namespace ON namespace.oid = source_table.relnamespace
            WHERE namespace.nspname = current_schema()
              AND source_table.relname = $1
              AND constraint_row.conname = $2
              AND constraint_row.contype = 'c'
            "#,
        )
        .bind(table)
        .bind(constraint)
        .fetch_one(pool)
        .await?;
        ensure!(validated, "missing validated check {table}.{constraint}");
    }
    for (table, index) in [
        (
            "analytics_delivery_batch",
            "idx_analytics_batch_published_cleanup",
        ),
        ("analytics_outbox", "idx_analytics_outbox_published_cleanup"),
    ] {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM pg_indexes
            WHERE schemaname = current_schema()
              AND tablename = $1
              AND indexname = $2
            "#,
        )
        .bind(table)
        .bind(index)
        .fetch_one(pool)
        .await?;
        ensure!(
            count == 1,
            "missing published cleanup index {table}.{index}"
        );
    }
    let reconciliation_payment_fk: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_constraint AS constraint_row
        JOIN pg_class AS source_table ON source_table.oid = constraint_row.conrelid
        JOIN pg_class AS target_table ON target_table.oid = constraint_row.confrelid
        JOIN pg_namespace AS namespace ON namespace.oid = source_table.relnamespace
        WHERE namespace.nspname = current_schema()
          AND constraint_row.contype = 'f'
          AND source_table.relname = 'payment_reconciliation'
          AND target_table.relname = 'payment_method'
          AND constraint_row.confdeltype = 'r'
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
        WHERE table_schema = current_schema()
          AND table_name = 'payment_reconciliation'
          AND column_name = 'expected_amount'
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        expected_amount_type == "bigint",
        "payment reconciliation expected_amount is not bigint"
    );
    let partial_unique_indexes: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_index AS index_row
        JOIN pg_class AS index_name ON index_name.oid = index_row.indexrelid
        JOIN pg_namespace AS namespace ON namespace.oid = index_name.relnamespace
        WHERE namespace.nspname = current_schema()
          AND index_name.relname IN ('uniq_unfinished_order_per_user', 'uniq_ticket_open_user')
          AND index_row.indisunique
          AND index_row.indpred IS NOT NULL
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        partial_unique_indexes == 2,
        "unfinished-order and open-ticket indexes are not partial unique indexes"
    );

    partial_unique_behavior(pool).await?;
    Ok(())
}

async fn installation_identity_invariant(pool: &PgPool) -> Result<()> {
    ensure!(
        matches!(installation_id(pool).await, Err(sqlx::Error::RowNotFound)),
        "an unbootstrapped database exposed an installation identity"
    );
    let expected = Uuid::new_v4();
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO system_installation
            (singleton, installation_id, created_at)
        VALUES (1, $1, $2)
        "#,
    )
    .bind(expected)
    .bind(now)
    .execute(pool)
    .await?;
    ensure!(installation_id(pool).await? == expected);
    ensure!(
        sqlx::query("UPDATE system_installation SET installation_id = $1 WHERE singleton = 1")
            .bind(Uuid::new_v4())
            .execute(pool)
            .await
            .is_err(),
        "installation UUID was mutable"
    );
    ensure!(
        sqlx::query("UPDATE system_installation SET created_at = $1 WHERE singleton = 1")
            .bind(now + 1)
            .execute(pool)
            .await
            .is_err(),
        "installation creation time was mutable"
    );
    ensure!(
        sqlx::query("DELETE FROM system_installation WHERE singleton = 1")
            .execute(pool)
            .await
            .is_err(),
        "installation identity was deletable"
    );
    Ok(())
}

async fn partial_unique_behavior(pool: &PgPool) -> Result<()> {
    let user_id = insert_user(pool, "partial-unique", "not-used").await?;
    let now = Utc::now().timestamp();
    let trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        r#"
        INSERT INTO orders (
            user_id, plan_id, type, period, trade_no, total_amount, status,
            commission_status, commission_balance, created_at, updated_at
        ) VALUES ($1, 0, 9, 'deposit', $2, 0, 0, 0, 0, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    ensure!(
        sqlx::query(
            r#"
            INSERT INTO orders (
                user_id, plan_id, type, period, trade_no, total_amount, status,
                commission_status, commission_balance, created_at, updated_at
            ) VALUES ($1, 0, 9, 'deposit', $2, 0, 1, 0, 0, $3, $4)
            "#,
        )
        .bind(user_id)
        .bind(Uuid::new_v4().hyphenated().to_string())
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .is_err(),
        "partial unique order index allowed two unfinished orders"
    );
    sqlx::query("UPDATE orders SET status = 2 WHERE trade_no = $1")
        .bind(&trade_no)
        .execute(pool)
        .await?;

    let ticket_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO ticket
            (user_id, subject, level, status, reply_status, created_at, updated_at)
        VALUES ($1, 'first', 0, 0, 0, $2, $3)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    ensure!(
        sqlx::query(
            r#"
            INSERT INTO ticket
                (user_id, subject, level, status, reply_status, created_at, updated_at)
            VALUES ($1, 'second', 0, 0, 0, $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .is_err(),
        "partial unique ticket index allowed two open tickets"
    );
    sqlx::query("UPDATE ticket SET status = 1 WHERE id = $1")
        .bind(ticket_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn analytics_outbox_invariant(pool: &PgPool) -> Result<()> {
    let now = Utc::now().timestamp();
    let event = AnalyticsEvent {
        event_id: format!("{:064x}", 1),
        event_name: "contract.event.v1".to_string(),
        schema_major: 1,
        report_key: random_traffic_key(),
        partition_month: Utc::now().format("%Y-%m-01").to_string(),
        occurred_at: now,
        payload: serde_json::json!({"contract": true}),
        payload_sha256: format!("{:064x}", 2),
    };
    let mut tx = pool.begin().await?;
    enqueue_event(&mut tx, &event, now).await?;
    enqueue_event(&mut tx, &event, now).await?;
    let mut conflict = event.clone();
    conflict.payload = serde_json::json!({"contract": false});
    ensure!(
        matches!(
            enqueue_event(&mut tx, &conflict, now).await,
            Err(OutboxError::EventConflict { .. })
        ),
        "analytics event id accepted conflicting immutable content"
    );
    tx.commit().await?;

    let owner = Uuid::new_v4();
    let batch = claim_delivery_batch(pool, owner, now, 30, 100)
        .await?
        .context("analytics event was not claimable")?;
    ensure!(batch.rows.len() == 1 && batch.rows[0].event == event);
    release_batch_for_retry(pool, &batch, "contract retry").await?;
    let replacement_owner = Uuid::new_v4();
    let retry = claim_delivery_batch(pool, replacement_owner, now + 1, 30, 100)
        .await?
        .context("released analytics batch was not reclaimable")?;
    ensure!(retry.batch_id == batch.batch_id && retry.lease_owner == replacement_owner);
    mark_batch_published(pool, &retry, now + 2).await?;
    let published: bool = sqlx::query_scalar(
        "SELECT published_at IS NOT NULL FROM analytics_outbox WHERE event_id = $1",
    )
    .bind(&event.event_id)
    .fetch_one(pool)
    .await?;
    ensure!(
        published,
        "published analytics batch left its event pending"
    );
    Ok(())
}

async fn traffic_epoch_invariant(
    pool: &PgPool,
    database_url: &str,
    database_name: &str,
    redis_url: &str,
) -> Result<()> {
    let user_id = insert_user(pool, "traffic", "not-used").await?;
    sqlx::query("UPDATE users SET u = 11, d = 13 WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    let stale_key = random_traffic_key();
    insert_traffic_report(pool, &stale_key, user_id, 0, 101, 103).await?;
    let reset = sqlx::query(
        "UPDATE users SET u = 0, d = 0, traffic_epoch = traffic_epoch + 1 WHERE id = $1",
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
        sqlx::query_as("SELECT u, d, traffic_epoch FROM users WHERE id = $1")
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
    let (u, d): (i64, i64) = sqlx::query_as("SELECT u, d FROM users WHERE id = $1")
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
    pool: &PgPool,
    report_key: &str,
    user_id: i64,
    epoch: i64,
    u: i64,
    d: i64,
) -> Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO server_traffic_report \
         (report_key, payload_hash, node_id, node_type, rate_text, rate_decimal_10_2,
          identity_kind, accepted_at, accounting_date, applied_at, created_at, updated_at) \
         VALUES ($1, $2, 1, 'contract', '1', 1.00, 'explicit', $3, $4, NULL, $5, $6)",
    )
    .bind(report_key)
    .bind(random_traffic_key())
    .bind(now)
    .bind(Utc::now().date_naive())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO server_traffic_report_item \
         (report_key, user_id, traffic_epoch, raw_u, raw_d, charged_u, charged_d)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(report_key)
    .bind(user_id)
    .bind(epoch)
    .bind(u)
    .bind(d)
    .bind(u)
    .bind(d)
    .execute(pool)
    .await?;
    Ok(())
}

async fn assert_report_consumed(pool: &PgPool, report_key: &str) -> Result<()> {
    let applied_at: Option<i64> =
        sqlx::query_scalar("SELECT applied_at FROM server_traffic_report WHERE report_key = $1")
            .bind(report_key)
            .fetch_one(pool)
            .await?;
    let item_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM server_traffic_report_item WHERE report_key = $1")
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

async fn invite_single_consumption(pool: &PgPool, redis_url: &str) -> Result<()> {
    let inviter_id = insert_user(pool, "inviter", "not-used").await?;
    let invite_code = Uuid::new_v4().simple().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO invite_code (user_id, code, status, pv, created_at, updated_at) \
         VALUES ($1, $2, 0, 0, $3, $4)",
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
    let status: i16 = sqlx::query_scalar("SELECT status FROM invite_code WHERE code = $1")
        .bind(&invite_code)
        .fetch_one(pool)
        .await?;
    let invited: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE invite_user_id = $1")
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
    pool: &PgPool,
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
            TicketCreateOutcome::Created(_) => created += 1,
            TicketCreateOutcome::OpenTicketExists => existed += 1,
            outcome => bail!("unexpected concurrent ticket outcome: {outcome:?}"),
        }
    }
    ensure!(
        (created, existed) == (1, 7),
        "one-open-ticket invariant failed"
    );
    let first_ticket: i64 =
        sqlx::query_scalar("SELECT id FROM ticket WHERE user_id = $1 AND status = 0")
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
        .await
        .map(|outcome| matches!(outcome, TicketCreateOutcome::Created(_)))?,
        "failed to create reply-race ticket"
    );
    let race_ticket: i64 =
        sqlx::query_scalar("SELECT id FROM ticket WHERE user_id = $1 AND status = 0")
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
    let race_status: i16 = sqlx::query_scalar("SELECT status FROM ticket WHERE id = $1")
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
        .await
        .map(|outcome| matches!(outcome, TicketCreateOutcome::Created(_)))?,
        "failed to create stale ticket"
    );
    let stale_ticket: i64 =
        sqlx::query_scalar("SELECT id FROM ticket WHERE user_id = $1 AND status = 0")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    insert_operator_reply(pool, stale_ticket, now - 90_000).await?;
    run_worker_once(database_url, database_name, redis_url, "check_ticket").await?;
    let stale_status: i16 = sqlx::query_scalar("SELECT status FROM ticket WHERE id = $1")
        .bind(stale_ticket)
        .fetch_one(pool)
        .await?;
    ensure!(
        stale_status == 1,
        "genuinely stale answered ticket was not closed"
    );
    Ok(())
}

async fn insert_operator_reply(pool: &PgPool, ticket_id: i64, timestamp: i64) -> Result<()> {
    sqlx::query(
        "INSERT INTO ticket_message (user_id, ticket_id, message, created_at, updated_at) \
         VALUES (0, $1, 'operator reply', $2, $3)",
    )
    .bind(ticket_id)
    .bind(timestamp)
    .bind(timestamp)
    .execute(pool)
    .await?;
    sqlx::query("UPDATE ticket SET status = 0, reply_status = 1, updated_at = $1 WHERE id = $2")
        .bind(timestamp)
        .bind(ticket_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn late_payment_reconciliation(
    pool: &PgPool,
    database_url: &str,
    redis_url: &str,
) -> Result<()> {
    let user_id = insert_user(pool, "late-payment", "not-used").await?;
    let payment_uuid = Uuid::new_v4().simple().to_string();
    let now = Utc::now().timestamp();
    let payment_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO payment_method (
            uuid, payment, name, config, enable, created_at, updated_at
        ) VALUES ($1, 'EPay', 'integration EPay', $2, 1, $3, $4)
        RETURNING id
        "#,
    )
    .bind(&payment_uuid)
    .bind(serde_json::json!({
        "key": "epay-secret",
        "pid": "integration",
        "url": "https://pay.invalid"
    }))
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    let trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        r#"
        INSERT INTO orders (
            user_id, plan_id, payment_id, type, period, trade_no, total_amount,
            status, commission_status, commission_balance, created_at, updated_at
        ) VALUES ($1, 0, $2, 1, 'deposit', $3, 1234, 2, 0, 0, $4, $5)
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
        installation_id(pool).await?,
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
    let (rows, occurrences, order_status): (i64, i64, i16) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*),
            COALESCE(MAX(occurrence_count), 0)::BIGINT,
            (SELECT status FROM orders WHERE trade_no = $1)
        FROM payment_reconciliation
        WHERE payment_id = $2 AND callback_no = 'EPAY-LATE-INTEGRATION'
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
        FROM payment_reconciliation
        WHERE payment_id = $1 AND callback_no = 'EPAY-AMOUNT-MISMATCH'
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
            "SELECT reason, trade_no, callback_no FROM payment_reconciliation \
             WHERE payment_id = $1 AND callback_no_hash = $2",
        )
        .bind(payment_id)
        .bind(sha256_bytes(&unknown_callback_no))
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
        "SELECT trade_no_hash = $1 \
         FROM payment_reconciliation \
         WHERE payment_id = $2 AND callback_no_hash = $3",
    )
    .bind(sha256_bytes(&missing_trade_no))
    .bind(payment_id)
    .bind(sha256_bytes(&unknown_callback_no))
    .fetch_one(pool)
    .await?;
    ensure!(
        trade_hash_matches,
        "bounded reconciliation label did not retain the raw trade identity hash"
    );
    let archived_state: (i16, Option<i64>) =
        sqlx::query_as("SELECT enable, archived_at FROM payment_method WHERE id = $1")
            .bind(payment_id)
            .fetch_one(pool)
            .await?;
    ensure!(
        archived_state.0 == 0 && archived_state.1.is_some(),
        "delayed callbacks did not preserve the archived verification version"
    );
    let expected_callback_hash_hex = sha256_hex(&unknown_callback_no);
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
        INSERT INTO orders (
            user_id, plan_id, payment_id, type, period, trade_no, total_amount,
            status, commission_status, commission_balance, created_at, updated_at
        ) VALUES ($1, 0, $2, 1, 'deposit', $3, 200, 0, 0, 0, $4, $5)
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
        i16,
        String,
        i32,
        bool,
    ) = sqlx::query_as(
        "SELECT status, callback_no, OCTET_LENGTH(callback_no), \
                    callback_no_hash = $1 \
             FROM orders WHERE trade_no = $2",
    )
    .bind(sha256_bytes(&paid_callback_no))
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
        "SELECT COUNT(*) FROM payment_reconciliation \
         WHERE payment_id = $1 AND callback_no_hash = $2",
    )
    .bind(payment_id)
    .bind(sha256_bytes(&paid_callback_no))
    .fetch_one(pool)
    .await?;
    ensure!(
        unexpected_reconciliation == 0,
        "an ordinary oversized callback replay was misclassified for reconciliation"
    );

    Ok(())
}

/// The exact serialized key set of every reachable admin endpoint that emits a
/// SQL-projected row. Each constant is the producer-side contract for one
/// endpoint's row shape: a new or removed key here is an API change the admin
/// frontend and its api-client contracts must consciously absorb, never an
/// accidental projection leak.
const ADMIN_USER_ROW_KEYS: &[&str] = &[
    "id",
    "email",
    "password",
    "balance",
    "commission_balance",
    "transfer_enable",
    "device_limit",
    "u",
    "d",
    "total_used",
    "alive_ip",
    "ips",
    "plan_id",
    "plan_name",
    "group_id",
    "expired_at",
    "uuid",
    "token",
    "subscribe_url",
    "banned",
    "is_admin",
    "is_staff",
    "invite_user_id",
    "discount",
    "commission_type",
    "commission_rate",
    "t",
    "speed_limit",
    "auto_renewal",
    "remind_expire",
    "remind_traffic",
    "remarks",
    "telegram_id",
    "last_login_at",
    "created_at",
    "updated_at",
];

const ADMIN_USER_TRAFFIC_KEYS: &[&str] = &["record_at", "u", "d", "server_rate"];

const ADMIN_STAT_RECORD_KEYS: &[&str] = &[
    "id",
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
];

const ADMIN_SYSTEM_LOG_KEYS: &[&str] = &[
    "id",
    "title",
    "level",
    "host",
    "uri",
    "method",
    "data",
    "ip",
    "context",
    "created_at",
    "updated_at",
];

const ADMIN_KNOWLEDGE_LIST_KEYS: &[&str] =
    &["id", "category", "title", "sort", "show", "updated_at"];

const ADMIN_KNOWLEDGE_DETAIL_KEYS: &[&str] = &[
    "id",
    "language",
    "category",
    "title",
    "body",
    "sort",
    "show",
    "created_at",
    "updated_at",
];

const ADMIN_TICKET_ROW_KEYS: &[&str] = &[
    "id",
    "user_id",
    "subject",
    "level",
    "status",
    "reply_status",
    "last_reply_user_id",
    "created_at",
    "updated_at",
];

const ADMIN_TICKET_MESSAGE_KEYS: &[&str] = &[
    "id",
    "user_id",
    "ticket_id",
    "message",
    "is_me",
    "created_at",
    "updated_at",
];

const ADMIN_COUPON_KEYS: &[&str] = &[
    "id",
    "code",
    "name",
    "type",
    "value",
    "show",
    "limit_use",
    "limit_use_with_user",
    "limit_plan_ids",
    "limit_period",
    "started_at",
    "ended_at",
    "created_at",
    "updated_at",
];

const ADMIN_GIFTCARD_KEYS: &[&str] = &[
    "id",
    "code",
    "name",
    "type",
    "value",
    "plan_id",
    "limit_use",
    "used_user_ids",
    "started_at",
    "ended_at",
    "created_at",
    "updated_at",
];

const ADMIN_ORDER_FETCH_KEYS: &[&str] = &[
    "id",
    "invite_user_id",
    "user_id",
    "email",
    "plan_id",
    "plan_name",
    "coupon_id",
    "type",
    "period",
    "trade_no",
    "callback_no",
    "total_amount",
    "handling_amount",
    "discount_amount",
    "surplus_amount",
    "refund_amount",
    "balance_amount",
    "surplus_order_ids",
    "status",
    "commission_status",
    "commission_balance",
    "actual_commission_balance",
    "payment_id",
    "payment_reconciliation_open_count",
    "paid_at",
    "created_at",
    "updated_at",
];

const ADMIN_ORDER_DETAIL_KEYS: &[&str] = &[
    "id",
    "invite_user_id",
    "user_id",
    "plan_id",
    "coupon_id",
    "type",
    "period",
    "trade_no",
    "callback_no",
    "total_amount",
    "handling_amount",
    "discount_amount",
    "surplus_amount",
    "refund_amount",
    "balance_amount",
    "surplus_order_ids",
    "status",
    "commission_status",
    "commission_balance",
    "actual_commission_balance",
    "payment_id",
    "paid_at",
    "created_at",
    "updated_at",
    "commission_log",
    "payment_reconciliations",
];

const ADMIN_COMMISSION_LOG_KEYS: &[&str] = &[
    "id",
    "invite_user_id",
    "user_id",
    "trade_no",
    "order_amount",
    "get_amount",
    "created_at",
    "updated_at",
];

const ADMIN_ORDER_RECONCILIATION_KEYS: &[&str] = &[
    "id",
    "payment_id",
    "provider",
    "trade_no",
    "trade_no_hash",
    "callback_no",
    "callback_no_hash",
    "reason",
    "order_status",
    "expected_amount",
    "settled_amount",
    "occurrence_count",
    "first_seen_at",
    "last_seen_at",
    "resolved_at",
    "resolution",
];

const ADMIN_RECONCILIATION_FETCH_KEYS: &[&str] = &[
    "id",
    "payment_id",
    "payment_name",
    "payment_archived_at",
    "provider",
    "trade_no",
    "trade_no_hash",
    "callback_no",
    "callback_no_hash",
    "reason",
    "order_status",
    "expected_amount",
    "settled_amount",
    "occurrence_count",
    "first_seen_at",
    "last_seen_at",
    "resolved_at",
    "resolution",
];

const ADMIN_SERVER_GROUP_LIST_KEYS: &[&str] = &[
    "id",
    "name",
    "created_at",
    "updated_at",
    "user_count",
    "server_count",
];

const ADMIN_SERVER_GROUP_SINGLE_KEYS: &[&str] = &["id", "name", "created_at", "updated_at"];

const ADMIN_SERVER_ROUTE_KEYS: &[&str] = &[
    "id",
    "remarks",
    "match",
    "action",
    "action_value",
    "created_at",
    "updated_at",
];

const ADMIN_SHADOWSOCKS_NODE_KEYS: &[&str] = &[
    "id",
    "group_id",
    "route_id",
    "parent_id",
    "tags",
    "name",
    "rate",
    "host",
    "port",
    "server_port",
    "cipher",
    "obfs",
    "obfs_settings",
    "show",
    "sort",
    "created_at",
    "updated_at",
    "type",
    "online",
    "last_check_at",
    "last_push_at",
    "available_status",
    "api_key",
];

fn assert_exact_keys(context: &str, value: &serde_json::Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .with_context(|| format!("{context}: row is not a JSON object"))?;
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    if actual != expected {
        let unexpected: Vec<_> = actual.difference(&expected).collect();
        let missing: Vec<_> = expected.difference(&actual).collect();
        bail!(
            "{context}: serialized key set drifted (unexpected {unexpected:?}, missing {missing:?})"
        );
    }
    Ok(())
}

fn admin_page_rows(output: AdminOutput, context: &str) -> Result<Vec<serde_json::Value>> {
    match output {
        AdminOutput::Page { data, .. } => {
            ensure!(
                !data.is_empty(),
                "{context}: paged response returned no rows"
            );
            Ok(data)
        }
        _ => bail!("{context}: expected a paged admin response"),
    }
}

fn admin_data(output: AdminOutput, context: &str) -> Result<serde_json::Value> {
    match output {
        AdminOutput::Data(value) => Ok(value),
        _ => bail!("{context}: expected a data admin response"),
    }
}

fn admin_data_rows(output: AdminOutput, context: &str) -> Result<Vec<serde_json::Value>> {
    let rows = admin_data(output, context)?
        .as_array()
        .cloned()
        .with_context(|| format!("{context}: response is not a JSON array"))?;
    ensure!(!rows.is_empty(), "{context}: response returned no rows");
    Ok(rows)
}

/// Pins the exact serialized key set of every reachable admin endpoint whose
/// rows are produced by a SQL projection, using the real AdminService against
/// the migrated schema. This is the DB-backed producer-side contract: any
/// projection edit that adds, renames, or drops a key fails here before it can
/// silently leak (or break) a field the admin frontend consumes.
async fn admin_projection_key_sets(pool: &PgPool, redis_url: &str) -> Result<()> {
    let now = Utc::now().timestamp();
    let admin = AdminService::new(
        pool.clone(),
        redis::Client::open(redis_url)?,
        installation_id(pool).await?,
        Arc::new(integration_config(pool, redis_url)?),
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()?,
        PasswordKdf::new(1),
        SmtpTransportCache::default(),
    );

    // Users: one inviter-linked target so the detail endpoint exercises the
    // conditional invite_user attachment, and the inviter itself as the
    // inviter-less shape.
    let inviter_id = insert_user(pool, "projection-inviter", "hash").await?;
    let user_id = insert_user(pool, "projection-user", "hash").await?;
    sqlx::query("UPDATE users SET invite_user_id = $1 WHERE id = $2")
        .bind(inviter_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    for row in admin_page_rows(admin.get("user/fetch", HashMap::new()).await?, "user/fetch")? {
        assert_exact_keys("user/fetch", &row, ADMIN_USER_ROW_KEYS)?;
    }

    let with_inviter = admin_data(
        admin
            .get(
                "user/getUserInfoById",
                HashMap::from([("id".to_string(), user_id.to_string())]),
            )
            .await?,
        "user/getUserInfoById",
    )?;
    let mut detail_keys = ADMIN_USER_ROW_KEYS.to_vec();
    detail_keys.push("invite_user");
    assert_exact_keys(
        "user/getUserInfoById (invited)",
        &with_inviter,
        &detail_keys,
    )?;
    assert_exact_keys(
        "user/getUserInfoById invite_user",
        &with_inviter["invite_user"],
        &["id", "email"],
    )?;
    let without_inviter = admin_data(
        admin
            .get(
                "user/getUserInfoById",
                HashMap::from([("id".to_string(), inviter_id.to_string())]),
            )
            .await?,
        "user/getUserInfoById",
    )?;
    assert_exact_keys(
        "user/getUserInfoById (no inviter)",
        &without_inviter,
        ADMIN_USER_ROW_KEYS,
    )?;
    let staff_detail = admin_data(
        admin
            .staff_get(
                "user/getUserInfoById",
                HashMap::from([("id".to_string(), user_id.to_string())]),
            )
            .await?,
        "staff user/getUserInfoById",
    )?;
    assert_exact_keys(
        "staff user/getUserInfoById",
        &staff_detail,
        ADMIN_USER_ROW_KEYS,
    )?;

    // Per-user traffic history.
    sqlx::query(
        "INSERT INTO user_traffic (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at) \
         VALUES ($1, 1.00, 100, 200, 'd', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    for row in admin_page_rows(
        admin
            .get(
                "stat/getStatUser",
                HashMap::from([("user_id".to_string(), user_id.to_string())]),
            )
            .await?,
        "stat/getStatUser",
    )? {
        assert_exact_keys("stat/getStatUser", &row, ADMIN_USER_TRAFFIC_KEYS)?;
    }

    // Aggregated stat records.
    sqlx::query(
        "INSERT INTO stat (record_at, record_type, order_count, order_total, commission_count, \
         commission_total, paid_count, paid_total, register_count, invite_count, \
         transfer_used_total, created_at, updated_at) \
         VALUES ($1, 'd', 0, 0, 0, 0, 0, 0, 0, 0, '0', $2, $3)",
    )
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    for row in admin_page_rows(
        admin.get("stat/getStatRecord", HashMap::new()).await?,
        "stat/getStatRecord",
    )? {
        assert_exact_keys("stat/getStatRecord", &row, ADMIN_STAT_RECORD_KEYS)?;
    }

    // System log.
    sqlx::query(
        "INSERT INTO system_log (title, level, host, uri, method, data, ip, context, created_at, updated_at) \
         VALUES ('projection pin', 'info', 'localhost', '/projection', 'GET', NULL, '127.0.0.1', NULL, $1, $2)",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    // W9 modern route: GET system/logs (§8 pagination + §7 DSL). The row key
    // set is unchanged from the legacy projection.
    let (log_rows, log_total) = admin
        .system_logs(
            v2board_compat::Pagination::resolve(None, None, 10)
                .map_err(|problem| anyhow::anyhow!("system/logs pagination: {problem:?}"))?,
            Some(r#"[{"field":"level","op":"eq","value":"info"}]"#),
            None,
            None,
        )
        .await?;
    if log_total < 1 || log_rows.is_empty() {
        anyhow::bail!("system/logs must return the seeded projection row");
    }
    for row in log_rows {
        assert_exact_keys("system/logs", &row, ADMIN_SYSTEM_LOG_KEYS)?;
    }

    // Knowledge list + detail (W10 modern routes: GET knowledge,
    // GET knowledge/{id} — key names unchanged; `show` is boolean and the
    // timestamps are RFC 3339 strings on the modern wire).
    let knowledge_id: i32 = sqlx::query_scalar(
        "INSERT INTO knowledge (language, category, title, body, sort, show, created_at, updated_at) \
         VALUES ('zh-CN', 'projection', 'projection pin', 'body', NULL, 0, $1, $2) RETURNING id",
    )
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    let knowledge_rows = admin.knowledge_list().await?;
    ensure!(
        !knowledge_rows.is_empty(),
        "knowledge list must return the seeded projection row"
    );
    for row in &knowledge_rows {
        assert_exact_keys(
            "knowledge",
            &serde_json::to_value(row)?,
            ADMIN_KNOWLEDGE_LIST_KEYS,
        )?;
    }
    let knowledge_detail = admin.knowledge_detail(i64::from(knowledge_id)).await?;
    assert_exact_keys(
        "knowledge detail",
        &serde_json::to_value(&knowledge_detail)?,
        ADMIN_KNOWLEDGE_DETAIL_KEYS,
    )?;

    // Ticket list + detail with one message.
    let ticket_id: i64 = sqlx::query_scalar(
        "INSERT INTO ticket (user_id, subject, level, status, reply_status, created_at, updated_at) \
         VALUES ($1, 'projection pin', 1, 0, 0, $2, $3) RETURNING id",
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO ticket_message (user_id, ticket_id, message, created_at, updated_at) \
         VALUES ($1, $2, 'projection message', $3, $4)",
    )
    .bind(user_id)
    .bind(ticket_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    for row in admin_page_rows(
        admin.get("ticket/fetch", HashMap::new()).await?,
        "ticket/fetch",
    )? {
        assert_exact_keys("ticket/fetch", &row, ADMIN_TICKET_ROW_KEYS)?;
    }
    let ticket_detail = admin_data(
        admin
            .get(
                "ticket/fetch",
                HashMap::from([("id".to_string(), ticket_id.to_string())]),
            )
            .await?,
        "ticket/fetch detail",
    )?;
    let mut ticket_detail_keys = ADMIN_TICKET_ROW_KEYS.to_vec();
    ticket_detail_keys.push("message");
    assert_exact_keys("ticket/fetch detail", &ticket_detail, &ticket_detail_keys)?;
    let ticket_messages = ticket_detail["message"]
        .as_array()
        .context("ticket/fetch detail: message is not an array")?;
    ensure!(
        !ticket_messages.is_empty(),
        "ticket/fetch detail returned no messages"
    );
    for message in ticket_messages {
        assert_exact_keys("ticket/fetch message", message, ADMIN_TICKET_MESSAGE_KEYS)?;
    }

    // Coupons and gift cards.
    sqlx::query(
        "INSERT INTO coupon (code, name, type, value, show, started_at, ended_at, created_at, updated_at) \
         VALUES ($1, 'projection pin', 1, 100, 0, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now)
    .bind(now + 3600)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let (coupon_rows, coupon_total) = admin
        .coupons_list(
            v2board_compat::Pagination {
                page: 1,
                per_page: 10,
            },
            None,
            None,
        )
        .await?;
    ensure!(
        coupon_total >= 1 && !coupon_rows.is_empty(),
        "coupons list must return the seeded projection row"
    );
    for row in &coupon_rows {
        assert_exact_keys("coupons", &serde_json::to_value(row)?, ADMIN_COUPON_KEYS)?;
    }
    sqlx::query(
        "INSERT INTO gift_card (code, name, type, value, started_at, ended_at, created_at, updated_at) \
         VALUES ($1, 'projection pin', 1, 100, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now)
    .bind(now + 3600)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let (giftcard_rows, giftcard_total) = admin
        .giftcards_list(
            v2board_compat::Pagination {
                page: 1,
                per_page: 10,
            },
            None,
            None,
        )
        .await?;
    ensure!(
        giftcard_total >= 1 && !giftcard_rows.is_empty(),
        "gift-cards list must return the seeded projection row"
    );
    for row in &giftcard_rows {
        assert_exact_keys(
            "gift-cards",
            &serde_json::to_value(row)?,
            ADMIN_GIFTCARD_KEYS,
        )?;
    }

    // Orders: a pending order with one commission log and one open
    // reconciliation row bound to the same trade identity.
    let trade_no = Uuid::new_v4().hyphenated().to_string();
    let order_id: i64 = sqlx::query_scalar(
        "INSERT INTO orders (user_id, plan_id, type, period, trade_no, total_amount, status, \
         commission_status, commission_balance, created_at, updated_at) \
         VALUES ($1, 0, 1, 'deposit', $2, 500, 0, 0, 0, $3, $4) RETURNING id",
    )
    .bind(user_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO commission_log (invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at) \
         VALUES ($1, $2, $3, 500, 50, $4, $5)",
    )
    .bind(inviter_id)
    .bind(user_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let projection_payment_id: i32 = sqlx::query_scalar(
        "INSERT INTO payment_method (uuid, payment, name, config, enable, created_at, updated_at) \
         VALUES ($1, 'EPay', 'projection pin', $2, 0, $3, $4) RETURNING id",
    )
    .bind(Uuid::new_v4().simple().to_string())
    .bind(serde_json::json!({ "key": "k", "pid": "p", "url": "https://pay.invalid" }))
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    let callback_no = format!("PROJECTION-{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO payment_reconciliation (payment_id, provider, trade_no, trade_no_hash, \
         callback_no, callback_no_hash, reason, order_status, expected_amount, settled_amount, \
         occurrence_count, first_seen_at, last_seen_at) \
         VALUES ($1, 'EPay', $2, $3, $4, $5, 'order_not_found', 0, 500, NULL, 1, $6, $7)",
    )
    .bind(projection_payment_id)
    .bind(&trade_no)
    .bind(sha256_bytes(&trade_no))
    .bind(&callback_no)
    .bind(sha256_bytes(&callback_no))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    for row in admin_page_rows(
        admin.get("order/fetch", HashMap::new()).await?,
        "order/fetch",
    )? {
        assert_exact_keys("order/fetch", &row, ADMIN_ORDER_FETCH_KEYS)?;
    }
    let order_detail = admin_data(
        admin
            .post(
                "order/detail",
                HashMap::from([("id".to_string(), order_id.to_string())]),
            )
            .await?,
        "order/detail",
    )?;
    assert_exact_keys("order/detail", &order_detail, ADMIN_ORDER_DETAIL_KEYS)?;
    let commission_rows = order_detail["commission_log"]
        .as_array()
        .context("order/detail: commission_log is not an array")?;
    ensure!(
        !commission_rows.is_empty(),
        "order/detail returned no commission log rows"
    );
    for row in commission_rows {
        assert_exact_keys(
            "order/detail commission_log",
            row,
            ADMIN_COMMISSION_LOG_KEYS,
        )?;
    }
    let reconciliation_rows = order_detail["payment_reconciliations"]
        .as_array()
        .context("order/detail: payment_reconciliations is not an array")?;
    ensure!(
        !reconciliation_rows.is_empty(),
        "order/detail returned no reconciliation rows"
    );
    for row in reconciliation_rows {
        assert_exact_keys(
            "order/detail payment_reconciliations",
            row,
            ADMIN_ORDER_RECONCILIATION_KEYS,
        )?;
    }
    for row in admin_page_rows(
        admin
            .get(
                "order/reconciliation/fetch",
                HashMap::from([("trade_no".to_string(), trade_no.clone())]),
            )
            .await?,
        "order/reconciliation/fetch",
    )? {
        assert_exact_keys(
            "order/reconciliation/fetch",
            &row,
            ADMIN_RECONCILIATION_FETCH_KEYS,
        )?;
    }

    // Server groups, routes, and one shadowsocks node through getNodes.
    let group_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) VALUES ('projection pin', $1, $2) RETURNING id",
    )
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    for row in admin_data_rows(
        admin.get("server/group/fetch", HashMap::new()).await?,
        "server/group/fetch",
    )? {
        assert_exact_keys("server/group/fetch", &row, ADMIN_SERVER_GROUP_LIST_KEYS)?;
    }
    let single_group = admin_data_rows(
        admin
            .get(
                "server/group/fetch",
                HashMap::from([("group_id".to_string(), group_id.to_string())]),
            )
            .await?,
        "server/group/fetch single",
    )?;
    assert_exact_keys(
        "server/group/fetch single",
        &single_group[0],
        ADMIN_SERVER_GROUP_SINGLE_KEYS,
    )?;

    sqlx::query(
        "INSERT INTO server_route (remarks, match, action, action_value, created_at, updated_at) \
         VALUES ('projection pin', '[\"*\"]'::jsonb, 'block', NULL, $1, $2)",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    for row in admin_data_rows(
        admin.get("server/route/fetch", HashMap::new()).await?,
        "server/route/fetch",
    )? {
        assert_exact_keys("server/route/fetch", &row, ADMIN_SERVER_ROUTE_KEYS)?;
    }

    let node_name = format!("projection-node-{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO server_shadowsocks (group_id, name, rate, host, port, server_port, cipher, created_at, updated_at) \
         VALUES ('[1]'::jsonb, $1, '1', 'ss.projection.test', '443', 443, 'aes-128-gcm', $2, $3)",
    )
    .bind(&node_name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let nodes = admin_data_rows(
        admin.get("server/manage/getNodes", HashMap::new()).await?,
        "server/manage/getNodes",
    )?;
    let node = nodes
        .iter()
        .find(|node| node["name"].as_str() == Some(node_name.as_str()))
        .context("seeded shadowsocks node missing from server/manage/getNodes")?;
    assert_exact_keys(
        "server/manage/getNodes shadowsocks",
        node,
        ADMIN_SHADOWSOCKS_NODE_KEYS,
    )?;

    Ok(())
}

async fn node_identity_epoch(pool: &PgPool) -> Result<()> {
    let node_id = 700_000 + i32::from(Uuid::new_v4().as_bytes()[0]);
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO server_credential \
         (node_type, node_id, credential_epoch, updated_at) VALUES ('v2node', $1, 0, $2)",
    )
    .bind(node_id)
    .bind(now)
    .execute(pool)
    .await?;
    let master = "integration-only-node-master-key-with-enough-entropy";
    let epoch: i64 = sqlx::query_scalar(
        "SELECT credential_epoch FROM server_credential WHERE node_type = 'v2node' AND node_id = $1",
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
        "UPDATE server_credential SET credential_epoch = credential_epoch + 1, updated_at = $1 \
         WHERE node_type = 'v2node' AND node_id = $2",
    )
    .bind(now + 1)
    .bind(node_id)
    .execute(pool)
    .await?;
    let revoked_epoch: i64 = sqlx::query_scalar(
        "SELECT credential_epoch FROM server_credential WHERE node_type = 'v2node' AND node_id = $1",
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

async fn auth_rate_limits(pool: &PgPool, database_url: &str, redis_url: &str) -> Result<()> {
    let redis = redis::Client::open(redis_url)?;
    let redis_keys = RedisKeyspace::new(installation_id(pool).await?);
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
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
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
        .zcard(redis_keys.key(&format!("REGISTER_IP_RATE_LIMIT_V2_{registration_ip}")))
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
    let keys: Vec<String> = conn
        .keys(redis_keys.pattern("PASSWORD_ERROR_LIMIT_*"))
        .await?;
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
    pool: &PgPool,
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
        .env("V2BOARD_PEER_DATABASE_PRINCIPAL", "v2board_api")
        .env("REDIS_URL", redis_url)
        .env("V2BOARD_ENV", "testing")
        .env("V2BOARD_SEED_LOCAL", "0")
        .env("V2BOARD_RUNTIME_ROOT", runtime_root)
        .env("V2BOARD_WORKER_HEALTH_FILE", &health_file)
        .env("V2BOARD_WORKER_HEARTBEAT_INTERVAL_SECONDS", "1")
        .env("APP_KEY", INTEGRATION_APP_KEY)
        .env("RUST_LOG", "v2board_workers=error")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawn isolated worker process {worker_bin}"))?;

    let redis_keys = RedisKeyspace::new(installation_id(pool).await?);
    let result = wait_for_worker_health(&mut child, &health_file, redis_url, &redis_keys).await;
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
    redis_keys: &RedisKeyspace,
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
        let heartbeats: BTreeMap<String, i64> = conn
            .hgetall(redis_keys.key("RUST_WORKER_LOOP_HEARTBEAT_AT"))
            .await?;
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

async fn migration_readiness_failure_modes(pool: &PgPool) -> Result<()> {
    let latest = MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| migration.version)
        .max()
        .context("embedded migration list is empty")?;
    let deleted = sqlx::query("DELETE FROM _sqlx_migrations WHERE version = $1")
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

async fn auth_service(pool: &PgPool, redis_url: &str, config: AppConfig) -> Result<AuthService> {
    let redis = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(redis).await?;
    Ok(AuthService::new(
        pool.clone(),
        manager,
        installation_id(pool).await?,
        Arc::new(config),
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()?,
        PasswordKdf::new(4),
        SmtpTransportCache::default(),
    ))
}

pub(crate) fn integration_config(_pool: &PgPool, redis_url: &str) -> Result<AppConfig> {
    let mut config = AppConfig::try_from_api_env().context("load integration AppConfig")?;
    config.environment = RuntimeEnvironment::Testing;
    config.redis_url = redis_url.to_string();
    config.app_key = INTEGRATION_APP_KEY.to_string();
    config.stop_register = false;
    config.email_verify = false;
    config.recaptcha_enable = false;
    config.email_whitelist_enable = false;
    config.email_gmail_limit_enable = false;
    config.try_out_plan_id = 0;
    Ok(config)
}

async fn insert_user(pool: &PgPool, label: &str, password: &str) -> Result<i64> {
    let email = format!("{label}-{}@example.test", Uuid::new_v4().simple());
    insert_user_with_email(pool, &email, password).await
}

async fn insert_user_with_email(pool: &PgPool, email: &str, password: &str) -> Result<i64> {
    let now = Utc::now().timestamp();
    sqlx::query_scalar(
        "INSERT INTO users (email, password, uuid, token, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(email)
    .bind(password)
    .bind(Uuid::new_v4().hyphenated().to_string())
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
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
            .env("V2BOARD_PEER_DATABASE_PRINCIPAL", "v2board_api")
            .env("REDIS_URL", &redis_url)
            .env("V2BOARD_ENV", "testing")
            .env("V2BOARD_SEED_LOCAL", "0")
            .env("V2BOARD_RUNTIME_ROOT", runtime_root)
            .env("APP_KEY", INTEGRATION_APP_KEY)
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

pub(crate) async fn flush_redis(redis: &redis::Client) -> Result<()> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    redis::cmd("FLUSHDB").query_async::<()>(&mut conn).await?;
    Ok(())
}

pub(crate) async fn create_database(
    root: &PgPool,
    database_name: &GeneratedDatabaseName,
) -> Result<()> {
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE {} WITH TEMPLATE template0 ENCODING 'UTF8'",
        database_name.quoted()
    )))
    .execute(root)
    .await?;
    Ok(())
}

pub(crate) async fn drop_database(
    root: &PgPool,
    database_name: &GeneratedDatabaseName,
) -> Result<()> {
    // A failed invariant may leave pooled or child-process sessions behind.
    // Terminate them by a bound value before issuing the necessarily dynamic
    // DROP DATABASE against the validated generated identifier.
    let _: Vec<bool> = sqlx::query_scalar(
        r#"
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = $1 AND pid <> pg_backend_pid()
        "#,
    )
    .bind(database_name.as_str())
    .fetch_all(root)
    .await?;
    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {} WITH (FORCE)",
        database_name.quoted()
    )))
    .execute(root)
    .await?;
    Ok(())
}

pub(crate) fn database_url_for(
    root_database_url: &str,
    database_name: &GeneratedDatabaseName,
) -> Result<String> {
    let mut url = Url::parse(root_database_url).context("parse integration root database URL")?;
    url.set_path(&format!("/{}", database_name.as_str()));
    Ok(url.to_string())
}

#[derive(Debug)]
pub(crate) struct GeneratedDatabaseName(String);

impl GeneratedDatabaseName {
    pub(crate) fn new(label: &str) -> Result<Self> {
        ensure!(
            !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()),
            "unsafe generated database label"
        );
        let value = format!(
            "v2board_{label}_{}",
            &Uuid::new_v4().simple().to_string()[..16]
        );
        validate_generated_database_name(&value)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn quoted(&self) -> String {
        // Validation excludes quotes and every other escaping case.
        format!("\"{}\"", self.0)
    }
}

fn validate_generated_database_name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 63
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
        "unsafe generated SQL identifier"
    );
    Ok(())
}

fn sha256_bytes(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn random_traffic_key() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(crate) fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn pass(name: &str) {
    println!("PASS {name}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_database_names_are_quote_safe_and_bounded() {
        let name = GeneratedDatabaseName::new("contract").unwrap();
        assert!(name.as_str().starts_with("v2board_contract_"));
        assert!(name.as_str().len() <= 63);
        assert_eq!(name.quoted(), format!("\"{}\"", name.as_str()));
        assert!(validate_generated_database_name("bad\";drop database postgres").is_err());
        assert!(validate_generated_database_name("Uppercase").is_err());
    }
}
