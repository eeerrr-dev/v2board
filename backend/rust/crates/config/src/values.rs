use std::{collections::BTreeSet, io};

use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::{Map, Value};

use crate::{
    keys::{
        BOOT_ONLY_CONFIGURATION_SCOPE, BOOT_ONLY_RUNTIME_KEYS_V1, CONFIGURATION_SCOPE_KEY,
        CONFIGURATION_SOURCE_KEY, FILE_ONLY_CONFIGURATION_SOURCE, FILE_ONLY_RUNTIME_KEYS_V1,
        MAX_CONFIG_DURATION_MINUTES, OPERATOR_AUTHORITY_MARKER, OPERATOR_CONFIG_KEYS_V1,
    },
    runtime::{ConfigParseMode, RuntimeRole},
    validation::{env_opt, invalid_setting},
};

pub(crate) fn config_or_env(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
) -> Option<String> {
    if operator_value_is_authoritative(config, config_key) {
        return config
            .get(config_key)
            .and_then(config_value_string)
            .filter(|value| !value.is_empty());
    }
    environment_value(config, env_key).or_else(|| {
        config
            .get(config_key)
            .and_then(config_value_string)
            .filter(|value| !value.is_empty())
    })
}

pub(crate) fn config_value_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

pub(crate) fn config_bool(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: bool,
) -> bool {
    config_or_env(config, config_key, env_key)
        .as_deref()
        .map(parse_bool)
        .unwrap_or(default)
}

pub(crate) fn config_i32(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: i32,
) -> i32 {
    config_or_env(config, config_key, env_key)
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(default)
}

pub(crate) fn config_i64(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: i64,
) -> i64 {
    config_or_env(config, config_key, env_key)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

pub(crate) fn config_decimal(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: Decimal,
) -> io::Result<Decimal> {
    let Some(value) = config_or_env(config, config_key, env_key) else {
        return Ok(default);
    };
    value
        .parse::<Decimal>()
        .map_err(|_| invalid_setting(config_key, "must be a finite decimal number"))
}

pub(crate) fn config_duration_minutes(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: i64,
) -> io::Result<i64> {
    let value = match config_or_env(config, config_key, env_key) {
        Some(value) => value.parse::<i64>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{config_key} must be an integer number of minutes"),
            )
        })?,
        None => default,
    };
    if !(1..=MAX_CONFIG_DURATION_MINUTES).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{config_key} must be between 1 and {MAX_CONFIG_DURATION_MINUTES} minutes"),
        ));
    }
    Ok(value)
}

pub(crate) fn config_list(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    default: &[&str],
) -> Vec<String> {
    if !operator_value_is_authoritative(config, config_key)
        && let Some(value) = environment_value(config, env_key)
    {
        return parse_list(&value);
    }
    if let Some(value) = config.get(config_key) {
        return match value {
            // An explicit empty JSON array means empty. Treating it as missing
            // would silently resurrect built-in operator defaults after a
            // provision spec deliberately disabled every item.
            Value::Array(items) => items.iter().filter_map(config_value_string).collect(),
            value => config_value_string(value)
                .map(|value| parse_list(&value))
                .unwrap_or_default(),
        };
    }
    default.iter().map(|item| (*item).to_string()).collect()
}

pub(crate) fn operator_value_is_authoritative(config: &Map<String, Value>, key: &str) -> bool {
    config
        .get(OPERATOR_AUTHORITY_MARKER)
        .and_then(Value::as_bool)
        == Some(true)
        && OPERATOR_CONFIG_KEYS_V1.contains(&key)
}

pub(crate) fn environment_value(config: &Map<String, Value>, env_key: &str) -> Option<String> {
    (!configuration_is_file_only(config))
        .then(|| env_opt(env_key))
        .flatten()
}

pub(crate) fn configuration_is_file_only(config: &Map<String, Value>) -> bool {
    config.get(CONFIGURATION_SOURCE_KEY).and_then(Value::as_str)
        == Some(FILE_ONLY_CONFIGURATION_SOURCE)
}

pub(crate) fn configuration_is_boot_only(config: &Map<String, Value>) -> bool {
    configuration_is_file_only(config)
        && config.get(CONFIGURATION_SCOPE_KEY).and_then(Value::as_str)
            == Some(BOOT_ONLY_CONFIGURATION_SCOPE)
}

