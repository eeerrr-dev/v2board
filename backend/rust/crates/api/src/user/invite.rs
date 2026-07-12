use axum::{
    Json,
    extract::{Form, Query, State},
    http::HeaderMap,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data, legacy_page};

use crate::{
    auth::{AuthQuery, require_user},
    runtime::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct TransferRequest {
    transfer_amount: i32,
}

#[derive(sqlx::FromRow)]
struct TransferUserRow {
    commission_balance: i32,
    balance: i32,
    invite_user_id: Option<i32>,
}

#[derive(sqlx::FromRow)]
struct TransferInviterRow {
    commission_type: i8,
    commission_rate: Option<i32>,
}

pub(crate) async fn user_transfer(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TransferRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    // UserTransfer FormRequest: transfer_amount required|integer|min:1.
    if payload.transfer_amount <= 0 {
        return Err(ApiError::validation_field(
            "transfer_amount",
            "The transfer amount parameter is wrong",
        ));
    }
    let config = state.config_snapshot();
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    let current = sqlx::query_as::<_, TransferUserRow>(
        "SELECT commission_balance, balance, invite_user_id FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    let (commission_balance, balance) = checked_transfer_balances(
        current.commission_balance,
        current.balance,
        payload.transfer_amount,
    )?;
    sqlx::query(
        "UPDATE v2_user SET commission_balance = ?, balance = ?, updated_at = ? WHERE id = ?",
    )
    .bind(commission_balance)
    .bind(balance)
    .bind(now)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    // OrderService::setInvite for this deposit order. Laravel only zeroes total_amount
    // AFTER setInvite runs, so the order carries the user's invite_user_id and the
    // inviter's commission is computed against the pre-zero transfer amount.
    let mut order_commission_balance = 0i32;
    if let Some(invite_user_id) = current.invite_user_id {
        let inviter = sqlx::query_as::<_, TransferInviterRow>(
            "SELECT commission_type, commission_rate FROM v2_user WHERE id = ? LIMIT 1",
        )
        .bind(invite_user_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(inviter) = inviter {
            let has_valid_order: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM v2_order WHERE user_id = ? AND status NOT IN (0, 2)",
            )
            .bind(user.id)
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
                    i64::from(payload.transfer_amount),
                    inviter.commission_rate,
                    config.invite_commission,
                )?;
            }
        }
    }

    sqlx::query(
        r#"
        INSERT INTO v2_order (
            user_id, invite_user_id, plan_id, period, trade_no, total_amount, surplus_amount,
            type, status, callback_no, commission_status, commission_balance, created_at, updated_at
        )
        VALUES (?, ?, 0, 'deposit', ?, 0, ?, 9, 3, '佣金划转 Commission transfer', 0, ?, ?, ?)
        "#,
    )
    .bind(user.id)
    .bind(current.invite_user_id)
    .bind(generate_trade_no())
    .bind(payload.transfer_amount)
    .bind(order_commission_balance)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(legacy_data(true))
}

pub(super) fn checked_transfer_balances(
    commission_balance: i32,
    balance: i32,
    transfer_amount: i32,
) -> Result<(i32, i32), ApiError> {
    if transfer_amount <= 0 {
        return Err(ApiError::legacy("The transfer amount parameter is wrong"));
    }
    let commission_balance = commission_balance
        .checked_sub(transfer_amount)
        .filter(|balance| *balance >= 0)
        .ok_or_else(|| ApiError::legacy("Insufficient commission balance"))?;
    let balance = balance
        .checked_add(transfer_amount)
        .ok_or_else(|| ApiError::legacy("Balance exceeds the supported range"))?;
    Ok((commission_balance, balance))
}

pub(crate) async fn invite_save(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let created = v2board_db::invite::create_invite_code(
        &state.db,
        user.id,
        state.config_snapshot().invite_gen_limit,
        Utc::now().timestamp(),
    )
    .await?;
    if !created {
        return Err(ApiError::legacy(
            "The maximum number of creations has been reached",
        ));
    }
    Ok(legacy_data(true))
}

pub(crate) async fn invite_fetch(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<v2board_db::invite::InviteFetchRow>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let data = v2board_db::invite::fetch_invite(&state.db, user.id).await?;
    Ok(legacy_data(data))
}

#[derive(Debug, Deserialize)]
pub(crate) struct PageQuery {
    current: Option<String>,
    #[serde(rename = "page_size", alias = "pageSize")]
    page_size: Option<String>,
    auth_data: Option<String>,
}

pub(crate) async fn invite_details(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<
    Json<v2board_compat::LegacyPageEnvelope<Vec<v2board_db::invite::CommissionDetailRow>>>,
    ApiError,
> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let (page_size, offset) =
        validate_pagination(query.current.as_deref(), query.page_size.as_deref())?;
    let (rows, total) =
        v2board_db::invite::fetch_commission_details(&state.db, user.id, page_size, offset).await?;
    Ok(legacy_page(rows, total))
}

const MAX_PAGE_SIZE: i64 = 100;

pub(super) fn validate_pagination(
    current: Option<&str>,
    page_size: Option<&str>,
) -> Result<(i64, i64), ApiError> {
    let current = parse_positive_page_value(current, "current", 1)?;
    let page_size = parse_positive_page_value(page_size, "page_size", 10)?;
    checked_pagination_values(current, page_size)
}

pub(super) fn checked_pagination_values(
    current: i64,
    page_size: i64,
) -> Result<(i64, i64), ApiError> {
    if current < 1 {
        return Err(ApiError::validation_field(
            "current",
            "Pagination value must be greater than zero",
        ));
    }
    if page_size < 1 {
        return Err(ApiError::validation_field(
            "page_size",
            "Pagination value must be greater than zero",
        ));
    }
    if page_size > MAX_PAGE_SIZE {
        return Err(ApiError::validation_field(
            "page_size",
            "Page size must not exceed 100",
        ));
    }
    let offset = current
        .checked_sub(1)
        .and_then(|page| page.checked_mul(page_size))
        .ok_or_else(|| ApiError::validation_field("current", "Page offset is too large"))?;
    Ok((page_size, offset))
}

fn parse_positive_page_value(
    raw: Option<&str>,
    field: &str,
    default: i64,
) -> Result<i64, ApiError> {
    let Some(raw) = raw else {
        return Ok(default);
    };
    let value = raw
        .trim()
        .parse::<i64>()
        .map_err(|_| ApiError::validation_field(field, "Pagination value must be an integer"))?;
    if value < 1 {
        return Err(ApiError::validation_field(
            field,
            "Pagination value must be greater than zero",
        ));
    }
    Ok(value)
}

fn generate_trade_no() -> String {
    format!(
        "{}{}",
        Utc::now().format("%Y%m%d%H%M%S"),
        Uuid::new_v4().simple()
    )
}
