use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::patch::NonNull;
use crate::time::Rfc3339Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatedId {
    pub id: i32,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SortIdsRequest {
    pub ids: Vec<i64>,
}

/// Admin plan response. Prices are always integer minor currency units.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminPlanItem {
    pub id: i32,
    pub group_id: i32,
    #[schema(minimum = 0, maximum = 2_147_483_647)]
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
    #[schema(minimum = 0)]
    pub count: i64,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanCreate {
    pub name: String,
    pub group_id: i64,
    #[schema(minimum = 0, maximum = 2_147_483_647)]
    pub transfer_enable: i64,
    #[serde(default)]
    #[schema(minimum = 0, maximum = 2_147_483_647)]
    pub device_limit: Option<i64>,
    #[serde(default)]
    #[schema(minimum = 0, maximum = 2_147_483_647)]
    pub speed_limit: Option<i64>,
    #[serde(default)]
    #[schema(minimum = 0, maximum = 2_147_483_647)]
    pub capacity_limit: Option<i64>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub month_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub quarter_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub half_year_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub year_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub two_year_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub three_year_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub onetime_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub reset_price: Option<i64>,
    #[serde(default)]
    #[schema(minimum = 0, maximum = 4)]
    pub reset_traffic_method: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanPatch {
    #[serde(default)]
    #[schema(value_type = String)]
    pub name: NonNull<String>,
    #[serde(default)]
    #[schema(value_type = i64)]
    pub group_id: NonNull<i64>,
    #[serde(default)]
    #[schema(value_type = i64, minimum = 0, maximum = 2_147_483_647)]
    pub transfer_enable: NonNull<i64>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = 0, maximum = 2_147_483_647)]
    pub device_limit: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = 0, maximum = 2_147_483_647)]
    pub speed_limit: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = 0, maximum = 2_147_483_647)]
    pub capacity_limit: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    pub content: Option<Option<String>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub month_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub quarter_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub half_year_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub year_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub two_year_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub three_year_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub onetime_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = -2_147_483_648, maximum = 2_147_483_647)]
    pub reset_price: Option<Option<i64>>,
    #[serde(default, with = "crate::patch")]
    #[schema(minimum = 0, maximum = 4)]
    pub reset_traffic_method: Option<Option<i64>>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub show: NonNull<bool>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub renew: NonNull<bool>,
    #[serde(default)]
    #[schema(value_type = bool)]
    pub force_update: NonNull<bool>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn patch_distinguishes_absent_clear_and_value() {
        let absent = serde_json::from_value::<PlanPatch>(json!({})).expect("absent patch");
        assert_eq!(absent.month_price, None);
        assert_eq!(absent.name, crate::patch::NonNull::Retain);

        let clear = serde_json::from_value::<PlanPatch>(json!({ "month_price": null }))
            .expect("clear patch");
        assert_eq!(clear.month_price, Some(None));

        let value = serde_json::from_value::<PlanPatch>(json!({ "month_price": 1200 }))
            .expect("value patch");
        assert_eq!(value.month_price, Some(Some(1200)));
    }

    #[test]
    fn patch_rejects_null_for_every_non_clearable_field() {
        for field in [
            "name",
            "group_id",
            "transfer_enable",
            "show",
            "renew",
            "force_update",
        ] {
            let mut body = serde_json::Map::new();
            body.insert(field.to_owned(), serde_json::Value::Null);
            assert!(
                serde_json::from_value::<PlanPatch>(serde_json::Value::Object(body)).is_err(),
                "{field}: null must not alias to Retain"
            );
        }
    }

    #[test]
    fn timestamps_are_rfc3339_on_the_wire() {
        let timestamp = Rfc3339Timestamp::from_epoch_seconds(1_700_000_000);
        assert_eq!(
            serde_json::to_value(timestamp).expect("timestamp"),
            "2023-11-14T22:13:20Z"
        );
    }
}
