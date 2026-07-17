//! Byte-exact golden fixtures for the pure-serde response bodies this crate
//! serializes onto the wire (`guest.*`, `passport.*`, `user.*` under
//! `frontend/packages/api-client/goldens`). Each document is a fully rendered
//! response body: the exact compat envelope around a hand-pinned, realistic
//! struct value. The api-client vitest suite parses every fixture with its
//! zod contract schema, so a serde field rename, type change, or envelope
//! change that would break the TypeScript contract fails here and in the
//! frontend gate instead of drifting silently. The DB-backed `admin.*`
//! fixtures are owned by `v2board-contract golden-responses`.
//!
//! Regenerate with `make contract-goldens` (UPDATE_GOLDENS=1); the default
//! test run verifies the checked-in fixtures byte-for-byte.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::json;
use v2board_compat::LegacyEnvelope;
use v2board_db::{
    order::{DepositPlan, OrderPlan, OrderRow},
    payment::PaymentMethodRow,
    plan::PlanRow,
    stat::TrafficLogRow,
    user::UserInfoRow,
};
use v2board_domain::auth::AuthData;

use crate::{
    client::GuestConfig,
    commerce::CheckoutEnvelope,
    user::{
        account::{CheckLoginResult, UserCommConfig},
        subscription::SubscribeInfo,
    },
};

/// 2023-11-14T22:13:20Z, shared with the contract crate's golden generator.
const GOLDEN_TIME: i64 = 1_700_000_000;
const GOLDEN_EXPIRED_AT: i64 = GOLDEN_TIME + 86_400 * 30;
const GOLDEN_EMAIL: &str = "golden-member@example.test";
const GOLDEN_UUID: &str = "00000000-0000-4000-8000-000000000002";
const GOLDEN_TOKEN: &str = "goldenmembertoken000000000000002";
/// The file-name prefixes this test owns inside the shared goldens directory.
const WIRE_GOLDEN_PREFIXES: &[&str] = &["guest.", "passport.", "user."];

fn goldens_dir() -> PathBuf {
    match std::env::var("V2BOARD_GOLDENS_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../frontend/packages/api-client/goldens"
        )),
    }
}

fn envelope<T: Serialize>(data: T) -> String {
    pretty(&LegacyEnvelope { data })
}

fn pretty<T: Serialize>(value: &T) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(value).expect("golden fixtures serialize infallibly")
    )
}

fn golden_plan() -> PlanRow {
    PlanRow {
        id: 1,
        group_id: 1,
        transfer_enable: 107_374_182_400,
        device_limit: Some(3),
        name: "Golden Plan".to_string(),
        speed_limit: None,
        show: 1,
        sort: Some(1),
        renew: 1,
        content: Some("golden plan content".to_string()),
        month_price: Some(1000),
        quarter_price: Some(2700),
        half_year_price: None,
        year_price: Some(9600),
        two_year_price: None,
        three_year_price: None,
        onetime_price: Some(15_000),
        reset_price: Some(300),
        reset_traffic_method: Some(0),
        capacity_limit: Some(50),
        created_at: GOLDEN_TIME,
        updated_at: GOLDEN_TIME,
    }
}

fn golden_plan_order() -> OrderRow {
    OrderRow {
        trade_no: "golden-trade-plan-00000000000001".to_string(),
        callback_no: None,
        plan_id: 1,
        coupon_id: None,
        payment_id: Some(1),
        r#type: 1,
        period: "month_price".to_string(),
        total_amount: 1000,
        handling_amount: Some(20),
        discount_amount: None,
        surplus_amount: None,
        refund_amount: None,
        balance_amount: None,
        surplus_order_ids: None,
        status: 0,
        commission_status: 0,
        commission_balance: 100,
        actual_commission_balance: None,
        invite_user_id: Some(1),
        paid_at: None,
        created_at: GOLDEN_TIME,
        updated_at: GOLDEN_TIME,
        plan: Some(OrderPlan::Full(Box::new(golden_plan()))),
        try_out_plan_id: None,
        surplus_orders: None,
        bounus: None,
        get_amount: None,
    }
}

