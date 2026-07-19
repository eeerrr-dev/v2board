use std::{collections::BTreeSet, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;
use v2board_db::installation_id;
use v2board_domain::{admin::AdminService, auth::PasswordKdf, smtp::SmtpTransportCache};

use super::harness::{insert_user, integration_config, sha256_bytes};

/// The exact serialized key set of every reachable admin endpoint that emits a
/// SQL-projected row. Each constant is the producer-side contract for one
/// endpoint's row shape: a new or removed key here is an API change the admin
/// frontend and its api-client contracts must consciously absorb, never an
/// accidental projection leak.
/// The modern W12 admin user projection (docs/api-dialect.md §6.6): the
/// credential column `t` is dropped alongside the already-absent
/// `password_algo`/`password_salt`/`last_login_ip`, and every epoch field
/// crosses the boundary as an RFC 3339 UTC string (§4.5).
const ADMIN_USER_ROW_KEYS: &[&str] = &[
    "id",
    "email",
    "password",
    "balance",
    "commission_balance",
    "transfer_enable",
    "device_limit",
    "u",
    "d",
    "total_used",
    "alive_ip",
    "ips",
    "plan_id",
    "plan_name",
    "group_id",
    "expired_at",
    "uuid",
    "token",
    "subscribe_url",
    "banned",
    "is_admin",
    "is_staff",
    "invite_user_id",
    "discount",
    "commission_type",
    "commission_rate",
    "speed_limit",
    "auto_renewal",
    "remind_expire",
    "remind_traffic",
    "remarks",
    "telegram_id",
    "last_login_at",
    "created_at",
    "updated_at",
];

const ADMIN_USER_TRAFFIC_KEYS: &[&str] = &["record_at", "u", "d", "server_rate"];

/// The W14 §6.8 series re-spec: `stats/orders` and `stats/records` rows are
/// `{series, date, value}` with stable snake_case series slugs.
const ADMIN_STAT_SERIES_KEYS: &[&str] = &["series", "date", "value"];

const ADMIN_SYSTEM_LOG_KEYS: &[&str] = &[
    "id",
    "title",
    "level",
    "host",
    "uri",
    "method",
    "data",
    "ip",
    "context",
    "created_at",
    "updated_at",
];

const ADMIN_AUDIT_LOG_KEYS: &[&str] = &[
    "id",
    "actor_id",
    "actor_email",
    "session_id",
    "surface",
    "method",
    "path",
    "status_code",
    "client_ip",
    "request_id",
    "created_at",
];

const ADMIN_KNOWLEDGE_LIST_KEYS: &[&str] =
    &["id", "category", "title", "sort", "show", "updated_at"];

const ADMIN_KNOWLEDGE_DETAIL_KEYS: &[&str] = &[
    "id",
    "language",
    "category",
    "title",
    "body",
    "sort",
    "show",
    "created_at",
    "updated_at",
];

const ADMIN_TICKET_ROW_KEYS: &[&str] = &[
    "id",
    "user_id",
    "subject",
    "level",
    "status",
    "reply_status",
    "last_reply_user_id",
    "created_at",
    "updated_at",
];

const ADMIN_TICKET_MESSAGE_KEYS: &[&str] = &[
    "id",
    "user_id",
    "ticket_id",
    "message",
    "is_me",
    "created_at",
    "updated_at",
];

const ADMIN_COUPON_KEYS: &[&str] = &[
    "id",
    "code",
    "name",
    "type",
    "value",
    "show",
    "limit_use",
    "limit_use_with_user",
    "limit_plan_ids",
    "limit_period",
    "started_at",
    "ended_at",
    "created_at",
    "updated_at",
];

const ADMIN_GIFTCARD_KEYS: &[&str] = &[
    "id",
    "code",
    "name",
    "type",
    "value",
    "plan_id",
    "limit_use",
    "used_user_ids",
    "started_at",
    "ended_at",
    "created_at",
    "updated_at",
];

const ADMIN_ORDER_FETCH_KEYS: &[&str] = &[
    "id",
    "invite_user_id",
    "user_id",
    "email",
    "plan_id",
    "plan_name",
    "coupon_id",
    "type",
    "period",
    "trade_no",
    "callback_no",
    "total_amount",
    "handling_amount",
    "discount_amount",
    "surplus_amount",
    "refund_amount",
    "balance_amount",
    "surplus_order_ids",
    "status",
    "commission_status",
    "commission_balance",
    "actual_commission_balance",
    "payment_id",
    "payment_reconciliation_open_count",
    "paid_at",
    "created_at",
    "updated_at",
];

const ADMIN_ORDER_DETAIL_KEYS: &[&str] = &[
    "id",
    "invite_user_id",
    "user_id",
    "plan_id",
    "coupon_id",
    "type",
    "period",
    "trade_no",
    "callback_no",
    "total_amount",
    "handling_amount",
    "discount_amount",
    "surplus_amount",
    "refund_amount",
    "balance_amount",
    "surplus_order_ids",
    "status",
    "commission_status",
    "commission_balance",
    "actual_commission_balance",
    "payment_id",
    "paid_at",
    "created_at",
    "updated_at",
    "commission_log",
    "payment_reconciliations",
];

const ADMIN_COMMISSION_LOG_KEYS: &[&str] = &[
    "id",
    "invite_user_id",
    "user_id",
    "trade_no",
    "order_amount",
    "get_amount",
    "created_at",
    "updated_at",
];

const ADMIN_ORDER_RECONCILIATION_KEYS: &[&str] = &[
    "id",
    "payment_id",
    "provider",
    "trade_no",
    "trade_no_hash",
    "callback_no",
    "callback_no_hash",
    "reason",
    "order_status",
    "expected_amount",
    "settled_amount",
    "occurrence_count",
    "first_seen_at",
    "last_seen_at",
    "resolved_at",
    "resolution",
];

const ADMIN_RECONCILIATION_FETCH_KEYS: &[&str] = &[
    "id",
    "payment_id",
    "payment_name",
    "payment_archived_at",
    "provider",
    "trade_no",
    "trade_no_hash",
    "callback_no",
    "callback_no_hash",
    "reason",
    "order_status",
    "expected_amount",
    "settled_amount",
    "occurrence_count",
    "first_seen_at",
    "last_seen_at",
    "resolved_at",
    "resolution",
];

const ADMIN_SERVER_GROUP_LIST_KEYS: &[&str] = &[
    "id",
    "name",
    "created_at",
    "updated_at",
    "user_count",
    "server_count",
];

const ADMIN_SERVER_ROUTE_KEYS: &[&str] = &[
    "id",
    "remarks",
    "match",
    "action",
    "action_value",
    "created_at",
    "updated_at",
];

const ADMIN_SHADOWSOCKS_NODE_KEYS: &[&str] = &[
    "id",
    "group_id",
    "route_id",
    "parent_id",
    "tags",
    "name",
    "rate",
    "host",
    "port",
    "server_port",
    "cipher",
    "obfs",
    "obfs_settings",
    "show",
    "sort",
    "created_at",
    "updated_at",
    "type",
    "online",
    "last_check_at",
    "last_push_at",
    "available_status",
    "api_key",
];

fn assert_exact_keys(context: &str, value: &serde_json::Value, expected: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .with_context(|| format!("{context}: row is not a JSON object"))?;
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    if actual != expected {
        let unexpected: Vec<_> = actual.difference(&expected).collect();
        let missing: Vec<_> = expected.difference(&actual).collect();
        bail!(
            "{context}: serialized key set drifted (unexpected {unexpected:?}, missing {missing:?})"
        );
    }
    Ok(())
}

/// §4.5: the named fields must cross the boundary as RFC 3339 UTC strings
/// (or null for nullable timestamps), never as raw epoch integers.
fn assert_rfc3339_fields(context: &str, value: &serde_json::Value, fields: &[&str]) -> Result<()> {
    for field in fields {
        let field_value = &value[*field];
        if field_value.is_null() {
            continue;
        }
        let text = field_value
            .as_str()
            .with_context(|| format!("{context}: {field} is not an RFC 3339 string"))?;
        chrono::DateTime::parse_from_rfc3339(text)
            .with_context(|| format!("{context}: {field} is not valid RFC 3339 ({text})"))?;
    }
    Ok(())
}

/// Pins the exact serialized key set of every reachable admin endpoint whose
/// rows are produced by a SQL projection, using the real AdminService against
/// the migrated schema. This is the DB-backed producer-side contract: any
/// projection edit that adds, renames, or drops a key fails here before it can
/// silently leak (or break) a field the admin frontend consumes.
pub(super) async fn admin_projection_key_sets(pool: &PgPool, redis_url: &str) -> Result<()> {
    let now = Utc::now().timestamp();
    let admin = AdminService::new(
        pool.clone(),
        redis::Client::open(redis_url)?,
        installation_id(pool).await?,
        Arc::new(integration_config(pool, redis_url)?),
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()?,
        PasswordKdf::new(1),
        SmtpTransportCache::default(),
    );

    // Users: one inviter-linked target so the detail endpoint exercises the
    // conditional invite_user attachment, and the inviter itself as the
    // inviter-less shape.
    let inviter_id = insert_user(pool, "projection-inviter", "hash").await?;
    let user_id = insert_user(pool, "projection-user", "hash").await?;
    sqlx::query("UPDATE users SET invite_user_id = $1 WHERE id = $2")
        .bind(inviter_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    // GET users (§6.6): the modern DSL list, W12 projection (no `t`).
    let (rows, _total) = admin
        .users_list(
            v2board_compat::Pagination::resolve(None, None, 10)
                .map_err(|problem| anyhow::anyhow!("users pagination: {problem:?}"))?,
            None,
            None,
            None,
        )
        .await?;
    for row in &rows {
        assert_exact_keys("users", row, ADMIN_USER_ROW_KEYS)?;
    }

    // GET users/{id} (§6.6): the bare projection with the conditional inviter.
    let with_inviter = admin.user_detail(user_id).await?;
    let mut detail_keys = ADMIN_USER_ROW_KEYS.to_vec();
    detail_keys.push("invite_user");
    assert_exact_keys("users/{id} (invited)", &with_inviter, &detail_keys)?;
    assert_exact_keys(
        "users/{id} invite_user",
        &with_inviter["invite_user"],
        &["id", "email"],
    )?;
    let without_inviter = admin.user_detail(inviter_id).await?;
    assert_exact_keys(
        "users/{id} (no inviter)",
        &without_inviter,
        ADMIN_USER_ROW_KEYS,
    )?;

    // Staff GET users/{id} (§6.9, W14): the staff-redacted view now shares
    // the W12 v2 projection — `t` dropped, RFC 3339 timestamps, and never an
    // `invite_user` attachment.
    let staff_detail = admin.staff_user_detail(user_id).await?;
    assert_exact_keys("staff users/{id}", &staff_detail, ADMIN_USER_ROW_KEYS)?;
    assert_rfc3339_fields(
        "staff users/{id}",
        &staff_detail,
        &["expired_at", "last_login_at", "created_at", "updated_at"],
    )?;

    // Per-user traffic history.
    sqlx::query(
        "INSERT INTO user_traffic (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at) \
         VALUES ($1, 1.00, 100, 200, 'd', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    // GET stats/user-traffic (§6.8, W14): §8 page, `server_rate` as a JSON
    // number, `record_at` RFC 3339.
    let (traffic_rows, traffic_total) = admin
        .stats_user_traffic(
            user_id,
            v2board_compat::Pagination::resolve(None, None, 10)
                .map_err(|problem| anyhow::anyhow!("stats/user-traffic pagination: {problem:?}"))?,
        )
        .await?;
    ensure!(
        traffic_total >= 1 && !traffic_rows.is_empty(),
        "stats/user-traffic must return the seeded projection row"
    );
    for row in &traffic_rows {
        assert_exact_keys("stats/user-traffic", row, ADMIN_USER_TRAFFIC_KEYS)?;
        ensure!(
            row["server_rate"].is_f64(),
            "stats/user-traffic: server_rate must cross as a JSON number"
        );
        assert_rfc3339_fields("stats/user-traffic", row, &["record_at"])?;
    }

    // Aggregated stat records.
    sqlx::query(
        "INSERT INTO stat (record_at, record_type, order_count, order_total, commission_count, \
         commission_total, paid_count, paid_total, register_count, invite_count, \
         transfer_used_total, created_at, updated_at) \
         VALUES ($1, 'd', 0, 0, 0, 0, 0, 0, 0, 0, '0', $2, $3)",
    )
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    // GET stats/records + stats/orders (§6.8, W14): the series re-spec —
    // `{series,date,value}` rows, snake_case slugs, integer values.
    let record_rows = admin.stats_records("d").await?;
    ensure!(
        !record_rows.is_empty(),
        "stats/records must return the seeded stat period"
    );
    for row in &record_rows {
        assert_exact_keys("stats/records", row, ADMIN_STAT_SERIES_KEYS)?;
        ensure!(
            row["value"].is_i64(),
            "stats/records: value must be an integer (cents/counts)"
        );
    }
    let order_rows = admin.stats_orders().await?;
    ensure!(
        !order_rows.is_empty(),
        "stats/orders must return the seeded stat period"
    );
    for row in &order_rows {
        assert_exact_keys("stats/orders", row, ADMIN_STAT_SERIES_KEYS)?;
    }

    // System log.
    sqlx::query(
        "INSERT INTO system_log (title, level, host, uri, method, data, ip, context, created_at, updated_at) \
         VALUES ('projection pin', 'info', 'localhost', '/projection', 'GET', NULL, '127.0.0.1', NULL, $1, $2)",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    // W9 modern route: GET system/logs (§8 pagination + §7 DSL). The row key
    // set is unchanged from the legacy projection.
    let (log_rows, log_total) = admin
        .system_logs(
            v2board_compat::Pagination::resolve(None, None, 10)
                .map_err(|problem| anyhow::anyhow!("system/logs pagination: {problem:?}"))?,
            Some(r#"[{"field":"level","op":"eq","value":"info"}]"#),
            None,
            None,
        )
        .await?;
    if log_total < 1 || log_rows.is_empty() {
        anyhow::bail!("system/logs must return the seeded projection row");
    }
    for row in log_rows {
        assert_exact_keys("system/logs", &row, ADMIN_SYSTEM_LOG_KEYS)?;
    }

    // Operator audit trail (§6.11 native addition: GET system/audit-logs,
    // same §8 pagination + §7 DSL; the trail itself is append-only).
    sqlx::query(
        "INSERT INTO audit_log (actor_id, actor_email, session_id, surface, method, path, status_code, client_ip, request_id, created_at) \
         VALUES ($1, 'projection@example.com', 'projection-session', 'admin', 'POST', '/config', 200, '127.0.0.1', 'projection-request', $2)",
    )
    .bind(user_id)
    .bind(now)
    .execute(pool)
    .await?;
    let (audit_rows, audit_total) = admin
        .audit_logs(
            v2board_compat::Pagination::resolve(None, None, 10)
                .map_err(|problem| anyhow::anyhow!("system/audit-logs pagination: {problem:?}"))?,
            Some(r#"[{"field":"surface","op":"eq","value":"admin"}]"#),
            None,
            None,
        )
        .await?;
    if audit_total < 1 || audit_rows.is_empty() {
        anyhow::bail!("system/audit-logs must return the seeded projection row");
    }
    for row in audit_rows {
        assert_exact_keys("system/audit-logs", &row, ADMIN_AUDIT_LOG_KEYS)?;
    }

    // Knowledge list + detail (W10 modern routes: GET knowledge,
    // GET knowledge/{id} — key names unchanged; `show` is boolean and the
    // timestamps are RFC 3339 strings on the modern wire).
    let knowledge_id: i32 = sqlx::query_scalar(
        "INSERT INTO knowledge (language, category, title, body, sort, show, created_at, updated_at) \
         VALUES ('zh-CN', 'projection', 'projection pin', 'body', NULL, 0, $1, $2) RETURNING id",
    )
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    let knowledge_rows = admin.knowledge_list().await?;
    ensure!(
        !knowledge_rows.is_empty(),
        "knowledge list must return the seeded projection row"
    );
    for row in &knowledge_rows {
        assert_exact_keys(
            "knowledge",
            &serde_json::to_value(row)?,
            ADMIN_KNOWLEDGE_LIST_KEYS,
        )?;
    }
    let knowledge_detail = admin.knowledge_detail(i64::from(knowledge_id)).await?;
    assert_exact_keys(
        "knowledge detail",
        &serde_json::to_value(&knowledge_detail)?,
        ADMIN_KNOWLEDGE_DETAIL_KEYS,
    )?;

    // Ticket list + detail with one message.
    let ticket_id: i64 = sqlx::query_scalar(
        "INSERT INTO ticket (user_id, subject, level, status, reply_status, created_at, updated_at) \
         VALUES ($1, 'projection pin', 1, 0, 0, $2, $3) RETURNING id",
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO ticket_message (user_id, ticket_id, message, created_at, updated_at) \
         VALUES ($1, $2, 'projection message', $3, $4)",
    )
    .bind(user_id)
    .bind(ticket_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    // GET tickets (§6.5, W14) on both prefixes: the admin list and the §6.9
    // staff mirror share the projection (key set unchanged from the legacy
    // rows; timestamps now RFC 3339).
    let ticket_pagination = || {
        v2board_compat::Pagination::resolve(None, None, 10)
            .map_err(|problem| anyhow::anyhow!("tickets pagination: {problem:?}"))
    };
    for (context, staff) in [("tickets", false), ("staff tickets", true)] {
        let (ticket_rows, ticket_total) = admin
            .tickets_list(ticket_pagination()?, None, &[], None, staff)
            .await?;
        ensure!(
            ticket_total >= 1 && !ticket_rows.is_empty(),
            "{context}: list must return the seeded projection row"
        );
        for row in &ticket_rows {
            assert_exact_keys(context, row, ADMIN_TICKET_ROW_KEYS)?;
            assert_rfc3339_fields(context, row, &["created_at", "updated_at"])?;
        }
    }
    let ticket_detail = admin.ticket_detail(ticket_id).await?;
    let mut ticket_detail_keys = ADMIN_TICKET_ROW_KEYS.to_vec();
    ticket_detail_keys.push("message");
    assert_exact_keys("tickets/{id}", &ticket_detail, &ticket_detail_keys)?;
    let ticket_messages = ticket_detail["message"]
        .as_array()
        .context("tickets/{id}: message is not an array")?;
    ensure!(
        !ticket_messages.is_empty(),
        "tickets/{{id}} returned no messages"
    );
    for message in ticket_messages {
        assert_exact_keys("tickets/{id} message", message, ADMIN_TICKET_MESSAGE_KEYS)?;
        assert_rfc3339_fields(
            "tickets/{id} message",
            message,
            &["created_at", "updated_at"],
        )?;
    }

    // Coupons and gift cards.
    sqlx::query(
        "INSERT INTO coupon (code, name, type, value, show, started_at, ended_at, created_at, updated_at) \
         VALUES ($1, 'projection pin', 1, 100, 0, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now)
    .bind(now + 3600)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let (coupon_rows, coupon_total) = admin
        .coupons_list(
            v2board_compat::Pagination {
                page: 1,
                per_page: 10,
            },
            None,
            None,
        )
        .await?;
    ensure!(
        coupon_total >= 1 && !coupon_rows.is_empty(),
        "coupons list must return the seeded projection row"
    );
    for row in &coupon_rows {
        assert_exact_keys("coupons", &serde_json::to_value(row)?, ADMIN_COUPON_KEYS)?;
    }
    sqlx::query(
        "INSERT INTO gift_card (code, name, type, value, started_at, ended_at, created_at, updated_at) \
         VALUES ($1, 'projection pin', 1, 100, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4().simple().to_string())
    .bind(now)
    .bind(now + 3600)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let (giftcard_rows, giftcard_total) = admin
        .giftcards_list(
            v2board_compat::Pagination {
                page: 1,
                per_page: 10,
            },
            None,
            None,
        )
        .await?;
    ensure!(
        giftcard_total >= 1 && !giftcard_rows.is_empty(),
        "gift-cards list must return the seeded projection row"
    );
    for row in &giftcard_rows {
        assert_exact_keys(
            "gift-cards",
            &serde_json::to_value(row)?,
            ADMIN_GIFTCARD_KEYS,
        )?;
    }

    // Orders: a pending order with one commission log and one open
    // reconciliation row bound to the same trade identity.
    let trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        "INSERT INTO orders (user_id, plan_id, type, period, trade_no, total_amount, status, \
         commission_status, commission_balance, created_at, updated_at) \
         VALUES ($1, 0, 1, 'deposit', $2, 500, 0, 0, 0, $3, $4)",
    )
    .bind(user_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO commission_log (invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at) \
         VALUES ($1, $2, $3, 500, 50, $4, $5)",
    )
    .bind(inviter_id)
    .bind(user_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let projection_payment_uuid = Uuid::new_v4().simple().to_string();
    let projection_payment_id: i32 = sqlx::query_scalar(
        "INSERT INTO payment_method (uuid, payment, name, config, enable, created_at, updated_at) \
         VALUES ($1, 'EPay', 'projection pin', $2, 0, $3, $4) RETURNING id",
    )
    .bind(&projection_payment_uuid)
    .bind(super::harness::encrypt_payment_fixture_config(
        "EPay",
        &projection_payment_uuid,
        &serde_json::json!({ "key": "k", "pid": "p", "url": "https://pay.invalid" }),
    )?)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    let callback_no = format!("PROJECTION-{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO payment_reconciliation (payment_id, provider, trade_no, trade_no_hash, \
         callback_no, callback_no_hash, reason, order_status, expected_amount, settled_amount, \
         occurrence_count, first_seen_at, last_seen_at) \
         VALUES ($1, 'EPay', $2, $3, $4, $5, 'order_not_found', 0, 500, NULL, 1, $6, $7)",
    )
    .bind(projection_payment_id)
    .bind(&trade_no)
    .bind(sha256_bytes(&trade_no))
    .bind(&callback_no)
    .bind(sha256_bytes(&callback_no))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    // W11 modern commerce routes: GET orders, GET orders/{trade_no},
    // GET payment-reconciliations. Order projections keep their key sets; the
    // list is `{items,total}` and the detail is a bare object.
    let commerce_page = || v2board_compat::Pagination {
        page: 1,
        per_page: 10,
    };
    let (order_rows, order_total) = admin
        .orders_list(commerce_page(), None, None, None, false)
        .await?;
    ensure!(
        order_total >= 1 && !order_rows.is_empty(),
        "orders list must return the seeded projection row"
    );
    for row in &order_rows {
        assert_exact_keys("orders", row, ADMIN_ORDER_FETCH_KEYS)?;
    }
    let order_detail = admin.order_detail(&trade_no).await?;
    assert_exact_keys("orders detail", &order_detail, ADMIN_ORDER_DETAIL_KEYS)?;
    let commission_rows = order_detail["commission_log"]
        .as_array()
        .context("orders detail: commission_log is not an array")?;
    ensure!(
        !commission_rows.is_empty(),
        "orders detail returned no commission log rows"
    );
    for row in commission_rows {
        assert_exact_keys(
            "orders detail commission_log",
            row,
            ADMIN_COMMISSION_LOG_KEYS,
        )?;
    }
    let reconciliation_rows = order_detail["payment_reconciliations"]
        .as_array()
        .context("orders detail: payment_reconciliations is not an array")?;
    ensure!(
        !reconciliation_rows.is_empty(),
        "orders detail returned no reconciliation rows"
    );
    for row in reconciliation_rows {
        assert_exact_keys(
            "orders detail payment_reconciliations",
            row,
            ADMIN_ORDER_RECONCILIATION_KEYS,
        )?;
    }
    let (reconciliation_list_rows, reconciliation_total) = admin
        .reconciliations_list(commerce_page(), None, None, None, Some(&trade_no), None)
        .await?;
    ensure!(
        reconciliation_total >= 1 && !reconciliation_list_rows.is_empty(),
        "payment-reconciliations must return the seeded projection row"
    );
    for row in &reconciliation_list_rows {
        assert_exact_keys(
            "payment-reconciliations",
            row,
            ADMIN_RECONCILIATION_FETCH_KEYS,
        )?;
    }

    // Server groups, routes, and one shadowsocks node through the W13
    // dialect-v2 projections (docs/api-dialect.md §6.7): bare arrays, RFC
    // 3339 timestamps, `show` boolean, numeric `rate`.
    let group_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) VALUES ('projection pin', $1, $2) RETURNING id",
    )
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    for row in admin.server_groups_list(None).await? {
        assert_exact_keys("server-groups", &row, ADMIN_SERVER_GROUP_LIST_KEYS)?;
        assert_rfc3339_fields("server-groups", &row, &["created_at", "updated_at"])?;
    }
    // The single-group filter keeps the same uniform projection (the legacy
    // count-less single fetch shape is gone with the dialect flip).
    let single_group = admin.server_groups_list(Some(i64::from(group_id))).await?;
    ensure!(
        single_group.len() == 1,
        "server-groups?group_id must return exactly the one matching row"
    );
    assert_exact_keys(
        "server-groups single",
        &single_group[0],
        ADMIN_SERVER_GROUP_LIST_KEYS,
    )?;

    sqlx::query(
        "INSERT INTO server_route (remarks, match, action, action_value, created_at, updated_at) \
         VALUES ('projection pin', '[\"*\"]'::jsonb, 'block', NULL, $1, $2)",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    for row in admin.server_routes_list().await? {
        assert_exact_keys("server-routes", &row, ADMIN_SERVER_ROUTE_KEYS)?;
        ensure!(
            row["match"].is_array(),
            "server-routes: match must always be an array"
        );
        assert_rfc3339_fields("server-routes", &row, &["created_at", "updated_at"])?;
    }

    let node_name = format!("projection-node-{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO server_shadowsocks (group_id, name, rate, host, port, server_port, cipher, created_at, updated_at) \
         VALUES ('[1]'::jsonb, $1, '1', 'ss.projection.test', '443', 443, 'aes-128-gcm', $2, $3)",
    )
    .bind(&node_name)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let nodes = admin.nodes_list().await?;
    let node = nodes
        .iter()
        .find(|node| node["name"].as_str() == Some(node_name.as_str()))
        .context("seeded shadowsocks node missing from GET nodes")?;
    assert_exact_keys("nodes shadowsocks", node, ADMIN_SHADOWSOCKS_NODE_KEYS)?;
    ensure!(
        node["show"].is_boolean(),
        "nodes: show must cross as a JSON boolean"
    );
    ensure!(
        node["rate"].is_number(),
        "nodes: rate must cross as a JSON number"
    );
    ensure!(
        node["port"].is_number() && node["server_port"].is_number(),
        "nodes: port/server_port must cross as JSON numbers"
    );
    assert_rfc3339_fields("nodes", node, &["created_at", "updated_at"])?;

    Ok(())
}
