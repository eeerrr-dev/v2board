#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficResetMethod {
    MonthStart,
    ExpiryDay,
    Never,
    YearStart,
    ExpiryAnniversary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledTrafficResetPolicy {
    Explicit(TrafficResetMethod),
    /// The retained scheduler contract has one intentional fall-through when
    /// the configured default is `YearStart`.
    LegacyDefault(TrafficResetMethod),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CalendarDay {
    month: u8,
    day: u8,
    last_day_of_month: u8,
}

impl CalendarDay {
    pub const fn new(month: u8, day: u8, last_day_of_month: u8) -> Result<Self, CalendarDayError> {
        if month == 0 || month > 12 {
            return Err(CalendarDayError::InvalidMonth(month));
        }
        if last_day_of_month < 28 || last_day_of_month > 31 {
            return Err(CalendarDayError::InvalidLastDay(last_day_of_month));
        }
        if day == 0 || day > last_day_of_month {
            return Err(CalendarDayError::InvalidDay(day));
        }
        Ok(Self {
            month,
            day,
            last_day_of_month,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CalendarDayError {
    InvalidMonth(u8),
    InvalidDay(u8),
    InvalidLastDay(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficResetFacts {
    pub now: CalendarDay,
    pub expiry: CalendarDay,
    pub now_epoch: i64,
    pub expiry_epoch: i64,
}

pub fn scheduled_traffic_reset_due(
    policy: ScheduledTrafficResetPolicy,
    facts: TrafficResetFacts,
) -> bool {
    let method = match policy {
        ScheduledTrafficResetPolicy::Explicit(method)
        | ScheduledTrafficResetPolicy::LegacyDefault(method) => method,
    };
    reset_method_matches(method, facts)
        || matches!(
            policy,
            ScheduledTrafficResetPolicy::LegacyDefault(TrafficResetMethod::YearStart)
        ) && reset_method_matches(TrafficResetMethod::ExpiryAnniversary, facts)
}

fn reset_method_matches(method: TrafficResetMethod, facts: TrafficResetFacts) -> bool {
    match method {
        TrafficResetMethod::MonthStart => facts.now.day == 1,
        TrafficResetMethod::ExpiryDay => {
            let calendar_matches = facts.expiry.day == facts.now.day
                || (facts.now.day == facts.now.last_day_of_month
                    && facts.expiry.day >= facts.now.last_day_of_month);
            calendar_matches && facts.now_epoch < facts.expiry_epoch.saturating_sub(2_160_000)
        }
        TrafficResetMethod::Never => false,
        TrafficResetMethod::YearStart => facts.now.month == 1 && facts.now.day == 1,
        TrafficResetMethod::ExpiryAnniversary => {
            facts.now.month == facts.expiry.month && facts.now.day == facts.expiry.day
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NewPeriodWindow {
    pub reset_days: i64,
    pub period_days: i64,
}

impl NewPeriodWindow {
    pub const fn for_method(method: TrafficResetMethod, scheduled_days: i64) -> Option<Self> {
        let (reset_days, period_days) = match method {
            TrafficResetMethod::MonthStart => (30, 30),
            TrafficResetMethod::ExpiryDay => (scheduled_days, 30),
            TrafficResetMethod::Never => return None,
            TrafficResetMethod::YearStart => (365, 365),
            TrafficResetMethod::ExpiryAnniversary => (scheduled_days, 365),
        };
        Some(Self {
            reset_days: if reset_days <= 0 {
                period_days
            } else {
                reset_days
            },
            period_days,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NewPeriodError {
    NegativeDuration,
    ResetPeriodOutOfRange,
    ExpiryOutOfRange,
}

pub fn checked_reset_subscription_expiry(
    expiry: i64,
    window: NewPeriodWindow,
    now: i64,
) -> Result<Option<i64>, NewPeriodError> {
    if window.reset_days < 0 || window.period_days < 0 {
        return Err(NewPeriodError::NegativeDuration);
    }
    let threshold = window
        .period_days
        .checked_add(1)
        .and_then(|days| days.checked_mul(86_400))
        .ok_or(NewPeriodError::ResetPeriodOutOfRange)?;
    let remaining = expiry
        .checked_sub(now)
        .ok_or(NewPeriodError::ExpiryOutOfRange)?;
    if threshold >= remaining {
        return Ok(None);
    }
    let reset_seconds = window
        .reset_days
        .checked_mul(86_400)
        .ok_or(NewPeriodError::ResetPeriodOutOfRange)?;
    expiry
        .checked_sub(reset_seconds)
        .map(Some)
        .ok_or(NewPeriodError::ExpiryOutOfRange)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubscriptionAvailability {
    pub banned: bool,
    pub transfer_enable: i64,
    pub expiry: Option<i64>,
}

impl SubscriptionAvailability {
    pub const fn is_available(self, now: i64) -> bool {
        let unexpired = match self.expiry {
            Some(expiry) => expiry > now,
            None => true,
        };
        !self.banned && self.transfer_enable > 0 && unexpired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(month: u8, day: u8, last: u8) -> CalendarDay {
        CalendarDay::new(month, day, last).unwrap()
    }

    #[test]
    fn scheduler_default_year_start_keeps_the_legacy_fallthrough() {
        let facts = TrafficResetFacts {
            now: day(6, 15, 30),
            expiry: day(6, 15, 30),
            now_epoch: 10,
            expiry_epoch: 3_000_000,
        };
        assert!(scheduled_traffic_reset_due(
            ScheduledTrafficResetPolicy::LegacyDefault(TrafficResetMethod::YearStart),
            facts
        ));
        assert!(!scheduled_traffic_reset_due(
            ScheduledTrafficResetPolicy::Explicit(TrafficResetMethod::YearStart),
            facts
        ));
    }

    #[test]
    fn expiry_day_clamps_to_short_month_and_keeps_guard() {
        let due = TrafficResetFacts {
            now: day(2, 28, 28),
            expiry: day(3, 31, 31),
            now_epoch: 0,
            expiry_epoch: 3_000_000,
        };
        assert!(scheduled_traffic_reset_due(
            ScheduledTrafficResetPolicy::Explicit(TrafficResetMethod::ExpiryDay),
            due
        ));
        assert!(!scheduled_traffic_reset_due(
            ScheduledTrafficResetPolicy::Explicit(TrafficResetMethod::ExpiryDay),
            TrafficResetFacts {
                now_epoch: 900_000,
                ..due
            }
        ));
    }

    #[test]
    fn new_period_math_is_checked_and_policy_driven() {
        let window = NewPeriodWindow::for_method(TrafficResetMethod::MonthStart, 4).unwrap();
        assert_eq!(
            window,
            NewPeriodWindow {
                reset_days: 30,
                period_days: 30
            }
        );
        assert_eq!(
            checked_reset_subscription_expiry(100 * 86_400, window, 0).unwrap(),
            Some(70 * 86_400)
        );
        assert_eq!(
            checked_reset_subscription_expiry(31 * 86_400, window, 0).unwrap(),
            None
        );
    }

    #[test]
    fn availability_is_a_pure_snapshot_decision() {
        assert!(
            SubscriptionAvailability {
                banned: false,
                transfer_enable: 1,
                expiry: None,
            }
            .is_available(100)
        );
        assert!(
            !SubscriptionAvailability {
                banned: false,
                transfer_enable: 1,
                expiry: Some(100),
            }
            .is_available(100)
        );
    }
}
