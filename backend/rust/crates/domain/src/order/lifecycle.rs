use chrono::{Datelike, Months, TimeZone, Utc};
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use sqlx::{MySql, QueryBuilder, Transaction};
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::{app_now, app_timezone};
use v2board_db::plan::PlanRow;

use super::{
    CouponRow, DraftOrder, GIB, OrderForCheckout, OrderService, PaymentForCheckout,
    SurplusOrderRow, UNFINISHED_ORDER_UNIQUE_KEY, UserForOrder, bounded_payment_identifier,
    payment_identifier_hash,
};

pub(super) fn checked_add_cents(
    left: i32,
    right: i32,
    message: &'static str,
) -> Result<i32, ApiError> {
    left.checked_add(right)
        .ok_or_else(|| ApiError::legacy(message))
}

pub(super) async fn credit_user_balance(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
    credit: i32,
    overflow_message: &'static str,
) -> Result<(), ApiError> {
    let current_balance: i32 =
        sqlx::query_scalar("SELECT balance FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    let new_balance = checked_add_cents(current_balance, credit, overflow_message)?;
    sqlx::query("UPDATE v2_user SET balance = ?, updated_at = ? WHERE id = ?")
        .bind(new_balance)
        .bind(Utc::now().timestamp())
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl OrderService {
    pub(super) async fn build_deposit_order(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: UserForOrder,
        period: &str,
        deposit_amount: Option<i32>,
        trade_no: String,
    ) -> Result<DraftOrder, ApiError> {
        if period != "deposit" {
            return Err(ApiError::legacy("Wrong plan period"));
        }
        let amount = deposit_amount.unwrap_or_default();
        if amount <= 0 {
            return Err(ApiError::legacy(
                "Failed to create order, deposit amount must be greater than 0",
            ));
        }
        if amount >= 9_999_999 {
            return Err(ApiError::legacy(
                "Deposit amount too large, please contact the administrator",
            ));
        }
        let mut draft = DraftOrder {
            user_id: user.id,
            plan_id: 0,
            coupon_id: None,
            r#type: 9,
            period: "deposit".to_string(),
            trade_no,
            total_amount: Decimal::from(amount),
            discount_amount: None,
            surplus_amount: None,
            refund_amount: None,
            balance_amount: None,
            surplus_order_ids: None,
            invite_user_id: None,
            commission_balance: Decimal::ZERO,
        };
        self.set_invite(tx, &user, &mut draft).await?;
        Ok(draft)
    }

    pub(super) async fn build_plan_order(
        &self,
        tx: &mut Transaction<'_, MySql>,
        mut user: UserForOrder,
        plan_id: i32,
        period: &str,
        coupon_code: Option<&str>,
        trade_no: String,
    ) -> Result<DraftOrder, ApiError> {
        let plan = v2board_db::plan::find_plan_for_update(tx, plan_id)
            .await?
            .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
        if period != "reset_price" {
            checked_plan_transfer_bytes(plan.transfer_enable)?;
        }
        let already_occupies_capacity = user.plan_id == Some(plan.id)
            && user
                .expired_at
                .is_none_or(|expired_at| expired_at >= Utc::now().timestamp());
        if !already_occupies_capacity
            && !have_capacity(tx, plan.id, plan.capacity_limit).await?
            && period != "reset_price"
        {
            return Err(ApiError::legacy("Current product is sold out"));
        }
        let price = purchasable_period_price(&plan, period)?;
        if period == "reset_price" && (!is_available(&user) || user.plan_id != Some(plan.id)) {
            return Err(ApiError::legacy(
                "Subscription has expired or no active subscription, unable to purchase Data Reset Package",
            ));
        }
        let hidden_unbuyable = plan.show == 0 && (plan.renew == 0 || user.plan_id != Some(plan.id));
        if hidden_unbuyable && period != "reset_price" {
            return Err(ApiError::legacy(
                "This subscription has been sold out, please choose another subscription",
            ));
        }
        if plan.renew == 0 && user.plan_id == Some(plan.id) && period != "reset_price" {
            return Err(ApiError::legacy(
                "This subscription cannot be renewed, please change to another subscription",
            ));
        }
        if plan.show == 0 && plan.renew != 0 && !is_available(&user) {
            return Err(ApiError::legacy(
                "This subscription has expired, please change to another subscription",
            ));
        }

        let mut draft = DraftOrder {
            user_id: user.id,
            plan_id: plan.id,
            coupon_id: None,
            r#type: 1,
            period: period.to_string(),
            trade_no,
            total_amount: Decimal::from(price),
            discount_amount: None,
            surplus_amount: None,
            refund_amount: None,
            balance_amount: None,
            surplus_order_ids: None,
            invite_user_id: None,
            commission_balance: Decimal::ZERO,
        };

        if let Some(code) = coupon_code.filter(|code| !code.trim().is_empty()) {
            self.apply_coupon(tx, code, &mut draft).await?;
        }
        apply_vip_discount(user.discount, &mut draft);
        self.set_order_type(tx, &user, &plan, &mut draft).await?;
        self.apply_balance(tx, &mut user, &mut draft).await?;
        self.set_invite(tx, &user, &mut draft).await?;
        Ok(draft)
    }

    async fn apply_coupon(
        &self,
        tx: &mut Transaction<'_, MySql>,
        code: &str,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        let coupon = sqlx::query_as::<_, CouponRow>(
            r#"
            SELECT
                id,
                `type`,
                value,
                `show`,
                limit_use,
                limit_use_with_user,
                limit_plan_ids,
                limit_period,
                started_at,
                ended_at
            FROM v2_coupon
            WHERE code = ?
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(code)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::legacy("Invalid coupon"))?;
        validate_coupon(tx, &coupon, draft).await?;

        // Mirror Laravel CouponService::use: set discount_amount (capped at the
        // order total) but do NOT reduce total_amount here. The single
        // total_amount -= discount_amount subtraction happens in
        // apply_vip_discount so the VIP percentage is computed on the original
        // (pre-coupon) total, matching OrderService::setVipDiscount.
        let discount = match coupon.r#type {
            1 => Decimal::from(coupon.value),
            2 => draft.total_amount * percent(coupon.value),
            _ => Decimal::ZERO,
        }
        .min(draft.total_amount);
        draft.discount_amount = Some(discount);
        draft.coupon_id = Some(coupon.id);

        if let Some(limit_use) = coupon.limit_use {
            if limit_use <= 0 {
                return Err(ApiError::legacy("Coupon failed"));
            }
            let result = sqlx::query("UPDATE v2_coupon SET limit_use = limit_use - 1 WHERE id = ?")
                .bind(coupon.id)
                .execute(&mut **tx)
                .await?;
            if result.rows_affected() == 0 {
                return Err(ApiError::legacy("Coupon failed"));
            }
        }
        Ok(())
    }

    async fn set_order_type(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: &UserForOrder,
        plan: &PlanRow,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        let now = Utc::now().timestamp();
        if draft.period == "reset_price" {
            draft.r#type = 4;
            return Ok(());
        }
        if user.plan_id.is_some()
            && user.plan_id != Some(draft.plan_id)
            && (user.expired_at.is_none() || user.expired_at.unwrap_or_default() > now)
        {
            if !self.config.plan_change_enable {
                return Err(ApiError::legacy(
                    "目前不允许更改订阅，请联系客服或提交工单操作",
                ));
            }
            draft.r#type = 3;
            if self.config.surplus_enable {
                self.apply_surplus_value(tx, user, draft).await?;
            }
            let surplus = draft.surplus_amount.unwrap_or_default();
            if surplus >= draft.total_amount {
                draft.refund_amount = Some(surplus - draft.total_amount);
                draft.total_amount = Decimal::ZERO;
            } else {
                draft.total_amount -= surplus;
            }
            return Ok(());
        }
        if user.expired_at.unwrap_or_default() > now && user.plan_id == Some(plan.id) {
            draft.r#type = 2;
        } else {
            draft.r#type = 1;
        }
        Ok(())
    }

    async fn apply_surplus_value(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: &UserForOrder,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        if user.expired_at.is_none() {
            let Some(last_order) = sqlx::query_as::<_, SurplusOrderRow>(
                r#"
                SELECT id, period, total_amount, balance_amount, surplus_amount, refund_amount, created_at
                FROM v2_order
                WHERE user_id = ? AND period = 'onetime_price' AND status = 3
                ORDER BY id DESC
                LIMIT 1
                "#,
            )
            .bind(user.id)
            .fetch_optional(&mut **tx)
            .await?
            else {
                return Ok(());
            };
            let total_traffic = Decimal::from(user.transfer_enable);
            if total_traffic <= Decimal::ZERO {
                return Ok(());
            }
            let paid_total = i64::from(last_order.total_amount)
                .checked_add(i64::from(last_order.balance_amount.unwrap_or_default()))
                .ok_or_else(|| {
                    ApiError::legacy("Subscription surplus amount exceeds the supported range")
                })?;
            if paid_total <= 0 {
                return Ok(());
            }
            let unused_traffic = Decimal::from(checked_unused_traffic(user)?);
            let remaining_ratio = checked_surplus_div(unused_traffic, total_traffic)?;
            draft.surplus_amount = Some(
                checked_surplus_mul(Decimal::from(paid_total), remaining_ratio)?.max(Decimal::ZERO),
            );
            draft.surplus_order_ids = fetch_surplus_order_ids(tx, user.id, true).await?;
            return Ok(());
        }

        let rows = sqlx::query_as::<_, SurplusOrderRow>(
            r#"
            SELECT id, period, total_amount, balance_amount, surplus_amount, refund_amount, created_at
            FROM v2_order
            WHERE user_id = ?
              AND period != 'reset_price'
              AND period != 'onetime_price'
              AND period != 'deposit'
              AND status = 3
            ORDER BY id ASC
            "#,
        )
        .bind(user.id)
        .fetch_all(&mut **tx)
        .await?;
        if rows.is_empty() {
            return Ok(());
        }
        let mut order_amount_sum = 0_i64;
        let mut order_month_sum = 0_u32;
        let mut last_validate_at: Option<i64> = None;
        let now = Utc::now().timestamp();
        for row in &rows {
            let Some(months) = period_months(&row.period) else {
                continue;
            };
            let order_end_time = checked_surplus_add_months(row.created_at, months)?;
            if order_end_time < now {
                continue;
            }
            last_validate_at =
                Some(last_validate_at.map_or(row.created_at, |last| last.max(row.created_at)));
            order_month_sum = checked_order_month_sum(order_month_sum, months)?;
            order_amount_sum = checked_order_amount_sum(
                order_amount_sum,
                row.total_amount,
                row.balance_amount,
                row.surplus_amount,
                row.refund_amount,
            )?;
        }
        let Some(last_validate_at) = last_validate_at else {
            return Ok(());
        };
        let expired_at_by_order = checked_surplus_add_months(last_validate_at, order_month_sum)?;
        let Some(expired_at_by_user) = user.expired_at else {
            return Ok(());
        };
        if expired_at_by_order < now || expired_at_by_user < now {
            return Ok(());
        }
        let order_surplus_second = checked_surplus_seconds(expired_at_by_user, now)?;
        let order_range_second = checked_surplus_seconds(expired_at_by_order, last_validate_at)?;
        if order_range_second <= 0 || user.transfer_enable <= 0 {
            return Ok(());
        }
        let remaining_traffic_ratio = checked_surplus_div(
            Decimal::from(checked_unused_traffic(user)?),
            Decimal::from(user.transfer_enable),
        )?;
        let avg_price_per_second = checked_surplus_div(
            Decimal::from(order_amount_sum),
            Decimal::from(order_range_second),
        )?;
        let surplus = if order_range_second <= 31 * 86_400 {
            let remaining_expired_time_ratio = checked_surplus_div(
                Decimal::from(order_surplus_second),
                Decimal::from(order_range_second),
            )?;
            let surplus_ratio = remaining_expired_time_ratio.min(remaining_traffic_ratio);
            checked_surplus_mul(
                checked_surplus_mul(avg_price_per_second, Decimal::from(order_surplus_second))?,
                surplus_ratio,
            )?
        } else {
            let month_seconds = 30 * 86_400;
            let first_month_remain_seconds = order_surplus_second % month_seconds;
            let surplus_ratio = checked_surplus_div(
                Decimal::from(first_month_remain_seconds),
                Decimal::from(month_seconds),
            )?
            .min(remaining_traffic_ratio);
            let later_months_seconds = order_surplus_second
                .checked_sub(first_month_remain_seconds)
                .ok_or_else(|| {
                    ApiError::legacy("Subscription surplus duration exceeds the supported range")
                })?;
            let first_month = checked_surplus_mul(
                checked_surplus_mul(avg_price_per_second, Decimal::from(month_seconds))?,
                surplus_ratio,
            )?;
            let later_months =
                checked_surplus_mul(avg_price_per_second, Decimal::from(later_months_seconds))?;
            checked_surplus_add(first_month, later_months)?
        };
        draft.surplus_amount = Some(surplus.max(Decimal::ZERO));
        draft.surplus_order_ids = Some(rows.into_iter().map(|row| row.id).collect());
        Ok(())
    }

    async fn apply_balance(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: &mut UserForOrder,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        if user.balance <= 0 || draft.total_amount <= Decimal::ZERO {
            return Ok(());
        }
        let use_balance = Decimal::from(user.balance).min(draft.total_amount);
        // Laravel passes the still-fractional deduction to
        // `UserService::addBalance(int $balance)`, whose `int` parameter truncates
        // toward zero before subtracting
        // it from the balance column. So the actual balance deduction is `trunc(use_balance)`,
        // NOT a round — e.g. a 0.5-cent total leaves the balance untouched. The recorded
        // `balance_amount` field, by contrast, is stored via Eloquent save() and DOES get
        // MySQL-rounded (round_cents at insert), so the two can legitimately differ by a cent.
        let use_balance_cents = use_balance
            .trunc()
            .to_i32()
            .ok_or_else(|| ApiError::internal("balance deduction is outside the integer range"))?;
        let result = sqlx::query(
            r#"
            UPDATE v2_user
            SET balance = balance - ?, updated_at = ?
            WHERE id = ? AND balance >= ?
            "#,
        )
        .bind(use_balance_cents)
        .bind(Utc::now().timestamp())
        .bind(user.id)
        .bind(use_balance_cents)
        .execute(&mut **tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(ApiError::legacy("Insufficient balance"));
        }
        user.balance -= use_balance_cents;
        draft.balance_amount = Some(use_balance);
        draft.total_amount -= use_balance;
        Ok(())
    }

    async fn set_invite(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: &UserForOrder,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        let Some(invite_user_id) = user.invite_user_id else {
            return Ok(());
        };
        if draft.total_amount <= Decimal::ZERO {
            return Ok(());
        }
        draft.invite_user_id = Some(invite_user_id);
        let inviter = sqlx::query_as::<_, UserForOrder>(USER_FOR_ORDER_SQL)
            .bind(invite_user_id)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(inviter) = inviter else {
            return Ok(());
        };
        let has_valid_order = have_valid_order(tx, user.id).await?;
        if !commission_is_eligible(
            inviter.commission_type,
            self.config.commission_first_time_enable,
            has_valid_order,
        ) {
            return Ok(());
        }
        draft.commission_balance = commission_amount(
            draft.total_amount,
            inviter.commission_rate,
            self.config.invite_commission,
        );
        Ok(())
    }

    pub(super) async fn open_order_in_tx(
        &self,
        tx: &mut Transaction<'_, MySql>,
        order: OrderForCheckout,
    ) -> Result<(), ApiError> {
        if order.r#type == 9 {
            let bonus = self.config.deposit_bonus(order.total_amount);
            let credit = checked_add_cents(
                order.total_amount,
                bonus,
                "Deposit principal and bonus exceed the supported cents range",
            )?;
            credit_user_balance(
                tx,
                order.user_id,
                credit,
                "Deposit credit exceeds the supported balance range",
            )
            .await?;
            sqlx::query("UPDATE v2_order SET status = 3, updated_at = ? WHERE id = ?")
                .bind(Utc::now().timestamp())
                .bind(order.id)
                .execute(&mut **tx)
                .await?;
            return Ok(());
        }

        let mut user = find_user_for_order(tx, order.user_id).await?;
        let plan = v2board_db::plan::find_plan_for_update(tx, order.plan_id)
            .await?
            .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
        if let Some(refund_amount) = order.refund_amount.filter(|amount| *amount > 0) {
            user.balance = checked_add_cents(
                user.balance,
                refund_amount,
                "Order refund exceeds the supported balance range",
            )?;
        }
        let surplus_ids = parse_i64_json_list(order.surplus_order_ids.as_deref());
        if let Some(ids) = surplus_ids.as_deref() {
            mark_surplus_orders(tx, ids).await?;
        }

        // Read the wall clock once so every expiry/reset decision in this open
        // uses a single consistent `now` (Laravel evaluates one Carbon::now()).
        let now = Utc::now().timestamp();
        match order.period.as_str() {
            "onetime_price" => buy_by_one_time(&mut user, &plan, surplus_ids.is_some())?,
            "reset_price" => reset_traffic(&mut user)?,
            period => buy_by_period(&mut user, &order, &plan, period, now)?,
        }
        match order.r#type {
            1 if self.config.new_order_event_id == 1 => reset_traffic(&mut user)?,
            2 if self.config.renew_order_event_id == 1 => reset_traffic(&mut user)?,
            3 if self.config.change_order_event_id == 1 => reset_traffic(&mut user)?,
            _ => {}
        }
        user.speed_limit = plan.speed_limit;
        save_opened_user(tx, &user).await?;
        sqlx::query("UPDATE v2_order SET status = 3, updated_at = ? WHERE id = ?")
            .bind(Utc::now().timestamp())
            .bind(order.id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

pub(super) async fn find_user_for_order(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
) -> Result<UserForOrder, ApiError> {
    sqlx::query_as::<_, UserForOrder>(USER_FOR_ORDER_SQL)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))
}

