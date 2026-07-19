use std::sync::Arc;

use rust_decimal::Decimal;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use v2board_compat::ApiError;
use v2board_config::AppConfig;
use v2board_db::DbPool;

mod checkout;
mod lifecycle;
mod payment_integrations;
mod settlement;

use lifecycle::{commission_amount, round_cents};
use settlement::{PaymentNotifyOutcome, VerifiedPaymentNotify};

pub use lifecycle::generate_order_no;
pub use settlement::{
    LatePaymentNotice, PaidOrderNotice, PaymentNotifyInput, PaymentNotifyResponse,
};

#[cfg(test)]
use checkout::{
    PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL, UNFINISHED_ORDER_FOR_UPDATE_SQL, payable_amount_cents,
    payment_config_snapshot_matches,
};
#[cfg(test)]
use lifecycle::{
    USER_FOR_ORDER_SQL, add_months, add_period_time, apply_vip_discount, buy_by_one_time,
    buy_by_period, calculate_handling_amount_cents, commission_is_eligible, percent,
};
#[cfg(test)]
use settlement::{
    ExpectedPaymentBinding, PAYMENT_NOTIFY_LOOKUP_SQL, PAYMENT_SETTLEMENT_ORDER_SQL,
    bounded_payment_audit_identity, is_ordinary_payment_replay, payment_amount_matches,
    payment_binding_matches, should_emit_late_payment_notice,
};

const GIB: i64 = 1_073_741_824;
const UNFINISHED_ORDER_UNIQUE_KEY: &str = "uniq_unfinished_order_per_user";

#[derive(Clone)]
pub struct OrderService {
    db: DbPool,
    config: Arc<AppConfig>,
}

/// The §5.5 create-order union (docs/api-dialect.md W4). The API layer
/// deserializes the discriminated `{kind: "plan" | "deposit"}` request body
/// and hands the domain the already-structural arm — the legacy
/// `plan_id: 0` + `period: "deposit"` sentinel is gone.
#[derive(Debug, Clone)]
pub enum SaveOrderInput {
    Plan {
        plan_id: i32,
        period: String,
        coupon_code: Option<String>,
    },
    Deposit {
        deposit_amount: i32,
    },
}

#[derive(Debug, Clone)]
pub struct CheckoutOrderInput {
    pub trade_no: String,
    pub method: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckoutResult {
    pub r#type: i16,
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
    invite_user_id: Option<i64>,
    commission_balance: Decimal,
}

#[derive(Debug, Clone, FromRow)]
struct UserForOrder {
    id: i64,
    invite_user_id: Option<i64>,
    balance: i32,
    discount: Option<i32>,
    commission_type: i16,
    commission_rate: Option<i32>,
    traffic_epoch: i64,
    u: i64,
    d: i64,
    transfer_enable: i64,
    device_limit: Option<i32>,
    banned: i16,
    group_id: Option<i32>,
    plan_id: Option<i32>,
    speed_limit: Option<i32>,
    expired_at: Option<i64>,
}

#[derive(Debug, Clone, FromRow)]
struct CouponRow {
    id: i32,
    r#type: i16,
    value: i32,
    show: i16,
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

#[derive(Clone, FromRow)]
struct PaymentForCheckout {
    id: i32,
    payment: String,
    enable: i16,
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

pub(super) fn payment_identifier_hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

pub(super) fn bounded_payment_identifier(value: &str) -> String {
    const MAX_BYTES: usize = 255;
    let mut bounded = String::with_capacity(value.len().min(MAX_BYTES));
    for character in value.chars() {
        if character.len_utf8() == 4 {
            let escaped = format!("\\u{{{:X}}}", u32::from(character));
            if bounded.len() + escaped.len() > MAX_BYTES {
                break;
            }
            bounded.push_str(&escaped);
        } else {
            let mut bytes = [0_u8; 4];
            let encoded = character.encode_utf8(&mut bytes);
            if bounded.len() + encoded.len() > MAX_BYTES {
                break;
            }
            bounded.push_str(encoded);
        }
    }
    bounded
}

impl OrderService {
    pub fn new(db: DbPool, config: Arc<AppConfig>) -> Self {
        Self { db, config }
    }

    /// `payment_method.config` is stored as an at-rest AES-256-GCM envelope
    /// bound to the row's driver and uuid. Every checkout/notify read decrypts
    /// it back to the exact plaintext JSON text the gateway integrations
    /// consume; a non-envelope or unauthentic column is a hard integrity
    /// failure, never a plaintext fallback.
    fn decrypt_payment_for_checkout(
        &self,
        mut payment: PaymentForCheckout,
    ) -> Result<PaymentForCheckout, ApiError> {
        let plaintext = crate::payment_secrets::decrypt_payment_config_canonical(
            &self.config.app_key,
            &payment.payment,
            &payment.uuid,
            &payment.config,
        )
        .map_err(|error| {
            ApiError::internal(format!("stored payment config failed decryption: {error}"))
        })?;
        payment.config = String::from_utf8(plaintext)
            .map_err(|_| ApiError::internal("decrypted payment config is not UTF-8"))?;
        Ok(payment)
    }
}
#[cfg(test)]
mod tests;
