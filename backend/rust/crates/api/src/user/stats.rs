use axum::{Json, extract::State, http::HeaderMap};
use chrono::{Datelike, TimeZone, Utc};
use serde::de::DeserializeOwned;
use v2board_api_contract::time::Rfc3339Timestamp;
pub(crate) use v2board_api_contract::{
    user::UserStats as UserStatsBody,
    user_activity::{
        TrafficLogView as TrafficLogBody, UserAnytlsExtra, UserHysteriaExtra, UserServerFields,
        UserServerView as ServerBody, UserShadowsocksExtra, UserTrojanExtra, UserTuicExtra,
        UserV2nodeExtra, UserVlessExtra, UserVmessExtra,
    },
};
use v2board_application::{
    account::AccountError,
    service_usage::{ServiceServer, ServiceUsageError, TrafficRecord},
};
use v2board_compat::{ApiError, Code, Problem};
use v2board_config::{app_now, app_timezone};

use crate::{auth::require_user, dialect::problem_from, locale::request_locale, runtime::AppState};

/// GET /user/stats — bare named counts (§5.3/§9.1).
pub(crate) async fn user_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserStatsBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let statistics = state
        .account_service()
        .statistics(user.id)
        .await
        .map_err(|error| match error {
            AccountError::NotFound => ApiError::from(Problem::new(Code::UserNotRegistered)),
            AccountError::TelegramUnbindFailed => {
                ApiError::from(Problem::new(Code::TelegramUnbindFailed))
            }
            AccountError::Repository(error) => ApiError::internal(error.to_string()),
        })
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(UserStatsBody {
        pending_order_count: statistics.pending_order_count,
        pending_ticket_count: statistics.pending_ticket_count,
        invited_user_count: statistics.invited_user_count,
    }))
}

/// Modern GET /user/servers row (docs/api-dialect.md §5.4, W6): boolean
/// `is_online`, numeric `rate`/`port` (§4.1), RFC 3339 `last_check_at`
/// (§4.5). The field set otherwise mirrors the db row the byte-frozen
/// subscribe pipeline (§2) keeps consuming in its legacy shape.
/// Project one legacy-typed server row onto the modern wire (§4.1). The
/// legacy `rate`/`port` columns are free-text VARCHAR, so the numeric
/// contract is enforced here: a non-numeric operator value is an internal
/// error, never a silent string passthrough.
fn server_extra<T: DeserializeOwned>(id: i32, raw: Option<&str>) -> Result<Option<T>, ApiError> {
    raw.map(|raw| {
        serde_json::from_str(raw).map_err(|error| {
            ApiError::internal(format!(
                "server {id} extra does not match its protocol DTO: {error}"
            ))
        })
    })
    .transpose()
}

pub(super) fn server_body(row: ServiceServer) -> Result<ServerBody, ApiError> {
    let kind = row.kind.clone();
    let id = row.id;
    let extra = row.extra_json;
    let fields = UserServerFields {
        id,
        parent_id: row.parent_id,
        group_id: row.group_ids,
        route_id: row.route_ids,
        name: row.name,
        rate: row.rate,
        host: row.host,
        port: row.port,
        cache_key: row.cache_key,
        last_check_at: row.last_check_at.map(Rfc3339Timestamp::from_epoch_seconds),
        is_online: row.online,
        tags: row.tags,
        sort: row.sort,
    };
    let extra = extra.as_deref();
    match kind.as_str() {
        "shadowsocks" => Ok(ServerBody::Shadowsocks {
            server: fields,
            extra: server_extra::<UserShadowsocksExtra>(id, extra)?,
        }),
        "vmess" => Ok(ServerBody::Vmess {
            server: fields,
            extra: server_extra::<UserVmessExtra>(id, extra)?,
        }),
        "trojan" => Ok(ServerBody::Trojan {
            server: fields,
            extra: server_extra::<UserTrojanExtra>(id, extra)?,
        }),
        "tuic" => Ok(ServerBody::Tuic {
            server: fields,
            extra: server_extra::<UserTuicExtra>(id, extra)?,
        }),
        "hysteria" => Ok(ServerBody::Hysteria {
            server: fields,
            extra: server_extra::<UserHysteriaExtra>(id, extra)?,
        }),
        "vless" => Ok(ServerBody::Vless {
            server: fields,
            extra: server_extra::<UserVlessExtra>(id, extra)?,
        }),
        "anytls" => Ok(ServerBody::Anytls {
            server: fields,
            extra: server_extra::<UserAnytlsExtra>(id, extra)?,
        }),
        "v2node" => Ok(ServerBody::V2node {
            server: fields,
            extra: server_extra::<UserV2nodeExtra>(id, extra)?,
        }),
        _ => Err(ApiError::internal(format!(
            "server {} has unsupported protocol {kind:?}",
            id
        ))),
    }
}

fn service_usage_error(error: ServiceUsageError) -> ApiError {
    match error {
        ServiceUsageError::UserNotRegistered => Problem::new(Code::UserNotRegistered).into(),
        ServiceUsageError::Repository(error) => ApiError::internal(error.to_string()),
    }
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
    let servers = state
        .service_usage_service()
        .servers(user.id, Utc::now().timestamp())
        .await
        .map_err(service_usage_error)
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
fn traffic_log_body(row: TrafficRecord) -> TrafficLogBody {
    TrafficLogBody {
        u: row.upload,
        d: row.download,
        record_at: Rfc3339Timestamp::from_epoch_seconds(row.recorded_at),
        user_id: row.user_id,
        server_rate: row.server_rate,
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
    let logs = state
        .service_usage_service()
        .traffic(user.id, first_day_of_month_timestamp())
        .await
        .map_err(service_usage_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(logs.into_iter().map(traffic_log_body).collect()))
}

fn first_day_of_month_timestamp() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}
