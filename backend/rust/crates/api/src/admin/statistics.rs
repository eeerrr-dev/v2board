use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::Value;
use v2board_compat::{Code, Page, Pagination, Problem, page};

use crate::{dialect::problem_from, locale::request_locale, runtime::AppState};

/// §8 default for `GET stats/user-traffic` (the legacy `getStatUser`
/// pageSize floor, 10).
const STAT_USER_TRAFFIC_DEFAULT_PER_PAGE: i64 = 10;

/// GET `stats/summary` (§6.8): bare object — the three legacy aliases
/// collapsed into one route; money fields are integer cents.
pub(super) async fn stats_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .stats_summary()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

#[derive(Deserialize)]
pub(super) struct StatsWindowQuery {
    window: Option<String>,
}

/// §6.8 `?window=today|previous`: the two legacy today/last routes collapse
/// into one; the selector is required and closed.
fn stats_window_today(query: &StatsWindowQuery) -> Result<bool, Problem> {
    match query.window.as_deref() {
        Some("today") => Ok(true),
        Some("previous") => Ok(false),
        _ => {
            Err(Problem::new(Code::ValidationFailed)
                .with_detail("window must be today or previous"))
        }
    }
}

/// GET `stats/server-rank` `?window=today|previous` (§6.8): bare array.
pub(super) async fn stats_server_rank(
    State(state): State<AppState>,
    Query(query): Query<StatsWindowQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, Problem> {
    let locale = request_locale(&headers);
    let today = stats_window_today(&query)?;
    state
        .admin_service(state.config_snapshot())
        .stats_server_rank(today)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `stats/user-rank` `?window=today|previous` (§6.8): bare array.
pub(super) async fn stats_user_rank(
    State(state): State<AppState>,
    Query(query): Query<StatsWindowQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, Problem> {
    let locale = request_locale(&headers);
    let today = stats_window_today(&query)?;
    state
        .admin_service(state.config_snapshot())
        .stats_user_rank(today)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `stats/orders` (§6.8): bare array of `{series, date, value}` rows
/// with the snake_case series slugs and integer-cent money.
pub(super) async fn stats_orders(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .stats_orders()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

#[derive(Deserialize)]
pub(super) struct StatsUserTrafficQuery {
    user_id: Option<i64>,
    page: Option<i64>,
    per_page: Option<i64>,
}

/// GET `stats/user-traffic` `?user_id=&page=&per_page=` (§6.8): §8 page;
/// `server_rate` crosses as a number.
pub(super) async fn stats_user_traffic(
    State(state): State<AppState>,
    Query(query): Query<StatsUserTrafficQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    let user_id = query
        .user_id
        .ok_or_else(|| Problem::new(Code::ValidationFailed).with_detail("user_id is required"))?;
    let pagination = Pagination::resolve(
        query.page,
        query.per_page,
        STAT_USER_TRAFFIC_DEFAULT_PER_PAGE,
    )?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .stats_user_traffic(user_id, pagination)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

#[derive(Deserialize)]
pub(super) struct StatsRecordsQuery {
    #[serde(rename = "type")]
    record_type: Option<String>,
}

/// GET `stats/records` `?type=` (§6.8): bare `{series, date, value}` array
/// over the `d`/`m` stat buckets (`d` when omitted, the legacy default).
pub(super) async fn stats_records(
    State(state): State<AppState>,
    Query(query): Query<StatsRecordsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, Problem> {
    let locale = request_locale(&headers);
    let record_type = match query.record_type.as_deref() {
        None | Some("d") => "d",
        Some("m") => "m",
        Some(_) => {
            return Err(Problem::new(Code::ValidationFailed).with_detail("type must be d or m"));
        }
    };
    state
        .admin_service(state.config_snapshot())
        .stats_records(record_type)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}
