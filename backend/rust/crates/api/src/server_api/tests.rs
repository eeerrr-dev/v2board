use std::collections::HashMap;

use axum::http::{HeaderMap, HeaderValue, header};
use rust_decimal::Decimal;
use serde_json::json;

use super::{
    ServerUserRow, TrafficEntry,
    config::{explode_php_lines, parse_i32_json_list, php_int_truthy, php_truthy},
    response::{etag_matches, sha1_hex},
    server_online_status,
    traffic::{
        ALIVE_CACHE_UPDATE_SCRIPT, charged_bytes, checked_traffic_pair, count_alive_ips,
        implicit_traffic_report_key, parse_server_rate, parse_traffic_entries,
        traffic_cache_entry_is_stale, traffic_entries_from_value, traffic_report_key,
        traffic_report_payload_hash, traffic_report_token,
    },
    users::{legacy_tidalab_user_response, server_user_without_uuid},
};

#[test]
fn online_status_handles_extreme_timestamps_without_overflow() {
    assert_eq!(server_online_status(10_000, Some(9_701)), 1);
    assert_eq!(server_online_status(10_000, Some(9_700)), 1);
    assert_eq!(server_online_status(10_000, Some(9_699)), 0);
    assert_eq!(server_online_status(i64::MAX, Some(i64::MIN)), 0);
    assert_eq!(server_online_status(i64::MIN, Some(i64::MIN)), 1);
}

#[test]
fn traffic_cache_staleness_handles_extreme_timestamps_without_overflow() {
    assert!(!traffic_cache_entry_is_stale(1_000, 900));
    assert!(traffic_cache_entry_is_stale(1_000, 899));
    assert!(traffic_cache_entry_is_stale(i64::MAX, i64::MIN));
    assert!(!traffic_cache_entry_is_stale(i64::MIN, i64::MAX));
}

#[test]
fn alive_cache_updates_are_atomic_and_batched_in_redis() {
    assert!(ALIVE_CACHE_UPDATE_SCRIPT.contains("for index, key in ipairs(KEYS)"));
    assert!(ALIVE_CACHE_UPDATE_SCRIPT.contains("value[node_bucket]"));
    assert!(ALIVE_CACHE_UPDATE_SCRIPT.contains("value.alive_ip = alive_count"));
    assert!(ALIVE_CACHE_UPDATE_SCRIPT.contains("redis.call('SET'"));
}

#[test]
fn persisted_traffic_accumulation_rejects_i64_overflow() {
    assert_eq!(checked_traffic_pair(10, 20, 1, 2).unwrap(), (11, 22));
    assert!(checked_traffic_pair(i64::MAX, 0, 1, 0).is_err());
    assert!(checked_traffic_pair(0, i64::MIN, 0, -1).is_err());
}

#[test]
fn traffic_statistics_use_bounded_atomic_upserts() {
    let source = include_str!("traffic.rs");
    let start = source.find("async fn persist_traffic_stats").unwrap();
    let end = source[start..]
        .find("pub(super) fn checked_traffic_pair")
        .map(|offset| start + offset)
        .unwrap();
    let stats = &source[start..end];
    assert!(stats.contains("entries.chunks(TRAFFIC_REPORT_SQL_BATCH_SIZE)"));
    assert!(stats.contains("ON CONFLICT (server_rate, user_id, record_at)"));
    assert!(stats.contains("user_traffic.u + EXCLUDED.u"));
    assert!(!stats.contains("SELECT id, u, d"));
    assert!(!stats.contains("UPDATE user_traffic SET"));
}

#[test]
fn traffic_report_analytics_use_one_bounded_bulk_enqueue() {
    let source = include_str!("traffic.rs");
    let start = source
        .find("async fn persist_durable_traffic_report")
        .unwrap();
    let end = source[start..]
        .find("fn is_internal_traffic_report_key")
        .map(|offset| start + offset)
        .unwrap();
    let persist = &source[start..end];
    assert!(persist.contains("Vec::<AnalyticsEvent>::with_capacity(items.len())"));
    assert!(persist.contains("enqueue_events(&mut tx, &analytics_events, now)"));
    assert!(!persist.contains("enqueue_event(&mut tx"));
}

