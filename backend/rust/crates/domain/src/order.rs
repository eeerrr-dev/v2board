use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Local, Months, TimeZone, Utc};
use hmac::{Hmac, KeyInit, Mac};
use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    sign::{Signer, Verifier},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Sha256, Sha512};
use sqlx::{FromRow, MySql, MySqlPool, QueryBuilder, Transaction};
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::AppConfig;
use v2board_db::plan::PlanRow;

use crate::payment_provider::{PaymentProviderManifest, payment_provider_manifest};

const GIB: i64 = 1_073_741_824;

#[derive(Clone)]
pub struct OrderService {
    db: MySqlPool,
    config: AppConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveOrderInput {
    pub plan_id: Option<i32>,
    pub period: Option<String>,
    pub coupon_code: Option<String>,
    pub deposit_amount: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutOrderInput {
    pub trade_no: String,
    pub method: Option<i32>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckoutResult {
    pub r#type: i8,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone)]
struct DraftOrder {
    user_id: i64,
    plan_id: i32,
    coupon_id: Option<i32>,
    r#type: i32,
    period: String,
    trade_no: String,
    // Amounts stay floats through the whole build so coupon/VIP/surplus/balance/
    // commission math is never rounded mid-pipeline; insert_order rounds once when
    // binding to the int amount columns, mirroring Laravel (which lets MySQL round
    // the persisted float). See round_cents / apply_vip_discount.
    total_amount: f64,
    discount_amount: Option<f64>,
    surplus_amount: Option<f64>,
    refund_amount: Option<f64>,
    balance_amount: Option<f64>,
    surplus_order_ids: Option<Vec<i64>>,
    invite_user_id: Option<i32>,
    commission_balance: f64,
}

#[derive(Debug, Clone, FromRow)]
struct UserForOrder {
    id: i64,
    invite_user_id: Option<i32>,
    balance: i32,
    discount: Option<i32>,
    commission_type: i8,
    commission_rate: Option<i32>,
    u: i64,
    d: i64,
    transfer_enable: i64,
    device_limit: Option<i32>,
    banned: i8,
    group_id: Option<i32>,
    plan_id: Option<i32>,
    speed_limit: Option<i32>,
    expired_at: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct CouponRow {
    id: i32,
    r#type: i8,
    value: i32,
    show: i8,
    limit_use: Option<i32>,
    limit_use_with_user: Option<i32>,
    limit_plan_ids: Option<String>,
    limit_period: Option<String>,
    started_at: i64,
    ended_at: i64,
}

#[derive(Debug, Clone, FromRow)]
struct SurplusOrderRow {
    id: i64,
    period: String,
    total_amount: i32,
    balance_amount: Option<i32>,
    surplus_amount: Option<i32>,
    refund_amount: Option<i32>,
    created_at: i64,
}

#[derive(Debug, Clone, FromRow)]
struct OrderForCheckout {
    id: i64,
    user_id: i64,
    plan_id: i32,
    r#type: i32,
    period: String,
    trade_no: String,
    total_amount: i32,
    refund_amount: Option<i32>,
    surplus_order_ids: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct PaymentForCheckout {
    id: i32,
    payment: String,
    enable: i8,
    uuid: String,
    config: String,
    notify_domain: Option<String>,
    handling_fee_fixed: Option<i32>,
    handling_fee_percent: Option<f64>,
}

#[derive(Debug, Clone)]
struct PaymentOrder {
    notify_url: String,
    return_url: String,
    trade_no: String,
    total_amount: i32,
    user_id: i64,
    stripe_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PaymentNotifyResponse {
    pub body: String,
    /// Set only when this callback made the fresh `status 0 -> 1` paid transition,
    /// so the HTTP layer can send Laravel's `成功收款` admin Telegram message exactly
    /// once (a gateway replay leaves this `None`). Mirrors `PaymentController::handle`,
    /// which sends the message only inside the `status !== 0` guard.
    pub paid_notice: Option<PaidOrderNotice>,
}

/// The order fields Laravel's `PaymentController::handle` reads to build the admin
/// `💰成功收款` Telegram message after a fresh paid transition.
#[derive(Debug, Clone)]
pub struct PaidOrderNotice {
    pub trade_no: String,
    pub total_amount: i64,
}

#[derive(Debug, Clone)]
pub struct PaymentNotifyInput {
    pub params: HashMap<String, String>,
    pub body: Vec<u8>,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct VerifiedPaymentNotify {
    trade_no: String,
    callback_no: String,
    custom_result: Option<String>,
}

enum PaymentNotifyOutcome {
    Verified(VerifiedPaymentNotify),
    Ignored(String),
}

impl OrderService {
    pub fn new(db: MySqlPool, config: AppConfig) -> Self {
        Self { db, config }
    }

    pub async fn save(&self, user_id: i64, input: SaveOrderInput) -> Result<String, ApiError> {
        // Laravel OrderSave FormRequest: plan_id `required`, period
        // `required|in:<period list>`. A failed FormRequest is a 422
        // `{message, errors:{field:[msg]}}`, not the 500 these pre-logic checks
        // used to return.
        let plan_id = input
            .plan_id
            .ok_or_else(|| order_validation("plan_id", "Plan ID cannot be empty"))?;
        let period = input
            .period
            .as_deref()
            .filter(|period| !period.trim().is_empty())
            .ok_or_else(|| order_validation("period", "Plan period cannot be empty"))?;
        if !is_valid_period(period) {
            return Err(order_validation("period", "Wrong plan period"));
        }

        let mut tx = self.db.begin().await?;
        let incomplete_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM v2_order WHERE user_id = ? AND status IN (0, 1)",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
        if incomplete_count > 0 {
            return Err(ApiError::legacy(
                "You have an unpaid or pending order, please try again later or cancel it",
            ));
        }

        let user = find_user_for_order(&mut tx, user_id).await?;
        let trade_no = generate_order_no();
        let now = Utc::now().timestamp();
        let draft = if plan_id == 0 {
            self.build_deposit_order(&mut tx, user, period, input.deposit_amount, trade_no)
                .await?
        } else {
            self.build_plan_order(
                &mut tx,
                user,
                plan_id,
                period,
                input.coupon_code.as_deref(),
                trade_no,
            )
            .await?
        };

        insert_order(&mut tx, &draft, now).await?;
        tx.commit().await?;
        Ok(draft.trade_no)
    }

    pub async fn checkout(
        &self,
        user_id: i64,
        input: CheckoutOrderInput,
    ) -> Result<CheckoutResult, ApiError> {
        let mut tx = self.db.begin().await?;
        let order = sqlx::query_as::<_, OrderForCheckout>(
            r#"
            SELECT
                id,
                user_id,
                plan_id,
                `type`,
                period,
                trade_no,
                total_amount,
                refund_amount,
                surplus_order_ids
            FROM v2_order
            WHERE trade_no = ? AND user_id = ? AND status = 0
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(&input.trade_no)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::legacy("Order does not exist or has been paid"))?;

        if order.total_amount <= 0 {
            mark_order_paid(&mut tx, order.id, &order.trade_no, Utc::now().timestamp()).await?;
            self.open_order_in_tx(&mut tx, order).await?;
            tx.commit().await?;
            return Ok(CheckoutResult {
                r#type: -1,
                data: json!(true),
            });
        }

        let method = input
            .method
            .ok_or_else(|| ApiError::legacy("Payment method is not available"))?;
        let payment = sqlx::query_as::<_, PaymentForCheckout>(
            r#"
            SELECT
                id,
                payment,
                enable,
                uuid,
                CAST(config AS CHAR) AS config,
                notify_domain,
                handling_fee_fixed,
                CAST(handling_fee_percent AS DOUBLE) AS handling_fee_percent
            FROM v2_payment
            WHERE id = ?
            LIMIT 1
            "#,
        )
        .bind(method)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::legacy("Payment method is not available"))?;
        if payment.enable != 1 {
            return Err(ApiError::legacy("Payment method is not available"));
        }

        let handling_amount = calculate_handling_amount(&order, &payment);
        sqlx::query(
            r#"
            UPDATE v2_order
            SET payment_id = ?, handling_amount = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(payment.id)
        .bind(handling_amount)
        .bind(Utc::now().timestamp())
        .bind(order.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        let payment_order = PaymentOrder {
            notify_url: payment_notify_url(&self.config, &payment),
            return_url: payment_return_url(&self.config, &order.trade_no),
            trade_no: order.trade_no,
            total_amount: order.total_amount + handling_amount.unwrap_or_default(),
            user_id,
            stripe_token: input.token,
        };
        self.pay_with_gateway(&payment, &payment_order).await
    }

    pub async fn handle_payment_notify(
        &self,
        method: &str,
        uuid: &str,
        input: PaymentNotifyInput,
    ) -> Result<PaymentNotifyResponse, ApiError> {
        // Laravel PaymentController::notify wraps the whole flow in try/catch and
        // re-aborts ANY failure (gate lookup, gateway verify, order handling, DB
        // errors) as a uniform `abort(500, 'fail')`, never leaking the internal
        // reason. Only the gateway's own direct response (Ignored) or the verified
        // custom_result/'success' escape the catch.
        match self.run_payment_notify(method, uuid, input).await {
            Ok(response) => Ok(response),
            Err(error) => {
                tracing::warn!(method, uuid, ?error, "payment notify failed");
                Err(ApiError::legacy("fail"))
            }
        }
    }

    async fn run_payment_notify(
        &self,
        method: &str,
        uuid: &str,
        input: PaymentNotifyInput,
    ) -> Result<PaymentNotifyResponse, ApiError> {
        let payment = sqlx::query_as::<_, PaymentForCheckout>(
            r#"
            SELECT
                id,
                payment,
                enable,
                uuid,
                CAST(config AS CHAR) AS config,
                notify_domain,
                handling_fee_fixed,
                CAST(handling_fee_percent AS DOUBLE) AS handling_fee_percent
            FROM v2_payment
            WHERE payment = ? AND uuid = ?
            LIMIT 1
            "#,
        )
        .bind(method)
        .bind(uuid)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("gate is not found"))?;
        if payment.enable != 1 {
            return Err(ApiError::legacy("gate is not enable"));
        }

        let outcome = self.verify_payment_notify(&payment, &input).await?;
        let PaymentNotifyOutcome::Verified(verified) = outcome else {
            return Ok(PaymentNotifyResponse {
                body: match outcome {
                    PaymentNotifyOutcome::Ignored(body) => body,
                    PaymentNotifyOutcome::Verified(_) => unreachable!(),
                },
                paid_notice: None,
            });
        };
        let paid_notice = self
            .paid_by_trade_no(&verified.trade_no, &verified.callback_no)
            .await?;
        Ok(PaymentNotifyResponse {
            body: verified
                .custom_result
                .unwrap_or_else(|| "success".to_string()),
            paid_notice,
        })
    }

    async fn pay_with_gateway(
        &self,
        payment: &PaymentForCheckout,
        order: &PaymentOrder,
    ) -> Result<CheckoutResult, ApiError> {
        let provider = require_payment_provider(&payment.payment)?;
        match provider.code {
            "EPay" => epay_pay(payment, order),
            "MGate" => mgate_pay(payment, order).await,
            "BEasyPaymentUSDT" => bepusdt_pay(payment, order).await,
            "CoinPayments" => coinpayments_pay(payment, order),
            "Coinbase" => coinbase_pay(payment, order).await,
            "BTCPay" => btcpay_pay(payment, order).await,
            "WechatPayNative" => wechat_pay_native_pay(payment, order).await,
            "AlipayF2F" => alipay_f2f_pay(&self.config, payment, order).await,
            "StripeCredit" => stripe_credit_pay(payment, order).await,
            "StripeAlipay" => stripe_source_pay(payment, order, "alipay").await,
            "StripeWepay" => stripe_source_pay(payment, order, "wechat").await,
            "StripeCheckout" => stripe_checkout_pay(payment, order).await,
            "StripeALL" => stripe_all_pay(self, payment, order).await,
            _ => unreachable!("payment provider manifest and checkout dispatch diverged"),
        }
    }

    async fn verify_payment_notify(
        &self,
        payment: &PaymentForCheckout,
        input: &PaymentNotifyInput,
    ) -> Result<PaymentNotifyOutcome, ApiError> {
        let provider = require_payment_provider(&payment.payment)?;
        match provider.code {
            "EPay" => epay_notify(payment, &input.params),
            "MGate" => mgate_notify(payment, &input.params),
            "BEasyPaymentUSDT" => bepusdt_notify(payment, &input.params),
            "CoinPayments" => coinpayments_notify(payment, input),
            "Coinbase" => coinbase_notify(payment, input),
            "BTCPay" => btcpay_notify(payment, input).await,
            "WechatPayNative" => wechat_pay_native_notify(payment, input),
            "AlipayF2F" => alipay_f2f_notify(payment, &input.params),
            "StripeCredit" | "StripeAlipay" | "StripeWepay" => {
                stripe_source_notify(payment, input).await
            }
            "StripeCheckout" => stripe_checkout_notify(payment, input),
            "StripeALL" => stripe_all_notify(payment, input),
            _ => unreachable!("payment provider manifest and notify dispatch diverged"),
        }
    }

    async fn user_email(&self, user_id: i64) -> Result<Option<String>, ApiError> {
        let email =
            sqlx::query_scalar::<_, String>("SELECT email FROM v2_user WHERE id = ? LIMIT 1")
                .bind(user_id)
                .fetch_optional(&self.db)
                .await?;
        Ok(email)
    }

    pub async fn paid_manually(&self, trade_no: &str) -> Result<(), ApiError> {
        let mut tx = self.db.begin().await?;
        let Some(order) = sqlx::query_as::<_, OrderForCheckout>(
            r#"
            SELECT
                id,
                user_id,
                plan_id,
                `type`,
                period,
                trade_no,
                total_amount,
                refund_amount,
                surplus_order_ids
            FROM v2_order
            WHERE trade_no = ?
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Err(ApiError::legacy("订单不存在"));
        };
        let status: i8 = sqlx::query_scalar("SELECT status FROM v2_order WHERE id = ?")
            .bind(order.id)
            .fetch_one(&mut *tx)
            .await?;
        if status != 0 {
            return Err(ApiError::legacy("只能对待支付的订单进行操作"));
        }

        mark_order_paid(
            &mut tx,
            order.id,
            "manual_operation",
            Utc::now().timestamp(),
        )
        .await?;
        self.open_order_in_tx(&mut tx, order).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn handle_pending_order(&self, trade_no: &str) -> Result<(), ApiError> {
        let mut tx = self.db.begin().await?;
        let Some(order) = sqlx::query_as::<_, OrderForCheckout>(
            r#"
            SELECT
                id,
                user_id,
                plan_id,
                `type`,
                period,
                trade_no,
                total_amount,
                refund_amount,
                surplus_order_ids
            FROM v2_order
            WHERE trade_no = ?
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&mut *tx)
        .await?
        else {
            tx.commit().await?;
            return Ok(());
        };

        let (status, created_at, balance_amount) = sqlx::query_as::<_, (i8, i64, Option<i32>)>(
            "SELECT status, created_at, balance_amount FROM v2_order WHERE id = ?",
        )
        .bind(order.id)
        .fetch_one(&mut *tx)
        .await?;

        match status {
            0 if created_at <= Utc::now().timestamp() - 7200 => {
                sqlx::query("UPDATE v2_order SET status = 2, updated_at = ? WHERE id = ?")
                    .bind(Utc::now().timestamp())
                    .bind(order.id)
                    .execute(&mut *tx)
                    .await?;
                if let Some(balance_amount) = balance_amount.filter(|amount| *amount > 0) {
                    sqlx::query(
                        "UPDATE v2_user SET balance = balance + ?, updated_at = ? WHERE id = ?",
                    )
                    .bind(balance_amount)
                    .bind(Utc::now().timestamp())
                    .bind(order.user_id)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            1 => {
                self.open_order_in_tx(&mut tx, order).await?;
            }
            _ => {}
        }

        tx.commit().await?;
        Ok(())
    }

    async fn paid_by_trade_no(
        &self,
        trade_no: &str,
        callback_no: &str,
    ) -> Result<Option<PaidOrderNotice>, ApiError> {
        // Commit the paid mark in its OWN transaction first, mirroring Laravel's
        // OrderService::paid() which save()s status=1 before dispatching OrderHandleJob.
        // A gateway-confirmed payment must be durable even if opening the order later
        // fails; folding the open into the same transaction (as before) meant a transient
        // error while granting the plan would roll back real, already-received money.
        let total_amount;
        {
            let mut tx = self.db.begin().await?;
            let Some((order_id, status, amount)) =
                sqlx::query_as::<_, (i64, i8, i64)>(
                    "SELECT id, status, total_amount FROM v2_order WHERE trade_no = ? LIMIT 1 FOR UPDATE",
                )
                .bind(trade_no)
                .fetch_optional(&mut *tx)
                .await?
            else {
                return Err(ApiError::legacy("Order does not exist"));
            };
            if status != 0 {
                // Already paid/opened/cancelled: idempotent no-op, safe on gateway replay.
                // Laravel's `handle()` returns early here without the `成功收款` message, so
                // report `None` and the caller suppresses the admin notification.
                tx.commit().await?;
                return Ok(None);
            }
            mark_order_paid(&mut tx, order_id, callback_no, Utc::now().timestamp()).await?;
            tx.commit().await?;
            total_amount = amount;
        }

        // Best-effort immediate open for responsiveness. On failure the order stays
        // durably paid (status=1) and check:order (handle_pending_order) re-opens it every
        // minute; do NOT surface the error to the gateway, which would re-deliver and then
        // short-circuit on the status!=0 guard above without ever retrying the open.
        if let Err(error) = self.handle_pending_order(trade_no).await {
            tracing::error!(
                trade_no,
                ?error,
                "order marked paid but opening failed; check_order will retry"
            );
        }
        Ok(Some(PaidOrderNotice {
            trade_no: trade_no.to_string(),
            total_amount,
        }))
    }

    async fn build_deposit_order(
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
            total_amount: amount as f64,
            discount_amount: None,
            surplus_amount: None,
            refund_amount: None,
            balance_amount: None,
            surplus_order_ids: None,
            invite_user_id: None,
            commission_balance: 0.0,
        };
        self.set_invite(tx, &user, &mut draft).await?;
        Ok(draft)
    }

    async fn build_plan_order(
        &self,
        tx: &mut Transaction<'_, MySql>,
        mut user: UserForOrder,
        plan_id: i32,
        period: &str,
        coupon_code: Option<&str>,
        trade_no: String,
    ) -> Result<DraftOrder, ApiError> {
        let plan = find_plan_for_update(tx, plan_id)
            .await?
            .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
        if user.plan_id != Some(plan.id)
            && !have_capacity(tx, plan.id, plan.capacity_limit).await?
            && period != "reset_price"
        {
            return Err(ApiError::legacy("Current product is sold out"));
        }
        let Some(price) = plan_period_price(&plan, period) else {
            return Err(ApiError::legacy(
                "This payment period cannot be purchased, please choose another period",
            ));
        };
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
            total_amount: price as f64,
            discount_amount: None,
            surplus_amount: None,
            refund_amount: None,
            balance_amount: None,
            surplus_order_ids: None,
            invite_user_id: None,
            commission_balance: 0.0,
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
            1 => coupon.value as f64,
            2 => draft.total_amount * (coupon.value as f64 / 100.0),
            _ => 0.0,
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
                draft.total_amount = 0.0;
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
            let total_traffic_gib = user.transfer_enable as f64 / GIB as f64;
            if total_traffic_gib <= 0.0 {
                return Ok(());
            }
            let paid_total =
                last_order.total_amount + last_order.balance_amount.unwrap_or_default();
            if paid_total <= 0 {
                return Ok(());
            }
            let unused_traffic_gib = (user.transfer_enable - (user.u + user.d)) as f64 / GIB as f64;
            let remaining_ratio = unused_traffic_gib / total_traffic_gib;
            draft.surplus_amount = Some(((paid_total as f64) * remaining_ratio).max(0.0));
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
            let order_end_time = add_months(row.created_at, months);
            if order_end_time < now {
                continue;
            }
            last_validate_at =
                Some(last_validate_at.map_or(row.created_at, |last| last.max(row.created_at)));
            order_month_sum += months;
            order_amount_sum += i64::from(row.total_amount)
                + i64::from(row.balance_amount.unwrap_or_default())
                + i64::from(row.surplus_amount.unwrap_or_default())
                - i64::from(row.refund_amount.unwrap_or_default());
        }
        let Some(last_validate_at) = last_validate_at else {
            return Ok(());
        };
        let expired_at_by_order = add_months(last_validate_at, order_month_sum);
        let Some(expired_at_by_user) = user.expired_at else {
            return Ok(());
        };
        if expired_at_by_order < now || expired_at_by_user < now {
            return Ok(());
        }
        let order_surplus_second = expired_at_by_user - now;
        let order_range_second = expired_at_by_order - last_validate_at;
        if order_range_second <= 0 || user.transfer_enable <= 0 {
            return Ok(());
        }
        let remaining_traffic_ratio =
            (user.transfer_enable - (user.u + user.d)) as f64 / user.transfer_enable as f64;
        let avg_price_per_second = order_amount_sum as f64 / order_range_second as f64;
        let surplus = if order_range_second <= 31 * 86_400 {
            let remaining_expired_time_ratio =
                order_surplus_second as f64 / order_range_second as f64;
            let surplus_ratio = remaining_expired_time_ratio.min(remaining_traffic_ratio);
            avg_price_per_second * order_surplus_second as f64 * surplus_ratio
        } else {
            let month_seconds = 30 * 86_400;
            let first_month_remain_seconds = order_surplus_second % month_seconds;
            let surplus_ratio = (first_month_remain_seconds as f64 / month_seconds as f64)
                .min(remaining_traffic_ratio);
            let later_months_seconds = order_surplus_second - first_month_remain_seconds;
            avg_price_per_second * month_seconds as f64 * surplus_ratio
                + avg_price_per_second * later_months_seconds as f64
        };
        draft.surplus_amount = Some(surplus.max(0.0));
        draft.surplus_order_ids = Some(rows.into_iter().map(|row| row.id).collect());
        Ok(())
    }

    async fn apply_balance(
        &self,
        tx: &mut Transaction<'_, MySql>,
        user: &mut UserForOrder,
        draft: &mut DraftOrder,
    ) -> Result<(), ApiError> {
        if user.balance <= 0 || draft.total_amount <= 0.0 {
            return Ok(());
        }
        let use_balance = (user.balance as f64).min(draft.total_amount);
        // Laravel passes the (still-float) deduction to `UserService::addBalance(int $balance)`,
        // whose `int` parameter coerces the float by TRUNCATION toward zero before subtracting
        // it from the balance column. So the actual balance deduction is `trunc(use_balance)`,
        // NOT a round — e.g. a 0.5-cent total leaves the balance untouched. The recorded
        // `balance_amount` field, by contrast, is stored via Eloquent save() and DOES get
        // MySQL-rounded (round_cents at insert), so the two can legitimately differ by a cent.
        let use_balance_cents = use_balance.trunc() as i32;
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
        if draft.total_amount <= 0.0 {
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
        let is_commission = match inviter.commission_type {
            0 => !self.config.commission_first_time_enable || !has_valid_order,
            1 => true,
            2 => !has_valid_order,
            _ => false,
        };
        if !is_commission {
            return Ok(());
        }
        let rate = inviter
            .commission_rate
            .filter(|rate| *rate > 0)
            .unwrap_or(self.config.invite_commission);
        draft.commission_balance = draft.total_amount * (rate as f64 / 100.0);
        Ok(())
    }

    async fn open_order_in_tx(
        &self,
        tx: &mut Transaction<'_, MySql>,
        order: OrderForCheckout,
    ) -> Result<(), ApiError> {
        if order.r#type == 9 {
            let bonus = self.config.deposit_bonus(order.total_amount);
            sqlx::query(
                r#"
                UPDATE v2_user
                SET balance = balance + ?, updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(order.total_amount + bonus)
            .bind(Utc::now().timestamp())
            .bind(order.user_id)
            .execute(&mut **tx)
            .await?;
            sqlx::query("UPDATE v2_order SET status = 3, updated_at = ? WHERE id = ?")
                .bind(Utc::now().timestamp())
                .bind(order.id)
                .execute(&mut **tx)
                .await?;
            return Ok(());
        }

        let mut user = find_user_for_order(tx, order.user_id).await?;
        let plan = find_plan_for_update(tx, order.plan_id)
            .await?
            .ok_or_else(|| ApiError::legacy("Subscription plan does not exist"))?;
        if let Some(refund_amount) = order.refund_amount.filter(|amount| *amount > 0) {
            user.balance += refund_amount;
        }
        let surplus_ids = parse_i64_json_list(order.surplus_order_ids.as_deref());
        if let Some(ids) = surplus_ids.as_deref() {
            mark_surplus_orders(tx, ids).await?;
        }

        match order.period.as_str() {
            "onetime_price" => buy_by_one_time(&mut user, &order, &plan, surplus_ids.is_some()),
            "reset_price" => reset_traffic(&mut user),
            period => buy_by_period(&mut user, &order, &plan, period),
        }
        match order.r#type {
            1 if self.config.new_order_event_id == 1 => reset_traffic(&mut user),
            2 if self.config.renew_order_event_id == 1 => reset_traffic(&mut user),
            3 if self.config.change_order_event_id == 1 => reset_traffic(&mut user),
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

fn require_payment_provider(method: &str) -> Result<&'static PaymentProviderManifest, ApiError> {
    payment_provider_manifest(method)
        .ok_or_else(|| ApiError::legacy(format!("Payment gateway {method} is not supported")))
}

async fn find_user_for_order(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
) -> Result<UserForOrder, ApiError> {
    sqlx::query_as::<_, UserForOrder>(USER_FOR_ORDER_SQL)
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))
}

async fn find_plan_for_update(
    tx: &mut Transaction<'_, MySql>,
    plan_id: i32,
) -> Result<Option<PlanRow>, sqlx::Error> {
    sqlx::query_as::<_, PlanRow>(
        r#"
        SELECT
            id,
            group_id,
            transfer_enable,
            device_limit,
            name,
            speed_limit,
            `show`,
            sort,
            renew,
            content,
            month_price,
            quarter_price,
            half_year_price,
            year_price,
            two_year_price,
            three_year_price,
            onetime_price,
            reset_price,
            reset_traffic_method,
            capacity_limit,
            created_at,
            updated_at
        FROM v2_plan
        WHERE id = ?
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(plan_id)
    .fetch_optional(&mut **tx)
    .await
}

async fn have_capacity(
    tx: &mut Transaction<'_, MySql>,
    plan_id: i32,
    capacity_limit: Option<i32>,
) -> Result<bool, sqlx::Error> {
    let Some(capacity_limit) = capacity_limit else {
        return Ok(true);
    };
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM v2_user
        WHERE plan_id = ?
          AND (expired_at >= UNIX_TIMESTAMP() OR expired_at IS NULL)
        "#,
    )
    .bind(plan_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok((capacity_limit - count as i32) > 0)
}

async fn validate_coupon(
    tx: &mut Transaction<'_, MySql>,
    coupon: &CouponRow,
    draft: &DraftOrder,
) -> Result<(), ApiError> {
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

async fn insert_order(
    tx: &mut Transaction<'_, MySql>,
    draft: &DraftOrder,
    now: i64,
) -> Result<(), sqlx::Error> {
    let surplus_order_ids = draft
        .surplus_order_ids
        .as_ref()
        .and_then(|ids| serde_json::to_string(ids).ok());
    sqlx::query(
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
    .bind(round_cents(draft.total_amount))
    .bind(draft.discount_amount.map(round_cents))
    .bind(draft.surplus_amount.map(round_cents))
    .bind(draft.refund_amount.map(round_cents))
    .bind(draft.balance_amount.map(round_cents))
    .bind(surplus_order_ids)
    .bind(round_cents(draft.commission_balance))
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn mark_order_paid(
    tx: &mut Transaction<'_, MySql>,
    order_id: i64,
    callback_no: &str,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE v2_order
        SET status = 1, paid_at = ?, callback_no = ?, updated_at = ?
        WHERE id = ? AND status = 0
        "#,
    )
    .bind(now)
    .bind(callback_no)
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

fn epay_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let key = config_required(&config, "key")?;
    let mut params = BTreeMap::from([
        ("money".to_string(), amount_yuan_string(order.total_amount)),
        ("name".to_string(), order.trade_no.clone()),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("return_url".to_string(), order.return_url.clone()),
        ("out_trade_no".to_string(), order.trade_no.clone()),
        ("pid".to_string(), config_required(&config, "pid")?),
    ]);
    if let Some(payment_type) = config_string(&config, "type").filter(|value| !value.is_empty()) {
        params.insert("type".to_string(), payment_type);
    }
    let signature = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&params), key))
    );
    params.insert("sign".to_string(), signature);
    params.insert("sign_type".to_string(), "MD5".to_string());
    let base_url = config_required(&config, "url")?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(format!(
            "{}/submit.php?{}",
            base_url.trim_end_matches('/'),
            form_query(&params)?
        )),
    })
}

fn epay_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let sign = params
        .get("sign")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify signature is missing"))?;
    let config = payment_config(payment)?;
    let key = config_required(&config, "key")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "sign" && key.as_str() != "sign_type")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), key))
    );
    if expected != *sign {
        return Err(ApiError::legacy("Payment notify signature is invalid"));
    }
    if params
        .get("trade_status")
        .map(String::as_str)
        .unwrap_or_default()
        != "TRADE_SUCCESS"
    {
        return Ok(PaymentNotifyOutcome::Ignored("fail".to_string()));
    }
    let trade_no = signed
        .remove("out_trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?;
    let callback_no = signed
        .remove("trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no,
        callback_no,
        custom_result: None,
    }))
}

