use super::*;

#[test]
fn random_char_has_requested_length_and_charset() {
    let value = random_char(16);
    assert_eq!(value.chars().count(), 16);
    assert!(value.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn gib_scaling_rejects_negative_and_overflowing_allowances() {
    assert_eq!(checked_gib_bytes(2, "transfer_enable").unwrap(), 2 * GIB);
    assert!(checked_gib_bytes(-1, "transfer_enable").is_err());
    assert!(checked_gib_bytes(i64::MAX, "transfer_enable").is_err());
}

#[test]
fn group_id_contains_matches_numeric_and_string_members() {
    assert!(group_id_contains("[1, 2, 3]", 2));
    assert!(group_id_contains("[\"1\", \"2\"]", 1));
    assert!(!group_id_contains("[1, 2]", 3));
    assert!(!group_id_contains("not json", 1));
    assert!(!group_id_contains("{}", 1));
}

#[test]
fn node_available_status_reports_three_states() {
    let now = 10_000;
    // last_check older than 5 min -> offline (0)
    assert_eq!(node_available_status(now, Some(now - 400), Some(now)), 0);
    // check fresh, push stale -> degraded (1)
    assert_eq!(node_available_status(now, Some(now), Some(now - 400)), 1);
    // both fresh -> online (2)
    assert_eq!(node_available_status(now, Some(now), Some(now)), 2);
    // missing cache values default to 0 -> offline
    assert_eq!(node_available_status(now, None, None), 0);
    // Extreme clocks must preserve the stale comparison without overflowing.
    assert_eq!(
        node_available_status(i64::MIN, Some(i64::MIN), Some(i64::MIN)),
        0
    );
    assert_eq!(
        node_available_status(i64::MAX, Some(i64::MIN), Some(i64::MIN)),
        0
    );
}

#[test]
fn normalize_stat_server_type_maps_legacy_v2ray() {
    assert_eq!(normalize_stat_server_type("v2ray"), "vmess");
    assert_eq!(normalize_stat_server_type("shadowsocks"), "shadowsocks");
}

#[test]
fn parse_alive_ip_extracts_count_and_ip_labels() {
    let raw = json!({
        "alive_ip": 2,
        "7": { "aliveips": ["1.2.3.4_ded", "5.6.7.8_abc"] }
    })
    .to_string();
    let (alive_ip, ips) = parse_alive_ip(&raw);
    assert_eq!(alive_ip, 2);
    assert_eq!(ips, "1.2.3.4_7, 5.6.7.8_7");
}

/// Asserts the error is a 422 validation failure on `field` with `message`
/// (which is also the top-level message), mirroring a single-rule FormRequest.
fn assert_validation(result: Result<(), ApiError>, field: &str, message: &str) {
    match result {
        Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed => {
            assert_eq!(problem.detail(), message, "detail");
            assert_eq!(
                problem
                    .errors()
                    .and_then(|errors| errors.get(field))
                    .map(Vec::as_slice),
                Some([message.to_string()].as_slice()),
                "errors[{field}]"
            );
        }
        other => panic!("expected 422 validation on {field}, got {other:?}"),
    }
}

#[test]
fn config_duration_minutes_reject_zero_negative_and_overflowing_values() {
    for field in [
        "show_subscribe_expire",
        "register_limit_expire",
        "password_limit_expire",
    ] {
        for value in [0_i64, -1, i64::MAX] {
            assert_validation(
                validate_config_json(&Map::from_iter([(field.to_string(), json!(value))])),
                field,
                "分钟数必须在安全范围内",
            );
        }
        assert!(
            validate_config_json(&Map::from_iter([(field.to_string(), json!(525_600))])).is_ok()
        );
    }
}

#[test]
fn csv_export_quotes_structural_characters_and_neutralizes_formulas() {
    let body = csv_export(
        &["email", "note", "value"],
        [vec![
            "user@example.com".to_string(),
            "comma, quote \" and\nnewline".to_string(),
            "=HYPERLINK(\"https://evil.example\")".to_string(),
        ]],
        false,
    )
    .expect("CSV export");

    let mut reader = csv::ReaderBuilder::new().from_reader(body.as_bytes());
    let record = reader.records().next().expect("row").expect("valid row");
    assert_eq!(&record[1], "comma, quote \" and\nnewline");
    assert_eq!(&record[2], "'=HYPERLINK(\"https://evil.example\")");
}

#[test]
fn payment_webhook_uuid_uses_the_full_uuid_entropy() {
    let uuid = random_payment_uuid();
    assert_eq!(uuid.len(), 32);
    assert!(uuid.bytes().all(|byte| byte.is_ascii_hexdigit()));
}
