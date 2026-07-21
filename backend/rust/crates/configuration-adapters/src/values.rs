use std::{collections::BTreeMap, str::FromStr};

use serde_json::{Map, Number, Value};
use v2board_application::configuration::{
    ConfigurationMap, ConfigurationPortError, ConfigurationValue,
};

pub(crate) fn map_from_json(
    values: Map<String, Value>,
) -> Result<ConfigurationMap, ConfigurationPortError> {
    values
        .into_iter()
        .map(|(key, value)| Ok((key, value_from_json(value)?)))
        .collect()
}

pub(crate) fn map_to_json(
    values: &ConfigurationMap,
) -> Result<Map<String, Value>, ConfigurationPortError> {
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), value_to_json(value)?)))
        .collect()
}

pub(crate) fn groups_from_json(
    value: Value,
) -> Result<BTreeMap<String, ConfigurationMap>, ConfigurationPortError> {
    let groups = value.as_object().ok_or_else(|| {
        ConfigurationPortError::Internal("operator configuration view is not an object".to_string())
    })?;
    groups
        .iter()
        .map(|(name, value)| {
            let group = value.as_object().cloned().ok_or_else(|| {
                ConfigurationPortError::Internal(format!(
                    "operator configuration group {name} is not an object"
                ))
            })?;
            Ok((name.clone(), map_from_json(group)?))
        })
        .collect()
}

fn value_from_json(value: Value) -> Result<ConfigurationValue, ConfigurationPortError> {
    match value {
        Value::Null => Ok(ConfigurationValue::Null),
        Value::Bool(value) => Ok(ConfigurationValue::Bool(value)),
        Value::Number(value) => value
            .as_i64()
            .map(ConfigurationValue::Integer)
            .map_or_else(|| Ok(ConfigurationValue::Number(value.to_string())), Ok),
        Value::String(value) => Ok(ConfigurationValue::String(value)),
        Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                Value::String(value) => Ok(value),
                _ => Err(ConfigurationPortError::Internal(
                    "operator configuration contains a non-string list".to_string(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(ConfigurationValue::StringList),
        Value::Object(_) => Err(ConfigurationPortError::Internal(
            "nested operator configuration objects are unsupported".to_string(),
        )),
    }
}

fn value_to_json(value: &ConfigurationValue) -> Result<Value, ConfigurationPortError> {
    match value {
        ConfigurationValue::Null => Ok(Value::Null),
        ConfigurationValue::Bool(value) => Ok(Value::Bool(*value)),
        ConfigurationValue::Integer(value) => Ok(Value::Number(Number::from(*value))),
        ConfigurationValue::Number(value) => {
            Number::from_str(value).map(Value::Number).map_err(|_| {
                ConfigurationPortError::Validation {
                    detail: format!("invalid configuration number {value}"),
                    security: false,
                }
            })
        }
        ConfigurationValue::String(value) => Ok(Value::String(value.clone())),
        ConfigurationValue::StringList(values) => Ok(Value::Array(
            values.iter().cloned().map(Value::String).collect(),
        )),
    }
}