async fn mgate_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let secret = config_required(&config, "mgate_app_secret")?;
    let mut params = BTreeMap::from([
        ("out_trade_no".to_string(), order.trade_no.clone()),
        ("total_amount".to_string(), order.total_amount.to_string()),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("return_url".to_string(), order.return_url.clone()),
        (
            "app_id".to_string(),
            config_required(&config, "mgate_app_id")?,
        ),
    ]);
    if let Some(currency) =
        config_string(&config, "mgate_source_currency").filter(|value| !value.is_empty())
    {
        params.insert("source_currency".to_string(), currency);
    }
    let signature = format!(
        "{:x}",
        md5::compute(format!("{}{}", form_query(&params)?, secret))
    );
    params.insert("sign".to_string(), signature);
    let base_url = config_required(&config, "mgate_url")?;
    let result = payment_http_client("MGate")?
        .post(format!(
            "{}/v1/gateway/fetch",
            base_url.trim_end_matches('/')
        ))
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_query(&params)?)
        .send()
        .await
        .map_err(|_| ApiError::legacy("网络异常"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|_| ApiError::legacy("接口请求失败"))?;

    if result
        .pointer("/data/trade_no")
        .and_then(|value| value.as_str())
        .is_none()
    {
        return Err(ApiError::legacy(payment_gateway_message(
            &result,
            "接口请求失败",
        )));
    }
    let pay_url = result
        .pointer("/data/pay_url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("接口请求失败"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(pay_url),
    })
}

