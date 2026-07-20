//! Overridable time source for deterministic calendar-boundary tests.
//!
//! Production always reads the process clock. Tests freeze the clock on their
//! own thread with [`freeze_time`] to pin business logic to an exact instant —
//! month-end traffic-reset clamping, Asia/Shanghai day boundaries, renewal
//! windows — instead of asserting only what happens to be true on the day the
//! suite runs.

use std::cell::Cell;

use chrono::{DateTime, Utc};

thread_local! {
    static FROZEN_NOW: Cell<Option<DateTime<Utc>>> = const { Cell::new(None) };
}

/// Current UTC instant from the process clock, unless the calling thread froze
/// it with [`freeze_time`]. Business code reads time through this (usually via
/// [`crate::app_now`]) rather than `Utc::now()` when its behavior depends on
/// calendar boundaries.
pub fn now_utc() -> DateTime<Utc> {
    FROZEN_NOW.with(Cell::get).unwrap_or_else(Utc::now)
}

/// Freezes [`now_utc`] on the current thread until the returned guard drops.
/// Test-only by convention. The override is thread-local: it composes with
/// parallel test threads and the single-threaded `#[tokio::test]` runtime,
/// but does not cross `tokio::spawn` or `std::thread::spawn` boundaries.
#[must_use = "the clock unfreezes as soon as the guard drops"]
pub fn freeze_time(at: DateTime<Utc>) -> FrozenTimeGuard {
    FrozenTimeGuard {
        previous: FROZEN_NOW.with(|cell| cell.replace(Some(at))),
    }
}

/// Restores the previous clock state (nested freezes restore the outer freeze)
/// when dropped.
pub struct FrozenTimeGuard {
    previous: Option<DateTime<Utc>>,
}

impl Drop for FrozenTimeGuard {
    fn drop(&mut self) {
        FROZEN_NOW.with(|cell| cell.set(self.previous));
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn frozen_clock_pins_nests_and_restores() {
        let outer = Utc.with_ymd_and_hms(2026, 2, 28, 12, 0, 0).unwrap();
        let inner = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 59).unwrap();
        {
            let _outer_guard = freeze_time(outer);
            assert_eq!(now_utc(), outer);
            assert_eq!(now_utc(), outer, "a frozen clock does not advance");
            {
                let _inner_guard = freeze_time(inner);
                assert_eq!(now_utc(), inner);
            }
            assert_eq!(
                now_utc(),
                outer,
                "dropping a nested freeze restores the outer one"
            );
        }
        let live = now_utc();
        assert_ne!(
            live, outer,
            "dropping the last guard resumes the process clock"
        );
        assert!(live.timestamp() > outer.timestamp());
    }
}
