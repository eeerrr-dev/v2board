use axum::{
    Json,
    extract::{Form, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use v2board_compat::ApiError;

use crate::{
    auth::{AuthQuery, require_user},
    runtime::AppState,
    validation::required_field,
};

#[derive(Debug, Deserialize)]
pub(crate) struct RedeemGiftcardRequest {
    giftcard: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct GiftcardRow {
    id: i64,
    r#type: i8,
    value: Option<i32>,
    plan_id: Option<i32>,
    limit_use: Option<i32>,
    started_at: i64,
    ended_at: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct GiftUserRow {
    id: i64,
    balance: i32,
    expired_at: Option<i64>,
    transfer_enable: i64,
    u: i64,
    d: i64,
    plan_id: Option<i32>,
}

pub(super) const GIFTCARD_FOR_UPDATE_SQL: &str = r#"
SELECT id, `type`, value, plan_id, limit_use, started_at, ended_at
FROM v2_giftcard
WHERE code = ?
LIMIT 1
FOR UPDATE
"#;

const GIB_BYTES: i64 = 1_073_741_824;
const SECONDS_PER_DAY: i64 = 86_400;

pub(crate) async fn redeem_giftcard(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<RedeemGiftcardRequest>,
) -> Result<Response, ApiError> {
    let auth_user = require_user(&state, &headers, query.auth_data).await?;
    // UserRedeemGiftCard FormRequest: giftcard required.
    let giftcard_code = required_field(
        payload.giftcard.as_deref(),
        "giftcard",
        "Giftcard cannot be empty",
    )?;
    let now = Utc::now().timestamp();
    let mut tx = state.db.begin().await?;
    let mut user = sqlx::query_as::<_, GiftUserRow>(
        "SELECT id, balance, expired_at, transfer_enable, u, d, plan_id FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE",
    )
    .bind(auth_user.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    let giftcard = sqlx::query_as::<_, GiftcardRow>(GIFTCARD_FOR_UPDATE_SQL)
        .bind(giftcard_code)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::legacy("The gift card does not exist"))?;
    if giftcard.started_at != 0 && now < giftcard.started_at {
        return Err(ApiError::legacy("The gift card is not yet valid"));
    }
    if giftcard.ended_at != 0 && now > giftcard.ended_at {
        return Err(ApiError::legacy("The gift card has expired"));
    }
    if giftcard.limit_use.is_some_and(|limit| limit <= 0) {
        return Err(ApiError::legacy(
            "The gift card usage limit has been reached",
        ));
    }
    let already_redeemed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM v2_giftcard_redemption WHERE giftcard_id = ? AND user_id = ?)",
    )
    .bind(giftcard.id)
    .bind(auth_user.id)
    .fetch_one(&mut *tx)
    .await?;
    if already_redeemed {
        return Err(ApiError::legacy(
            "The gift card has already been used by this user",
        ));
    }
    let value = giftcard.value.unwrap_or_default();
    if matches!(giftcard.r#type, 1 | 2 | 3 | 5) && value < 0 {
        return Err(ApiError::legacy("Gift card value cannot be negative"));
    }
    let mut group_id = None::<i32>;
    let mut device_limit = None::<i32>;
    // Laravel case 5 assigns device_limit unconditionally (including NULL) and never
    // touches speed_limit; `apply_plan_card` drives the IF() in the UPDATE so a NULL plan
    // device_limit overwrites the user's value instead of being swallowed by COALESCE.
    let mut apply_plan_card = false;
    match giftcard.r#type {
        1 => {
            user.balance = checked_add_cents(
                user.balance,
                value,
                "Gift card redemption exceeds the supported balance range",
            )?;
        }
        2 => {
            let expired_at = user
                .expired_at
                .ok_or_else(|| ApiError::legacy("Not suitable gift card type"))?;
            user.expired_at = Some(checked_add_giftcard_days(expired_at.max(now), value)?);
        }
        3 => {
            let bytes = checked_gib_bytes(i64::from(value))?;
            user.transfer_enable = user
                .transfer_enable
                .checked_add(bytes)
                .ok_or_else(|| ApiError::legacy("Gift card traffic exceeds the supported range"))?;
        }
        4 => {
            user.u = 0;
            user.d = 0;
        }
        5 => {
            let can_apply = user.plan_id.is_none()
                || user.expired_at.is_some_and(|expired_at| expired_at < now);
            if !can_apply {
                return Err(ApiError::legacy("Not suitable gift card type"));
            }
            let plan_id = giftcard
                .plan_id
                .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
            let plan = v2board_db::plan::find_plan_for_update(&mut tx, plan_id)
                .await?
                .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
            if let Some(capacity_limit) = plan.capacity_limit {
                let has_reservation: bool = sqlx::query_scalar(
                    r#"
                    SELECT EXISTS(
                        SELECT 1
                        FROM v2_order
                        WHERE user_id = ? AND plan_id = ?
                          AND status IN (0, 1) AND `type` IN (1, 3)
                    )
                    "#,
                )
                .bind(user.id)
                .bind(plan.id)
                .fetch_one(&mut *tx)
                .await?;
                let capacity_used =
                    v2board_db::plan::capacity_usage_for_update(&mut tx, plan.id).await?;
                if !giftcard_plan_has_capacity(capacity_limit, capacity_used, has_reservation) {
                    return Err(ApiError::legacy("Current product is sold out"));
                }
            }
            user.plan_id = Some(plan.id);
            group_id = Some(plan.group_id);
            device_limit = plan.device_limit;
            apply_plan_card = true;
            user.transfer_enable = checked_gib_bytes(plan.transfer_enable)?;
            user.u = 0;
            user.d = 0;
            user.expired_at = if value == 0 {
                None
            } else {
                Some(checked_add_giftcard_days(now, value)?)
            };
        }
        _ => return Err(ApiError::legacy("Unknown gift card type")),
    }

    sqlx::query(
        r#"
        UPDATE v2_user
        SET balance = ?, expired_at = ?, transfer_enable = ?, u = ?, d = ?,
            plan_id = ?, group_id = COALESCE(?, group_id),
            device_limit = IF(?, ?, device_limit), updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(user.balance)
    .bind(user.expired_at)
    .bind(user.transfer_enable)
    .bind(user.u)
    .bind(user.d)
    .bind(user.plan_id)
    .bind(group_id)
    .bind(apply_plan_card as i32)
    .bind(device_limit)
    .bind(now)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO v2_giftcard_redemption (giftcard_id, user_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(giftcard.id)
    .bind(auth_user.id)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE v2_giftcard SET limit_use = ?, updated_at = ? WHERE id = ?")
        .bind(giftcard.limit_use.map(|limit| limit - 1))
        .bind(now)
        .bind(giftcard.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({
        "data": true,
        "type": giftcard.r#type,
        "value": giftcard.value,
    }))
    .into_response())
}

pub(super) fn checked_add_cents(left: i32, right: i32, message: &str) -> Result<i32, ApiError> {
    left.checked_add(right)
        .ok_or_else(|| ApiError::legacy(message))
}

pub(super) fn checked_gib_bytes(gib: i64) -> Result<i64, ApiError> {
    if gib < 0 {
        return Err(ApiError::legacy("Gift card traffic cannot be negative"));
    }
    gib.checked_mul(GIB_BYTES)
        .ok_or_else(|| ApiError::legacy("Gift card traffic exceeds the supported range"))
}

pub(super) fn checked_add_giftcard_days(base: i64, days: i32) -> Result<i64, ApiError> {
    if days < 0 {
        return Err(ApiError::legacy("Gift card duration cannot be negative"));
    }
    let seconds = i64::from(days)
        .checked_mul(SECONDS_PER_DAY)
        .ok_or_else(|| ApiError::legacy("Gift card duration exceeds the supported range"))?;
    base.checked_add(seconds)
        .ok_or_else(|| ApiError::legacy("Gift card duration exceeds the supported range"))
}

pub(super) fn giftcard_plan_has_capacity(
    capacity_limit: i32,
    capacity_used: i64,
    has_existing_reservation: bool,
) -> bool {
    has_existing_reservation || capacity_used < i64::from(capacity_limit)
}