fn golden_deposit_order() -> OrderRow {
    OrderRow {
        trade_no: "golden-trade-deposit-00000000002".to_string(),
        callback_no: None,
        plan_id: 0,
        coupon_id: None,
        payment_id: None,
        r#type: 1,
        period: "deposit".to_string(),
        total_amount: 500,
        handling_amount: None,
        discount_amount: None,
        surplus_amount: None,
        refund_amount: None,
        balance_amount: None,
        surplus_order_ids: None,
        status: 0,
        commission_status: 0,
        commission_balance: 0,
        actual_commission_balance: None,
        invite_user_id: None,
        paid_at: None,
        created_at: GOLDEN_TIME + 10,
        updated_at: GOLDEN_TIME + 10,
        plan: None,
        try_out_plan_id: None,
        surplus_orders: None,
        bounus: None,
        get_amount: None,
    }
}

/// Every wire fixture this test owns, as `(file name, exact body)`.
fn documents() -> Vec<(&'static str, String)> {
    let auth_login = envelope(AuthData {
        is_admin: 0,
        auth_data: "golden-opaque-session-token-0001".to_string(),
    });

    let check_login = envelope(CheckLoginResult {
        is_login: true,
        is_admin: None,
    });
    let check_login_admin = envelope(CheckLoginResult {
        is_login: true,
        is_admin: Some(true),
    });

    let user_info = envelope(UserInfoRow {
        email: GOLDEN_EMAIL.to_string(),
        transfer_enable: 107_374_182_400,
        device_limit: Some(3),
        last_login_at: None,
        created_at: GOLDEN_TIME,
        banned: 0,
        auto_renewal: Some(0),
        remind_expire: Some(1),
        remind_traffic: Some(1),
        expired_at: Some(GOLDEN_EXPIRED_AT),
        balance: 1000,
        commission_balance: 500,
        plan_id: Some(1),
        discount: None,
        commission_rate: None,
        telegram_id: None,
        uuid: GOLDEN_UUID.to_string(),
        avatar_url: format!(
            "https://cravatar.cn/avatar/{:x}?s=64&d=identicon",
            md5::compute(GOLDEN_EMAIL.as_bytes())
        ),
    });

    let subscribe = envelope(SubscribeInfo {
        plan_id: Some(1),
        token: GOLDEN_TOKEN.to_string(),
        expired_at: Some(GOLDEN_EXPIRED_AT),
        u: 1_073_741_824,
        d: 2_147_483_648,
        transfer_enable: 107_374_182_400,
        device_limit: Some(3),
        email: GOLDEN_EMAIL.to_string(),
        uuid: GOLDEN_UUID.to_string(),
        plan: Some(golden_plan()),
        alive_ip: 0,
        subscribe_url: format!(
            "https://golden.v2board.test/api/v1/client/subscribe?token={GOLDEN_TOKEN}"
        ),
        reset_day: Some(15),
        allow_new_period: 1,
    });

    let comm_config = envelope(UserCommConfig {
        is_telegram: 0,
        telegram_discuss_link: None,
        withdraw_methods: vec!["alipay".to_string(), "usdt".to_string()],
        withdraw_close: 0,
        currency: "CNY".to_string(),
        currency_symbol: "¥".to_string(),
        commission_distribution_enable: 0,
        commission_distribution_l1: Some("50".to_string()),
        commission_distribution_l2: None,
        commission_distribution_l3: None,
    });

    let order_fetch = envelope(vec![golden_plan_order(), golden_deposit_order()]);

    let mut plan_order_detail = golden_plan_order();
    plan_order_detail.try_out_plan_id = Some(0);
    let order_detail = envelope(plan_order_detail);

    let mut deposit_detail = golden_deposit_order();
    deposit_detail.plan = Some(OrderPlan::Deposit(DepositPlan {
        id: 0,
        name: "deposit",
    }));
    deposit_detail.bounus = Some(50);
    deposit_detail.get_amount = Some(550);
    let order_detail_deposit = envelope(deposit_detail);

    let payment_methods = envelope(vec![
        PaymentMethodRow {
            id: 1,
            name: "Golden EPay".to_string(),
            payment: "EPay".to_string(),
            icon: None,
            handling_fee_fixed: Some(20),
            handling_fee_percent: Some("0.50".to_string()),
        },
        PaymentMethodRow {
            id: 2,
            name: "Golden Stripe".to_string(),
            payment: "StripeCheckout".to_string(),
            icon: Some("/icons/stripe.svg".to_string()),
            handling_fee_fixed: None,
            handling_fee_percent: None,
        },
    ]);

    let checkout_redirect = pretty(&CheckoutEnvelope {
        r#type: 1,
        data: json!("https://checkout.golden.test/session/golden-0001"),
    });
    let checkout_qr = pretty(&CheckoutEnvelope {
        r#type: 0,
        data: json!("golden-qr-checkout-payload"),
    });

    let traffic_log = envelope(vec![
        TrafficLogRow {
            u: 1_073_741_824,
            d: 2_147_483_648,
            record_at: GOLDEN_TIME,
            user_id: 2,
            server_rate: "1.00".to_string(),
        },
        TrafficLogRow {
            u: 104_857_600,
            d: 209_715_200,
            record_at: GOLDEN_TIME - 86_400,
            user_id: 2,
            server_rate: "1.50".to_string(),
        },
    ]);

    let guest_config = envelope(GuestConfig {
        tos_url: Some("https://golden.v2board.test/tos".to_string()),
        is_email_verify: 0,
        is_invite_force: 0,
        email_whitelist_suffix: json!(["gmail.com", "example.test"]),
        is_recaptcha: 0,
        recaptcha_site_key: None,
        app_description: Some("Golden description".to_string()),
        app_url: Some("https://golden.v2board.test".to_string()),
        logo: None,
    });
    let guest_config_whitelist_disabled = envelope(GuestConfig {
        tos_url: None,
        is_email_verify: 1,
        is_invite_force: 1,
        email_whitelist_suffix: json!(0),
        is_recaptcha: 1,
        recaptcha_site_key: Some("golden-recaptcha-site-key".to_string()),
        app_description: None,
        app_url: None,
        logo: Some("https://golden.v2board.test/logo.png".to_string()),
    });

    vec![
        ("guest.comm.config.json", guest_config),
        (
            "guest.comm.config.whitelist-disabled.json",
            guest_config_whitelist_disabled,
        ),
        ("passport.auth.login.json", auth_login),
        ("user.checkLogin.admin.json", check_login_admin),
        ("user.checkLogin.json", check_login),
        ("user.comm.config.json", comm_config),
        ("user.getSubscribe.json", subscribe),
        ("user.info.json", user_info),
        ("user.order.checkout.json", checkout_redirect),
        ("user.order.checkout.qr.json", checkout_qr),
        ("user.order.detail.deposit.json", order_detail_deposit),
        ("user.order.detail.json", order_detail),
        ("user.order.fetch.json", order_fetch),
        ("user.order.getPaymentMethod.json", payment_methods),
        ("user.stat.getTrafficLog.json", traffic_log),
    ]
}

