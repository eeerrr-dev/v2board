use std::collections::BTreeSet;

use crate::mysql_import_policy::LegacyOrderPolicyError;

use super::*;

#[test]
fn registry_is_complete_and_stable() {
    audit_registry().expect("registry");
    assert_eq!(TABLE_MAPPINGS.len(), 14);
    assert_eq!(registry_sha256().expect("hash").len(), 64);
    assert!(SCALAR_REFERENCES.contains(&ScalarReference {
        source_table: "v2_order",
        target_table: "orders",
        column: "payment_id",
        source_referenced_table: "v2_payment",
        target_referenced_table: "payment_method",
        rule: ScalarReferenceRule::Nullable,
    }));
}

#[test]
fn public_row_conversion_enforces_the_stripe_policy() {
    let payment_ids = BTreeSet::from([7, 99]);
    let stripe_ids = BTreeSet::from([7]);

    let mut stripe_payment = complete_source_row(&PAYMENT);
    stripe_payment.insert(
        "payment".to_string(),
        SourceValue::Text("StripeCredit".to_string()),
    );
    assert_eq!(
        transform_mysql_import_row(&PAYMENT, &stripe_payment, &payment_ids, &stripe_ids).unwrap(),
        MysqlImportRowDisposition::Discard
    );

    let mut manual_payment = stripe_payment.clone();
    manual_payment.insert("payment".to_string(), SourceValue::Text("EPay".to_string()));
    assert!(matches!(
        transform_mysql_import_row(&PAYMENT, &manual_payment, &payment_ids, &stripe_ids).unwrap(),
        MysqlImportRowDisposition::Retain(_)
    ));

    let mut order = complete_source_row(&ORDER);
    order.insert("payment_id".to_string(), SourceValue::I64(7));
    order.insert(
        "callback_no".to_string(),
        SourceValue::Text("pi_legacy".to_string()),
    );
    for status in [0, 1] {
        order.insert("status".to_string(), SourceValue::I64(status));
        assert_eq!(
            transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids).unwrap(),
            MysqlImportRowDisposition::Discard
        );
    }
    for status in [2, 3, 4] {
        order.insert("status".to_string(), SourceValue::I64(status));
        let MysqlImportRowDisposition::Retain(row) =
            transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids).unwrap()
        else {
            panic!("terminal Stripe history must be retained");
        };
        assert_eq!(row.get("payment_id"), Some(&CanonicalValue::Null));
        assert_eq!(row.get("callback_no"), Some(&CanonicalValue::Null));
        assert_eq!(row.get("callback_no_hash"), Some(&CanonicalValue::Null));
    }
    order.insert("status".to_string(), SourceValue::I64(5));
    assert!(matches!(
        transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids),
        Err(ConverterError::OrderPolicy(
            LegacyOrderPolicyError::UnsupportedStripeStatus(5)
        ))
    ));

    order.insert("status".to_string(), SourceValue::I64(3));
    order.insert("payment_id".to_string(), SourceValue::I64(404));
    assert_eq!(
        transform_mysql_import_row(&ORDER, &order, &payment_ids, &stripe_ids),
        Err(ConverterError::OrderPolicy(
            LegacyOrderPolicyError::UnknownPaymentId(404)
        ))
    );
}

fn complete_source_row(mapping: &TableMapping) -> SourceRow {
    let mut row = SourceRow::new();
    for column in mapping.direct_columns {
        row.insert((*column).to_string(), SourceValue::I64(1));
    }
    for column in mapping.transformed_columns {
        let value = match column.rule {
            ColumnRule::ExactDecimal => SourceValue::Text("1".to_string()),
            ColumnRule::Json(JsonShape::Any) => SourceValue::Text("{}".to_string()),
            ColumnRule::Json(JsonShape::Array) => SourceValue::Text("[]".to_string()),
            ColumnRule::PositiveIdArray {
                require_non_empty, ..
            } => SourceValue::Text(if require_non_empty { "[1]" } else { "[]" }.to_string()),
        };
        row.insert(column.source.to_string(), value);
    }
    for column in mapping.consumed_source_columns {
        row.insert(column.source.to_string(), SourceValue::Null);
    }
    row
}

