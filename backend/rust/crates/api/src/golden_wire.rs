//! Byte-exact golden fixtures for the pure-serde response bodies this crate
//! serializes onto the wire (`auth.*`, `problem.*`, `guest.*`, `passport.*`,
//! `user.*` under `frontend/packages/api-client/goldens`). Each document is a
//! fully rendered modern-dialect response body (docs/api-dialect.md §1, §3):
//! the bare success object or RFC 9457 problem body, around a hand-pinned,
//! realistic struct value (W6 flipped this test's last legacy-envelope
//! fixture). The api-client vitest suite parses every fixture
//! with its zod contract schema, so a serde field rename, type change, or
//! envelope change that would break the TypeScript contract fails here and in
//! the frontend gate instead of drifting silently. The DB-backed `admin.*`
//! fixtures are owned by `v2board-contract golden-responses`.
//!
//! Regenerate with `make contract-goldens` (UPDATE_GOLDENS=1); the default
//! test run verifies the checked-in fixtures byte-for-byte.

use std::path::PathBuf;

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::json;
use v2board_compat::{Code, Problem};
use v2board_db::{
    coupon::CouponRow,
    order::{DepositPlan, OrderPlan, OrderRow},
    payment::PaymentMethodRow,
    plan::PlanRow,
    user::UserInfoRow,
};
use v2board_domain::{auth::AuthData, order::StripePaymentIntentResult};

use crate::{
    auth::{QuickLoginUrl, SessionState, StepUpGrant},
    client::PublicConfig,
    commerce::{
        CheckoutOutcome, CouponBody, CreatedOrder, OrderBody, OrderStatusBody, PaymentMethodBody,
        PlanBody,
    },
    user::{
        account::{SessionBody, UserConfig, UserProfileBody},
        content::{KnowledgeDetail, KnowledgeSummary, NoticeItem, TelegramBot},
        giftcard::GiftCardRedemptionBody,
        invite::{CommissionItem, InviteBody, InviteCodeBody, InviteStatBody},
        stats::{ServerBody, TrafficLogBody, UserStatsBody},
        subscription::{ResetTokenBody, SubscriptionBody},
    },
};

/// 2023-11-14T22:13:20Z, shared with the contract crate's golden generator.
const GOLDEN_TIME: i64 = 1_700_000_000;
const GOLDEN_EXPIRED_AT: i64 = GOLDEN_TIME + 86_400 * 30;
const GOLDEN_EMAIL: &str = "golden-member@example.test";
const GOLDEN_UUID: &str = "00000000-0000-4000-8000-000000000002";
const GOLDEN_TOKEN: &str = "goldenmembertoken000000000000002";
/// The file-name prefixes this test owns inside the shared goldens directory.
/// `passport.` and `guest.` stay owned with zero fixtures so a retired legacy
/// fixture can never linger unpinned (W3 flipped both namespaces' last
/// fixture-bearing routes onto `/public/*`).
const WIRE_GOLDEN_PREFIXES: &[&str] = &[
    "auth.",
    "guest.",
    "passport.",
    "problem.",
    "public.",
    "user.",
];

