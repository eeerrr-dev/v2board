//! Transport contracts for the administrative business surfaces.
//!
//! These types deliberately describe the HTTP wire rather than database rows
//! or application-service inputs. Built-in payment-provider configuration is
//! a closed, provider-discriminated contract: neither arbitrary keys nor
//! non-string values cross the administrative write boundary.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, de};
use utoipa::ToSchema;

use crate::{patch::NonNull, time::Rfc3339Timestamp};

// --- Shared admin filter DSL -------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AdminFilterOperator {
    Eq,
    Neq,
    Like,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
}

/// JSON number preserving the integer/unsigned/decimal distinction needed by
/// the filter DSL's second-pass column validation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum AdminFilterNumber {
    Integer(i64),
    Unsigned(u64),
    Decimal(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum AdminFilterScalar {
    Bool(bool),
    Number(AdminFilterNumber),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum AdminFilterValue {
    Null,
    Bool(bool),
    Number(AdminFilterNumber),
    String(String),
    Array(Vec<AdminFilterScalar>),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminFilterClause {
    pub field: String,
    pub op: AdminFilterOperator,
    pub value: AdminFilterValue,
}

// --- Payments ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum PaymentProviderCode {
    AlipayF2F,
    BEasyPaymentUSDT,
    BTCPay,
    CoinPayments,
    Coinbase,
    EPay,
    MGate,
    StripeALL,
    StripeAlipay,
    StripeCheckout,
    StripeCredit,
    StripeWepay,
    WechatPayNative,
}

impl PaymentProviderCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlipayF2F => "AlipayF2F",
            Self::BEasyPaymentUSDT => "BEasyPaymentUSDT",
            Self::BTCPay => "BTCPay",
            Self::CoinPayments => "CoinPayments",
            Self::Coinbase => "Coinbase",
            Self::EPay => "EPay",
            Self::MGate => "MGate",
            Self::StripeALL => "StripeALL",
            Self::StripeAlipay => "StripeAlipay",
            Self::StripeCheckout => "StripeCheckout",
            Self::StripeCredit => "StripeCredit",
            Self::StripeWepay => "StripeWepay",
            Self::WechatPayNative => "WechatPayNative",
        }
    }
}

