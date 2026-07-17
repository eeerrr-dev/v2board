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
use serde::{Deserialize, Serialize};
use v2board_compat::{ApiError, Code, Page, Pagination, Problem, json::rfc3339, page};

use crate::{
    auth::require_user, dialect::DialectJson, dialect::problem_from, locale::request_locale,
    runtime::AppState,
};

/// §8: `/user/commissions` keeps the legacy default page size.
const COMMISSIONS_DEFAULT_PER_PAGE: i64 = 10;

/// POST /user/commission-transfers request (§5.3): integer cents; the
/// api-client `100*amount` conversion stays at its boundary.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommissionTransferRequest {
    transfer_amount: i32,
}

#[derive(sqlx::FromRow)]
struct TransferUserRow {
    commission_balance: i32,
    balance: i32,
    invite_user_id: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct TransferInviterRow {
    commission_type: i16,
    commission_rate: Option<i32>,
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
    transfer(&state, user.id, payload.transfer_amount)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn transfer(state: &AppState, user_id: i64, transfer_amount: i32) -> Result<(), ApiError> {
    // UserTransfer FormRequest: transfer_amount required|integer|min:1 →
    // the dedicated §3.4 `transfer_amount_invalid` rejection.
    if transfer_amount <= 0 {
        return Err(Problem::new(Code::TransferAmountInvalid).into());
    }
    let config = state.config_snapshot();
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    let current = sqlx::query_as::<_, TransferUserRow>(
        "SELECT commission_balance, balance, invite_user_id FROM users WHERE id = $1 LIMIT 1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))?;
    let (commission_balance, balance) =
        checked_transfer_balances(current.commission_balance, current.balance, transfer_amount)?;
    sqlx::query(
        "UPDATE users SET commission_balance = $1, balance = $2, updated_at = $3 WHERE id = $4",
    )
    .bind(commission_balance)
    .bind(balance)
    .bind(now)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    // OrderService::setInvite for this deposit order. Laravel only zeroes total_amount
    // AFTER setInvite runs, so the order carries the user's invite_user_id and the
    // inviter's commission is computed against the pre-zero transfer amount.
    let mut order_commission_balance = 0i32;
    if let Some(invite_user_id) = current.invite_user_id {
        let inviter = sqlx::query_as::<_, TransferInviterRow>(
            "SELECT commission_type, commission_rate FROM users WHERE id = $1 LIMIT 1",
        )
        .bind(invite_user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(inviter) = inviter {
            let has_valid_order: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM orders WHERE user_id = $1 AND status NOT IN (0, 2)",
            )
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
            let is_commission = match inviter.commission_type {
                0 => !config.commission_first_time_enable || has_valid_order == 0,
                1 => true,
                2 => has_valid_order == 0,
                _ => false,
            };
            if is_commission {
                order_commission_balance = v2board_domain::order::commission_amount_cents(
                    i64::from(transfer_amount),
                    inviter.commission_rate,
                    config.invite_commission,
                )?;
            }
        }
    }

    sqlx::query(
        r#"
        INSERT INTO orders (
            user_id, invite_user_id, plan_id, period, trade_no, total_amount, surplus_amount,
            "type", status, callback_no, commission_status, commission_balance, created_at, updated_at
        )
        VALUES ($1, $2, 0, 'deposit', $3, 0, $4, 9, 3, '佣金划转 Commission transfer', 0, $5, $6, $7)
        "#,
    )
    .bind(user_id)
    .bind(current.invite_user_id)
    .bind(v2board_domain::order::generate_order_no())
    .bind(transfer_amount)
    .bind(order_commission_balance)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(super) fn checked_transfer_balances(
    commission_balance: i32,
    balance: i32,
    transfer_amount: i32,
) -> Result<(i32, i32), ApiError> {
    if transfer_amount <= 0 {
        return Err(Problem::new(Code::TransferAmountInvalid).into());
    }
    let commission_balance = commission_balance
        .checked_sub(transfer_amount)
        .filter(|balance| *balance >= 0)
        .ok_or_else(|| ApiError::from(Problem::new(Code::InsufficientCommissionBalance)))?;
    let balance = balance
        .checked_add(transfer_amount)
        .ok_or_else(|| ApiError::from(Problem::new(Code::BalanceOutOfRange)))?;
    Ok((commission_balance, balance))
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
    let created = v2board_db::invite::create_invite_code(
        &state.db,
        user.id,
        state.config_snapshot().invite_gen_limit,
        Utc::now().timestamp(),
    )
    .await
    .map_err(|error| problem_from(error.into(), locale))?;
    if !created {
        return Err(problem_from(
            Problem::new(Code::InviteCodeLimitReached).into(),
            locale,
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Bare GET /user/invite body (§5.6): the active invite codes on modern
/// value types plus the §9.2 named stat object.
#[derive(Debug, Serialize)]
pub(crate) struct InviteBody {
    pub(crate) codes: Vec<InviteCodeBody>,
    pub(crate) stat: InviteStatBody,
}

/// One active invite code (§5.6): RFC 3339 timestamps (§4.5). The legacy
/// `user_id` (the authenticated caller) and `status` (constant 0 — the
/// endpoint only returns active codes) columns carried no information and
/// are dropped from the modern body.
#[derive(Debug, Serialize)]
pub(crate) struct InviteCodeBody {
    pub(crate) id: i32,
    pub(crate) code: String,
    pub(crate) pv: i32,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

impl From<v2board_db::invite::InviteCodeRow> for InviteCodeBody {
    fn from(row: v2board_db::invite::InviteCodeRow) -> Self {
        Self {
            id: row.id,
            code: row.code,
            pv: row.pv,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// §9.2: the legacy stat 5-tuple as a named object. Commissions stay
/// integer cents; `commission_rate` stays an integer percent (default 10
/// when unset).
#[derive(Debug, Serialize)]
pub(crate) struct InviteStatBody {
    pub(crate) registered_count: i64,
    pub(crate) valid_commission: i64,
    pub(crate) pending_commission: i64,
    pub(crate) commission_rate: i64,
    pub(crate) available_commission: i64,
}

impl From<v2board_db::invite::InviteStat> for InviteStatBody {
    fn from(stat: v2board_db::invite::InviteStat) -> Self {
        Self {
            registered_count: stat.registered_count,
            valid_commission: stat.valid_commission,
            pending_commission: stat.pending_commission,
            commission_rate: stat.commission_rate,
            available_commission: stat.available_commission,
        }
    }
}

impl From<v2board_db::invite::InviteFetchRow> for InviteBody {
    fn from(row: v2board_db::invite::InviteFetchRow) -> Self {
        Self {
            codes: row.codes.into_iter().map(InviteCodeBody::from).collect(),
            stat: InviteStatBody::from(row.stat),
        }
    }
}

/// GET /user/invite — bare `{codes, stat}` (§5.6).
pub(crate) async fn invite_get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InviteBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let data = v2board_db::invite::fetch_invite(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    Ok(Json(InviteBody::from(data)))
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommissionsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

/// One settled commission entry (§5.6). Money stays integer cents — the
/// api-client `amount/100` display conversion stays at its boundary.
#[derive(Debug, Serialize)]
pub(crate) struct CommissionItem {
    pub(crate) id: i64,
    pub(crate) trade_no: String,
    pub(crate) order_amount: i32,
    pub(crate) get_amount: i32,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
}

impl From<v2board_db::invite::CommissionDetailRow> for CommissionItem {
    fn from(row: v2board_db::invite::CommissionDetailRow) -> Self {
        Self {
            id: row.id,
            trade_no: row.trade_no,
            order_amount: row.order_amount,
            get_amount: row.get_amount,
            created_at: row.created_at,
        }
    }
}

/// GET /user/commissions?page=&per_page= — the §8 `{items, total}` page
/// envelope (was `/user/invite/details` with `current`/`page_size`).
pub(crate) async fn commissions_list(
    State(state): State<AppState>,
    Query(query): Query<CommissionsQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<CommissionItem>>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let pagination = Pagination::resolve(query.page, query.per_page, COMMISSIONS_DEFAULT_PER_PAGE)?;
    let (rows, total) = v2board_db::invite::fetch_commission_details(
        &state.db,
        user.id,
        pagination.limit(),
        pagination.offset(),
    )
    .await
    .map_err(|error| problem_from(error.into(), locale))?;
    Ok(page(
        rows.into_iter().map(CommissionItem::from).collect(),
        total,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/api-dialect.md §5.6/§9.2 (W7): the invite body is the bare
    /// `{codes, stat}` object — RFC 3339 code timestamps, the named stat
    /// object, and integer-cents commissions.
    #[test]
    fn invite_body_serializes_the_named_stat_object() {
        let body = InviteBody::from(v2board_db::invite::InviteFetchRow {
            codes: vec![v2board_db::invite::InviteCodeRow {
                id: 1,
                user_id: 2,
                code: "goldinv1".to_string(),
                status: 0,
                pv: 3,
                created_at: 1_700_000_000,
                updated_at: 1_700_000_000,
            }],
            stat: v2board_db::invite::InviteStat {
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
        let item = CommissionItem::from(v2board_db::invite::CommissionDetailRow {
            id: 7,
            trade_no: "trade-0007".to_string(),
            order_amount: 1_000,
            get_amount: 100,
            created_at: 1_700_000_000,
        });
        let axum::Json(envelope) = page(vec![item], 42);
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
