//! Coupon and gift-card transport contracts for the admin namespace.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{patch::NonNull, time::Rfc3339Timestamp};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminCouponItem {
    pub id: i32,
    pub code: String,
    pub name: String,
    #[serde(rename = "type")]
    #[schema(minimum = 1, maximum = 2)]
    pub coupon_type: i16,
    pub value: i32,
    pub show: bool,
    #[schema(required)]
    pub limit_use: Option<i32>,
    #[schema(required)]
    pub limit_use_with_user: Option<i32>,
    #[schema(required)]
    pub limit_plan_ids: Option<Vec<i64>>,
    #[schema(required)]
    pub limit_period: Option<Vec<String>>,
    pub started_at: Rfc3339Timestamp,
    pub ended_at: Rfc3339Timestamp,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

/// POST `/coupons`. A positive `generate_count` selects the CSV success arm;
/// absent/zero selects the 201 JSON `{id}` arm.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CouponGenerateRequest {
    pub name: String,
    #[serde(rename = "type")]
    #[schema(minimum = 1, maximum = 2)]
    pub coupon_type: i64,
    pub value: i64,
    pub started_at: Rfc3339Timestamp,
    pub ended_at: Rfc3339Timestamp,
    #[serde(default)]
    pub limit_use: Option<i64>,
    #[serde(default)]
    pub limit_use_with_user: Option<i64>,
    #[serde(default)]
    pub limit_plan_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub limit_period: Option<Vec<String>>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub generate_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CouponPatchRequest {
    #[serde(default)]
    #[schema(value_type = String)]
    pub name: NonNull<String>,
    #[serde(default, rename = "type")]
    #[schema(value_type = i64, minimum = 1, maximum = 2)]
    pub coupon_type: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub value: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = String, format = DateTime)]
    pub started_at: NonNull<Rfc3339Timestamp>,
    #[serde(default)]
    #[schema(value_type = String, format = DateTime)]
    pub ended_at: NonNull<Rfc3339Timestamp>,
    #[serde(default, with = "crate::patch")]
    pub limit_use: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub limit_use_with_user: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub limit_plan_ids: Option<Option<Vec<i64>>>,
    #[serde(default, with = "crate::patch")]
    pub limit_period: Option<Option<Vec<String>>>,
    #[serde(default)]
    #[schema(value_type = String)]
    pub code: NonNull<String>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub show: NonNull<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminGiftcardItem {
    pub id: i32,
    pub code: String,
    pub name: String,
    #[serde(rename = "type")]
    #[schema(minimum = 1, maximum = 5)]
    pub card_type: i16,
    #[schema(required)]
    pub value: Option<i32>,
    #[schema(required)]
    pub plan_id: Option<i32>,
    #[schema(required)]
    pub limit_use: Option<i32>,
    pub used_user_ids: Vec<i64>,
    pub started_at: Rfc3339Timestamp,
    pub ended_at: Rfc3339Timestamp,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

/// POST `/gift-cards`; `generate_count` chooses between the CSV download and
/// created-id success representations exactly as on coupon generation.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GiftcardGenerateRequest {
    pub name: String,
    #[serde(rename = "type")]
    #[schema(minimum = 1, maximum = 5)]
    pub card_type: i64,
    #[serde(default)]
    pub value: Option<i64>,
    #[serde(default)]
    pub plan_id: Option<i64>,
    pub started_at: Rfc3339Timestamp,
    pub ended_at: Rfc3339Timestamp,
    #[serde(default)]
    pub limit_use: Option<i64>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub generate_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GiftcardPatchRequest {
    #[serde(default)]
    #[schema(value_type = String)]
    pub name: NonNull<String>,
    #[serde(default, rename = "type")]
    #[schema(value_type = i64, minimum = 1, maximum = 5)]
    pub card_type: NonNull<i64>,
    #[serde(default, with = "crate::patch")]
    pub value: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub plan_id: Option<Option<i64>>,
    #[serde(default)]
    #[schema(value_type = String, format = DateTime)]
    pub started_at: NonNull<Rfc3339Timestamp>,
    #[serde(default)]
    #[schema(value_type = String, format = DateTime)]
    pub ended_at: NonNull<Rfc3339Timestamp>,
    #[serde(default, with = "crate::patch")]
    pub limit_use: Option<Option<i64>>,
    #[serde(default)]
    #[schema(value_type = String)]
    pub code: NonNull<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coupon_patch_rejects_null_for_set_only_fields_and_preserves_clears() {
        assert!(
            serde_json::from_value::<CouponPatchRequest>(serde_json::json!({"name": null}))
                .is_err()
        );
        let patch = serde_json::from_value::<CouponPatchRequest>(serde_json::json!({
            "limit_plan_ids": null
        }))
        .expect("nullable coupon scope");
        assert_eq!(patch.limit_plan_ids, Some(None));
    }

    #[test]
    fn giftcard_patch_distinguishes_retain_clear_and_set() {
        let retain = serde_json::from_value::<GiftcardPatchRequest>(serde_json::json!({}))
            .expect("retain patch");
        assert_eq!(retain.plan_id, None);
        let clear = serde_json::from_value::<GiftcardPatchRequest>(serde_json::json!({
            "plan_id": null
        }))
        .expect("clear patch");
        assert_eq!(clear.plan_id, Some(None));
        let set = serde_json::from_value::<GiftcardPatchRequest>(serde_json::json!({
            "plan_id": 7
        }))
        .expect("set patch");
        assert_eq!(set.plan_id, Some(Some(7)));
    }
}
