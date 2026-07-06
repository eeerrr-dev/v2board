use std::{collections::HashMap, env, str::FromStr};

use apalis::prelude::*;
use apalis_cron::{CronStream, Tick};
use chrono::{Datelike, Local, Months, TimeZone, Utc};
use cron::Schedule;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::authentication::Credentials,
};
use redis::AsyncCommands;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, MySql, MySqlPool, QueryBuilder};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use v2board_config::AppConfig;
use v2board_domain::order::OrderService;

#[derive(Clone)]
struct WorkerState {
    config: AppConfig,
    db: MySqlPool,
    redis: redis::Client,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "job", rename_all = "snake_case")]
enum QueueJob {
    TrafficUpdate {
        user_id: Option<i64>,
    },
    CheckOrder {
        trade_no: Option<String>,
    },
    OrderHandle {
        trade_no: String,
    },
    CheckCommission,
    CheckTicket,
    CheckRenewal,
    ResetTraffic,
    ResetLog,
    SendRemindMail,
    Statistics,
    SendEmail {
        email: String,
        subject: String,
        template_name: Option<String>,
        template_value: Option<Value>,
    },
    SendTelegram {
        telegram_id: i64,
        text: String,
    },
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

    let conn = apalis_redis::connect(state.config.redis_url.clone()).await?;
    let storage = apalis_redis::RedisStorage::new(conn);
    run_worker_runtime(state, storage).await
}