fn object(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    value.as_object().cloned().unwrap()
}

#[test]
fn parse_server_rate_coerces_non_numeric_to_zero() {
    assert_eq!(parse_server_rate("0.5"), Decimal::new(5, 1));
    assert_eq!(parse_server_rate("1"), Decimal::ONE);
    assert_eq!(parse_server_rate("2.5"), Decimal::new(25, 1));
    // PHP coerces empty / non-numeric to 0 (charged traffic becomes 0), never 1.
    assert_eq!(parse_server_rate(""), Decimal::ZERO);
    assert_eq!(parse_server_rate("abc"), Decimal::ZERO);
}

#[test]
fn charged_bytes_multiplies_and_rounds() {
    assert_eq!(charged_bytes(100, Decimal::new(5, 1)).unwrap(), 50);
    assert_eq!(
        charged_bytes(1_000_000_000, Decimal::ONE).unwrap(),
        1_000_000_000
    );
    // half rounds away from zero (3 * 0.5 = 1.5 -> 2)
    assert_eq!(charged_bytes(3, Decimal::new(5, 1)).unwrap(), 2);
    // rate coerced to 0 zeroes the charge regardless of raw bytes
    assert_eq!(charged_bytes(9_999, Decimal::ZERO).unwrap(), 0);
    // Decimal multiplication stays exact beyond f64's integer precision.
    assert_eq!(
        charged_bytes(9_007_199_254_740_991, Decimal::new(11, 1)).unwrap(),
        9_907_919_180_215_090
    );
    assert!(charged_bytes(i64::MAX, Decimal::from(2)).is_err());
}

#[test]
fn traffic_array_dedups_user_id_last_write_wins() {
    // ShadowsocksTidalabController.php:78-80 last-write-wins per user_id (not summed).
    let entries = traffic_entries_from_value(
        &json!([
            { "user_id": 7, "u": 100, "d": 200 },
            { "user_id": 9, "u": 1, "d": 2 },
            { "user_id": 7, "u": 5, "d": 6 },
        ]),
        true,
    )
    .unwrap()
    .entries;
    assert_eq!(entries.len(), 2);
    // first-appearance order preserved (7 then 9); user 7 carries the LAST pair
    assert_eq!(entries[0].user_id, 7);
    assert_eq!((entries[0].u, entries[0].d), (5, 6));
    assert_eq!(entries[1].user_id, 9);
    assert_eq!((entries[1].u, entries[1].d), (1, 2));
}

#[test]
fn durable_traffic_reports_reject_malformed_or_negative_rows() {
    assert!(traffic_entries_from_value(&json!([{ "user_id": 7, "u": 1 }]), true).is_err());
    assert!(parse_traffic_entries(Some(&json!({ "7": [-1, 2] })), &HashMap::new(), true,).is_err());
    assert!(
        parse_traffic_entries(
            Some(&json!({ "2147483648": [1, 2] })),
            &HashMap::new(),
            true,
        )
        .is_err()
    );
}

#[test]
fn legacy_traffic_reports_keep_documented_counter_coercion_but_surface_it() {
    let parsed = parse_traffic_entries(
        Some(&json!([
            { "user_id": 7, "u": 5 },
            "ignored",
        ])),
        &HashMap::new(),
        false,
    )
    .unwrap();
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!((parsed.entries[0].u, parsed.entries[0].d), (5, 0));
    assert_eq!(parsed.ignored_rows, 1);
    assert_eq!(parsed.defaulted_counters, 1);
}

