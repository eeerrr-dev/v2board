use chrono::{Datelike, Months, TimeZone, Utc};
use v2board_application::maintenance::{
    RetentionCutoff, RetentionDataset, RetentionService, ScheduledTrafficResetRun,
    ScheduledTrafficResetService, TrafficResetCalendar,
};
#[cfg(test)]
use v2board_application::maintenance::{ScheduledResetCandidate, scheduled_reset_is_due};
use v2board_config::{app_now, app_timezone};
use v2board_db::maintenance::PostgresMaintenanceRepository;
use v2board_domain_model::CalendarDay;

use crate::{
    lease::{release_scheduler_lock, run_with_lease},
    state::WorkerState,
    time::timestamp_before,
    traffic_adapters::acquire_traffic_reset_lock,
};

const RESET_USER_BATCH_SIZE: i64 = 500;
const RETENTION_DELETE_BATCH_SIZE: i64 = 5_000;
const RETENTION_MAX_BATCHES_PER_TABLE: usize = 20;

#[derive(Clone, Copy, Debug, Default)]
struct AppTrafficResetCalendar;

impl TrafficResetCalendar for AppTrafficResetCalendar {
    fn day_at(&self, timestamp: i64) -> Option<CalendarDay> {
        let instant = app_timezone().timestamp_opt(timestamp, 0).single()?;
        calendar_day(instant.year(), instant.month(), instant.day())
    }
}

pub(crate) async fn run_traffic(state: &WorkerState) -> anyhow::Result<()> {
    let reset_lock = acquire_traffic_reset_lock(state).await?;
    // Keep reset work in the lease-owning future. Dropping this future on loss
    // of the outer scheduler lease cancels the database work instead of
    // detaching an unmonitored child task.
    let result = run_with_lease(reset_traffic_inner(state), state, &reset_lock).await;
    if let Err(release_error) = release_scheduler_lock(state, reset_lock).await {
        if result.is_ok() {
            return Err(release_error);
        }
        tracing::warn!(?release_error, "failed to release traffic reset barrier");
    }
    result
}

async fn reset_traffic_inner(state: &WorkerState) -> anyhow::Result<()> {
    let now = app_now();
    let Some(now_day) = calendar_day(now.year(), now.month(), now.day()) else {
        anyhow::bail!("application timezone produced an invalid calendar day");
    };
    let command = ScheduledTrafficResetRun {
        now_epoch: now.timestamp(),
        now_day,
        reset_key: now.format("%Y-%m-%d").to_string(),
        default_method: state.config.reset_traffic_method,
        batch_size: RESET_USER_BATCH_SIZE,
    };
    ScheduledTrafficResetService::new(
        PostgresMaintenanceRepository::new(state.db.clone()),
        AppTrafficResetCalendar,
    )
    .run(&command)
    .await?;
    Ok(())
}

#[cfg(test)]
fn should_reset_user(user: &ScheduledResetCandidate, default_method: i32) -> bool {
    let now = app_now();
    let Some(now_day) = calendar_day(now.year(), now.month(), now.day()) else {
        return false;
    };
    scheduled_reset_is_due(
        user,
        &ScheduledTrafficResetRun {
            now_epoch: now.timestamp(),
            now_day,
            reset_key: now.format("%Y-%m-%d").to_string(),
            default_method,
            batch_size: RESET_USER_BATCH_SIZE,
        },
        &AppTrafficResetCalendar,
    )
}

fn calendar_day(year: i32, month: u32, day: u32) -> Option<CalendarDay> {
    let last_day = last_day_of_month(year, month)?;
    CalendarDay::new(
        u8::try_from(month).ok()?,
        u8::try_from(day).ok()?,
        u8::try_from(last_day).ok()?,
    )
    .ok()
}

pub(crate) async fn run_log(state: &WorkerState) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let stat_before =
        month_delta_timestamp(2).unwrap_or_else(|| timestamp_before(now, 60 * 86_400));
    let log_before = month_delta_timestamp(1).unwrap_or_else(|| timestamp_before(now, 30 * 86_400));
    RetentionService::new(PostgresMaintenanceRepository::new(state.db.clone()))
        .prune(
            &[
                RetentionCutoff {
                    dataset: RetentionDataset::UserTraffic,
                    before: stat_before,
                },
                RetentionCutoff {
                    dataset: RetentionDataset::ServerTraffic,
                    before: stat_before,
                },
                RetentionCutoff {
                    dataset: RetentionDataset::SystemLog,
                    before: log_before,
                },
            ],
            RETENTION_DELETE_BATCH_SIZE,
            RETENTION_MAX_BATCHES_PER_TABLE,
        )
        .await?;
    Ok(())
}

fn month_delta_timestamp(months: u32) -> Option<i64> {
    app_now()
        .checked_sub_months(Months::new(months))
        .map(|date| date.timestamp())
}

