//! Process-local per-IP request limiter for the unauthenticated internal
//! surfaces (the `auth` and `public` families).
//!
//! The §2 frozen external namespaces — client subscriptions, server/node
//! polling, guest payment notify, and the Telegram webhook — are exempt by
//! design: the WAF must not challenge them (AGENTS.md), and this limiter must
//! not either. Authenticated user/admin traffic keeps its per-function Redis
//! limits (registration, password attempts, email sends). Cloudflare owns
//! volumetric DDoS at the edge; this layer is defense-in-depth against
//! single-address credential stuffing and enumeration, so a full table
//! fails open rather than taking the site down.

use std::{collections::HashMap, net::IpAddr, sync::Mutex};

use axum::{
    extract::{Request, State},
    http::{HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use v2board_compat::{ApiError, Code, Problem};

use crate::{
    metrics::{RouteFamily, classify_request_path},
    runtime::AppState,
};

const DEFAULT_LIMIT_PER_MINUTE: u32 = 120;
const DEFAULT_MAX_TRACKED_CLIENTS: usize = 65_536;
const WINDOW_SECONDS: i64 = 60;

/// Fixed-window per-IP counter. One window per client; the table prunes
/// expired windows when it fills and fails open past its hard cap.
#[derive(Debug)]
pub(crate) struct HttpRateLimiter {
    limit_per_minute: u32,
    max_tracked_clients: usize,
    windows: Mutex<HashMap<IpAddr, WindowCounter>>,
}

#[derive(Debug, Clone, Copy)]
struct WindowCounter {
    window: i64,
    count: u32,
}

impl HttpRateLimiter {
    /// `V2BOARD_HTTP_RATE_LIMIT_PER_MINUTE` overrides the default; `0`
    /// disables the limiter entirely.
    pub(crate) fn from_env() -> Self {
        let limit = match std::env::var("V2BOARD_HTTP_RATE_LIMIT_PER_MINUTE") {
            Ok(raw) => match raw.trim().parse::<u32>() {
                Ok(limit) => limit,
                Err(_) => {
                    tracing::warn!(
                        raw,
                        "V2BOARD_HTTP_RATE_LIMIT_PER_MINUTE is not an unsigned integer; keeping the default"
                    );
                    DEFAULT_LIMIT_PER_MINUTE
                }
            },
            Err(_) => DEFAULT_LIMIT_PER_MINUTE,
        };
        Self::bounded(limit, DEFAULT_MAX_TRACKED_CLIENTS)
    }

    fn bounded(limit_per_minute: u32, max_tracked_clients: usize) -> Self {
        Self {
            limit_per_minute,
            max_tracked_clients,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Counts the request against its client window and returns whether it is
    /// admitted.
    fn admit(&self, client: IpAddr, unix_time: i64) -> bool {
        if self.limit_per_minute == 0 {
            return true;
        }
        let window = unix_time.div_euclid(WINDOW_SECONDS);
        let Ok(mut windows) = self.windows.lock() else {
            return true;
        };
        if windows.len() >= self.max_tracked_clients && !windows.contains_key(&client) {
            windows.retain(|_, counter| counter.window == window);
            if windows.len() >= self.max_tracked_clients {
                tracing::warn!(
                    tracked = windows.len(),
                    "HTTP rate-limit table is full of live windows; admitting without counting"
                );
                return true;
            }
        }
        let counter = windows
            .entry(client)
            .or_insert(WindowCounter { window, count: 0 });
        if counter.window != window {
            *counter = WindowCounter { window, count: 0 };
        }
        counter.count = counter.count.saturating_add(1);
        counter.count <= self.limit_per_minute
    }
}

/// Only the unauthenticated internal families are limited; every §2 frozen
/// external namespace and the authenticated surfaces stay exempt.
fn family_is_rate_limited(family: RouteFamily) -> bool {
    matches!(family, RouteFamily::Auth | RouteFamily::Public)
}

pub(crate) async fn http_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let config = state.config_snapshot();
    let family = classify_request_path(request.uri().path(), &config.admin_path());
    if !family_is_rate_limited(family) {
        return next.run(request).await;
    }
    // Requests without a resolved client identity (loopback probes in local
    // mode) are not counted rather than sharing one synthetic bucket.
    let Some(client_ip) = request
        .extensions()
        .get::<crate::runtime::ClientIp>()
        .map(|client_ip| client_ip.0)
    else {
        return next.run(request).await;
    };
    let unix_time = Utc::now().timestamp();
    if state.http_rate_limiter.admit(client_ip, unix_time) {
        return next.run(request).await;
    }
    let mut response = ApiError::from(Problem::new(Code::RateLimited)).into_response();
    let retry_after_seconds = WINDOW_SECONDS - unix_time.rem_euclid(WINDOW_SECONDS);
    if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(last_octet: u8) -> IpAddr {
        IpAddr::from([203, 0, 113, last_octet])
    }

    #[test]
    fn requests_are_admitted_up_to_the_window_limit() {
        let limiter = HttpRateLimiter::bounded(3, 16);
        assert!(limiter.admit(client(1), 1_700_000_000));
        assert!(limiter.admit(client(1), 1_700_000_001));
        assert!(limiter.admit(client(1), 1_700_000_002));
        assert!(!limiter.admit(client(1), 1_700_000_003));
        assert!(
            limiter.admit(client(2), 1_700_000_003),
            "limits are per client address"
        );
    }

    #[test]
    fn the_window_resets_after_a_minute() {
        // 1_699_999_980 is an exact window boundary (divisible by 60).
        let limiter = HttpRateLimiter::bounded(1, 16);
        assert!(limiter.admit(client(1), 1_699_999_980));
        assert!(!limiter.admit(client(1), 1_700_000_039));
        assert!(limiter.admit(client(1), 1_700_000_040));
    }

    #[test]
    fn zero_disables_the_limiter() {
        let limiter = HttpRateLimiter::bounded(0, 16);
        for _ in 0..100 {
            assert!(limiter.admit(client(1), 1_700_000_000));
        }
    }

    #[test]
    fn a_full_table_prunes_expired_windows_then_fails_open() {
        let limiter = HttpRateLimiter::bounded(1, 4);
        for last_octet in 1..=4 {
            assert!(limiter.admit(client(last_octet), 1_700_000_000));
        }
        // Same window, table full of live counters: the untracked client is
        // admitted without counting rather than evicting a live window.
        assert!(limiter.admit(client(5), 1_700_000_001));
        assert!(limiter.admit(client(5), 1_700_000_002));
        // Next window: the stale counters prune, so new clients count again.
        assert!(limiter.admit(client(6), 1_700_000_060));
        assert!(!limiter.admit(client(6), 1_700_000_061));
    }

    #[test]
    fn only_the_unauthenticated_internal_families_are_limited() {
        for family in [RouteFamily::Auth, RouteFamily::Public] {
            assert!(family_is_rate_limited(family), "{family:?}");
        }
        for family in [
            RouteFamily::User,
            RouteFamily::Admin,
            RouteFamily::Staff,
            RouteFamily::Server,
            RouteFamily::Client,
            RouteFamily::Guest,
            RouteFamily::Assets,
            RouteFamily::Internal,
            RouteFamily::Other,
        ] {
            assert!(!family_is_rate_limited(family), "{family:?}");
        }
    }
}
