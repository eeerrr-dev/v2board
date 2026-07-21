//! Invite & commission family — modern dialect (docs/api-dialect.md §5.6,
//! the §5.3 `/user/commission-transfers` row, §8, §9.2, Appendix A §W7):
//! bare success bodies on modern value types, the §9.2 named invite stat
//! object, `page`/`per_page` pagination, the one deliberate 204-no-body
//! create (`POST /user/invite-codes`), and problem+json failures.

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
use v2board_api_contract::{
    Page,
    time::Rfc3339Timestamp,
    user::CommissionTransferRequest,
    user_activity::{CommissionView, InviteCodeView, InviteStatView, InviteView},
};
use v2board_application::invite::{
    CommissionEntry, CommissionTransferPolicy, InviteCode, InviteError, InviteOverview,
    InviteStatistics,
};
use v2board_compat::{ApiError, Code, Pagination, Problem};

use crate::{
    auth::require_user, dialect::DialectJson, dialect::problem_from, locale::request_locale,
    runtime::AppState,
};

/// §8: `/user/commissions` keeps the legacy default page size.
const COMMISSIONS_DEFAULT_PER_PAGE: i64 = 10;

fn invite_error(error: InviteError) -> ApiError {
    match error {
        InviteError::TransferAmountInvalid => Problem::new(Code::TransferAmountInvalid).into(),
        InviteError::UserNotRegistered => Problem::new(Code::UserNotRegistered).into(),
        InviteError::InsufficientCommissionBalance => {
            Problem::new(Code::InsufficientCommissionBalance).into()
        }
        InviteError::BalanceOutOfRange => Problem::new(Code::BalanceOutOfRange).into(),
        InviteError::CommissionAmountOutOfRange => Problem::new(Code::PaymentAmountOutOfRange)
            .with_detail("Order amount is outside the supported range")
            .into(),
        InviteError::InviteCodeLimitReached => Problem::new(Code::InviteCodeLimitReached).into(),
        InviteError::Repository(error) => ApiError::internal(error.to_string()),
    }
}

