use std::collections::{BTreeMap, BTreeSet, HashMap};

use axum::{
    Json,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::{Datelike, TimeZone, Utc};
use redis::AsyncCommands;
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, QueryBuilder, Transaction};
use uuid::Uuid;
use v2board_analytics::{
    AnalyticsEvent, IdentityKind, ReportedTrafficEvent, TrafficEventCore, enqueue_events,
};
use v2board_compat::ApiError;

use crate::{json_value::value_to_i64, runtime::AppState};

use super::{
    ParsedTrafficEntries, ServerNodeRow, TrafficEntry,
    config::parse_i32_json_list,
    repository::{load_server_node, load_uniproxy_node},
    request::required_i32_param,
};

const TRAFFIC_REPORT_SQL_BATCH_SIZE: usize = 500;

#[derive(Debug, Clone)]
struct AcceptedTrafficItem {
    user_id: i64,
    traffic_epoch: i64,
    raw_u: i64,
    raw_d: i64,
    charged_u: i64,
    charged_d: i64,
}

// Merge every user's per-node alive-IP bucket in Redis itself. Reports from
// different nodes can race; a client-side GET/SET loop loses one of those
// updates and also costs two network round trips per user. This bounded script
// makes the merge atomic and sends the whole report once.
pub(super) const ALIVE_CACHE_UPDATE_SCRIPT: &str = r#"
local node_bucket = ARGV[1]
local now = tonumber(ARGV[2])
local device_limit_mode = tonumber(ARGV[3])

for index, key in ipairs(KEYS) do
    local value = {}
    local current = redis.call('GET', key)
    if current then
        local ok, decoded = pcall(cjson.decode, current)
        if ok and type(decoded) == 'table' then
            value = decoded
        end
    end

    local ok, aliveips = pcall(cjson.decode, ARGV[index + 3])
    if not ok or type(aliveips) ~= 'table' then
        return redis.error_reply('invalid alive-IP payload')
    end
    value[node_bucket] = { aliveips = aliveips, lastupdateAt = now }

    local stale = {}
    for bucket, node in pairs(value) do
        if bucket ~= 'alive_ip' then
            local last_update = 0
            if type(node) == 'table' then
                last_update = tonumber(node.lastupdateAt) or 0
            end
            if now - last_update > 100 then
                table.insert(stale, bucket)
            end
        end
    end
    for _, bucket in ipairs(stale) do
        value[bucket] = nil
    end

    local alive_count = 0
    if device_limit_mode == 1 then
        local unique = {}
        for bucket, node in pairs(value) do
            if bucket ~= 'alive_ip' and type(node) == 'table' and type(node.aliveips) == 'table' then
                for _, ip_node in ipairs(node.aliveips) do
                    if type(ip_node) == 'string' then
                        local separator = string.find(ip_node, '_', 1, true)
                        local ip = separator and string.sub(ip_node, 1, separator - 1) or ip_node
                        unique[ip] = true
                    end
                end
            end
        end
        for _ in pairs(unique) do
            alive_count = alive_count + 1
        end
    else
        for bucket, node in pairs(value) do
            if bucket ~= 'alive_ip' and type(node) == 'table' and type(node.aliveips) == 'table' then
                alive_count = alive_count + #node.aliveips
            end
        end
    end

    value.alive_ip = alive_count
    redis.call('SET', key, cjson.encode(value), 'EX', 120)
end

