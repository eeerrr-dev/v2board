use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, MySqlPool};
use v2board_compat::ApiError;
use v2board_config::AppConfig;

#[cfg(test)]
use openssl::pkey::PKey;

mod lifecycle;
mod payment_integrations;

use lifecycle::{
    calculate_handling_amount, calculate_handling_amount_cents, commission_amount,
    credit_user_balance, find_user_for_order, generate_order_no, insert_order, is_valid_period,
    mark_order_paid, round_cents,
};

#[cfg(test)]
use lifecycle::{
    USER_FOR_ORDER_SQL, add_months, add_period_time, apply_vip_discount, buy_by_one_time,
    buy_by_period, commission_is_eligible, percent,
};

use payment_integrations::{
    alipay_f2f_notify, alipay_f2f_pay, bepusdt_notify, bepusdt_pay, btcpay_notify, btcpay_pay,
    coinbase_notify, coinbase_pay, coinpayments_notify, coinpayments_pay, config_string,
    epay_notify, epay_pay, mgate_notify, mgate_pay, payment_config, payment_notify_url,
    payment_return_url, require_payment_provider, stripe_all_notify, stripe_all_pay,
    stripe_cancel_intent, stripe_checkout_notify, stripe_checkout_pay, stripe_credit_prepare,
    stripe_payment_intent_notify, stripe_source_notify, stripe_source_pay,
    wechat_pay_native_notify, wechat_pay_native_pay,
};

const GIB: i64 = 1_073_741_824;
const UNFINISHED_ORDER_UNIQUE_KEY: &str = "uniq_unfinished_order_per_user";
const PAYMENT_SETTLEMENT_ORDER_SQL: &str = r#"
    SELECT id, status, total_amount, handling_amount, user_id, payment_id, callback_no
    FROM v2_order
    WHERE trade_no = ?
    LIMIT 1
    FOR UPDATE
"#;
const PAYMENT_NOTIFY_LOOKUP_SQL: &str = r#"
    SELECT
        id,
        payment,
        enable,
        uuid,
        CAST(config AS CHAR) AS config,
        notify_domain,
        handling_fee_fixed,
        handling_fee_percent
    FROM v2_payment
    WHERE payment = ? AND uuid = ?
    LIMIT 1
"#;
const PAYMENT_CONFIG_FOR_UPDATE_SQL: &str =
    "SELECT payment, CAST(config AS CHAR) FROM v2_payment WHERE id = ? LIMIT 1 FOR UPDATE";
const UNFINISHED_ORDER_FOR_UPDATE_SQL: &str =
    "SELECT id FROM v2_order WHERE user_id = ? AND status IN (0, 1) LIMIT 1 FOR UPDATE";
