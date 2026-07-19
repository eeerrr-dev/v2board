use std::collections::HashMap;

use chrono::Utc;
use v2board_compat::{ApiError, Code, Problem};
use v2board_db::DbTransaction;

use super::lifecycle::{credit_user_balance, mark_order_paid};
use super::payment_integrations::{
    alipay_f2f_notify, bepusdt_notify, btcpay_notify, coinbase_notify, coinpayments_notify,
    epay_notify, mgate_notify, require_payment_provider, stripe_all_notify, stripe_checkout_notify,
    stripe_payment_intent_notify, stripe_source_notify, wechat_pay_native_notify,
};
use super::{
    OrderForCheckout, OrderService, PaymentForCheckout, bounded_payment_identifier,
    payment_identifier_hash,
};

pub(super) const PAYMENT_SETTLEMENT_ORDER_SQL: &str = r#"
    SELECT id, status, total_amount::BIGINT, handling_amount::BIGINT, user_id, payment_id,
           callback_no, callback_no_hash
    FROM orders
    WHERE trade_no = $1
    LIMIT 1
    FOR UPDATE
"#;
pub(super) const PAYMENT_NOTIFY_LOOKUP_SQL: &str = r#"
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
    WHERE payment = $1 AND uuid = $2
    LIMIT 1
"#;

#[derive(Debug, Clone)]
pub struct PaymentNotifyResponse {
    pub body: String,
    /// Set only when this callback made the fresh `status 0 -> 1` paid transition,
    /// so the HTTP layer can send Laravel's `成功收款` admin Telegram message exactly
    /// once (a gateway replay leaves this `None`). Mirrors `PaymentController::handle`,
    /// which sends the message only inside the `status !== 0` guard.
    pub paid_notice: Option<PaidOrderNotice>,
    /// Set only when authenticated money arrived for an order that can no
    /// longer make the normal pending -> paid transition. The event is already
    /// durable in payment_reconciliation when this notice is returned.
    pub late_payment_notice: Option<LatePaymentNotice>,
}

/// The order fields Laravel's `PaymentController::handle` reads to build the admin
/// `💰成功收款` Telegram message after a fresh paid transition.
#[derive(Debug, Clone)]
pub struct PaidOrderNotice {
    pub trade_no: String,
    pub total_amount: i64,
}

#[derive(Debug, Clone)]
pub struct LatePaymentNotice {
    pub trade_no: String,
    pub trade_no_hash: String,
    pub callback_no: String,
    pub callback_no_hash: String,
    pub reason: &'static str,
    pub order_status: i16,
    pub expected_amount: i64,
    pub settled_amount: Option<i64>,
}

#[derive(Debug, Default)]
struct PaymentSettlementNotices {
    paid: Option<PaidOrderNotice>,
    late: Option<LatePaymentNotice>,
}