return #KEYS
"#;

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
        let node_type = fallback_node_type.unwrap_or("shadowsocks").to_string();
        let node_id = required_i32_param(params, "node_id")?;
        let Some(node) = load_server_node(&state.db, &node_type, node_id).await? else {
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
            node_type,
            ignored_rows = parsed.ignored_rows,
            defaulted_counters = parsed.defaulted_counters,
            "legacy traffic payload required compatibility coercion"
        );
    }
    let entries = parsed.entries;
    server_cache_count(
        &state.redis,
        "ONLINE_USER",
        &node_type,
        node.id,
        entries.len() as i64,
    )
    .await?;
    server_cache_timestamp(&state.redis, "LAST_PUSH_AT", &node_type, node.id).await?;
    if !entries.is_empty() {
        // Success means the complete report (charged deltas + daily statistics) is
        // durably committed. The worker applies user counters exactly once. Older
        // nodes without a client id receive a server-generated report identity;
        // they cannot get retry deduplication across a lost HTTP response, but they
        // no longer get a false 200 after partial Redis/SQL persistence.
        persist_traffic_fetch(state, &node, &node_type, &entries, report_token.as_deref()).await?;
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
    let user_ids = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM users
        WHERE CAST(u AS DECIMAL(30,0)) + CAST(d AS DECIMAL(30,0))
              < CAST(transfer_enable AS DECIMAL(30,0))
          AND (expired_at >= $1 OR expired_at IS NULL)
          AND banned = 0
          AND device_limit > 0
        "#,
    )
    .bind(Utc::now().timestamp())
    .fetch_all(&state.db)
    .await?;

    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let mut alive = serde_json::Map::new();
    let keys = user_ids
        .iter()
        .map(|user_id| format!("ALIVE_IP_USER_{user_id}"))
        .collect::<Vec<_>>();
    let cached = conn.mget::<_, Vec<Option<String>>>(&keys).await?;
    for (user_id, value) in user_ids.into_iter().zip(cached) {
        if let Some(value) = value
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&value)
            && let Some(alive_ip) = value.get("alive_ip").and_then(value_to_i64)
        {
            alive.insert(user_id.to_string(), json!(alive_ip));
        }
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
    let node_bucket = format!("{node_type}{}", node.id);
    let mut updates = Vec::with_capacity(object.len());
    for (uid, ips) in object {
        let Ok(user_id) = uid.parse::<i64>() else {
            continue;
        };
        let Some(ips) = ips.as_array() else {
            continue;
        };
        updates.push((
            format!("ALIVE_IP_USER_{user_id}"),
            serde_json::to_string(ips)
                .expect("an alive-IP serde_json::Value array is always serializable"),
        ));
    }
    if !updates.is_empty() {
        let mut conn = state.redis.get_multiplexed_async_connection().await?;
        let script = redis::Script::new(ALIVE_CACHE_UPDATE_SCRIPT);
        let mut invocation = script.prepare_invoke();
        for (key, _) in &updates {
            invocation.key(key);
        }
        invocation.arg(node_bucket).arg(now).arg(device_limit_mode);
        for (_, ips) in &updates {
            invocation.arg(ips);
        }
        let updated = invocation.invoke_async::<i64>(&mut conn).await?;
        let expected = i64::try_from(updates.len()).map_err(|_| {
            ApiError::internal("Alive-IP update count is outside the supported range")
        })?;
        if updated != expected {
            return Err(ApiError::internal(
                "Alive-IP cache update returned an unexpected count",
            ));
        }
    }
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
    redis: &redis::Client,
    suffix: &str,
    node_type: &str,
    node_id: i32,
) -> Result<(), ApiError> {
    server_cache_count(redis, suffix, node_type, node_id, Utc::now().timestamp()).await
}

async fn server_cache_count(
    redis: &redis::Client,
    suffix: &str,
    node_type: &str,
    node_id: i32,
    value: i64,
) -> Result<(), ApiError> {
    let key = format!(
        "SERVER_{}_{}_{node_id}",
        node_type.to_ascii_uppercase(),
        suffix
    );
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let _: () = conn.set_ex(key, value, 3600).await?;
    Ok(())
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
    node_type: &str,
    entries: &[TrafficEntry],
    report_token: Option<&str>,
) -> Result<(), ApiError> {
    let rate = parse_server_rate(&node.rate);
    let report_key = report_token.map_or_else(
        || implicit_traffic_report_key(node_type, node.id),
        |token| traffic_report_key(node_type, node.id, token),
    );
    persist_durable_traffic_report(state, node, node_type, entries, rate, &report_key).await
}