#[derive(Clone)]
pub struct OrderService {
    db: MySqlPool,
    config: Arc<AppConfig>,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckoutResult {
    pub r#type: i8,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct StripePaymentIntentResult {
    pub public_key: String,
    pub client_secret: String,
    pub amount: i64,
    pub currency: String,
}

/// Calculate the integer-cent commission stored on an order without passing
/// through binary floating point. Fractional cents use MySQL's assignment
/// behavior (midpoint away from zero), and values outside the amount column's
/// signed 32-bit range are rejected instead of saturating.
pub fn commission_amount_cents(
    total_amount: i64,
    commission_rate: Option<i32>,
    default_rate: i32,
) -> Result<i32, ApiError> {
    round_cents(commission_amount(
        Decimal::from(total_amount),
        commission_rate,
        default_rate,
    ))
}

#[derive(Debug, Clone)]
struct DraftOrder {
    user_id: i64,
    plan_id: i32,
    coupon_id: Option<i32>,
    r#type: i32,
    period: String,
    trade_no: String,
    // Monetary values remain exact decimals through coupon/VIP/surplus/balance/
    // commission math. They are rounded only when bound to the integer-cent DB
    // columns, preserving Laravel's externally visible deferred-rounding contract
    // without binary floating-point drift.
    total_amount: Decimal,
    discount_amount: Option<Decimal>,
    surplus_amount: Option<Decimal>,
    refund_amount: Option<Decimal>,
    balance_amount: Option<Decimal>,
    surplus_order_ids: Option<Vec<i64>>,
    invite_user_id: Option<i32>,
    commission_balance: Decimal,
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
    handling_fee_percent: Option<Decimal>,
}

#[derive(Debug, Clone)]
struct PaymentOrder {
    notify_url: String,
    return_url: String,
    trade_no: String,
    total_amount: i32,
    user_id: i64,
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
    /// Authenticated Stripe metadata user. Non-Stripe gateways leave this empty;
    /// the locked order binding validates it whenever present.
    authenticated_user_id: Option<i64>,
    /// Authenticated amount in the order's integer-cent accounting unit. It is
    /// compared only after the matching order row is locked, immediately before
    /// the paid transition.
    settled_amount_cents: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedPaymentBinding {
    payment_id: i32,
    user_id: Option<i64>,
    callback_no: Option<String>,
}

fn payment_binding_matches(
    expected: &ExpectedPaymentBinding,
    order_user_id: i64,
    payment_id: Option<i32>,
    bound_callback_no: Option<&str>,
) -> bool {
    Some(expected.payment_id) == payment_id
        && expected
            .user_id
            .is_none_or(|user_id| user_id == order_user_id)
        && expected
            .callback_no
            .as_deref()
            .is_none_or(|callback_no| bound_callback_no == Some(callback_no))
}

fn payment_amount_matches(
    order_amount_cents: i64,
    handling_amount_cents: Option<i64>,
    settled_amount_cents: i64,
) -> bool {
    order_amount_cents.checked_add(handling_amount_cents.unwrap_or_default())
        == Some(settled_amount_cents)
}

fn payable_amount_cents(
    order_amount_cents: i32,
    handling_amount_cents: Option<i32>,
) -> Result<i32, ApiError> {
    order_amount_cents
        .checked_add(handling_amount_cents.unwrap_or_default())
        .filter(|amount| *amount > 0)
        .ok_or_else(|| ApiError::legacy("Payment amount is outside the supported range"))
}

fn payment_config_snapshot_matches(
    current: Option<(String, String)>,
    expected_method: &str,
    expected_config: &Value,
) -> bool {
    current.is_some_and(|(method, config)| {
        method == expected_method
            && serde_json::from_str::<Value>(&config).is_ok_and(|config| &config == expected_config)
    })
}

#[derive(Debug)]
enum PaymentNotifyOutcome {
    Verified(VerifiedPaymentNotify),
    Ignored(String),
}

impl OrderService {
    pub fn new(db: MySqlPool, config: Arc<AppConfig>) -> Self {
        Self { db, config }
    }