#[test]
fn schema_v1_discards_only_audited_operational_history() {
    assert_eq!(MYSQL_IMPORT_SCHEMA_VERSION, 1);
    assert_eq!(
        copied_table_mappings()
            .map(|mapping| mapping.source)
            .collect::<BTreeSet<_>>(),
        TABLE_MAPPINGS
            .iter()
            .map(|mapping| mapping.source)
            .collect()
    );
    assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_server_group"));
    assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_stat"));
    assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_user"));
    assert!(copied_table_mappings().any(|mapping| mapping.source == "v2_payment"));
    assert_eq!(
        TARGET_GENERATED_COLUMNS,
        &[("orders", &["referenced_plan_id"] as &[&str])]
    );
    assert!(
        TABLE_MAPPINGS
            .iter()
            .all(|mapping| !mapping.target.starts_with("v2_"))
    );
    assert!(
        DERIVED_MAPPINGS
            .iter()
            .all(|mapping| !mapping.target.starts_with("v2_"))
    );
    assert!(
        DISCARDED_TARGET_TABLES
            .iter()
            .all(|table| !table.starts_with("v2_"))
    );
    assert_eq!(
        DISCARDED_SOURCE_TABLES,
        [
            "failed_jobs",
            "v2_log",
            "v2_mail_log",
            "v2_stat_server",
            "v2_stat_user",
            "v2_server_route",
            "v2_server_shadowsocks",
            "v2_server_vmess",
            "v2_server_trojan",
            "v2_server_tuic",
            "v2_server_hysteria",
            "v2_server_vless",
            "v2_server_anytls",
            "v2_server_v2node",
            "v2_tutorial",
        ]
    );
    assert_eq!(discarded_target_tables().count(), 14);
    assert_eq!(
        discarded_target_tables().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "system_log",
            "mail_log",
            "server_anytls",
            "server_credential",
            "server_hysteria",
            "server_route",
            "server_shadowsocks",
            "server_trojan",
            "server_tuic",
            "server_v2node",
            "server_vless",
            "server_vmess",
            "server_traffic",
            "user_traffic",
        ])
    );
    for table in DISCARDED_SOURCE_TABLES {
        assert!(mapping_for_source(table).is_none());
    }
    assert_eq!(built_derived_mappings().count(), 1);
}

#[test]
fn permanent_user_credentials_and_counters_are_direct() {
    let user = mapping_for_source("v2_user").expect("user mapping");
    for column in [
        "id",
        "email",
        "password",
        "password_algo",
        "password_salt",
        "uuid",
        "token",
        "balance",
        "commission_balance",
        "t",
        "u",
        "d",
        "transfer_enable",
    ] {
        assert!(user.direct_columns.contains(&column), "missing {column}");
    }
    assert!(user.direct_columns.contains(&"invite_user_id"));
}

