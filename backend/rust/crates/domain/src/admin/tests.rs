use super::commerce::{
    optional_nonnegative_i32, parse_payment_config, payment_config_input,
    reconciliation_resolution, reconciliation_resolved_filter, required_nonnegative_i32,
    resolve_redacted_payment_config,
};
use super::configuration::{bulk_mail_payload_hash, drop_unchanged_effective_secure_path};
use super::content::{TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT, validate_ticket_message_length};
use super::users::decimal_gib_filter_bytes;
use super::*;
use crate::mail::outbox::{mail_message_id, prepared_mail_payload_hash};

#[tokio::test]
async fn user_listing_mints_method_one_subscribe_urls_in_one_round_trip_per_row() {
    use crate::subscribe_link::test_support::MockRedis;
    use std::path::PathBuf;
    use v2board_config::RuntimePaths;

    let paths = RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    let mut config = AppConfig::try_from_api_config_map(Map::new(), paths).expect("test config");
    config.subscribe_url = Some("https://sub.example".to_string());
    config.show_subscribe_method = 1;
    let redis_keys = RedisKeyspace::new(Uuid::nil());

    let mut users = vec![
        json!({ "id": 1, "token": "token-one" }),
        json!({ "id": 2, "token": "token-two" }),
    ];
    // The mint script reuses the cached otp_ token when present, so the reply
    // is whatever Redis already holds for the row (Admin/UserController.php:103).
    let mut conn = Some(MockRedis::new([
        redis::Value::BulkString(b"minted-one".to_vec()),
        redis::Value::BulkString(b"minted-two".to_vec()),
    ]));
    super::users::attach_subscribe_urls(&config, &redis_keys, &mut conn, &mut users)
        .await
        .expect("attach subscribe urls");

    assert_eq!(
        users[0]["subscribe_url"],
        json!(config.subscribe_url_for_token("minted-one"))
    );
    assert_eq!(
        users[1]["subscribe_url"],
        json!(config.subscribe_url_for_token("minted-two"))
    );
    let conn = conn.expect("mock connection");
    // One shared connection, one mint round-trip per listed user.
    assert_eq!(conn.commands.len(), 2);
    assert!(
        conn.command_args(0)
            .contains(&redis_keys.key("otp_token-one"))
    );
    assert!(
        conn.command_args(1)
            .contains(&redis_keys.key("otp_token-two"))
    );

    // Method 0 stays byte-identical to the raw-token URL and needs no Redis.
    config.show_subscribe_method = 0;
    let mut raw_users = vec![json!({ "id": 1, "token": "token-one" })];
    let mut no_conn: Option<MockRedis> = None;
    super::users::attach_subscribe_urls(&config, &redis_keys, &mut no_conn, &mut raw_users)
        .await
        .expect("method 0 attach");
    assert_eq!(
        raw_users[0]["subscribe_url"],
        json!(config.subscribe_url_for_token("token-one"))
    );
}

#[test]
fn admin_user_exports_route_every_subscribe_url_through_the_shared_minter() {
    // The listing, generate CSV, and dump CSV surfaces must all mint through
    // subscribe_link so show_subscribe_method 1/2 never leak permanent tokens.
    let source = include_str!("users.rs");
    assert!(!source.contains("subscribe_url_for_token"));
    assert_eq!(
        source
            .matches("crate::subscribe_link::subscribe_url_for_user")
            .count(),
        3
    );
    // Methods 0/2 must not acquire a Redis connection for exports.
    assert!(source.contains("show_subscribe_method != 1"));
}

#[test]
fn telegram_webhook_secret_is_stable_scoped_and_header_safe() {
    let first = telegram_webhook_secret("app-key", "123:bot-token");
    assert_eq!(first, telegram_webhook_secret("app-key", "123:bot-token"));
    assert_ne!(first, telegram_webhook_secret("other-key", "123:bot-token"));
    assert_ne!(first, telegram_webhook_secret("app-key", "456:bot-token"));
    assert_eq!(first.len(), 64);
    assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect()
}

