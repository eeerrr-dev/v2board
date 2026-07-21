use chrono::Utc;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;
use v2board_analytics::{
    AnalyticsAdmissionPolicy, install_analytics_admission_policy, refresh_analytics_admission,
};
use v2board_application::{
    worker_statistics::{StatisticsWindow, StatisticsWorkerService},
    worker_traffic::TrafficAccountingRepository,
};
use v2board_db::{
    worker_statistics::PostgresStatisticsWorkerRepository,
    worker_traffic::PostgresTrafficAccountingRepository,
};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn traffic_accounting_and_daily_statistics_are_real_postgres_ports() {
    let Some(pool) = integration_pool().await else {
        return;
    };
    let installation_id = install_analytics_admission(&pool).await;
    let now = Utc::now().timestamp();
    let marker = Uuid::new_v4().simple().to_string();
    let server_id = i32::try_from(
        u32::from_be_bytes(marker.as_bytes()[..4].try_into().unwrap()) % (i32::MAX as u32 - 1),
    )
    .unwrap()
        + 1;
    let inviter_id = insert_user(&pool, &format!("a{}", &marker[..12]), None, now).await;
    let user_id = insert_user(&pool, &format!("b{}", &marker[..12]), Some(inviter_id), now).await;
    sqlx::query("UPDATE users SET u = 5, d = 7, traffic_epoch = 2 WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("seed accounting user state");

    let paid_trade = marker.clone();
    let unpaid_trade = marker.chars().rev().collect::<String>();
    sqlx::query(
        "INSERT INTO orders \
         (user_id, plan_id, type, period, trade_no, total_amount, status, paid_at, created_at, updated_at) \
         VALUES ($1, 0, 1, 'month', $2, 100, 3, $3, $3, $3), \
                ($1, 0, 1, 'month', $4, 20, 0, NULL, $3, $3)",
    )
    .bind(user_id)
    .bind(&paid_trade)
    .bind(now)
    .bind(&unpaid_trade)
    .execute(&pool)
    .await
    .expect("seed statistic orders");
    sqlx::query(
        "INSERT INTO commission_log \
         (invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at) \
         VALUES ($1, $2, $3, 100, 7, $4, $4)",
    )
    .bind(inviter_id)
    .bind(user_id)
    .bind(&paid_trade)
    .bind(now)
    .execute(&pool)
    .await
    .expect("seed statistic commission");
    sqlx::query(
        "INSERT INTO server_traffic \
         (server_id, server_type, u, d, record_type, record_at, created_at, updated_at) \
         VALUES ($1, 'integration', 10, 20, 'd', $2, $3, $3)",
    )
    .bind(server_id)
    .bind(now - 100)
    .bind(now)
    .execute(&pool)
    .await
    .expect("seed statistic traffic");

    let statistics =
        StatisticsWorkerService::new(PostgresStatisticsWorkerRepository::new(pool.clone()))
            .run(
                StatisticsWindow {
                    start_at: now - 10,
                    end_at: now + 10,
                },
                now + 1,
            )
            .await
            .expect("aggregate and persist daily statistics");
    assert_eq!(
        (
            statistics.order_count,
            statistics.order_total,
            statistics.paid_count,
            statistics.paid_total,
        ),
        (2, 120, 1, 100)
    );
    assert_eq!(
        (
            statistics.commission_count,
            statistics.commission_total,
            statistics.register_count,
            statistics.invite_count,
            statistics.transfer_used_total,
        ),
        (1, 7, 2, 1, 30)
    );
    let stored: (i32, i64, String) = sqlx::query_as(
        "SELECT order_count, order_total, transfer_used_total FROM stat WHERE record_at = $1",
    )
    .bind(now - 10)
    .fetch_one(&pool)
    .await
    .expect("load persisted daily statistics");
    assert_eq!(stored, (2, 120, "30".to_string()));

    let report_key = format!("{}{}", marker, marker);
    insert_traffic_report(&pool, &report_key, user_id, "explicit", now).await;
    let repository = PostgresTrafficAccountingRepository::new(pool.clone());
    let applied = repository
        .apply_next(&installation_id.to_string(), now + 2)
        .await
        .expect("apply current-epoch traffic report")
        .expect("traffic report was claimed");
    assert_eq!(applied.report_key, report_key);
    assert_eq!((applied.stale_items, applied.missing_users), (0, 0));
    let counters: (i64, i64, i64) =
        sqlx::query_as("SELECT u, d, traffic_epoch FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .expect("load accounted user");
    assert_eq!(counters, (8, 11, 2));
    assert_report_committed(&pool, &report_key).await;

    let rollback_key = format!("{}{}", unpaid_trade, unpaid_trade);
    insert_traffic_report(&pool, &rollback_key, user_id, "implicit", now + 3).await;
    assert!(
        repository
            .apply_next(&installation_id.to_string(), now + 4)
            .await
            .is_err()
    );
    let rolled_back: (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT u, d, (SELECT applied_at FROM server_traffic_report WHERE report_key = $2) \
         FROM users WHERE id = $1",
    )
    .bind(user_id)
    .bind(&rollback_key)
    .fetch_one(&pool)
    .await
    .expect("verify failed report rollback");
    assert_eq!(rolled_back, (8, 11, None));
    let rolled_back_events: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM analytics_outbox WHERE report_key = $1")
            .bind(&rollback_key)
            .fetch_one(&pool)
            .await
            .expect("verify failed report analytics rollback");
    assert_eq!(rolled_back_events, 0);

    cleanup_fixture(
        &pool,
        &report_key,
        &rollback_key,
        &paid_trade,
        &unpaid_trade,
        server_id,
        now - 10,
        user_id,
        inviter_id,
    )
    .await;
}

async fn insert_user(pool: &PgPool, label: &str, inviter_id: Option<i64>, now: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users \
         (invite_user_id, email, password, uuid, token, transfer_enable, expired_at, created_at, updated_at) \
         VALUES ($1, $2, 'unused', $3, $4, 1000, $5, $6, $6) RETURNING id",
    )
    .bind(inviter_id)
    .bind(format!("{label}@example.test"))
    .bind(Uuid::new_v4().hyphenated().to_string())
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now + 1_000)
    .bind(now)
    .fetch_one(pool)
    .await
    .expect("insert worker repository user")
}