pub(crate) fn validate_configuration_source(
    config: &Map<String, Value>,
    runtime_role: RuntimeRole,
    parse_mode: ConfigParseMode,
) -> io::Result<()> {
    match config.get(CONFIGURATION_SOURCE_KEY) {
        None => Ok(()),
        Some(Value::String(value)) if value == FILE_ONLY_CONFIGURATION_SOURCE => {
            if config.get("runtime_role").and_then(Value::as_str) != Some(runtime_role.file_value())
            {
                return Err(invalid_setting(
                    "runtime_role",
                    &format!(
                        "must be the exact string {} for this process",
                        runtime_role.file_value()
                    ),
                ));
            }
            let boot_only = config.get(CONFIGURATION_SCOPE_KEY).and_then(Value::as_str)
                == Some(BOOT_ONLY_CONFIGURATION_SCOPE);
            match parse_mode {
                ConfigParseMode::BootOnly if !boot_only => {
                    return Err(invalid_setting(
                        CONFIGURATION_SCOPE_KEY,
                        "must be the exact string boot_only for the boot parser",
                    ));
                }
                ConfigParseMode::FullRuntime if config.contains_key(CONFIGURATION_SCOPE_KEY) => {
                    return Err(invalid_setting(
                        CONFIGURATION_SCOPE_KEY,
                        "is not supported by the full-runtime parser",
                    ));
                }
                ConfigParseMode::OperatorAuthority
                | ConfigParseMode::BootOnly
                | ConfigParseMode::FullRuntime => {}
            }
            let mut expected = if boot_only {
                BOOT_ONLY_RUNTIME_KEYS_V1
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>()
            } else {
                FILE_ONLY_RUNTIME_KEYS_V1
                    .iter()
                    .copied()
                    .chain([
                        "runtime_role",
                        "database_url",
                        "peer_database_principal",
                        "redis_url",
                    ])
                    .collect::<BTreeSet<_>>()
            };
            if parse_mode.is_operator_authority() && boot_only {
                expected.extend(OPERATOR_CONFIG_KEYS_V1.iter().copied());
            }
            if runtime_role == RuntimeRole::Worker {
                expected.extend([
                    "clickhouse_url",
                    "clickhouse_database",
                    "clickhouse_writer_username",
                    "clickhouse_writer_password",
                ]);
            }
            let mut actual = config.keys().map(String::as_str).collect::<BTreeSet<_>>();
            if parse_mode.is_operator_authority() {
                actual.remove(OPERATOR_AUTHORITY_MARKER);
            }
            let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(invalid_setting(
                    CONFIGURATION_SOURCE_KEY,
                    &format!("file_only document is missing keys: {}", missing.join(", ")),
                ));
            }
            let unknown = actual.difference(&expected).copied().collect::<Vec<_>>();
            if !unknown.is_empty() {
                return Err(invalid_setting(
                    CONFIGURATION_SOURCE_KEY,
                    &format!(
                        "file_only document contains unsupported keys: {}",
                        unknown.join(", ")
                    ),
                ));
            }
            Ok(())
        }
        Some(_) => Err(invalid_setting(
            CONFIGURATION_SOURCE_KEY,
            "must be the exact string file_only when present",
        )),
    }
}

fn parse_bool(value: &str) -> bool {
    parse_bool_strict(value).unwrap_or(false)
}

pub(crate) fn parse_bool_strict(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub(crate) fn parse_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// `"amount:bonus"` yuan tiers → the best reward (in cents) whose threshold `total_amount`
/// (cents) has reached. Decimal arithmetic preserves the established truncation to whole cents
/// without binary floating-point underflow at values such as 0.29 yuan.
pub(crate) fn deposit_bonus_from_tiers(tiers: &[String], total_amount: i32) -> i32 {
    let mut bonus = 0;
    for tier in tiers {
        let Some((amount, value)) = tier.split_once(':') else {
            continue;
        };
        let (Some(amount), Some(value)) = (yuan_to_cents(amount), yuan_to_cents(value)) else {
            continue;
        };
        if total_amount >= amount {
            bonus = bonus.max(value);
        }
    }
    bonus
}

fn yuan_to_cents(value: &str) -> Option<i32> {
    let value = value.trim().parse::<Decimal>().ok()?;
    if value.is_sign_negative() {
        return None;
    }
    (value * Decimal::from(100)).trunc().to_i32()
}
