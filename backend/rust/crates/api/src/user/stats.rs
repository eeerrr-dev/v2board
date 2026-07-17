use axum::{Json, extract::State, http::HeaderMap};
use chrono::{Datelike, TimeZone, Utc};
use serde::Serialize;
use v2board_compat::{
    ApiError, Code, Problem,
    json::{rfc3339, rfc3339_option},
};
use v2board_config::{app_now, app_timezone};

use crate::{auth::require_user, dialect::problem_from, locale::request_locale, runtime::AppState};

use super::subscription::user_is_available;

/// Bare GET /user/stats body (docs/api-dialect.md §9.1, W5): the legacy
/// `[pending_orders, pending_tickets, invited_users]` tuple as a named object.
#[derive(Debug, Serialize)]
pub(crate) struct UserStatsBody {
    pub(crate) pending_order_count: i64,
    pub(crate) pending_ticket_count: i64,
    pub(crate) invited_user_count: i64,
}

/// GET /user/stats — bare named counts (§5.3/§9.1).
pub(crate) async fn user_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserStatsBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let pending_order_count = v2board_db::user::count_pending_orders(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    let pending_ticket_count = v2board_db::user::count_pending_tickets(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    let invited_user_count = v2board_db::user::count_invited_users(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    Ok(Json(UserStatsBody {
        pending_order_count,
        pending_ticket_count,
        invited_user_count,
    }))
}

/// Modern GET /user/servers row (docs/api-dialect.md §5.4, W6): boolean
/// `is_online`, numeric `rate`/`port` (§4.1), RFC 3339 `last_check_at`
/// (§4.5). The field set otherwise mirrors the db row the byte-frozen
/// subscribe pipeline (§2) keeps consuming in its legacy shape.
#[derive(Debug, Serialize)]
pub(crate) struct ServerBody {
    pub(crate) id: i32,
    pub(crate) parent_id: Option<i32>,
    pub(crate) group_id: Vec<i32>,
    pub(crate) route_id: Option<Vec<i32>>,
    pub(crate) name: String,
    pub(crate) rate: f64,
    pub(crate) r#type: String,
    pub(crate) host: String,
    pub(crate) port: i64,
    pub(crate) cache_key: String,
    #[serde(with = "rfc3339_option")]
    pub(crate) last_check_at: Option<i64>,
    pub(crate) is_online: bool,
    pub(crate) tags: Option<Vec<String>>,
    pub(crate) sort: Option<i32>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub(crate) extra: serde_json::Value,
}

/// Project one legacy-typed server row onto the modern wire (§4.1). The
/// legacy `rate`/`port` columns are free-text VARCHAR, so the numeric
/// contract is enforced here: a non-numeric operator value is an internal
/// error, never a silent string passthrough.
pub(super) fn server_body(
    row: v2board_db::server::AvailableServerRow,
) -> Result<ServerBody, ApiError> {
    let rate = row
        .rate
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|rate| rate.is_finite())
        .ok_or_else(|| {
            ApiError::internal(format!(
                "server {} rate {:?} is not numeric",
                row.id, row.rate
            ))
        })?;
    let port = match &row.port {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
    .ok_or_else(|| {
        ApiError::internal(format!(
            "server {} port {} is not numeric",
            row.id, row.port
        ))
    })?;
    Ok(ServerBody {
        id: row.id,
        parent_id: row.parent_id,
        group_id: row.group_id,
        route_id: row.route_id,
        name: row.name,
        rate,
        r#type: row.r#type,
        host: row.host,
        port,
        cache_key: row.cache_key,
        last_check_at: row.last_check_at,
        is_online: row.is_online != 0,
        tags: row.tags,
        sort: row.sort,
        extra: row.extra,
    })
}

/// GET /user/servers — bare array (§5.4, W6). An unavailable subscription
/// keeps answering an empty list (the SPA's empty-state subscribe/renew
/// routing is the Tier-1 outcome).
pub(crate) async fn user_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerBody>>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::UserNotRegistered, locale))?;
    if !user_is_available(&access) {
        return Ok(Json(Vec::new()));
    }
    let mut servers = v2board_db::server::fetch_available_servers(&state.db, access.group_id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    crate::server_api::hydrate_online_status(&state, &mut servers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let servers = servers
        .into_iter()
        .map(server_body)
        .collect::<Result<Vec<_>, ApiError>>()
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(servers))
}

/// Modern GET /user/traffic-logs row (docs/api-dialect.md §5.4, W6): numeric
/// `server_rate` (§4.1; the NUMERIC column serialized as a JSON number) and
/// RFC 3339 `record_at` (§4.5 — still the period-start marker).
#[derive(Debug, Serialize)]
pub(crate) struct TrafficLogBody {
    pub(crate) u: i64,
    pub(crate) d: i64,
    #[serde(with = "rfc3339")]
    pub(crate) record_at: i64,
    pub(crate) user_id: i64,
    pub(crate) server_rate: f64,
}

impl From<v2board_db::stat::TrafficLogRow> for TrafficLogBody {
    fn from(row: v2board_db::stat::TrafficLogRow) -> Self {
        Self {
            u: row.u,
            d: row.d,
            record_at: row.record_at,
            user_id: row.user_id,
            server_rate: row.server_rate,
        }
    }
}

/// GET /user/traffic-logs — bare array (§5.4, W6), current month, newest
/// first (the legacy window is unchanged).
pub(crate) async fn user_traffic_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TrafficLogBody>>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let logs =
        v2board_db::stat::fetch_traffic_logs(&state.db, user.id, first_day_of_month_timestamp())
            .await
            .map_err(|error| problem_from(error.into(), locale))?;
    Ok(Json(logs.into_iter().map(TrafficLogBody::from).collect()))
}

fn first_day_of_month_timestamp() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}
