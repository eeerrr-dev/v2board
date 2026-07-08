//! v2board background worker.
//!
//! Job side-effects (traffic accounting, order settlement, commission payout,
//! reminder mail, log/traffic resets, statistics) run **synchronously by design**
//! in this release. There is no Horizon-style asynchronous queue: the API request
//! path performs its own work inline, and everything the Laravel scheduler drove
//! (`app/Console/Kernel.php`) is executed here by an in-process
//! [`apalis_cron::CronStream`] scheduler, guarded by a distributed Redis lock and
//! reporting per-job metrics to Redis. Because nothing enqueues Apalis Redis jobs,
//! the vestigial Redis queue consumer has been removed rather than kept as
//! misleading dead code.
use std::{collections::HashMap, env, str::FromStr, time::Duration};

use apalis::prelude::*;
use apalis_cron::{CronStream, Tick};
use chrono::{DateTime, Datelike, FixedOffset, Months, TimeZone, Utc};
use cron::Schedule;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::header::ContentType,
    transport::smtp::authentication::Credentials,
};
use redis::AsyncCommands;
use sqlx::{FromRow, MySql, MySqlPool, QueryBuilder};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use v2board_config::{AppConfig, app_now, app_timezone};
use v2board_domain::order::OrderService;

#[derive(Clone)]
struct WorkerState {
    config: AppConfig,
    db: MySqlPool,
    redis: redis::Client,
}

#[derive(Debug, Clone, Copy)]
enum ScheduledTask {
    TrafficUpdate,
    Statistics,
    CheckOrder,
    CheckCommission,
    CheckTicket,
    CheckRenewal,
    ResetTraffic,
    ResetLog,
    SendRemindMail,
}

#[cfg(test)]
const SCHEDULED_TASK_NAMES: &[&str] = &[
    "traffic_update",
    "statistics",
    "check_order",
    "check_commission",
    "check_ticket",
    "check_renewal",
    "reset_traffic",
    "reset_log",
    "send_remind_mail",
];

#[derive(Debug, Clone)]
struct ScheduledJob {
    name: &'static str,
    task: ScheduledTask,
}

#[derive(Debug, Clone)]
struct SchedulerLock {
    key: String,
    token: String,
}

#[derive(Debug, Clone, FromRow)]
struct CommissionOrderRow {
    id: i64,
    invite_user_id: i64,
    user_id: i64,
    trade_no: String,
    total_amount: i32,
    commission_balance: i32,
    actual_commission_balance: Option<i32>,
}

#[derive(Debug, Clone, FromRow)]
struct InviterRow {
    id: i64,
    invite_user_id: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct RenewalUserRow {
    id: i64,
    balance: i32,
    plan_id: i32,
    expired_at: i64,
}

#[derive(Debug, Clone, FromRow)]
struct RenewalPlanRow {
    id: i32,
    renew: i8,
    month_price: Option<i32>,
    quarter_price: Option<i32>,
    half_year_price: Option<i32>,
    year_price: Option<i32>,
    two_year_price: Option<i32>,
    three_year_price: Option<i32>,
}

#[derive(Debug, Clone, FromRow)]
struct ResetUserRow {
    id: i64,
    expired_at: i64,
    reset_traffic_method: Option<i8>,
}

#[derive(Debug, Clone, FromRow)]
struct ReminderUserRow {
    id: i64,
    email: String,
    remind_expire: Option<i8>,
    remind_traffic: Option<i8>,
    expired_at: Option<i64>,
    u: i64,
    d: i64,
    transfer_enable: i64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::from_env();
    let db = v2board_db::connect_mysql(&config.database_url).await?;
    let redis = redis::Client::open(config.redis_url.clone())?;
    let state = WorkerState { config, db, redis };
    let args = env::args().skip(1).collect::<Vec<_>>();
    if !args.is_empty() {
        return run_command(&args, &state).await;
    }