#[test]
fn wire_bodies_match_checked_in_goldens() {
    let dir = goldens_dir();
    assert!(
        dir.is_dir(),
        "golden fixtures directory {} is unavailable; run through `make rust-test` or \
         `make contract-goldens` so frontend/packages/api-client/goldens is mounted",
        dir.display()
    );
    let documents = documents();
    let update = std::env::var("UPDATE_GOLDENS").is_ok_and(|value| value == "1");

    let mut failures = Vec::new();
    for (file_name, expected) in &documents {
        let path = dir.join(file_name);
        if update {
            std::fs::write(&path, expected)
                .unwrap_or_else(|error| panic!("write golden fixture {file_name}: {error}"));
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(actual) if &actual == expected => {}
            Ok(_) => failures.push(format!("{file_name}: content drifted")),
            Err(_) => failures.push(format!("{file_name}: fixture is missing")),
        }
    }

    // The fixture set and the directory must stay bijective for the owned
    // prefixes so the vitest suite never carries an unpinned orphan.
    let expected_names: std::collections::BTreeSet<&str> =
        documents.iter().map(|(name, _)| *name).collect();
    for entry in std::fs::read_dir(&dir).expect("list the goldens directory") {
        let file_name = entry.expect("read goldens directory entry").file_name();
        let name = file_name
            .to_str()
            .expect("golden fixture names are UTF-8")
            .to_string();
        let owned = WIRE_GOLDEN_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix));
        if owned && !expected_names.contains(name.as_str()) {
            if update {
                std::fs::remove_file(dir.join(&name))
                    .unwrap_or_else(|error| panic!("remove stale golden fixture {name}: {error}"));
            } else {
                failures.push(format!("{name}: fixture has no generating wire shape"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "wire golden fixtures drifted from the live serialization; regenerate with \
         `make contract-goldens` and review the diff:\n  {}",
        failures.join("\n  ")
    );
}