    pub async fn save(&self, user_id: i64, input: SaveOrderInput) -> Result<String, ApiError> {
        // Laravel OrderSave FormRequest: plan_id `required`, period
        // `required|in:<period list>`. A failed FormRequest is a 422
        // `{message, errors:{field:[msg]}}`, not the 500 these pre-logic checks
        // used to return.
        let plan_id = input
            .plan_id
            .ok_or_else(|| ApiError::validation_field("plan_id", "Plan ID cannot be empty"))?;
        let period = input
            .period
            .as_deref()
            .filter(|period| !period.trim().is_empty())
            .ok_or_else(|| ApiError::validation_field("period", "Plan period cannot be empty"))?;
        if !is_valid_period(period) {
            return Err(ApiError::validation_field("period", "Wrong plan period"));
        }

        let mut tx = self.db.begin().await?;
        // Serialize order creation on the user row before checking the invariant.
        // Checking first leaves a classic write-skew window: two transactions can
        // both observe zero unfinished orders, then queue on this same user lock and
        // each insert after it is released. The generated-column unique index is the
        // final database guard for writers that do not use this service.
        let user = find_user_for_order(&mut tx, user_id).await?;
        // This is a locking read, not a snapshot read. Consequently the first
        // consistent read in build_plan_order happens only after its plan-row
        // lock is acquired and observes every reservation committed by the
        // previous holder of that lock.
        let incomplete_order_id: Option<i64> = sqlx::query_scalar(UNFINISHED_ORDER_FOR_UPDATE_SQL)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if incomplete_order_id.is_some() {
            return Err(ApiError::legacy(
                "You have an unpaid or pending order, please try again later or cancel it",
            ));
        }

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
        let binding = sqlx::query_as::<_, (Option<i32>, Option<String>)>(
            "SELECT payment_id, callback_no FROM v2_order WHERE id = ?",
        )
        .bind(order.id)
        .fetch_one(&mut *tx)
        .await?;

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
                handling_fee_percent
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
        if payment.payment == "StripeCredit" {
            return Err(ApiError::legacy(
                "Stripe payments must be confirmed with Payment Element",
            ));
        }

        let handling_amount = calculate_handling_amount(&order, &payment)?;
        let payable_amount = payable_amount_cents(order.total_amount, handling_amount)?;
        tx.commit().await?;

        // Cancel exactly the Stripe intent from the row snapshot above. The CAS
        // below then prevents a concurrent Payment Element preparation from being
        // overwritten after we have finished the network request.
        if !self
            .cancel_stripe_intent_binding(binding.0, binding.1.as_deref())
            .await?
        {
            return Err(ApiError::legacy("Order does not exist or has been paid"));
        }
        let expected_config = serde_json::from_str::<Value>(&payment.config)
            .map_err(|_| ApiError::legacy("Payment config is invalid"))?;
        // Bind under the same payment-row lock and lock order used by admin
        // save/drop. Either checkout wins and admin observes the pending binding,
        // or admin wins and this stale checkout aborts before contacting the new
        // gateway. This prevents deletion/config mutation from racing every
        // provider, not only Payment Element.
        let mut bind_tx = self.db.begin().await?;
        let current_payment = sqlx::query_as::<_, (String, String)>(PAYMENT_CONFIG_FOR_UPDATE_SQL)
            .bind(payment.id)
            .fetch_optional(&mut *bind_tx)
            .await?;
        if !payment_config_snapshot_matches(current_payment, &payment.payment, &expected_config) {
            bind_tx.rollback().await?;
            return Err(ApiError::legacy(
                "Payment configuration changed, please try again",
            ));
        }
        let updated = sqlx::query(
            r#"
            UPDATE v2_order
            SET payment_id = ?, handling_amount = ?, callback_no = NULL, updated_at = ?
            WHERE id = ? AND status = 0 AND payment_id <=> ? AND callback_no <=> ?
            "#,
        )
        .bind(payment.id)
        .bind(handling_amount)
        .bind(Utc::now().timestamp())
        .bind(order.id)
        .bind(binding.0)
        .bind(&binding.1)
        .execute(&mut *bind_tx)
        .await?;
        if updated.rows_affected() != 1 {
            bind_tx.rollback().await?;
            return Err(ApiError::legacy("Order does not exist or has been paid"));
        }
        bind_tx.commit().await?;

        let payment_order = PaymentOrder {
            notify_url: payment_notify_url(&self.config, &payment),
            return_url: payment_return_url(&self.config, &order.trade_no),
            trade_no: order.trade_no,
            total_amount: payable_amount,
            user_id,
        };
        self.pay_with_gateway(&payment, &payment_order).await
    }