    run_worker_runtime(state).await
}

async fn run_worker_runtime(state: WorkerState) -> anyhow::Result<()> {
    let traffic_update_schedule = Schedule::from_str("0 * * * * * *")?;
    let statistics_schedule = Schedule::from_str("0 10 0 * * * *")?;
    let check_order_schedule = Schedule::from_str("0 * * * * * *")?;
    let check_commission_schedule = Schedule::from_str("0 0/15 * * * * *")?;
    let check_ticket_schedule = Schedule::from_str("0 * * * * * *")?;
    let check_renewal_schedule = Schedule::from_str("0 30 22 * * * *")?;
    let reset_traffic_schedule = Schedule::from_str("0 0 0 * * * *")?;
    let reset_log_schedule = Schedule::from_str("0 0 0 * * * *")?;
    let send_remind_mail_schedule = Schedule::from_str("0 30 11 * * * *")?;
    let scheduled_jobs = [
        (
            "scheduler-traffic-update",
            traffic_update_schedule,
            ScheduledJob {
                name: "traffic_update",
                task: ScheduledTask::TrafficUpdate,
            },
        ),
        (
            "scheduler-statistics",
            statistics_schedule,
            ScheduledJob {
                name: "statistics",
                task: ScheduledTask::Statistics,
            },
        ),
        (
            "scheduler-check-order",
            check_order_schedule,
            ScheduledJob {
                name: "check_order",
                task: ScheduledTask::CheckOrder,
            },
        ),
        (
            "scheduler-check-commission",
            check_commission_schedule,
            ScheduledJob {
                name: "check_commission",
                task: ScheduledTask::CheckCommission,
            },
        ),
        (
            "scheduler-check-ticket",
            check_ticket_schedule,
            ScheduledJob {
                name: "check_ticket",
                task: ScheduledTask::CheckTicket,
            },
        ),
        (
            "scheduler-check-renewal",
            check_renewal_schedule,
            ScheduledJob {
                name: "check_renewal",
                task: ScheduledTask::CheckRenewal,
            },
        ),
        (
            "scheduler-reset-traffic",
            reset_traffic_schedule,
            ScheduledJob {
                name: "reset_traffic",
                task: ScheduledTask::ResetTraffic,
            },
        ),
        (
            "scheduler-reset-log",
            reset_log_schedule,
            ScheduledJob {
                name: "reset_log",
                task: ScheduledTask::ResetLog,
            },
        ),
        (
            "scheduler-send-remind-mail",
            send_remind_mail_schedule,
            ScheduledJob {
                name: "send_remind_mail",
                task: ScheduledTask::SendRemindMail,
            },
        ),
    ];

    tracing::info!("v2board rust worker starting");

    let mut monitor = Monitor::new();
    for (worker_name, schedule, job) in scheduled_jobs {
        let state = state.clone();
        monitor = monitor.register(move |_| {
            WorkerBuilder::new(worker_name)
                .backend(CronStream::new_with_timezone(
                    schedule.clone(),
                    app_timezone(),
                ))
                .data(job.clone())
                .data(state.clone())
                .build(handle_cron_tick)
        });
    }
    monitor.run().await?;

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("v2board_workers=info,apalis=info"));
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn run_command(args: &[String], state: &WorkerState) -> anyhow::Result<()> {
    match args {
        [command, name] if command == "run-once" => run_scheduled_job_once(name, state).await,
        _ => anyhow::bail!("unknown worker command; expected `run-once <scheduled-job-name>`"),
    }
}

async fn run_scheduled_job_once(name: &str, state: &WorkerState) -> anyhow::Result<()> {
    let task = scheduled_task_by_name(name)
        .ok_or_else(|| anyhow::anyhow!("unknown scheduled job `{name}`"))?;
    mark_scheduler_alive(state).await?;
    let scheduler_lock = acquire_scheduler_lock(state, name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("scheduled job `{name}` is already locked"))?;

    let result = run_scheduled_job(task, state).await;
    let metric_result = if let Err(error) = &result {
        tracing::error!(job = name, ?error, "one-shot scheduled job failed");
        let _ = record_worker_metric(state, name, false).await;
        Ok(())
    } else {
        record_worker_metric(state, name, true).await
    };
    let release_result = release_scheduler_lock(state, scheduler_lock).await;
    metric_result?;
    release_result?;
    result
}

fn scheduled_task_by_name(name: &str) -> Option<ScheduledTask> {
    match name {
        "traffic_update" => Some(ScheduledTask::TrafficUpdate),
        "statistics" => Some(ScheduledTask::Statistics),
        "check_order" => Some(ScheduledTask::CheckOrder),
        "check_commission" => Some(ScheduledTask::CheckCommission),
        "check_ticket" => Some(ScheduledTask::CheckTicket),
        "check_renewal" => Some(ScheduledTask::CheckRenewal),
        "reset_traffic" => Some(ScheduledTask::ResetTraffic),
        "reset_log" => Some(ScheduledTask::ResetLog),
        "send_remind_mail" => Some(ScheduledTask::SendRemindMail),
        _ => None,
    }
}

async fn handle_cron_tick(
    tick: Tick<FixedOffset>,
    job: Data<ScheduledJob>,
    state: Data<WorkerState>,
) -> Result<(), BoxDynError> {
    tracing::info!(
        job = job.name,
        tick = %tick.get_timestamp(),
        "received scheduled job"
    );
    if let Err(error) = mark_scheduler_alive(&state).await {
        tracing::warn!(?error, "failed to update scheduler heartbeat");
    }

    let scheduler_lock = match acquire_scheduler_lock(&state, job.name).await {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            tracing::info!(
                job = job.name,
                "scheduled job skipped because another worker owns it"
            );
            return Ok(());
        }
        Err(error) => {
            tracing::error!(
                job = job.name,
                ?error,
                "failed to acquire scheduled job lock"
            );
            let _ = record_worker_metric(&state, job.name, false).await;
            return Ok(());
        }
    };

    // Run the job in a spawned task so a panic inside it (an unexpected unwrap/index) is caught
    // as a JoinError instead of unwinding past the lock release — otherwise a panicking job would
    // leave RUST_SCHEDULER_LOCK_ held for its full 900s TTL and silently stall the minutely
    // scheduler. The lock is released on every outcome (success, error, panic).
    let job_state = state.clone();
    let task = job.task;
    let job_result = tokio::spawn(async move { run_scheduled_job(task, &job_state).await }).await;
    if let Err(error) = release_scheduler_lock(&state, scheduler_lock).await {
        tracing::warn!(
            job = job.name,
            ?error,
            "failed to release scheduled job lock"
        );
    }
    match job_result {
        Ok(Ok(())) => {
            if let Err(error) = record_worker_metric(&state, job.name, true).await {
                tracing::warn!(
                    job = job.name,
                    ?error,
                    "failed to record scheduled job metric"
                );
            }
        }
        Ok(Err(error)) => {
            tracing::error!(job = job.name, ?error, "scheduled job failed");
            let _ = record_worker_metric(&state, job.name, false).await;
        }
        Err(error) => {
            tracing::error!(job = job.name, %error, "scheduled job panicked");
            let _ = record_worker_metric(&state, job.name, false).await;
        }
    }
    Ok(())
}