async fn insert_traffic_report(
    pool: &PgPool,
    report_key: &str,
    user_id: i64,
    identity_kind: &str,
    now: i64,
) {
    sqlx::query(
        "INSERT INTO server_traffic_report \
         (report_key, payload_hash, node_id, node_type, rate_text, rate_decimal_10_2, \
          identity_kind, accepted_at, accounting_date, applied_at, created_at, updated_at) \
         VALUES ($1, $2, 1, 'integration', '1', 1.00, $3, $4, $5, NULL, $4, $4)",
    )
    .bind(report_key)
    .bind("b".repeat(64))
    .bind(identity_kind)
    .bind(now)
    .bind(Utc::now().date_naive())
    .execute(pool)
    .await
    .expect("insert durable traffic report");
    sqlx::query(
        "INSERT INTO server_traffic_report_item \
         (report_key, user_id, traffic_epoch, raw_u, raw_d, charged_u, charged_d) \
         VALUES ($1, $2, 2, 1, 2, 3, 4)",
    )
    .bind(report_key)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert durable traffic item");
}

async fn assert_report_committed(pool: &PgPool, report_key: &str) {
    let state: (Option<i64>, i64, i64) = sqlx::query_as(
        "SELECT applied_at, \
                (SELECT COUNT(*) FROM server_traffic_report_item WHERE report_key = $1), \
                (SELECT COUNT(*) FROM analytics_outbox WHERE report_key = $1) \
         FROM server_traffic_report WHERE report_key = $1",
    )
    .bind(report_key)
    .fetch_one(pool)
    .await
    .expect("load applied traffic transaction state");
    assert!(state.0.is_some());
    assert_eq!((state.1, state.2), (0, 1));
}

