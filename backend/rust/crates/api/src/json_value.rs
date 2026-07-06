pub(super) fn value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        serde_json::Value::Array(values) => values.first().and_then(value_to_string),
        serde_json::Value::Null | serde_json::Value::Object(_) => None,
    }
}

pub(super) fn value_to_i64(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(value) => value.as_i64(),
        serde_json::Value::String(value) => value.parse::<i64>().ok(),
        serde_json::Value::Bool(value) => Some(i64::from(*value)),
        serde_json::Value::Array(values) => values.first().and_then(value_to_i64),
        serde_json::Value::Null | serde_json::Value::Object(_) => None,
    }
}