async fn run_scheduled_job(task: ScheduledTask, state: &WorkerState) -> anyhow::Result<()> {
    match task {
        ScheduledTask::TrafficUpdate => traffic_update(state).await,
        ScheduledTask::Statistics => statistics(state).await,
        ScheduledTask::CheckOrder => check_order(state).await,
        ScheduledTask::CheckCommission => check_commission(state).await,
        ScheduledTask::CheckTicket => check_ticket(state).await,
        ScheduledTask::CheckRenewal => check_renewal(state).await,
        ScheduledTask::ResetTraffic => reset_traffic(state).await,
        ScheduledTask::ResetLog => reset_log(state).await,
        ScheduledTask::SendRemindMail => send_remind_mail(state).await,
    }
}

async fn mark_scheduler_alive(state: &WorkerState) -> anyhow::Result<()> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: () = conn
        .set("SCHEDULE_LAST_CHECK_AT_", Utc::now().timestamp())
        .await?;
    Ok(())
}

async fn acquire_scheduler_lock(
    state: &WorkerState,
    name: &str,
) -> anyhow::Result<Option<SchedulerLock>> {
    let key = format!("RUST_SCHEDULER_LOCK_{name}");
    let token = Uuid::new_v4().to_string();
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&token)
        .arg("NX")
        .arg("EX")
        .arg(900)
        .query_async(&mut conn)
        .await?;
    Ok(acquired.map(|_| SchedulerLock { key, token }))
}

async fn release_scheduler_lock(
    state: &WorkerState,
    scheduler_lock: SchedulerLock,
) -> anyhow::Result<()> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: i64 = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("DEL", KEYS[1])
        end
        return 0
        "#,
    )
    .key(scheduler_lock.key)
    .arg(scheduler_lock.token)
    .invoke_async(&mut conn)
    .await?;
    Ok(())
}

async fn record_worker_metric(
    state: &WorkerState,
    name: &str,
    success: bool,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: () = conn.hincr("RUST_WORKER_JOBS_TOTAL", name, 1).await?;
    let _: () = conn.hset("RUST_WORKER_LAST_RUN_AT", name, now).await?;
    if success {
        let _: () = conn.hset("RUST_WORKER_LAST_SUCCESS_AT", name, now).await?;
    } else {
        let _: () = conn.hincr("RUST_WORKER_JOBS_FAILED", name, 1).await?;
        let _: () = conn.hset("RUST_WORKER_LAST_FAILURE_AT", name, now).await?;
    }
    Ok(())
}

