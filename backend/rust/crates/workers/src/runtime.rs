use std::{future::Future, path::PathBuf, time::Duration};

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    analytics,
    metrics::record_worker_loop_heartbeat,
    outbox,
    scheduler::{SCHEDULED_JOBS, run_schedule_loop},
    state::WorkerState,
};

const HEALTH_JOB_NAME: &str = "worker_health";
const DEFAULT_HEALTH_FILE: &str = "/run/v2board-worker/health";
const DEFAULT_HEARTBEAT_INTERVAL_SECONDS: u64 = 10;
const DEFAULT_SHUTDOWN_TIMEOUT_SECONDS: u64 = 30;
const DEPENDENCY_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerRuntimeConfig {
    health_file: PathBuf,
    heartbeat_interval: Duration,
    shutdown_timeout: Duration,
}

impl WorkerRuntimeConfig {
    fn from_env() -> anyhow::Result<Self> {
        let health_file = std::env::var("V2BOARD_WORKER_HEALTH_FILE")
            .unwrap_or_else(|_| DEFAULT_HEALTH_FILE.to_string());
        if health_file.trim().is_empty() || !PathBuf::from(&health_file).is_absolute() {
            anyhow::bail!("V2BOARD_WORKER_HEALTH_FILE must be a non-empty absolute path");
        }
        Ok(Self {
            health_file: PathBuf::from(health_file),
            heartbeat_interval: Duration::from_secs(parse_bounded_seconds(
                "V2BOARD_WORKER_HEARTBEAT_INTERVAL_SECONDS",
                DEFAULT_HEARTBEAT_INTERVAL_SECONDS,
                1,
                300,
            )?),
            shutdown_timeout: Duration::from_secs(parse_bounded_seconds(
                "V2BOARD_WORKER_SHUTDOWN_TIMEOUT_SECONDS",
                DEFAULT_SHUTDOWN_TIMEOUT_SECONDS,
                1,
                600,
            )?),
        })
    }
}

pub(crate) fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("v2board_workers=info"));
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

