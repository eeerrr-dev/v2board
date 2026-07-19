use super::*;

use super::orders::{OrderPatchAction, order_patch_action};
use super::payments::handling_fee_percent_decimal;
use super::plans::plan_patch_validation;

/// §6.4: PATCH `orders/{trade_no}` demands **exactly one** of `status`
/// and `commission_status` — both or neither is a 422 validation
/// problem, and each arm keeps its legacy `in:` vocabulary.
#[test]
fn order_patch_enforces_the_exactly_one_field_rule() {
    let neither: OrderPatch = serde_json::from_value(json!({})).unwrap();
    assert!(matches!(
        order_patch_action(&neither),
        Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
    ));
    let both: OrderPatch =
        serde_json::from_value(json!({ "status": 1, "commission_status": 1 })).unwrap();
    assert!(matches!(
        order_patch_action(&both),
        Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
    ));

    let status: OrderPatch = serde_json::from_value(json!({ "status": 2 })).unwrap();
    assert_eq!(
        order_patch_action(&status).unwrap(),
        OrderPatchAction::Status(2)
    );
    let commission: OrderPatch = serde_json::from_value(json!({ "commission_status": 3 })).unwrap();
    assert_eq!(
        order_patch_action(&commission).unwrap(),
        OrderPatchAction::CommissionStatus(3)
    );

    // Legacy Laravel vocabularies: status in 0-3, commission_status in 0/1/3.
    let bad_status: OrderPatch = serde_json::from_value(json!({ "status": 4 })).unwrap();
    assert!(order_patch_action(&bad_status).is_err());
    let bad_commission: OrderPatch =
        serde_json::from_value(json!({ "commission_status": 2 })).unwrap();
    assert!(order_patch_action(&bad_commission).is_err());

    // deny_unknown_fields: the legacy reconciliation_id demultiplex is
    // gone — it must not parse as an order patch.
    assert!(serde_json::from_value::<OrderPatch>(json!({ "reconciliation_id": 7 })).is_err());
}

/// §4.4: the payment PATCH distinguishes absent (retain), null (clear),
/// and value (set) for the nullable metadata columns.
#[test]
fn payment_patch_distinguishes_absent_null_and_value() {
    let absent: PaymentPatch = serde_json::from_value(json!({})).unwrap();
    assert!(absent.icon.is_none());
    assert!(absent.notify_domain.is_none());
    assert!(absent.handling_fee_fixed.is_none());
    assert!(absent.enable.is_none());

    let cleared: PaymentPatch = serde_json::from_value(json!({
        "icon": null,
        "notify_domain": null,
        "handling_fee_fixed": null,
        "handling_fee_percent": null
    }))
    .unwrap();
    assert_eq!(cleared.icon, Some(None));
    assert_eq!(cleared.notify_domain, Some(None));
    assert_eq!(cleared.handling_fee_fixed, Some(None));
    assert!(matches!(cleared.handling_fee_percent, Some(None)));

    let set: PaymentPatch = serde_json::from_value(json!({
        "name": "Renamed",
        "handling_fee_fixed": 20,
        "handling_fee_percent": 0.5,
        "enable": true
    }))
    .unwrap();
    assert_eq!(set.handling_fee_fixed, Some(Some(20)));
    assert_eq!(set.enable, Some(true));
    assert_eq!(
        handling_fee_percent_decimal(set.handling_fee_percent.unwrap().as_ref()).unwrap(),
        Some(Decimal::new(5, 1))
    );
}

/// §6.2: the legacy 0.1–100 handling-fee window survives on the JSON
/// number representation.
#[test]
fn handling_fee_percent_window_is_exact() {
    for valid in [json!(0.1), json!(100), json!(2.75)] {
        let number = valid.as_number().unwrap().clone();
        assert!(
            handling_fee_percent_decimal(Some(&number)).is_ok(),
            "{number}"
        );
    }
    for invalid in [json!(0), json!(0.09), json!(100.01), json!(-3)] {
        let number = invalid.as_number().unwrap().clone();
        assert!(
            handling_fee_percent_decimal(Some(&number)).is_err(),
            "{number}"
        );
    }
    assert_eq!(handling_fee_percent_decimal(None).unwrap(), None);
}