async fn have_capacity(
    tx: &mut Transaction<'_, MySql>,
    plan_id: i32,
    capacity_limit: Option<i32>,
) -> Result<bool, sqlx::Error> {
    let Some(capacity_limit) = capacity_limit else {
        return Ok(true);
    };
    let capacity_used = v2board_db::plan::capacity_usage_for_update(tx, plan_id).await?;
    Ok(capacity_has_slot(capacity_limit, capacity_used))
}

pub(super) fn capacity_has_slot(capacity_limit: i32, capacity_used: i64) -> bool {
    capacity_used < i64::from(capacity_limit)
}

async fn validate_coupon(
    tx: &mut Transaction<'_, MySql>,
    coupon: &CouponRow,
    draft: &DraftOrder,
) -> Result<(), ApiError> {
    validate_coupon_discount(coupon.r#type, coupon.value)?;
    if coupon.show == 0 {
        return Err(ApiError::legacy("Invalid coupon"));
    }
    if matches!(coupon.limit_use, Some(limit_use) if limit_use <= 0) {
        return Err(ApiError::legacy("This coupon is no longer available"));
    }
    let now = Utc::now().timestamp();
    if now < coupon.started_at {
        return Err(ApiError::legacy("This coupon has not yet started"));
    }
    if now > coupon.ended_at {
        return Err(ApiError::legacy("This coupon has expired"));
    }
    if let Some(plan_ids) = parse_i32_json_list(coupon.limit_plan_ids.as_deref())
        && !plan_ids.contains(&draft.plan_id)
    {
        return Err(ApiError::legacy(
            "The coupon code cannot be used for this subscription",
        ));
    }
    if let Some(periods) = parse_string_json_list(coupon.limit_period.as_deref())
        && !periods.iter().any(|period| period == &draft.period)
    {
        return Err(ApiError::legacy(
            "The coupon code cannot be used for this period",
        ));
    }
    if let Some(limit) = coupon.limit_use_with_user {
        let used_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM v2_order
            WHERE coupon_id = ? AND user_id = ? AND status NOT IN (0, 2)
            "#,
        )
        .bind(coupon.id)
        .bind(draft.user_id)
        .fetch_one(&mut **tx)
        .await?;
        if used_count >= i64::from(limit) {
            return Err(ApiError::legacy(format!(
                "The coupon can only be used {limit} per person"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_coupon_discount(coupon_type: i8, value: i32) -> Result<(), ApiError> {
    let valid = match coupon_type {
        1 => value >= 0,
        2 => (0..=100).contains(&value),
        _ => false,
    };
    if !valid {
        return Err(ApiError::legacy("Invalid coupon discount value"));
    }
    Ok(())
}

/// Whether the inviter earns commission on this order, mirroring the
/// `commission_type` switch in OrderService::setInvite (lines 146-157).
/// `has_valid_order` is whether the buyer already has a completed order, which
/// gates first-purchase-only commission (types 0 and 2).
pub(super) fn commission_is_eligible(
    commission_type: i8,
    first_time_enable: bool,
    has_valid_order: bool,
) -> bool {
    match commission_type {
        // case 0: pay unless first-time gating is on and the buyer already ordered.
        0 => !first_time_enable || !has_valid_order,
        // case 1: always pay.
        1 => true,
        // case 2: pay only on the buyer's first order.
        2 => !has_valid_order,
        // unrecognized type: no commission (the switch leaves $isCommission false).
        _ => false,
    }
}

/// The inviter's commission for an order: `total_amount * rate%`. A per-inviter
/// `commission_rate` takes effect when set (`if ($inviter->commission_rate)`);
/// otherwise the global `invite_commission` default applies (OrderService::
/// setInvite lines 160-164).
pub(super) fn percent(value: i32) -> Decimal {
    Decimal::from(value) / Decimal::from(100)
}

pub(super) fn commission_amount(
    total_amount: Decimal,
    commission_rate: Option<i32>,
    default_rate: i32,
) -> Decimal {
    let rate = commission_rate
        .filter(|rate| *rate != 0)
        .unwrap_or(default_rate);
    total_amount * percent(rate)
}

async fn have_valid_order(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_order WHERE user_id = ? AND status NOT IN (0, 2)",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(count > 0)
}

async fn fetch_surplus_order_ids(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
    include_one_time: bool,
) -> Result<Option<Vec<i64>>, sqlx::Error> {
    let sql = if include_one_time {
        r#"
        SELECT id
        FROM v2_order
        WHERE user_id = ? AND period != 'reset_price' AND status = 3
        "#
    } else {
        r#"
        SELECT id
        FROM v2_order
        WHERE user_id = ?
          AND period != 'reset_price'
          AND period != 'onetime_price'
          AND period != 'deposit'
          AND status = 3
        "#
    };
    let ids: Vec<i64> = sqlx::query_scalar(sql)
        .bind(user_id)
        .fetch_all(&mut **tx)
        .await?;
    Ok((!ids.is_empty()).then_some(ids))
}

pub(super) async fn insert_order(
    tx: &mut Transaction<'_, MySql>,
    draft: &DraftOrder,
    now: i64,
) -> Result<(), ApiError> {
    let surplus_order_ids = draft
        .surplus_order_ids
        .as_ref()
        .map(|ids| serde_json::to_string(ids).expect("integer order IDs are JSON serializable"));
    let total_amount = round_cents(draft.total_amount)?;
    let discount_amount = draft.discount_amount.map(round_cents).transpose()?;
    let surplus_amount = draft.surplus_amount.map(round_cents).transpose()?;
    let refund_amount = draft.refund_amount.map(round_cents).transpose()?;
    let balance_amount = draft.balance_amount.map(round_cents).transpose()?;
    let commission_balance = round_cents(draft.commission_balance)?;
    let result = sqlx::query(
        r#"
        INSERT INTO v2_order (
            invite_user_id,
            user_id,
            plan_id,
            coupon_id,
            `type`,
            period,
            trade_no,
            total_amount,
            discount_amount,
            surplus_amount,
            refund_amount,
            balance_amount,
            surplus_order_ids,
            status,
            commission_status,
            commission_balance,
            created_at,
            updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?)
        "#,
    )
    .bind(draft.invite_user_id)
    .bind(draft.user_id)
    .bind(draft.plan_id)
    .bind(draft.coupon_id)
    .bind(draft.r#type)
    .bind(&draft.period)
    .bind(&draft.trade_no)
    .bind(total_amount)
    .bind(discount_amount)
    .bind(surplus_amount)
    .bind(refund_amount)
    .bind(balance_amount)
    .bind(surplus_order_ids)
    .bind(commission_balance)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await;
    match result {
        Ok(_) => Ok(()),
        Err(error) if is_unfinished_order_unique_violation(&error) => Err(ApiError::legacy(
            "You have an unpaid or pending order, please try again later or cancel it",
        )),
        Err(error) => Err(error.into()),
    }
}

fn is_unfinished_order_unique_violation(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some_and(|error| {
        error.constraint() == Some(UNFINISHED_ORDER_UNIQUE_KEY)
            || error.message().contains(UNFINISHED_ORDER_UNIQUE_KEY)
    })
}

pub(super) async fn mark_order_paid(
    tx: &mut Transaction<'_, MySql>,
    order_id: i64,
    callback_no: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    let callback_no_label = bounded_payment_identifier(callback_no);
    let callback_no_hash = payment_identifier_hash(callback_no);
    sqlx::query(
        r#"
        UPDATE v2_order
        SET status = 1, paid_at = ?, callback_no = ?, callback_no_hash = ?, updated_at = ?
        WHERE id = ? AND status = 0
        "#,
    )
    .bind(now)
    .bind(callback_no_label)
    .bind(callback_no_hash.as_slice())
    .bind(now)
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn mark_surplus_orders(
    tx: &mut Transaction<'_, MySql>,
    ids: &[i64],
) -> Result<(), sqlx::Error> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_order SET status = 4 WHERE id IN (");
    let mut separated = builder.separated(", ");
    for id in ids {
        separated.push_bind(id);
    }
    separated.push_unseparated(")");
    builder.build().execute(&mut **tx).await?;
    Ok(())
}

async fn save_opened_user(
    tx: &mut Transaction<'_, MySql>,
    user: &UserForOrder,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE v2_user
        SET
            balance = ?,
            traffic_epoch = ?,
            u = ?,
            d = ?,
            transfer_enable = ?,
            device_limit = ?,
            group_id = ?,
            plan_id = ?,
            speed_limit = ?,
            expired_at = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(user.balance)
    .bind(user.traffic_epoch)
    .bind(user.u)
    .bind(user.d)
    .bind(user.transfer_enable)
    .bind(user.device_limit)
    .bind(user.group_id)
    .bind(user.plan_id)
    .bind(user.speed_limit)
    .bind(user.expired_at)
    .bind(Utc::now().timestamp())
    .bind(user.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) fn apply_vip_discount(discount: Option<i32>, draft: &mut DraftOrder) {
    // Port of OrderService::setVipDiscount. The VIP percentage is applied to the
    // still-original total_amount (the coupon step only recorded discount_amount
    // without reducing the total), then the accumulated coupon+VIP
    // discount_amount is subtracted from the total exactly once. When neither a
    // coupon nor a VIP discount applies, discount_amount stays None and the
    // total is left untouched, matching Laravel's null discount_amount.
    //
    // Laravel rounds only when the value lands in the int column at save() time.
    // Keep exact decimal fractions here: rounding the coupon and VIP portions
    // separately before summing drifts by a cent from the persisted contract.
    if let Some(discount) = discount.filter(|discount| *discount > 0) {
        let value = draft.total_amount * percent(discount);
        draft.discount_amount = Some(draft.discount_amount.unwrap_or_default() + value);
    }
    if let Some(discount_amount) = draft.discount_amount {
        draft.total_amount -= discount_amount;
    }
}

pub(super) fn calculate_handling_amount(
    order: &OrderForCheckout,
    payment: &PaymentForCheckout,
) -> Result<Option<i32>, ApiError> {
    calculate_handling_amount_cents(order.total_amount, payment)
}

pub(super) fn calculate_handling_amount_cents(
    order_total_amount: i32,
    payment: &PaymentForCheckout,
) -> Result<Option<i32>, ApiError> {
    let fixed = payment.handling_fee_fixed.unwrap_or_default();
    let percent = payment.handling_fee_percent.unwrap_or_default();
    if fixed < 0 || percent < Decimal::ZERO {
        return Err(ApiError::legacy(
            "Payment handling fee must not be negative",
        ));
    }
    if fixed == 0 && percent.is_zero() {
        return Ok(None);
    }
    let amount = Decimal::from(order_total_amount)
        .checked_mul(percent)
        .and_then(|amount| amount.checked_div(Decimal::from(100)))
        .and_then(|amount| amount.checked_add(Decimal::from(fixed)))
        .ok_or_else(|| ApiError::legacy("Payment handling fee is outside the supported range"))?;
    let amount = amount
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i32()
        .ok_or_else(|| ApiError::legacy("Payment handling fee is outside the supported range"))?;
    Ok(Some(amount))
}

pub(super) fn buy_by_period(
    user: &mut UserForOrder,
    order: &OrderForCheckout,
    plan: &PlanRow,
    period: &str,
    now: i64,
) -> Result<(), ApiError> {
    let transfer_enable = checked_plan_transfer_bytes(plan.transfer_enable)?;
    if order.r#type == 3 {
        user.expired_at = Some(now);
    }
    user.transfer_enable = transfer_enable;
    user.device_limit = plan.device_limit;
    if user.expired_at.is_none() || order.r#type == 1 {
        reset_traffic(user)?;
    }
    if order.r#type == 2
        && let Some(expired_at) = user.expired_at
        && is_same_local_month_day(expired_at, now)
    {
        reset_traffic(user)?;
    }
    user.plan_id = Some(plan.id);
    user.group_id = Some(plan.group_id);
    user.expired_at = Some(add_period_time(period, user.expired_at.unwrap_or(now), now));
    Ok(())
}

pub(super) fn buy_by_one_time(
    user: &mut UserForOrder,
    plan: &PlanRow,
    has_surplus_orders: bool,
) -> Result<(), ApiError> {
    // Work in bytes so fractional leftover GiB is preserved. Laravel computes
    // (plan_gib + leftover_bytes/GiB) * GiB, which is algebraically
    // plan_bytes + leftover_bytes; the earlier integer division here truncated
    // the fractional GiB (OrderService::buyByOneTime, :331-337).
    let mut transfer_enable = checked_plan_transfer_bytes(plan.transfer_enable)?;
    if !has_surplus_orders {
        let not_used_traffic = checked_unused_traffic(user)?;
        if not_used_traffic > 0 && user.expired_at.is_none() {
            transfer_enable = transfer_enable
                .checked_add(not_used_traffic)
                .ok_or_else(|| {
                    ApiError::legacy("Subscription traffic allowance exceeds the supported range")
                })?;
        }
    }
    reset_traffic(user)?;
    user.transfer_enable = transfer_enable;
    user.device_limit = plan.device_limit;
    user.plan_id = Some(plan.id);
    user.group_id = Some(plan.group_id);
    user.expired_at = None;
    Ok(())
}

fn reset_traffic(user: &mut UserForOrder) -> Result<(), ApiError> {
    user.traffic_epoch = user
        .traffic_epoch
        .checked_add(1)
        .ok_or_else(|| ApiError::internal("user traffic epoch exceeds the supported range"))?;
    user.u = 0;
    user.d = 0;
    Ok(())
}

fn is_available(user: &UserForOrder) -> bool {
    let unexpired = user
        .expired_at
        .map(|expired_at| expired_at > Utc::now().timestamp())
        .unwrap_or(true);
    user.banned == 0 && user.transfer_enable > 0 && unexpired
}

fn plan_period_price(plan: &PlanRow, period: &str) -> Option<i32> {
    match period {
        "month_price" => plan.month_price,
        "quarter_price" => plan.quarter_price,
        "half_year_price" => plan.half_year_price,
        "year_price" => plan.year_price,
        "two_year_price" => plan.two_year_price,
        "three_year_price" => plan.three_year_price,
        "onetime_price" => plan.onetime_price,
        "reset_price" => plan.reset_price,
        _ => None,
    }
}

pub(super) fn purchasable_period_price(plan: &PlanRow, period: &str) -> Result<i32, ApiError> {
    let price = plan_period_price(plan, period).ok_or_else(|| {
        ApiError::legacy("This payment period cannot be purchased, please choose another period")
    })?;
    if price < 0 {
        return Err(ApiError::legacy("Subscription price must not be negative"));
    }
    Ok(price)
}

fn checked_plan_transfer_bytes(transfer_gib: i64) -> Result<i64, ApiError> {
    if transfer_gib < 0 {
        return Err(ApiError::legacy(
            "Subscription traffic allowance must not be negative",
        ));
    }
    transfer_gib.checked_mul(GIB).ok_or_else(|| {
        ApiError::legacy("Subscription traffic allowance exceeds the supported range")
    })
}

pub(super) fn checked_unused_traffic(user: &UserForOrder) -> Result<i64, ApiError> {
    let used_traffic = user
        .u
        .checked_add(user.d)
        .ok_or_else(|| ApiError::legacy("Used traffic exceeds the supported range"))?;
    user.transfer_enable
        .checked_sub(used_traffic)
        .ok_or_else(|| ApiError::legacy("Unused traffic exceeds the supported range"))
}

pub(super) fn checked_order_month_sum(current: u32, months: u32) -> Result<u32, ApiError> {
    current
        .checked_add(months)
        .ok_or_else(|| ApiError::legacy("Subscription surplus months exceed the supported range"))
}

pub(super) fn checked_order_amount_sum(
    current: i64,
    total_amount: i32,
    balance_amount: Option<i32>,
    surplus_amount: Option<i32>,
    refund_amount: Option<i32>,
) -> Result<i64, ApiError> {
    current
        .checked_add(i64::from(total_amount))
        .and_then(|amount| amount.checked_add(i64::from(balance_amount.unwrap_or_default())))
        .and_then(|amount| amount.checked_add(i64::from(surplus_amount.unwrap_or_default())))
        .and_then(|amount| amount.checked_sub(i64::from(refund_amount.unwrap_or_default())))
        .ok_or_else(|| ApiError::legacy("Subscription surplus amount exceeds the supported range"))
}

pub(super) fn checked_surplus_seconds(end: i64, start: i64) -> Result<i64, ApiError> {
    end.checked_sub(start).ok_or_else(|| {
        ApiError::legacy("Subscription surplus duration exceeds the supported range")
    })
}

pub(super) fn checked_surplus_add_months(timestamp: i64, months: u32) -> Result<i64, ApiError> {
    let base = app_timezone()
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or_else(|| ApiError::legacy("Subscription timestamp is outside the supported range"))?;
    base.checked_add_months(Months::new(months))
        .map(|date| date.timestamp())
        .ok_or_else(|| ApiError::legacy("Subscription surplus expiry exceeds the supported range"))
}

pub(super) fn checked_surplus_mul(left: Decimal, right: Decimal) -> Result<Decimal, ApiError> {
    left.checked_mul(right)
        .ok_or_else(|| ApiError::legacy("Subscription surplus amount exceeds the supported range"))
}

pub(super) fn checked_surplus_add(left: Decimal, right: Decimal) -> Result<Decimal, ApiError> {
    left.checked_add(right)
        .ok_or_else(|| ApiError::legacy("Subscription surplus amount exceeds the supported range"))
}

pub(super) fn checked_surplus_div(left: Decimal, right: Decimal) -> Result<Decimal, ApiError> {
    left.checked_div(right)
        .ok_or_else(|| ApiError::legacy("Subscription surplus ratio exceeds the supported range"))
}

pub(super) fn is_valid_period(period: &str) -> bool {
    matches!(
        period,
        "month_price"
            | "quarter_price"
            | "half_year_price"
            | "year_price"
            | "two_year_price"
            | "three_year_price"
            | "onetime_price"
            | "reset_price"
            | "deposit"
    )
}

fn period_months(period: &str) -> Option<u32> {
    match period {
        "month_price" => Some(1),
        "quarter_price" => Some(3),
        "half_year_price" => Some(6),
        "year_price" => Some(12),
        "two_year_price" => Some(24),
        "three_year_price" => Some(36),
        _ => None,
    }
}

pub(super) fn add_period_time(period: &str, timestamp: i64, now: i64) -> i64 {
    let base = timestamp.max(now);
    period_months(period)
        .map(|months| add_months(base, months))
        .unwrap_or(base)
}

pub(super) fn add_months(timestamp: i64, months: u32) -> i64 {
    let base = app_timezone()
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(app_now);
    base.checked_add_months(Months::new(months))
        .unwrap_or(base)
        .timestamp()
}

fn is_same_local_month_day(left: i64, right: i64) -> bool {
    let timezone = app_timezone();
    let Some(left) = timezone.timestamp_opt(left, 0).single() else {
        return false;
    };
    let Some(right) = timezone.timestamp_opt(right, 0).single() else {
        return false;
    };
    left.month() == right.month() && left.day() == right.day()
}

/// Round an exact decimal cents amount to the integer stored in the DB's amount
/// columns. MySQL rounds a fractional numeric value half away from zero when it
/// is assigned to an integer column, so make that boundary explicit.
pub(super) fn round_cents(amount: Decimal) -> Result<i32, ApiError> {
    amount
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i32()
        .ok_or_else(|| ApiError::legacy("Order amount is outside the supported range"))
}

pub(super) fn generate_order_no() -> String {
    let now = app_now();
    let bytes = *Uuid::new_v4().as_bytes();
    let random = 10_000 + (u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 90_000);
    format!(
        "{}{:06}{}",
        now.format("%Y%m%d%H%M%S"),
        now.timestamp_subsec_micros(),
        random
    )
}

fn parse_i64_json_list(value: Option<&str>) -> Option<Vec<i64>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<i64>>(value)
        .ok()
        .filter(|items| !items.is_empty())
}

fn parse_i32_json_list(value: Option<&str>) -> Option<Vec<i32>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<i32>>(value)
        .ok()
        .filter(|items| !items.is_empty())
}

fn parse_string_json_list(value: Option<&str>) -> Option<Vec<String>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<String>>(value)
        .ok()
        .filter(|items| !items.is_empty())
}

pub(super) const USER_FOR_ORDER_SQL: &str = r#"
SELECT
    id,
    invite_user_id,
    balance,
    discount,
    commission_type,
    commission_rate,
    traffic_epoch,
    u,
    d,
    transfer_enable,
    device_limit,
    banned,
    group_id,
    plan_id,
    speed_limit,
    expired_at
FROM v2_user
WHERE id = ?
LIMIT 1
FOR UPDATE
"#;
