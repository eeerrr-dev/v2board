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
fn user_column_and_operator_reject_unknown_input() {
    assert_eq!(user_column("email"), Some("email"));
    assert_eq!(user_column("id"), Some("id"));
    assert_eq!(user_column("password"), None);
    assert_eq!(user_column("email); DROP TABLE"), None);
    assert_eq!(user_filter_operator("="), Some("="));
    assert_eq!(user_filter_operator("like"), Some("like"));
    assert_eq!(user_filter_operator("!="), Some("<>"));
    // 模糊 is rewritten to `like` before reaching the operator whitelist.
    assert_eq!(user_filter_operator("模糊"), None);
    assert_eq!(user_filter_operator("; DELETE"), None);
}

#[test]
fn user_sort_whitelists_expression_and_direction() {
    let mut params = HashMap::new();
    assert_eq!(user_sort(&params), ("u.created_at".to_string(), "DESC"));

    params.insert("sort".to_string(), "total_used".to_string());
    params.insert("sort_type".to_string(), "ASC".to_string());
    assert_eq!(
        user_sort(&params),
        (
            "(CAST(u.u AS DECIMAL(65,0)) + CAST(u.d AS DECIMAL(65,0)))".to_string(),
            "ASC"
        )
    );

    params.insert("sort".to_string(), "email".to_string());
    assert_eq!(user_sort(&params), ("u.email".to_string(), "ASC"));

    params.insert("sort".to_string(), "bogus".to_string());
    params.insert("sort_type".to_string(), "sideways".to_string());
    assert_eq!(user_sort(&params), ("u.created_at".to_string(), "DESC"));
}

#[test]
fn user_total_used_sql_widens_before_adding_bigint_counters() {
    let users_source = include_str!("../users.rs");
    let widened = "CAST(u.u AS DECIMAL(65,0)) + CAST(u.d AS DECIMAL(65,0))";
    assert_eq!(users_source.matches(widened).count(), 3);
    assert!(!users_source.contains("'total_used', u.u + u.d"));

    let max_total = i128::from(i64::MAX) + i128::from(i64::MAX);
    assert_eq!(max_total, 18_446_744_073_709_551_614_i128);
}

#[test]
fn collect_filter_entries_groups_by_index_and_keeps_raw_null() {
    let mut params = HashMap::new();
    params.insert("filter[0][key]".to_string(), "plan_id".to_string());
    params.insert("filter[0][condition]".to_string(), "=".to_string());
    params.insert("filter[0][value]".to_string(), "null".to_string());
    params.insert("filter[1][key]".to_string(), "email".to_string());
    let entries = collect_filter_entries(&params);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get("key").map(String::as_str), Some("plan_id"));
    // Raw "null" must survive so plan_id == 'null' -> IS NULL still fires.
    assert_eq!(entries[0].get("value").map(String::as_str), Some("null"));
    assert_eq!(entries[1].get("key").map(String::as_str), Some("email"));
}

#[test]
fn joined_array_display_joins_or_defaults() {
    let mut params = HashMap::new();
    assert_eq!(joined_array_display(&params, "limit_plan_ids"), "不限制");
    params.insert("limit_plan_ids[0]".to_string(), "1".to_string());
    params.insert("limit_plan_ids[1]".to_string(), "3".to_string());
    assert_eq!(joined_array_display(&params, "limit_plan_ids"), "1/3");
}

