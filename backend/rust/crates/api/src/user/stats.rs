use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use chrono::{Datelike, TimeZone, Utc};
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};
use v2board_config::{app_now, app_timezone};

use crate::{
    auth::{AuthQuery, require_user},
    runtime::AppState,
};

use super::subscription::user_is_available;

pub(crate) async fn user_stat(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<[i64; 3]>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let pending_orders = v2board_db::user::count_pending_orders(&state.db, user.id).await?;
    let pending_tickets = v2board_db::user::count_pending_tickets(&state.db, user.id).await?;
    let invited_users = v2board_db::user::count_invited_users(&state.db, user.id).await?;
    Ok(legacy_data([
        pending_orders,
        pending_tickets,
        invited_users,
    ]))
}

pub(crate) async fn server_fetch(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::server::AvailableServerRow>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    if !user_is_available(&access) {
        return Ok(legacy_data(Vec::new()));
    }
    let mut servers =
        v2board_db::server::fetch_available_servers(&state.db, access.group_id).await?;
    crate::server_api::hydrate_online_status(&state.redis, &mut servers).await?;
    Ok(legacy_data(servers))
}

pub(crate) async fn user_traffic_logs(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<v2board_db::stat::TrafficLogRow>>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
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