async fn persist_durable_traffic_report(
    state: &AppState,
    node: &ServerNodeRow,
    node_type: &str,
    entries: &[TrafficEntry],
    rate: Decimal,
    report_key: &str,
) -> Result<(), ApiError> {
    let payload_hash = traffic_report_payload_hash(node.id, &node.rate, node_type, entries);
    let now = Utc::now().timestamp();
    let accounting_date = v2board_config::app_now().date_naive();
    let identity_kind = if is_internal_traffic_report_key(report_key) {
        IdentityKind::Implicit
    } else {
        IdentityKind::Explicit
    };
    let rate_text = canonical_rate_text(&node.rate, rate);
    let rate_decimal_10_2 = rate_decimal_10_2(rate)?;
    let mut tx = state.db.begin().await?;
    let inserted = match sqlx::query(
        r#"
        INSERT INTO server_traffic_report
            (report_key, payload_hash, node_id, node_type, rate_text, rate_decimal_10_2,
             identity_kind, accepted_at, accounting_date, applied_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)
        "#,
    )
    .bind(report_key)
    .bind(&payload_hash)
    .bind(node.id)
    .bind(node_type)
    .bind(&rate_text)
    .bind(rate_decimal_10_2)
    .bind(identity_kind_db_value(identity_kind))
    .bind(now)
    .bind(accounting_date)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    {
        Ok(_) => true,
        Err(error)
            if error
                .as_database_error()
                .is_some_and(|error| error.is_unique_violation()) =>
        {
            false
        }
        Err(error) => return Err(error.into()),
    };
    if !inserted {
        let existing_hash: String = sqlx::query_scalar(
            "SELECT payload_hash FROM server_traffic_report WHERE report_key = $1 FOR UPDATE",
        )
        .bind(report_key)
        .fetch_one(&mut *tx)
        .await?;
        if existing_hash != payload_hash {
            return Err(ApiError::bad_request(
                "Traffic report idempotency key was reused with a different payload",
            ));
        }
        tx.commit().await?;
        return Ok(());
    }

    // The user rows are the serialization point between report acceptance and
    // every subscription mutation that resets traffic. Capturing the epoch
    // while those rows are locked gives each item one unambiguous quota period.
    // It also prevents a compromised node from charging users outside the
    // groups currently assigned to that node.
    let epochs = lock_report_users(&mut tx, node, entries).await?;
    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        let epoch = *epochs
            .get(&entry.user_id)
            .ok_or_else(|| ApiError::bad_request("Traffic report contains an unauthorized user"))?;
        items.push(AcceptedTrafficItem {
            user_id: entry.user_id,
            traffic_epoch: epoch,
            raw_u: entry.u,
            raw_d: entry.d,
            charged_u: charged_bytes(entry.u, rate)?,
            charged_d: charged_bytes(entry.d, rate)?,
        });
    }
    for chunk in items.chunks(TRAFFIC_REPORT_SQL_BATCH_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO server_traffic_report_item \
             (report_key, user_id, traffic_epoch, raw_u, raw_d, charged_u, charged_d) ",
        );
        builder.push_values(chunk, |mut row, item| {
            row.push_bind(report_key)
                .push_bind(item.user_id)
                .push_bind(item.traffic_epoch)
                .push_bind(item.raw_u)
                .push_bind(item.raw_d)
                .push_bind(item.charged_u)
                .push_bind(item.charged_d);
        });
        builder.build().execute(&mut *tx).await?;
    }
    let rate_decimal_text = decimal_with_scale(rate_decimal_10_2, 2);
    let mut analytics_events = Vec::<AnalyticsEvent>::with_capacity(items.len());
    for item in &items {
        let core = TrafficEventCore {
            installation_id: state.installation_id.to_string(),
            report_key: report_key.to_owned(),
            payload_hash: payload_hash.clone(),
            identity_kind,
            user_id: item.user_id.to_string(),
            traffic_epoch: item.traffic_epoch.to_string(),
            server_id: node.id.to_string(),
            server_type: node_type.to_owned(),
            rate_text: rate_text.clone(),
            rate_decimal_10_2: rate_decimal_text.clone(),
            raw_u: item.raw_u.to_string(),
            raw_d: item.raw_d.to_string(),
            charged_u: item.charged_u.to_string(),
            charged_d: item.charged_d.to_string(),
            accepted_at: now,
            accounting_date: accounting_date.format("%Y-%m-%d").to_string(),
            accounting_timezone: "Asia/Shanghai".to_owned(),
        };
        let event = ReportedTrafficEvent::new(core)
            .and_then(ReportedTrafficEvent::into_outbox)
            .map_err(traffic_analytics_event_error)?;
        analytics_events.push(event);
    }
    enqueue_events(&mut tx, &analytics_events, now)
        .await
        .map_err(traffic_analytics_outbox_error)?;
    persist_traffic_stats(&mut tx, node, node_type, entries, rate).await?;
    tx.commit().await?;
    Ok(())
}