#[test]
fn notice_tags_preserve_bracketed_strings_as_json() {
    let values = params(&[("tags[1]", "运营"), ("tags[0]", "弹窗")]);
    assert_eq!(
        json_array_string(&values, "tags").unwrap().as_deref(),
        Some(r#"["弹窗","运营"]"#)
    );
    assert_eq!(json_array_string(&HashMap::new(), "tags").unwrap(), None);
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

fn params(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
fn pagination_has_a_hard_cap_and_checked_offset() {
    let defaults = page(&HashMap::new()).unwrap();
    assert_eq!(defaults.limit, 10);
    assert_eq!(defaults.offset, 0);

    let second = page(&params(&[("current", "2"), ("pageSize", "25")])).unwrap();
    assert_eq!(second.limit, 25);
    assert_eq!(second.offset, 25);

    assert!(page(&params(&[("current", "0")])).is_err());
    assert!(page(&params(&[("page_size", "101")])).is_err());
    assert!(page(&params(&[("current", "not-an-integer")])).is_err());
    assert!(
        page(&params(&[
            ("current", "9223372036854775807"),
            ("pageSize", "100"),
        ]))
        .is_err()
    );
}

/// Asserts the error is a 422 validation failure on `field` with `message`
/// (which is also the top-level message), mirroring a single-rule FormRequest.
fn assert_validation(result: Result<(), ApiError>, field: &str, message: &str) {
    match result {
        Err(ApiError::Validation {
            message: top,
            errors,
        }) => {
            assert_eq!(top, message, "top-level message");
            assert_eq!(
                errors.get(field).map(Vec::as_slice),
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
        for value in ["0", "-1", "9223372036854775807"] {
            assert_validation(
                validate_config_params(&params(&[(field, value)])),
                field,
                "分钟数必须在安全范围内",
            );
        }
        assert!(validate_config_params(&params(&[(field, "525600")])).is_ok());
    }
}

// A complete, valid coupon/giftcard payload used as the baseline that each
// test perturbs one field at a time.
fn valid_coupon() -> Vec<(&'static str, &'static str)> {
    vec![
        ("name", "Promo"),
        ("type", "1"),
        ("value", "100"),
        ("started_at", "1700000000"),
        ("ended_at", "1800000000"),
    ]
}

#[test]
fn coupon_generate_validation_reports_first_failure_in_declaration_order() {
    // A fully valid single-create payload passes.
    assert!(coupon_generate_validation(&params(&valid_coupon())).is_ok());

    // generate_count integer then max, ahead of every other field.
    let mut p = valid_coupon();
    p.push(("generate_count", "abc"));
    assert_validation(
        coupon_generate_validation(&params(&p)),
        "generate_count",
        "生成数量必须为数字",
    );
    let mut p = valid_coupon();
    p.push(("generate_count", "501"));
    assert_validation(
        coupon_generate_validation(&params(&p)),
        "generate_count",
        "生成数量最大为500个",
    );
    let mut p = valid_coupon();
    p.push(("generate_count", "500"));
    assert!(coupon_generate_validation(&params(&p)).is_ok());

    // Required + enum checks.
    assert_validation(
        coupon_generate_validation(&params(&[
            ("type", "1"),
            ("value", "1"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ])),
        "name",
        "名称不能为空",
    );
    assert_validation(
        coupon_generate_validation(&params(&[
            ("name", "n"),
            ("type", "9"),
            ("value", "1"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ])),
        "type",
        "类型格式有误",
    );
    assert_validation(
        coupon_generate_validation(&params(&[
            ("name", "n"),
            ("type", "1"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ])),
        "value",
        "金额或比例不能为空",
    );
    assert_validation(
        coupon_generate_validation(&params(&[
            ("name", "n"),
            ("type", "1"),
            ("value", "x"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ])),
        "value",
        "金额或比例格式有误",
    );
    for (coupon_type, value) in [("1", "-1"), ("1", "2147483648"), ("2", "-1"), ("2", "101")] {
        assert_validation(
            coupon_generate_validation(&params(&[
                ("name", "n"),
                ("type", coupon_type),
                ("value", value),
                ("started_at", "1"),
                ("ended_at", "2"),
            ])),
            "value",
            "金额或比例格式有误",
        );
    }

    // A scalar limit_plan_ids fails `array`; a bracketed one passes.
    let mut p = valid_coupon();
    p.push(("limit_plan_ids", "5"));
    assert_validation(
        coupon_generate_validation(&params(&p)),
        "limit_plan_ids",
        "指定订阅格式有误",
    );
    let mut p = valid_coupon();
    p.push(("limit_plan_ids[0]", "5"));
    assert!(coupon_generate_validation(&params(&p)).is_ok());
}

#[test]
fn giftcard_generate_validation_uses_required_if_and_untranslated_keys() {
    // type=5 requires value and plan_id; the required_if failure surfaces the
    // untranslated key, not V2Board's dead `value.required` message.
    assert_validation(
        giftcard_generate_validation(&params(&[
            ("name", "g"),
            ("type", "5"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ])),
        "value",
        "validation.required_if",
    );
    assert_validation(
        giftcard_generate_validation(&params(&[
            ("name", "g"),
            ("type", "5"),
            ("value", "10"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ])),
        "plan_id",
        "validation.required_if",
    );
    // type=4 needs neither value nor plan_id.
    assert!(
        giftcard_generate_validation(&params(&[
            ("name", "g"),
            ("type", "4"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ]))
        .is_ok()
    );
    // A non-integer plan_id falls back to `validation.integer`.
    assert_validation(
        giftcard_generate_validation(&params(&[
            ("name", "g"),
            ("type", "5"),
            ("value", "10"),
            ("plan_id", "abc"),
            ("started_at", "1"),
            ("ended_at", "1"),
        ])),
        "plan_id",
        "validation.integer",
    );
    // type enum covers 1..=5.
    assert_validation(
        giftcard_generate_validation(&params(&[("name", "g"), ("type", "6")])),
        "type",
        "类型格式有误",
    );
    assert_validation(
        giftcard_generate_validation(&params(&[
            ("name", "g"),
            ("type", "3"),
            ("value", "-1"),
            ("started_at", "1"),
            ("ended_at", "2"),
        ])),
        "value",
        "数值格式有误",
    );
    assert_validation(
        giftcard_generate_validation(&params(&[
            ("name", "g"),
            ("type", "3"),
            ("value", "2147483648"),
            ("started_at", "1"),
            ("ended_at", "2"),
        ])),
        "value",
        "数值格式有误",
    );
}

#[test]
fn user_generate_validation_requires_suffix_and_integer_checks() {
    assert!(user_generate_validation(&params(&[("email_suffix", "example.com")])).is_ok());
    assert_validation(
        user_generate_validation(&params(&[])),
        "email_suffix",
        "validation.required",
    );
    assert_validation(
        user_generate_validation(&params(&[("expired_at", "soon"), ("email_suffix", "x")])),
        "expired_at",
        "validation.integer",
    );
    assert_validation(
        user_generate_validation(&params(&[("generate_count", "999"), ("email_suffix", "x")])),
        "generate_count",
        "生成数量最大为500个",
    );
}

/// The required common columns every server save request must supply.
fn server_common() -> Vec<(&'static str, &'static str)> {
    vec![
        ("group_id", "[1]"),
        ("name", "n"),
        ("rate", "1"),
        ("host", "h"),
        ("port", "1"),
        ("server_port", "1"),
    ]
}

fn saved_columns(kind: &str, extra: &[(&str, &str)]) -> Vec<&'static str> {
    let mut pairs = server_common();
    pairs.extend_from_slice(extra);
    server_save_values(kind, &params(&pairs))
        .unwrap()
        .into_iter()
        .map(|(column, _)| column)
        .collect()
}

#[test]
fn server_save_omits_unsubmitted_optional_columns() {
    // A minimal shadowsocks save writes only required columns — never `sort`,
    // `show`, or the optional obfs pair — so a partial update preserves them.
    let cols = saved_columns("shadowsocks", &[("cipher", "aes-128-gcm")]);
    assert_eq!(
        cols,
        vec![
            "group_id",
            "name",
            "rate",
            "host",
            "port",
            "server_port",
            "cipher"
        ]
    );
    for absent in ["sort", "show", "obfs", "obfs_settings", "route_id", "tags"] {
        assert!(!cols.contains(&absent), "unexpected column {absent}");
    }

    // Supplying the optional keys opts them back in.
    let cols = saved_columns(
        "shadowsocks",
        &[
            ("cipher", "aes-128-gcm"),
            ("obfs", "http"),
            ("show", "1"),
            ("route_id[0]", "2"),
        ],
    );
    for present in ["obfs", "show", "route_id"] {
        assert!(cols.contains(&present), "missing column {present}");
    }
    assert!(!cols.contains(&"sort"));
}

#[test]
fn server_save_vmess_never_writes_legacy_rules_column() {
    let cols = saved_columns("vmess", &[("tls", "1"), ("network", "tcp")]);
    assert!(cols.contains(&"tls") && cols.contains(&"network"));
    for absent in [
        "rules",
        "networkSettings",
        "tlsSettings",
        "ruleSettings",
        "dnsSettings",
    ] {
        assert!(!cols.contains(&absent), "unexpected column {absent}");
    }
    // A submitted settings blob is written.
    let cols = saved_columns(
        "vmess",
        &[("tls", "1"), ("network", "tcp"), ("tlsSettings[x]", "1")],
    );
    assert!(cols.contains(&"tlsSettings"));
    assert!(!cols.contains(&"rules"));
}

#[test]
fn server_save_hysteria_always_writes_bandwidth_and_obfs_password() {
    // up_mbps/down_mbps/obfs_password are controller-assigned, so always present;
    // obfs and server_name are present-gated.
    let cols = saved_columns("hysteria", &[("version", "2"), ("insecure", "0")]);
    for always in [
        "version",
        "up_mbps",
        "down_mbps",
        "obfs_password",
        "insecure",
    ] {
        assert!(cols.contains(&always), "missing column {always}");
    }
    for absent in ["obfs", "server_name", "sort", "show"] {
        assert!(!cols.contains(&absent), "unexpected column {absent}");
    }
}

#[test]
fn server_save_vless_gates_settings_flow_and_sort() {
    // tcp + tls=0: no forced settings/flow, sort omitted.
    let base = [("tls", "0"), ("network", "tcp")];
    let cols = saved_columns("vless", &base);
    for absent in [
        "tls_settings",
        "flow",
        "sort",
        "network_settings",
        "encryption",
    ] {
        assert!(!cols.contains(&absent), "unexpected column {absent}");
    }
    // tls=2 forces reality tls_settings even when unsubmitted.
    assert!(saved_columns("vless", &[("tls", "2"), ("network", "tcp")]).contains(&"tls_settings"));
    // A non-tcp network forces flow (to null).
    assert!(saved_columns("vless", &[("tls", "0"), ("network", "ws")]).contains(&"flow"));
    // sort is only written when submitted.
    let mut with_sort = base.to_vec();
    with_sort.push(("sort", "5"));
    assert!(saved_columns("vless", &with_sort).contains(&"sort"));
}

#[test]
fn server_save_v2node_defaults_cipher_only_for_shadowsocks() {
    let ss = saved_columns(
        "v2node",
        &[
            ("protocol", "shadowsocks"),
            ("tls", "0"),
            ("network", "tcp"),
        ],
    );
    assert!(ss.contains(&"cipher"));
    for always in [
        "protocol",
        "up_mbps",
        "down_mbps",
        "obfs_password",
        "disable_sni",
    ] {
        assert!(ss.contains(&always), "missing column {always}");
    }
    for absent in ["listen_ip", "sort", "obfs", "tls_settings"] {
        assert!(!ss.contains(&absent), "unexpected column {absent}");
    }

    // vmess protocol never defaults cipher.
    let vmess = saved_columns(
        "v2node",
        &[("protocol", "vmess"), ("tls", "0"), ("network", "tcp")],
    );
    assert!(!vmess.contains(&"cipher"));
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