async fn bepusdt_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let api_token = config_required(&config, "bepusdt_apitoken")?;
    let mut signed = BTreeMap::from([
        ("amount".to_string(), amount_yuan_string(order.total_amount)),
        (
            "trade_type".to_string(),
            config_required(&config, "bepusdt_trade_type")?,
        ),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("order_id".to_string(), order.trade_no.clone()),
        ("redirect_url".to_string(), order.return_url.clone()),
    ]);
    let signature = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), api_token))
    );
    signed.insert("signature".to_string(), signature);

    let mut json_params = serde_json::Map::new();
    for (key, value) in &signed {
        if key == "amount" {
            let amount = value
                .parse::<f64>()
                .map_err(|_| ApiError::internal("invalid bepusdt amount"))?;
            json_params.insert(key.clone(), json!(amount));
        } else {
            json_params.insert(key.clone(), json!(value));
        }
    }

    let base_url = config_required(&config, "bepusdt_url")?;
    let result = payment_http_client("BEPUSDT")?
        .post(format!(
            "{}/api/v1/order/create-transaction",
            base_url.trim_end_matches('/')
        ))
        .json(&json_params)
        .send()
        .await
        .map_err(|_| ApiError::legacy("网络异常"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|_| ApiError::legacy("接口请求失败"))?;
    if result
        .get("status_code")
        .and_then(|value| value.as_i64())
        .unwrap_or_default()
        != 200
    {
        return Err(ApiError::legacy(format!(
            "Failed to create order. Error: {}",
            payment_gateway_message(&result, "接口请求失败")
        )));
    }
    let payment_url = result
        .pointer("/data/payment_url")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("接口请求失败"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(payment_url),
    })
}

