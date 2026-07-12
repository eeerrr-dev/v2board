use std::time::Duration;

use chrono::{DateTime, Datelike, FixedOffset, Months, TimeZone, Utc};
use sqlx::{FromRow, Postgres, QueryBuilder};
use uuid::Uuid;
use v2board_config::{app_now, app_timezone};

use crate::{
    lease::{SCHEDULER_LOCK_TTL_SECS, SchedulerLock, release_scheduler_lock, run_with_lease},
    state::WorkerState,
    time::timestamp_before,
};

pub(crate) const TRAFFIC_RESET_LOCK_KEY: &str = "traffic_reset_lock";
const TRAFFIC_UPDATE_SCHEDULER_LOCK_KEY: &str = "RUST_SCHEDULER_LOCK_traffic_update";
const RESET_LOCK_IO_TIMEOUT: Duration = Duration::from_secs(5);
const RESET_USER_BATCH_SIZE: i64 = 500;
const RETENTION_DELETE_BATCH_SIZE: i64 = 5_000;
const RETENTION_MAX_BATCHES_PER_TABLE: usize = 20;
const STAT_USER_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id FROM v2_stat_user
    WHERE record_at < $1
    ORDER BY record_at, id
    LIMIT $2
)
DELETE FROM v2_stat_user AS target
USING doomed
WHERE target.id = doomed.id
"#;
const STAT_SERVER_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id FROM v2_stat_server
    WHERE record_at < $1
    ORDER BY record_at, id
    LIMIT $2
)
DELETE FROM v2_stat_server AS target
USING doomed
WHERE target.id = doomed.id
"#;
const SYSTEM_LOG_RETENTION_SQL: &str = r#"
WITH doomed AS (
    SELECT id FROM v2_log
    WHERE created_at < $1
    ORDER BY created_at, id
    LIMIT $2
)
DELETE FROM v2_log AS target
USING doomed
WHERE target.id = doomed.id
"#;

#[derive(Debug, Clone, FromRow)]
struct ResetUserRow {
    id: i64,
    expired_at: i64,
    reset_traffic_method: Option<i16>,
}

async fn acquire_traffic_reset_lock(state: &WorkerState) -> anyhow::Result<SchedulerLock> {
    let token = Uuid::new_v4().to_string();
    loop {
        let acquired: i64 = tokio::time::timeout(RESET_LOCK_IO_TIMEOUT, async {
            let mut conn = state.redis.get_multiplexed_async_connection().await?;
            // This is an availability barrier; quota_epoch is the durable
            // correctness fence. Redis executes the two-key admission check
            // atomically so ordinary runs avoid producing stale reports.
            redis::Script::new(
                r#"
                if redis.call("EXISTS", KEYS[1]) == 1
                    or redis.call("EXISTS", KEYS[2]) == 1 then
                    return 0
                end
                local result = redis.call("SET", KEYS[1], ARGV[1], "NX", "EX", ARGV[2])
                if result then return 1 end
                return 0
                "#,
            )
            .key(TRAFFIC_RESET_LOCK_KEY)
            .key(TRAFFIC_UPDATE_SCHEDULER_LOCK_KEY)
            .arg(&token)
            .arg(SCHEDULER_LOCK_TTL_SECS)
            .invoke_async(&mut conn)
            .await
        })
        .await
        .map_err(|_| anyhow::anyhow!("timed out acquiring traffic reset barrier"))??;
        if acquired == 1 {
            return Ok(SchedulerLock {
                key: TRAFFIC_RESET_LOCK_KEY.to_string(),
                token,
            });
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
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
    let now = Utc::now().timestamp();
    let reset_key = app_now().format("%Y-%m-%d").to_string();
    // INNER JOIN, not LEFT JOIN: ResetTraffic.php groups existing plans by
    // reset_traffic_method and only resets users whose plan_id is in one of those
    // GROUP_CONCAT lists (`whereIn('plan_id', $planIds)`). A user with a NULL or
    // orphaned plan_id is never in any list, so it is never reset. The join keeps
    // only users backed by a real plan; a matched row with a NULL method genuinely
    // means "plan exists but method is NULL" and falls through to the config default.
    let mut after_id = 0_i64;
    loop {
        let mut tx = state.db.begin().await?;
        let users = sqlx::query_as::<_, ResetUserRow>(
            r#"
            SELECT u.id, u.expired_at, p.reset_traffic_method
            FROM v2_user u
            INNER JOIN v2_plan p ON p.id = u.plan_id
            WHERE u.id > $1
              AND u.expired_at IS NOT NULL
              AND u.expired_at > $2
            ORDER BY u.id
            LIMIT $3
            FOR UPDATE
            "#,
        )
        .bind(after_id)
        .bind(now)
        .bind(RESET_USER_BATCH_SIZE)
        .fetch_all(&mut *tx)
        .await?;
        let Some(last_id) = users.last().map(|user| user.id) else {
            tx.commit().await?;
            break;
        };
        let ids = users
            .iter()
            .filter(|user| should_reset_user(user, state.config.reset_traffic_method))
            .map(|user| user.id)
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            let mut builder = QueryBuilder::<Postgres>::new(
                "UPDATE v2_user SET traffic_epoch = traffic_epoch + 1, \
                 u = 0, d = 0, scheduled_traffic_reset_key = ",
            );
            builder.push_bind(&reset_key);
            builder.push(", updated_at = ");
            builder.push_bind(now);
            builder.push(" WHERE id IN (");
            {
                let mut separated = builder.separated(", ");
                for id in ids {
                    separated.push_bind(id);
                }
            }
            builder.push(
                ") AND (scheduled_traffic_reset_key IS NULL OR scheduled_traffic_reset_key <> ",
            );
            builder.push_bind(&reset_key);
            builder.push(")");
            builder.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
        after_id = last_id;
    }
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
                && Utc::now().timestamp() < timestamp_before(expired_at, 2_160_000)
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

pub(crate) async fn run_log(state: &WorkerState) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let stat_before =
        month_delta_timestamp(2).unwrap_or_else(|| timestamp_before(now, 60 * 86_400));
    let log_before = month_delta_timestamp(1).unwrap_or_else(|| timestamp_before(now, 30 * 86_400));
    for (sql, cutoff) in [
        (STAT_USER_RETENTION_SQL, stat_before),
        (STAT_SERVER_RETENTION_SQL, stat_before),
        (SYSTEM_LOG_RETENTION_SQL, log_before),
    ] {
        for _ in 0..RETENTION_MAX_BATCHES_PER_TABLE {
            let deleted = sqlx::query(sql)
                .bind(cutoff)
                .bind(RETENTION_DELETE_BATCH_SIZE)
                .execute(&state.db)
                .await?
                .rows_affected();
            if deleted < RETENTION_DELETE_BATCH_SIZE as u64 {
                break;
            }
        }
    }
    Ok(())
}

fn month_delta_timestamp(months: u32) -> Option<i64> {
    app_now()
        .checked_sub_months(Months::new(months))
        .map(|date| date.timestamp())
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn retention_deletes_are_index_ordered_and_bounded() {
        for sql in [
            STAT_USER_RETENTION_SQL,
            STAT_SERVER_RETENTION_SQL,
            SYSTEM_LOG_RETENTION_SQL,
        ] {
            assert!(sql.contains("WITH doomed AS"));
            assert!(sql.contains("ORDER BY"));
            assert!(sql.contains("LIMIT $2"));
            assert!(sql.contains("USING doomed"));
        }
    }
}