#[derive(Clone)]
pub struct PaymentNotifyInput {
    pub params: HashMap<String, String>,
    pub body: Vec<u8>,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub(super) struct VerifiedPaymentNotify {
    pub(super) trade_no: String,
    pub(super) callback_no: String,
    pub(super) custom_result: Option<String>,
    /// Authenticated Stripe metadata user. Non-Stripe gateways leave this empty;
    /// the locked order binding validates it whenever present.
    pub(super) authenticated_user_id: Option<i64>,
    /// Authenticated amount in the order's integer-cent accounting unit. It is
    /// compared only after the matching order row is locked, immediately before
    /// the paid transition.
    pub(super) settled_amount_cents: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedPaymentBinding {
    pub(super) payment_id: i32,
    pub(super) user_id: Option<i64>,
    pub(super) callback_no: Option<String>,
}

fn payment_callback_identity_matches(
    bound_callback_no: Option<&str>,
    bound_callback_no_hash: Option<&[u8]>,
    callback_no: &str,
) -> bool {
    let callback_no_hash = payment_identifier_hash(callback_no);
    bound_callback_no_hash.map_or_else(
        || bound_callback_no == Some(callback_no),
        |bound_hash| bound_hash == callback_no_hash,
    )
}

pub(super) fn payment_binding_matches(
    expected: &ExpectedPaymentBinding,
    order_user_id: i64,
    payment_id: Option<i32>,
    bound_callback_no: Option<&str>,
    bound_callback_no_hash: Option<&[u8]>,
) -> bool {
    Some(expected.payment_id) == payment_id
        && expected
            .user_id
            .is_none_or(|user_id| user_id == order_user_id)
        && expected.callback_no.as_deref().is_none_or(|callback_no| {
            payment_callback_identity_matches(
                bound_callback_no,
                bound_callback_no_hash,
                callback_no,
            )
        })
}

pub(super) fn is_ordinary_payment_replay(
    status: i16,
    bound_callback_no: Option<&str>,
    bound_callback_no_hash: Option<&[u8]>,
    callback_no: &str,
) -> bool {
    matches!(status, 1 | 3 | 4)
        && payment_callback_identity_matches(bound_callback_no, bound_callback_no_hash, callback_no)
}

pub(super) fn should_emit_late_payment_notice(first_observation: bool) -> bool {
    first_observation
}

pub(super) fn payment_amount_matches(
    order_amount_cents: i64,
    handling_amount_cents: Option<i64>,
    settled_amount_cents: i64,
) -> bool {
    order_amount_cents.checked_add(handling_amount_cents.unwrap_or_default())
        == Some(settled_amount_cents)
}

struct PaymentReconciliation<'a> {
    payment_id: i32,
    provider: &'a str,
    trade_no: &'a str,
    callback_no: &'a str,
    reason: &'a str,
    order_status: i16,
    expected_amount: i64,
    settled_amount: Option<i64>,
}

pub(super) fn bounded_payment_audit_identity(value: &str) -> (String, String) {
    (
        bounded_payment_identifier(value),
        hex::encode(payment_identifier_hash(value)),
    )
}

async fn upsert_payment_reconciliation(
    tx: &mut DbTransaction<'_>,
    record: PaymentReconciliation<'_>,
) -> Result<bool, ApiError> {
    let now = Utc::now().timestamp();
    let trade_no_hash = payment_identifier_hash(record.trade_no);
    let callback_no_hash = payment_identifier_hash(record.callback_no);
    let trade_no = bounded_payment_identifier(record.trade_no);
    let callback_no = bounded_payment_identifier(record.callback_no);
    let first_observation = sqlx::query_scalar::<_, bool>(
        r#"
        INSERT INTO payment_reconciliation (
            payment_id, provider, trade_no, trade_no_hash,
            callback_no, callback_no_hash, reason, order_status,
            expected_amount, settled_amount, occurrence_count,
            first_seen_at, last_seen_at, resolved_at, resolution
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, $11, $12, NULL, NULL)
        ON CONFLICT (payment_id, callback_no_hash) DO UPDATE SET
            occurrence_count = payment_reconciliation.occurrence_count + 1,
            last_seen_at = EXCLUDED.last_seen_at
        RETURNING occurrence_count = 1
        "#,
    )
    .bind(record.payment_id)
    .bind(record.provider)
    .bind(trade_no)
    .bind(trade_no_hash.as_slice())
    .bind(callback_no)
    .bind(callback_no_hash.as_slice())
    .bind(record.reason)
    .bind(record.order_status)
    .bind(record.expected_amount)
    .bind(record.settled_amount)
    .bind(now)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;
    Ok(first_observation)
}

#[derive(Debug)]
pub(super) enum PaymentNotifyOutcome {
    Verified(VerifiedPaymentNotify),
    Ignored(String),
}

impl OrderService {
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
        // `enable` and `archived_at` gate new checkouts only. An authenticated
        // callback can arrive after an operator archives any gateway version;
        // rejecting it would strand money already accepted by the provider. The
        // row's driver/config/UUID are immutable and retained for this purpose.

