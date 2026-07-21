use std::{collections::HashMap, sync::Arc};

use serde_json::Value;
use v2board_application::order::{
    CheckoutOutcome, GatewayOrder, OrderFailure, OrderPortError, PaymentGateway, PaymentMethod,
    PaymentNotifyInput as ApplicationPaymentNotifyInput, PaymentSnapshotVerifier,
    PaymentVerification, PortResult, PreparedStripeIntent, StripePaymentIntent,
    VerifiedPaymentNotification,
};
use v2board_compat::{ApiError, Code};
use v2board_config::AppConfig;

use crate::payment_secrets;

mod integrations;
#[cfg(test)]
mod tests;

use integrations::{
    alipay_f2f_notify, alipay_f2f_pay, bepusdt_notify, bepusdt_pay, btcpay_notify, btcpay_pay,
    coinbase_notify, coinbase_pay, coinpayments_notify, coinpayments_pay, epay_notify, epay_pay,
    mgate_notify, mgate_pay, payment_config, payment_notify_url, payment_return_url,
    require_payment_provider, stripe_all_notify, stripe_all_pay, stripe_cancel_intent,
    stripe_checkout_notify, stripe_checkout_pay, stripe_credit_prepare,
    stripe_payment_intent_notify, stripe_source_notify, stripe_source_pay,
    wechat_pay_native_notify, wechat_pay_native_pay,
};

#[derive(Clone)]
pub struct RuntimePaymentGateway {
    config: Arc<AppConfig>,
}

impl RuntimePaymentGateway {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }

    fn open_payment(&self, payment: &PaymentMethod) -> Result<PaymentForCheckout, ApiError> {
        let plaintext = payment_secrets::decrypt_payment_config_canonical(
            &self.config.app_key,
            &payment.provider,
            &payment.uuid,
            &payment.sealed_config,
        )
        .map_err(|error| {
            ApiError::internal(format!("stored payment config failed decryption: {error}"))
        })?;
        let config = String::from_utf8(plaintext)
            .map_err(|_| ApiError::internal("decrypted payment config is not UTF-8"))?;
        Ok(PaymentForCheckout {
            id: payment.id,
            payment: payment.provider.clone(),
            uuid: payment.uuid.clone(),
            config,
            notify_domain: payment.notify_domain.clone(),
        })
    }

    fn payment_order(&self, payment: &PaymentForCheckout, order: &GatewayOrder) -> PaymentOrder {
        PaymentOrder {
            notify_url: payment_notify_url(&self.config, payment),
            return_url: payment_return_url(&self.config, &order.trade_no),
            trade_no: order.trade_no.clone(),
            total_amount: order.total_amount,
            user_id: order.user_id,
            user_email: order.user_email.clone(),
        }
    }
}

impl PaymentSnapshotVerifier for RuntimePaymentGateway {
    fn equivalent(&self, expected: &PaymentMethod, current: &PaymentMethod) -> PortResult<bool> {
        if expected.id != current.id
            || expected.provider != current.provider
            || expected.uuid != current.uuid
            || !current.enabled
            || expected.notify_domain != current.notify_domain
            || expected.handling_fee_fixed != current.handling_fee_fixed
            || expected.handling_fee_percent != current.handling_fee_percent
        {
            return Ok(false);
        }
        let expected = self
            .open_payment(expected)
            .map_err(|error| payment_config_error("decrypt expected payment snapshot", error))?;
        let current = self
            .open_payment(current)
            .map_err(|error| payment_config_error("decrypt current payment snapshot", error))?;
        let expected = serde_json::from_str::<Value>(&expected.config)
            .map_err(|error| gateway_error("decode expected payment snapshot", error))?;
        let current = serde_json::from_str::<Value>(&current.config)
            .map_err(|error| gateway_error("decode current payment snapshot", error))?;
        Ok(expected == current)
    }
}

