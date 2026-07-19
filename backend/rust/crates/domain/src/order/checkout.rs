use chrono::Utc;
use serde_json::{Value, json};
use v2board_compat::{ApiError, Code, Problem};

use super::lifecycle::{
    calculate_handling_amount, calculate_handling_amount_cents, find_user_for_order, insert_order,
    is_valid_period, mark_order_paid,
};
use super::payment_integrations::{
    alipay_f2f_pay, bepusdt_pay, btcpay_pay, coinbase_pay, coinpayments_pay, config_string,
    epay_pay, mgate_pay, payment_config, payment_notify_url, payment_return_url,
    require_payment_provider, stripe_all_pay, stripe_cancel_intent, stripe_checkout_pay,
    stripe_credit_prepare, stripe_source_pay, wechat_pay_native_pay,
};
use super::{
    CheckoutOrderInput, CheckoutResult, OrderForCheckout, OrderService, PaymentForCheckout,
    PaymentOrder, SaveOrderInput, StripePaymentIntentResult, bounded_payment_identifier,
    generate_order_no, payment_identifier_hash,
};

pub(super) const PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL: &str = "SELECT payment, CAST(config AS TEXT) FROM payment_method \
     WHERE id = $1 AND enable = 1 AND archived_at IS NULL LIMIT 1 FOR SHARE";
pub(super) const UNFINISHED_ORDER_FOR_UPDATE_SQL: &str =
    "SELECT id FROM orders WHERE user_id = $1 AND status IN (0, 1) LIMIT 1 FOR UPDATE";

pub(super) fn payable_amount_cents(
    order_amount_cents: i32,
    handling_amount_cents: Option<i32>,
) -> Result<i32, ApiError> {
    order_amount_cents
        .checked_add(handling_amount_cents.unwrap_or_default())
        .filter(|amount| *amount > 0)
        .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentAmountOutOfRange)))
}

/// The stored column is an encrypted envelope; the snapshot comparison
/// decrypts the currently active row (with the immutable driver/uuid binding)
/// and compares the plaintext gateway configs.
pub(super) fn payment_config_snapshot_matches(
    app_key: &str,
    uuid: &str,
    current: Option<(String, String)>,
    expected_method: &str,
    expected_config: &Value,
) -> bool {
    current.is_some_and(|(method, config)| {
        method == expected_method
            && crate::payment_secrets::decrypt_payment_config(app_key, &method, uuid, &config)
                .is_ok_and(|config| &config == expected_config)
    })
}

