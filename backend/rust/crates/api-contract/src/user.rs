//! Field-level wire contracts for user account, subscription, and closely
//! related authenticated actions.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::time::Rfc3339Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserProfile {
    pub email: String,
    #[schema(minimum = 0)]
    pub transfer_enable: i64,
    #[schema(required, minimum = 0)]
    pub device_limit: Option<i32>,
    #[schema(required)]
    pub last_login_at: Option<Rfc3339Timestamp>,
    pub created_at: Rfc3339Timestamp,
    pub banned: bool,
    pub auto_renewal: bool,
    pub remind_expire: bool,
    pub remind_traffic: bool,
    #[schema(required)]
    pub expired_at: Option<Rfc3339Timestamp>,
    pub balance: i32,
    pub commission_balance: i32,
    #[schema(required)]
    pub plan_id: Option<i32>,
    #[schema(required)]
    pub discount: Option<i32>,
    #[schema(required)]
    pub commission_rate: Option<i32>,
    #[schema(required)]
    pub telegram_id: Option<i64>,
    pub uuid: String,
    pub avatar_url: String,
}

/// Preference update semantics: absent retains, explicit `null` clears, and a
/// boolean value sets the preference.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserProfilePatch {
    #[serde(default, with = "crate::patch")]
    pub auto_renewal: Option<Option<bool>>,
    #[serde(default, with = "crate::patch")]
    pub remind_expire: Option<Option<bool>>,
    #[serde(default, with = "crate::patch")]
    pub remind_traffic: Option<Option<bool>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PasswordUpdateRequest {
    #[schema(min_length = 8)]
    pub old_password: String,
    #[schema(min_length = 8)]
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserStats {
    #[schema(minimum = 0)]
    pub pending_order_count: i64,
    #[schema(minimum = 0)]
    pub pending_ticket_count: i64,
    #[schema(minimum = 0)]
    pub invited_user_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserSession {
    pub session_id: String,
    pub ip: String,
    pub ua: String,
    pub login_at: Rfc3339Timestamp,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    pub is_telegram: bool,
    #[schema(required)]
    pub telegram_discuss_link: Option<String>,
    pub withdraw_methods: Vec<String>,
    pub withdraw_close: bool,
    pub currency: String,
    pub currency_symbol: String,
    pub commission_distribution_enable: bool,
    #[schema(required)]
    pub commission_distribution_l1: Option<f64>,
    #[schema(required)]
    pub commission_distribution_l2: Option<f64>,
    #[schema(required)]
    pub commission_distribution_l3: Option<f64>,
}

/// The plan representation embedded by `/user/subscription` and returned by
/// the user commerce plan endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserPlan {
    pub id: i32,
    pub group_id: i32,
    #[schema(minimum = 0)]
    pub transfer_enable: i64,
    #[schema(required, minimum = 0)]
    pub device_limit: Option<i32>,
    pub name: String,
    #[schema(required, minimum = 0)]
    pub speed_limit: Option<i32>,
    pub show: bool,
    #[schema(required)]
    pub sort: Option<i32>,
    pub renew: bool,
    #[schema(required)]
    pub content: Option<String>,
    #[schema(required)]
    pub month_price: Option<i32>,
    #[schema(required)]
    pub quarter_price: Option<i32>,
    #[schema(required)]
    pub half_year_price: Option<i32>,
    #[schema(required)]
    pub year_price: Option<i32>,
    #[schema(required)]
    pub two_year_price: Option<i32>,
    #[schema(required)]
    pub three_year_price: Option<i32>,
    #[schema(required)]
    pub onetime_price: Option<i32>,
    #[schema(required)]
    pub reset_price: Option<i32>,
    #[schema(required, minimum = 0, maximum = 4)]
    pub reset_traffic_method: Option<i16>,
    #[schema(required, minimum = 0)]
    pub capacity_limit: Option<i32>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    #[schema(required)]
    pub plan_id: Option<i32>,
    pub token: String,
    #[schema(required)]
    pub expired_at: Option<Rfc3339Timestamp>,
    #[schema(minimum = 0)]
    pub u: i64,
    #[schema(minimum = 0)]
    pub d: i64,
    #[schema(minimum = 0)]
    pub transfer_enable: i64,
    #[schema(required, minimum = 0)]
    pub device_limit: Option<i32>,
    pub email: String,
    pub uuid: String,
    #[schema(required)]
    pub plan: Option<UserPlan>,
    #[schema(minimum = 0)]
    pub alive_ip: i64,
    pub subscribe_url: String,
    #[schema(required, minimum = 0)]
    pub reset_day: Option<i64>,
    pub allow_new_period: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetSubscriptionToken {
    pub subscribe_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TelegramBot {
    pub username: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CommissionTransferRequest {
    #[schema(minimum = 1)]
    pub transfer_amount: i32,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GiftCardRedemptionRequest {
    #[schema(min_length = 1)]
    pub giftcard: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GiftCardRedemption {
    pub r#type: i16,
    #[schema(required)]
    pub value: Option<i32>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn profile_patch_distinguishes_retain_clear_and_set() {
        let retain = serde_json::from_value::<UserProfilePatch>(json!({})).expect("retain");
        assert_eq!(retain.auto_renewal, None);

        let clear = serde_json::from_value::<UserProfilePatch>(json!({
            "auto_renewal": null
        }))
        .expect("clear");
        assert_eq!(clear.auto_renewal, Some(None));

        let set = serde_json::from_value::<UserProfilePatch>(json!({
            "auto_renewal": true
        }))
        .expect("set");
        assert_eq!(set.auto_renewal, Some(Some(true)));
    }

    #[test]
    fn profile_patch_rejects_unknown_preferences() {
        assert!(
            serde_json::from_value::<UserProfilePatch>(json!({
                "remind_expiry": true
            }))
            .is_err()
        );
    }
}