async fn run_worker_runtime(
    state: WorkerState,
    storage: apalis_redis::RedisStorage<QueueJob>,
) -> anyhow::Result<()> {
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

    let mut monitor = Monitor::new().register({
        let storage = storage.clone();
        let state = state.clone();
        move |_| {
            WorkerBuilder::new("v2board-redis-worker")
                .backend(storage.clone())
                .data(state.clone())
                .build(handle_queue_job)
        }
    });
    for (worker_name, schedule, job) in scheduled_jobs {
        let state = state.clone();
        monitor = monitor.register(move |_| {
            WorkerBuilder::new(worker_name)
                .backend(CronStream::new(schedule.clone()))
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

async fn handle_queue_job(job: QueueJob, state: Data<WorkerState>) -> Result<(), BoxDynError> {
    let name = queue_job_name(&job);
    if let Err(error) = run_queue_job(job, &state).await {
        tracing::error!(job = name, ?error, "queued job failed");
        let _ = record_worker_metric(&state, name, false).await;
    } else if let Err(error) = record_worker_metric(&state, name, true).await {
        tracing::warn!(job = name, ?error, "failed to record queued job metric");
    }
    Ok(())
}

async fn handle_cron_tick(
    tick: Tick,
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

    if let Err(error) = run_scheduled_job(job.task, &state).await {
        tracing::error!(job = job.name, ?error, "scheduled job failed");
        let _ = record_worker_metric(&state, job.name, false).await;
    } else if let Err(error) = record_worker_metric(&state, job.name, true).await {
        tracing::warn!(
            job = job.name,
            ?error,
            "failed to record scheduled job metric"
        );
    }
    if let Err(error) = release_scheduler_lock(&state, scheduler_lock).await {
        tracing::warn!(
            job = job.name,
            ?error,
            "failed to release scheduled job lock"
        );
    }
    Ok(())
}

fn queue_job_name(job: &QueueJob) -> &'static str {
    match job {
        QueueJob::TrafficUpdate { .. } => "traffic_update",
        QueueJob::CheckOrder { .. } => "check_order",
        QueueJob::OrderHandle { .. } => "order_handle",
        QueueJob::CheckCommission => "check_commission",
        QueueJob::CheckTicket => "check_ticket",
        QueueJob::CheckRenewal => "check_renewal",
        QueueJob::ResetTraffic => "reset_traffic",
        QueueJob::ResetLog => "reset_log",
        QueueJob::SendRemindMail => "send_remind_mail",
        QueueJob::Statistics => "statistics",
        QueueJob::SendEmail { .. } => "send_email",
        QueueJob::SendTelegram { .. } => "send_telegram",
    }
}

async fn run_queue_job(job: QueueJob, state: &WorkerState) -> anyhow::Result<()> {
    match job {
        QueueJob::TrafficUpdate { user_id } => traffic_update(state, user_id).await,
        QueueJob::CheckOrder { trade_no } => check_order(state, trade_no).await,
        QueueJob::OrderHandle { trade_no } => handle_order(state, &trade_no).await,
        QueueJob::CheckCommission => check_commission(state).await,
        QueueJob::CheckTicket => check_ticket(state).await,
        QueueJob::CheckRenewal => check_renewal(state).await,
        QueueJob::ResetTraffic => reset_traffic(state).await,
        QueueJob::ResetLog => reset_log(state).await,
        QueueJob::SendRemindMail => send_remind_mail(state).await,
        QueueJob::Statistics => statistics(state).await,
        QueueJob::SendEmail {
            email,
            subject,
            template_name,
            template_value,
        } => {
            send_email(
                state,
                &email,
                &subject,
                template_name
                    .as_deref()
                    .unwrap_or("mail.default.notification"),
                &mail_body(&state.config, &subject, template_value.as_ref()),
            )
            .await
        }
        QueueJob::SendTelegram { telegram_id, text } => {
            send_telegram(state, telegram_id, &text).await
        }
    }
}

async fn run_scheduled_job(task: ScheduledTask, state: &WorkerState) -> anyhow::Result<()> {
    match task {
        ScheduledTask::TrafficUpdate => traffic_update(state, None).await,
        ScheduledTask::Statistics => statistics(state).await,
        ScheduledTask::CheckOrder => check_order(state, None).await,
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

async fn traffic_update(state: &WorkerState, only_user_id: Option<i64>) -> anyhow::Result<()> {
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
    if let Some(only_user_id) = only_user_id {
        user_ids.retain(|id| *id == only_user_id);
    }
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

async fn check_order(state: &WorkerState, trade_no: Option<String>) -> anyhow::Result<()> {
    if let Some(trade_no) = trade_no {
        return handle_order(state, &trade_no).await;
    }
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
    let mut invite_user_id = Some(order.invite_user_id);
    let mut actual_commission_balance = order.actual_commission_balance.unwrap_or_default();
    for share in shares {
        let Some(current_inviter_id) = invite_user_id else {
            break;
        };
        let Some(inviter) = sqlx::query_as::<_, InviterRow>(
            "SELECT id, invite_user_id FROM v2_user WHERE id = ? LIMIT 1",
        )
        .bind(current_inviter_id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            invite_user_id = None;
            continue;
        };
        if share <= 0 {
            invite_user_id = inviter.invite_user_id;
            continue;
        }
        let commission_balance = order.commission_balance * share / 100;
        if commission_balance <= 0 {
            invite_user_id = inviter.invite_user_id;
            continue;
        }
        if state.config.withdraw_close_enable {
            sqlx::query("UPDATE v2_user SET balance = balance + ?, updated_at = ? WHERE id = ?")
                .bind(commission_balance)
                .bind(Utc::now().timestamp())
                .bind(inviter.id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query(
                "UPDATE v2_user SET commission_balance = commission_balance + ?, updated_at = ? WHERE id = ?",
            )
            .bind(commission_balance)
            .bind(Utc::now().timestamp())
            .bind(inviter.id)
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
        .bind(inviter.id)
        .bind(order.user_id)
        .bind(&order.trade_no)
        .bind(order.total_amount)
        .bind(commission_balance)
        .bind(Utc::now().timestamp())
        .bind(Utc::now().timestamp())
        .execute(&mut *tx)
        .await?;
        actual_commission_balance += commission_balance;
        invite_user_id = inviter.invite_user_id;
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

fn commission_shares(config: &AppConfig) -> Vec<i32> {
    if !config.commission_distribution_enable {
        return vec![100];
    }
    [
        config.commission_distribution_l1.as_deref(),
        config.commission_distribution_l2.as_deref(),
        config.commission_distribution_l3.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|value| value.parse::<i32>().ok())
    .collect()
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

fn renewal_price(plan: &RenewalPlanRow, period: &str) -> Option<i32> {
    match period {
        "month_price" => plan.month_price,
        "quarter_price" => plan.quarter_price,
        "half_year_price" => plan.half_year_price,
        "year_price" => plan.year_price,
        "two_year_price" => plan.two_year_price,
        "three_year_price" => plan.three_year_price,
        _ => None,
    }
    .filter(|price| *price > 0)
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
    let users = sqlx::query_as::<_, ResetUserRow>(
        r#"
        SELECT u.id, u.expired_at, p.reset_traffic_method
        FROM v2_user u
        LEFT JOIN v2_plan p ON p.id = u.plan_id
        WHERE u.expired_at IS NOT NULL
          AND u.expired_at > ?
        "#,
    )
    .bind(now)
    .fetch_all(&state.db)
    .await?;
    let ids = users
        .into_iter()
        .filter(|user| should_reset_user(user, &state.config))
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

fn should_reset_user(user: &ResetUserRow, config: &AppConfig) -> bool {
    let method = user
        .reset_traffic_method
        .map(i32::from)
        .unwrap_or(config.reset_traffic_method);
    let now = Local::now();
    let Some(expired) = Local.timestamp_opt(user.expired_at, 0).single() else {
        return false;
    };
    match method {
        0 => now.day() == 1,
        1 => {
            let last_day = last_day_of_current_month();
            let today = now.day();
            let expire_day = expired.day();
            (expire_day == today || (today == last_day && expire_day >= last_day))
                && Utc::now().timestamp() < user.expired_at - 2_160_000
        }
        2 => false,
        3 => now.month() == 1 && now.day() == 1,
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
            send_email(
                state,
                &user.email,
                &subject,
                "remindExpire",
                &default_mail_body(&state.config, &subject),
            )
            .await?;
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
                send_email(
                    state,
                    &user.email,
                    &subject,
                    "remindTraffic",
                    &default_mail_body(&state.config, &subject),
                )
                .await?;
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
    body: &str,
) -> anyhow::Result<()> {
    let template = format!(
        "mail.{}.{}",
        state.config.email_template.as_deref().unwrap_or("default"),
        template_name
    );
    let error = match send_email_inner(&state.config, email, subject, body).await {
        Ok(()) => None,
        Err(error) => Some(error.to_string()),
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

async fn send_telegram(state: &WorkerState, telegram_id: i64, text: &str) -> anyhow::Result<()> {
    let token = state
        .config
        .telegram_bot_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Telegram bot token is not configured"))?;
    let body = serde_urlencoded::to_string([
        ("chat_id", telegram_id.to_string()),
        ("text", escape_telegram_markdown(text)),
        ("parse_mode", "markdown".to_string()),
    ])?;
    let response = reqwest::Client::new()
        .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("Telegram request failed");
    }
    Ok(())
}

fn escape_telegram_markdown(text: &str) -> String {
    text.replace('_', "\\_")
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
    let builder = Message::builder()
        .from(from.parse()?)
        .to(email.parse()?)
        .subject(subject);
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

fn mail_body(config: &AppConfig, subject: &str, value: Option<&Value>) -> String {
    match value {
        Some(value) => format!("{subject}\n\n{value}"),
        None => default_mail_body(config, subject),
    }
}

fn default_mail_body(config: &AppConfig, subject: &str) -> String {
    let url = config.app_url.as_deref().unwrap_or_default();
    format!("{}\n\n{}\n{}", config.app_name, subject, url)
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
    Local
        .timestamp_opt(base, 0)
        .single()?
        .checked_add_months(Months::new(months))
        .map(|date| date.timestamp())
}

fn month_delta_timestamp(months: u32) -> Option<i64> {
    Local::now()
        .checked_sub_months(Months::new(months))
        .map(|date| date.timestamp())
}

fn today_start_timestamp() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

fn last_day_of_current_month() -> u32 {
    let today = Local::now().date_naive();
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

    #[test]
    fn queue_job_names_cover_all_worker_jobs() {
        let jobs = [
            QueueJob::TrafficUpdate { user_id: None },
            QueueJob::CheckOrder { trade_no: None },
            QueueJob::OrderHandle {
                trade_no: "T".to_string(),
            },
            QueueJob::CheckCommission,
            QueueJob::CheckTicket,
            QueueJob::CheckRenewal,
            QueueJob::ResetTraffic,
            QueueJob::ResetLog,
            QueueJob::SendRemindMail,
            QueueJob::Statistics,
            QueueJob::SendEmail {
                email: "user@example.test".to_string(),
                subject: "subject".to_string(),
                template_name: None,
                template_value: None,
            },
            QueueJob::SendTelegram {
                telegram_id: 1,
                text: "message".to_string(),
            },
        ];
        let names = jobs.iter().map(queue_job_name).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "traffic_update",
                "check_order",
                "order_handle",
                "check_commission",
                "check_ticket",
                "check_renewal",
                "reset_traffic",
                "reset_log",
                "send_remind_mail",
                "statistics",
                "send_email",
                "send_telegram",
            ]
        );
    }
}