impl PaymentGateway for RuntimePaymentGateway {
    async fn checkout(
        &self,
        payment: &PaymentMethod,
        order: &GatewayOrder,
    ) -> PortResult<CheckoutOutcome> {
        let payment = self
            .open_payment(payment)
            .map_err(|error| payment_config_error("open checkout payment", error))?;
        let order = self.payment_order(&payment, order);
        let provider = require_payment_provider(&payment.payment)
            .map_err(|error| gateway_api_error("select checkout provider", error))?;
        let result = match provider.code {
            "EPay" => epay_pay(&payment, &order),
            "MGate" => mgate_pay(&payment, &order).await,
            "BEasyPaymentUSDT" => bepusdt_pay(&payment, &order).await,
            "CoinPayments" => coinpayments_pay(&payment, &order),
            "Coinbase" => coinbase_pay(&payment, &order).await,
            "BTCPay" => btcpay_pay(&payment, &order).await,
            "WechatPayNative" => wechat_pay_native_pay(&payment, &order).await,
            "AlipayF2F" => alipay_f2f_pay(&self.config, &payment, &order).await,
            "StripeCredit" => {
                return Err(OrderPortError::business(
                    "checkout",
                    OrderFailure::PaymentMethodUnavailable,
                    "Stripe payments must be confirmed with Payment Element",
                ));
            }
            "StripeAlipay" => stripe_source_pay(&payment, &order, "alipay").await,
            "StripeWepay" => stripe_source_pay(&payment, &order, "wechat").await,
            "StripeCheckout" => stripe_checkout_pay(&payment, &order).await,
            "StripeALL" => stripe_all_pay(&payment, &order).await,
            _ => unreachable!("payment provider manifest and checkout dispatch diverged"),
        }
        .map_err(|error| gateway_api_error("execute checkout", error))?;
        match result.r#type {
            0 => result
                .data
                .as_str()
                .map(|value| CheckoutOutcome::QrCode(value.to_string()))
                .ok_or_else(|| {
                    OrderPortError::infrastructure("decode checkout", "QR payload is not text")
                }),
            1 => result
                .data
                .as_str()
                .map(|value| CheckoutOutcome::Redirect(value.to_string()))
                .ok_or_else(|| {
                    OrderPortError::infrastructure(
                        "decode checkout",
                        "redirect payload is not text",
                    )
                }),
            kind => Err(OrderPortError::infrastructure(
                "decode checkout",
                format!("unsupported checkout result type {kind}"),
            )),
        }
    }

    async fn prepare_stripe_intent(
        &self,
        payment: &PaymentMethod,
        order: &GatewayOrder,
        reusable_intent: Option<&str>,
    ) -> PortResult<PreparedStripeIntent> {
        let payment = self
            .open_payment(payment)
            .map_err(|error| payment_config_error("open Stripe payment", error))?;
        let order = self.payment_order(&payment, order);
        let (intent_id, prepared) = stripe_credit_prepare(&payment, &order, reusable_intent)
            .await
            .map_err(|error| gateway_api_error("prepare Stripe intent", error))?;
        Ok(PreparedStripeIntent {
            intent_id,
            response: StripePaymentIntent {
                public_key: prepared.public_key,
                client_secret: prepared.client_secret,
                amount: prepared.amount,
                currency: prepared.currency,
            },
        })
    }

    async fn cancel_stripe_intent(
        &self,
        payment: Option<&PaymentMethod>,
        intent_id: Option<&str>,
    ) -> PortResult<bool> {
        let (Some(payment), Some(intent_id)) = (payment, intent_id) else {
            return Ok(true);
        };
        if !intent_id.starts_with("pi_") {
            return Ok(true);
        }
        if payment.provider != "StripeCredit" {
            return Err(OrderPortError::business(
                "cancel Stripe intent",
                OrderFailure::StripeBindingInvalid,
                "bound payment is not StripeCredit",
            ));
        }
        let payment = self
            .open_payment(payment)
            .map_err(|error| payment_config_error("open Stripe cancellation payment", error))?;
        let config = payment_config(&payment)
            .map_err(|error| payment_config_error("decode Stripe cancellation payment", error))?;
        stripe_cancel_intent(&config, intent_id)
            .await
            .map_err(|error| gateway_api_error("cancel Stripe intent", error))
    }

    async fn verify_notification(
        &self,
        payment: &PaymentMethod,
        input: &ApplicationPaymentNotifyInput,
    ) -> PortResult<PaymentVerification> {
        let payment = self
            .open_payment(payment)
            .map_err(|error| payment_config_error("open notification payment", error))?;
        let input = PaymentNotifyInput {
            params: input.params.clone().into_iter().collect(),
            body: input.body.clone(),
            headers: input.headers.clone().into_iter().collect(),
        };
        let provider = require_payment_provider(&payment.payment)
            .map_err(|error| gateway_api_error("select notification provider", error))?;
        let result = match provider.code {
            "EPay" => epay_notify(&payment, &input.params),
            "MGate" => mgate_notify(&payment, &input.params),
            "BEasyPaymentUSDT" => bepusdt_notify(&payment, &input.params),
            "CoinPayments" => coinpayments_notify(&payment, &input),
            "Coinbase" => coinbase_notify(&payment, &input),
            "BTCPay" => btcpay_notify(&payment, &input).await,
            "WechatPayNative" => wechat_pay_native_notify(&payment, &input),
            "AlipayF2F" => alipay_f2f_notify(&payment, &input.params),
            "StripeCredit" => stripe_payment_intent_notify(&payment, &input),
            "StripeAlipay" | "StripeWepay" => stripe_source_notify(&payment, &input).await,
            "StripeCheckout" => stripe_checkout_notify(&payment, &input),
            "StripeALL" => stripe_all_notify(&payment, &input),
            _ => unreachable!("payment provider manifest and notify dispatch diverged"),
        }
        .map_err(|error| gateway_api_error("verify payment notification", error))?;
        Ok(match result {
            PaymentNotifyOutcome::Ignored(body) => PaymentVerification::Ignored(body),
            PaymentNotifyOutcome::Verified(verified) => {
                PaymentVerification::Verified(VerifiedPaymentNotification {
                    trade_no: verified.trade_no,
                    callback_no: verified.callback_no,
                    custom_response: verified.custom_result,
                    authenticated_user_id: verified.authenticated_user_id,
                    settled_amount_cents: verified.settled_amount_cents,
                })
            }
        })
    }
}

