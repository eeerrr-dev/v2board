use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    outbox,
    scheduler::{SCHEDULED_JOBS, run_schedule_loop},
    state::WorkerState,
};

pub(crate) fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("v2board_workers=info"));
    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

pub(crate) async fn run(state: WorkerState) -> anyhow::Result<()> {
    tracing::info!("v2board rust worker starting");
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut loops = tokio::task::JoinSet::new();
    for job in SCHEDULED_JOBS.iter().copied() {
        let state = state.clone();
        let shutdown = shutdown_rx.clone();
        loops.spawn(async move { (job.name, run_schedule_loop(job, state, shutdown).await) });
    }
    {
        let state = state.clone();
        let shutdown = shutdown_rx.clone();
        loops.spawn(async move { (outbox::JOB_NAME, outbox::run_loop(state, shutdown).await) });
    }

    let shutdown_signal = shutdown_signal();
    tokio::pin!(shutdown_signal);
    loop {
        tokio::select! {
            () = &mut shutdown_signal => {
                tracing::info!("worker shutdown requested; waiting for active jobs");
                let _ = shutdown_tx.send(true);
                break;
            }
            joined = loops.join_next() => {
                match joined {
                    Some(Ok((name, Ok(())))) => {
                        tracing::error!(job = name, "scheduler loop exited unexpectedly");
                    }
                    Some(Ok((name, Err(error)))) => {
                        tracing::error!(job = name, ?error, "scheduler loop failed");
                    }
                    Some(Err(error)) => {
                        tracing::error!(?error, "scheduler loop panicked");
                    }
                    None => anyhow::bail!("all scheduler loops exited"),
                }
                // A single malformed/failed loop is isolated; every other job
                // keeps its own timer and continues running.
            }
        }
    }

    while let Some(joined) = loops.join_next().await {
        match joined {
            Ok((name, Err(error))) => {
                tracing::warn!(job = name, ?error, "scheduler loop failed during shutdown");
            }
            Err(error) => tracing::warn!(?error, "scheduler loop panicked during shutdown"),
            Ok((_, Ok(()))) => {}
        }
    }
    Ok(())
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
