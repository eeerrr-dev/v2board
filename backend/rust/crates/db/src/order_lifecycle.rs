use crate::DbTransaction;
use crate::plan::PlanRow;
use chrono::{DateTime, Datelike, FixedOffset, Months, TimeZone, Utc};
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use serde_json::json;
use sqlx::types::Json;
use sqlx::{Postgres, QueryBuilder};
use v2board_application::order::{CreateOrderPolicy, FulfillmentPolicy, PaymentSnapshotVerifier};
use v2board_domain_model::{
    CommissionEligibility, CouponKind, CouponRuleViolation, CouponUseContext, OrderKind,
    OrderPeriod, OrderState, PlanPricePeriod, SubscriptionAvailability,
    commission_is_eligible as domain_commission_is_eligible, validate_coupon,
};

use super::order_runtime::{
    ApiError, Code, DraftOrder, GIB, OrderForCheckout, PaymentForCheckout, PostgresOrderRepository,
    Problem, SurplusOrderRow, UNFINISHED_ORDER_UNIQUE_KEY, UserForOrder,
    bounded_payment_identifier, payment_identifier_hash,
};

pub(super) struct PlanOrderDraftInput<'a> {
    pub user: UserForOrder,
    pub plan_id: i32,
    pub period: &'a str,
    pub coupon_code: Option<&'a str>,
    pub trade_no: String,
    pub policy: CreateOrderPolicy,
}

pub(super) fn checked_add_cents(
    left: i32,
    right: i32,
    message: &'static str,
) -> Result<i32, ApiError> {
    left.checked_add(right)
        .ok_or_else(|| ApiError::legacy(message))
}

