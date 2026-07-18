use super::*;

pub(in super::super) fn checked_gib_bytes(value: i64, field: &str) -> Result<i64, ApiError> {
    if value < 0 {
        return Err(ApiError::validation_field(
            field,
            "Traffic allowance must not be negative",
        ));
    }
    value.checked_mul(GIB).ok_or_else(|| {
        ApiError::validation_field(field, "Traffic allowance exceeds the supported range")
    })
}

pub(in super::super) fn csv_export(
    headers: &[&str],
    rows: impl IntoIterator<Item = Vec<String>>,
    include_utf8_bom: bool,
) -> Result<String, ApiError> {
    let mut writer = CsvExportWriter::new(headers, include_utf8_bom)?;
    for row in rows {
        writer.write_row(row)?;
    }
    writer.finish()
}

pub(in super::super) struct CsvExportWriter {
    writer: csv::Writer<Vec<u8>>,
    include_utf8_bom: bool,
}

impl CsvExportWriter {
    pub(in super::super) fn new(
        headers: &[&str],
        include_utf8_bom: bool,
    ) -> Result<Self, ApiError> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .terminator(csv::Terminator::CRLF)
            .from_writer(Vec::new());
        writer
            .write_record(headers)
            .map_err(|_| ApiError::internal("failed to write CSV header"))?;
        Ok(Self {
            writer,
            include_utf8_bom,
        })
    }

    pub(in super::super) fn write_row(&mut self, row: Vec<String>) -> Result<(), ApiError> {
        let safe_row = row
            .into_iter()
            .map(|value| neutralize_spreadsheet_formula(&value))
            .collect::<Vec<_>>();
        self.writer
            .write_record(safe_row)
            .map_err(|_| ApiError::internal("failed to write CSV row"))
    }

    pub(in super::super) fn finish(self) -> Result<String, ApiError> {
        let bytes = self
            .writer
            .into_inner()
            .map_err(|_| ApiError::internal("failed to finalize CSV export"))?;
        let body = String::from_utf8(bytes)
            .map_err(|_| ApiError::internal("CSV export was not valid UTF-8"))?;
        Ok(if self.include_utf8_bom {
            format!("\u{feff}{body}")
        } else {
            body
        })
    }
}

