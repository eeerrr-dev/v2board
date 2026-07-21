use std::collections::HashMap;

use axum::{
    Json,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::{Datelike, TimeZone, Utc};
#[cfg(test)]
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use v2board_application::server_runtime::{
    AliveUpdate, PersistTrafficError, PersistTrafficReport, RuntimeTrafficEntry, ServerMetric,
};
use v2board_compat::ApiError;
use v2board_domain_model::ServerKind;

use crate::{json_value::value_to_i64, runtime::AppState};

use super::{
    ParsedTrafficEntries, ServerNodeRow, TrafficEntry,
    request::{load_server_node, load_uniproxy_node, required_i32_param},
};

pub(super) const ALIVE_CACHE_MAX_IPS_PER_USER: usize = 256;
pub(super) const ALIVE_CACHE_MAX_USER_PAYLOAD_BYTES: usize = 16 * 1024;

pub(super) async fn server_push(
    state: &AppState,
    headers: &HeaderMap,
    params: &HashMap<String, String>,
    body: Option<&serde_json::Value>,
    uniproxy: bool,
    fallback_node_type: Option<&str>,
) -> Result<Response, ApiError> {
    let (node_type, node) = if uniproxy {
        load_uniproxy_node(state, params).await?
    } else {
        // Legacy Deepbwork/Tidalab submit endpoints hardcode their protocol per-controller
        // (e.g. DeepbworkController::submit -> ServerVmess::find + trafficFetch(..,'vmess')) and
        // never honor a request `node_type`. Force the caller's fixed protocol so a submit with a
        // spoofed node_type cannot load a different protocol's node / write its SERVER_* keys.
        let node_type = ServerKind::try_from(fallback_node_type.unwrap_or("shadowsocks"))
            .map_err(|_| ApiError::legacy("server is not exist"))?;
        let node_id = required_i32_param(params, "node_id")?;
        let Some(node) = load_server_node(state, node_type, node_id).await? else {
            return Ok(Json(json!({ "ret": 0, "msg": "server is not found" })).into_response());
        };
        (node_type, node)
    };

    let report_token = traffic_report_token(headers, params)?;
    if state.config_snapshot().server_require_idempotency_key && report_token.is_none() {
        return Err(ApiError::bad_request(
            "Traffic report idempotency key is required",
        ));
    }
    let parsed = parse_traffic_entries(body, params, report_token.is_some())?;
    if parsed.ignored_rows != 0 || parsed.defaulted_counters != 0 {
        tracing::warn!(
            node_id = node.id,
            node_type = node_type.as_str(),
            ignored_rows = parsed.ignored_rows,
            defaulted_counters = parsed.defaulted_counters,
            "legacy traffic payload required compatibility coercion"
        );
    }
    let entries = parsed.entries;
    server_cache_count(
        state,
        ServerMetric::OnlineUser,
        node_type,
        node.id,
        entries.len() as i64,
    )
    .await?;
    state
        .server_runtime_service()
        .write_metric(
            node_type,
            node.id,
            ServerMetric::LastPushAt,
            Utc::now().timestamp(),
        )
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    if !entries.is_empty() {
        // Success means the complete report (charged deltas + daily statistics) is
        // durably committed. The worker applies user counters exactly once. Older
        // nodes without a client id receive a server-generated report identity;
        // they cannot get retry deduplication across a lost HTTP response, but they
        // no longer get a false 200 after partial Redis/SQL persistence.
        persist_traffic_fetch(state, &node, node_type, &entries, report_token.as_deref()).await?;
    }

    if uniproxy {
        Ok(Json(json!({ "data": true })).into_response())
    } else {
        Ok(Json(json!({ "ret": 1, "msg": "ok" })).into_response())
    }
}

pub(super) async fn server_alive_list(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Response, ApiError> {
    // UniProxyController::__construct (UniProxyController.php:21-37) resolves and validates the
    // node before every action, aborting 500 'server is not exist' on a missing/invalid node_id.
    // Reproduce that gate here so alivelist matches `alive`/`user`/`push` (which already validate).
    load_uniproxy_node(state, params).await?;
    let mut alive = serde_json::Map::new();
    for (user_id, alive_ip) in state
        .server_runtime_service()
        .alive_counts(Utc::now().timestamp())
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
    {
        alive.insert(user_id.to_string(), json!(alive_ip));
    }
    Ok(Json(json!({ "alive": alive })).into_response())
}

pub(super) async fn server_alive(
    state: &AppState,
    params: &HashMap<String, String>,
    body: Option<&serde_json::Value>,
) -> Result<Response, ApiError> {
    let (node_type, node) = load_uniproxy_node(state, params).await?;
    let Some(object) = body.and_then(serde_json::Value::as_object) else {
        return Ok(Json(json!({ "data": true })).into_response());
    };

    let now = Utc::now().timestamp();
    // UniProxyController::alive :174 reads device_limit_mode to decide how alive IPs are counted.
    let device_limit_mode = state.config_snapshot().device_limit_mode;
    let node_bucket = format!("{}{}", node_type.as_str(), node.id);
    let mut updates = Vec::with_capacity(object.len());
    for (uid, ips) in object {
        let Ok(user_id) = uid.parse::<i64>() else {
            continue;
        };
        let Some(ips) = ips.as_array() else {
            continue;
        };
        if ips.len() > ALIVE_CACHE_MAX_IPS_PER_USER {
            return Err(ApiError::bad_request(
                "Alive-IP user payload exceeds the supported entry limit",
            ));
        }
        let ips = serde_json::to_string(ips)
            .expect("an alive-IP serde_json::Value array is always serializable");
        if ips.len() > ALIVE_CACHE_MAX_USER_PAYLOAD_BYTES {
            return Err(ApiError::bad_request(
                "Alive-IP user payload exceeds the supported size limit",
            ));
        }
        updates.push(AliveUpdate {
            user_id,
            ips_json: ips,
        });
    }
    state
        .server_runtime_service()
        .merge_alive(&node_bucket, now, device_limit_mode, &updates)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(json!({ "data": true })).into_response())
}

#[cfg(test)]
pub(super) fn traffic_cache_entry_is_stale(now: i64, last_update: i64) -> bool {
    now.saturating_sub(last_update) > 100
}

/// Count alive IPs for a user across their per-node `aliveips` buckets.
///
/// Mirrors UniProxyController::alive (:172-192): with `device_limit_mode == 1` the count is
/// the number of UNIQUE client IPs (deduped by `explode("_", ip_NodeId)[0]`, the substring
/// before the first `_`); otherwise it is the raw sum of connection entries across nodes.
/// The `alive_ip` bookkeeping key is never itself a node bucket, so it is skipped.
#[cfg(test)]
pub(super) fn count_alive_ips(
    nodes: &serde_json::Map<String, serde_json::Value>,
    device_limit_mode: i32,
) -> usize {
    if device_limit_mode == 1 {
        let mut unique = std::collections::HashSet::new();
        for (key, node) in nodes {
            if key == "alive_ip" {
                continue;
            }
            let Some(ips) = node.get("aliveips").and_then(serde_json::Value::as_array) else {
                continue;
            };
            for ip_node in ips {
                if let Some(text) = ip_node.as_str() {
                    // explode("_", ip_NodeId)[0]: substring before the first '_'.
                    unique.insert(text.split('_').next().unwrap_or(text).to_string());
                }
            }
        }
        unique.len()
    } else {
        nodes
            .iter()
            .filter(|(key, _)| key.as_str() != "alive_ip")
            .filter_map(|(_, node)| node.get("aliveips").and_then(serde_json::Value::as_array))
            .map(Vec::len)
            .sum()
    }
}

pub(super) async fn server_cache_timestamp(
    state: &AppState,
    node_type: ServerKind,
    node_id: i32,
) -> Result<(), ApiError> {
    server_cache_count(
        state,
        ServerMetric::LastCheckAt,
        node_type,
        node_id,
        Utc::now().timestamp(),
    )
    .await
}

async fn server_cache_count(
    state: &AppState,
    metric: ServerMetric,
    node_type: ServerKind,
    node_id: i32,
    value: i64,
) -> Result<(), ApiError> {
    state
        .server_runtime_service()
        .write_metric(node_type, node_id, metric, value)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(super) fn parse_traffic_entries(
    body: Option<&serde_json::Value>,
    params: &HashMap<String, String>,
    strict: bool,
) -> Result<ParsedTrafficEntries, ApiError> {
    let parsed = if let Some(value) = body {
        traffic_entries_from_value(value, strict)?
    } else {
        traffic_entry_from_params(params, strict)?
    };
    for entry in &parsed.entries {
        validate_traffic_entry(entry)?;
    }
    Ok(parsed)
}

fn traffic_entry_from_params(
    params: &HashMap<String, String>,
    strict: bool,
) -> Result<ParsedTrafficEntries, ApiError> {
    let fields = [params.get("user_id"), params.get("u"), params.get("d")];
    if fields.iter().all(|value| value.is_none()) {
        return Ok(ParsedTrafficEntries {
            entries: Vec::new(),
            ignored_rows: 0,
            defaulted_counters: 0,
        });
    }
    let parsed = fields.map(|value| value.and_then(|value| value.parse::<i64>().ok()));
    if let [Some(user_id), Some(u), Some(d)] = parsed {
        return Ok(ParsedTrafficEntries {
            entries: vec![TrafficEntry { user_id, u, d }],
            ignored_rows: 0,
            defaulted_counters: 0,
        });
    }
    if strict {
        return Err(ApiError::bad_request("Invalid traffic data"));
    }
    Ok(ParsedTrafficEntries {
        entries: Vec::new(),
        ignored_rows: 1,
        defaulted_counters: 0,
    })
}

pub(super) fn traffic_entries_from_value(
    value: &serde_json::Value,
    strict: bool,
) -> Result<ParsedTrafficEntries, ApiError> {
    let mut ignored_rows = 0;
    let mut defaulted_counters = 0;
    let entries = match value {
        serde_json::Value::Array(items) => {
            // Tidalab folds the array into `$formatData[$user_id]`, so duplicate
            // users are last-write-wins while retaining first-key order.
            let mut order = Vec::new();
            let mut latest = HashMap::new();
            for item in items {
                let Some(object) = item.as_object() else {
                    if strict {
                        return Err(ApiError::bad_request("Invalid traffic data"));
                    }
                    ignored_rows += 1;
                    continue;
                };
                let user_id = traffic_integer(object.get("user_id"), strict);
                let (u, u_defaulted) = traffic_counter(object.get("u"), strict);
                let (d, d_defaulted) = traffic_counter(object.get("d"), strict);
                let (Some(user_id), Some(u), Some(d)) = (user_id, u, d) else {
                    if strict {
                        return Err(ApiError::bad_request("Invalid traffic data"));
                    }
                    ignored_rows += 1;
                    continue;
                };
                defaulted_counters += usize::from(u_defaulted) + usize::from(d_defaulted);
                if !latest.contains_key(&user_id) {
                    order.push(user_id);
                }
                latest.insert(user_id, TrafficEntry { user_id, u, d });
            }
            order
                .into_iter()
                .filter_map(|user_id| latest.remove(&user_id))
                .collect()
        }
        serde_json::Value::Object(object) => {
            if object.contains_key("user_id") {
                let user_id = traffic_integer(object.get("user_id"), strict);
                let (u, u_defaulted) = traffic_counter(object.get("u"), strict);
                let (d, d_defaulted) = traffic_counter(object.get("d"), strict);
                let (Some(user_id), Some(u), Some(d)) = (user_id, u, d) else {
                    return if strict {
                        Err(ApiError::bad_request("Invalid traffic data"))
                    } else {
                        Ok(ParsedTrafficEntries {
                            entries: Vec::new(),
                            ignored_rows: 1,
                            defaulted_counters: 0,
                        })
                    };
                };
                defaulted_counters += usize::from(u_defaulted) + usize::from(d_defaulted);
                vec![TrafficEntry { user_id, u, d }]
            } else {
                let mut entries = Vec::with_capacity(object.len());
                for (user_id, value) in object {
                    let Ok(user_id) = user_id.parse::<i64>() else {
                        if strict {
                            return Err(ApiError::bad_request("Invalid traffic data"));
                        }
                        ignored_rows += 1;
                        continue;
                    };
                    let Some((u, d, defaulted)) = traffic_pair_from_value(value, strict) else {
                        if strict {
                            return Err(ApiError::bad_request("Invalid traffic data"));
                        }
                        ignored_rows += 1;
                        continue;
                    };
                    defaulted_counters += defaulted;
                    entries.push(TrafficEntry { user_id, u, d });
                }
                entries
            }
        }
        _ if strict => return Err(ApiError::bad_request("Invalid traffic data")),
        _ => {
            ignored_rows = 1;
            Vec::new()
        }
    };
    Ok(ParsedTrafficEntries {
        entries,
        ignored_rows,
        defaulted_counters,
    })
}

fn traffic_integer(value: Option<&serde_json::Value>, strict: bool) -> Option<i64> {
    if strict {
        match value? {
            serde_json::Value::Number(value) => value.as_i64(),
            serde_json::Value::String(value) => value.parse().ok(),
            _ => None,
        }
    } else {
        value.and_then(value_to_i64)
    }
}

fn traffic_counter(value: Option<&serde_json::Value>, strict: bool) -> (Option<i64>, bool) {
    let parsed = traffic_integer(value, strict);
    if strict || parsed.is_some() {
        (parsed, false)
    } else {
        (Some(0), true)
    }
}

fn traffic_pair_from_value(value: &serde_json::Value, strict: bool) -> Option<(i64, i64, usize)> {
    let (first, second) = match value {
        serde_json::Value::Array(items) if !strict || items.len() == 2 => {
            (items.first(), items.get(1))
        }
        serde_json::Value::Object(object) => (object.get("u"), object.get("d")),
        _ => return None,
    };
    let (u, u_defaulted) = traffic_counter(first, strict);
    let (d, d_defaulted) = traffic_counter(second, strict);
    Some((u?, d?, usize::from(u_defaulted) + usize::from(d_defaulted)))
}

fn validate_traffic_entry(entry: &TrafficEntry) -> Result<(), ApiError> {
    if !(1..=i64::from(i32::MAX)).contains(&entry.user_id) || entry.u < 0 || entry.d < 0 {
        return Err(ApiError::bad_request("Invalid traffic data"));
    }
    Ok(())
}

pub(super) fn traffic_report_token(
    headers: &HeaderMap,
    params: &HashMap<String, String>,
) -> Result<Option<String>, ApiError> {
    let header = headers
        .get("idempotency-key")
        .map(|value| {
            value
                .to_str()
                .map(str::trim)
                .map_err(|_| ApiError::bad_request("Traffic report idempotency key is invalid"))
        })
        .transpose()?
        .filter(|value| !value.is_empty());
    let param = ["report_id", "idempotency_key"]
        .into_iter()
        .find_map(|key| params.get(key))
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if header.is_some() && param.is_some() && header != param {
        return Err(ApiError::bad_request(
            "Conflicting traffic report idempotency keys",
        ));
    }
    let token = header.or(param);
    if token.is_some_and(|value| value.len() > 512) {
        return Err(ApiError::bad_request(
            "Traffic report idempotency key is too long",
        ));
    }
    Ok(token.map(str::to_owned))
}

pub(super) fn traffic_report_key(node_type: &str, node_id: i32, token: &str) -> String {
    sha256_hex(format!("{node_type}\0{node_id}\0{token}").as_bytes())
}

pub(super) fn implicit_traffic_report_key(node_type: &str, node_id: i32) -> String {
    let digest = traffic_report_key(node_type, node_id, &Uuid::new_v4().to_string());
    format!("i-{}", &digest[..62])
}

pub(super) fn traffic_report_payload_hash(
    node_id: i32,
    rate: &str,
    node_type: &str,
    entries: &[TrafficEntry],
) -> String {
    let mut canonical = format!("{node_type}\n{node_id}\n{rate}\n").into_bytes();
    let mut entries = entries.to_vec();
    entries.sort_unstable_by_key(|entry| entry.user_id);
    for entry in entries {
        canonical
            .extend_from_slice(format!("{}:{}:{}\n", entry.user_id, entry.u, entry.d).as_bytes());
    }
    sha256_hex(&canonical)
}

async fn persist_traffic_fetch(
    state: &AppState,
    node: &ServerNodeRow,
    node_type: ServerKind,
    entries: &[TrafficEntry],
    report_token: Option<&str>,
) -> Result<(), ApiError> {
    let node_type_text = node_type.as_str();
    let report_key = report_token.map_or_else(
        || implicit_traffic_report_key(node_type_text, node.id),
        |token| traffic_report_key(node_type_text, node.id, token),
    );
    let now = Utc::now().timestamp();
    let accounting_date = v2board_config::app_now().date_naive();
    let report = PersistTrafficReport {
        installation_id: state.installation_id.to_string(),
        report_key,
        payload_hash: traffic_report_payload_hash(node.id, &node.rate, node_type_text, entries),
        node_id: node.id,
        node_kind: node_type,
        group_ids: node.group_ids.clone(),
        rate: node.rate.clone(),
        entries: entries
            .iter()
            .map(|entry| RuntimeTrafficEntry {
                user_id: entry.user_id,
                upload: entry.u,
                download: entry.d,
            })
            .collect(),
        accepted_at: now,
        accounting_date: accounting_date.format("%Y-%m-%d").to_string(),
        accounting_record_at: today_start_timestamp(),
    };
    state
        .server_runtime_service()
        .persist_traffic(report)
        .await
        .map_err(persist_traffic_error)
}

fn persist_traffic_error(error: PersistTrafficError) -> ApiError {
    match error {
        PersistTrafficError::IdempotencyConflict => ApiError::bad_request(
            "Traffic report idempotency key was reused with a different payload",
        ),
        PersistTrafficError::UnauthorizedUser => {
            ApiError::bad_request("Traffic report contains an unauthorized user")
        }
        PersistTrafficError::RateOutOfRange => {
            ApiError::bad_request("Server traffic rate is outside the supported range")
        }
        PersistTrafficError::ChargeOutOfRange => {
            ApiError::bad_request("Server traffic charge is outside the supported range")
        }
        PersistTrafficError::TotalOutOfRange => {
            ApiError::bad_request("Server traffic total is outside the supported range")
        }
        PersistTrafficError::AnalyticsRateLimited => {
            tracing::warn!("traffic analytics soft-pressure rate limit reached");
            ApiError::too_many_requests(
                "Traffic ingestion is temporarily rate limited; retry later",
            )
        }
        PersistTrafficError::AnalyticsUnavailable => {
            tracing::warn!("traffic analytics admission refused the transaction");
            ApiError::service_unavailable(
                "Traffic ingestion is temporarily unavailable; retry later",
            )
        }
        PersistTrafficError::AnalyticsEventInvalid => {
            tracing::error!("refusing to persist an invalid traffic analytics event");
            ApiError::internal("failed to persist traffic analytics event")
        }
        PersistTrafficError::Repository(error) => {
            tracing::error!(?error, "failed to enqueue the traffic analytics event");
            ApiError::internal("failed to persist traffic analytics event")
        }
    }
}

#[cfg(test)]
pub(super) fn checked_traffic_pair(
    current_u: i64,
    current_d: i64,
    additional_u: i64,
    additional_d: i64,
) -> Result<(i64, i64), ApiError> {
    Ok((
        current_u.checked_add(additional_u).ok_or_else(|| {
            ApiError::bad_request("Server traffic total is outside the supported range")
        })?,
        current_d.checked_add(additional_d).ok_or_else(|| {
            ApiError::bad_request("Server traffic total is outside the supported range")
        })?,
    ))
}

fn today_start_timestamp() -> i64 {
    // StatUserJob/StatServerJob bucket `record_at` on Laravel's app timezone (Asia/Shanghai),
    // not the process TZ. The rust-api container sets no TZ, so chrono::Local would be UTC and
    // near-midnight pushes would mis-bucket the daily stat rows. Use the pinned +8 offset like
    // the workers crate (config::app_now/app_timezone).
    let now = v2board_config::app_now();
    v2board_config::app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|date| date.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

/// Coerce a node's `rate` to a multiplier. Laravel reads `$server['rate']` as a raw string and
/// PHP coerces a non-numeric / empty value to 0 (so charged traffic becomes 0), NOT to 1 — the
/// pinned "traffic-charge coercion" contract.
#[cfg(test)]
pub(super) fn parse_server_rate(rate: &str) -> Decimal {
    rate.parse::<Decimal>().unwrap_or(Decimal::ZERO)
}

/// Charged bytes billed against a user's quota: raw counter × node rate, rounded to an integer
/// for the durable traffic outbox consumed by the worker.
#[cfg(test)]
pub(super) fn charged_bytes(bytes: i64, rate: Decimal) -> Result<i64, ApiError> {
    Decimal::from(bytes)
        .checked_mul(rate)
        .and_then(|value| {
            value
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_i64()
        })
        .ok_or_else(|| {
            ApiError::bad_request("Server traffic charge is outside the supported range")
        })
}

#[cfg(test)]
mod admission_tests {
    use axum::{http::StatusCode, response::IntoResponse};
    use v2board_application::server_runtime::PersistTrafficError;

    use super::persist_traffic_error;

    #[test]
    fn traffic_hard_capacity_refusals_are_retryable_service_unavailable() {
        let response =
            persist_traffic_error(PersistTrafficError::AnalyticsUnavailable).into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn traffic_soft_pressure_uses_rate_limit_status() {
        let response =
            persist_traffic_error(PersistTrafficError::AnalyticsRateLimited).into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn admission_integrity_failures_are_not_mislabeled_as_capacity_pressure() {
        let response =
            persist_traffic_error(PersistTrafficError::AnalyticsEventInvalid).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