fn is_internal_traffic_report_key(report_key: &str) -> bool {
    report_key.starts_with("i-")
}

fn identity_kind_db_value(identity_kind: IdentityKind) -> &'static str {
    match identity_kind {
        IdentityKind::Explicit => "explicit",
        IdentityKind::Implicit => "implicit",
    }
}

fn canonical_rate_text(raw: &str, parsed: Decimal) -> String {
    if raw.trim().parse::<Decimal>().is_ok() {
        raw.trim().to_owned()
    } else {
        parsed.normalize().to_string()
    }
}

fn rate_decimal_10_2(rate: Decimal) -> Result<Decimal, ApiError> {
    let rounded = rate.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    let maximum = Decimal::new(9_999_999_999, 2);
    if rounded.is_sign_negative() || rounded > maximum {
        return Err(ApiError::bad_request(
            "Server traffic rate is outside the supported range",
        ));
    }
    Ok(rounded)
}

fn decimal_with_scale(mut value: Decimal, scale: u32) -> String {
    value.rescale(scale);
    value.to_string()
}

fn traffic_analytics_event_error(error: v2board_analytics::EventValidationError) -> ApiError {
    tracing::error!(
        ?error,
        "refusing to persist an invalid traffic analytics event"
    );
    ApiError::internal("failed to persist traffic analytics event")
}

fn traffic_analytics_outbox_error(error: v2board_analytics::OutboxError) -> ApiError {
    use v2board_analytics::{AnalyticsAdmissionError, OutboxError};

    match error {
        OutboxError::Admission(error @ AnalyticsAdmissionError::SoftRateLimited) => {
            tracing::warn!(?error, "traffic analytics soft-pressure rate limit reached");
            ApiError::too_many_requests(
                "Traffic ingestion is temporarily rate limited; retry later",
            )
        }
        OutboxError::Admission(
            error @ (AnalyticsAdmissionError::HardStop
            | AnalyticsAdmissionError::MissingOrMismatchedPolicy
            | AnalyticsAdmissionError::InvalidState),
        ) => {
            tracing::warn!(
                ?error,
                "traffic analytics admission refused the transaction"
            );
            ApiError::service_unavailable(
                "Traffic ingestion is temporarily unavailable; retry later",
            )
        }
        error => {
            tracing::error!(?error, "failed to enqueue the traffic analytics event");
            ApiError::internal("failed to persist traffic analytics event")
        }
    }
}

async fn lock_report_users(
    tx: &mut Transaction<'_, Postgres>,
    node: &ServerNodeRow,
    entries: &[TrafficEntry],
) -> Result<BTreeMap<i64, i64>, ApiError> {
    let user_ids = entries
        .iter()
        .map(|entry| entry.user_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if user_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let group_ids = parse_i32_json_list(Some(&node.group_id));
    if group_ids.is_empty() {
        return Err(ApiError::bad_request(
            "Traffic report contains an unauthorized user",
        ));
    }

    let mut epochs = BTreeMap::new();
    for user_chunk in user_ids.chunks(TRAFFIC_REPORT_SQL_BATCH_SIZE) {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT id, traffic_epoch FROM users WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for user_id in user_chunk {
                separated.push_bind(*user_id);
            }
        }
        builder.push(") AND group_id IN (");
        {
            let mut separated = builder.separated(", ");
            for group_id in &group_ids {
                separated.push_bind(*group_id);
            }
        }
        builder.push(") ORDER BY id FOR UPDATE");
        for (user_id, epoch) in builder
            .build_query_as::<(i64, i64)>()
            .fetch_all(&mut **tx)
            .await?
        {
            epochs.insert(user_id, epoch);
        }
    }
    if epochs.len() != user_ids.len() {
        return Err(ApiError::bad_request(
            "Traffic report contains an unauthorized user",
        ));
    }
    Ok(epochs)
}

