//! Field-level transport contracts for the authenticated plan, order,
//! checkout, payment-method, and coupon surface.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{time::Rfc3339Timestamp, user::UserPlan};

/// The plan attached to an order. Deposit orders retain their established
/// minimal marker while subscription orders carry the canonical user plan.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum OrderPlan {
    Full(Box<UserPlan>),
    Deposit(DepositOrderPlan),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DepositOrderPlan {
    #[schema(minimum = 0, maximum = 0)]
    pub id: i32,
    #[schema(pattern = "^deposit$")]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserOrder {
    pub trade_no: String,
    #[schema(required)]
    pub callback_no: Option<String>,
    #[schema(minimum = 0)]
    pub plan_id: i32,
    #[schema(required)]
    pub coupon_id: Option<i32>,
    #[schema(required)]
    pub payment_id: Option<i32>,
    /// Order kind code: 1, 2, 3, 4, or 9.
    #[schema(minimum = 1, maximum = 9)]
    pub r#type: i32,
    #[schema(
        pattern = "^(month_price|quarter_price|half_year_price|year_price|two_year_price|three_year_price|onetime_price|reset_price|deposit)$"
    )]
    pub period: String,
    pub total_amount: i32,
    #[schema(required)]
    pub handling_amount: Option<i32>,
    #[schema(required)]
    pub discount_amount: Option<i32>,
    #[schema(required)]
    pub surplus_amount: Option<i32>,
    #[schema(required)]
    pub refund_amount: Option<i32>,
    #[schema(required)]
    pub balance_amount: Option<i32>,
    #[schema(required)]
    pub surplus_order_ids: Option<Vec<i64>>,
    #[schema(minimum = 0, maximum = 4)]
    pub status: i16,
    #[schema(minimum = 0, maximum = 3)]
    pub commission_status: i16,
    pub commission_balance: i32,
    #[schema(required)]
    pub actual_commission_balance: Option<i32>,
    #[schema(required)]
    pub invite_user_id: Option<i64>,
    #[schema(required)]
    pub paid_at: Option<Rfc3339Timestamp>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<OrderPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub try_out_plan_id: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(no_recursion)]
    pub surplus_orders: Option<Vec<UserOrder>>,
    /// Historical wire spelling retained by the modern dialect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounus: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get_amount: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PaymentMethod {
    pub id: i32,
    pub name: String,
    pub payment: String,
    #[schema(required)]
    pub icon: Option<String>,
    #[schema(required)]
    pub handling_fee_fixed: Option<i32>,
    #[schema(required)]
    pub handling_fee_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Coupon {
    pub id: i32,
    pub code: String,
    pub name: String,
    #[schema(minimum = 1, maximum = 2)]
    pub r#type: i16,
    pub value: i32,
    pub show: bool,
    #[schema(required)]
    pub limit_use: Option<i32>,
    #[schema(required)]
    pub limit_use_with_user: Option<i32>,
    #[schema(required)]
    pub limit_plan_ids: Option<Vec<i32>>,
    #[schema(required)]
    pub limit_period: Option<Vec<String>>,
    pub started_at: Rfc3339Timestamp,
    pub ended_at: Rfc3339Timestamp,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

/// A plan purchase or a balance deposit, discriminated without legacy
/// `plan_id: 0` / `period: "deposit"` sentinels.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreateOrderRequest {
    Plan(PlanOrderRequest),
    Deposit(DepositOrderRequest),
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanOrderRequest {
    #[schema(minimum = 1)]
    pub plan_id: i32,
    #[schema(
        pattern = "^(month_price|quarter_price|half_year_price|year_price|two_year_price|three_year_price|onetime_price|reset_price)$"
    )]
    pub period: String,
    /// Omitted when no coupon is applied; an empty-string sentinel is not a
    /// valid client representation.
    #[serde(default)]
    pub coupon_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DepositOrderRequest {
    #[schema(minimum = 1, maximum = 9_999_998)]
    pub deposit_amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OrderStatus {
    #[schema(minimum = 0, maximum = 4)]
    pub status: i16,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckoutRequest {
    /// Optional only for zero-total orders, which settle without a gateway.
    #[serde(default)]
    #[schema(minimum = 1)]
    pub method_id: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StripeIntentRequest {
    #[schema(minimum = 1)]
    pub method_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckoutOutcome {
    QrCode { payload: String },
    Redirect { url: String },
    Settled,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StripePaymentIntent {
    #[schema(min_length = 1)]
    pub public_key: String,
    #[schema(min_length = 1)]
    pub client_secret: String,
    #[schema(minimum = 1)]
    pub amount: i64,
    #[schema(pattern = "^[a-z]{3}$")]
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CouponCheckRequest {
    #[schema(min_length = 1)]
    pub code: String,
    #[serde(default)]
    #[schema(minimum = 1)]
    pub plan_id: Option<i32>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn create_order_union_rejects_legacy_sentinels_and_mixed_arms() {
        assert!(
            serde_json::from_value::<CreateOrderRequest>(json!({
                "kind": "plan",
                "plan_id": 2,
                "period": "month_price",
                "deposit_amount": 500
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<CreateOrderRequest>(json!({
                "kind": "deposit",
                "deposit_amount": 500,
                "period": "deposit"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<CreateOrderRequest>(json!({
                "plan_id": 2,
                "period": "month_price"
            }))
            .is_err()
        );
    }

    #[test]
    fn checkout_union_serializes_with_a_stable_discriminator() {
        assert_eq!(
            serde_json::to_value(CheckoutOutcome::Settled).expect("settled"),
            json!({ "kind": "settled" })
        );
        assert_eq!(
            serde_json::to_value(CheckoutOutcome::QrCode {
                payload: "qr-payload".to_string()
            })
            .expect("QR checkout"),
            json!({ "kind": "qr_code", "payload": "qr-payload" })
        );
    }

    #[test]
    fn checkout_request_is_closed_and_method_is_optional() {
        let zero_total =
            serde_json::from_value::<CheckoutRequest>(json!({})).expect("zero-total checkout");
        assert_eq!(zero_total.method_id, None);
        assert!(serde_json::from_value::<CheckoutRequest>(json!({ "method": 3 })).is_err());
    }

    #[test]
    fn stripe_intent_requires_the_gateway_method() {
        assert!(serde_json::from_value::<StripeIntentRequest>(json!({})).is_err());
        let request = serde_json::from_value::<StripeIntentRequest>(json!({ "method_id": 3 }))
            .expect("Stripe method");
        assert_eq!(request.method_id, 3);
    }
}