fn mgate_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let sign = params
        .get("sign")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify signature is missing"))?;
    let config = payment_config(payment)?;
    let secret = config_required(&config, "mgate_app_secret")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "sign")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = format!(
        "{:x}",
        md5::compute(format!("{}{}", form_query(&signed)?, secret))
    );
    if expected != *sign {
        return Err(ApiError::legacy("Payment notify signature is invalid"));
    }
    let trade_no = signed
        .remove("out_trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?;
    let callback_no = signed
        .remove("trade_no")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no,
        callback_no,
        custom_result: None,
    }))
}

fn bepusdt_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let sign = params
        .get("signature")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify signature is missing"))?;
    let config = payment_config(payment)?;
    let api_token = config_required(&config, "bepusdt_apitoken")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "signature")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let expected = format!(
        "{:x}",
        md5::compute(format!("{}{}", canonical_query(&signed), api_token))
    );
    if expected != *sign {
        return Ok(PaymentNotifyOutcome::Ignored(
            "cannot pass verification".to_string(),
        ));
    }
    if params.get("status").map(String::as_str).unwrap_or_default() != "2" {
        return Ok(PaymentNotifyOutcome::Ignored("failed".to_string()));
    }
    let trade_no = signed
        .remove("order_id")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?;
    let callback_no = signed
        .remove("trade_id")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no,
        callback_no,
        custom_result: Some("ok".to_string()),
    }))
}

async fn stripe_credit_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let currency = config_required(&config, "currency")?;
    let exchange = exchange_cny_to(&currency).await?;
    let token = order
        .stripe_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::legacy("Payment failed. Please check your credit card information")
        })?;
    let params = BTreeMap::from([
        (
            "amount".to_string(),
            stripe_amount(order.total_amount, exchange).to_string(),
        ),
        ("currency".to_string(), currency.clone()),
        ("source".to_string(), token.to_string()),
        ("metadata[user_id]".to_string(), order.user_id.to_string()),
        ("metadata[out_trade_no]".to_string(), order.trade_no.clone()),
        ("metadata[identifier]".to_string(), String::new()),
    ]);
    let charge = stripe_post(&config, "charges", &params).await?;
    if charge.get("paid").and_then(Value::as_bool) != Some(true) {
        return Err(ApiError::legacy(
            "Payment failed. Please check your credit card information",
        ));
    }
    Ok(CheckoutResult {
        r#type: 2,
        data: json!(true),
    })
}

async fn stripe_source_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
    source_type: &str,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let currency = config_required(&config, "currency")?;
    let exchange = exchange_cny_to(&currency).await?;
    let params = BTreeMap::from([
        (
            "amount".to_string(),
            stripe_amount(order.total_amount, exchange).to_string(),
        ),
        ("currency".to_string(), currency),
        ("type".to_string(), source_type.to_string()),
        ("statement_descriptor".to_string(), order.trade_no.clone()),
        ("metadata[user_id]".to_string(), order.user_id.to_string()),
        ("metadata[out_trade_no]".to_string(), order.trade_no.clone()),
        ("metadata[identifier]".to_string(), String::new()),
        ("redirect[return_url]".to_string(), order.return_url.clone()),
    ]);
    let source = stripe_post(&config, "sources", &params).await?;
    let (result_type, data) = if source_type == "wechat" {
        (
            0,
            value_path_str(&source, &["wechat", "qr_code_url"])
                .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?,
        )
    } else {
        (
            1,
            value_path_str(&source, &["redirect", "url"])
                .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?,
        )
    };
    Ok(CheckoutResult {
        r#type: result_type,
        data: json!(data),
    })
}

async fn stripe_checkout_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let currency = config_required(&config, "currency")?;
    let exchange = exchange_cny_to(&currency).await?;
    let custom_field_name = config_string(&config, "stripe_custom_field_name")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Contact Infomation".to_string());
    let params = BTreeMap::from([
        ("success_url".to_string(), order.return_url.clone()),
        ("cancel_url".to_string(), order.return_url.clone()),
        ("client_reference_id".to_string(), order.trade_no.clone()),
        ("line_items[0][price_data][currency]".to_string(), currency),
        (
            "line_items[0][price_data][product_data][name]".to_string(),
            order.trade_no.clone(),
        ),
        (
            "line_items[0][price_data][unit_amount]".to_string(),
            stripe_amount(order.total_amount, exchange).to_string(),
        ),
        ("line_items[0][quantity]".to_string(), "1".to_string()),
        ("mode".to_string(), "payment".to_string()),
        ("invoice_creation[enabled]".to_string(), "true".to_string()),
        (
            "phone_number_collection[enabled]".to_string(),
            "true".to_string(),
        ),
        (
            "custom_fields[0][key]".to_string(),
            "contactinfo".to_string(),
        ),
        (
            "custom_fields[0][label][type]".to_string(),
            "custom".to_string(),
        ),
        (
            "custom_fields[0][label][custom]".to_string(),
            custom_field_name,
        ),
        ("custom_fields[0][type]".to_string(), "text".to_string()),
    ]);
    let session = stripe_post(&config, "checkout/sessions", &params).await?;
    let url = session
        .get("url")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(url),
    })
}

async fn stripe_all_pay(
    service: &OrderService,
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let method = config_required(&config, "payment_method")?;
    if method == "cards" {
        return stripe_all_cards_pay(service, &config, order).await;
    }
    let currency = config_required(&config, "currency")?;
    let exchange = exchange_cny_to(&currency).await?;
    let payment_method = stripe_post(
        &config,
        "payment_methods",
        &BTreeMap::from([("type".to_string(), method.clone())]),
    )
    .await?;
    let payment_method_id = payment_method
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    let user_email = service.user_email(order.user_id).await?.unwrap_or_default();
    let mut params = BTreeMap::from([
        (
            "amount".to_string(),
            stripe_amount(order.total_amount, exchange).to_string(),
        ),
        ("currency".to_string(), currency),
        ("confirm".to_string(), "true".to_string()),
        ("payment_method".to_string(), payment_method_id.to_string()),
        (
            "automatic_payment_methods[enabled]".to_string(),
            "true".to_string(),
        ),
        (
            "statement_descriptor".to_string(),
            format!(
                "user-#{}-{}",
                order.user_id,
                right_chars(&order.trade_no, 8)
            ),
        ),
        ("metadata[user_id]".to_string(), order.user_id.to_string()),
        ("metadata[customer_email]".to_string(), user_email),
        ("metadata[out_trade_no]".to_string(), order.trade_no.clone()),
        ("return_url".to_string(), order.return_url.clone()),
    ]);
    if method == "wechat_pay" {
        params.insert(
            "payment_method_options[wechat_pay][client]".to_string(),
            "web".to_string(),
        );
    }
    let intent = stripe_post(&config, "payment_intents", &params).await?;
    let (result_type, data) = match method.as_str() {
        "alipay" => (
            1,
            value_path_str(&intent, &["next_action", "alipay_handle_redirect", "url"])
                .ok_or_else(|| ApiError::legacy("unable get Alipay redirect url"))?,
        ),
        "wechat_pay" => (
            0,
            value_path_str(
                &intent,
                &["next_action", "wechat_pay_display_qr_code", "data"],
            )
            .ok_or_else(|| ApiError::legacy("unable get WeChat Pay redirect url"))?,
        ),
        _ => return Err(ApiError::legacy("Payment gateway request failed")),
    };
    Ok(CheckoutResult {
        r#type: result_type,
        data: json!(data),
    })
}

async fn stripe_all_cards_pay(
    service: &OrderService,
    config: &serde_json::Map<String, Value>,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let currency = config_required(config, "currency")?;
    let exchange = exchange_cny_to(&currency).await?;
    let mut params = BTreeMap::from([
        ("success_url".to_string(), order.return_url.clone()),
        ("client_reference_id".to_string(), order.trade_no.clone()),
        ("payment_method_types[0]".to_string(), "card".to_string()),
        ("line_items[0][price_data][currency]".to_string(), currency),
        (
            "line_items[0][price_data][unit_amount]".to_string(),
            stripe_amount(order.total_amount, exchange).to_string(),
        ),
        (
            "line_items[0][price_data][product_data][name]".to_string(),
            format!(
                "user-#{}-{}",
                order.user_id,
                right_chars(&order.trade_no, 8)
            ),
        ),
        ("line_items[0][quantity]".to_string(), "1".to_string()),
        ("mode".to_string(), "payment".to_string()),
        ("invoice_creation[enabled]".to_string(), "true".to_string()),
        (
            "phone_number_collection[enabled]".to_string(),
            "false".to_string(),
        ),
    ]);
    if let Some(email) = service.user_email(order.user_id).await? {
        params.insert("customer_email".to_string(), email);
    }
    let session = stripe_post_with_config(config, "checkout/sessions", &params).await?;
    let url = session
        .get("url")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(url),
    })
}