fn gateway_error(operation: &'static str, error: impl std::fmt::Display) -> OrderPortError {
    OrderPortError::infrastructure(operation, error)
}

fn payment_config_error(operation: &'static str, error: impl std::fmt::Display) -> OrderPortError {
    OrderPortError::business(
        operation,
        OrderFailure::PaymentConfigInvalid,
        error.to_string(),
    )
}

fn gateway_api_error(operation: &'static str, error: ApiError) -> OrderPortError {
    let failure = match &error {
        ApiError::Problem(problem) => match problem.code() {
            Code::PaymentMethodUnavailable => Some(OrderFailure::PaymentMethodUnavailable),
            Code::PaymentConfigInvalid => Some(OrderFailure::PaymentConfigInvalid),
            Code::PaymentGatewayUnsupported => Some(OrderFailure::PaymentGatewayUnsupported),
            Code::PaymentAmountOutOfRange => Some(OrderFailure::PaymentAmountOutOfRange),
            Code::HandlingFeeOutOfRange => Some(OrderFailure::HandlingFeeOutOfRange),
            Code::StripeBindingInvalid => Some(OrderFailure::StripeBindingInvalid),
            _ => None,
        },
        _ => None,
    };
    if let Some(failure) = failure {
        OrderPortError::business(operation, failure, error.to_string())
    } else {
        OrderPortError::infrastructure(operation, error)
    }
}

#[derive(Clone)]
struct PaymentForCheckout {
    id: i32,
    payment: String,
    uuid: String,
    config: String,
    notify_domain: Option<String>,
}

#[derive(Debug, Clone)]
struct PaymentOrder {
    notify_url: String,
    return_url: String,
    trade_no: String,
    total_amount: i32,
    user_id: i64,
    user_email: Option<String>,
}

#[derive(Debug, Clone)]
struct CheckoutResult {
    r#type: i16,
    data: Value,
}

#[derive(Debug, Clone)]
struct StripePaymentIntentResult {
    public_key: String,
    client_secret: String,
    amount: i64,
    currency: String,
}

#[derive(Clone)]
struct PaymentNotifyInput {
    params: HashMap<String, String>,
    body: Vec<u8>,
    headers: HashMap<String, String>,
}

#[derive(Debug)]
enum PaymentNotifyOutcome {
    Verified(VerifiedPaymentNotify),
    Ignored(String),
}

#[derive(Debug, Clone)]
struct VerifiedPaymentNotify {
    trade_no: String,
    callback_no: String,
    custom_result: Option<String>,
    authenticated_user_id: Option<i64>,
    settled_amount_cents: Option<i64>,
}