macro_rules! payment_config {
    ($name:ident { $($field:ident $(=> $wire:literal)?),+ $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $(
                $(#[serde(rename = $wire)])?
                pub $field: String,
            )+
        }
    };
}

payment_config!(AlipayF2FConfig {
    app_id,
    private_key,
    public_key,
    product_name,
});
payment_config!(BEasyPaymentUsdtConfig {
    bepusdt_url,
    bepusdt_apitoken,
    bepusdt_trade_type,
});
payment_config!(BtcPayConfig {
    btcpay_url,
    btcpay_store_id => "btcpay_storeId",
    btcpay_api_key,
    btcpay_webhook_key,
});
payment_config!(CoinPaymentsConfig {
    coinpayments_merchant_id,
    coinpayments_ipn_secret,
    coinpayments_currency,
});
payment_config!(CoinbaseConfig {
    coinbase_url,
    coinbase_api_key,
    coinbase_webhook_key,
});
payment_config!(EPayConfig { url, pid, key, r#type => "type" });
payment_config!(MGateConfig {
    mgate_url,
    mgate_app_id,
    mgate_app_secret,
    mgate_source_currency,
});
payment_config!(StripeAllConfig {
    currency,
    stripe_sk_live,
    stripe_webhook_key,
    payment_method,
});
payment_config!(StripeAlipayConfig {
    currency,
    stripe_sk_live,
    stripe_webhook_key,
});
payment_config!(StripeCheckoutConfig {
    currency,
    stripe_sk_live,
    stripe_pk_live,
    stripe_webhook_key,
    stripe_custom_field_name,
});
payment_config!(StripeCreditConfig {
    currency,
    stripe_sk_live,
    stripe_pk_live,
    stripe_webhook_key,
});
payment_config!(StripeWepayConfig {
    currency,
    stripe_sk_live,
    stripe_webhook_key,
});
payment_config!(WechatPayNativeConfig {
    app_id,
    mch_id,
    api_key,
});

/// Typed provider configuration after the root request discriminator has been
/// decoded. This enum is an application-adapter hand-off type, not a second
/// wire envelope.
#[derive(Debug, Clone)]
pub enum PaymentProviderConfig {
    AlipayF2F(AlipayF2FConfig),
    BEasyPaymentUSDT(BEasyPaymentUsdtConfig),
    BTCPay(BtcPayConfig),
    CoinPayments(CoinPaymentsConfig),
    Coinbase(CoinbaseConfig),
    EPay(EPayConfig),
    MGate(MGateConfig),
    StripeALL(StripeAllConfig),
    StripeAlipay(StripeAlipayConfig),
    StripeCheckout(StripeCheckoutConfig),
    StripeCredit(StripeCreditConfig),
    StripeWepay(StripeWepayConfig),
    WechatPayNative(WechatPayNativeConfig),
}

impl PaymentProviderConfig {
    #[must_use]
    pub const fn code(&self) -> PaymentProviderCode {
        match self {
            Self::AlipayF2F(_) => PaymentProviderCode::AlipayF2F,
            Self::BEasyPaymentUSDT(_) => PaymentProviderCode::BEasyPaymentUSDT,
            Self::BTCPay(_) => PaymentProviderCode::BTCPay,
            Self::CoinPayments(_) => PaymentProviderCode::CoinPayments,
            Self::Coinbase(_) => PaymentProviderCode::Coinbase,
            Self::EPay(_) => PaymentProviderCode::EPay,
            Self::MGate(_) => PaymentProviderCode::MGate,
            Self::StripeALL(_) => PaymentProviderCode::StripeALL,
            Self::StripeAlipay(_) => PaymentProviderCode::StripeAlipay,
            Self::StripeCheckout(_) => PaymentProviderCode::StripeCheckout,
            Self::StripeCredit(_) => PaymentProviderCode::StripeCredit,
            Self::StripeWepay(_) => PaymentProviderCode::StripeWepay,
            Self::WechatPayNative(_) => PaymentProviderCode::WechatPayNative,
        }
    }

    pub fn into_string_map(self) -> Result<BTreeMap<String, String>, serde_json::Error> {
        let value = match self {
            Self::AlipayF2F(value) => serde_json::to_value(value),
            Self::BEasyPaymentUSDT(value) => serde_json::to_value(value),
            Self::BTCPay(value) => serde_json::to_value(value),
            Self::CoinPayments(value) => serde_json::to_value(value),
            Self::Coinbase(value) => serde_json::to_value(value),
            Self::EPay(value) => serde_json::to_value(value),
            Self::MGate(value) => serde_json::to_value(value),
            Self::StripeALL(value) => serde_json::to_value(value),
            Self::StripeAlipay(value) => serde_json::to_value(value),
            Self::StripeCheckout(value) => serde_json::to_value(value),
            Self::StripeCredit(value) => serde_json::to_value(value),
            Self::StripeWepay(value) => serde_json::to_value(value),
            Self::WechatPayNative(value) => serde_json::to_value(value),
        }?;
        serde_json::from_value(value)
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminPaymentCreateFields {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub notify_domain: Option<String>,
    #[serde(default)]
    pub handling_fee_fixed: Option<i64>,
    #[serde(default)]
    pub handling_fee_percent: Option<f64>,
}

/// POST `/payments` request. `payment` selects one closed configuration DTO;
/// unknown providers and provider fields are rejected by Serde at extraction.
#[derive(Debug, Clone, ToSchema)]
#[serde(tag = "payment")]
pub enum AdminPaymentCreateRequest {
    AlipayF2F {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: AlipayF2FConfig,
    },
    BEasyPaymentUSDT {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: BEasyPaymentUsdtConfig,
    },
    BTCPay {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: BtcPayConfig,
    },
    CoinPayments {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: CoinPaymentsConfig,
    },
    Coinbase {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: CoinbaseConfig,
    },
    EPay {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: EPayConfig,
    },
    MGate {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: MGateConfig,
    },
    StripeALL {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: StripeAllConfig,
    },
    StripeAlipay {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: StripeAlipayConfig,
    },
    StripeCheckout {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: StripeCheckoutConfig,
    },
    StripeCredit {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: StripeCreditConfig,
    },
    StripeWepay {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: StripeWepayConfig,
    },
    WechatPayNative {
        #[serde(flatten)]
        fields: AdminPaymentCreateFields,
        config: WechatPayNativeConfig,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminPaymentCreateEnvelope {
    name: String,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    notify_domain: Option<String>,
    #[serde(default)]
    handling_fee_fixed: Option<i64>,
    #[serde(default)]
    handling_fee_percent: Option<f64>,
    payment: PaymentProviderCode,
    config: BTreeMap<String, String>,
}

fn decode_payment_config<T, E>(config: BTreeMap<String, String>) -> Result<T, E>
where
    T: for<'de> Deserialize<'de>,
    E: de::Error,
{
    serde_json::from_value(
        serde_json::to_value(config).expect("a string map always serializes as JSON"),
    )
    .map_err(E::custom)
}

impl<'de> Deserialize<'de> for AdminPaymentCreateRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let envelope = AdminPaymentCreateEnvelope::deserialize(deserializer)?;
        let fields = AdminPaymentCreateFields {
            name: envelope.name,
            icon: envelope.icon,
            notify_domain: envelope.notify_domain,
            handling_fee_fixed: envelope.handling_fee_fixed,
            handling_fee_percent: envelope.handling_fee_percent,
        };
        let config = match envelope.payment {
            PaymentProviderCode::AlipayF2F => PaymentProviderConfig::AlipayF2F(
                decode_payment_config::<AlipayF2FConfig, D::Error>(envelope.config)?,
            ),
            PaymentProviderCode::BEasyPaymentUSDT => {
                PaymentProviderConfig::BEasyPaymentUSDT(decode_payment_config::<
                    BEasyPaymentUsdtConfig,
                    D::Error,
                >(envelope.config)?)
            }
            PaymentProviderCode::BTCPay => {
                PaymentProviderConfig::BTCPay(decode_payment_config::<BtcPayConfig, D::Error>(
                    envelope.config,
                )?)
            }
            PaymentProviderCode::CoinPayments => {
                PaymentProviderConfig::CoinPayments(decode_payment_config::<
                    CoinPaymentsConfig,
                    D::Error,
                >(envelope.config)?)
            }
            PaymentProviderCode::Coinbase => PaymentProviderConfig::Coinbase(
                decode_payment_config::<CoinbaseConfig, D::Error>(envelope.config)?,
            ),
            PaymentProviderCode::EPay => {
                PaymentProviderConfig::EPay(decode_payment_config::<EPayConfig, D::Error>(
                    envelope.config,
                )?)
            }
            PaymentProviderCode::MGate => {
                PaymentProviderConfig::MGate(decode_payment_config::<MGateConfig, D::Error>(
                    envelope.config,
                )?)
            }
            PaymentProviderCode::StripeALL => PaymentProviderConfig::StripeALL(
                decode_payment_config::<StripeAllConfig, D::Error>(envelope.config)?,
            ),
            PaymentProviderCode::StripeAlipay => {
                PaymentProviderConfig::StripeAlipay(decode_payment_config::<
                    StripeAlipayConfig,
                    D::Error,
                >(envelope.config)?)
            }
            PaymentProviderCode::StripeCheckout => {
                PaymentProviderConfig::StripeCheckout(decode_payment_config::<
                    StripeCheckoutConfig,
                    D::Error,
                >(envelope.config)?)
            }
            PaymentProviderCode::StripeCredit => {
                PaymentProviderConfig::StripeCredit(decode_payment_config::<
                    StripeCreditConfig,
                    D::Error,
                >(envelope.config)?)
            }
            PaymentProviderCode::StripeWepay => {
                PaymentProviderConfig::StripeWepay(decode_payment_config::<
                    StripeWepayConfig,
                    D::Error,
                >(envelope.config)?)
            }
            PaymentProviderCode::WechatPayNative => {
                PaymentProviderConfig::WechatPayNative(decode_payment_config::<
                    WechatPayNativeConfig,
                    D::Error,
                >(envelope.config)?)
            }
        };
        Ok(match config {
            PaymentProviderConfig::AlipayF2F(config) => Self::AlipayF2F { fields, config },
            PaymentProviderConfig::BEasyPaymentUSDT(config) => {
                Self::BEasyPaymentUSDT { fields, config }
            }
            PaymentProviderConfig::BTCPay(config) => Self::BTCPay { fields, config },
            PaymentProviderConfig::CoinPayments(config) => Self::CoinPayments { fields, config },
            PaymentProviderConfig::Coinbase(config) => Self::Coinbase { fields, config },
            PaymentProviderConfig::EPay(config) => Self::EPay { fields, config },
            PaymentProviderConfig::MGate(config) => Self::MGate { fields, config },
            PaymentProviderConfig::StripeALL(config) => Self::StripeALL { fields, config },
            PaymentProviderConfig::StripeAlipay(config) => Self::StripeAlipay { fields, config },
            PaymentProviderConfig::StripeCheckout(config) => {
                Self::StripeCheckout { fields, config }
            }
            PaymentProviderConfig::StripeCredit(config) => Self::StripeCredit { fields, config },
            PaymentProviderConfig::StripeWepay(config) => Self::StripeWepay { fields, config },
            PaymentProviderConfig::WechatPayNative(config) => {
                Self::WechatPayNative { fields, config }
            }
        })
    }
}

impl AdminPaymentCreateRequest {
    #[must_use]
    pub fn into_parts(self) -> (AdminPaymentCreateFields, PaymentProviderConfig) {
        match self {
            Self::AlipayF2F { fields, config } => {
                (fields, PaymentProviderConfig::AlipayF2F(config))
            }
            Self::BEasyPaymentUSDT { fields, config } => {
                (fields, PaymentProviderConfig::BEasyPaymentUSDT(config))
            }
            Self::BTCPay { fields, config } => (fields, PaymentProviderConfig::BTCPay(config)),
            Self::CoinPayments { fields, config } => {
                (fields, PaymentProviderConfig::CoinPayments(config))
            }
            Self::Coinbase { fields, config } => (fields, PaymentProviderConfig::Coinbase(config)),
            Self::EPay { fields, config } => (fields, PaymentProviderConfig::EPay(config)),
            Self::MGate { fields, config } => (fields, PaymentProviderConfig::MGate(config)),
            Self::StripeALL { fields, config } => {
                (fields, PaymentProviderConfig::StripeALL(config))
            }
            Self::StripeAlipay { fields, config } => {
                (fields, PaymentProviderConfig::StripeAlipay(config))
            }
            Self::StripeCheckout { fields, config } => {
                (fields, PaymentProviderConfig::StripeCheckout(config))
            }
            Self::StripeCredit { fields, config } => {
                (fields, PaymentProviderConfig::StripeCredit(config))
            }
            Self::StripeWepay { fields, config } => {
                (fields, PaymentProviderConfig::StripeWepay(config))
            }
            Self::WechatPayNative { fields, config } => {
                (fields, PaymentProviderConfig::WechatPayNative(config))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminPaymentItem {
    pub id: i32,
    pub name: String,
    pub payment: String,
    #[schema(required)]
    pub icon: Option<String>,
    #[schema(required)]
    pub handling_fee_fixed: Option<i32>,
    #[schema(required)]
    pub handling_fee_percent: Option<f64>,
    pub uuid: String,
    /// Provider manifests own this nested key vocabulary. Values returned by
    /// the server are always strings and secrets are redacted.
    pub config: BTreeMap<String, String>,
    #[schema(required)]
    pub notify_domain: Option<String>,
    pub notify_url: String,
    pub enable: bool,
    #[schema(required)]
    pub sort: Option<i32>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
    pub legacy_md5_signature: bool,
    #[schema(required)]
    pub security_warning: Option<String>,
}

/// PATCH only changes payment-row metadata. Provider identity and verification
/// material are immutable versions and therefore do not belong to this DTO.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminPaymentPatchRequest {
    #[serde(default)]
    #[schema(value_type = String)]
    pub name: NonNull<String>,
    #[serde(default, with = "crate::patch")]
    pub icon: Option<Option<String>>,
    #[serde(default, with = "crate::patch")]
    pub notify_domain: Option<Option<String>>,
    #[serde(default, with = "crate::patch")]
    pub handling_fee_fixed: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub handling_fee_percent: Option<Option<f64>>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub enable: NonNull<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PaymentProviderFormField {
    pub label: String,
    pub description: String,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

pub type PaymentProviderForm = BTreeMap<String, PaymentProviderFormField>;

// --- Orders and reconciliations --------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminOrderFields {
    pub id: i64,
    #[schema(required)]
    pub invite_user_id: Option<i64>,
    pub user_id: i64,
    pub plan_id: i32,
    #[schema(required)]
    pub coupon_id: Option<i32>,
    pub r#type: i32,
    pub period: String,
    pub trade_no: String,
    #[schema(required)]
    pub callback_no: Option<String>,
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
    pub status: i16,
    pub commission_status: i16,
    pub commission_balance: i32,
    #[schema(required)]
    pub actual_commission_balance: Option<i32>,
    #[schema(required)]
    pub payment_id: Option<i32>,
    #[schema(required)]
    pub paid_at: Option<Rfc3339Timestamp>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminOrderListItem {
    #[serde(flatten)]
    pub order: AdminOrderFields,
    pub email: String,
    #[schema(required)]
    pub plan_name: Option<String>,
    pub payment_reconciliation_open_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminCommissionLogItem {
    pub id: i64,
    pub invite_user_id: i64,
    pub user_id: i64,
    pub trade_no: String,
    pub order_amount: i32,
    pub get_amount: i32,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminPaymentReconciliationItem {
    pub id: i64,
    pub payment_id: i32,
    pub provider: String,
    pub trade_no: String,
    pub trade_no_hash: String,
    pub callback_no: String,
    pub callback_no_hash: String,
    pub reason: String,
    pub order_status: i16,
    pub expected_amount: i64,
    #[schema(required)]
    pub settled_amount: Option<i64>,
    pub occurrence_count: i32,
    pub first_seen_at: Rfc3339Timestamp,
    pub last_seen_at: Rfc3339Timestamp,
    #[schema(required)]
    pub resolved_at: Option<Rfc3339Timestamp>,
    #[schema(required)]
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminPaymentReconciliationListItem {
    #[serde(flatten)]
    pub reconciliation: AdminPaymentReconciliationItem,
    pub payment_name: String,
    #[schema(required)]
    pub payment_archived_at: Option<Rfc3339Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminOrderDetail {
    #[serde(flatten)]
    pub order: AdminOrderFields,
    pub commission_log: Vec<AdminCommissionLogItem>,
    pub payment_reconciliations: Vec<AdminPaymentReconciliationItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surplus_orders: Option<Vec<AdminOrderFields>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminOrderStatusPatch {
    pub status: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminOrderCommissionStatusPatch {
    pub commission_status: i64,
}

/// Exactly one order state field is accepted. The closed variant objects make
/// both-fields and neither-field payloads invalid at the transport boundary.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum AdminOrderPatchRequest {
    Status(AdminOrderStatusPatch),
    CommissionStatus(AdminOrderCommissionStatusPatch),
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminOrderCreateRequest {
    pub email: String,
    pub plan_id: i64,
    pub period: String,
    #[serde(default)]
    pub total_amount: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationResolveRequest {
    pub resolution: String,
}

// --- Tickets ----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminTicketItem {
    pub id: i64,
    pub user_id: i64,
    pub subject: String,
    pub level: i16,
    pub status: i16,
    pub reply_status: i16,
    #[schema(required)]
    pub last_reply_user_id: Option<i64>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminTicketMessageItem {
    pub id: i64,
    pub user_id: i64,
    pub ticket_id: i64,
    pub message: String,
    pub is_me: bool,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminTicketDetail {
    #[serde(flatten)]
    pub ticket: AdminTicketItem,
    pub message: Vec<AdminTicketMessageItem>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TicketReplyRequest {
    pub message: String,
}

// --- Users ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserFields {
    pub id: i64,
    pub email: String,
    /// Kept for the established admin editor contract; always the empty string.
    pub password: String,
    pub balance: i32,
    pub commission_balance: i32,
    pub transfer_enable: i64,
    #[schema(required)]
    pub device_limit: Option<i32>,
    pub u: i64,
    pub d: i64,
    #[schema(required)]
    pub plan_id: Option<i32>,
    #[schema(required)]
    pub group_id: Option<i32>,
    #[schema(required)]
    pub expired_at: Option<Rfc3339Timestamp>,
    pub uuid: String,
    pub token: String,
    pub banned: i16,
    pub is_admin: i16,
    pub is_staff: i16,
    pub admin_permissions: Vec<String>,
    #[schema(required)]
    pub invite_user_id: Option<i64>,
    #[schema(required)]
    pub discount: Option<i32>,
    pub commission_type: i16,
    #[schema(required)]
    pub commission_rate: Option<i32>,
    #[schema(required)]
    pub speed_limit: Option<i32>,
    #[schema(required)]
    pub auto_renewal: Option<i16>,
    #[schema(required)]
    pub remind_expire: Option<i16>,
    #[schema(required)]
    pub remind_traffic: Option<i16>,
    #[schema(required)]
    pub remarks: Option<String>,
    #[schema(required)]
    pub telegram_id: Option<i64>,
    #[schema(required)]
    pub last_login_at: Option<Rfc3339Timestamp>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserListItem {
    #[serde(flatten)]
    pub user: AdminUserFields,
    pub total_used: u64,
    pub alive_ip: i64,
    pub ips: String,
    #[schema(required)]
    pub plan_name: Option<String>,
    pub subscribe_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminInviterItem {
    pub id: i64,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserDetail {
    /// Detail preserves every computed field exposed by the admin user list.
    #[serde(flatten)]
    pub user: AdminUserListItem,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite_user: Option<AdminInviterItem>,
}

/// A distinct staff response type prevents future admin-only additions from
/// automatically widening the staff transport contract.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StaffUserDetail {
    /// Staff receives the same computed user projection, without inviter data.
    #[serde(flatten)]
    pub user: AdminUserListItem,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserGenerateRequest {
    #[serde(default)]
    pub email_prefix: Option<String>,
    pub email_suffix: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub plan_id: Option<i64>,
    #[serde(default)]
    pub expired_at: Option<Rfc3339Timestamp>,
    #[serde(default)]
    pub generate_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserPatchRequest {
    #[serde(default)]
    #[schema(value_type = String)]
    pub email: NonNull<String>,
    #[serde(default)]
    #[schema(value_type = String)]
    pub password: NonNull<String>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub transfer_enable: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub u: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub d: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub balance: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub commission_balance: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub commission_type: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub banned: NonNull<bool>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub is_admin: NonNull<bool>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub is_staff: NonNull<bool>,
    #[serde(default)]
    #[schema(value_type = Vec<String>)]
    pub admin_permissions: NonNull<Vec<String>>,
    #[serde(default, with = "crate::patch")]
    pub device_limit: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub commission_rate: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub discount: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub speed_limit: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub plan_id: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub remarks: Option<Option<String>>,
    #[serde(default, with = "crate::patch")]
    pub expired_at: Option<Option<Rfc3339Timestamp>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StaffUserPatchRequest {
    #[serde(default)]
    #[schema(value_type = String)]
    pub email: NonNull<String>,
    #[serde(default)]
    #[schema(value_type = String)]
    pub password: NonNull<String>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub transfer_enable: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub u: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub d: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub balance: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub commission_balance: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub banned: NonNull<bool>,
    #[serde(default, with = "crate::patch")]
    pub device_limit: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub commission_rate: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub discount: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub plan_id: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub expired_at: Option<Option<Rfc3339Timestamp>>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserFilterRequest {
    #[serde(default)]
    pub filter: Option<Vec<AdminFilterClause>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserMailRequest {
    pub subject: String,
    pub content: String,
    #[serde(default)]
    pub filter: Option<Vec<AdminFilterClause>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminSetInviterRequest {
    #[serde(default)]
    pub invite_user_email: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripe_checkout_create() -> serde_json::Value {
        serde_json::json!({
            "name": "Card",
            "payment": "StripeCheckout",
            "config": {
                "currency": "USD",
                "stripe_sk_live": "sk_live_test",
                "stripe_pk_live": "pk_live_test",
                "stripe_webhook_key": "whsec_test",
                "stripe_custom_field_name": "contact"
            }
        })
    }

    #[test]
    fn payment_create_selects_the_exact_provider_configuration() {
        let request = serde_json::from_value::<AdminPaymentCreateRequest>(stripe_checkout_create())
            .expect("valid Stripe Checkout request");
        let (fields, config) = request.into_parts();
        assert_eq!(fields.name, "Card");
        assert_eq!(config.code(), PaymentProviderCode::StripeCheckout);
        assert_eq!(
            config
                .into_string_map()
                .expect("closed configuration serializes to a string map")
                .get("currency")
                .map(String::as_str),
            Some("USD")
        );
    }

    #[test]
    fn payment_create_rejects_unknown_provider_and_configuration_fields() {
        let mut unknown_provider = stripe_checkout_create();
        unknown_provider["payment"] = serde_json::json!("FutureGateway");
        assert!(serde_json::from_value::<AdminPaymentCreateRequest>(unknown_provider).is_err());

        let mut unknown_config = stripe_checkout_create();
        unknown_config["config"]["secret_key"] = serde_json::json!("not-a-manifest-field");
        assert!(serde_json::from_value::<AdminPaymentCreateRequest>(unknown_config).is_err());

        let mut unknown_root = stripe_checkout_create();
        unknown_root["provider_options"] = serde_json::json!({});
        assert!(serde_json::from_value::<AdminPaymentCreateRequest>(unknown_root).is_err());
    }

    #[test]
    fn payment_patch_cannot_rotate_provider_or_credentials() {
        assert!(
            serde_json::from_value::<AdminPaymentPatchRequest>(serde_json::json!({
                "name": "Renamed",
                "payment": "StripeCheckout"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<AdminPaymentPatchRequest>(serde_json::json!({
                "config": {}
            }))
            .is_err()
        );
    }
}