fn goldens_dir() -> PathBuf {
    match std::env::var("V2BOARD_GOLDENS_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../frontend/packages/api-client/goldens"
        )),
    }
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
    // Modern-dialect auth family (docs/api-dialect.md §5.2): bare success
    // bodies, no envelope. login/register/token-login share the AuthData
    // shape.
    let auth_login = pretty(&AuthData {
        is_admin: false,
        auth_data: "golden-opaque-session-token-0001".to_string(),
    });

    let auth_session = pretty(&SessionState {
        is_login: true,
        is_admin: None,
    });
    let auth_session_admin = pretty(&SessionState {
        is_login: true,
        is_admin: Some(true),
    });
    let auth_session_logged_out = pretty(&SessionState {
        is_login: false,
        is_admin: None,
    });

    let auth_step_up = pretty(&StepUpGrant {
        step_up_token: "golden-step-up-token-000000000001".to_string(),
        expires_in: 900,
    });

    let auth_quick_login_url = pretty(&QuickLoginUrl {
        url: "https://golden.v2board.test/login?verify=golden-temp-token-0001&redirect=dashboard"
            .to_string(),
    });

    // Modern-dialect problem bodies (docs/api-dialect.md §3.1): the exact
    // `{type, title, status, code, detail, errors?}` serialization the
    // api-client problem schema consumes.
    let problem_session_expired = pretty(&Problem::localized(Code::SessionExpired, "zh-CN"));
    let problem_validation = pretty(&Problem::validation(IndexMap::from([(
        "email".to_string(),
        vec!["邮箱格式不正确".to_string()],
    )])));

    // Modern-dialect user account & subscription family (docs/api-dialect.md
    // §5.3, §5.4, §9.1, §9.4, W5): bare bodies, RFC 3339 timestamps, boolean
    // flags, and the named-object tuple/scalar replacements.
    let user_profile = pretty(&UserProfileBody::from(UserInfoRow {
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
    }));

    let user_stats = pretty(&UserStatsBody {
        pending_order_count: 2,
        pending_ticket_count: 0,
        invited_user_count: 7,
    });

    let user_sessions = pretty(&vec![
        SessionBody {
            session_id: "golden-session-digest-0000000002".to_string(),
            ip: "203.0.113.7".to_string(),
            ua: "GoldenBrowser/2.0".to_string(),
            login_at: GOLDEN_TIME + 3_600,
            current: true,
        },
        SessionBody {
            session_id: "golden-session-digest-0000000001".to_string(),
            ip: String::new(),
            ua: String::new(),
            login_at: GOLDEN_TIME,
            current: false,
        },
    ]);

    let gift_card_redemption = pretty(&GiftCardRedemptionBody {
        r#type: 1,
        value: Some(1234),
    });

    let subscription = pretty(&SubscriptionBody {
        plan_id: Some(1),
        token: GOLDEN_TOKEN.to_string(),
        expired_at: Some(GOLDEN_EXPIRED_AT),
        u: 1_073_741_824,
        d: 2_147_483_648,
        transfer_enable: 107_374_182_400,
        device_limit: Some(3),
        email: GOLDEN_EMAIL.to_string(),
        uuid: GOLDEN_UUID.to_string(),
        plan: Some(PlanBody::from(golden_plan())),
        alive_ip: 0,
        subscribe_url: format!(
            "https://golden.v2board.test/api/v1/client/subscribe?token={GOLDEN_TOKEN}"
        ),
        reset_day: Some(15),
        allow_new_period: true,
    });

    let subscription_reset_token = pretty(&ResetTokenBody {
        subscribe_url: format!(
            "https://golden.v2board.test/api/v1/client/subscribe?token={GOLDEN_TOKEN}"
        ),
    });

    // Modern-dialect user config (docs/api-dialect.md §5.3, W3): bare body,
    // boolean flags, numeric distribution rates.
    let user_config = pretty(&UserConfig {
        is_telegram: false,
        telegram_discuss_link: None,
        withdraw_methods: vec!["alipay".to_string(), "usdt".to_string()],
        withdraw_close: false,
        currency: "CNY".to_string(),
        currency_symbol: "¥".to_string(),
        commission_distribution_enable: true,
        commission_distribution_l1: Some(0.5),
        commission_distribution_l2: None,
        commission_distribution_l3: None,
    });

    // Modern-dialect user commerce family (docs/api-dialect.md §5.5, §9.3,
    // §9.4, W4): bare bodies, RFC 3339 timestamps, boolean `show`/`renew`,
    // numeric `handling_fee_percent`, and the discriminated checkout union.
    let plans = pretty(&vec![PlanBody::from(golden_plan())]);
    let plan_detail = pretty(&PlanBody::from(golden_plan()));

    let orders = pretty(&vec![
        OrderBody::from(golden_plan_order()),
        OrderBody::from(golden_deposit_order()),
    ]);

    let mut plan_order_detail = golden_plan_order();
    plan_order_detail.try_out_plan_id = Some(0);
    let order_detail = pretty(&OrderBody::from(plan_order_detail));

    let mut deposit_detail = golden_deposit_order();
    deposit_detail.plan = Some(OrderPlan::Deposit(DepositPlan {
        id: 0,
        name: "deposit",
    }));
    deposit_detail.bounus = Some(50);
    deposit_detail.get_amount = Some(550);
    let order_detail_deposit = pretty(&OrderBody::from(deposit_detail));

    let order_created = pretty(&CreatedOrder {
        trade_no: "golden-trade-plan-00000000000001".to_string(),
    });
    let order_status = pretty(&OrderStatusBody { status: 0 });

    let payment_methods = pretty(&vec![
        PaymentMethodBody::from(PaymentMethodRow {
            id: 1,
            name: "Golden EPay".to_string(),
            payment: "EPay".to_string(),
            icon: None,
            handling_fee_fixed: Some(20),
            handling_fee_percent: Some("0.50".to_string()),
        }),
        PaymentMethodBody::from(PaymentMethodRow {
            id: 2,
            name: "Golden Stripe".to_string(),
            payment: "StripeCheckout".to_string(),
            icon: Some("/icons/stripe.svg".to_string()),
            handling_fee_fixed: None,
            handling_fee_percent: None,
        }),
    ]);

    let checkout_redirect = pretty(&CheckoutOutcome::Redirect {
        url: "https://checkout.golden.test/session/golden-0001".to_string(),
    });
    let checkout_qr = pretty(&CheckoutOutcome::QrCode {
        payload: "golden-qr-checkout-payload".to_string(),
    });
    let checkout_settled = pretty(&CheckoutOutcome::Settled);

    let stripe_intent = pretty(&StripePaymentIntentResult {
        public_key: "pk_test_golden000000000000000001".to_string(),
        client_secret: "pi_golden_secret_000000000000001".to_string(),
        amount: 1020,
        currency: "usd".to_string(),
    });

    let coupon_check = pretty(&CouponBody::from(CouponRow {
        id: 1,
        code: "GOLDEN10".to_string(),
        name: "Golden Coupon".to_string(),
        r#type: 2,
        value: 10,
        show: 1,
        limit_use: Some(100),
        limit_use_with_user: Some(1),
        limit_plan_ids: Some(vec![1]),
        limit_period: Some(vec!["month_price".to_string()]),
        started_at: GOLDEN_TIME - 86_400,
        ended_at: GOLDEN_EXPIRED_AT,
        created_at: GOLDEN_TIME,
        updated_at: GOLDEN_TIME,
    }));

    // Modern-dialect service family (docs/api-dialect.md §5.4, W6): bare
    // arrays, numeric rates/ports, boolean is_online, RFC 3339 timestamps.
    let traffic_logs = pretty(&vec![
        TrafficLogBody {
            u: 1_073_741_824,
            d: 2_147_483_648,
            record_at: GOLDEN_TIME,
            user_id: 2,
            server_rate: 1.0,
        },
        TrafficLogBody {
            u: 104_857_600,
            d: 209_715_200,
            record_at: GOLDEN_TIME - 86_400,
            user_id: 2,
            server_rate: 1.5,
        },
    ]);

    let servers = pretty(&vec![
        ServerBody {
            id: 1,
            parent_id: None,
            group_id: vec![1],
            route_id: Some(vec![2]),
            name: "Golden Node 01".to_string(),
            rate: 1.0,
            r#type: "shadowsocks".to_string(),
            host: "node-01.golden.v2board.test".to_string(),
            port: 443,
            cache_key: format!("shadowsocks-1-{GOLDEN_TIME}-1"),
            last_check_at: Some(GOLDEN_TIME),
            is_online: true,
            tags: Some(vec!["IEPL".to_string(), "Golden".to_string()]),
            sort: Some(1),
            extra: json!({
                "cipher": "aes-128-gcm",
                "obfs": null,
                "obfs_settings": null,
                "created_at": GOLDEN_TIME,
            }),
        },
        ServerBody {
            id: 2,
            parent_id: None,
            group_id: vec![1],
            route_id: None,
            name: "Golden Node 02".to_string(),
            rate: 2.5,
            r#type: "trojan".to_string(),
            host: "node-02.golden.v2board.test".to_string(),
            port: 8443,
            cache_key: format!("trojan-2-{GOLDEN_TIME}-0"),
            last_check_at: None,
            is_online: false,
            tags: None,
            sort: Some(2),
            // Null extra pins the skip_serializing_if omission.
            extra: serde_json::Value::Null,
        },
    ]);

    // Modern-dialect public config (docs/api-dialect.md §5.1, W3): bare body,
    // boolean flags, and an always-array whitelist (the `0` sentinel died).
    let public_config = pretty(&PublicConfig {
        tos_url: Some("https://golden.v2board.test/tos".to_string()),
        is_email_verify: false,
        is_invite_force: false,
        email_whitelist_suffix: vec!["gmail.com".to_string(), "example.test".to_string()],
        is_recaptcha: false,
        recaptcha_site_key: None,
        app_description: Some("Golden description".to_string()),
        app_url: Some("https://golden.v2board.test".to_string()),
        logo: None,
    });
    let public_config_whitelist_disabled = pretty(&PublicConfig {
        tos_url: None,
        is_email_verify: true,
        is_invite_force: true,
        email_whitelist_suffix: Vec::new(),
        is_recaptcha: true,
        recaptcha_site_key: Some("golden-recaptcha-site-key".to_string()),
        app_description: None,
        app_url: None,
        logo: Some("https://golden.v2board.test/logo.png".to_string()),
    });

    // Modern-dialect user content family (docs/api-dialect.md §5.8, W3).
    let notices_page = pretty(&v2board_compat::Page {
        items: vec![
            NoticeItem {
                id: 2,
                title: "Golden popup notice".to_string(),
                content: "<p>Golden popup body</p>".to_string(),
                show: true,
                img_url: None,
                tags: Some(vec!["弹窗".to_string()]),
                created_at: GOLDEN_TIME + 86_400,
                updated_at: GOLDEN_TIME + 86_400,
            },
            NoticeItem {
                id: 1,
                title: "Golden notice".to_string(),
                content: "<p>Golden notice body</p>".to_string(),
                show: true,
                img_url: Some("https://golden.v2board.test/notice.png".to_string()),
                tags: None,
                created_at: GOLDEN_TIME,
                updated_at: GOLDEN_TIME,
            },
        ],
        total: 7,
    });

    let knowledge_list = pretty(&std::collections::BTreeMap::from([(
        "Golden Apps".to_string(),
        vec![KnowledgeSummary {
            id: 3,
            category: "Golden Apps".to_string(),
            title: "Golden setup guide".to_string(),
            sort: Some(1),
            show: true,
            updated_at: GOLDEN_TIME,
        }],
    )]));

    let knowledge_detail = pretty(&KnowledgeDetail {
        id: 3,
        language: "en-US".to_string(),
        category: "Golden Apps".to_string(),
        title: "Golden setup guide".to_string(),
        body: format!(
            "<p>Use https://golden.v2board.test/api/v1/client/subscribe?token={GOLDEN_TOKEN}</p>"
        ),
        sort: Some(1),
        show: true,
        created_at: GOLDEN_TIME,
        updated_at: GOLDEN_TIME,
    });

    let knowledge_categories = pretty(&vec![
        json!({ "category": "Golden Apps" }),
        json!({ "category": "Golden Tutorials" }),
    ]);

    let telegram_bot = pretty(&TelegramBot {
        username: "golden_v2board_bot".to_string(),
    });

    // Modern-dialect invite & commission family (docs/api-dialect.md §5.6,
    // §8, §9.2, W7): the bare `{codes, stat}` body with the named stat
    // object, and the commissions `{items, total}` page envelope. Money
    // stays integer cents.
    let invite = pretty(&InviteBody {
        codes: vec![
            InviteCodeBody {
                id: 2,
                code: "goldinv2".to_string(),
                pv: 0,
                created_at: GOLDEN_TIME + 86_400,
                updated_at: GOLDEN_TIME + 86_400,
            },
            InviteCodeBody {
                id: 1,
                code: "goldinv1".to_string(),
                pv: 3,
                created_at: GOLDEN_TIME,
                updated_at: GOLDEN_TIME,
            },
        ],
        stat: InviteStatBody {
            registered_count: 12,
            valid_commission: 12_300,
            pending_commission: 4_500,
            commission_rate: 10,
            available_commission: 8_000,
        },
    });

    let commissions = pretty(&v2board_compat::Page {
        items: vec![
            CommissionItem {
                id: 2,
                trade_no: "golden-trade-commission-0000002".to_string(),
                order_amount: 2_000,
                get_amount: 200,
                created_at: GOLDEN_TIME + 86_400,
            },
            CommissionItem {
                id: 1,
                trade_no: "golden-trade-commission-0000001".to_string(),
                order_amount: 1_000,
                get_amount: 100,
                created_at: GOLDEN_TIME,
            },
        ],
        total: 12,
    });

    vec![
        ("auth.login.json", auth_login),
        ("auth.quick-login-url.json", auth_quick_login_url),
        ("auth.session.admin.json", auth_session_admin),
        ("auth.session.json", auth_session),
        ("auth.session.logged-out.json", auth_session_logged_out),
        ("auth.step-up.json", auth_step_up),
        ("problem.session-expired.json", problem_session_expired),
        ("problem.validation.json", problem_validation),
        ("public.config.json", public_config),
        (
            "public.config.whitelist-disabled.json",
            public_config_whitelist_disabled,
        ),
        ("user.config.json", user_config),
        (
            "user.gift-card-redemptions.create.json",
            gift_card_redemption,
        ),
        ("user.commissions.json", commissions),
        ("user.invite.json", invite),
        ("user.knowledge-categories.json", knowledge_categories),
        ("user.knowledge.detail.json", knowledge_detail),
        ("user.knowledge.json", knowledge_list),
        ("user.profile.json", user_profile),
        ("user.sessions.json", user_sessions),
        ("user.stats.json", user_stats),
        ("user.subscription.json", subscription),
        (
            "user.subscription.reset-token.json",
            subscription_reset_token,
        ),
        ("user.coupons.check.json", coupon_check),
        ("user.orders.checkout.qr.json", checkout_qr),
        ("user.orders.checkout.redirect.json", checkout_redirect),
        ("user.orders.checkout.settled.json", checkout_settled),
        ("user.orders.create.json", order_created),
        ("user.orders.detail.deposit.json", order_detail_deposit),
        ("user.orders.detail.json", order_detail),
        ("user.orders.json", orders),
        ("user.orders.status.json", order_status),
        ("user.orders.stripe-intent.json", stripe_intent),
        ("user.payment-methods.json", payment_methods),
        ("user.plans.detail.json", plan_detail),
        ("user.plans.json", plans),
        ("user.notices.json", notices_page),
        ("user.servers.json", servers),
        ("user.telegram-bot.json", telegram_bot),
        ("user.traffic-logs.json", traffic_logs),
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