async fn stripe_source_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let event = stripe_event(&config, input)?;
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "source.chargeable" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            let source_id = object
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            let amount = object
                .get("amount")
                .and_then(Value::as_i64)
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            let currency = object
                .get("currency")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            let mut params = BTreeMap::from([
                ("amount".to_string(), amount.to_string()),
                ("currency".to_string(), currency.to_string()),
                ("source".to_string(), source_id.to_string()),
            ]);
            add_metadata_params(&mut params, "metadata", object.get("metadata"));
            stripe_post_with_config(&config, "charges", &params).await?;
            Ok(PaymentNotifyOutcome::Ignored("success".to_string()))
        }
        "charge.succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            if object.get("status").and_then(Value::as_str) != Some("succeeded") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            let trade_no = value_path_str(object, &["metadata", "out_trade_no"])
                .or_else(|| value_path_str(object, &["source", "metadata", "out_trade_no"]))
                .ok_or_else(|| ApiError::legacy("order error"))?;
            let callback_no = object
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
                trade_no,
                callback_no: callback_no.to_string(),
                custom_result: None,
            }))
        }
        _ => Err(ApiError::legacy("event is not support")),
    }
}

fn stripe_checkout_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let event = stripe_event(&config, input)?;
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "checkout.session.completed" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            if object.get("payment_status").and_then(Value::as_str) != Some("paid") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            stripe_session_verified(object)
        }
        "checkout.session.async_payment_succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("event is not support"))?;
            stripe_session_verified(object)
        }
        _ => Err(ApiError::legacy("event is not support")),
    }
}

fn stripe_all_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let event = stripe_event(&config, input)?;
    match event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "payment_intent.succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("webhook events are not supported"))?;
            if object.get("status").and_then(Value::as_str) != Some("succeeded") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            let trade_no = value_path_str(object, &["metadata", "out_trade_no"])
                .ok_or_else(|| ApiError::legacy("order error"))?;
            let callback_no = object
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::legacy("webhook events are not supported"))?;
            Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
                trade_no,
                callback_no: callback_no.to_string(),
                custom_result: None,
            }))
        }
        "checkout.session.completed" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("webhook events are not supported"))?;
            if object.get("payment_status").and_then(Value::as_str) != Some("paid") {
                return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
            }
            stripe_session_verified(object)
        }
        "checkout.session.async_payment_succeeded" => {
            let object = event
                .pointer("/data/object")
                .ok_or_else(|| ApiError::legacy("webhook events are not supported"))?;
            stripe_session_verified(object)
        }
        _ => Err(ApiError::legacy("webhook events are not supported")),
    }
}

fn stripe_session_verified(object: &Value) -> Result<PaymentNotifyOutcome, ApiError> {
    let trade_no = object
        .get("client_reference_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("event is not support"))?;
    let callback_no = object
        .get("payment_intent")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("event is not support"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: trade_no.to_string(),
        callback_no: callback_no.to_string(),
        custom_result: None,
    }))
}

fn coinpayments_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let params = BTreeMap::from([
        ("cmd".to_string(), "_pay_simple".to_string()),
        ("reset".to_string(), "1".to_string()),
        (
            "merchant".to_string(),
            config_required(&config, "coinpayments_merchant_id")?,
        ),
        ("item_name".to_string(), order.trade_no.clone()),
        ("item_number".to_string(), order.trade_no.clone()),
        ("want_shipping".to_string(), "0".to_string()),
        (
            "currency".to_string(),
            config_required(&config, "coinpayments_currency")?,
        ),
        (
            "amountf".to_string(),
            format!("{:.2}", order.total_amount as f64 / 100.0),
        ),
        ("success_url".to_string(), url_origin(&order.return_url)),
        ("cancel_url".to_string(), order.return_url.clone()),
        ("ipn_url".to_string(), order.notify_url.clone()),
    ]);
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(format!(
            "https://www.coinpayments.net/index.php?{}",
            form_query(&params)?
        )),
    })
}

fn coinpayments_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let merchant = config_required(&config, "coinpayments_merchant_id")?;
    if input.params.get("merchant").map(|value| value.trim()) != Some(merchant.trim()) {
        return Err(ApiError::legacy("No or incorrect Merchant ID passed"));
    }
    let secret = config_required(&config, "coinpayments_ipn_secret")?;
    let signed = input
        .params
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let request = form_query(&signed)?;
    let expected = hmac_sha512_hex(secret.as_bytes(), request.as_bytes())?;
    let actual = header_value(&input.headers, "hmac")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    let status = input
        .params
        .get("status")
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or_default();
    if status >= 100 || status == 2 {
        return Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
            trade_no: required_param(&input.params, "item_number")?,
            callback_no: required_param(&input.params, "txn_id")?,
            custom_result: Some("IPN OK".to_string()),
        }));
    }
    if status < 0 {
        return Err(ApiError::legacy("Payment Timed Out or Error"));
    }
    Ok(PaymentNotifyOutcome::Ignored("IPN OK: pending".to_string()))
}

async fn coinbase_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let params = BTreeMap::from([
        ("name".to_string(), "订阅套餐".to_string()),
        (
            "description".to_string(),
            format!("订单号 {}", order.trade_no),
        ),
        ("pricing_type".to_string(), "fixed_price".to_string()),
        (
            "local_price[amount]".to_string(),
            format!("{:.2}", order.total_amount as f64 / 100.0),
        ),
        ("local_price[currency]".to_string(), "CNY".to_string()),
        ("metadata[outTradeNo]".to_string(), order.trade_no.clone()),
    ]);
    let result = payment_http_client("Coinbase")?
        .post(config_required(&config, "coinbase_url")?)
        .header(
            "X-CC-Api-Key",
            config_required(&config, "coinbase_api_key")?,
        )
        .header("X-CC-Version", "2018-03-22")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_query(&params)?)
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?
        .json::<Value>()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let hosted_url = result
        .pointer("/data/hosted_url")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("error!"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(hosted_url),
    })
}

fn coinbase_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let payload = trimmed_body(&input.body);
    let expected = hmac_sha256_hex(
        config_required(&config, "coinbase_webhook_key")?.as_bytes(),
        payload.as_bytes(),
    )?;
    let actual = header_value(&input.headers, "x-cc-webhook-signature")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    let value = serde_json::from_str::<Value>(&payload)
        .map_err(|_| ApiError::legacy("Payment notify body is invalid"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: value_path_str(&value, &["event", "data", "metadata", "outTradeNo"])
            .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?,
        callback_no: value_path_str(&value, &["event", "id"])
            .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?,
        custom_result: None,
    }))
}

async fn btcpay_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let payload = json!({
        "jsonResponse": true,
        "amount": format!("{:.2}", order.total_amount as f64 / 100.0),
        "currency": "CNY",
        "metadata": { "orderId": order.trade_no },
    });
    let base = config_required(&config, "btcpay_url")?;
    let store_id = config_required(&config, "btcpay_storeId")?;
    let result = payment_http_client("BTCPay")?
        .post(format!(
            "{}api/v1/stores/{store_id}/invoices",
            ensure_trailing_slash(&base)
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("token {}", config_required(&config, "btcpay_api_key")?),
        )
        .json(&payload)
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?
        .json::<Value>()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let checkout_link = result
        .get("checkoutLink")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("error!"))?;
    Ok(CheckoutResult {
        r#type: 1,
        data: json!(checkout_link),
    })
}

async fn btcpay_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let payload = trimmed_body(&input.body);
    let expected = format!(
        "sha256={}",
        hmac_sha256_hex(
            config_required(&config, "btcpay_webhook_key")?.as_bytes(),
            payload.as_bytes(),
        )?
    );
    let actual = header_value(&input.headers, "btcpay-sig")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    let body = serde_json::from_str::<Value>(&payload)
        .map_err(|_| ApiError::legacy("Payment notify body is invalid"))?;
    let invoice_id = body
        .get("invoiceId")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?;
    let base = config_required(&config, "btcpay_url")?;
    let store_id = config_required(&config, "btcpay_storeId")?;
    let invoice = payment_http_client("BTCPay")?
        .get(format!(
            "{}api/v1/stores/{store_id}/invoices/{invoice_id}",
            ensure_trailing_slash(&base)
        ))
        .header(
            reqwest::header::AUTHORIZATION,
            format!("token {}", config_required(&config, "btcpay_api_key")?),
        )
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?
        .json::<Value>()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: value_path_str(&invoice, &["metadata", "orderId"])
            .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?,
        callback_no: invoice_id.to_string(),
        custom_result: None,
    }))
}

async fn wechat_pay_native_pay(
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let api_key = config_required(&config, "api_key")?;
    let mut params = BTreeMap::from([
        ("appid".to_string(), config_required(&config, "app_id")?),
        ("mch_id".to_string(), config_required(&config, "mch_id")?),
        ("nonce_str".to_string(), Uuid::new_v4().simple().to_string()),
        ("body".to_string(), order.trade_no.clone()),
        ("out_trade_no".to_string(), order.trade_no.clone()),
        ("total_fee".to_string(), order.total_amount.to_string()),
        ("spbill_create_ip".to_string(), "0.0.0.0".to_string()),
        ("fee_type".to_string(), "CNY".to_string()),
        ("notify_url".to_string(), order.notify_url.clone()),
        ("trade_type".to_string(), "NATIVE".to_string()),
    ]);
    params.insert("sign".to_string(), wechat_sign(&params, &api_key));
    let response = payment_http_client("WechatPayNative")?
        .post("https://api.mch.weixin.qq.com/pay/unifiedorder")
        .header(reqwest::header::CONTENT_TYPE, "text/xml")
        .body(xml_from_params(&params))
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?
        .text()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let response_params = parse_xml_map(&response);
    if response_params.get("return_code").map(String::as_str) != Some("SUCCESS") {
        return Err(ApiError::legacy(
            response_params
                .get("return_msg")
                .cloned()
                .unwrap_or_else(|| "Payment gateway request failed".to_string()),
        ));
    }
    let code_url = response_params
        .get("code_url")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("Payment gateway request failed"))?;
    Ok(CheckoutResult {
        r#type: 0,
        data: json!(code_url),
    })
}

