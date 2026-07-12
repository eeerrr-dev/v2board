use super::*;

#[derive(Debug, FromRow)]
pub(in super::super) struct UserDumpRow {
    pub(in super::super) id: i64,
    pub(in super::super) email: String,
    pub(in super::super) balance: i32,
    pub(in super::super) commission_balance: i32,
    pub(in super::super) transfer_enable: i64,
    pub(in super::super) u: i64,
    pub(in super::super) d: i64,
    pub(in super::super) device_limit: Option<i32>,
    pub(in super::super) expired_at: Option<i64>,
    pub(in super::super) plan_name: Option<String>,
    pub(in super::super) token: String,
}

/// Parses an `ALIVE_IP_USER_<id>` cache payload into `(alive_ip, ips)`.
/// Mirrors UserController::fetch :89-102.
pub(in super::super) fn parse_alive_ip(raw: &str) -> (i64, String) {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return (0, String::new());
    };
    let Some(object) = value.as_object() else {
        return (0, String::new());
    };
    let alive_ip = object
        .get("alive_ip")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let mut ips = Vec::new();
    for (node_type_id, data) in object {
        if node_type_id == "alive_ip" {
            continue;
        }
        let Some(alive_ips) = data.get("aliveips").and_then(Value::as_array) else {
            continue;
        };
        for entry in alive_ips {
            let Some(entry) = entry.as_str() else {
                continue;
            };
            let ip = entry.split('_').next().unwrap_or_default();
            ips.push(format!("{ip}_{node_type_id}"));
        }
    }
    (alive_ip, ips.join(", "))
}

/// Random `[a-zA-Z0-9]` string of `len` chars. Ports Helper::randomChar.
pub(in super::super) fn random_char(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    (0..len)
        .map(|index| CHARS[(bytes[index] as usize) % CHARS.len()] as char)
        .collect()
}

/// PHP-compatible display time in the application's pinned timezone.
pub(in super::super) fn local_datetime(ts: i64) -> String {
    app_timezone()
        .timestamp_opt(ts, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Month/day display in the application's pinned timezone.
pub(in super::super) fn local_month_day(ts: i64) -> String {
    app_timezone()
        .timestamp_opt(ts, 0)
        .single()
        .map(|value| value.format("%m-%d").to_string())
        .unwrap_or_default()
}

/// Node availability status, ported from ServerService::mergeData :414-420.
pub(in super::super) fn node_available_status(
    now: i64,
    last_check_at: Option<i64>,
    last_push_at: Option<i64>,
) -> i64 {
    let stale_before = now.saturating_sub(300);
    if stale_before >= last_check_at.unwrap_or_default() {
        0
    } else if stale_before >= last_push_at.unwrap_or_default() {
        1
    } else {
        2
    }
}

/// Maps a `v2_stat_server.server_type` onto the canonical node-table key used
/// for name resolution. Legacy stats recorded vmess nodes as `v2ray`.
pub(in super::super) fn normalize_stat_server_type(server_type: &str) -> String {
    match server_type {
        "v2ray" => "vmess".to_string(),
        other => other.to_string(),
    }
}

/// True when a server's `group_id` JSON array contains `target` (loose match,
/// mirroring PHP `in_array` against string/int group ids).
pub(in super::super) fn group_id_contains(group_id_json: &str, target: i64) -> bool {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(group_id_json) else {
        return false;
    };
    let target_string = target.to_string();
    items.iter().any(|item| match item {
        Value::Number(number) => number.as_i64() == Some(target),
        Value::String(value) => value == &target_string,
        _ => false,
    })
}

pub(in super::super) fn first_day_of_month() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

pub(in super::super) fn first_day_of_previous_month() -> i64 {
    let now = app_now();
    let (year, month) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };
    app_timezone()
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

pub(in super::super) fn start_of_today() -> i64 {
    let now = app_now();
    app_timezone()
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

pub(in super::super) fn start_of_yesterday() -> i64 {
    start_of_today() - 86_400
}