#[test]
fn traffic_report_idempotency_key_is_explicit_and_unambiguous() {
    let mut headers = HeaderMap::new();
    headers.insert("idempotency-key", HeaderValue::from_static("report-7"));
    assert_eq!(
        traffic_report_token(&headers, &HashMap::new()).unwrap(),
        Some("report-7".to_string())
    );
    assert_eq!(
        traffic_report_token(
            &HeaderMap::new(),
            &HashMap::from([("report_id".to_string(), "report-8".to_string())]),
        )
        .unwrap(),
        Some("report-8".to_string())
    );
    assert!(
        traffic_report_token(
            &headers,
            &HashMap::from([("report_id".to_string(), "different".to_string())]),
        )
        .is_err()
    );
    let mut invalid_headers = HeaderMap::new();
    invalid_headers.insert("idempotency-key", HeaderValue::from_bytes(&[0xff]).unwrap());
    assert!(traffic_report_token(&invalid_headers, &HashMap::new()).is_err());
}

#[test]
fn traffic_report_identity_is_node_scoped_and_payload_order_independent() {
    assert_eq!(traffic_report_key("vmess", 1, "same").len(), 64);
    assert_ne!(
        traffic_report_key("vmess", 1, "same"),
        traffic_report_key("vmess", 2, "same")
    );
    let first = vec![
        TrafficEntry {
            user_id: 7,
            u: 10,
            d: 20,
        },
        TrafficEntry {
            user_id: 9,
            u: 30,
            d: 40,
        },
    ];
    let mut reversed = first.clone();
    reversed.reverse();
    assert_eq!(
        traffic_report_payload_hash(1, "1.0", "vmess", &first),
        traffic_report_payload_hash(1, "1.0", "vmess", &reversed)
    );
    assert_ne!(
        traffic_report_payload_hash(1, "1.0", "vmess", &first),
        traffic_report_payload_hash(1, "2.0", "vmess", &first)
    );
    let implicit = implicit_traffic_report_key("vmess", 1);
    assert_eq!(implicit.len(), 64);
    assert!(implicit.starts_with("i-"));
}

#[test]
fn count_alive_ips_mode0_sums_raw_connections() {
    // device_limit_mode 0 (UniProxyController::alive :185-191): raw connection count.
    let nodes = object(json!({
        "shadowsocks5": { "aliveips": ["1.1.1.1_5", "1.1.1.1_5", "2.2.2.2_5"], "lastupdateAt": 0 },
        "trojan9": { "aliveips": ["1.1.1.1_9"], "lastupdateAt": 0 },
        "alive_ip": 99,
    }));
    assert_eq!(count_alive_ips(&nodes, 0), 4);
}

#[test]
fn count_alive_ips_mode1_dedups_unique_ips() {
    // device_limit_mode 1 (UniProxyController::alive :174-184): unique IPs, deduped by the
    // substring before the first '_', across every node bucket.
    let nodes = object(json!({
        "shadowsocks5": { "aliveips": ["1.1.1.1_5", "1.1.1.1_5", "2.2.2.2_5"], "lastupdateAt": 0 },
        "trojan9": { "aliveips": ["1.1.1.1_9"], "lastupdateAt": 0 },
        "alive_ip": 99,
    }));
    // {1.1.1.1, 2.2.2.2} regardless of node id suffix -> 2.
    assert_eq!(count_alive_ips(&nodes, 1), 2);
}

#[test]
fn legacy_tidalab_user_etag_scopes_to_data_array() {
    // ShadowsocksTidalab:51 hashes the BARE $result array, not the {data} wrapper.
    let data = vec![json!({ "id": 1, "port": 443, "cipher": "aes-128-gcm", "secret": "uuid-1" })];
    let array_etag = sha1_hex(&serde_json::to_vec(&data).unwrap());
    let wrapper_etag = sha1_hex(&serde_json::to_vec(&json!({ "data": data.clone() })).unwrap());
    assert_ne!(
        array_etag, wrapper_etag,
        "wrapper and array hashes must differ"
    );

    let response = legacy_tidalab_user_response("shadowsocks", data, &HeaderMap::new()).unwrap();
    let etag = response
        .headers()
        .get(header::ETAG)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(etag, format!("\"{array_etag}\""));
}

#[test]
fn v2_config_etag_matches_quoted_if_none_match() {
    // Item 7 pin (intentionally more correct than ServerController.php:105): a client that
    // echoes the emitted quoted ETag is matched via `contains`, yielding a 304.
    let mut headers = HeaderMap::new();
    headers.insert(
        header::IF_NONE_MATCH,
        HeaderValue::from_static("\"abc123\""),
    );
    assert!(etag_matches(&headers, "abc123"));
}