fn wechat_pay_native_notify(
    payment: &PaymentForCheckout,
    input: &PaymentNotifyInput,
) -> Result<PaymentNotifyOutcome, ApiError> {
    let config = payment_config(payment)?;
    let api_key = config_required(&config, "api_key")?;
    let sign = input
        .params
        .get("sign")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    let signed = input
        .params
        .iter()
        .filter(|(key, value)| key.as_str() != "sign" && !value.is_empty())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    if wechat_sign(&signed, &api_key) != *sign {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    if input.params.get("return_code").map(String::as_str) != Some("SUCCESS")
        || input.params.get("result_code").map(String::as_str) != Some("SUCCESS")
    {
        return Ok(PaymentNotifyOutcome::Ignored("FAIL".to_string()));
    }
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: required_param(&input.params, "out_trade_no")?,
        callback_no: required_param(&input.params, "transaction_id")?,
        custom_result: Some(
            "<xml><return_code><![CDATA[SUCCESS]]></return_code><return_msg><![CDATA[OK]]></return_msg></xml>"
                .to_string(),
        ),
    }))
}

async fn alipay_f2f_pay(
    app_config: &AppConfig,
    payment: &PaymentForCheckout,
    order: &PaymentOrder,
) -> Result<CheckoutResult, ApiError> {
    let config = payment_config(payment)?;
    let subject = config_string(&config, "product_name")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{} - 订阅", app_config.app_name));
    let biz_content = serde_json::to_string(&json!({
        "subject": subject,
        "out_trade_no": order.trade_no,
        "total_amount": order.total_amount as f64 / 100.0,
    }))
    .map_err(|_| ApiError::internal("failed to build alipay payload"))?;
    let mut params = BTreeMap::from([
        ("app_id".to_string(), config_required(&config, "app_id")?),
        ("method".to_string(), "alipay.trade.precreate".to_string()),
        ("charset".to_string(), "UTF-8".to_string()),
        ("sign_type".to_string(), "RSA2".to_string()),
        (
            "timestamp".to_string(),
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        ),
        ("biz_content".to_string(), biz_content),
        ("version".to_string(), "1.0".to_string()),
        ("_input_charset".to_string(), "UTF-8".to_string()),
        ("notify_url".to_string(), order.notify_url.clone()),
    ]);
    let signature = alipay_sign(
        &config_required(&config, "private_key")?,
        &canonical_query(&params),
    )?;
    params.insert("sign".to_string(), signature);
    let response = payment_http_client("AlipayF2F")?
        .get(format!(
            "https://openapi.alipay.com/gateway.do?{}",
            form_query(&params)?
        ))
        .send()
        .await
        .map_err(|_| ApiError::legacy("从支付宝请求失败"))?
        .json::<Value>()
        .await
        .map_err(|_| ApiError::legacy("从支付宝请求失败"))?;
    let response = response
        .get("alipay_trade_precreate_response")
        .ok_or_else(|| ApiError::legacy("从支付宝请求失败"))?;
    if response.get("msg").and_then(Value::as_str) != Some("Success") {
        return Err(ApiError::legacy(
            response
                .get("sub_msg")
                .and_then(Value::as_str)
                .unwrap_or("从支付宝请求失败"),
        ));
    }
    let qr_code = response
        .get("qr_code")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("获取付款二维码失败"))?;
    Ok(CheckoutResult {
        r#type: 0,
        data: json!(qr_code),
    })
}

fn alipay_f2f_notify(
    payment: &PaymentForCheckout,
    params: &HashMap<String, String>,
) -> Result<PaymentNotifyOutcome, ApiError> {
    if params.get("trade_status").map(String::as_str) != Some("TRADE_SUCCESS") {
        return Ok(PaymentNotifyOutcome::Ignored("success".to_string()));
    }
    let config = payment_config(payment)?;
    let sign = required_param(params, "sign")?;
    let mut signed = params
        .iter()
        .filter(|(key, _)| key.as_str() != "sign" && key.as_str() != "sign_type")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let public_key = config_required(&config, "public_key")?;
    if !alipay_verify(&public_key, &canonical_query(&signed), &sign)? {
        return Err(ApiError::legacy("verify error"));
    }
    Ok(PaymentNotifyOutcome::Verified(VerifiedPaymentNotify {
        trade_no: signed
            .remove("out_trade_no")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Payment notify trade_no is missing"))?,
        callback_no: signed
            .remove("trade_no")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Payment notify callback_no is missing"))?,
        custom_result: None,
    }))
}

fn payment_notify_url(config: &AppConfig, payment: &PaymentForCheckout) -> String {
    let path = format!(
        "/api/v1/guest/payment/notify/{}/{}",
        payment.payment, payment.uuid
    );
    if let Some(domain) = payment
        .notify_domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{}{}", domain.trim_end_matches('/'), path);
    }
    if let Some(app_url) = config
        .app_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{}{}", app_url.trim_end_matches('/'), path);
    }
    path
}

fn payment_return_url(config: &AppConfig, trade_no: &str) -> String {
    if let Some(app_url) = config
        .app_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{}/#/order/{trade_no}", app_url.trim_end_matches('/'));
    }
    format!("/#/order/{trade_no}")
}

fn payment_config(
    payment: &PaymentForCheckout,
) -> Result<serde_json::Map<String, serde_json::Value>, ApiError> {
    match serde_json::from_str::<serde_json::Value>(&payment.config)
        .map_err(|_| ApiError::legacy("Payment config is invalid"))?
    {
        serde_json::Value::Object(config) => Ok(config),
        _ => Err(ApiError::legacy("Payment config is invalid")),
    }
}

fn config_required(
    config: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, ApiError> {
    config_string(config, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy(format!("Payment config {key} is missing")))
}

fn config_string(config: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    match config.get(key)? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

fn amount_yuan_string(cents: i32) -> String {
    let mut amount = format!("{:.2}", cents as f64 / 100.0);
    while amount.contains('.') && amount.ends_with('0') {
        amount.pop();
    }
    if amount.ends_with('.') {
        amount.pop();
    }
    amount
}

async fn exchange_cny_to(currency: &str) -> Result<f64, ApiError> {
    let currency = currency.trim().to_ascii_uppercase();
    if currency == "CNY" {
        return Ok(1.0);
    }

    let client = payment_http_client("V2Board-Rust-Currency")?;
    if let Ok(response) = client
        .get("https://api.exchangerate-api.com/v4/latest/CNY")
        .send()
        .await
        && let Ok(value) = response.json::<Value>().await
        && let Some(rate) = value
            .get("rates")
            .and_then(|rates| rates.get(&currency))
            .and_then(Value::as_f64)
        && rate > 0.0
    {
        return Ok(rate);
    }

    if let Ok(response) = client
        .get(format!(
            "https://api.frankfurter.app/latest?from=CNY&to={currency}"
        ))
        .send()
        .await
        && let Ok(value) = response.json::<Value>().await
        && let Some(rate) = value
            .get("rates")
            .and_then(|rates| rates.get(&currency))
            .and_then(Value::as_f64)
        && rate > 0.0
    {
        return Ok(rate);
    }

    Err(ApiError::legacy(
        "Currency conversion has timed out, please try again later",
    ))
}

fn stripe_amount(cents: i32, exchange: f64) -> i64 {
    ((cents.max(0) as f64) * exchange).floor() as i64
}

async fn stripe_post(
    config: &serde_json::Map<String, Value>,
    path: &str,
    params: &BTreeMap<String, String>,
) -> Result<Value, ApiError> {
    stripe_post_with_config(config, path, params).await
}

async fn stripe_post_with_config(
    config: &serde_json::Map<String, Value>,
    path: &str,
    params: &BTreeMap<String, String>,
) -> Result<Value, ApiError> {
    let response = payment_http_client("Stripe")?
        .post(format!("https://api.stripe.com/v1/{path}"))
        .bearer_auth(config_required(config, "stripe_sk_live")?)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_query(params)?)
        .send()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|_| ApiError::legacy("Payment gateway request failed"))?;
    let value = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "raw": text }));
    if !status.is_success() {
        let message = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("Payment gateway request failed");
        return Err(ApiError::legacy(message));
    }
    Ok(value)
}

fn stripe_event(
    config: &serde_json::Map<String, Value>,
    input: &PaymentNotifyInput,
) -> Result<Value, ApiError> {
    let signature = header_value(&input.headers, "stripe-signature")
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    let timestamp = signature
        .split(',')
        .find_map(|part| part.trim().strip_prefix("t="))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy("HMAC signature does not match"))?;
    let expected = hmac_sha256_hex(
        config_required(config, "stripe_webhook_key")?.as_bytes(),
        format!("{}.", timestamp)
            .as_bytes()
            .iter()
            .copied()
            .chain(input.body.iter().copied())
            .collect::<Vec<_>>()
            .as_slice(),
    )?;
    let verified = signature
        .split(',')
        .filter_map(|part| part.trim().strip_prefix("v1="))
        .any(|value| value.eq_ignore_ascii_case(&expected));
    if !verified {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    // Enforce Stripe's default 300s replay tolerance (matching
    // \Stripe\Webhook::constructEvent). Without this a validly-signed event can
    // be replayed indefinitely.
    let signed_at = timestamp
        .parse::<i64>()
        .map_err(|_| ApiError::legacy("HMAC signature does not match"))?;
    if Utc::now().timestamp() - signed_at > STRIPE_WEBHOOK_TOLERANCE_SECS {
        return Err(ApiError::legacy("HMAC signature does not match"));
    }
    serde_json::from_slice::<Value>(&input.body)
        .map_err(|_| ApiError::legacy("Payment notify body is invalid"))
}

/// Stripe's default webhook timestamp tolerance, in seconds.
const STRIPE_WEBHOOK_TOLERANCE_SECS: i64 = 300;

fn add_metadata_params(
    params: &mut BTreeMap<String, String>,
    prefix: &str,
    metadata: Option<&Value>,
) {
    let Some(metadata) = metadata.and_then(Value::as_object) else {
        return;
    };
    for (key, value) in metadata {
        if let Some(value) = json_scalar_string(value) {
            params.insert(format!("{prefix}[{key}]"), value);
        }
    }
}

fn value_path_str(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    json_scalar_string(current)
}

