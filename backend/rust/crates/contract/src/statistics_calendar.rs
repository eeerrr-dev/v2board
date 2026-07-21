use chrono::{Datelike, TimeZone, Utc};
use v2board_application::statistics::{StatisticsBoundaries, StatisticsCalendar};
use v2board_config::{app_now, app_timezone};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ContractStatisticsCalendar;

impl StatisticsCalendar for ContractStatisticsCalendar {
    fn boundaries(&self) -> StatisticsBoundaries {
        let now = app_now();
        let timezone = app_timezone();
        let today = timezone
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .map(|value| value.timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        let month = timezone
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .map(|value| value.timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        let (previous_year, previous_month) = if now.month() == 1 {
            (now.year() - 1, 12)
        } else {
            (now.year(), now.month() - 1)
        };
        let previous_month = timezone
            .with_ymd_and_hms(previous_year, previous_month, 1, 0, 0, 0)
            .single()
            .map(|value| value.timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        StatisticsBoundaries {
            now: now.timestamp(),
            today,
            yesterday: today.saturating_sub(86_400),
            month,
            previous_month,
        }
    }

    fn month_day(&self, timestamp: i64) -> String {
        app_timezone()
            .timestamp_opt(timestamp, 0)
            .single()
            .map(|value| value.format("%m-%d").to_string())
            .unwrap_or_default()
    }
}