fn neutralize_spreadsheet_formula(value: &str) -> String {
    if matches!(
        value.trim_start().as_bytes().first(),
        Some(b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r')
    ) {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

pub(in super::super) fn normalize_admin_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

pub(in super::super) async fn fetch_json_list(
    db: &DbPool,
    sql: &str,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(in super::super) async fn fetch_json_list_bind(
    db: &DbPool,
    sql: &str,
    bind: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(in super::super) async fn fetch_json_list_page(
    db: &DbPool,
    sql: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(in super::super) async fn fetch_json_list_page_bind(
    db: &DbPool,
    sql: &str,
    bind: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(in super::super) async fn fetch_json_list_page_bind_text(
    db: &DbPool,
    sql: &str,
    bind: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

pub(in super::super) async fn fetch_json_one(
    db: &DbPool,
    sql: &str,
    bind: i64,
) -> Result<Option<Value>, ApiError> {
    let Some(row) = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .fetch_optional(db)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(row.0))
}

pub(in super::super) fn json_rows(rows: Vec<Json<Value>>) -> Vec<Value> {
    rows.into_iter().map(|row| row.0).collect()
}

pub(in super::super) fn required_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<String, ApiError> {
    params
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::business(format!("{key} cannot be empty")))
}

pub(in super::super) fn optional_i64(params: &HashMap<String, String>, key: &str) -> Option<i64> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| value.parse::<i64>().ok())
}

pub(in super::super) fn optional_decimal(
    params: &HashMap<String, String>,
    key: &str,
) -> Option<Decimal> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| value.parse::<Decimal>().ok())
}

pub(in super::super) fn required_i64(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<i64, ApiError> {
    optional_i64(params, key).ok_or_else(|| ApiError::business(format!("{key} cannot be empty")))
}

pub(in super::super) struct Pagination {
    pub limit: i64,
    pub offset: i64,
}

const MAX_PAGE_SIZE: i64 = 100;

pub(in super::super) fn page(params: &HashMap<String, String>) -> Result<Pagination, ApiError> {
    let current = parse_page_value(params.get("current"), "current", 1)?;
    let page_size = parse_page_value(
        params.get("pageSize").or_else(|| params.get("page_size")),
        "page_size",
        10,
    )?;
    if page_size > MAX_PAGE_SIZE {
        return Err(ApiError::validation_field(
            "page_size",
            "Page size must not exceed 100",
        ));
    }
    let offset = current
        .checked_sub(1)
        .and_then(|page| page.checked_mul(page_size))
        .ok_or_else(|| ApiError::validation_field("current", "Page offset is too large"))?;
    Ok(Pagination {
        limit: page_size,
        offset,
    })
}

fn parse_page_value(raw: Option<&String>, field: &str, default: i64) -> Result<i64, ApiError> {
    let Some(raw) = raw else {
        return Ok(default);
    };
    let value = raw
        .trim()
        .parse::<i64>()
        .map_err(|_| ApiError::validation_field(field, "Pagination value must be an integer"))?;
    if value < 1 {
        return Err(ApiError::validation_field(
            field,
            "Pagination value must be greater than zero",
        ));
    }
    Ok(value)
}

/// `ORDER BY` clause for admin list endpoints that accept `sort`/`sort_type`
/// (coupon/giftcard fetch), mirroring `Coupon::orderBy($sort, $sortType)`: the
/// direction is whitelisted to ASC/DESC (anything else, including a missing param,
/// falls back to DESC), and the column defaults to `id` when the param is absent or
/// empty. The column is backtick-wrapped with any backticks doubled, the same way
/// Laravel's query grammar quotes an identifier, so an unknown column produces a SQL
/// error rather than an injection point.
pub(in super::super) fn admin_sort_clause(params: &HashMap<String, String>) -> String {
    let direction = match params.get("sort_type").map(String::as_str) {
        Some("ASC") => "ASC",
        _ => "DESC",
    };
    // MySQL orders NULL before non-NULL for ASC and after it for DESC;
    // PostgreSQL defaults are the reverse. Pin the legacy list/pagination
    // contract explicitly for every allowed or operator-supplied column.
    let nulls = if direction == "ASC" {
        "NULLS FIRST"
    } else {
        "NULLS LAST"
    };
    let column = params
        .get("sort")
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("id");
    format!(
        "ORDER BY \"{}\" {direction} {nulls}",
        column.replace('"', "\"\"")
    )
}

pub(in super::super) fn array_param(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Vec<i64>, ApiError> {
    let mut values = BTreeMap::<usize, i64>::new();
    for (raw_key, raw_value) in params {
        if let Some(index) = bracket_index(raw_key, key)
            && let Ok(value) = raw_value.parse::<i64>()
        {
            values.insert(index, value);
        }
    }
    if let Some(value) = params.get(key)
        && let Ok(parsed) = serde_json::from_str::<Vec<i64>>(value)
    {
        return Ok(parsed);
    }
    let values = values.into_values().collect::<Vec<_>>();
    if values.is_empty() {
        return Err(ApiError::business("参数有误"));
    }
    Ok(values)
}

pub(in super::super) fn json_array_param(
    params: &HashMap<String, String>,
    key: &str,
) -> Vec<Value> {
    let mut values = BTreeMap::<usize, Value>::new();
    for (raw_key, raw_value) in params {
        if let Some(index) = bracket_index(raw_key, key) {
            values.insert(index, json_scalar(raw_value));
        }
    }
    values.into_values().collect()
}

pub(in super::super) fn bracket_index(raw_key: &str, key: &str) -> Option<usize> {
    raw_key
        .strip_prefix(&format!("{key}["))
        .and_then(|value| value.strip_suffix(']'))
        .and_then(|value| value.parse::<usize>().ok())
}

pub(in super::super) fn nested_json(params: &HashMap<String, String>, key: &str) -> Value {
    let mut root = Value::Object(Map::new());
    for (raw_key, raw_value) in params {
        if let Some(path) = bracket_path(raw_key, key) {
            insert_nested_json(&mut root, &path, json_scalar(raw_value));
        }
    }
    if matches!(&root, Value::Object(object) if object.is_empty())
        && let Some(value) = params.get(key)
        && let Ok(parsed) = serde_json::from_str::<Value>(value)
    {
        return parsed;
    }
    root
}

pub(in super::super) fn bracket_path(raw_key: &str, key: &str) -> Option<Vec<String>> {
    let mut rest = raw_key.strip_prefix(key)?;
    if rest.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    while let Some(value) = rest.strip_prefix('[') {
        let (part, tail) = value.split_once(']')?;
        parts.push(part.to_string());
        rest = tail;
    }
    (rest.is_empty() && !parts.is_empty()).then_some(parts)
}

pub(in super::super) fn insert_nested_json(root: &mut Value, path: &[String], value: Value) {
    let Some((head, tail)) = path.split_first() else {
        *root = value;
        return;
    };
    if tail.is_empty() {
        if let Value::Object(object) = root {
            object.insert(head.clone(), value);
        }
        return;
    }
    if !root.is_object() {
        *root = Value::Object(Map::new());
    }
    let Value::Object(object) = root else {
        return;
    };
    let child = object
        .entry(head.clone())
        .or_insert_with(|| Value::Object(Map::new()));
    insert_nested_json(child, tail, value);
}

pub(in super::super) fn json_scalar(value: &str) -> Value {
    if value.eq_ignore_ascii_case("null") {
        Value::Null
    } else if value == "true" {
        Value::Bool(true)
    } else if value == "false" {
        Value::Bool(false)
    } else if let Ok(value) = value.parse::<i64>() {
        json!(value)
    } else if let Ok(value) = value.parse::<f64>()
        && value.is_finite()
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string()))
    } else {
        json!(value)
    }
}

pub(in super::super) fn json_string(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json::Value is always serializable")
}

pub(in super::super) fn truthy(value: Option<&String>) -> bool {
    matches!(
        value.map(String::as_str),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

pub(in super::super) fn random_payment_uuid() -> String {
    Uuid::new_v4().simple().to_string()
}

pub(in super::super) fn random_token() -> String {
    Uuid::new_v4().simple().to_string()
}

pub(in super::super) fn is_server_path(path: &str, action: &str) -> bool {
    path.starts_with("server/") && path.ends_with(&format!("/{action}"))
}

pub(in super::super) fn server_table_from_path(path: &str) -> Result<&'static str, ApiError> {
    let kind = server_kind_from_path(path)?;
    SERVER_TABLES
        .iter()
        .find(|(item, _)| *item == kind)
        .map(|(_, table)| *table)
        .ok_or_else(|| ApiError::business("Invalid server type"))
}

pub(in super::super) fn server_kind_from_path(path: &str) -> Result<&str, ApiError> {
    let mut parts = path.split('/');
    let _server = parts.next();
    parts
        .next()
        .ok_or_else(|| ApiError::business("Invalid server type"))
}

pub(in super::super) fn ensure_safe_table(table: &str) -> Result<(), ApiError> {
    let allowed = [
        "plan",
        "payment_method",
        "notice",
        "knowledge",
        "coupon",
        "gift_card",
        "server_group",
        "server_route",
        "users",
        "server_shadowsocks",
        "server_vmess",
        "server_trojan",
        "server_tuic",
        "server_vless",
        "server_hysteria",
        "server_anytls",
        "server_v2node",
    ];
    if allowed.contains(&table) {
        Ok(())
    } else {
        Err(ApiError::business("Invalid table"))
    }
}

pub(in super::super) fn ensure_toggle_column(column: &str) -> Result<(), ApiError> {
    if matches!(column, "show" | "enable") {
        Ok(())
    } else {
        Err(ApiError::business("Invalid column"))
    }
}