fn json_scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn hmac_sha256_hex(key: &[u8], payload: &[u8]) -> Result<String, ApiError> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
        .map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn hmac_sha512_hex(key: &[u8], payload: &[u8]) -> Result<String, ApiError> {
    let mut mac = <Hmac<Sha512> as KeyInit>::new_from_slice(key)
        .map_err(|_| ApiError::internal("invalid hmac key"))?;
    mac.update(payload);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn header_value(headers: &HashMap<String, String>, name: &str) -> Option<String> {
    headers
        .get(&name.to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .cloned()
}

fn required_param(params: &HashMap<String, String>, key: &str) -> Result<String, ApiError> {
    params
        .get(key)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| ApiError::legacy(format!("Payment notify {key} is missing")))
}

fn trimmed_body(body: &[u8]) -> String {
    String::from_utf8_lossy(body).trim().to_string()
}

fn ensure_trailing_slash(value: &str) -> String {
    let value = value.trim();
    if value.ends_with('/') {
        value.to_string()
    } else {
        format!("{value}/")
    }
}

fn url_origin(value: &str) -> String {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value.to_string();
    };
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() {
        value.to_string()
    } else {
        format!("{scheme}://{host}")
    }
}

fn right_chars(value: &str, count: usize) -> String {
    let length = value.chars().count();
    value.chars().skip(length.saturating_sub(count)).collect()
}

fn wechat_sign(params: &BTreeMap<String, String>, api_key: &str) -> String {
    let data = params
        .iter()
        .filter(|(key, value)| key.as_str() != "sign" && !value.is_empty())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{:X}", md5::compute(format!("{data}&key={api_key}")))
}

fn xml_from_params(params: &BTreeMap<String, String>) -> String {
    let mut xml = String::from("<xml>");
    for (key, value) in params {
        xml.push('<');
        xml.push_str(key);
        xml.push('>');
        xml.push_str(&xml_escape(value));
        xml.push_str("</");
        xml.push_str(key);
        xml.push('>');
    }
    xml.push_str("</xml>");
    xml
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parse_xml_map(body: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let mut cursor = body;
    while let Some(start) = cursor.find('<') {
        let after_open = &cursor[start + 1..];
        let Some(close) = after_open.find('>') else {
            break;
        };
        let tag = &after_open[..close];
        if tag.starts_with('/') || tag == "xml" || tag.contains(' ') {
            cursor = &after_open[close + 1..];
            continue;
        }
        let value_start = start + 1 + close + 1;
        let close_tag = format!("</{tag}>");
        let Some(value_end) = cursor[value_start..].find(&close_tag) else {
            cursor = &after_open[close + 1..];
            continue;
        };
        let raw_value = &cursor[value_start..value_start + value_end];
        let value = raw_value
            .strip_prefix("<![CDATA[")
            .and_then(|value| value.strip_suffix("]]>"))
            .unwrap_or(raw_value);
        params.insert(tag.to_string(), xml_unescape(value));
        cursor = &cursor[value_start + value_end + close_tag.len()..];
    }
    params
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn alipay_sign(private_key: &str, data: &str) -> Result<String, ApiError> {
    let private_key = normalize_private_key(private_key);
    let key = PKey::private_key_from_pem(private_key.as_bytes())
        .map_err(|_| ApiError::legacy("支付宝私钥错误"))?;
    let mut signer = Signer::new(MessageDigest::sha256(), &key)
        .map_err(|_| ApiError::legacy("支付宝签名失败"))?;
    signer
        .update(data.as_bytes())
        .map_err(|_| ApiError::legacy("支付宝签名失败"))?;
    let signature = signer
        .sign_to_vec()
        .map_err(|_| ApiError::legacy("支付宝签名失败"))?;
    Ok(standard_base64_encode(&signature))
}

fn alipay_verify(public_key: &str, data: &str, signature: &str) -> Result<bool, ApiError> {
    let public_key = normalize_public_key(public_key);
    let key = PKey::public_key_from_pem(public_key.as_bytes())
        .map_err(|_| ApiError::legacy("支付宝公钥错误"))?;
    let signature = standard_base64_decode(&signature.replace(' ', "+"))
        .ok_or_else(|| ApiError::legacy("verify error"))?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &key)
        .map_err(|_| ApiError::legacy("verify error"))?;
    verifier
        .update(data.as_bytes())
        .map_err(|_| ApiError::legacy("verify error"))?;
    verifier
        .verify(&signature)
        .map_err(|_| ApiError::legacy("verify error"))
}

fn normalize_private_key(value: &str) -> String {
    normalize_pem(value, "RSA PRIVATE KEY")
}

fn normalize_public_key(value: &str) -> String {
    normalize_pem(value, "PUBLIC KEY")
}

fn normalize_pem(value: &str, label: &str) -> String {
    let value = value.trim();
    if value.contains("BEGIN ") {
        return value.to_string();
    }
    let compact = value.split_whitespace().collect::<String>();
    let lines = compact
        .as_bytes()
        .chunks(64)
        .map(|chunk| String::from_utf8_lossy(chunk).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    format!("-----BEGIN {label}-----\n{lines}\n-----END {label}-----")
}

fn standard_base64_decode(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let c0 = base64_value(chunk[0])?;
        let c1 = base64_value(chunk[1])?;
        let c2 = if chunk[2] == b'=' {
            0
        } else {
            base64_value(chunk[2])?
        };
        let c3 = if chunk[3] == b'=' {
            0
        } else {
            base64_value(chunk[3])?
        };
        let combined = ((c0 as u32) << 18) | ((c1 as u32) << 12) | ((c2 as u32) << 6) | c3 as u32;
        output.push(((combined >> 16) & 0xff) as u8);
        if chunk[2] != b'=' {
            output.push(((combined >> 8) & 0xff) as u8);
        }
        if chunk[3] != b'=' {
            output.push((combined & 0xff) as u8);
        }
    }
    Some(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn standard_base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied();
        let b2 = bytes.get(index + 2).copied();

        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(
            TABLE[(((b0 & 0b0000_0011) << 4) | (b1.unwrap_or_default() >> 4)) as usize] as char,
        );
        if let Some(b1) = b1 {
            output.push(
                TABLE[(((b1 & 0b0000_1111) << 2) | (b2.unwrap_or_default() >> 6)) as usize] as char,
            );
        } else {
            output.push('=');
        }
        if let Some(b2) = b2 {
            output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
        index += 3;
    }
    output
}

fn canonical_query(params: &BTreeMap<String, String>) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn form_query(params: &BTreeMap<String, String>) -> Result<String, ApiError> {
    serde_urlencoded::to_string(params)
        .map_err(|_| ApiError::internal("failed to encode payment query"))
}

fn payment_http_client(user_agent: &'static str) -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder()
        .user_agent(user_agent)
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|_| ApiError::internal("failed to build payment http client"))
}

fn payment_gateway_message(value: &serde_json::Value, default: &str) -> String {
    if let Some(message) = value.get("message").and_then(|value| value.as_str()) {
        return message.to_string();
    }
    if let Some(errors) = value.get("errors").and_then(|value| value.as_object()) {
        for value in errors.values() {
            if let Some(message) = value
                .as_array()
                .and_then(|items| items.first())
                .and_then(|value| value.as_str())
            {
                return message.to_string();
            }
        }
    }
    default.to_string()
}

fn apply_vip_discount(discount: Option<i32>, draft: &mut DraftOrder) {
    // Port of OrderService::setVipDiscount. The VIP percentage is applied to the
    // still-original total_amount (the coupon step only recorded discount_amount
    // without reducing the total), then the accumulated coupon+VIP
    // discount_amount is subtracted from the total exactly once. When neither a
    // coupon nor a VIP discount applies, discount_amount stays None and the
    // total is left untouched, matching Laravel's null discount_amount.
    //
    // Laravel keeps this as float math (discount_amount + total * vip/100) and only
    // rounds when the value lands in the int column at save() time. Rounding the
    // coupon and VIP portions separately before summing drifted by a cent from
    // Laravel's persisted result, so the rounding is deferred to insert_order.
    if let Some(discount) = discount.filter(|discount| *discount > 0) {
        let value = draft.total_amount * (discount as f64 / 100.0);
        draft.discount_amount = Some(draft.discount_amount.unwrap_or_default() + value);
    }
    if let Some(discount_amount) = draft.discount_amount {
        draft.total_amount -= discount_amount;
    }
}

fn calculate_handling_amount(
    order: &OrderForCheckout,
    payment: &PaymentForCheckout,
) -> Option<i32> {
    let fixed = payment.handling_fee_fixed.unwrap_or_default();
    let percent = payment.handling_fee_percent.unwrap_or_default();
    if fixed == 0 && percent == 0.0 {
        return None;
    }
    Some(((order.total_amount as f64 * (percent / 100.0)) + fixed as f64).round() as i32)
}

fn buy_by_period(user: &mut UserForOrder, order: &OrderForCheckout, plan: &PlanRow, period: &str) {
    let now = Utc::now().timestamp();
    if order.r#type == 3 {
        user.expired_at = Some(now);
    }
    user.transfer_enable = plan.transfer_enable * GIB;
    user.device_limit = plan.device_limit;
    if user.expired_at.is_none() || order.r#type == 1 {
        reset_traffic(user);
    }
    if order.r#type == 2
        && let Some(expired_at) = user.expired_at
        && is_same_local_month_day(expired_at, now)
    {
        reset_traffic(user);
    }
    user.plan_id = Some(plan.id);
    user.group_id = Some(plan.group_id);
    user.expired_at = Some(add_period_time(period, user.expired_at.unwrap_or(now)));
}

fn buy_by_one_time(
    user: &mut UserForOrder,
    order: &OrderForCheckout,
    plan: &PlanRow,
    has_surplus_orders: bool,
) {
    // Work in bytes so fractional leftover GiB is preserved. Laravel computes
    // (plan_gib + leftover_bytes/GiB) * GiB, which is algebraically
    // plan_bytes + leftover_bytes; the earlier integer division here truncated
    // the fractional GiB (OrderService::buyByOneTime, :331-337).
    let mut transfer_enable = plan.transfer_enable * GIB;
    if !has_surplus_orders {
        let not_used_traffic = user.transfer_enable - (user.u + user.d);
        if not_used_traffic > 0 && user.expired_at.is_none() {
            transfer_enable += not_used_traffic;
        }
    }
    let _ = order;
    reset_traffic(user);
    user.transfer_enable = transfer_enable;
    user.device_limit = plan.device_limit;
    user.plan_id = Some(plan.id);
    user.group_id = Some(plan.group_id);
    user.expired_at = None;
}

