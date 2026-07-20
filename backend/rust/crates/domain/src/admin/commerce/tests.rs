use super::*;
use v2board_domain_model::{
    MoneyMinor, PlanPricePeriod, PlanPriceUpdate, PlanPriceUpdates, PlanPrices,
};

use super::orders::{OrderPatchAction, order_patch_action};
use super::payments::handling_fee_percent_decimal;
use super::plans::{
    plan_create_validation, plan_in_use_problem, plan_patch_validation,
    plan_reference_for_constraint, plan_sort_ids,
};

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

/// §4.4 + amount windows on the plan PATCH: the typed update collection
/// distinguishes retain, clear, and set before the use case runs.
#[test]
fn plan_patch_distinguishes_absent_null_and_value_and_validates_amounts() {
    let mut prices = PlanPriceUpdates::default();
    prices.set(PlanPricePeriod::Month, PlanPriceUpdate::Clear);
    let patch = PlanPatchCommand {
        prices,
        capacity_limit: Some(Some(50)),
        show: Some(true),
        force_update: Some(true),
        ..PlanPatchCommand::default()
    };
    assert_eq!(
        patch.prices.get(PlanPricePeriod::Month),
        PlanPriceUpdate::Clear
    );
    assert_eq!(patch.capacity_limit, Some(Some(50)));
    assert_eq!(
        patch.prices.get(PlanPricePeriod::Quarter),
        PlanPriceUpdate::Retain
    );
    assert_eq!(patch.show, Some(true));
    assert_eq!(patch.force_update, Some(true));
    assert!(plan_patch_validation(&patch).is_ok());

    assert_eq!(MoneyMinor::try_from(-1).unwrap().get(), -1);
    let wide_transfer = PlanPatchCommand {
        transfer_enable: Some(2_147_483_648_i64),
        ..PlanPatchCommand::default()
    };
    assert!(matches!(
        plan_patch_validation(&wide_transfer),
        Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
    ));
    let negative_transfer = PlanPatchCommand {
        transfer_enable: Some(-1),
        ..PlanPatchCommand::default()
    };
    assert!(matches!(
        plan_patch_validation(&negative_transfer),
        Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
    ));
    let bad_reset = PlanPatchCommand {
        reset_traffic_method: Some(Some(40_000)),
        ..PlanPatchCommand::default()
    };
    assert!(plan_patch_validation(&bad_reset).is_err());
}

#[test]
fn plan_create_and_patch_reject_reset_methods_outside_the_wire_enum() {
    for invalid in [-1, 5] {
        let create = PlanCreateCommand {
            name: "reset enum".to_string(),
            group_id: 1,
            transfer_enable: 1,
            device_limit: None,
            speed_limit: None,
            capacity_limit: None,
            content: None,
            prices: PlanPrices::default(),
            reset_traffic_method: Some(invalid),
        };
        assert!(matches!(
            plan_create_validation(&create),
            Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
        ));

        let patch = PlanPatchCommand {
            reset_traffic_method: Some(Some(invalid)),
            ..PlanPatchCommand::default()
        };
        assert!(matches!(
            plan_patch_validation(&patch),
            Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
        ));
    }

    for valid in [0, 4] {
        let patch = PlanPatchCommand {
            reset_traffic_method: Some(Some(valid)),
            ..PlanPatchCommand::default()
        };
        assert!(plan_patch_validation(&patch).is_ok());
    }
}

#[test]
fn plan_sort_requires_unique_positive_i32_ids_before_database_access() {
    assert_eq!(plan_sort_ids(&[]).unwrap(), Vec::<i32>::new());
    assert_eq!(plan_sort_ids(&[3, 1, 2]).unwrap(), vec![3, 1, 2]);
    for invalid in [&[1, 1][..], &[0][..], &[-1][..], &[2_147_483_648][..]] {
        assert!(matches!(
            plan_sort_ids(invalid),
            Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
        ));
    }
}

#[test]
fn plan_delete_foreign_keys_map_to_stable_plan_in_use_dependencies() {
    use v2board_db::plan::PlanReferenceKind;

    for (constraint, expected, detail) in [
        (
            "orders_referenced_plan_id_fkey",
            PlanReferenceKind::Order,
            "该订阅下存在订单无法删除",
        ),
        (
            "users_plan_id_fkey",
            PlanReferenceKind::User,
            "该订阅下存在用户无法删除",
        ),
        (
            "gift_card_plan_id_fkey",
            PlanReferenceKind::GiftCard,
            "该订阅仍被礼品卡使用，无法删除",
        ),
    ] {
        let dependency = plan_reference_for_constraint(Some(constraint));
        assert_eq!(dependency, Some(expected));
        let problem = plan_in_use_problem(expected);
        assert_eq!(problem.code(), Code::PlanInUse);
        assert_eq!(problem.detail(), detail);
    }
    assert_eq!(plan_reference_for_constraint(None), None);
    assert_eq!(
        plan_reference_for_constraint(Some("future_plan_reference_fkey")),
        None
    );
}

/// The application plan projection keeps booleans, minor units, and epoch
/// seconds; the HTTP adapter owns RFC 3339 serialization. Payment remains on
/// its older application DTO until that family gets the same boundary split.
#[test]
fn commerce_items_serialize_modern_value_types() {
    let mut prices = PlanPrices::default();
    prices.set(
        PlanPricePeriod::Month,
        Some(MoneyMinor::try_from(1_000).unwrap()),
    );
    let plan = AdminPlanView {
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
        prices,
        reset_traffic_method: Some(0),
        capacity_limit: None,
        count: 3,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
    };
    assert!(plan.show);
    assert!(!plan.renew);
    assert_eq!(plan.count, 3);
    assert_eq!(
        plan.prices.get(PlanPricePeriod::Month).map(MoneyMinor::get),
        Some(1000)
    );
    assert_eq!(plan.created_at, 1_700_000_000);

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
