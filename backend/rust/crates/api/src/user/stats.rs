use axum::{Json, extract::State, http::HeaderMap};
use chrono::{Datelike, TimeZone, Utc};
use serde::Serialize;
use v2board_compat::{ApiError, LegacyEnvelope, Problem, legacy_data};
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

pub(crate) async fn server_fetch(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::server::AvailableServerRow>>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::business("The user does not exist"))?;
    if !user_is_available(&access) {
        return Ok(legacy_data(Vec::new()));
    }
    let mut servers =
        v2board_db::server::fetch_available_servers(&state.db, access.group_id).await?;
    crate::server_api::hydrate_online_status(&state, &mut servers).await?;
    Ok(legacy_data(servers))
}

pub(crate) async fn user_traffic_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::stat::TrafficLogRow>>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let logs =
        v2board_db::stat::fetch_traffic_logs(&state.db, user.id, first_day_of_month_timestamp())
            .await?;
    Ok(legacy_data(logs))
}

fn first_day_of_month_timestamp() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}