#[allow(clippy::too_many_arguments)]
async fn cleanup_fixture(
    pool: &PgPool,
    report_key: &str,
    rollback_key: &str,
    paid_trade: &str,
    unpaid_trade: &str,
    server_id: i32,
    statistic_record_at: i64,
    user_id: i64,
    inviter_id: i64,
) {
    sqlx::query("DELETE FROM analytics_outbox WHERE report_key IN ($1, $2)")
        .bind(report_key)
        .bind(rollback_key)
        .execute(pool)
        .await
        .expect("clean worker analytics fixture");
    sqlx::query("DELETE FROM server_traffic_report WHERE report_key IN ($1, $2)")
        .bind(report_key)
        .bind(rollback_key)
        .execute(pool)
        .await
        .expect("clean durable traffic fixture");
    sqlx::query("DELETE FROM stat WHERE record_at = $1")
        .bind(statistic_record_at)
        .execute(pool)
        .await
        .expect("clean statistic fixture");
    sqlx::query("DELETE FROM server_traffic WHERE server_id = $1 AND server_type = 'integration'")
        .bind(server_id)
        .execute(pool)
        .await
        .expect("clean server-traffic fixture");
    sqlx::query("DELETE FROM commission_log WHERE trade_no = $1")
        .bind(paid_trade)
        .execute(pool)
        .await
        .expect("clean commission fixture");
    sqlx::query("DELETE FROM orders WHERE trade_no IN ($1, $2)")
        .bind(paid_trade)
        .bind(unpaid_trade)
        .execute(pool)
        .await
        .expect("clean order fixtures");
    sqlx::query("DELETE FROM users WHERE id IN ($1, $2)")
        .bind(user_id)
        .bind(inviter_id)
        .execute(pool)
        .await
        .expect("clean user fixtures");
}

async fn install_analytics_admission(pool: &PgPool) -> Uuid {
    if let Some(installation_id) =
        sqlx::query_scalar("SELECT installation_id FROM system_installation WHERE singleton = 1")
            .fetch_optional(pool)
            .await
            .expect("load test installation")
    {
        return installation_id;
    }
    let installation_id = Uuid::new_v4();
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO system_installation (singleton, installation_id, created_at) VALUES (1, $1, $2)",
    )
    .bind(installation_id)
    .bind(now)
    .execute(pool)
    .await
    .expect("insert test installation");
    let gib = 1024_u64 * 1024 * 1024;
    install_analytics_admission_policy(
        pool,
        installation_id,
        &AnalyticsAdmissionPolicy {
            recovery_pending_rows: 2_500,
            soft_pending_rows: 3_000,
            hard_pending_rows: 4_000,
            recovery_relation_bytes: 20 * gib,
            soft_relation_bytes: 30 * gib,
            hard_relation_bytes: 40 * gib,
            recovery_oldest_age_seconds: 60,
            soft_oldest_age_seconds: 300,
            hard_oldest_age_seconds: 3_600,
            database_capacity_bytes: 128 * gib,
            hard_min_headroom_bytes: 16 * gib,
            soft_min_headroom_bytes: 32 * gib,
            recovery_min_headroom_bytes: 48 * gib,
            event_reservation_bytes: 4_096,
            soft_max_new_rows_per_second: 100_000,
            sample_interval_seconds: 1,
            stale_after_seconds: 30,
            capacity_evidence: "worker repository integration test".to_string(),
        },
        now,
    )
    .await
    .expect("install analytics admission policy");
    refresh_analytics_admission(pool)
        .await
        .expect("refresh analytics admission state");
    installation_id
}

async fn integration_pool() -> Option<PgPool> {
    let database_url = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("connect to disposable worker repository database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply PostgreSQL migrations");
    Some(pool)
}