pub(super) async fn credit_user_balance(
    tx: &mut DbTransaction<'_>,
    user_id: i64,
    credit: i32,
    overflow_message: &'static str,
) -> Result<(), ApiError> {
    let current_balance: i32 =
        sqlx::query_scalar("SELECT balance FROM users WHERE id = $1 LIMIT 1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    let new_balance = checked_add_cents(current_balance, credit, overflow_message)?;
    sqlx::query("UPDATE users SET balance = $1, updated_at = $2 WHERE id = $3")
        .bind(new_balance)
        .bind(Utc::now().timestamp())
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl<V> PostgresOrderRepository<V>
where
    V: PaymentSnapshotVerifier,
{
    pub(super) async fn build_deposit_order(
        &self,
        tx: &mut DbTransaction<'_>,
        user: UserForOrder,
        deposit_amount: i32,
        trade_no: String,
        policy: CreateOrderPolicy,
    ) -> Result<DraftOrder, ApiError> {
        // The deposit arm of the §5.5 order union is structural: no period
        // sentinel to validate. The amount bounds keep the legacy wording as
        // presentation-only detail on the range problem.
        let amount = deposit_amount;
        if amount <= 0 {
            return Err(Problem::new(Code::PaymentAmountOutOfRange)
                .with_detail("Failed to create order, deposit amount must be greater than 0")
                .into());
        }
        if amount >= 9_999_999 {
            return Err(Problem::new(Code::PaymentAmountOutOfRange)
                .with_detail("Deposit amount too large, please contact the administrator")
                .into());
        }
        let mut draft = DraftOrder {
            user_id: user.id,
            plan_id: 0,
            coupon_id: None,
            kind: OrderKind::BalanceDeposit.code(),
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
        self.set_invite(tx, &user, &mut draft, policy).await?;
        Ok(draft)
    }

    pub(super) async fn build_plan_order(
        &self,
        tx: &mut DbTransaction<'_>,
        input: PlanOrderDraftInput<'_>,
    ) -> Result<DraftOrder, ApiError> {
        let PlanOrderDraftInput {
            mut user,
            plan_id,
            period,
            coupon_code,
            trade_no,
            policy,
        } = input;
        let plan = crate::plan::find_plan_for_update(tx, plan_id)
            .await?
            .ok_or_else(|| ApiError::from(Problem::new(Code::PlanUnavailable)))?;
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
            return Err(Problem::new(Code::PlanSoldOut).into());
        }
        let price = purchasable_period_price(&plan, period)?;
        if period == "reset_price" && (!is_available(&user) || user.plan_id != Some(plan.id)) {
            return Err(Problem::new(Code::PlanPeriodUnavailable)
                .with_detail(
                    "Subscription has expired or no active subscription, unable to purchase Data Reset Package",
                )
                .into());
        }
        let hidden_unbuyable = !plan.show && (!plan.renew || user.plan_id != Some(plan.id));
        if hidden_unbuyable && period != "reset_price" {
            return Err(Problem::new(Code::PlanSoldOut)
                .with_detail(
                    "This subscription has been sold out, please choose another subscription",
                )
                .into());
        }
        if !plan.renew && user.plan_id == Some(plan.id) && period != "reset_price" {
            return Err(Problem::new(Code::RenewalNotAllowed)
                .with_detail(
                    "This subscription cannot be renewed, please change to another subscription",
                )
                .into());
        }
        if !plan.show && plan.renew && !is_available(&user) {
            return Err(Problem::new(Code::RenewalNotAllowed)
                .with_detail("This subscription has expired, please change to another subscription")
                .into());
        }

        let mut draft = DraftOrder {
            user_id: user.id,
            plan_id: plan.id,
            coupon_id: None,
            kind: OrderKind::NewSubscription.code(),
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
        self.set_order_type(tx, &user, &plan, &mut draft, policy)
            .await?;
        self.apply_balance(tx, &mut user, &mut draft).await?;
        self.set_invite(tx, &user, &mut draft, policy).await?;
        Ok(draft)
    }

    async fn apply_coupon(
        &self,
        tx: &mut DbTransaction<'_>,
        code: &str,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        let coupon = crate::coupon::find_coupon_for_update(tx, code)
            .await?
            .ok_or_else(|| ApiError::from(Problem::new(Code::CouponInvalid)))?;
        let user_use_count = if coupon.per_user_limit.is_some() {
            crate::coupon::count_user_coupon_uses_in_transaction(tx, coupon.id, draft.user_id)
                .await?
        } else {
            0
        };
        let kind = validate_coupon(
            &coupon,
            CouponUseContext {
                plan_id: Some(draft.plan_id),
                period: Some(&draft.period),
                user_use_count,
                now: Utc::now().timestamp(),
            },
        )
        .map_err(coupon_rule_error)?;

        // Mirror Laravel CouponService::use: set discount_amount (capped at the
        // order total) but do NOT reduce total_amount here. The single
        // total_amount -= discount_amount subtraction happens in
        // apply_vip_discount so the VIP percentage is computed on the original
        // (pre-coupon) total, matching OrderService::setVipDiscount.
        let discount = match kind {
            CouponKind::Amount => Decimal::from(coupon.value),
            CouponKind::Percentage => draft.total_amount * percent(coupon.value),
        }
        .min(draft.total_amount);
        draft.discount_amount = Some(discount);
        draft.coupon_id = Some(coupon.id);

        if coupon.remaining_uses.is_some()
            && !crate::coupon::decrement_coupon_use(tx, coupon.id).await?
        {
            return Err(Problem::new(Code::CouponExhausted).into());
        }
        Ok(())
    }

    async fn set_order_type(
        &self,
        tx: &mut DbTransaction<'_>,
        user: &UserForOrder,
        plan: &PlanRow,
        draft: &mut DraftOrder,
        policy: CreateOrderPolicy,
    ) -> Result<(), ApiError> {
        let now = Utc::now().timestamp();
        if draft.period == "reset_price" {
            draft.kind = OrderKind::TrafficReset.code();
            return Ok(());
        }
        if user.plan_id.is_some()
            && user.plan_id != Some(draft.plan_id)
            && (user.expired_at.is_none() || user.expired_at.unwrap_or_default() > now)
        {
            if !policy.plan_change_enabled {
                return Err(Problem::new(Code::PlanChangeDisabled).into());
            }
            draft.kind = OrderKind::PlanChange.code();
            if policy.surplus_enabled {
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
            draft.kind = OrderKind::Renewal.code();
        } else {
            draft.kind = OrderKind::NewSubscription.code();
        }
        Ok(())
    }

    async fn apply_surplus_value(
        &self,
        tx: &mut DbTransaction<'_>,
        user: &UserForOrder,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        if user.expired_at.is_none() {
            let Some(last_order) = sqlx::query_as::<_, SurplusOrderRow>(
                r#"
                SELECT id, period, total_amount, balance_amount, surplus_amount, refund_amount, created_at
                FROM orders
                WHERE user_id = $1 AND period = 'onetime_price' AND status = 3
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
                    ApiError::from(
                        Problem::new(Code::SubscriptionValueOutOfRange)
                            .with_detail("Subscription surplus amount exceeds the supported range"),
                    )
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
            FROM orders
            WHERE user_id = $1
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
            let later_months_seconds =
                order_surplus_second
                    .checked_sub(first_month_remain_seconds)
                    .ok_or_else(|| {
                        ApiError::from(Problem::new(Code::SubscriptionValueOutOfRange).with_detail(
                            "Subscription surplus duration exceeds the supported range",
                        ))
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
        tx: &mut DbTransaction<'_>,
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
            UPDATE users
            SET balance = balance - $1, updated_at = $2
            WHERE id = $3 AND balance >= $4
            "#,
        )
        .bind(use_balance_cents)
        .bind(Utc::now().timestamp())
        .bind(user.id)
        .bind(use_balance_cents)
        .execute(&mut **tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(Problem::new(Code::InsufficientBalance).into());
        }
        user.balance -= use_balance_cents;
        draft.balance_amount = Some(use_balance);
        draft.total_amount -= use_balance;
        Ok(())
    }

    async fn set_invite(
        &self,
        tx: &mut DbTransaction<'_>,
        user: &UserForOrder,
        draft: &mut DraftOrder,
        policy: CreateOrderPolicy,
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
            policy.commission_first_time_enabled,
            has_valid_order,
        ) {
            return Ok(());
        }
        draft.commission_balance = commission_amount(
            draft.total_amount,
            inviter.commission_rate,
            policy.default_commission_rate,
        );
        Ok(())
    }

    pub(super) async fn open_order_in_tx(
        &self,
        tx: &mut DbTransaction<'_>,
        order: OrderForCheckout,
        fulfillment: FulfillmentPolicy,
    ) -> Result<(), ApiError> {
        if order.kind == OrderKind::BalanceDeposit.code() {
            let bonus = fulfillment.deposit_bonus;
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
            sqlx::query("UPDATE orders SET status = 3, updated_at = $1 WHERE id = $2")
                .bind(Utc::now().timestamp())
                .bind(order.id)
                .execute(&mut **tx)
                .await?;
            return Ok(());
        }

        let mut user = find_user_for_order(tx, order.user_id).await?;
        let plan = crate::plan::find_plan_for_update(tx, order.plan_id)
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
        if order_event_resets_traffic(
            order.kind,
            fulfillment.new_order_event_id,
            fulfillment.renewal_order_event_id,
            fulfillment.change_order_event_id,
        ) {
            reset_traffic(&mut user)?;
        }
        user.speed_limit = plan.speed_limit;
        save_opened_user(tx, &user).await?;
        sqlx::query("UPDATE orders SET status = 3, updated_at = $1 WHERE id = $2")
            .bind(Utc::now().timestamp())
            .bind(order.id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

/// Whether the configured post-fulfilment hook resets traffic for this order.
/// Unknown codes deliberately remain a no-op, matching the former wildcard
/// match arm and preventing corrupt historical rows from selecting a hook.
pub(super) const fn order_event_resets_traffic(
    order_kind_code: i32,
    new_order_event_id: i32,
    renew_order_event_id: i32,
    change_order_event_id: i32,
) -> bool {
    match OrderKind::from_code(order_kind_code) {
        Some(OrderKind::NewSubscription) => new_order_event_id == 1,
        Some(OrderKind::Renewal) => renew_order_event_id == 1,
        Some(OrderKind::PlanChange) => change_order_event_id == 1,
        Some(OrderKind::TrafficReset | OrderKind::BalanceDeposit) | None => false,
    }
}

pub(super) async fn find_user_for_order(
    tx: &mut DbTransaction<'_>,
    user_id: i64,
) -> Result<UserForOrder, ApiError> {
    sqlx::query_as::<_, UserForOrder>(USER_FOR_ORDER_SQL)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))
}

async fn have_capacity(
    tx: &mut DbTransaction<'_>,
    plan_id: i32,
    capacity_limit: Option<i32>,
) -> Result<bool, sqlx::Error> {
    let Some(capacity_limit) = capacity_limit else {
        return Ok(true);
    };
    let capacity_used = crate::plan::capacity_usage_for_update(tx, plan_id).await?;
    Ok(capacity_has_slot(capacity_limit, capacity_used))
}

pub(super) fn capacity_has_slot(capacity_limit: i32, capacity_used: i64) -> bool {
    capacity_used < i64::from(capacity_limit)
}

fn coupon_rule_error(violation: CouponRuleViolation) -> ApiError {
    let problem = match violation {
        CouponRuleViolation::InvalidDiscount => {
            Problem::new(Code::CouponInvalid).with_detail("Invalid coupon discount value")
        }
        CouponRuleViolation::Hidden => Problem::new(Code::CouponInvalid),
        CouponRuleViolation::Unavailable => Problem::new(Code::CouponUnavailable),
        CouponRuleViolation::NotStarted => Problem::new(Code::CouponNotStarted),
        CouponRuleViolation::Expired => Problem::new(Code::CouponExpired),
        CouponRuleViolation::PlanNotApplicable => Problem::new(Code::CouponNotApplicable)
            .with_detail("The coupon code cannot be used for this subscription"),
        CouponRuleViolation::PeriodNotApplicable => Problem::new(Code::CouponNotApplicable)
            .with_detail("The coupon code cannot be used for this period"),
        CouponRuleViolation::UserLimitExceeded(limit) => Problem::new(Code::CouponNotApplicable)
            .with_detail(format!("The coupon can only be used {limit} per person")),
    };
    problem.into()
}

/// Whether the inviter earns commission on this order, mirroring the
/// `commission_type` switch in OrderService::setInvite (lines 146-157).
/// `has_valid_order` is whether the buyer already has a completed order, which
/// gates first-purchase-only commission (types 0 and 2).
pub(super) fn commission_is_eligible(
    commission_type: i16,
    first_time_enable: bool,
    has_valid_order: bool,
) -> bool {
    let policy = match commission_type {
        0 => CommissionEligibility::ConfigurableFirstPurchase,
        1 => CommissionEligibility::Always,
        2 => CommissionEligibility::FirstPurchaseOnly,
        _ => return false,
    };
    domain_commission_is_eligible(policy, first_time_enable, has_valid_order)
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

async fn have_valid_order(tx: &mut DbTransaction<'_>, user_id: i64) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orders WHERE user_id = $1 AND status NOT IN (0, 2)",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(count > 0)
}

async fn fetch_surplus_order_ids(
    tx: &mut DbTransaction<'_>,
    user_id: i64,
    include_one_time: bool,
) -> Result<Option<Vec<i64>>, sqlx::Error> {
    let sql = if include_one_time {
        r#"
        SELECT id
        FROM orders
        WHERE user_id = $1 AND period != 'reset_price' AND status = 3
        "#
    } else {
        r#"
        SELECT id
        FROM orders
        WHERE user_id = $1
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
    tx: &mut DbTransaction<'_>,
    draft: &DraftOrder,
    now: i64,
) -> Result<(), ApiError> {
    let surplus_order_ids = draft.surplus_order_ids.as_ref().map(|ids| Json(json!(ids)));
    let total_amount = round_cents(draft.total_amount)?;
    let discount_amount = draft.discount_amount.map(round_cents).transpose()?;
    let surplus_amount = draft.surplus_amount.map(round_cents).transpose()?;
    let refund_amount = draft.refund_amount.map(round_cents).transpose()?;
    let balance_amount = draft.balance_amount.map(round_cents).transpose()?;
    let commission_balance = round_cents(draft.commission_balance)?;
    let result = sqlx::query(
        r#"
        INSERT INTO orders (
            invite_user_id,
            user_id,
            plan_id,
            coupon_id,
            "type",
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
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 0, 0, $14, $15, $16)
        "#,
    )
    .bind(draft.invite_user_id)
    .bind(draft.user_id)
    .bind(draft.plan_id)
    .bind(draft.coupon_id)
    .bind(draft.kind)
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
        Err(error) if is_unfinished_order_unique_violation(&error) => {
            Err(Problem::new(Code::PendingOrderExists).into())
        }
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
    tx: &mut DbTransaction<'_>,
    order_id: i64,
    callback_no: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    let callback_no_label = bounded_payment_identifier(callback_no);
    let callback_no_hash = payment_identifier_hash(callback_no);
    sqlx::query(
        r#"
        UPDATE orders
        SET status = $1, paid_at = $2, callback_no = $3, callback_no_hash = $4, updated_at = $5
        WHERE id = $6 AND status = $7
        "#,
    )
    .bind(OrderState::Activating.code())
    .bind(now)
    .bind(callback_no_label)
    .bind(callback_no_hash.as_slice())
    .bind(now)
    .bind(order_id)
    .bind(OrderState::Pending.code())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn mark_surplus_orders(tx: &mut DbTransaction<'_>, ids: &[i64]) -> Result<(), sqlx::Error> {
    for chunk in ids.chunks(500) {
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE orders SET status = ");
        builder.push_bind(OrderState::Credited.code());
        builder.push(" WHERE id IN (");
        let mut separated = builder.separated(", ");
        for id in chunk {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        builder.build().execute(&mut **tx).await?;
    }
    Ok(())
}

async fn save_opened_user(
    tx: &mut DbTransaction<'_>,
    user: &UserForOrder,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE users
        SET
            balance = $1,
            traffic_epoch = $2,
            u = $3,
            d = $4,
            transfer_enable = $5,
            device_limit = $6,
            group_id = $7,
            plan_id = $8,
            speed_limit = $9,
            expired_at = $10,
            updated_at = $11
        WHERE id = $12
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
        return Err(Problem::new(Code::HandlingFeeOutOfRange)
            .with_detail("Payment handling fee must not be negative")
            .into());
    }
    if fixed == 0 && percent.is_zero() {
        return Ok(None);
    }
    let amount = Decimal::from(order_total_amount)
        .checked_mul(percent)
        .and_then(|amount| amount.checked_div(Decimal::from(100)))
        .and_then(|amount| amount.checked_add(Decimal::from(fixed)))
        .ok_or_else(|| ApiError::from(Problem::new(Code::HandlingFeeOutOfRange)))?;
    let amount = amount
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i32()
        .ok_or_else(|| ApiError::from(Problem::new(Code::HandlingFeeOutOfRange)))?;
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
    if order.kind == OrderKind::PlanChange.code() {
        user.expired_at = Some(now);
    }
    user.transfer_enable = transfer_enable;
    user.device_limit = plan.device_limit;
    if user.expired_at.is_none() || order.kind == OrderKind::NewSubscription.code() {
        reset_traffic(user)?;
    }
    if order.kind == OrderKind::Renewal.code()
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
    SubscriptionAvailability {
        banned: user.banned != 0,
        transfer_enable: user.transfer_enable,
        expiry: user.expired_at,
    }
    .is_available(Utc::now().timestamp())
}

fn plan_period_price(plan: &PlanRow, period: &str) -> Option<i32> {
    plan.price(order_period_from_storage(period)?.plan_period()?)
}

pub(super) fn purchasable_period_price(plan: &PlanRow, period: &str) -> Result<i32, ApiError> {
    let price = plan_period_price(plan, period)
        .ok_or_else(|| ApiError::from(Problem::new(Code::PlanPeriodUnavailable)))?;
    if price < 0 {
        return Err(Problem::new(Code::PaymentAmountOutOfRange)
            .with_detail("Subscription price must not be negative")
            .into());
    }
    Ok(price)
}

fn checked_plan_transfer_bytes(transfer_gib: i64) -> Result<i64, ApiError> {
    if transfer_gib < 0 {
        return Err(subscription_value_out_of_range(
            "Subscription traffic allowance must not be negative",
        ));
    }
    transfer_gib.checked_mul(GIB).ok_or_else(|| {
        subscription_value_out_of_range(
            "Subscription traffic allowance exceeds the supported range",
        )
    })
}

pub(super) fn checked_unused_traffic(user: &UserForOrder) -> Result<i64, ApiError> {
    let used_traffic = user.u.checked_add(user.d).ok_or_else(|| {
        subscription_value_out_of_range("Used traffic exceeds the supported range")
    })?;
    user.transfer_enable
        .checked_sub(used_traffic)
        .ok_or_else(|| {
            subscription_value_out_of_range("Unused traffic exceeds the supported range")
        })
}

pub(super) fn checked_order_month_sum(current: u32, months: u32) -> Result<u32, ApiError> {
    current.checked_add(months).ok_or_else(|| {
        subscription_value_out_of_range("Subscription surplus months exceed the supported range")
    })
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
        .ok_or_else(|| {
            subscription_value_out_of_range(
                "Subscription surplus amount exceeds the supported range",
            )
        })
}

pub(super) fn checked_surplus_seconds(end: i64, start: i64) -> Result<i64, ApiError> {
    end.checked_sub(start).ok_or_else(|| {
        subscription_value_out_of_range("Subscription surplus duration exceeds the supported range")
    })
}

pub(super) fn checked_surplus_add_months(timestamp: i64, months: u32) -> Result<i64, ApiError> {
    let base = order_timezone()
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or_else(|| {
            subscription_value_out_of_range("Subscription timestamp is outside the supported range")
        })?;
    base.checked_add_months(Months::new(months))
        .map(|date| date.timestamp())
        .ok_or_else(|| {
            subscription_value_out_of_range(
                "Subscription surplus expiry exceeds the supported range",
            )
        })
}

pub(super) fn checked_surplus_mul(left: Decimal, right: Decimal) -> Result<Decimal, ApiError> {
    left.checked_mul(right).ok_or_else(|| {
        subscription_value_out_of_range("Subscription surplus amount exceeds the supported range")
    })
}

pub(super) fn checked_surplus_add(left: Decimal, right: Decimal) -> Result<Decimal, ApiError> {
    left.checked_add(right).ok_or_else(|| {
        subscription_value_out_of_range("Subscription surplus amount exceeds the supported range")
    })
}

pub(super) fn checked_surplus_div(left: Decimal, right: Decimal) -> Result<Decimal, ApiError> {
    left.checked_div(right).ok_or_else(|| {
        subscription_value_out_of_range("Subscription surplus ratio exceeds the supported range")
    })
}

/// §3.4 `subscription_value_out_of_range`: the "Subscription/traffic …
/// exceeds the supported range" family shares one code; the specific legacy
/// message stays as presentation-only `detail`.
fn subscription_value_out_of_range(detail: &'static str) -> ApiError {
    Problem::new(Code::SubscriptionValueOutOfRange)
        .with_detail(detail)
        .into()
}

pub(super) const fn plan_period_storage_name(period: PlanPricePeriod) -> &'static str {
    match period {
        PlanPricePeriod::Month => "month_price",
        PlanPricePeriod::Quarter => "quarter_price",
        PlanPricePeriod::HalfYear => "half_year_price",
        PlanPricePeriod::Year => "year_price",
        PlanPricePeriod::TwoYear => "two_year_price",
        PlanPricePeriod::ThreeYear => "three_year_price",
        PlanPricePeriod::OneTime => "onetime_price",
        PlanPricePeriod::Reset => "reset_price",
    }
}

fn period_months(period: &str) -> Option<u32> {
    order_period_from_storage(period)?.recurring_months()
}

fn order_period_from_storage(period: &str) -> Option<OrderPeriod> {
    match period {
        "month_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Month)),
        "quarter_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Quarter)),
        "half_year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::HalfYear)),
        "year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Year)),
        "two_year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::TwoYear)),
        "three_year_price" => Some(OrderPeriod::Plan(PlanPricePeriod::ThreeYear)),
        "onetime_price" => Some(OrderPeriod::Plan(PlanPricePeriod::OneTime)),
        "reset_price" => Some(OrderPeriod::Plan(PlanPricePeriod::Reset)),
        "deposit" => Some(OrderPeriod::Deposit),
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
    let base = order_timezone()
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(order_now);
    base.checked_add_months(Months::new(months))
        .unwrap_or(base)
        .timestamp()
}

fn is_same_local_month_day(left: i64, right: i64) -> bool {
    let timezone = order_timezone();
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
        .ok_or_else(|| {
            ApiError::from(
                Problem::new(Code::PaymentAmountOutOfRange)
                    .with_detail("Order amount is outside the supported range"),
            )
        })
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
FROM users
WHERE id = $1
LIMIT 1
FOR UPDATE
"#;

fn order_timezone() -> FixedOffset {
    FixedOffset::east_opt(8 * 3_600).expect("the pinned application offset is valid")
}

fn order_now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&order_timezone())
}
