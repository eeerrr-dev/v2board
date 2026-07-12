use super::commerce::{
    PENDING_PAYMENT_ORDER_SQL, optional_nonnegative_i32, parse_payment_config,
    required_nonnegative_i32,
};
use super::configuration::bulk_mail_payload_hash;
use super::content::TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT;
use super::users::decimal_gib_filter_bytes;
use super::*;
use crate::mail::outbox::{mail_message_id, prepared_mail_payload_hash};

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

fn validation_parts(error: ApiError) -> (String, HashMap<String, Vec<String>>) {
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
fn pending_payment_order_blocks_driver_or_config_changes_only() {
    assert!(pending_order_blocks_payment_update(true, true, false));
    assert!(pending_order_blocks_payment_update(true, false, true));
    assert!(!pending_order_blocks_payment_update(true, false, false));
    assert!(!pending_order_blocks_payment_update(false, true, true));
}

#[test]
fn pending_payment_query_covers_every_gateway_binding() {
    assert!(PENDING_PAYMENT_ORDER_SQL.contains("payment_id = ? AND status = 0"));
    assert!(!PENDING_PAYMENT_ORDER_SQL.contains("callback_no"));
    assert!(!PENDING_PAYMENT_ORDER_SQL.contains("Stripe"));
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
