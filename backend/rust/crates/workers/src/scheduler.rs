use std::{str::FromStr, time::Duration};

use chrono::{DateTime, FixedOffset};
use cron::Schedule;
use v2board_config::app_now;

use crate::{
    commission,
    lease::{SchedulerLock, acquire_scheduler_lock, release_scheduler_lock, run_task_with_lease},
    metrics::{mark_scheduler_alive, record_worker_metric},
    orders, reminders, renewal, reset,
    state::WorkerState,
    statistics, tickets, traffic,
};

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScheduledJob {
    pub(crate) name: &'static str,
    expression: &'static str,
    task: ScheduledTask,
}

pub(crate) const SCHEDULED_JOBS: &[ScheduledJob] = &[
    ScheduledJob {
        name: "traffic_update",
        expression: "0 * * * * * *",
        task: ScheduledTask::TrafficUpdate,
    },
    ScheduledJob {
        name: "statistics",
        expression: "0 10 0 * * * *",
        task: ScheduledTask::Statistics,
    },
    ScheduledJob {
        name: "check_order",
        expression: "0 * * * * * *",
        task: ScheduledTask::CheckOrder,
    },
    ScheduledJob {
        name: "check_commission",
        expression: "0 0/15 * * * * *",
        task: ScheduledTask::CheckCommission,
    },
    ScheduledJob {
        name: "check_ticket",
        expression: "0 * * * * * *",
        task: ScheduledTask::CheckTicket,
    },
    ScheduledJob {
        name: "check_renewal",
        expression: "0 30 22 * * * *",
        task: ScheduledTask::CheckRenewal,
    },
    ScheduledJob {
        name: "reset_traffic",
        expression: "0 0 0 * * * *",
        task: ScheduledTask::ResetTraffic,
    },
    ScheduledJob {
        name: "reset_log",
        expression: "0 0 0 * * * *",
        task: ScheduledTask::ResetLog,
    },
    ScheduledJob {
        name: "send_remind_mail",
        expression: "0 30 11 * * * *",
        task: ScheduledTask::SendRemindMail,
    },
];

fn next_scheduled_tick(
    schedule: &Schedule,
    after: DateTime<FixedOffset>,
) -> Option<DateTime<FixedOffset>> {
    schedule.after(&after).next()
}

pub(crate) async fn run_schedule_loop(
    job: ScheduledJob,
    state: WorkerState,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let schedule = Schedule::from_str(job.expression)?;
    loop {
        if *shutdown.borrow() {
            return Ok(());
        }
        let now = app_now();
        let tick = next_scheduled_tick(&schedule, now)
            .ok_or_else(|| anyhow::anyhow!("schedule for {} has no future tick", job.name))?;
        let delay = tick
            .signed_duration_since(app_now())
            .to_std()
            .unwrap_or(Duration::ZERO);
        let deadline = tokio::time::Instant::now() + delay;
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
                continue;
            }
        }

        // The next tick is computed only after this execution completes. If a
        // job runs across one or more scheduled instants, those historical ticks
        // are intentionally skipped rather than replayed in a burst.
        run_scheduled_tick(job, &state, tick).await;
    }
}

pub(crate) async fn run_command(args: &[String], state: &WorkerState) -> anyhow::Result<()> {
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

    let result = run_scheduled_job_with_lease(task, state, &scheduler_lock).await;
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

async fn run_scheduled_tick(job: ScheduledJob, state: &WorkerState, tick: DateTime<FixedOffset>) {
    tracing::info!(job = job.name, %tick, "received scheduled job");
    if let Err(error) = mark_scheduler_alive(state).await {
        tracing::warn!(?error, "failed to update scheduler heartbeat");
    }

    let scheduler_lock = match acquire_scheduler_lock(state, job.name).await {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            tracing::info!(
                job = job.name,
                "scheduled job skipped because another worker owns it"
            );
            return;
        }
        Err(error) => {
            tracing::error!(
                job = job.name,
                ?error,
                "failed to acquire scheduled job lock"
            );
            let _ = record_worker_metric(state, job.name, false).await;
            return;
        }
    };

    // The lease is renewed while the job runs. Losing Redis ownership aborts the
    // task before the original TTL can expire and a second scheduler can overlap
    // it; dropping an in-flight SQL transaction rolls it back.
    let job_result = run_scheduled_job_with_lease(job.task, state, &scheduler_lock).await;
    if let Err(error) = release_scheduler_lock(state, scheduler_lock).await {
        tracing::warn!(
            job = job.name,
            ?error,
            "failed to release scheduled job lock"
        );
    }
    match job_result {
        Ok(()) => {
            if let Err(error) = record_worker_metric(state, job.name, true).await {
                tracing::warn!(
                    job = job.name,
                    ?error,
                    "failed to record scheduled job metric"
                );
            }
        }
        Err(error) => {
            tracing::error!(job = job.name, ?error, "scheduled job failed");
            let _ = record_worker_metric(state, job.name, false).await;
        }
    }
}