#[test]
fn php_int_truthy_matches_php_cast() {
    assert!(php_int_truthy(&json!(1)));
    assert!(php_int_truthy(&json!(true)));
    assert!(php_int_truthy(&json!("1")));
    assert!(!php_int_truthy(&json!(0)));
    assert!(!php_int_truthy(&json!(false)));
    assert!(!php_int_truthy(&json!("")));
    assert!(!php_int_truthy(&json!("0")));
}

#[test]
fn php_truthy_filters_like_array_filter() {
    assert!(php_truthy(&json!("geosite:cn")));
    assert!(!php_truthy(&json!("")));
    assert!(!php_truthy(&json!("0")));
    assert!(!php_truthy(&serde_json::Value::Null));
}

#[test]
fn server_group_ids_accept_legacy_json_numeric_strings() {
    assert_eq!(
        parse_i32_json_list(Some(&r#"["1",2,"invalid"]"#.to_string())),
        vec![1, 2]
    );
    assert_eq!(parse_i32_json_list(Some(&"3".to_string())), vec![3]);
}

#[test]
fn server_route_lookup_is_parameter_bounded_and_restores_manifest_order_in_rust() {
    let source = include_str!("config.rs");
    let start = source.find("async fn server_routes").unwrap();
    let end = source[start..]
        .find("pub(super) fn parse_i32_json_list")
        .map(|offset| start + offset)
        .unwrap();
    let implementation = &source[start..end];
    assert!(implementation.contains("unique_ids.chunks(500)"));
    assert!(implementation.contains("rows.sort_by_key"));
    assert!(!implementation.contains("ORDER BY CASE"));
}

#[test]
fn tidalab_user_keeps_null_speed_and_device_limit() {
    // TrojanTidalab/Deepbwork serialize the raw user model, so both keys stay present and are
    // emitted as JSON null when the column is null (they are NOT array_filtered like UniProxy).
    let user = ServerUserRow {
        id: 7,
        uuid: "uuid-7".to_string(),
        speed_limit: None,
        device_limit: None,
    };
    let item = server_user_without_uuid(&user);
    assert_eq!(item.get("id"), Some(&json!(7)));
    assert_eq!(item.get("speed_limit"), Some(&serde_json::Value::Null));
    assert_eq!(item.get("device_limit"), Some(&serde_json::Value::Null));
    assert!(!item.contains_key("uuid"));

    let user = ServerUserRow {
        id: 8,
        uuid: "uuid-8".to_string(),
        speed_limit: Some(100),
        device_limit: Some(3),
    };
    let item = server_user_without_uuid(&user);
    assert_eq!(item.get("speed_limit"), Some(&json!(100)));
    assert_eq!(item.get("device_limit"), Some(&json!(3)));
}

#[test]
fn uniproxy_user_drops_null_speed_and_device_limit() {
    // UniProxyController::user array_filters null attributes away, so the struct serialization
    // (used only by the uniproxy user endpoint) must keep skipping None. Guard against a
    // regression that would leak null keys onto the uniproxy path.
    let user = ServerUserRow {
        id: 1,
        uuid: "uuid-1".to_string(),
        speed_limit: None,
        device_limit: None,
    };
    let value = serde_json::to_value(&user).unwrap();
    let object = value.as_object().unwrap();
    assert_eq!(object.get("uuid"), Some(&json!("uuid-1")));
    assert!(!object.contains_key("speed_limit"));
    assert!(!object.contains_key("device_limit"));
}

#[test]
fn explode_php_lines_splits_and_filters_falsy() {
    assert!(explode_php_lines(None).is_empty());
    assert!(explode_php_lines(Some("")).is_empty());
    assert_eq!(
        explode_php_lines(Some("baidu.com\n\ngoogle.com\n0")),
        vec![json!("baidu.com"), json!("google.com")]
    );
}