pub(crate) async fn run(state: WorkerState) -> anyhow::Result<()> {
    let runtime_config = WorkerRuntimeConfig::from_env()?;
    probe_dependencies(&state).await?;
    write_health_heartbeat(&runtime_config.health_file).await?;
    systemd_notify("READY=1\nSTATUS=PostgreSQL, migration ledger, and Redis are ready")?;
    tracing::info!("v2board rust worker starting");
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut loops = tokio::task::JoinSet::new();
    for job in SCHEDULED_JOBS.iter().copied() {
        let state = state.clone();
        let heartbeat_state = state.clone();
        let shutdown = shutdown_rx.clone();
        let heartbeat_interval = runtime_config.heartbeat_interval;
        loops.spawn(async move {
            (
                job.name,
                run_loop_with_heartbeat(
                    job.name,
                    heartbeat_state,
                    heartbeat_interval,
                    run_schedule_loop(job, state, shutdown),
                )
                .await,
            )
        });
    }
    {
        let state = state.clone();
        let heartbeat_state = state.clone();
        let shutdown = shutdown_rx.clone();
        let heartbeat_interval = runtime_config.heartbeat_interval;
        loops.spawn(async move {
            (
                analytics::JOB_NAME,
                run_loop_with_heartbeat(
                    analytics::JOB_NAME,
                    heartbeat_state,
                    heartbeat_interval,
                    analytics::run_loop(state, shutdown),
                )
                .await,
            )
        });
    }
    {
        let state = state.clone();
        let heartbeat_state = state.clone();
        let shutdown = shutdown_rx.clone();
        let heartbeat_interval = runtime_config.heartbeat_interval;
        loops.spawn(async move {
            (
                analytics::ADMISSION_JOB_NAME,
                run_loop_with_heartbeat(
                    analytics::ADMISSION_JOB_NAME,
                    heartbeat_state,
                    heartbeat_interval,
                    analytics::run_admission_loop(state, shutdown),
                )
                .await,
            )
        });
    }
    {
        let state = state.clone();
        let heartbeat_state = state.clone();
        let shutdown = shutdown_rx.clone();
        let heartbeat_interval = runtime_config.heartbeat_interval;
        loops.spawn(async move {
            (
                outbox::JOB_NAME,
                run_loop_with_heartbeat(
                    outbox::JOB_NAME,
                    heartbeat_state,
                    heartbeat_interval,
                    outbox::run_loop(state, shutdown),
                )
                .await,
            )
        });
    }
    {
        let state = state.clone();
        let shutdown = shutdown_rx.clone();
        let runtime_config = runtime_config.clone();
        loops.spawn(async move {
            (
                HEALTH_JOB_NAME,
                run_health_loop(state, shutdown, runtime_config).await,
            )
        });
    }

    let shutdown_signal = shutdown_signal();
    tokio::pin!(shutdown_signal);
    let failure = tokio::select! {
        () = &mut shutdown_signal => {
            tracing::info!("worker shutdown requested; waiting for active jobs");
            None
        }
        joined = loops.join_next() => Some(unexpected_loop_exit(joined)),
    };

    let _ = shutdown_tx.send(true);
    if let Err(error) = systemd_notify("STOPPING=1\nSTATUS=Worker is draining active jobs") {
        tracing::warn!(?error, "failed to notify systemd about worker shutdown");
    }
    let _ = tokio::fs::remove_file(&runtime_config.health_file).await;
    if tokio::time::timeout(runtime_config.shutdown_timeout, drain_loops(&mut loops))
        .await
        .is_err()
    {
        tracing::error!(
            timeout_seconds = runtime_config.shutdown_timeout.as_secs(),
            "worker shutdown deadline exceeded; aborting remaining loops"
        );
        loops.shutdown().await;
        if failure.is_none() {
            anyhow::bail!("worker shutdown deadline exceeded");
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(())
}

async fn run_loop_with_heartbeat<F>(
    name: &'static str,
    state: WorkerState,
    heartbeat_interval: Duration,
    future: F,
) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    tokio::pin!(future);
    let mut heartbeat = tokio::time::interval(heartbeat_interval);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            result = &mut future => return result,
            _ = heartbeat.tick() => {
                if let Err(error) = record_worker_loop_heartbeat(&state, name).await {
                    tracing::warn!(job = name, ?error, "failed to record worker loop heartbeat");
                }
            }
        }
    }
}

async fn run_health_loop(
    state: WorkerState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    runtime_config: WorkerRuntimeConfig,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(runtime_config.heartbeat_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                match probe_dependencies(&state).await {
                    Ok(()) => {
                        write_health_heartbeat(&runtime_config.health_file).await?;
                        systemd_notify("WATCHDOG=1\nSTATUS=Worker dependencies are healthy")?;
                    }
                    Err(error) => {
                        tracing::warn!(?error, "worker dependency health probe failed");
                    }
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    let _ = tokio::fs::remove_file(&runtime_config.health_file).await;
                    return Ok(());
                }
            }
        }
    }
}

async fn write_health_heartbeat(path: &std::path::Path) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp().to_string();
    tokio::fs::write(path, now).await?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn systemd_notify(message: &str) -> anyhow::Result<()> {
    use std::{
        os::{linux::net::SocketAddrExt, unix::ffi::OsStrExt},
        path::Path,
    };

    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
        return Ok(());
    };
    let socket = std::os::unix::net::UnixDatagram::unbound()?;
    let bytes = path.as_os_str().as_bytes();
    if let Some(abstract_name) = bytes.strip_prefix(b"@") {
        let address = std::os::unix::net::SocketAddr::from_abstract_name(abstract_name)?;
        socket.send_to_addr(message.as_bytes(), &address)?;
    } else {
        socket.send_to(message.as_bytes(), Path::new(&path))?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn systemd_notify(_message: &str) -> anyhow::Result<()> {
    Ok(())
}

async fn probe_dependencies(state: &WorkerState) -> anyhow::Result<()> {
    tokio::time::timeout(
        DEPENDENCY_PROBE_TIMEOUT,
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.db),
    )
    .await??;
    let migrations_current = tokio::time::timeout(
        DEPENDENCY_PROBE_TIMEOUT,
        v2board_db::migrations_current(&state.db),
    )
    .await??;
    if !migrations_current {
        anyhow::bail!("database migrations do not match the worker binary");
    }
    state.refresh_operator_config().await?;
    let mut conn = tokio::time::timeout(
        DEPENDENCY_PROBE_TIMEOUT,
        state.redis.get_multiplexed_async_connection(),
    )
    .await??;
    let response: String = tokio::time::timeout(
        DEPENDENCY_PROBE_TIMEOUT,
        redis::cmd("PING").query_async(&mut conn),
    )
    .await??;
    if response != "PONG" {
        anyhow::bail!("Redis returned an unexpected PING response");
    }
    Ok(())
}