/// §4.4 + amount windows on the plan PATCH: double-Option clears, and
/// every amount keeps the non-negative 32-bit window.
#[test]
fn plan_patch_distinguishes_absent_null_and_value_and_validates_amounts() {
    let patch: PlanPatch = serde_json::from_value(json!({
        "month_price": null,
        "capacity_limit": 50,
        "show": true,
        "force_update": true
    }))
    .unwrap();
    assert_eq!(patch.month_price, Some(None));
    assert_eq!(patch.capacity_limit, Some(Some(50)));
    assert!(patch.quarter_price.is_none());
    assert_eq!(patch.show, Some(true));
    assert_eq!(patch.force_update, Some(true));
    assert!(plan_patch_validation(&patch).is_ok());

    for invalid in [
        json!({ "month_price": -1 }),
        json!({ "transfer_enable": 2_147_483_648_i64 }),
    ] {
        let patch: PlanPatch = serde_json::from_value(invalid).unwrap();
        assert!(matches!(
            plan_patch_validation(&patch),
            Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
        ));
    }
    let bad_reset: PlanPatch =
        serde_json::from_value(json!({ "reset_traffic_method": 40_000 })).unwrap();
    assert!(plan_patch_validation(&bad_reset).is_err());
}

/// §6.2/§4.5: admin plan and payment items serialize bool flags and
/// RFC 3339 timestamps (prices cents, `handling_fee_percent` a number).
#[test]
fn commerce_items_serialize_modern_value_types() {
    let plan = AdminPlanItem {
        id: 1,
        group_id: 1,
        transfer_enable: 100,
        device_limit: None,
        name: "Golden Plan".to_string(),
        speed_limit: None,
        show: true,
        sort: Some(1),
        renew: false,
        content: None,
        month_price: Some(1000),
        quarter_price: None,
        half_year_price: None,
        year_price: None,
        two_year_price: None,
        three_year_price: None,
        onetime_price: None,
        reset_price: None,
        reset_traffic_method: Some(0),
        capacity_limit: None,
        count: 3,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
    };
    let encoded = serde_json::to_value(&plan).unwrap();
    assert_eq!(encoded["show"], json!(true));
    assert_eq!(encoded["renew"], json!(false));
    assert_eq!(encoded["count"], json!(3));
    assert_eq!(encoded["month_price"], json!(1000));
    assert_eq!(encoded["created_at"], json!("2023-11-14T22:13:20Z"));

    let payment = AdminPaymentItem {
        id: 7,
        name: "Golden EPay".to_string(),
        payment: "EPay".to_string(),
        icon: None,
        handling_fee_fixed: Some(20),
        handling_fee_percent: Some(0.5),
        uuid: "goldenepayuuid000000000000000001".to_string(),
        config: json!({ "pid": "1000" }),
        notify_domain: None,
        notify_url: "https://golden.v2board.test/api/v1/guest/payment/notify/EPay/goldenepayuuid000000000000000001".to_string(),
        enable: true,
        sort: Some(1),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        legacy_md5_signature: true,
        security_warning: Some("warning"),
    };
    let encoded = serde_json::to_value(&payment).unwrap();
    assert_eq!(encoded["enable"], json!(true));
    assert_eq!(encoded["handling_fee_percent"], json!(0.5));
    assert_eq!(encoded["updated_at"], json!("2023-11-14T22:13:20Z"));
    assert_eq!(encoded["legacy_md5_signature"], json!(true));
}

/// §6.4: the assign body is the named JSON object with an optional
/// `total_amount`; unknown fields are rejected.
#[test]
fn order_assign_body_is_strict_json() {
    let assign: OrderAssign = serde_json::from_value(json!({
        "email": "member@example.test",
        "plan_id": 1,
        "period": "month_price"
    }))
    .unwrap();
    assert_eq!(assign.total_amount, None);
    assert!(
        serde_json::from_value::<OrderAssign>(json!({
            "email": "member@example.test",
            "plan_id": 1,
            "period": "month_price",
            "id": 9
        }))
        .is_err()
    );
}