#[test]
fn positive_id_arrays_normalize_only_canonical_decimal_strings() {
    let mapping = mapping_for_source("v2_coupon").expect("coupon");
    let column = &mapping.transformed_columns[0];
    let value = normalize_id_array(mapping, column, r#"[1,"2",3]"#, i32::MAX as u64, false)
        .expect("valid ids");
    assert_eq!(value, serde_json::json!([1, 2, 3]));

    for invalid in [r#"[0]"#, r#"["01"]"#, r#"[" 1"]"#, r#"[1.0]"#, r#"null"#] {
        assert!(
            normalize_id_array(mapping, column, invalid, i32::MAX as u64, false).is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn decimal_normalization_never_uses_float() {
    assert_eq!(normalize_decimal("001.2300"), Some("1.23".to_string()));
    assert_eq!(normalize_decimal("-0.00"), Some("0".to_string()));
    assert_eq!(
        normalize_decimal("9007199254740993.25"),
        Some("9007199254740993.25".to_string())
    );
    assert_eq!(normalize_decimal("1e3"), None);
    assert_eq!(normalize_decimal(" 1"), None);
}

#[test]
fn sql_is_one_ordered_source_and_copy_stream_per_table() {
    let user = mapping_for_source("v2_user").expect("user");
    let source_sql = source_stream_sql(user).expect("source sql");
    assert!(source_sql.ends_with("FROM `v2_user` ORDER BY `id` ASC"));
    assert!(!source_sql.contains("LIMIT"));
    assert!(!source_sql.contains("OFFSET"));
    assert_eq!(SOURCE_ID_LOWER_BOUND, 0);

    let target_sql = target_copy_sql(user).expect("target sql");
    assert!(source_sql.contains("FROM `v2_user`"));
    assert!(target_sql.starts_with("COPY \"users\""));
    assert!(target_sql.contains("\"invite_user_id\""));
    assert!(target_sql.contains("NULL E'\\\\N'"));
    assert!(!target_sql.contains("INSERT"));

    let verify = target_verify_stream_sql(user).expect("verify sql");
    assert!(verify.starts_with("SELECT \"id\", \"invite_user_id\""));
    assert!(verify.ends_with("FROM \"users\" ORDER BY \"id\" ASC"));

    let reset = sequence_reset_sql(user).expect("sequence reset");
    assert!(reset.contains("GREATEST(COALESCE(MAX(id), 1), 1)"));
}

#[test]
fn canonical_hash_is_stable_across_independent_streams() {
    let user = mapping_for_source("v2_user").expect("user");
    let rows = [
        transform_row(user, &complete_source_row(user)).expect("first row"),
        transform_row(user, &complete_source_row(user)).expect("second row"),
    ];
    let mut expected = CanonicalRowsHasher::for_mapping(user).expect("expected hasher");
    expected.update_row(&rows[0]).expect("first row");
    expected.update_row(&rows[1]).expect("second row");

    let mut streamed = CanonicalRowsHasher::for_mapping(user).expect("stream hasher");
    streamed.update_row(&rows[0]).expect("first row");
    streamed.update_row(&rows[1]).expect("second row");
    assert_eq!(streamed.finish(), expected.finish());
}

#[test]
fn canonical_hash_matches_postgres_integer_and_json_semantics() {
    let columns = ["integer", "document"];
    let left = CanonicalRow::from([
        ("integer".to_string(), CanonicalValue::U64(7)),
        (
            "document".to_string(),
            CanonicalValue::Json(CanonicalJson::parse(r#"{"b":2,"a":1}"#).unwrap()),
        ),
    ]);
    let right = CanonicalRow::from([
        ("integer".to_string(), CanonicalValue::I64(7)),
        (
            "document".to_string(),
            CanonicalValue::Json(CanonicalJson::parse(r#"{"a":1,"b":2}"#).unwrap()),
        ),
    ]);
    let mut left_hash = CanonicalRowsHasher::new("fixture", &columns).unwrap();
    left_hash.update_row(&left).unwrap();
    let mut right_hash = CanonicalRowsHasher::new("fixture", &columns).unwrap();
    right_hash.update_row(&right).unwrap();
    assert_eq!(left_hash.finish(), right_hash.finish());
}

#[test]
fn json_numbers_remain_exact_and_hash_by_postgres_numeric_value() {
    let source = CanonicalJson::parse(
        r#"{"big":9007199254740993.25,"exponent":1.2300e3,"one":1.00,"zero":-0.0}"#,
    )
    .unwrap();
    let target =
        CanonicalJson::parse(r#"{"zero":0,"one":1.0,"exponent":1230.0,"big":9007199254740993.25}"#)
            .unwrap();
    assert!(
        source
            .to_compact_json()
            .unwrap()
            .contains("900719925474099325e-2")
    );
    assert_eq!(canonical_json_number("1e3").as_deref(), Some("1e3"));
    assert_eq!(canonical_json_number("1000").as_deref(), Some("1e3"));
    assert_eq!(canonical_json_number("1.00").as_deref(), Some("1e0"));
    assert_eq!(canonical_json_number("-0.0").as_deref(), Some("0"));

    let columns = ["document"];
    let mut source_hash = CanonicalRowsHasher::new("fixture", &columns).unwrap();
    source_hash
        .update_row(&CanonicalRow::from([(
            "document".to_string(),
            CanonicalValue::Json(source),
        )]))
        .unwrap();
    let mut target_hash = CanonicalRowsHasher::new("fixture", &columns).unwrap();
    target_hash
        .update_row(&CanonicalRow::from([(
            "document".to_string(),
            CanonicalValue::Json(target),
        )]))
        .unwrap();
    assert_eq!(source_hash.finish(), target_hash.finish());
}

#[test]
fn exact_json_matches_jsonb_object_key_semantics() {
    let duplicate = CanonicalJson::parse(r#"{"a":1,"\u0061":2}"#).unwrap();
    let collapsed = CanonicalJson::parse(r#"{"a":2}"#).unwrap();
    assert_eq!(duplicate, collapsed);
    assert_eq!(duplicate.to_compact_json().unwrap(), r#"{"a":2e0}"#);
}

#[test]
fn giftcard_redemptions_have_explicit_unknown_time_provenance() {
    let rows = expand_giftcard_redemptions(3, &SourceValue::Text(r#"[9,"7",9]"#.to_string()))
        .expect("redemptions");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].user_id, 7);
    assert_eq!(rows[0].created_at, 0);
    assert_eq!(rows[0].created_at_provenance, "legacy_unknown");
}