async fn run_scheduled_job(task: ScheduledTask, state: &WorkerState) -> anyhow::Result<()> {
    // Each job gets one immutable snapshot. A valid edit becomes the shared
    // last-known-good value; a malformed edit is logged and this job continues
    // consistently on the previous snapshot.
    let state = state.snapshot_config_for_job().await;
    match task {
        ScheduledTask::TrafficUpdate => traffic::run(&state).await,
        ScheduledTask::Statistics => statistics::run(&state).await,
        ScheduledTask::CheckOrder => orders::run(&state).await,
        ScheduledTask::CheckCommission => commission::run(&state).await,
        ScheduledTask::CheckTicket => tickets::run(&state).await,
        ScheduledTask::CheckRenewal => renewal::run(&state).await,
        ScheduledTask::ResetTraffic => reset::run_traffic(&state).await,
        ScheduledTask::ResetLog => reset::run_log(&state).await,
        ScheduledTask::SendRemindMail => reminders::run(&state).await,
    }
}

async fn run_scheduled_job_with_lease(
    task: ScheduledTask,
    state: &WorkerState,
    scheduler_lock: &SchedulerLock,
) -> anyhow::Result<()> {
    let job_state = state.clone();
    let task_handle = tokio::spawn(async move { run_scheduled_job(task, &job_state).await });
    run_task_with_lease(task_handle, state, scheduler_lock).await
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use v2board_config::app_timezone;

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
        assert_eq!(
            SCHEDULED_JOBS
                .iter()
                .map(|job| job.name)
                .collect::<Vec<_>>(),
            SCHEDULED_TASK_NAMES
        );
    }

    #[test]
    fn scheduled_task_lookup_covers_the_scheduler_matrix() {
        for (name, job) in SCHEDULED_TASK_NAMES.iter().zip(SCHEDULED_JOBS) {
            assert!(
                scheduled_task_by_name(name).is_some(),
                "{name} is scheduled but cannot be run once"
            );
            assert!(Schedule::from_str(job.expression).is_ok());
        }
        assert!(scheduled_task_by_name("missing").is_none());
    }

    #[test]
    fn schedules_use_application_timezone_and_skip_the_current_tick() {
        let timezone = app_timezone();
        let after = timezone
            .with_ymd_and_hms(2026, 7, 11, 22, 29, 59)
            .single()
            .unwrap();
        let renewal = SCHEDULED_JOBS
            .iter()
            .find(|job| job.name == "check_renewal")
            .unwrap();
        let schedule = Schedule::from_str(renewal.expression).unwrap();
        assert_eq!(
            next_scheduled_tick(&schedule, after),
            timezone.with_ymd_and_hms(2026, 7, 11, 22, 30, 0).single()
        );

        let exact_tick = timezone
            .with_ymd_and_hms(2026, 7, 11, 22, 30, 0)
            .single()
            .unwrap();
        assert_eq!(
            next_scheduled_tick(&schedule, exact_tick),
            timezone.with_ymd_and_hms(2026, 7, 12, 22, 30, 0).single()
        );
        for job in SCHEDULED_JOBS {
            let schedule = Schedule::from_str(job.expression).unwrap();
            assert!(next_scheduled_tick(&schedule, exact_tick).unwrap() > exact_tick);
        }
    }
}