async fn traffic_update(state: &WorkerState) -> anyhow::Result<()> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    if conn.exists::<_, bool>("traffic_reset_lock").await? {
        return Ok(());
    }
    let uploads = conn
        .hgetall::<_, HashMap<String, i64>>("v2board_upload_traffic")
        .await
        .unwrap_or_default();
    let downloads = conn
        .hgetall::<_, HashMap<String, i64>>("v2board_download_traffic")
        .await
        .unwrap_or_default();
    let _: () = conn.del("v2board_upload_traffic").await?;
    let _: () = conn.del("v2board_download_traffic").await?;
    if uploads.is_empty() && downloads.is_empty() {
        return Ok(());
    }

    let mut user_ids = uploads
        .keys()
        .chain(downloads.keys())
        .filter_map(|id| id.parse::<i64>().ok())
        .collect::<Vec<_>>();
    user_ids.sort_unstable();
    user_ids.dedup();
    if user_ids.is_empty() {
        return Ok(());
    }

    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    for user_id in user_ids {
        let upload = uploads
            .get(&user_id.to_string())
            .copied()
            .unwrap_or_default();
        let download = downloads
            .get(&user_id.to_string())
            .copied()
            .unwrap_or_default();
        sqlx::query("UPDATE v2_user SET u = u + ?, d = d + ?, t = ?, updated_at = ? WHERE id = ?")
            .bind(upload)
            .bind(download)
            .bind(now)
            .bind(now)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn check_order(state: &WorkerState) -> anyhow::Result<()> {
    let trade_nos = sqlx::query_scalar::<_, String>(
        "SELECT trade_no FROM v2_order WHERE status IN (0, 1) ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await?;
    for trade_no in trade_nos {
        if let Err(error) = handle_order(state, &trade_no).await {
            tracing::error!(trade_no, ?error, "order handle failed");
        }
    }
    Ok(())
}

async fn handle_order(state: &WorkerState, trade_no: &str) -> anyhow::Result<()> {
    OrderService::new(state.db.clone(), state.config.clone())
        .handle_pending_order(trade_no)
        .await
        .map_err(anyhow::Error::new)
}

async fn check_commission(state: &WorkerState) -> anyhow::Result<()> {
    if state.config.commission_auto_check_enable {
        sqlx::query(
            r#"
            UPDATE v2_order
            SET commission_status = 1, updated_at = ?
            WHERE commission_status = 0
              AND invite_user_id IS NOT NULL
              AND status IN (3, 4)
              AND updated_at <= ?
            "#,
        )
        .bind(Utc::now().timestamp())
        .bind(Utc::now().timestamp() - 3 * 86_400)
        .execute(&state.db)
        .await?;
    }

    let orders = sqlx::query_as::<_, CommissionOrderRow>(
        r#"
        SELECT id, invite_user_id, user_id, trade_no, total_amount, commission_balance,
               actual_commission_balance
        FROM v2_order
        WHERE commission_status = 1
          AND invite_user_id IS NOT NULL
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    for order in orders {
        if let Err(error) = pay_commission_order(state, &order).await {
            tracing::error!(
                trade_no = order.trade_no,
                ?error,
                "commission payment failed"
            );
        }
    }
    Ok(())
}

async fn pay_commission_order(
    state: &WorkerState,
    order: &CommissionOrderRow,
) -> anyhow::Result<()> {
    let shares = commission_shares(&state.config);
    let mut tx = state.db.begin().await?;

    // Prefetch the invite chain (bounded by the number of share levels) so the payout
    // walk itself is decided by the pure `plan_commission_payouts` port of payHandle.
    // The linear chain is a superset of the inviters the walk can reach, because the
    // pointer only advances after a real payout.
    let mut chain: HashMap<i64, InviterRow> = HashMap::new();
    let mut cursor = Some(order.invite_user_id);
    for _ in 0..shares.len() {
        let Some(id) = cursor else {
            break;
        };
        if chain.contains_key(&id) {
            break;
        }
        let Some(row) = sqlx::query_as::<_, InviterRow>(
            "SELECT id, invite_user_id FROM v2_user WHERE id = ? LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            break;
        };
        cursor = row.invite_user_id;
        chain.insert(id, row);
    }

    let payouts = plan_commission_payouts(
        &shares,
        order.commission_balance,
        order.invite_user_id,
        |id| chain.get(&id).cloned(),
    );

    let mut actual_commission_balance = order.actual_commission_balance.unwrap_or_default();
    for payout in &payouts {
        let now = Utc::now().timestamp();
        if state.config.withdraw_close_enable {
            sqlx::query("UPDATE v2_user SET balance = balance + ?, updated_at = ? WHERE id = ?")
                .bind(payout.amount)
                .bind(now)
                .bind(payout.inviter_id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(
                "UPDATE v2_user SET commission_balance = commission_balance + ?, updated_at = ? WHERE id = ?",
            )
            .bind(payout.amount)
            .bind(now)
            .bind(payout.inviter_id)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"
            INSERT INTO v2_commission_log
                (invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(payout.inviter_id)
        .bind(order.user_id)
        .bind(&order.trade_no)
        .bind(order.total_amount)
        .bind(payout.amount)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        actual_commission_balance += payout.amount;
    }
    sqlx::query(
        "UPDATE v2_order SET commission_status = 2, actual_commission_balance = ?, updated_at = ? WHERE id = ?",
    )
    .bind(actual_commission_balance)
    .bind(Utc::now().timestamp())
    .bind(order.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommissionPayout {
    inviter_id: i64,
    amount: i32,
}

/// Pure port of `CheckCommission::payHandle` (CheckCommission.php:95-123): walk the
/// invite chain and, for each configured share level, pay the CURRENT inviter. A zero
/// share (or a zero commission product) `continue`s WITHOUT advancing the chain pointer
/// (CheckCommission.php:98-100), so the SAME inviter is re-evaluated at the next level;
/// the pointer only advances after a real payout (CheckCommission.php:120).
fn plan_commission_payouts<F>(
    shares: &[i32],
    commission_balance: i32,
    first_inviter: i64,
    mut lookup: F,
) -> Vec<CommissionPayout>
where
    F: FnMut(i64) -> Option<InviterRow>,
{
    let mut invite_user_id = Some(first_inviter);
    let mut payouts = Vec::new();
    for &share in shares {
        let Some(current) = invite_user_id else {
            break;
        };
        // Laravel `if (!$inviter) continue;` (CheckCommission.php:97) leaves the pointer
        // unchanged; a missing user never becomes found on a later level, so no further
        // payout is possible and we can stop.
        let Some(inviter) = lookup(current) else {
            break;
        };
        if share <= 0 {
            continue;
        }
        // Laravel computes `commission_balance * (share / 100)` as a float and stores it
        // into an INT column, so MySQL rounds (half away from zero). It skips a level only
        // when that product is exactly zero (`if (!$commissionBalance) continue;`,
        // CheckCommission.php:100) — NOT when the rounded amount is zero — and still
        // advances the pointer for a sub-cent payout. Mirror both: round the payout and
        // gate on the raw product rather than the truncated amount.
        let raw = f64::from(commission_balance) * f64::from(share) / 100.0;
        if raw == 0.0 {
            continue;
        }
        let amount = raw.round() as i32;
        payouts.push(CommissionPayout {
            inviter_id: inviter.id,
            amount,
        });
        invite_user_id = inviter.invite_user_id;
    }
    payouts
}

fn commission_shares(config: &AppConfig) -> Vec<i32> {
    if !config.commission_distribution_enable {
        return vec![100];
    }
    // CheckCommission.php:85-93 builds a fixed 3-level array with `(int)` casts, so an
    // unset/NULL/non-numeric level becomes 0 while still occupying its slot. Dropping it
    // (as a filter would) would shift later shares onto the wrong inviter.
    vec![
        parse_share(config.commission_distribution_l1.as_deref()),
        parse_share(config.commission_distribution_l2.as_deref()),
        parse_share(config.commission_distribution_l3.as_deref()),
    ]
}

fn parse_share(value: Option<&str>) -> i32 {
    value
        .map(str::trim)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0)
}

async fn check_ticket(state: &WorkerState) -> anyhow::Result<()> {
    let ticket_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT t.id
        FROM v2_ticket t
        WHERE t.status = 0
          AND t.updated_at <= ?
          AND t.reply_status = 1
          AND COALESCE((
              SELECT tm.user_id
              FROM v2_ticket_message tm
              WHERE tm.ticket_id = t.id
              ORDER BY tm.id DESC
              LIMIT 1
          ), 0) <> t.user_id
        "#,
    )
    .bind(Utc::now().timestamp() - 86_400)
    .fetch_all(&state.db)
    .await?;
    if ticket_ids.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_ticket SET status = 1, updated_at = ");
    builder.push_bind(Utc::now().timestamp());
    builder.push(" WHERE id IN (");
    {
        let mut separated = builder.separated(", ");
        for id in ticket_ids {
            separated.push_bind(id);
        }
    }
    builder.push(")");
    builder.build().execute(&state.db).await?;
    Ok(())
}

async fn check_renewal(state: &WorkerState) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let users = sqlx::query_as::<_, RenewalUserRow>(
        r#"
        SELECT id, balance, plan_id, expired_at
        FROM v2_user
        WHERE auto_renewal <> 0
          AND plan_id IS NOT NULL
          AND expired_at IS NOT NULL
          AND expired_at > ?
          AND expired_at - ? < ?
        "#,
    )
    .bind(now)
    .bind(now)
    .bind(2 * 86_400)
    .fetch_all(&state.db)
    .await?;

    for user in users {
        if let Err(error) = renew_user(state, user).await {
            tracing::warn!(?error, "auto renewal failed");
        }
    }
    Ok(())
}

async fn renew_user(state: &WorkerState, user: RenewalUserRow) -> anyhow::Result<()> {
    let latest_period = sqlx::query_scalar::<_, String>(
        r#"
        SELECT period
        FROM v2_order
        WHERE user_id = ?
          AND period NOT IN ('reset_price', 'onetime_price', 'deposit')
          AND status = 3
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;
    let Some(latest_period) = latest_period else {
        disable_auto_renewal(&state.db, user.id).await?;
        return Ok(());
    };
    let Some(plan) = sqlx::query_as::<_, RenewalPlanRow>(
        r#"
        SELECT id, renew, month_price, quarter_price, half_year_price, year_price,
               two_year_price, three_year_price
        FROM v2_plan
        WHERE id = ?
        LIMIT 1
        "#,
    )
    .bind(user.plan_id)
    .fetch_optional(&state.db)
    .await?
    else {
        disable_auto_renewal(&state.db, user.id).await?;
        return Ok(());
    };
    let Some(price) = renewal_price(&plan, &latest_period) else {
        disable_auto_renewal(&state.db, user.id).await?;
        return Ok(());
    };
    if plan.renew == 0 || user.balance < price {
        disable_auto_renewal(&state.db, user.id).await?;
        return Ok(());
    }
    let Some(expired_at) = add_period(user.expired_at, &latest_period) else {
        disable_auto_renewal(&state.db, user.id).await?;
        return Ok(());
    };

    let trade_no = generate_trade_no();
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE v2_user SET balance = balance - ?, expired_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(price)
    .bind(expired_at)
    .bind(now)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO v2_order
            (user_id, plan_id, `type`, period, trade_no, total_amount, balance_amount, status, created_at, updated_at)
        VALUES (?, ?, 2, ?, ?, 0, ?, 3, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(plan.id)
    .bind(latest_period)
    .bind(trade_no)
    .bind(price)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The plan's price (in cents) for a recurring renewal period. Returns `Some(0)`
/// when the column is NULL or zero — Laravel's CheckRenewal compares
/// `balance < $plan[$period]` where a NULL price coerces to 0, so a free/unpriced
/// period auto-renews at no cost rather than disabling auto-renewal. Only a period
/// that is not a recognized recurring key yields `None` (cannot be renewed).
fn renewal_price(plan: &RenewalPlanRow, period: &str) -> Option<i32> {
    match period {
        "month_price" => Some(plan.month_price.unwrap_or(0)),
        "quarter_price" => Some(plan.quarter_price.unwrap_or(0)),
        "half_year_price" => Some(plan.half_year_price.unwrap_or(0)),
        "year_price" => Some(plan.year_price.unwrap_or(0)),
        "two_year_price" => Some(plan.two_year_price.unwrap_or(0)),
        "three_year_price" => Some(plan.three_year_price.unwrap_or(0)),
        _ => None,
    }
}

async fn disable_auto_renewal(db: &MySqlPool, user_id: i64) -> anyhow::Result<()> {
    sqlx::query("UPDATE v2_user SET auto_renewal = 0, updated_at = ? WHERE id = ?")
        .bind(Utc::now().timestamp())
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn reset_traffic(state: &WorkerState) -> anyhow::Result<()> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: () = conn.set_ex("traffic_reset_lock", 1, 300).await?;
    let now = Utc::now().timestamp();
    // INNER JOIN, not LEFT JOIN: ResetTraffic.php groups existing plans by
    // reset_traffic_method and only resets users whose plan_id is in one of those
    // GROUP_CONCAT lists (`whereIn('plan_id', $planIds)`). A user with a NULL or
    // orphaned plan_id is never in any list, so it is never reset. The join keeps
    // only users backed by a real plan; a matched row with a NULL method genuinely
    // means "plan exists but method is NULL" and falls through to the config default.
    let users = sqlx::query_as::<_, ResetUserRow>(
        r#"
        SELECT u.id, u.expired_at, p.reset_traffic_method
        FROM v2_user u
        INNER JOIN v2_plan p ON p.id = u.plan_id
        WHERE u.expired_at IS NOT NULL
          AND u.expired_at > ?
        "#,
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    let ids = users
        .into_iter()
        .filter(|user| should_reset_user(user, state.config.reset_traffic_method))
        .map(|user| user.id)
        .collect::<Vec<_>>();
    if !ids.is_empty() {
        let mut builder =
            QueryBuilder::<MySql>::new("UPDATE v2_user SET u = 0, d = 0 WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for id in ids {
                separated.push_bind(id);
            }
        }
        builder.push(")");
        builder.build().execute(&state.db).await?;
    }
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let _: () = conn.del("traffic_reset_lock").await?;
    Ok(())
}

fn should_reset_user(user: &ResetUserRow, default_method: i32) -> bool {
    let now = app_now();
    let Some(expired) = app_timezone().timestamp_opt(user.expired_at, 0).single() else {
        return false;
    };
    match user.reset_traffic_method {
        // A plan with an explicit reset_traffic_method uses exactly that branch
        // (ResetTraffic.php:84-106, each `case` has a `break`).
        Some(method) => reset_matches(i32::from(method), &now, &expired, user.expired_at),
        // A plan whose reset_traffic_method is NULL uses the config default, but the
        // NULL branch's inner switch omits the `break` after `case 3`
        // (ResetTraffic.php:76-80), so a default of 3 ALSO runs resetByExpireYear
        // (case 4). Mirror that fall-through: reset timing is a billing contract.
        None => {
            reset_matches(default_method, &now, &expired, user.expired_at)
                || (default_method == 3 && reset_matches(4, &now, &expired, user.expired_at))
        }
    }
}

fn reset_matches(
    method: i32,
    now: &DateTime<FixedOffset>,
    expired: &DateTime<FixedOffset>,
    expired_at: i64,
) -> bool {
    match method {
        // resetByMonthFirstDay (ResetTraffic.php:142-152)
        0 => now.day() == 1,
        // resetByExpireDay (ResetTraffic.php:154-175)
        1 => {
            let last_day = last_day_of_current_month();
            let today = now.day();
            let expire_day = expired.day();
            (expire_day == today || (today == last_day && expire_day >= last_day))
                && Utc::now().timestamp() < expired_at - 2_160_000
        }
        // no action (ResetTraffic.php:73-74/94-96)
        2 => false,
        // resetByYearFirstDay (ResetTraffic.php:130-140)
        3 => now.month() == 1 && now.day() == 1,
        // resetByExpireYear (ResetTraffic.php:112-128)
        4 => now.month() == expired.month() && now.day() == expired.day(),
        _ => false,
    }
}

async fn reset_log(state: &WorkerState) -> anyhow::Result<()> {
    let stat_before =
        month_delta_timestamp(2).unwrap_or_else(|| Utc::now().timestamp() - 60 * 86_400);
    let log_before =
        month_delta_timestamp(1).unwrap_or_else(|| Utc::now().timestamp() - 30 * 86_400);
    sqlx::query("DELETE FROM v2_stat_user WHERE record_at < ?")
        .bind(stat_before)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM v2_stat_server WHERE record_at < ?")
        .bind(stat_before)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM v2_log WHERE created_at < ?")
        .bind(log_before)
        .execute(&state.db)
        .await?;
    Ok(())
}

async fn send_remind_mail(state: &WorkerState) -> anyhow::Result<()> {
    let users = sqlx::query_as::<_, ReminderUserRow>(
        r#"
        SELECT id, email, remind_expire, remind_traffic, expired_at, u, d, transfer_enable
        FROM v2_user
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    for user in users {
        if user.remind_expire.unwrap_or_default() != 0
            && user.expired_at.is_some_and(|expired_at| {
                expired_at - 86_400 < Utc::now().timestamp() && expired_at > Utc::now().timestamp()
            })
        {
            let subject = format!(
                "The service in {} is about to expire",
                state.config.app_name
            );
            send_email(state, &user.email, &subject, "remindExpire").await?;
        }
        if user.remind_traffic.unwrap_or_default() != 0
            && user
                .expired_at
                .is_none_or(|expired_at| expired_at >= Utc::now().timestamp())
            && traffic_warns(user.u, user.d, user.transfer_enable)
        {
            let key = format!("LAST_SEND_EMAIL_REMIND_TRAFFIC_{}", user.id);
            let mut conn = state.redis.get_multiplexed_async_connection().await?;
            if !conn.exists::<_, bool>(&key).await? {
                let _: () = conn.set_ex(&key, 1, 86_400).await?;
                let subject = format!(
                    "The traffic usage in {} has reached 95%",
                    state.config.app_name
                );
                send_email(state, &user.email, &subject, "remindTraffic").await?;
            }
        }
    }
    Ok(())
}

fn traffic_warns(u: i64, d: i64, transfer_enable: i64) -> bool {
    let used = u + d;
    used > 0 && transfer_enable > 0 && used * 100 >= transfer_enable * 95 && used < transfer_enable
}

async fn send_email(
    state: &WorkerState,
    email: &str,
    subject: &str,
    template_name: &str,
) -> anyhow::Result<()> {
    // Keep v2_mail_log.template_name identical to Laravel (SendEmailJob.php:51),
    // e.g. `mail.default.remindTraffic`.
    let template = format!(
        "mail.{}.{}",
        state.config.email_template.as_deref().unwrap_or("default"),
        template_name
    );
    let body = v2board_domain::mail::render_reminder(
        template_name,
        &state.config.app_name,
        state.config.app_url.as_deref().unwrap_or_default(),
        subject,
    );

    // Laravel SendEmailJob has tries=3 (SendEmailJob.php:19) and paces sends with a 2s
    // sleep (SendEmailJob.php:53). Ported synchronously: retry up to 3 attempts with a
    // short backoff between attempts for transient SMTP failures, then record the final
    // outcome once into v2_mail_log. The happy path is NOT throttled (the 2s sleep only
    // existed to pace the async queue), and unlike the Laravel job — which swallows the
    // exception before its queue can ever retry — this actually retries.
    //
    // The backoff is a std sleep (this cold failure path runs on the multi-thread
    // runtime) rather than `tokio::time::sleep`, so it does not rely on tokio's optional
    // `time` feature, which the workers crate does not declare.
    let max_attempts = 3;
    let mut attempt = 0;
    let error = loop {
        attempt += 1;
        match send_email_inner(&state.config, email, subject, &body).await {
            Ok(()) => break None,
            Err(error) if attempt < max_attempts => {
                tracing::warn!(email, attempt, ?error, "mail send failed, retrying");
                std::thread::sleep(Duration::from_secs(2));
            }
            Err(error) => break Some(error.to_string()),
        }
    };

    sqlx::query(
        r#"
        INSERT INTO v2_mail_log
            (email, subject, template_name, error, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(email)
    .bind(subject)
    .bind(template)
    .bind(error)
    .bind(Utc::now().timestamp())
    .bind(Utc::now().timestamp())
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn send_email_inner(
    config: &AppConfig,
    email: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    let host = config
        .email_host
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Email host is not configured"))?;
    let from = config
        .email_from_address
        .as_deref()
        .or(config.email_username.as_deref())
        .ok_or_else(|| anyhow::anyhow!("Email sender is not configured"))?;
    // Laravel renders Blade HTML mail (SendEmailJob.php:54, MailService.php), so the
    // delivered body is text/html rather than plaintext.
    let builder = Message::builder()
        .from(from.parse()?)
        .to(email.parse()?)
        .subject(subject)
        .header(ContentType::TEXT_HTML);
    let message = builder.body(body.to_string())?;
    let mut transport = match config
        .email_encryption
        .as_deref()
        .map(str::to_ascii_lowercase)
    {
        Some(encryption) if encryption == "ssl" => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
        }
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?,
    };
    if let Some(port) = config.email_port.and_then(|port| u16::try_from(port).ok()) {
        transport = transport.port(port);
    }
    if let (Some(username), Some(password)) = (&config.email_username, &config.email_password) {
        transport = transport.credentials(Credentials::new(username.clone(), password.clone()));
    }
    transport.build().send(message).await?;
    Ok(())
}

async fn statistics(state: &WorkerState) -> anyhow::Result<()> {
    let end_at = today_start_timestamp();
    let start_at = end_at - 86_400;
    let order_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_order WHERE created_at >= ? AND created_at < ?",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let order_total: i64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(total_amount), 0) AS SIGNED) FROM v2_order WHERE created_at >= ? AND created_at < ?",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let paid_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_order WHERE paid_at >= ? AND paid_at < ? AND status NOT IN (0, 2)",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let paid_total: i64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(total_amount), 0) AS SIGNED) FROM v2_order WHERE paid_at >= ? AND paid_at < ? AND status NOT IN (0, 2)",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let commission_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_commission_log WHERE created_at >= ? AND created_at < ?",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let commission_total: i64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(get_amount), 0) AS SIGNED) FROM v2_commission_log WHERE created_at >= ? AND created_at < ?",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let register_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE created_at >= ? AND created_at < ?")
            .bind(start_at)
            .bind(end_at)
            .fetch_one(&state.db)
            .await?;
    let invite_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_user WHERE created_at >= ? AND created_at < ? AND invite_user_id IS NOT NULL",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    let transfer_used_total: i64 = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(u) + SUM(d), 0) AS SIGNED) FROM v2_stat_server WHERE created_at >= ? AND created_at < ?",
    )
    .bind(start_at)
    .bind(end_at)
    .fetch_one(&state.db)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO v2_stat
            (record_at, record_type, order_count, order_total, commission_count,
             commission_total, paid_count, paid_total, register_count, invite_count,
             transfer_used_total, created_at, updated_at)
        VALUES (?, 'd', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            order_count = VALUES(order_count),
            order_total = VALUES(order_total),
            commission_count = VALUES(commission_count),
            commission_total = VALUES(commission_total),
            paid_count = VALUES(paid_count),
            paid_total = VALUES(paid_total),
            register_count = VALUES(register_count),
            invite_count = VALUES(invite_count),
            transfer_used_total = VALUES(transfer_used_total),
            updated_at = VALUES(updated_at)
        "#,
    )
    .bind(start_at)
    .bind(order_count)
    .bind(order_total)
    .bind(commission_count)
    .bind(commission_total)
    .bind(paid_count)
    .bind(paid_total)
    .bind(register_count)
    .bind(invite_count)
    .bind(transfer_used_total.to_string())
    .bind(Utc::now().timestamp())
    .bind(Utc::now().timestamp())
    .execute(&state.db)
    .await?;
    Ok(())
}

fn add_period(timestamp: i64, period: &str) -> Option<i64> {
    let months = match period {
        "month_price" => 1,
        "quarter_price" => 3,
        "half_year_price" => 6,
        "year_price" => 12,
        "two_year_price" => 24,
        "three_year_price" => 36,
        _ => return None,
    };
    let base = if timestamp < Utc::now().timestamp() {
        Utc::now().timestamp()
    } else {
        timestamp
    };
    app_timezone()
        .timestamp_opt(base, 0)
        .single()?
        .checked_add_months(Months::new(months))
        .map(|date| date.timestamp())
}

fn month_delta_timestamp(months: u32) -> Option<i64> {
    app_now()
        .checked_sub_months(Months::new(months))
        .map(|date| date.timestamp())
}

fn today_start_timestamp() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

fn last_day_of_current_month() -> u32 {
    let today = app_now().date_naive();
    let (year, month) = if today.month() == 12 {
        (today.year() + 1, 1)
    } else {
        (today.year(), today.month() + 1)
    };
    let first_next_month = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
    (first_next_month - chrono::Duration::days(1)).day()
}

fn generate_trade_no() -> String {
    format!(
        "{}{}",
        Utc::now().format("%Y%m%d%H%M%S"),
        Uuid::new_v4().simple()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renewal_price_treats_null_or_zero_as_free_not_disable() {
        let plan = RenewalPlanRow {
            id: 1,
            renew: 1,
            month_price: None,      // unpriced period
            quarter_price: Some(0), // explicitly free
            half_year_price: Some(1000),
            year_price: None,
            two_year_price: None,
            three_year_price: None,
        };
        // Laravel free-renews (balance < NULL/0 is false) instead of disabling.
        assert_eq!(renewal_price(&plan, "month_price"), Some(0));
        assert_eq!(renewal_price(&plan, "quarter_price"), Some(0));
        assert_eq!(renewal_price(&plan, "half_year_price"), Some(1000));
        // A non-recurring period cannot be auto-renewed.
        assert_eq!(renewal_price(&plan, "reset_price"), None);
        assert_eq!(renewal_price(&plan, "onetime_price"), None);
    }

    #[test]
    fn scheduled_task_matrix_matches_laravel_scheduler_jobs() {
        assert_eq!(
            SCHEDULED_TASK_NAMES,
            &[
                "traffic_update",
                "statistics",
                "check_order",
                "check_commission",
                "check_ticket",
                "check_renewal",
                "reset_traffic",
                "reset_log",
                "send_remind_mail",
            ]
        );
    }

    #[test]
    fn scheduled_task_lookup_covers_the_scheduler_matrix() {
        for name in SCHEDULED_TASK_NAMES {
            assert!(
                scheduled_task_by_name(name).is_some(),
                "{name} is scheduled but cannot be run once"
            );
        }
        assert!(scheduled_task_by_name("missing").is_none());
    }

    fn inviter(id: i64, invited_by: Option<i64>) -> InviterRow {
        InviterRow {
            id,
            invite_user_id: invited_by,
        }
    }

    fn three_level_chain() -> HashMap<i64, InviterRow> {
        // 1 (invited by 2) -> 2 (invited by 3) -> 3 (top of the chain).
        [
            (1, inviter(1, Some(2))),
            (2, inviter(2, Some(3))),
            (3, inviter(3, None)),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn commission_zero_share_does_not_advance_invite_chain() {
        let chain = three_level_chain();
        // shares [0, 50, 0]: level 0 pays nobody but must NOT advance the pointer, so the
        // direct inviter (id 1) is the one paid at level 1's 50% share. Level 2's 0 share
        // again does not advance. Mirrors CheckCommission::payHandle.
        let payouts = plan_commission_payouts(&[0, 50, 0], 100, 1, |id| chain.get(&id).cloned());
        assert_eq!(
            payouts,
            vec![CommissionPayout {
                inviter_id: 1,
                amount: 50,
            }]
        );
    }

    #[test]
    fn commission_positive_shares_walk_up_the_chain() {
        let chain = three_level_chain();
        let payouts = plan_commission_payouts(&[50, 30, 20], 100, 1, |id| chain.get(&id).cloned());
        assert_eq!(
            payouts,
            vec![
                CommissionPayout {
                    inviter_id: 1,
                    amount: 50,
                },
                CommissionPayout {
                    inviter_id: 2,
                    amount: 30,
                },
                CommissionPayout {
                    inviter_id: 3,
                    amount: 20,
                },
            ]
        );
    }

    #[test]
    fn commission_single_full_share_pays_direct_inviter() {
        // The distribution-disabled path produces shares = [100].
        let chain = three_level_chain();
        let payouts = plan_commission_payouts(&[100], 250, 1, |id| chain.get(&id).cloned());
        assert_eq!(
            payouts,
            vec![CommissionPayout {
                inviter_id: 1,
                amount: 250,
            }]
        );
    }

    #[test]
    fn parse_share_coerces_missing_and_non_numeric_to_zero() {
        // Matches PHP `(int)config(...)`: NULL/absent and unparseable become 0, and the
        // slot is preserved so later levels are not shifted.
        assert_eq!(parse_share(None), 0);
        assert_eq!(parse_share(Some("")), 0);
        assert_eq!(parse_share(Some("abc")), 0);
        assert_eq!(parse_share(Some(" 40 ")), 40);
    }

    #[test]
    fn null_reset_method_default_three_falls_through_to_expire_year() {
        // A plan with reset_traffic_method = NULL whose expiry anniversary (m-d) is today.
        let now_ts = app_now().timestamp();
        let user = ResetUserRow {
            id: 1,
            expired_at: now_ts,
            reset_traffic_method: None,
        };
        // Config default 3: Laravel's NULL branch omits the `break` after case 3, so it
        // also runs resetByExpireYear (case 4) -> an anniversary-today user resets even
        // when it is not Jan 1. This holds every day the test runs.
        assert!(should_reset_user(&user, 3));
        // Config default 2 ("no action") does not fall through, so the same user is left
        // alone.
        assert!(!should_reset_user(&user, 2));
    }

    #[test]
    fn explicit_reset_method_ignores_config_default_fall_through() {
        let now_ts = app_now().timestamp();
        // Explicit method 2 ("no action") must never reset, regardless of config default.
        let user = ResetUserRow {
            id: 1,
            expired_at: now_ts,
            reset_traffic_method: Some(2),
        };
        assert!(!should_reset_user(&user, 3));
    }
}