/// POST /user/commission-transfers — 204 on success (§5.3, W7).
pub(crate) async fn commission_transfer_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<CommissionTransferRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let config = state.config_snapshot();
    state
        .invite_service()
        .transfer_commission(
            user.id,
            payload.transfer_amount,
            CommissionTransferPolicy {
                first_purchase_only: config.commission_first_time_enable,
                default_commission_rate: config.invite_commission,
            },
            Utc::now().timestamp(),
        )
        .await
        .map_err(invite_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
pub(super) fn checked_transfer_balances(
    commission_balance: i32,
    balance: i32,
    transfer_amount: i32,
) -> Result<(i32, i32), ApiError> {
    v2board_application::invite::checked_transfer_balances(
        commission_balance,
        balance,
        transfer_amount,
    )
    .map_err(invite_error)
}

/// POST /user/invite-codes — the one deliberate 204-no-body create (§1,
/// §5.6): invite codes are never individually addressed afterwards, so the
/// SPA refetches `GET /user/invite` instead of consuming a created id.
pub(crate) async fn invite_code_create(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    state
        .invite_service()
        .create_invite_code(
            user.id,
            state.config_snapshot().invite_gen_limit,
            Utc::now().timestamp(),
        )
        .await
        .map_err(invite_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Bare GET /user/invite body (§5.6): the active invite codes on modern
/// value types plus the §9.2 named stat object.
/// One active invite code (§5.6): RFC 3339 timestamps (§4.5). The legacy
/// `user_id` (the authenticated caller) and `status` (constant 0 — the
/// endpoint only returns active codes) columns carried no information and
/// are dropped from the modern body.
fn invite_code_view(row: InviteCode) -> InviteCodeView {
    InviteCodeView {
        id: row.id,
        code: row.code,
        pv: row.views,
        created_at: Rfc3339Timestamp::from_epoch_seconds(row.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(row.updated_at),
    }
}

/// §9.2: the legacy stat 5-tuple as a named object. Commissions stay
/// integer cents; `commission_rate` stays an integer percent (default 10
/// when unset).
fn invite_stat_view(stat: InviteStatistics) -> InviteStatView {
    InviteStatView {
        registered_count: stat.registered_count,
        valid_commission: stat.valid_commission,
        pending_commission: stat.pending_commission,
        commission_rate: stat.commission_rate,
        available_commission: stat.available_commission,
    }
}

fn invite_view(row: InviteOverview) -> InviteView {
    InviteView {
        codes: row.codes.into_iter().map(invite_code_view).collect(),
        stat: invite_stat_view(row.statistics),
    }
}

/// GET /user/invite — bare `{codes, stat}` (§5.6).
pub(crate) async fn invite_get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InviteView>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let data = state
        .invite_service()
        .overview(user.id)
        .await
        .map_err(invite_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(invite_view(data)))
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommissionsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

/// One settled commission entry (§5.6). Money stays integer cents — the
/// api-client `amount/100` display conversion stays at its boundary.
fn commission_view(row: CommissionEntry) -> CommissionView {
    CommissionView {
        id: row.id,
        trade_no: row.trade_no,
        order_amount: row.order_amount,
        get_amount: row.amount,
        created_at: Rfc3339Timestamp::from_epoch_seconds(row.created_at),
    }
}

/// GET /user/commissions?page=&per_page= — the §8 `{items, total}` page
/// envelope (was `/user/invite/details` with `current`/`page_size`).
pub(crate) async fn commissions_list(
    State(state): State<AppState>,
    Query(query): Query<CommissionsQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<CommissionView>>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let pagination = Pagination::resolve(query.page, query.per_page, COMMISSIONS_DEFAULT_PER_PAGE)?;
    let page = state
        .invite_service()
        .commissions(user.id, pagination.limit(), pagination.offset())
        .await
        .map_err(invite_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(Page::new(
        page.items.into_iter().map(commission_view).collect(),
        page.total,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/api-dialect.md §5.6/§9.2 (W7): the invite body is the bare
    /// `{codes, stat}` object — RFC 3339 code timestamps, the named stat
    /// object, and integer-cents commissions.
    #[test]
    fn invite_body_serializes_the_named_stat_object() {
        let body = invite_view(InviteOverview {
            codes: vec![InviteCode {
                id: 1,
                code: "goldinv1".to_string(),
                views: 3,
                created_at: 1_700_000_000,
                updated_at: 1_700_000_000,
            }],
            statistics: InviteStatistics {
                registered_count: 12,
                valid_commission: 12_300,
                pending_commission: 4_500,
                commission_rate: 10,
                available_commission: 8_000,
            },
        });
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({
                "codes": [{
                    "id": 1,
                    "code": "goldinv1",
                    "pv": 3,
                    "created_at": "2023-11-14T22:13:20Z",
                    "updated_at": "2023-11-14T22:13:20Z",
                }],
                "stat": {
                    "registered_count": 12,
                    "valid_commission": 12_300,
                    "pending_commission": 4_500,
                    "commission_rate": 10,
                    "available_commission": 8_000,
                },
            })
        );
    }

    /// docs/api-dialect.md §5.6/§8 (W7): commissions ship the `{items,
    /// total}` page envelope with RFC 3339 timestamps and cents amounts.
    #[test]
    fn commissions_page_envelope_serializes_modern_value_types() {
        let item = commission_view(CommissionEntry {
            id: 7,
            trade_no: "trade-0007".to_string(),
            order_amount: 1_000,
            amount: 100,
            created_at: 1_700_000_000,
        });
        let envelope = Page::new(vec![item], 42);
        assert_eq!(
            serde_json::to_value(&envelope).unwrap(),
            serde_json::json!({
                "items": [{
                    "id": 7,
                    "trade_no": "trade-0007",
                    "order_amount": 1_000,
                    "get_amount": 100,
                    "created_at": "2023-11-14T22:13:20Z",
                }],
                "total": 42,
            })
        );
    }

    /// The transfer rejections carry the §3.4 registry codes.
    #[test]
    fn transfer_rejections_map_to_registry_codes() {
        let problem_code = |error: ApiError| match error {
            ApiError::Problem(problem) => problem.code(),
            other => panic!("expected a problem, got {other:?}"),
        };
        assert_eq!(
            problem_code(checked_transfer_balances(100, 200, 0).unwrap_err()),
            Code::TransferAmountInvalid
        );
        assert_eq!(
            problem_code(checked_transfer_balances(10, 200, 25).unwrap_err()),
            Code::InsufficientCommissionBalance
        );
        assert_eq!(
            problem_code(checked_transfer_balances(100, i32::MAX, 1).unwrap_err()),
            Code::BalanceOutOfRange
        );
    }
}
