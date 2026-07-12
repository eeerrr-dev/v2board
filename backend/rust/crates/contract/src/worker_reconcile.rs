use std::{collections::BTreeMap, env};

use anyhow::{Result, bail};
use chrono::Utc;
use redis::AsyncCommands;
use serde_json::{Value, json};
use sqlx::MySqlPool;

pub async fn run() -> Result<()> {
    let database_url = env_or("DATABASE_URL", "mysql://v2board:v2board@mysql:3306/v2board");
    let redis_url = env_or("REDIS_URL", "redis://redis:6379/1");
    let strict = env_bool("WORKER_RECONCILE_STRICT", true);
    let pool = MySqlPool::connect(&database_url).await?;
    let redis = redis::Client::open(redis_url.as_str())?;
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let now = Utc::now().timestamp();
    let mut checks = Vec::new();

    let heartbeat = conn
        .get::<_, Option<i64>>("SCHEDULE_LAST_CHECK_AT_")
        .await?
        .unwrap_or_default();
    let heartbeat_age = (heartbeat > 0).then_some(now.saturating_sub(heartbeat));
    checks.push(ReconcileCheck::new(
        "scheduler_heartbeat_recent",
        heartbeat > 0 && now.saturating_sub(heartbeat) <= 180,
        json!({ "last_seen": heartbeat, "age_seconds": heartbeat_age }),
        true,
    ));

    let last_runs = conn
        .hgetall::<_, BTreeMap<String, i64>>("RUST_WORKER_LAST_RUN_AT")
        .await?;
    for job in ["traffic_update", "check_order", "check_ticket"] {
        let recent = last_runs
            .get(job)
            .map(|last_run| now.saturating_sub(*last_run) <= 180)
            .unwrap_or(false);
        checks.push(ReconcileCheck::new(
            format!("worker_metric_{job}_recent"),
            recent,
            json!({ "last_seen": last_runs.get(job), "age_seconds": last_runs.get(job).map(|last_run| now.saturating_sub(*last_run)) }),
            true,
        ));
    }

    let scheduler_locks = conn.keys::<_, Vec<String>>("RUST_SCHEDULER_LOCK_*").await?;
    checks.push(ReconcileCheck::new(
        "scheduler_locks_released",
        scheduler_locks.is_empty(),
        json!({ "locks": scheduler_locks }),
        true,
    ));

    let durable_traffic_pending = count(
        &pool,
        "SELECT COUNT(*) FROM v2_server_traffic_report WHERE applied_at IS NULL",
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "durable_traffic_reports_drained",
        durable_traffic_pending == 0,
        json!({ "pending_reports": durable_traffic_pending }),
        true,
    ));

    let traffic_reset_lock_exists = conn.exists::<_, bool>("traffic_reset_lock").await?;
    checks.push(ReconcileCheck::new(
        "traffic_reset_lock_absent",
        !traffic_reset_lock_exists,
        json!({ "exists": traffic_reset_lock_exists }),
        true,
    ));

    let pending_paid_orders =
        count(&pool, "SELECT COUNT(*) FROM v2_order WHERE status = 1").await?;
    checks.push(ReconcileCheck::new(
        "paid_orders_opened",
        pending_paid_orders == 0,
        json!({ "status_1_orders": pending_paid_orders }),
        true,
    ));

    let expired_unpaid_orders = count_with_i64(
        &pool,
        "SELECT COUNT(*) FROM v2_order WHERE status = 0 AND created_at <= ?",
        now.saturating_sub(7_200),
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "expired_unpaid_orders_cancelled",
        expired_unpaid_orders == 0,
        json!({ "expired_status_0_orders": expired_unpaid_orders }),
        true,
    ));

    let duplicate_unfinished_users = count(
        &pool,
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT user_id
            FROM v2_order
            WHERE status IN (0, 1)
            GROUP BY user_id
            HAVING COUNT(*) > 1
        ) duplicate_users
        "#,
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "unfinished_order_invariant",
        duplicate_unfinished_users == 0,
        json!({ "users_with_multiple_unfinished_orders": duplicate_unfinished_users }),
        true,
    ));

    let stale_tickets = count_with_i64(
        &pool,
        r#"
        SELECT COUNT(*)
        FROM v2_ticket t
        WHERE t.status = 0
          AND t.reply_status = 1
          AND t.updated_at <= ?
          AND (
            SELECT tm.user_id
            FROM v2_ticket_message tm
            WHERE tm.ticket_id = t.id
            ORDER BY tm.id DESC
            LIMIT 1
          ) <> t.user_id
        "#,
        now.saturating_sub(86_400),
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "stale_answered_tickets_closed",
        stale_tickets == 0,
        json!({ "stale_open_tickets": stale_tickets }),
        true,
    ));

    let commission_ready = count(
        &pool,
        r#"
        SELECT COUNT(*)
        FROM v2_order
        WHERE commission_status = 1
          AND invite_user_id IS NOT NULL
        "#,
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "commission_ready_queue_drained",
        commission_ready == 0,
        json!({ "commission_status_1_orders": commission_ready }),
        false,
    ));

    let yesterday = yesterday_start_timestamp();
    let stat_exists = count_with_i64(
        &pool,
        "SELECT COUNT(*) FROM v2_stat WHERE record_at = ?",
        yesterday,
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "yesterday_statistics_present",
        stat_exists > 0,
        json!({ "record_at": yesterday, "rows": stat_exists }),
        false,
    ));

    report_reconcile_results(&checks, strict)?;
    Ok(())
}

struct ReconcileCheck {
    name: String,
    ok: bool,
    strict: bool,
    details: Value,
}

impl ReconcileCheck {
    fn new(name: impl Into<String>, ok: bool, details: Value, strict: bool) -> Self {
        Self {
            name: name.into(),
            ok,
            strict,
            details,
        }
    }
}

fn report_reconcile_results(checks: &[ReconcileCheck], strict_mode: bool) -> Result<()> {
    let mut failed = 0;
    let mut warnings = 0;
    for check in checks {
        if check.ok {
            println!("PASS {} {}", check.name, check.details);
        } else if check.strict && strict_mode {
            failed += 1;
            println!("FAIL {} {}", check.name, check.details);
        } else {
            warnings += 1;
            println!("WARN {} {}", check.name, check.details);
        }
    }
    if failed > 0 {
        bail!("worker reconciliation failed: {failed} strict checks failed");
    }
    println!(
        "Worker reconciliation OK: {} checks passed, {warnings} warnings.",
        checks.len() - failed - warnings
    );
    Ok(())
}

async fn count(pool: &MySqlPool, sql: &'static str) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

async fn count_with_i64(pool: &MySqlPool, sql: &'static str, value: i64) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(value)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

fn yesterday_start_timestamp() -> i64 {
    let now = Utc::now();
    let today = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight");
    today.and_utc().timestamp().saturating_sub(86_400)
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(default)
}