fn reset_traffic(user: &mut UserForOrder) {
    user.u = 0;
    user.d = 0;
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

/// Laravel FormRequest validation failure: HTTP 422 `{message, errors:{field:[msg]}}`.
/// The top-level message mirrors Laravel's first validation error message.
fn order_validation(field: &str, message: &str) -> ApiError {
    ApiError::validation(
        message,
        HashMap::from([(field.to_string(), vec![message.to_string()])]),
    )
}

fn is_valid_period(period: &str) -> bool {
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

fn add_period_time(period: &str, timestamp: i64) -> i64 {
    let base = timestamp.max(Utc::now().timestamp());
    period_months(period)
        .map(|months| add_months(base, months))
        .unwrap_or(base)
}

fn add_months(timestamp: i64, months: u32) -> i64 {
    let base = Local
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Local::now);
    base.checked_add_months(Months::new(months))
        .unwrap_or(base)
        .timestamp()
}

fn is_same_local_month_day(left: i64, right: i64) -> bool {
    let Some(left) = Local.timestamp_opt(left, 0).single() else {
        return false;
    };
    let Some(right) = Local.timestamp_opt(right, 0).single() else {
        return false;
    };
    left.month() == right.month() && left.day() == right.day()
}

/// Round a float cents amount to the integer stored in the DB's int amount
/// columns. Mirrors MySQL's implicit float->int rounding (half away from zero),
/// which is how Laravel's un-cast Order amounts land in `int(11)` columns, so the
/// Rust pipeline can defer all rounding to persist time.
fn round_cents(amount: f64) -> i32 {
    amount.round() as i32
}

fn generate_order_no() -> String {
    let now = Local::now();
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

const USER_FOR_ORDER_SQL: &str = r#"
SELECT
    id,
    invite_user_id,
    balance,
    discount,
    commission_type,
    commission_rate,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_payment(method: &str, config: Value) -> PaymentForCheckout {
        PaymentForCheckout {
            id: 1,
            payment: method.to_string(),
            enable: 1,
            uuid: "payment-uuid".to_string(),
            config: config.to_string(),
            notify_domain: None,
            handling_fee_fixed: None,
            handling_fee_percent: None,
        }
    }

    fn unwrap_verified(outcome: PaymentNotifyOutcome) -> VerifiedPaymentNotify {
        match outcome {
            PaymentNotifyOutcome::Verified(verified) => verified,
            PaymentNotifyOutcome::Ignored(body) => panic!("expected verified notify, got {body}"),
        }
    }

    #[test]
    fn payment_provider_registry_matches_laravel_builtin_plugins() {
        assert_eq!(
            crate::payment_provider::payment_provider_codes(),
            vec![
                "AlipayF2F",
                "BEasyPaymentUSDT",
                "BTCPay",
                "CoinPayments",
                "Coinbase",
                "EPay",
                "MGate",
                "StripeALL",
                "StripeAlipay",
                "StripeCheckout",
                "StripeCredit",
                "StripeWepay",
                "WechatPayNative",
            ]
        );
    }

    #[test]
    fn pay_and_notify_dispatch_cover_provider_registry() {
        for method in crate::payment_provider::payment_provider_codes() {
            assert!(
                require_payment_provider(method).is_ok(),
                "{method} is listed but checkout/notify dispatch cannot resolve it"
            );
        }
    }

    #[test]
    fn epay_notify_fixture_verifies_signature_and_extracts_trade_numbers() {
        let payment = fixture_payment("EPay", json!({ "key": "epay-secret" }));
        let mut signed = BTreeMap::from([
            ("money".to_string(), "12.34".to_string()),
            ("out_trade_no".to_string(), "T202607060001".to_string()),
            ("trade_no".to_string(), "EPAY-CALLBACK-1".to_string()),
            ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
        ]);
        let sign = format!(
            "{:x}",
            md5::compute(format!("{}{}", canonical_query(&signed), "epay-secret"))
        );
        signed.insert("sign".to_string(), sign);
        signed.insert("sign_type".to_string(), "MD5".to_string());
        let params = signed.into_iter().collect::<HashMap<_, _>>();

        let verified = unwrap_verified(epay_notify(&payment, &params).unwrap());

        assert_eq!(verified.trade_no, "T202607060001");
        assert_eq!(verified.callback_no, "EPAY-CALLBACK-1");
    }

    #[test]
    fn mgate_notify_fixture_verifies_signature_and_extracts_trade_numbers() {
        let payment = fixture_payment("MGate", json!({ "mgate_app_secret": "mgate-secret" }));
        let mut signed = BTreeMap::from([
            ("out_trade_no".to_string(), "T202607060002".to_string()),
            ("trade_no".to_string(), "MGATE-CALLBACK-1".to_string()),
        ]);
        let sign = format!(
            "{:x}",
            md5::compute(format!(
                "{}{}",
                form_query(&signed).unwrap(),
                "mgate-secret"
            ))
        );
        signed.insert("sign".to_string(), sign);
        let params = signed.into_iter().collect::<HashMap<_, _>>();

        let verified = unwrap_verified(mgate_notify(&payment, &params).unwrap());

        assert_eq!(verified.trade_no, "T202607060002");
        assert_eq!(verified.callback_no, "MGATE-CALLBACK-1");
    }

    #[test]
    fn bepusdt_notify_fixture_verifies_signature_and_returns_custom_ok_body() {
        let payment = fixture_payment(
            "BEasyPaymentUSDT",
            json!({ "bepusdt_apitoken": "bepusdt-secret" }),
        );
        let mut signed = BTreeMap::from([
            ("order_id".to_string(), "T202607060003".to_string()),
            ("status".to_string(), "2".to_string()),
            ("trade_id".to_string(), "BEPUSDT-CALLBACK-1".to_string()),
        ]);
        let signature = format!(
            "{:x}",
            md5::compute(format!("{}{}", canonical_query(&signed), "bepusdt-secret"))
        );
        signed.insert("signature".to_string(), signature);
        let params = signed.into_iter().collect::<HashMap<_, _>>();

        let verified = unwrap_verified(bepusdt_notify(&payment, &params).unwrap());

        assert_eq!(verified.trade_no, "T202607060003");
        assert_eq!(verified.callback_no, "BEPUSDT-CALLBACK-1");
        assert_eq!(verified.custom_result.as_deref(), Some("ok"));
    }

    #[test]
    fn stripe_checkout_notify_fixture_verifies_webhook_hmac() {
        let payment = fixture_payment(
            "StripeCheckout",
            json!({ "stripe_webhook_key": "whsec_test" }),
        );
        let body = json!({
            "type": "checkout.session.completed",
            "data": {
                "object": {
                    "payment_status": "paid",
                    "client_reference_id": "T202607060004",
                    "payment_intent": "pi_callback_1"
                }
            }
        })
        .to_string()
        .into_bytes();
        // Use a fresh timestamp so the webhook falls inside Stripe's 300s
        // replay tolerance regardless of when the suite runs.
        let timestamp = Utc::now().timestamp().to_string();
        let signature = stripe_test_signature(&timestamp, &body);
        let input = PaymentNotifyInput {
            params: HashMap::new(),
            body,
            headers: HashMap::from([(
                "stripe-signature".to_string(),
                format!("t={timestamp},v1={signature}"),
            )]),
        };

        let verified = unwrap_verified(stripe_checkout_notify(&payment, &input).unwrap());

        assert_eq!(verified.trade_no, "T202607060004");
        assert_eq!(verified.callback_no, "pi_callback_1");
    }

    #[test]
    fn stripe_checkout_notify_rejects_replayed_stale_webhook() {
        let payment = fixture_payment(
            "StripeCheckout",
            json!({ "stripe_webhook_key": "whsec_test" }),
        );
        let body = json!({
            "type": "checkout.session.completed",
            "data": { "object": { "payment_status": "paid", "client_reference_id": "T1" } }
        })
        .to_string()
        .into_bytes();
        // A correctly-signed event whose timestamp is older than the tolerance
        // must be rejected as a replay.
        let stale = (Utc::now().timestamp() - STRIPE_WEBHOOK_TOLERANCE_SECS - 60).to_string();
        let signature = stripe_test_signature(&stale, &body);
        let input = PaymentNotifyInput {
            params: HashMap::new(),
            body,
            headers: HashMap::from([(
                "stripe-signature".to_string(),
                format!("t={stale},v1={signature}"),
            )]),
        };

        assert!(stripe_checkout_notify(&payment, &input).is_err());
    }

    fn stripe_test_signature(timestamp: &str, body: &[u8]) -> String {
        hmac_sha256_hex(
            b"whsec_test",
            format!("{timestamp}.")
                .as_bytes()
                .iter()
                .copied()
                .chain(body.iter().copied())
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap()
    }

    fn draft_fixture(total_amount: f64) -> DraftOrder {
        DraftOrder {
            user_id: 1,
            plan_id: 1,
            coupon_id: None,
            r#type: 1,
            period: "month_price".to_string(),
            trade_no: "test".to_string(),
            total_amount,
            discount_amount: None,
            surplus_amount: None,
            refund_amount: None,
            balance_amount: None,
            surplus_order_ids: None,
            invite_user_id: None,
            commission_balance: 0.0,
        }
    }

    #[test]
    fn vip_and_coupon_discount_defer_rounding_to_persist() {
        // total=1990, coupon 33% (656.7) then VIP 15% (298.5). Laravel keeps floats:
        // discount_amount = 656.7 + 298.5 = 955.2 -> persist 955; total = 1990 -
        // 955.2 = 1034.8 -> persist 1035. Rounding each portion first would drift to
        // 956 / 1034, so the rounding must be deferred to persist time.
        let mut draft = draft_fixture(1990.0);
        draft.discount_amount = Some(1990.0 * (33.0 / 100.0)); // coupon step, pre-VIP
        apply_vip_discount(Some(15), &mut draft);
        assert_eq!(round_cents(draft.discount_amount.unwrap()), 955);
        assert_eq!(round_cents(draft.total_amount), 1035);
    }

    #[test]
    fn no_coupon_no_vip_leaves_total_untouched_and_discount_null() {
        // Laravel's setVipDiscount leaves discount_amount NULL and total unchanged
        // when neither a coupon nor a VIP discount applies.
        let mut draft = draft_fixture(1990.0);
        apply_vip_discount(None, &mut draft);
        assert!(draft.discount_amount.is_none());
        assert_eq!(round_cents(draft.total_amount), 1990);
    }
}
