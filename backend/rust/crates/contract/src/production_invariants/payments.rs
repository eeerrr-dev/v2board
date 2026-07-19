use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use chrono::Utc;
use sqlx::PgPool;
use tokio::task::JoinSet;
use uuid::Uuid;
use v2board_db::installation_id;
use v2board_domain::{
    admin::AdminService,
    auth::PasswordKdf,
    order::{OrderService, PaymentNotifyInput},
    smtp::SmtpTransportCache,
};

use super::harness::{insert_user, integration_config, sha256_bytes, sha256_hex};

pub(super) async fn late_payment_reconciliation(
    pool: &PgPool,
    database_url: &str,
    redis_url: &str,
) -> Result<()> {
    let user_id = insert_user(pool, "late-payment", "not-used").await?;
    let payment_uuid = Uuid::new_v4().simple().to_string();
    let now = Utc::now().timestamp();
    let payment_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO payment_method (
            uuid, payment, name, config, enable, created_at, updated_at
        ) VALUES ($1, 'EPay', 'integration EPay', $2, 1, $3, $4)
        RETURNING id
        "#,
    )
    .bind(&payment_uuid)
    .bind(serde_json::json!({
        "key": "epay-secret",
        "pid": "integration",
        "url": "https://pay.invalid"
    }))
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    let trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        r#"
        INSERT INTO orders (
            user_id, plan_id, payment_id, type, period, trade_no, total_amount,
            status, commission_status, commission_balance, created_at, updated_at
        ) VALUES ($1, 0, $2, 1, 'deposit', $3, 1234, 2, 0, 0, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(payment_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    // Exercise the real admin path: drop must archive rather than delete, hide
    // the version from ordinary reads, and retain it for delayed callbacks.
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
    admin.payment_delete(i64::from(payment_id)).await?;
    let payments = admin.payments_list().await?;
    ensure!(
        payments.iter().all(|row| row.id != payment_id),
        "archived payment remained visible in the ordinary admin list"
    );

    let mut signed = BTreeMap::from([
        ("money".to_string(), "12.34".to_string()),
        ("out_trade_no".to_string(), trade_no.clone()),
        ("trade_no".to_string(), "EPAY-LATE-INTEGRATION".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = signed
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    signed.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    signed.insert("sign_type".to_string(), "MD5".to_string());
    let input = PaymentNotifyInput {
        params: signed.into_iter().collect::<HashMap<_, _>>(),
        body: Vec::new(),
        headers: HashMap::new(),
    };

    let mut config = integration_config(pool, redis_url)?;
    config.database_url = database_url.to_string();
    let order = OrderService::new(pool.clone(), Arc::new(config));
    let mut callbacks = JoinSet::new();
    const CALLBACK_COUNT: usize = 8;
    for _ in 0..CALLBACK_COUNT {
        let order = order.clone();
        let input = input.clone();
        let payment_uuid = payment_uuid.clone();
        callbacks.spawn(async move {
            order
                .handle_payment_notify("EPay", &payment_uuid, input)
                .await
        });
    }
    let mut first_notices = 0;
    while let Some(joined) = callbacks.join_next().await {
        let response = joined??;
        ensure!(
            response.body == "success",
            "authenticated EPay callback was not acknowledged"
        );
        first_notices += usize::from(response.late_payment_notice.is_some());
        ensure!(
            response.paid_notice.is_none(),
            "cancelled order was incorrectly marked paid"
        );
    }
    ensure!(
        first_notices == 1,
        "late payment emitted {first_notices} first-seen notices"
    );
    let (rows, occurrences, order_status): (i64, i64, i16) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*),
            COALESCE(MAX(occurrence_count), 0)::BIGINT,
            (SELECT status FROM orders WHERE trade_no = $1)
        FROM payment_reconciliation
        WHERE payment_id = $2 AND callback_no = 'EPAY-LATE-INTEGRATION'
        "#,
    )
    .bind(&trade_no)
    .bind(payment_id)
    .fetch_one(pool)
    .await?;
    ensure!(
        rows == 1 && occurrences == CALLBACK_COUNT as i64 && order_status == 2,
        "late payment reconciliation was not one-row idempotent"
    );

    let mut mismatched = BTreeMap::from([
        ("money".to_string(), "1.00".to_string()),
        ("out_trade_no".to_string(), trade_no.clone()),
        ("trade_no".to_string(), "EPAY-AMOUNT-MISMATCH".to_string()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = mismatched
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    mismatched.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    mismatched.insert("sign_type".to_string(), "MD5".to_string());
    let response = order
        .handle_payment_notify(
            "EPay",
            &payment_uuid,
            PaymentNotifyInput {
                params: mismatched.into_iter().collect(),
                body: Vec::new(),
                headers: HashMap::new(),
            },
        )
        .await?;
    let notice = response
        .late_payment_notice
        .context("authenticated amount mismatch did not produce a reconciliation notice")?;
    ensure!(
        notice.reason == "settled_amount_mismatch" && response.paid_notice.is_none(),
        "authenticated amount mismatch was not classified safely"
    );
    let (reason, expected_amount, settled_amount): (String, i64, Option<i64>) = sqlx::query_as(
        r#"
        SELECT reason, expected_amount, settled_amount
        FROM payment_reconciliation
        WHERE payment_id = $1 AND callback_no = 'EPAY-AMOUNT-MISMATCH'
        "#,
    )
    .bind(payment_id)
    .fetch_one(pool)
    .await?;
    ensure!(
        reason == "settled_amount_mismatch"
            && expected_amount == 1234
            && settled_amount == Some(100),
        "authenticated amount mismatch was not durably recorded with exact amounts"
    );

    let missing_trade_no = format!("UNKNOWN-{}", "界".repeat(150));
    let unknown_callback_no = format!("EPAY-🚀{}", "X".repeat(400));
    let mut unknown_order = BTreeMap::from([
        ("money".to_string(), "2.00".to_string()),
        ("out_trade_no".to_string(), missing_trade_no.clone()),
        ("trade_no".to_string(), unknown_callback_no.clone()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = unknown_order
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    unknown_order.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    unknown_order.insert("sign_type".to_string(), "MD5".to_string());
    let response = order
        .handle_payment_notify(
            "EPay",
            &payment_uuid,
            PaymentNotifyInput {
                params: unknown_order.into_iter().collect(),
                body: Vec::new(),
                headers: HashMap::new(),
            },
        )
        .await?;
    ensure!(
        response.late_payment_notice.as_ref().is_some_and(|notice| {
            notice.reason == "order_not_found"
                && notice.trade_no.len() <= 255
                && notice.callback_no.len() <= 255
                && notice.trade_no_hash.len() == 64
                && notice.callback_no_hash.len() == 64
        }),
        "authenticated payment for an unknown order was not surfaced"
    );
    let (unknown_reason, stored_trade_no, stored_callback_no): (String, String, String) =
        sqlx::query_as(
            "SELECT reason, trade_no, callback_no FROM payment_reconciliation \
             WHERE payment_id = $1 AND callback_no_hash = $2",
        )
        .bind(payment_id)
        .bind(sha256_bytes(&unknown_callback_no))
        .fetch_one(pool)
        .await?;
    ensure!(
        unknown_reason == "order_not_found",
        "authenticated payment for an unknown order was not durable"
    );
    ensure!(
        stored_trade_no.len() <= 255 && stored_callback_no.len() <= 255,
        "oversized provider identifiers were not stored as bounded UTF-8 labels"
    );
    let trade_hash_matches: bool = sqlx::query_scalar(
        "SELECT trade_no_hash = $1 \
         FROM payment_reconciliation \
         WHERE payment_id = $2 AND callback_no_hash = $3",
    )
    .bind(sha256_bytes(&missing_trade_no))
    .bind(payment_id)
    .bind(sha256_bytes(&unknown_callback_no))
    .fetch_one(pool)
    .await?;
    ensure!(
        trade_hash_matches,
        "bounded reconciliation label did not retain the raw trade identity hash"
    );
    let archived_state: (i16, Option<i64>) =
        sqlx::query_as("SELECT enable, archived_at FROM payment_method WHERE id = $1")
            .bind(payment_id)
            .fetch_one(pool)
            .await?;
    ensure!(
        archived_state.0 == 0 && archived_state.1.is_some(),
        "delayed callbacks did not preserve the archived verification version"
    );
    let expected_callback_hash_hex = sha256_hex(&unknown_callback_no);
    let (list_rows, list_total) = admin
        .reconciliations_list(
            v2board_compat::Pagination {
                page: 1,
                per_page: 10,
            },
            None,
            None,
            None,
            Some(&missing_trade_no),
            None,
        )
        .await?;
    ensure!(
        list_total == 1
            && list_rows.first().is_some_and(|row| {
                row["reason"] == "order_not_found"
                    && row["callback_no_hash"].as_str() == Some(expected_callback_hash_hex.as_str())
            }),
        "unknown-order reconciliation was not discoverable through the global admin API"
    );

    let paid_trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        r#"
        INSERT INTO orders (
            user_id, plan_id, payment_id, type, period, trade_no, total_amount,
            status, commission_status, commission_balance, created_at, updated_at
        ) VALUES ($1, 0, $2, 1, 'deposit', $3, 200, 0, 0, 0, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(payment_id)
    .bind(&paid_trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let paid_callback_no = format!("EPAY-PAID-🚀{}", "Y".repeat(400));
    let mut paid_callback = BTreeMap::from([
        ("money".to_string(), "2.00".to_string()),
        ("out_trade_no".to_string(), paid_trade_no.clone()),
        ("trade_no".to_string(), paid_callback_no.clone()),
        ("trade_status".to_string(), "TRADE_SUCCESS".to_string()),
    ]);
    let canonical = paid_callback
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    paid_callback.insert(
        "sign".to_string(),
        format!("{:x}", md5::compute(format!("{canonical}epay-secret"))),
    );
    paid_callback.insert("sign_type".to_string(), "MD5".to_string());
    let paid_input = PaymentNotifyInput {
        params: paid_callback.into_iter().collect(),
        body: Vec::new(),
        headers: HashMap::new(),
    };
    let paid_response = order
        .handle_payment_notify("EPay", &payment_uuid, paid_input.clone())
        .await?;
    ensure!(
        paid_response.paid_notice.is_some() && paid_response.late_payment_notice.is_none(),
        "oversized authenticated callback did not complete the normal paid transition"
    );
    let (paid_status, callback_label, callback_label_bytes, callback_hash_matches): (
        i16,
        String,
        i32,
        bool,
    ) = sqlx::query_as(
        "SELECT status, callback_no, OCTET_LENGTH(callback_no), \
                    callback_no_hash = $1 \
             FROM orders WHERE trade_no = $2",
    )
    .bind(sha256_bytes(&paid_callback_no))
    .bind(&paid_trade_no)
    .fetch_one(pool)
    .await?;
    ensure!(
        matches!(paid_status, 1 | 3 | 4)
            && callback_label_bytes <= 255
            && callback_label.contains("\\u{1F680}")
            && !callback_label.contains('🚀')
            && callback_hash_matches,
        "normal settlement did not retain a bounded label and complete callback identity"
    );
    let replay = order
        .handle_payment_notify("EPay", &payment_uuid, paid_input)
        .await?;
    ensure!(
        replay.paid_notice.is_none() && replay.late_payment_notice.is_none(),
        "oversized callback hash did not make an exact provider replay idempotent"
    );
    let unexpected_reconciliation: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM payment_reconciliation \
         WHERE payment_id = $1 AND callback_no_hash = $2",
    )
    .bind(payment_id)
    .bind(sha256_bytes(&paid_callback_no))
    .fetch_one(pool)
    .await?;
    ensure!(
        unexpected_reconciliation == 0,
        "an ordinary oversized callback replay was misclassified for reconciliation"
    );

    Ok(())
}