    pub async fn prepare_stripe_intent(
        &self,
        user_id: i64,
        input: CheckoutOrderInput,
    ) -> Result<StripePaymentIntentResult, ApiError> {
        let method = input
            .method
            .ok_or_else(|| ApiError::legacy("Payment method is not available"))?;
        let mut tx = self.db.begin().await?;
        let order = sqlx::query_as::<_, (i64, String, i32, Option<String>, Option<i32>)>(
            r#"
            SELECT id, trade_no, total_amount, callback_no, payment_id
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
        if order.2 <= 0 {
            return Err(ApiError::legacy("Order does not exist or has been paid"));
        }

        let payment = sqlx::query_as::<_, PaymentForCheckout>(
            r#"
            SELECT id, payment, enable, uuid, CAST(config AS CHAR) AS config,
                   notify_domain, handling_fee_fixed,
                   handling_fee_percent
            FROM v2_payment
            WHERE id = ? AND payment = 'StripeCredit'
            LIMIT 1
            "#,
        )
        .bind(method)
        .fetch_optional(&mut *tx)
        .await?
        .filter(|payment| payment.enable == 1)
        .ok_or_else(|| ApiError::legacy("Payment method is not available"))?;

        let handling_amount =
            calculate_handling_amount_cents(order.2, &payment)?.filter(|amount| *amount != 0);
        let payable_amount = payable_amount_cents(order.2, handling_amount)?;
        tx.commit().await?;

        if order.4 != Some(payment.id)
            && !self
                .cancel_stripe_intent_binding(order.4, order.3.as_deref())
                .await?
        {
            return Err(ApiError::legacy("Order does not exist or has been paid"));
        }

        let payment_order = PaymentOrder {
            notify_url: payment_notify_url(&self.config, &payment),
            return_url: payment_return_url(&self.config, &order.1),
            trade_no: order.1.clone(),
            total_amount: payable_amount,
            user_id,
        };
        let reusable_intent = (order.4 == Some(payment.id))
            .then_some(order.3.as_deref())
            .flatten();
        let (intent_id, prepared) =
            stripe_credit_prepare(&payment, &payment_order, reusable_intent).await?;
        // The Stripe network call stays outside a database transaction. Once it
        // returns, take the same payment-row lock used by admin save/drop, verify
        // the exact driver/config snapshot, and bind the intent before releasing
        // that lock. This closes the prepare-vs-config-edit gap without holding a
        // row lock across an external request.
        let expected_config = serde_json::from_str::<Value>(&payment.config)
            .map_err(|_| ApiError::legacy("Payment config is invalid"))?;
        let mut bind_tx = self.db.begin().await?;
        let current_payment = sqlx::query_as::<_, (String, String)>(PAYMENT_CONFIG_FOR_UPDATE_SQL)
            .bind(payment.id)
            .fetch_optional(&mut *bind_tx)
            .await?;
        let payment_changed =
            !payment_config_snapshot_matches(current_payment, "StripeCredit", &expected_config);
        let binding_state = if payment_changed {
            bind_tx.rollback().await?;
            "payment_changed"
        } else {
            let updated = sqlx::query(
                r#"
                UPDATE v2_order
                SET payment_id = ?, handling_amount = ?, callback_no = ?, updated_at = ?
                WHERE id = ? AND status = 0 AND payment_id <=> ? AND callback_no <=> ?
                "#,
            )
            .bind(payment.id)
            .bind(handling_amount)
            .bind(&intent_id)
            .bind(Utc::now().timestamp())
            .bind(order.0)
            .bind(order.4)
            .bind(&order.3)
            .execute(&mut *bind_tx)
            .await?;
            let bound = if updated.rows_affected() == 1 {
                true
            } else {
                let current = sqlx::query_as::<_, (i8, Option<i32>, Option<String>)>(
                    "SELECT status, payment_id, callback_no FROM v2_order WHERE id = ?",
                )
                .bind(order.0)
                .fetch_optional(&mut *bind_tx)
                .await?;
                current.is_some_and(|current| {
                    current.0 == 0
                        && current.1 == Some(payment.id)
                        && current.2.as_deref() == Some(intent_id.as_str())
                })
            };
            bind_tx.commit().await?;
            if bound { "bound" } else { "order_changed" }
        };
        if binding_state != "bound" {
            let config = payment_config(&payment)?;
            if !stripe_cancel_intent(&config, &intent_id).await? {
                return Err(ApiError::legacy("The Stripe payment has already succeeded"));
            }
            return Err(ApiError::legacy(if binding_state == "payment_changed" {
                "Stripe payment configuration changed, please try again"
            } else {
                "Order does not exist or has been paid"
            }));
        }
        Ok(prepared)
    }

    pub async fn cancel_stripe_intent_binding(
        &self,
        payment_id: Option<i32>,
        intent_id: Option<&str>,
    ) -> Result<bool, ApiError> {
        let (Some(payment_id), Some(intent_id)) = (payment_id, intent_id) else {
            return Ok(true);
        };
        if !intent_id.starts_with("pi_") {
            return Ok(true);
        }
        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT payment, CAST(config AS CHAR) FROM v2_payment WHERE id = ? LIMIT 1",
        )
        .bind(payment_id)
        .fetch_optional(&self.db)
        .await?;
        let Some((method, config)) = row else {
            return Err(ApiError::legacy("Stripe payment binding is invalid"));
        };
        if method != "StripeCredit" {
            return Err(ApiError::legacy("Stripe payment binding is invalid"));
        }
        let mut config = match serde_json::from_str::<Value>(&config)
            .map_err(|_| ApiError::legacy("Payment config is invalid"))?
        {
            Value::Object(config) => config,
            _ => return Err(ApiError::legacy("Payment config is invalid")),
        };
        if let Some(secret_key) = config_string(&config, "stripe_sk_live") {
            config.insert(
                "stripe_sk_live".to_string(),
                Value::String(secret_key.trim().to_string()),
            );
        }
        stripe_cancel_intent(&config, intent_id).await
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
        let payment = sqlx::query_as::<_, PaymentForCheckout>(PAYMENT_NOTIFY_LOOKUP_SQL)
            .bind(method)
            .bind(uuid)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::legacy("gate is not found"))?;
        // `enable` gates new checkouts only. An authenticated callback can arrive
        // after an operator disables any gateway; rejecting it would strand money
        // already accepted by the provider. Admin mutations/deletion are guarded
        // while a pending order remains bound to this payment id.

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
        let expected_binding = ExpectedPaymentBinding {
            payment_id: payment.id,
            user_id: verified.authenticated_user_id,
            // Payment Element binds its intent id before the client can confirm
            // it, so retain its stronger exact callback binding. Other gateways
            // generate their callback ids only after checkout.
            callback_no: (payment.payment == "StripeCredit").then(|| verified.callback_no.clone()),
        };
        let paid_notice = self
            .paid_by_trade_no(
                &verified.trade_no,
                &verified.callback_no,
                &expected_binding,
                verified.settled_amount_cents,
            )
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
            "StripeCredit" => Err(ApiError::legacy(
                "Stripe payments must be confirmed with Payment Element",
            )),
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
            "StripeCredit" => stripe_payment_intent_notify(payment, input),
            "StripeAlipay" | "StripeWepay" => stripe_source_notify(payment, input).await,
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
        let expected_binding = sqlx::query_as::<_, (i8, Option<i32>, Option<String>)>(
            "SELECT status, payment_id, callback_no FROM v2_order WHERE trade_no = ? LIMIT 1",
        )
        .bind(trade_no)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("订单不存在"))?;
        if expected_binding.0 != 0 {
            return Err(ApiError::legacy("只能对待支付的订单进行操作"));
        }
        // Manual settlement must first make the browser-held client secret inert.
        // Otherwise the order can be opened manually and then charged by Stripe a
        // moment later, whose webhook will be ignored because the order is no longer
        // pending.
        if !self
            .cancel_stripe_intent_binding(expected_binding.1, expected_binding.2.as_deref())
            .await?
        {
            return Err(ApiError::legacy("只能对待支付的订单进行操作"));
        }

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
        let current_binding = sqlx::query_as::<_, (i8, Option<i32>, Option<String>)>(
            "SELECT status, payment_id, callback_no FROM v2_order WHERE id = ?",
        )
        .bind(order.id)
        .fetch_one(&mut *tx)
        .await?;
        if current_binding != expected_binding {
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
        let Some(expiration_snapshot) =
            sqlx::query_as::<_, (i8, i64, Option<i32>, Option<String>)>(
                r#"
                SELECT status, created_at, payment_id, callback_no
                FROM v2_order
                WHERE trade_no = ?
                LIMIT 1
                "#,
            )
            .bind(trade_no)
            .fetch_optional(&self.db)
            .await?
        else {
            return Ok(());
        };
        let should_expire = expiration_snapshot.0 == 0
            && expiration_snapshot.1 <= Utc::now().timestamp().saturating_sub(7_200);
        if should_expire
            && !self
                .cancel_stripe_intent_binding(
                    expiration_snapshot.2,
                    expiration_snapshot.3.as_deref(),
                )
                .await?
        {
            // Stripe already reports success. Leave the order pending for the signed
            // success webhook instead of expiring a charge that is in settlement.
            return Ok(());
        }

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

        let (status, created_at, balance_amount, payment_id, callback_no) =
            sqlx::query_as::<_, (i8, i64, Option<i32>, Option<i32>, Option<String>)>(
                "SELECT status, created_at, balance_amount, payment_id, callback_no FROM v2_order WHERE id = ?",
            )
            .bind(order.id)
            .fetch_one(&mut *tx)
            .await?;

        match status {
            0 if should_expire
                && created_at <= Utc::now().timestamp().saturating_sub(7_200)
                && payment_id == expiration_snapshot.2
                && callback_no == expiration_snapshot.3 =>
            {
                sqlx::query("UPDATE v2_order SET status = 2, updated_at = ? WHERE id = ?")
                    .bind(Utc::now().timestamp())
                    .bind(order.id)
                    .execute(&mut *tx)
                    .await?;
                if let Some(balance_amount) = balance_amount.filter(|amount| *amount > 0) {
                    credit_user_balance(
                        &mut tx,
                        order.user_id,
                        balance_amount,
                        "Order balance refund exceeds the supported balance range",
                    )
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
        expected_binding: &ExpectedPaymentBinding,
        settled_amount_cents: Option<i64>,
    ) -> Result<Option<PaidOrderNotice>, ApiError> {
        // Commit the paid mark in its OWN transaction first, mirroring Laravel's
        // OrderService::paid() which save()s status=1 before dispatching OrderHandleJob.
        // A gateway-confirmed payment must be durable even if opening the order later
        // fails; folding the open into the same transaction (as before) meant a transient
        // error while granting the plan would roll back real, already-received money.
        let total_amount;
        {
            let mut tx = self.db.begin().await?;
            let Some((
                order_id,
                status,
                amount,
                handling_amount,
                order_user_id,
                payment_id,
                bound_callback_no,
            )) =
                sqlx::query_as::<_, (i64, i8, i64, Option<i64>, i64, Option<i32>, Option<String>)>(
                    PAYMENT_SETTLEMENT_ORDER_SQL,
                )
                .bind(trade_no)
                .fetch_optional(&mut *tx)
                .await?
            else {
                return Err(ApiError::legacy("Order does not exist"));
            };
            // Every callback must still belong to the payment method currently
            // bound to this exact locked row. This closes the method-switch TOCTOU
            // gap for all gateways. Stripe additionally checks authenticated user
            // metadata, and Payment Element checks its pre-bound intent id.
            if !payment_binding_matches(
                expected_binding,
                order_user_id,
                payment_id,
                bound_callback_no.as_deref(),
            ) {
                tx.commit().await?;
                return Ok(None);
            }
            if status != 0 {
                // Already paid/opened/cancelled: idempotent no-op, safe on gateway replay.
                // Laravel's `handle()` returns early here without the `成功收款` message, so
                // report `None` and the caller suppresses the admin notification.
                tx.commit().await?;
                return Ok(None);
            }
            if let Some(settled_amount_cents) = settled_amount_cents
                && !payment_amount_matches(amount, handling_amount, settled_amount_cents)
            {
                tx.rollback().await?;
                return Err(ApiError::legacy("Payment amount does not match the order"));
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
}
#[cfg(test)]
mod tests;