        let outcome = self.verify_payment_notify(&payment, &input).await?;
        let PaymentNotifyOutcome::Verified(verified) = outcome else {
            return Ok(PaymentNotifyResponse {
                body: match outcome {
                    PaymentNotifyOutcome::Ignored(body) => body,
                    PaymentNotifyOutcome::Verified(_) => unreachable!(),
                },
                paid_notice: None,
                late_payment_notice: None,
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
        let notices = self
            .paid_by_trade_no(
                &verified.trade_no,
                &verified.callback_no,
                &payment.payment,
                &expected_binding,
                verified.settled_amount_cents,
            )
            .await?;
        Ok(PaymentNotifyResponse {
            body: verified
                .custom_result
                .unwrap_or_else(|| "success".to_string()),
            paid_notice: notices.paid,
            late_payment_notice: notices.late,
        })
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

    pub async fn paid_manually(&self, trade_no: &str) -> Result<(), ApiError> {
        let expected_binding = sqlx::query_as::<_, (i16, Option<i32>, Option<String>)>(
            "SELECT status, payment_id, callback_no FROM orders WHERE trade_no = $1 LIMIT 1",
        )
        .bind(trade_no)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;
        if expected_binding.0 != 0 {
            return Err(ApiError::from(Problem::new(Code::OrderNotPending)));
        }
        // Manual settlement must first make the browser-held client secret inert.
        // Otherwise the order can be opened manually and then charged by Stripe a
        // moment later, whose webhook will be ignored because the order is no longer
        // pending.
        if !self
            .cancel_stripe_intent_binding(expected_binding.1, expected_binding.2.as_deref())
            .await?
        {
            return Err(ApiError::from(Problem::new(Code::OrderNotPending)));
        }

        let mut tx = self.db.begin().await?;
        let Some(order) = sqlx::query_as::<_, OrderForCheckout>(
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
            WHERE trade_no = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Err(ApiError::from(Problem::new(Code::OrderNotFound)));
        };
        let current_binding = sqlx::query_as::<_, (i16, Option<i32>, Option<String>)>(
            "SELECT status, payment_id, callback_no FROM orders WHERE id = $1",
        )
        .bind(order.id)
        .fetch_one(&mut *tx)
        .await?;
        if current_binding != expected_binding {
            return Err(ApiError::from(Problem::new(Code::OrderNotPending)));
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
            sqlx::query_as::<_, (i16, i64, Option<i32>, Option<String>)>(
                r#"
                SELECT status, created_at, payment_id, callback_no
                FROM orders
                WHERE trade_no = $1
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
                "type",
                period,
                trade_no,
                total_amount,
                refund_amount,
                surplus_order_ids::text AS surplus_order_ids
            FROM orders
            WHERE trade_no = $1
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
            sqlx::query_as::<_, (i16, i64, Option<i32>, Option<i32>, Option<String>)>(
                "SELECT status, created_at, balance_amount, payment_id, callback_no FROM orders WHERE id = $1",
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
                sqlx::query("UPDATE orders SET status = 2, updated_at = $1 WHERE id = $2")
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
        provider: &str,
        expected_binding: &ExpectedPaymentBinding,
        settled_amount_cents: Option<i64>,
    ) -> Result<PaymentSettlementNotices, ApiError> {
        // Commit the paid mark in its OWN transaction first, mirroring Laravel's
        // OrderService::paid() which save()s status=1 before dispatching OrderHandleJob.
        // A gateway-confirmed payment must be durable even if opening the order later
        // fails; folding the open into the same transaction (as before) meant a transient
        // error while granting the plan would roll back real, already-received money.
        let total_amount;
        {
            let mut tx = self.db.begin().await?;
            let payment_exists: Option<i32> =
                sqlx::query_scalar("SELECT id FROM payment_method WHERE id = $1 LIMIT 1 FOR SHARE")
                    .bind(expected_binding.payment_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if payment_exists.is_none() {
                return Err(ApiError::legacy(
                    "Payment verification material no longer exists",
                ));
            }
            let Some((
                order_id,
                status,
                amount,
                handling_amount,
                order_user_id,
                payment_id,
                bound_callback_no,
                bound_callback_no_hash,
            )) = sqlx::query_as::<
                _,
                (
                    i64,
                    i16,
                    i64,
                    Option<i64>,
                    i64,
                    Option<i32>,
                    Option<String>,
                    Option<Vec<u8>>,
                ),
            >(PAYMENT_SETTLEMENT_ORDER_SQL)
            .bind(trade_no)
            .fetch_optional(&mut *tx)
            .await?
            else {
                let rows_affected = upsert_payment_reconciliation(
                    &mut tx,
                    PaymentReconciliation {
                        payment_id: expected_binding.payment_id,
                        provider,
                        trade_no,
                        callback_no,
                        reason: "order_not_found",
                        order_status: -1,
                        expected_amount: 0,
                        settled_amount: settled_amount_cents,
                    },
                )
                .await?;
                tx.commit().await?;
                let (trade_no_label, trade_no_hash) = bounded_payment_audit_identity(trade_no);
                let (callback_no_label, callback_no_hash) =
                    bounded_payment_audit_identity(callback_no);
                tracing::error!(
                    trade_no = %trade_no_label,
                    trade_no_hash,
                    callback_no = %callback_no_label,
                    callback_no_hash,
                    provider,
                    reason = "order_not_found",
                    "authenticated payment requires reconciliation"
                );
                return Ok(PaymentSettlementNotices {
                    paid: None,
                    late: should_emit_late_payment_notice(rows_affected).then_some(
                        LatePaymentNotice {
                            trade_no: trade_no_label,
                            trade_no_hash,
                            callback_no: callback_no_label,
                            callback_no_hash,
                            reason: "order_not_found",
                            order_status: -1,
                            expected_amount: 0,
                            settled_amount: settled_amount_cents,
                        },
                    ),
                });
            };
            // Every callback must still belong to the payment method currently
            // bound to this exact locked row. This closes the method-switch TOCTOU
            // gap for all gateways. Stripe additionally checks authenticated user
            // metadata, and Payment Element checks its pre-bound intent id.
            let binding_matches = payment_binding_matches(
                expected_binding,
                order_user_id,
                payment_id,
                bound_callback_no.as_deref(),
                bound_callback_no_hash.as_deref(),
            );
            let amount_matches = settled_amount_cents.is_none_or(|settled_amount_cents| {
                payment_amount_matches(amount, handling_amount, settled_amount_cents)
            });
            let previously_reconciled = if binding_matches && amount_matches && status == 0 {
                let callback_no_hash = payment_identifier_hash(callback_no);
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM payment_reconciliation \
                     WHERE payment_id = $1 AND callback_no_hash = $2)",
                )
                .bind(expected_binding.payment_id)
                .bind(callback_no_hash.as_slice())
                .fetch_one(&mut *tx)
                .await?
            } else {
                false
            };
            let reconciliation_reason = if !binding_matches {
                Some("payment_binding_mismatch")
            } else if !amount_matches {
                Some("settled_amount_mismatch")
            } else if previously_reconciled {
                Some("previously_reconciled")
            } else if status != 0 {
                let ordinary_replay = is_ordinary_payment_replay(
                    status,
                    bound_callback_no.as_deref(),
                    bound_callback_no_hash.as_deref(),
                    callback_no,
                );
                if ordinary_replay {
                    let callback_no_hash = payment_identifier_hash(callback_no);
                    if bound_callback_no_hash.as_deref() != Some(callback_no_hash.as_slice()) {
                        sqlx::query("UPDATE orders SET callback_no_hash = $1 WHERE id = $2")
                            .bind(callback_no_hash.as_slice())
                            .bind(order_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                    tx.commit().await?;
                    return Ok(PaymentSettlementNotices::default());
                }
                Some("order_not_pending")
            } else {
                None
            };
            if let Some(reason) = reconciliation_reason {
                let expected_amount = amount
                    .checked_add(handling_amount.unwrap_or_default())
                    .ok_or_else(|| {
                        ApiError::legacy("Payment amount is outside the supported range")
                    })?;
                let rows_affected = upsert_payment_reconciliation(
                    &mut tx,
                    PaymentReconciliation {
                        payment_id: expected_binding.payment_id,
                        provider,
                        trade_no,
                        callback_no,
                        reason,
                        order_status: status,
                        expected_amount,
                        settled_amount: settled_amount_cents,
                    },
                )
                .await?;
                tx.commit().await?;
                let (trade_no_label, trade_no_hash) = bounded_payment_audit_identity(trade_no);
                let (callback_no_label, callback_no_hash) =
                    bounded_payment_audit_identity(callback_no);
                tracing::error!(
                    trade_no = %trade_no_label,
                    trade_no_hash,
                    callback_no = %callback_no_label,
                    callback_no_hash,
                    provider,
                    order_status = status,
                    reason,
                    "authenticated payment requires reconciliation"
                );
                return Ok(PaymentSettlementNotices {
                    paid: None,
                    late: should_emit_late_payment_notice(rows_affected).then_some(
                        LatePaymentNotice {
                            trade_no: trade_no_label,
                            trade_no_hash,
                            callback_no: callback_no_label,
                            callback_no_hash,
                            reason,
                            order_status: status,
                            expected_amount,
                            settled_amount: settled_amount_cents,
                        },
                    ),
                });
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
        Ok(PaymentSettlementNotices {
            paid: Some(PaidOrderNotice {
                trade_no: trade_no.to_string(),
                total_amount,
            }),
            late: None,
        })
    }
}
