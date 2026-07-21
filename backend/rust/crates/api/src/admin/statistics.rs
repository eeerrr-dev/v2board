use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use v2board_api_contract::{
    Page,
    admin_platform::{
        AdminStatSummaryView, AdminUserTrafficView, ServerRankView, StatSeriesPointView,
        UserRankView,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::statistics::{StatisticsBucket, StatisticsSummary, StatisticsWindow};
use v2board_compat::{ApiError, Code, Pagination, Problem};

use crate::{dialect::problem_from, locale::request_locale, runtime::AppState};

fn statistics_error(error: v2board_application::RepositoryError) -> ApiError {
    ApiError::internal(error.to_string())
}

fn summary_body(summary: StatisticsSummary) -> AdminStatSummaryView {
    AdminStatSummaryView {
        online_user: summary.online_user,
        month_income: summary.month_income,
        month_register_total: summary.month_register_total,
        day_register_total: summary.day_register_total,
        ticket_pending_total: summary.ticket_pending_total,
        commission_pending_total: summary.commission_pending_total,
        payment_reconciliation_pending_total: summary.payment_reconciliation_pending_total,
        payment_reconciliation_pending_amount: summary.payment_reconciliation_pending_amount,
        day_income: summary.day_income,
        last_month_income: summary.last_month_income,
        commission_month_payout: summary.commission_month_payout,
        commission_last_month_payout: summary.commission_last_month_payout,
    }
}

/// §8 default for `GET stats/user-traffic` (the legacy `getStatUser`
/// pageSize floor, 10).
const STAT_USER_TRAFFIC_DEFAULT_PER_PAGE: i64 = 10;

/// GET `stats/summary` (§6.8): bare object — the three legacy aliases
/// collapsed into one route; money fields are integer cents.
pub(super) async fn stats_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminStatSummaryView>, Problem> {
    let locale = request_locale(&headers);
    let summary = state
        .statistics_service()
        .summary()
        .await
        .map_err(statistics_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(summary_body(summary)))
}

#[derive(Deserialize)]
pub(super) struct StatsWindowQuery {
    window: Option<String>,
}

/// §6.8 `?window=today|previous`: the two legacy today/last routes collapse
/// into one; the selector is required and closed.
fn stats_window(query: &StatsWindowQuery) -> Result<StatisticsWindow, Problem> {
    match query.window.as_deref() {
        Some("today") => Ok(StatisticsWindow::Today),
        Some("previous") => Ok(StatisticsWindow::Previous),
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
) -> Result<Json<Vec<ServerRankView>>, Problem> {
    let locale = request_locale(&headers);
    let window = stats_window(&query)?;
    let values = state
        .statistics_service()
        .server_rank(window)
        .await
        .map_err(statistics_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(
        values
            .into_iter()
            .map(|value| ServerRankView {
                server_id: value.server_id,
                server_type: value.server_type,
                server_name: value.server_name,
                u: value.upload,
                d: value.download,
                total: value.total_gib,
            })
            .collect(),
    ))
}

/// GET `stats/user-rank` `?window=today|previous` (§6.8): bare array.
pub(super) async fn stats_user_rank(
    State(state): State<AppState>,
    Query(query): Query<StatsWindowQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserRankView>>, Problem> {
    let locale = request_locale(&headers);
    let window = stats_window(&query)?;
    let values = state
        .statistics_service()
        .user_rank(window)
        .await
        .map_err(statistics_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(
        values
            .into_iter()
            .map(|value| UserRankView {
                user_id: value.user_id,
                email: value.email,
                u: value.upload,
                d: value.download,
                total: value.total_gib,
            })
            .collect(),
    ))
}

/// GET `stats/orders` (§6.8): bare array of `{series, date, value}` rows
/// with the snake_case series slugs and integer-cent money.
pub(super) async fn stats_orders(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<StatSeriesPointView>>, Problem> {
    let locale = request_locale(&headers);
    let values = state
        .statistics_service()
        .series(StatisticsBucket::Daily)
        .await
        .map_err(statistics_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(
        values
            .into_iter()
            .map(|value| StatSeriesPointView {
                series: value.series.to_string(),
                date: value.date,
                value: value.value,
            })
            .collect(),
    ))
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
) -> Result<Json<Page<AdminUserTrafficView>>, Problem> {
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
        .statistics_service()
        .user_traffic(user_id, pagination.limit(), pagination.offset())
        .await
        .map_err(statistics_error)
        .map_err(|error| problem_from(error, locale))?;
    let items = items
        .into_iter()
        .map(|value| AdminUserTrafficView {
            record_at: Rfc3339Timestamp::from_epoch_seconds(value.recorded_at),
            u: value.upload,
            d: value.download,
            server_rate: value.server_rate,
        })
        .collect();
    Ok(Json(Page::new(items, total)))
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
) -> Result<Json<Vec<StatSeriesPointView>>, Problem> {
    let locale = request_locale(&headers);
    let bucket = match query.record_type.as_deref() {
        None | Some("d") => StatisticsBucket::Daily,
        Some("m") => StatisticsBucket::Monthly,
        Some(_) => {
            return Err(Problem::new(Code::ValidationFailed).with_detail("type must be d or m"));
        }
    };
    let values = state
        .statistics_service()
        .series(bucket)
        .await
        .map_err(statistics_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(
        values
            .into_iter()
            .map(|value| StatSeriesPointView {
                series: value.series.to_string(),
                date: value.date,
                value: value.value,
            })
            .collect(),
    ))
}
