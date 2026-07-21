use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Result, ensure};
use chrono::Utc;
use sqlx::PgPool;
use tokio::task::JoinSet;
use uuid::Uuid;
use v2board_application::{
    order::PaymentNotifyInput,
    payment::{PaymentCreateInput, PaymentService},
    reconciliation::ReconciliationService,
};
use v2board_db::{
    admin_payment::PostgresPaymentRepository, reconciliation::PostgresReconciliationRepository,
};
use v2board_order_adapters::runtime_order_service;
use v2board_payment_adapters::{EncryptedPaymentSecurity, Sha256ReconciliationIdentityHasher};

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
    .bind(super::harness::encrypt_payment_fixture_config(
        "EPay",
        &payment_uuid,
        &serde_json::json!({
            "key": "epay-secret",
            "pid": "integration",
            "url": "https://pay.invalid"
        }),
    )?)
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
    let payment_config = integration_config(pool, redis_url)?;
    let payments = PaymentService::new(
        PostgresPaymentRepository::new(pool.clone()),
        EncryptedPaymentSecurity::new(payment_config.app_key.clone()),
        payment_config.app_url.clone(),
    );
    let reconciliations = ReconciliationService::new(
        PostgresReconciliationRepository::new(pool.clone()),
        Sha256ReconciliationIdentityHasher,
    );
    payments.archive(i64::from(payment_id), now).await?;
    let active_payments = payments.payments().await?;
    ensure!(
        active_payments.iter().all(|row| row.id != payment_id),
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
        params: signed,
        body: Vec::new(),
        headers: BTreeMap::new(),
    };

    let mut config = integration_config(pool, redis_url)?;
    config.database_url = database_url.to_string();
    let order = runtime_order_service(pool.clone(), Arc::new(config));
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
                headers: BTreeMap::new(),
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
                headers: BTreeMap::new(),
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
    let list_page = reconciliations
        .reconciliations(10, 0, None, None, None, Some(&missing_trade_no), None)
        .await?;
    ensure!(
        list_page.total == 1
            && list_page.items.first().is_some_and(|row| {
                row.reason == "order_not_found"
                    && row.callback_no_hash == expected_callback_hash_hex
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
        headers: BTreeMap::new(),
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

/// After the real admin create path runs, the `payment_method.config` column
/// must hold only the AES-256-GCM envelope: no submitted secret may appear in
/// the raw column text, the envelope must decrypt (bound to the row's driver
/// and uuid) back to exactly the submitted config, and the admin wire must
/// keep returning the redacted PLAINTEXT shape rather than the envelope.
pub(super) async fn payment_config_at_rest_opacity(pool: &PgPool, redis_url: &str) -> Result<()> {
    let mut config = integration_config(pool, redis_url)?;
    config.app_url = Some("https://opacity.integration.invalid".to_string());
    let payments = PaymentService::new(
        PostgresPaymentRepository::new(pool.clone()),
        EncryptedPaymentSecurity::new(config.app_key.clone()),
        config.app_url.clone(),
    );
    let submitted = BTreeMap::from([
        ("url".to_string(), "https://opacity.pay.invalid".to_string()),
        ("pid".to_string(), "opacity-epay-pid-4242".to_string()),
        (
            "key".to_string(),
            "opacity-epay-secret-key-material".to_string(),
        ),
    ]);
    let now = Utc::now().timestamp();
    let payment_id = payments
        .create(
            PaymentCreateInput {
                name: "at-rest opacity".to_string(),
                provider: "EPay".to_string(),
                config: submitted.clone(),
                icon: None,
                notify_domain: None,
                handling_fee_fixed: None,
                handling_fee_percent: None,
            },
            now,
        )
        .await?;

    let (raw_config, payment_uuid): (String, String) =
        sqlx::query_as("SELECT CAST(config AS TEXT), uuid FROM payment_method WHERE id = $1")
            .bind(payment_id)
            .fetch_one(pool)
            .await?;
    for secret in submitted.values() {
        ensure!(
            !raw_config.contains(secret),
            "submitted payment secret {secret:?} appeared in the raw stored config column"
        );
    }
    let envelope: serde_json::Value = serde_json::from_str(&raw_config)
        .context("stored payment config column is not valid JSON")?;
    let envelope = envelope
        .as_object()
        .context("stored payment config column is not a JSON object")?;
    ensure!(
        envelope.len() == 4
            && ["format_version", "nonce", "ciphertext", "tag"]
                .iter()
                .all(|key| envelope.contains_key(*key))
            && envelope
                .get("format_version")
                .and_then(serde_json::Value::as_i64)
                == Some(1),
        "stored payment config is not the version-1 encrypted envelope shape"
    );
    let decrypted = v2board_payment_adapters::payment_secrets::decrypt_payment_config(
        super::harness::INTEGRATION_APP_KEY,
        "EPay",
        &payment_uuid,
        &raw_config,
    )
    .context("stored payment envelope did not decrypt with the driver/uuid binding")?;
    ensure!(
        decrypted
            == serde_json::Value::Object(
                submitted
                    .iter()
                    .map(|(key, value)| { (key.clone(), serde_json::Value::String(value.clone())) })
                    .collect(),
            ),
        "decrypted payment envelope does not round-trip the submitted config"
    );

    let listed = payments
        .payments()
        .await?
        .into_iter()
        .find(|row| row.id == payment_id)
        .context("created payment method is missing from the admin list")?;
    ensure!(
        listed.config.get("key").map(String::as_str) == Some("********")
            && listed.config.get("url").map(String::as_str) == Some("https://opacity.pay.invalid")
            && !listed.config.contains_key("ciphertext"),
        "admin payment list stopped returning the redacted plaintext config shape"
    );
    payments.archive(i64::from(payment_id), now + 1).await?;
    Ok(())
}