async fn persist_traffic_stats(
    tx: &mut Transaction<'_, Postgres>,
    node: &ServerNodeRow,
    node_type: &str,
    entries: &[TrafficEntry],
    rate: Decimal,
) -> Result<(), ApiError> {
    let record_at = today_start_timestamp();
    let now = Utc::now().timestamp();
    let mut total_u = 0_i64;
    let mut total_d = 0_i64;
    for entry in entries {
        (total_u, total_d) = checked_traffic_pair(total_u, total_d, entry.u, entry.d)?;
    }

    // A single row-alias upsert per fixed-size chunk replaces the former
    // SELECT + UPDATE/INSERT round trip for every user.  The unique statistics
    // key serializes concurrent node reports, while strict PostgreSQL arithmetic
    // rejects rather than wraps a signed BIGINT overflow.
    for chunk in entries.chunks(TRAFFIC_REPORT_SQL_BATCH_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO user_traffic \
             (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at) ",
        );
        builder.push_values(chunk, |mut row, entry| {
            row.push_bind(entry.user_id)
                .push_bind(rate)
                .push_bind(entry.u)
                .push_bind(entry.d)
                .push_bind("d")
                .push_bind(record_at)
                .push_bind(now)
                .push_bind(now);
        });
        builder.push(
            " ON CONFLICT (server_rate, user_id, record_at) DO UPDATE SET \
             u = user_traffic.u + EXCLUDED.u, \
             d = user_traffic.d + EXCLUDED.d, \
             updated_at = EXCLUDED.updated_at",
        );
        builder
            .build()
            .execute(&mut **tx)
            .await
            .map_err(traffic_stat_write_error)?;
    }

    sqlx::query(
        r#"
        INSERT INTO server_traffic
            (server_id, server_type, u, d, record_type, record_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, 'd', $5, $6, $7)
        ON CONFLICT (server_id, server_type, record_at) DO UPDATE SET
            u = server_traffic.u + EXCLUDED.u,
            d = server_traffic.d + EXCLUDED.d,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(node.id)
    .bind(node_type)
    .bind(total_u)
    .bind(total_d)
    .bind(record_at)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(traffic_stat_write_error)?;
    Ok(())
}

fn traffic_stat_write_error(error: sqlx::Error) -> ApiError {
    let is_overflow = error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code.as_ref() == "22003");
    if is_overflow {
        ApiError::bad_request("Server traffic total is outside the supported range")
    } else {
        ApiError::Database(error)
    }
}

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
pub(super) fn parse_server_rate(rate: &str) -> Decimal {
    rate.parse::<Decimal>().unwrap_or(Decimal::ZERO)
}

/// Charged bytes billed against a user's quota: raw counter × node rate, rounded to an integer
/// for the durable traffic outbox consumed by the worker.
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
    use v2board_analytics::{AnalyticsAdmissionError, OutboxError};

    use super::traffic_analytics_outbox_error;

    #[test]
    fn traffic_hard_capacity_refusals_are_retryable_service_unavailable() {
        for error in [
            AnalyticsAdmissionError::HardStop,
            AnalyticsAdmissionError::MissingOrMismatchedPolicy,
            AnalyticsAdmissionError::InvalidState,
        ] {
            let response =
                traffic_analytics_outbox_error(OutboxError::Admission(error)).into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        }
    }

    #[test]
    fn traffic_soft_pressure_uses_rate_limit_status() {
        let response = traffic_analytics_outbox_error(OutboxError::Admission(
            AnalyticsAdmissionError::SoftRateLimited,
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn admission_integrity_failures_are_not_mislabeled_as_capacity_pressure() {
        let response = traffic_analytics_outbox_error(OutboxError::Admission(
            AnalyticsAdmissionError::Overflow,
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