impl OrderService {
    pub async fn save(&self, user_id: i64, input: SaveOrderInput) -> Result<String, ApiError> {
        // The request union is structural (§5.5): plan_id/period presence is
        // enforced by the API-layer request struct. The period vocabulary
        // check stays here; §3.4 folds the legacy "Wrong plan period" 422
        // onto the 400 plan_period_unavailable problem.
        if let SaveOrderInput::Plan { period, .. } = &input
            && !is_valid_period(period)
        {
            return Err(Problem::new(Code::PlanPeriodUnavailable)
                .with_detail("Wrong plan period")
                .into());
        }

        let mut tx = self.db.begin().await?;
        // Order lifecycle transactions use one global lock order: unfinished/
        // target order first, then user, then plan/payment. The generated-column
        // unique key remains the authoritative write-skew guard when two empty
        // range locks coexist under InnoDB gap-lock semantics.
        let incomplete_order_id: Option<i64> = sqlx::query_scalar(UNFINISHED_ORDER_FOR_UPDATE_SQL)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if incomplete_order_id.is_some() {
            return Err(Problem::new(Code::PendingOrderExists).into());
        }
        let user = find_user_for_order(&mut tx, user_id).await?;

        let trade_no = generate_order_no();
        let now = Utc::now().timestamp();
        let draft = match input {
            SaveOrderInput::Deposit { deposit_amount } => {
                self.build_deposit_order(&mut tx, user, deposit_amount, trade_no)
                    .await?
            }
            SaveOrderInput::Plan {
                plan_id,
                period,
                coupon_code,
            } => {
                self.build_plan_order(
                    &mut tx,
                    user,
                    plan_id,
                    &period,
                    coupon_code.as_deref(),
                    trade_no,
                )
                .await?
            }
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
                "type",
                period,
                trade_no,
                total_amount,
                refund_amount,
                surplus_order_ids::text AS surplus_order_ids
            FROM orders
            WHERE trade_no = $1 AND user_id = $2 AND status = 0
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(&input.trade_no)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;
        let binding = sqlx::query_as::<_, (Option<i32>, Option<String>)>(
            "SELECT payment_id, callback_no FROM orders WHERE id = $1",
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
            .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodUnavailable)))?;
        let payment = sqlx::query_as::<_, PaymentForCheckout>(
            r#"
            SELECT
                id,
                payment,
                enable,
                uuid,
                CAST(config AS TEXT) AS config,
                notify_domain,
                handling_fee_fixed,
                handling_fee_percent
            FROM payment_method
            WHERE id = $1 AND archived_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(method)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodUnavailable)))?;
        let payment = self.decrypt_payment_for_checkout(payment)?;
        if payment.enable != 1 {
            return Err(Problem::new(Code::PaymentMethodUnavailable).into());
        }
        if payment.payment == "StripeCredit" {
            return Err(Problem::new(Code::PaymentMethodUnavailable)
                .with_detail("Stripe payments must be confirmed with Payment Element")
                .into());
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
            return Err(Problem::new(Code::OrderNotFound).into());
        }
        let expected_config = serde_json::from_str::<Value>(&payment.config)
            .map_err(|_| ApiError::from(Problem::new(Code::PaymentConfigInvalid)))?;
        // A shared payment-version lock lets unrelated checkouts use the same
        // gateway concurrently while serializing an archive/toggle. The exact
        // immutable driver/config snapshot must still be active before binding.
        let mut bind_tx = self.db.begin().await?;
        let current_payment =
            sqlx::query_as::<_, (String, String)>(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL)
                .bind(payment.id)
                .fetch_optional(&mut *bind_tx)
                .await?;
        if !payment_config_snapshot_matches(
            &self.config.app_key,
            &payment.uuid,
            current_payment,
            &payment.payment,
            &expected_config,
        ) {
            bind_tx.rollback().await?;
            return Err(Problem::new(Code::PaymentConfigInvalid)
                .with_detail("Payment configuration changed, please try again")
                .into());
        }
        let updated = sqlx::query(
            r#"
            UPDATE orders
            SET payment_id = $1, handling_amount = $2, callback_no = NULL,
                callback_no_hash = NULL, updated_at = $3
            WHERE id = $4 AND status = 0
              AND payment_id IS NOT DISTINCT FROM $5
              AND callback_no IS NOT DISTINCT FROM $6
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
            return Err(Problem::new(Code::OrderNotFound).into());
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
            .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodUnavailable)))?;
        let mut tx = self.db.begin().await?;
        let order = sqlx::query_as::<_, (i64, String, i32, Option<String>, Option<i32>)>(
            r#"
            SELECT id, trade_no, total_amount, callback_no, payment_id
            FROM orders
            WHERE trade_no = $1 AND user_id = $2 AND status = 0
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(&input.trade_no)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;
        if order.2 <= 0 {
            return Err(Problem::new(Code::OrderNotFound).into());
        }

        let payment = sqlx::query_as::<_, PaymentForCheckout>(
            r#"
            SELECT id, payment, enable, uuid, CAST(config AS TEXT) AS config,
                   notify_domain, handling_fee_fixed,
                   handling_fee_percent
            FROM payment_method
            WHERE id = $1 AND payment = 'StripeCredit' AND archived_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(method)
        .fetch_optional(&mut *tx)
        .await?
        .filter(|payment| payment.enable == 1)
        .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodUnavailable)))?;
        let payment = self.decrypt_payment_for_checkout(payment)?;

        let handling_amount =
            calculate_handling_amount_cents(order.2, &payment)?.filter(|amount| *amount != 0);
        let payable_amount = payable_amount_cents(order.2, handling_amount)?;
        tx.commit().await?;

        if order.4 != Some(payment.id)
            && !self
                .cancel_stripe_intent_binding(order.4, order.3.as_deref())
                .await?
        {
            return Err(Problem::new(Code::OrderNotFound).into());
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
        // returns, a shared payment-version lock verifies that the exact immutable
        // driver/config snapshot remains active before binding the intent. An
        // archive that won the race makes this path cancel the new intent.
        let expected_config = serde_json::from_str::<Value>(&payment.config)
            .map_err(|_| ApiError::from(Problem::new(Code::PaymentConfigInvalid)))?;
        let mut bind_tx = self.db.begin().await?;
        let current_payment =
            sqlx::query_as::<_, (String, String)>(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL)
                .bind(payment.id)
                .fetch_optional(&mut *bind_tx)
                .await?;
        let payment_changed = !payment_config_snapshot_matches(
            &self.config.app_key,
            &payment.uuid,
            current_payment,
            "StripeCredit",
            &expected_config,
        );
        let binding_state = if payment_changed {
            bind_tx.rollback().await?;
            "payment_changed"
        } else {
            let intent_label = bounded_payment_identifier(&intent_id);
            let intent_hash = payment_identifier_hash(&intent_id);
            let updated = sqlx::query(
                r#"
                UPDATE orders
                SET payment_id = $1, handling_amount = $2, callback_no = $3,
                    callback_no_hash = $4, updated_at = $5
                WHERE id = $6 AND status = 0
                  AND payment_id IS NOT DISTINCT FROM $7
                  AND callback_no IS NOT DISTINCT FROM $8
                "#,
            )
            .bind(payment.id)
            .bind(handling_amount)
            .bind(intent_label)
            .bind(intent_hash.as_slice())
            .bind(Utc::now().timestamp())
            .bind(order.0)
            .bind(order.4)
            .bind(&order.3)
            .execute(&mut *bind_tx)
            .await?;
            let bound = if updated.rows_affected() == 1 {
                true
            } else {
                let current = sqlx::query_as::<_, (i16, Option<i32>, Option<String>)>(
                    "SELECT status, payment_id, callback_no FROM orders WHERE id = $1",
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
                return Err(Problem::new(Code::StripeBindingInvalid)
                    .with_detail("The Stripe payment has already succeeded")
                    .into());
            }
            return Err(if binding_state == "payment_changed" {
                // §3.4: payment_changed binding-state rejections are
                // stripe_binding_invalid.
                Problem::new(Code::StripeBindingInvalid)
                    .with_detail("Stripe payment configuration changed, please try again")
                    .into()
            } else {
                Problem::new(Code::OrderNotFound).into()
            });
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
        let row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT payment, uuid, CAST(config AS TEXT) FROM payment_method WHERE id = $1 LIMIT 1",
        )
        .bind(payment_id)
        .fetch_optional(&self.db)
        .await?;
        let Some((method, uuid, config)) = row else {
            return Err(Problem::new(Code::StripeBindingInvalid).into());
        };
        if method != "StripeCredit" {
            return Err(Problem::new(Code::StripeBindingInvalid).into());
        }
        let mut config = match crate::payment_secrets::decrypt_payment_config(
            &self.config.app_key,
            &method,
            &uuid,
            &config,
        ) {
            Ok(Value::Object(config)) => config,
            _ => return Err(Problem::new(Code::PaymentConfigInvalid).into()),
        };
        if let Some(secret_key) = config_string(&config, "stripe_sk_live") {
            config.insert(
                "stripe_sk_live".to_string(),
                Value::String(secret_key.trim().to_string()),
            );
        }
        stripe_cancel_intent(&config, intent_id).await
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
            "StripeCredit" => Err(Problem::new(Code::PaymentMethodUnavailable)
                .with_detail("Stripe payments must be confirmed with Payment Element")
                .into()),
            "StripeAlipay" => stripe_source_pay(payment, order, "alipay").await,
            "StripeWepay" => stripe_source_pay(payment, order, "wechat").await,
            "StripeCheckout" => stripe_checkout_pay(payment, order).await,
            "StripeALL" => stripe_all_pay(self, payment, order).await,
            _ => unreachable!("payment provider manifest and checkout dispatch diverged"),
        }
    }

    pub(super) async fn user_email(&self, user_id: i64) -> Result<Option<String>, ApiError> {
        let email =
            sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1 LIMIT 1")
                .bind(user_id)
                .fetch_optional(&self.db)
                .await?;
        Ok(email)
    }
}
