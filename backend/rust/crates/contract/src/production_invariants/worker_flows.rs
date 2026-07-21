use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use redis::AsyncCommands;
use sqlx::PgPool;
use tokio::task::JoinSet;
use v2board_application::ticket::{
    NewTicket, TicketCreateOutcome, TicketRepository, UserTicketReply, UserTicketReplyOutcome,
};
use v2board_config::RedisKeyspace;
use v2board_db::{installation_id, ticket::PostgresTicketRepository};
use v2board_domain_model::TicketLevel;

use super::harness::{env_or, insert_user, operator_authority_config, random_traffic_key};

const DEFAULT_WORKER_BIN: &str = "/app/target/debug/v2board-workers";

pub(super) async fn traffic_epoch_invariant(
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

pub(super) async fn ticket_state_machine(
    pool: &PgPool,
    database_url: &str,
    database_name: &str,
    redis_url: &str,
) -> Result<()> {
    let user_id = insert_user(pool, "ticket", "not-used").await?;
    let now = Utc::now().timestamp();
    let mut creates = JoinSet::new();
    for sequence in 0..8 {
        let pool = pool.clone();
        creates.spawn(async move {
            PostgresTicketRepository::new(pool)
                .create(NewTicket {
                    user_id,
                    subject: format!("concurrent ticket {sequence}"),
                    level: TicketLevel::Medium,
                    message: "initial message".to_string(),
                    created_at: now,
                    require_paid_order: false,
                })
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
        PostgresTicketRepository::new(pool.clone())
            .close_as_user(user_id, first_ticket, now + 1)
            .await?,
        "failed to close the ticket used by the uniqueness check"
    );

    ensure!(
        PostgresTicketRepository::new(pool.clone())
            .create(NewTicket {
                user_id,
                subject: "reply race".to_string(),
                level: TicketLevel::Medium,
                message: "user opening message".to_string(),
                created_at: now - 90_000,
                require_paid_order: false,
            })
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
    let reply = PostgresTicketRepository::new(pool.clone())
        .reply_as_user(UserTicketReply {
            ticket_id: race_ticket,
            user_id,
            message: "reply concurrent with auto-close".to_string(),
            replied_at: Utc::now().timestamp(),
        })
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
    PostgresTicketRepository::new(pool.clone())
        .close_as_user(user_id, race_ticket, now + 2)
        .await?;

    ensure!(
        PostgresTicketRepository::new(pool.clone())
            .create(NewTicket {
                user_id,
                subject: "stale answered ticket".to_string(),
                level: TicketLevel::Medium,
                message: "old user opening message".to_string(),
                created_at: now - 90_000,
                require_paid_order: false,
            })
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

pub(super) async fn worker_health_process(
    pool: &PgPool,
    database_url: &str,
    database_name: &str,
    redis_url: &str,
) -> Result<()> {
    let worker_bin = env_or("RUST_INTEGRATION_WORKER_BIN", DEFAULT_WORKER_BIN);
    let app_key = operator_authority_config()?.app_key;
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
        .env("APP_KEY", app_key)
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
    let app_key = operator_authority_config()?.app_key;
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
            .env("APP_KEY", app_key)
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