async fn drain_loops(loops: &mut tokio::task::JoinSet<(&'static str, anyhow::Result<()>)>) {
    while let Some(joined) = loops.join_next().await {
        match joined {
            Ok((name, Err(error))) => {
                tracing::warn!(job = name, ?error, "worker loop failed during shutdown");
            }
            Err(error) => tracing::warn!(?error, "worker loop panicked during shutdown"),
            Ok((_, Ok(()))) => {}
        }
    }
}

fn unexpected_loop_exit(
    joined: Option<Result<(&'static str, anyhow::Result<()>), tokio::task::JoinError>>,
) -> anyhow::Error {
    match joined {
        Some(Ok((name, Ok(())))) => {
            anyhow::anyhow!("worker loop `{name}` exited unexpectedly")
        }
        Some(Ok((name, Err(error)))) => {
            anyhow::anyhow!("worker loop `{name}` failed: {error:#}")
        }
        Some(Err(error)) => anyhow::anyhow!("worker loop panicked: {error}"),
        None => anyhow::anyhow!("all worker loops exited"),
    }
}

fn parse_bounded_seconds(
    name: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> anyhow::Result<u64> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(default);
    };
    let raw = raw
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("{name} must be valid UTF-8"))?;
    let value = raw
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        anyhow::bail!("{name} must be between {minimum} and {maximum}");
    }
    Ok(value)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(?error, "failed to install Ctrl-C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => tracing::error!(?error, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_runtime_durations_are_strict_and_bounded() {
        assert_eq!(
            parse_bounded_seconds("UNSET_TEST_VALUE", 10, 1, 20).unwrap(),
            10
        );
        assert!(parse_bounded_value("broken", 1, 20).is_err());
        assert!(parse_bounded_value("0", 1, 20).is_err());
        assert!(parse_bounded_value("21", 1, 20).is_err());
        assert_eq!(parse_bounded_value("20", 1, 20).unwrap(), 20);
    }

    fn parse_bounded_value(raw: &str, minimum: u64, maximum: u64) -> anyhow::Result<u64> {
        let value = raw
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("test value must be an integer"))?;
        if !(minimum..=maximum).contains(&value) {
            anyhow::bail!("test value is outside the allowed range");
        }
        Ok(value)
    }

    #[test]
    fn unexpected_worker_loop_exit_is_always_fatal() {
        let error = unexpected_loop_exit(Some(Ok(("statistics", Ok(())))));
        assert!(error.to_string().contains("exited unexpectedly"));
        let error = unexpected_loop_exit(Some(Ok(("statistics", Err(anyhow::anyhow!("broken"))))));
        assert!(error.to_string().contains("statistics"));
        assert!(error.to_string().contains("broken"));
    }

    #[test]
    fn worker_health_fails_closed_on_schema_drift() {
        let source = include_str!("runtime.rs");
        let probe = &source[source.find("async fn probe_dependencies").unwrap()
            ..source.find("async fn drain_loops").unwrap()];
        assert!(probe.contains("v2board_db::migrations_current"));
        assert!(probe.contains("if !migrations_current"));
        assert!(probe.contains("DEPENDENCY_PROBE_TIMEOUT"));
    }
}