fn validation_parts(error: ApiError) -> (String, indexmap::IndexMap<String, Vec<String>>) {
    match error {
        ApiError::Validation { message, errors } => (message, errors),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn route_save_accepts_valid_payload() {
    let ok = params(&[
        ("remarks", "cn"),
        ("action", "block"),
        ("match", r#"["1.1.1.1"]"#),
    ]);
    assert!(route_save_validation(&ok).is_none());
}

#[test]
fn admin_ticket_reply_respects_mysql_text_limit() {
    assert!(validate_ticket_message_length(&"a".repeat(65_535)).is_ok());
    assert!(validate_ticket_message_length(&"a".repeat(65_536)).is_err());
}

#[test]
fn every_payment_row_is_an_immutable_verification_version() {
    assert!(payment_verification_version_blocks_update(true, false));
    assert!(payment_verification_version_blocks_update(false, true));
    assert!(!payment_verification_version_blocks_update(false, false));
}

#[test]
fn redacted_payment_secret_preserves_only_an_existing_same_provider_value() {
    let submitted = json!({
        "currency": "usd",
        "stripe_sk_live": crate::payment_provider::REDACTED_PAYMENT_SECRET,
        "stripe_pk_live": "pk_new",
        "stripe_webhook_key": crate::payment_provider::REDACTED_PAYMENT_SECRET,
    });
    let current = r#"{"currency":"eur","stripe_sk_live":"sk_existing","stripe_pk_live":"pk_old","stripe_webhook_key":"whsec_existing"}"#;
    let resolved = resolve_redacted_payment_config(
        "StripeCheckout",
        Some(("StripeCheckout", current)),
        submitted.clone(),
    )
    .unwrap();
    assert_eq!(resolved["stripe_sk_live"], "sk_existing");
    assert_eq!(resolved["stripe_webhook_key"], "whsec_existing");
    assert_eq!(resolved["stripe_pk_live"], "pk_new");
    assert!(resolve_redacted_payment_config("StripeCheckout", None, submitted).is_err());
}

#[test]
fn built_in_payment_text_fields_never_coerce_numeric_or_boolean_looking_values() {
    let input = params(&[
        ("config[pid]", "00123"),
        ("config[key]", "true"),
        ("config[type]", "null"),
    ]);
    let config = payment_config_input(&input, "EPay");
    assert_eq!(config["pid"], "00123");
    assert_eq!(config["key"], "true");
    assert_eq!(config["type"], "null");
}

#[test]
fn redacted_known_config_round_trip_preserves_exact_legacy_json_types() {
    let current = r#"{"currency":"usd","stripe_sk_live":"secret","stripe_pk_live":{"malformed":"private"},"stripe_webhook_key":null}"#;
    let submitted = json!({
        "currency": "usd",
        "stripe_sk_live": crate::payment_provider::REDACTED_PAYMENT_SECRET,
        "stripe_pk_live": crate::payment_provider::REDACTED_PAYMENT_SECRET,
        "stripe_webhook_key": "",
    });
    let resolved = resolve_redacted_payment_config(
        "StripeCheckout",
        Some(("StripeCheckout", current)),
        submitted,
    )
    .unwrap();
    assert_eq!(resolved, serde_json::from_str::<Value>(current).unwrap());
}

#[test]
fn absent_optional_payment_fields_stay_absent_on_metadata_round_trip() {
    let current = r#"{"currency":"usd","stripe_sk_live":"secret","stripe_pk_live":"public","stripe_webhook_key":"whsec"}"#;
    let submitted = json!({
        "currency": "usd",
        "stripe_sk_live": crate::payment_provider::REDACTED_PAYMENT_SECRET,
        "stripe_pk_live": "public",
        "stripe_webhook_key": crate::payment_provider::REDACTED_PAYMENT_SECRET,
        "stripe_custom_field_name": "",
    });
    let resolved = resolve_redacted_payment_config(
        "StripeCheckout",
        Some(("StripeCheckout", current)),
        submitted,
    )
    .unwrap();
    assert_eq!(resolved, serde_json::from_str::<Value>(current).unwrap());
}

#[test]
fn unknown_payment_provider_redaction_round_trip_preserves_hidden_values() {
    let current = r#"{"token":"secret","nested":{"private":true}}"#;
    let resolved = resolve_redacted_payment_config(
        "ExternalLegacy",
        Some(("ExternalLegacy", current)),
        json!({
            "token": crate::payment_provider::REDACTED_PAYMENT_SECRET,
            "nested": crate::payment_provider::REDACTED_PAYMENT_SECRET,
        }),
    )
    .unwrap();
    assert_eq!(resolved, serde_json::from_str::<Value>(current).unwrap());
    assert!(
        resolve_redacted_payment_config(
            "ExternalLegacy",
            None,
            json!({ "token": crate::payment_provider::REDACTED_PAYMENT_SECRET }),
        )
        .is_err()
    );
}

#[test]
fn reconciliation_list_defaults_open_and_validates_resolution_filter() {
    assert_eq!(reconciliation_resolved_filter(&HashMap::new()).unwrap(), 0);
    assert_eq!(
        reconciliation_resolved_filter(&params(&[("resolved", "resolved")])).unwrap(),
        1
    );
    assert_eq!(
        reconciliation_resolved_filter(&params(&[("resolved", "all")])).unwrap(),
        2
    );
    assert!(reconciliation_resolved_filter(&params(&[("resolved", "maybe")])).is_err());
}

#[test]
fn payment_reconciliation_resolution_is_structured_bounded_and_actor_scoped() {
    let encoded = reconciliation_resolution("admin@example.test", "refunded by provider").unwrap();
    let decoded: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded["actor"], "admin@example.test");
    assert_eq!(decoded["note"], "refunded by provider");
    assert!(reconciliation_resolution("admin@example.test", &"x".repeat(161)).is_err());
}

#[test]
fn traffic_filters_scale_decimal_gib_without_binary_float_drift() {
    assert_eq!(decimal_gib_filter_bytes("1.5").unwrap(), 1_610_612_736);
    assert_eq!(decimal_gib_filter_bytes("0.000000001").unwrap(), 1);
    assert!(decimal_gib_filter_bytes("not-a-number").is_err());
    assert!(decimal_gib_filter_bytes("1e100").is_err());
}

#[test]
fn plan_amount_fields_reject_negative_invalid_and_database_overflow_values() {
    let valid = params(&[("transfer_enable", "100"), ("month_price", "1999")]);
    assert_eq!(
        required_nonnegative_i32(&valid, "transfer_enable").unwrap(),
        100
    );
    assert_eq!(
        optional_nonnegative_i32(&valid, "month_price").unwrap(),
        Some(1999)
    );

    for value in ["-1", "2147483648", "not-an-integer"] {
        let input = params(&[("month_price", value)]);
        assert!(optional_nonnegative_i32(&input, "month_price").is_err());
    }
}

#[test]
fn bulk_mail_identity_is_actor_scoped_and_payload_canonical() {
    let first = mail_batch_key("admin:one@example.test", "retry-key");
    assert_eq!(first.len(), 64);
    assert_eq!(first, mail_batch_key("admin:one@example.test", "retry-key"));
    assert_ne!(first, mail_batch_key("staff:one@example.test", "retry-key"));

    let first_params = params(&[
        ("subject", "Notice"),
        ("content", "Body"),
        ("filter[0][key]", "email"),
        ("_idempotency_key", "first"),
    ]);
    let reordered = params(&[
        ("_idempotency_key", "second"),
        ("filter[0][key]", "email"),
        ("content", "Body"),
        ("subject", "Notice"),
    ]);
    assert_eq!(
        bulk_mail_payload_hash(&first_params),
        bulk_mail_payload_hash(&reordered)
    );
}

#[test]
fn bulk_mail_message_ids_are_stable_per_recipient() {
    let batch_key = mail_batch_key("admin:one@example.test", "retry-key");
    let message_id = mail_message_id(&batch_key, "USER@example.test");
    assert_eq!(message_id, mail_message_id(&batch_key, "user@example.test"));
    assert_ne!(
        message_id,
        mail_message_id(&batch_key, "other@example.test")
    );
    assert!(message_id.starts_with('<'));
    assert!(message_id.ends_with("@mail.v2board.local>"));
}

#[test]
fn prepared_mail_payload_identity_covers_envelope_and_recipient() {
    let envelope = PreparedMailEnvelope {
        sender: "Site <sender@example.test>".to_string(),
        template_name: "mail.default.notify".to_string(),
        subject: "Subject".to_string(),
        body: "Body".to_string(),
    };
    let first = prepared_mail_payload_hash(&envelope, &["one@example.test".to_string()]);
    assert_eq!(
        first,
        prepared_mail_payload_hash(&envelope, &["one@example.test".to_string()])
    );
    assert_ne!(
        first,
        prepared_mail_payload_hash(&envelope, &["two@example.test".to_string()])
    );
}

#[test]
fn admin_smtp_probe_has_an_end_to_end_deadline() {
    let source = include_str!("configuration.rs");
    let start = source.find("async fn send_test_mail").unwrap();
    let probe = &source[start..];
    assert!(probe.contains("tokio::time::timeout"));
    assert!(probe.contains("http_request_timeout_seconds"));
}

#[test]
fn admin_config_is_typed_and_security_validated_before_revision_commit() {
    let source = include_str!("configuration.rs");
    let security = source.find("validate_security_update").unwrap();
    let typed = source.find("with_operator_config").unwrap();
    let commit = source.find("operator_config::commit").unwrap();
    assert!(security < commit);
    assert!(typed < commit);
    assert!(!source.contains("update_config_atomic"));
}

#[test]
fn config_candidate_validation_rejects_lossy_or_unsafe_scalars() {
    for (key, invalid) in [
        ("email_port", "smtp"),
        ("email_port", "0"),
        ("email_port", "65536"),
        ("try_out_hour", "NaN"),
        ("commission_withdraw_limit", "inf"),
        ("server_push_interval", "-1"),
        ("server_node_report_min_traffic", "-1"),
        ("register_limit_count", "-1"),
    ] {
        assert!(
            validate_config_params(&params(&[(key, invalid)])).is_err(),
            "{key}={invalid} must fail before commit"
        );
    }
}

#[test]
fn config_save_rejects_an_explicit_empty_secure_path_but_drops_an_unchanged_fallback() {
    for empty in ["", " ", "\t\n"] {
        let error = validate_config_params(&params(&[("secure_path", empty)]))
            .expect_err("an explicit empty admin path must be a 422 validation error");
        let (message, errors) = validation_parts(error);
        assert_eq!(message, "后台路径不能为空");
        assert_eq!(errors["secure_path"], vec!["后台路径不能为空"]);
    }

    let mut unchanged_fallback = params(&[("secure_path", "/old123/")]);
    drop_unchanged_effective_secure_path(&mut unchanged_fallback, "old123");
    assert!(!unchanged_fallback.contains_key("secure_path"));
    validate_config_params(&unchanged_fallback)
        .expect("an unchanged effective fallback remains a no-op");

    let mut empty = params(&[("secure_path", "")]);
    drop_unchanged_effective_secure_path(&mut empty, "old123");
    assert!(empty.contains_key("secure_path"));
}

#[test]
fn config_merge_preserves_exact_decimals_and_only_declared_empty_lists() {
    let exact = "0.1234567890123456789012345678";
    let input = params(&[
        ("try_out_hour", exact),
        ("commission_withdraw_limit", "10.05"),
        ("commission_distribution_l1", exact),
        ("deposit_bounus", "[]"),
        ("commission_withdraw_method", "[]"),
        ("email_whitelist_suffix", "[]"),
        ("app_name", "[]"),
        ("email_password", "00001234"),
    ]);
    validate_config_params(&input).expect("exact decimal candidate");
    let mut merged = Map::new();
    merge_config_params(&mut merged, &input);
    assert_eq!(merged["try_out_hour"], Value::String(exact.to_string()));
    assert_eq!(
        merged["commission_distribution_l1"],
        Value::String(exact.to_string())
    );
    assert_eq!(
        merged["email_password"],
        Value::String("00001234".to_string())
    );
    for key in [
        "deposit_bounus",
        "commission_withdraw_method",
        "email_whitelist_suffix",
    ] {
        assert_eq!(merged[key], Value::Array(Vec::new()));
    }
    assert_eq!(merged["app_name"], Value::String("[]".to_string()));
}

#[test]
fn config_arrays_reject_bare_scalars_and_normalize_indexed_items() {
    for key in [
        "deposit_bounus",
        "commission_withdraw_method",
        "email_whitelist_suffix",
    ] {
        assert!(validate_config_params(&params(&[(key, "one,two")])).is_err());
        assert!(validate_config_params(&params(&[(key, "[]")])).is_ok());
    }

    let input = params(&[
        ("commission_withdraw_method[0]", "  Alipay  "),
        ("commission_withdraw_method[1]", "   "),
        ("commission_withdraw_method[2]", "USDT"),
    ]);
    validate_config_params(&input).expect("indexed array");
    let mut merged = Map::new();
    merge_config_params(&mut merged, &input);
    assert_eq!(
        merged["commission_withdraw_method"],
        serde_json::json!(["Alipay", "USDT"])
    );

    assert!(
        validate_config_params(&params(&[
            ("email_whitelist_suffix", "[]"),
            ("email_whitelist_suffix[0]", "example.com"),
        ]))
        .is_err()
    );
}

#[test]
fn clearing_email_port_normalizes_only_that_optional_scalar_to_null() {
    let mut merged = serde_json::json!({
        "email_port": 587,
        "email_host": "smtp.example.com"
    })
    .as_object()
    .expect("object")
    .clone();
    merge_config_params(
        &mut merged,
        &params(&[("email_port", ""), ("email_host", "")]),
    );
    assert_eq!(merged["email_port"], Value::Null);
    assert_eq!(merged["email_host"], Value::String(String::new()));
}

#[test]
fn ticket_reply_notification_is_enqueued_inside_the_reply_transaction() {
    let source = include_str!("content.rs");
    let start = source
        .find("pub(super) async fn ticket_reply")
        .expect("ticket reply implementation");
    let end = source[start..]
        .find("pub(super) async fn ticket_close")
        .map(|offset| start + offset)
        .expect("ticket close implementation");
    let reply = &source[start..end];
    assert!(reply.contains("self.db.begin()"));
    assert!(reply.contains("enqueue_prepared_mail"));
    assert!(reply.contains("tx.commit()"));
    assert!(!reply.contains("send_mail("));
    assert!(reply.contains("ticket reply notification envelope invalid"));
    assert!(reply.contains("ticket reply notification recipient invalid"));
    let prepared = reply.find("prepare_ticket_reply_notification").unwrap();
    let gate = reply.find("reserve_ticket_notification_gate").unwrap();
    let transaction = reply.find("self.db.begin()").unwrap();
    assert!(prepared < gate && gate < transaction);
    assert!(reply.contains("release_ticket_notification_gate"));
}

#[test]
fn ticket_notification_gate_is_atomic_and_owner_scoped_on_rollback() {
    let source = include_str!("content.rs");
    assert!(source.contains("redis::cmd(\"SET\")"));
    assert!(source.contains(".arg(\"NX\")"));
    assert!(source.contains(".arg(\"EX\")"));
    assert!(!source.contains("conn.exists::<_, bool>(&cache_key)"));
    assert!(TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT.contains("GET"));
    assert!(TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT.contains("ARGV[1]"));
    assert!(TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT.contains("DEL"));
}

#[test]
fn stored_payment_config_requires_a_valid_json_object() {
    let object = parse_payment_config(r#"{"api_key":"secret","nested":{"enabled":true}}"#)
        .expect("valid object config");
    assert_eq!(object["api_key"], json!("secret"));
    assert_eq!(object["nested"]["enabled"], json!(true));

    assert!(matches!(
        parse_payment_config("not-json"),
        Err(ApiError::Internal(_))
    ));
    assert!(matches!(
        parse_payment_config(r#"["not","an","object"]"#),
        Err(ApiError::Internal(_))
    ));
}

#[test]
fn route_save_default_out_needs_no_match() {
    // required_unless:action,default_out: match is optional once action is default_out.
    let ok = params(&[("remarks", "fallback"), ("action", "default_out")]);
    assert!(route_save_validation(&ok).is_none());
}

#[test]
fn route_save_nonempty_match_array_passes_required_even_if_falsy() {
    // Laravel `required` counts a non-empty array; array_filter drops "0" later.
    let ok = params(&[
        ("remarks", "cn"),
        ("action", "block"),
        ("match", r#"["0"]"#),
    ]);
    assert!(route_save_validation(&ok).is_none());
}

#[test]
fn route_save_missing_remarks_reports_first() {
    let error = route_save_validation(&params(&[("action", "block"), ("match", r#"["1.1.1.1"]"#)]))
        .expect("missing remarks must fail");
    let (message, errors) = validation_parts(error);
    assert_eq!(message, "备注不能为空");
    assert_eq!(errors["remarks"], vec!["备注不能为空".to_string()]);
}

#[test]
fn route_save_missing_match_reports_required_unless() {
    let error = route_save_validation(&params(&[("remarks", "cn"), ("action", "block")]))
        .expect("missing match must fail");
    let (message, errors) = validation_parts(error);
    assert_eq!(message, "匹配值不能为空");
    assert_eq!(errors["match"], vec!["匹配值不能为空".to_string()]);
}

#[test]
fn route_save_missing_action_reports_required() {
    let error = route_save_validation(&params(&[("remarks", "cn"), ("match", r#"["1.1.1.1"]"#)]))
        .expect("missing action must fail");
    let (message, _) = validation_parts(error);
    assert_eq!(message, "动作类型不能为空");
}

#[test]
fn route_save_invalid_action_reports_in_rule() {
    let error = route_save_validation(&params(&[
        ("remarks", "cn"),
        ("action", "teleport"),
        ("match", r#"["1.1.1.1"]"#),
    ]))
    .expect("invalid action must fail");
    let (message, errors) = validation_parts(error);
    assert_eq!(message, "动作类型参数有误");
    assert_eq!(errors["action"], vec!["动作类型参数有误".to_string()]);
}

#[test]
fn route_save_empty_payload_reports_first_field() {
    // Every field fails; Laravel's field order makes remarks the reported message,
    // and the codebase's single-field 422 shape keys only that first failure.
    let error = route_save_validation(&params(&[])).expect("empty payload must fail");
    let (message, errors) = validation_parts(error);
    assert_eq!(message, "备注不能为空");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors["remarks"], vec!["备注不能为空".to_string()]);
}