#[cfg(test)]
fn last_day_of_current_month() -> u32 {
    let today = app_now().date_naive();
    last_day_of_month(today.year(), today.month()).unwrap_or(today.day())
}

fn last_day_of_month(year: i32, month: u32) -> Option<u32> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month.checked_add(1)?)
    };
    let first_next_month = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
    Some((first_next_month - chrono::Duration::days(1)).day())
}

#[cfg(test)]
mod tests {
    use v2board_config::freeze_time;

    use super::*;

    fn shanghai_ts(year: i32, month: u32, day: u32, hour: u32) -> i64 {
        app_timezone()
            .with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .expect("valid Shanghai timestamp")
            .timestamp()
    }

    fn expire_day_user(expired_at: i64) -> ScheduledResetCandidate {
        ScheduledResetCandidate {
            id: 1,
            expired_at,
            reset_traffic_method: Some(1),
        }
    }

    #[test]
    fn expire_day_reset_clamps_to_the_short_month_end() {
        // Expiry anniversary on the 31st; February has no 31st, so the reset
        // must fire on February's last day. The frozen instant is still
        // Feb 27 in UTC — the calendar decision is pinned to Asia/Shanghai.
        let expired_at = shanghai_ts(2026, 3, 31, 10);
        let frozen = Utc.with_ymd_and_hms(2026, 2, 27, 20, 0, 0).unwrap();
        assert_eq!(frozen.with_timezone(&app_timezone()).day(), 28);
        let _clock = freeze_time(frozen);
        assert!(should_reset_user(&expire_day_user(expired_at), 2));
    }

    #[test]
    fn expire_day_reset_skips_ordinary_non_matching_days() {
        let expired_at = shanghai_ts(2026, 3, 31, 10);
        let _clock = freeze_time(Utc.with_ymd_and_hms(2026, 2, 26, 20, 0, 0).unwrap());
        // Shanghai Feb 27: neither the expire day nor the month's last day.
        assert!(!should_reset_user(&expire_day_user(expired_at), 2));
    }

    #[test]
    fn expire_day_reset_yields_to_the_25_day_expiry_guard() {
        // The day matches (the 31st), but the subscription expires later the
        // same day — inside the 25-day guard window, so no reset is due.
        let expired_at = shanghai_ts(2026, 3, 31, 23);
        let _clock = freeze_time(Utc.with_ymd_and_hms(2026, 3, 30, 17, 0, 0).unwrap());
        assert!(!should_reset_user(&expire_day_user(expired_at), 2));
    }

    #[test]
    fn month_first_day_reset_uses_the_shanghai_calendar_day() {
        let user = ScheduledResetCandidate {
            id: 1,
            expired_at: shanghai_ts(2027, 1, 1, 0),
            reset_traffic_method: Some(0),
        };
        // UTC Feb 28 16:30 is already Mar 1 00:30 in Shanghai: reset fires.
        {
            let _clock = freeze_time(Utc.with_ymd_and_hms(2026, 2, 28, 16, 30, 0).unwrap());
            assert!(should_reset_user(&user, 2));
        }
        // UTC Mar 1 20:00 is already Mar 2 in Shanghai: no reset.
        let _clock = freeze_time(Utc.with_ymd_and_hms(2026, 3, 1, 20, 0, 0).unwrap());
        assert!(!should_reset_user(&user, 2));
    }

    #[test]
    fn expire_year_reset_fires_only_on_the_exact_anniversary() {
        let user = ScheduledResetCandidate {
            id: 1,
            expired_at: shanghai_ts(2027, 6, 15, 12),
            reset_traffic_method: Some(4),
        };
        {
            let _clock = freeze_time(Utc.with_ymd_and_hms(2026, 6, 15, 4, 0, 0).unwrap());
            assert!(should_reset_user(&user, 2));
        }
        let _clock = freeze_time(Utc.with_ymd_and_hms(2026, 6, 16, 4, 0, 0).unwrap());
        assert!(!should_reset_user(&user, 2));
    }

    #[test]
    fn last_day_of_month_handles_december_and_leap_february() {
        {
            let _clock = freeze_time(Utc.with_ymd_and_hms(2026, 12, 15, 4, 0, 0).unwrap());
            assert_eq!(last_day_of_current_month(), 31);
        }
        let _clock = freeze_time(Utc.with_ymd_and_hms(2028, 2, 10, 4, 0, 0).unwrap());
        assert_eq!(last_day_of_current_month(), 29);
    }

    #[test]
    fn null_reset_method_default_three_falls_through_to_expire_year() {
        // A plan with reset_traffic_method = NULL whose expiry anniversary (m-d) is today.
        let now_ts = app_now().timestamp();
        let user = ScheduledResetCandidate {
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
        let user = ScheduledResetCandidate {
            id: 1,
            expired_at: now_ts,
            reset_traffic_method: Some(2),
        };
        assert!(!should_reset_user(&user, 3));
    }
}
