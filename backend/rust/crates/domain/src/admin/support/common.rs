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

pub(in super::super) fn random_payment_uuid() -> String {
    Uuid::new_v4().simple().to_string()
}

pub(in super::super) fn random_token() -> String {
    Uuid::new_v4().simple().to_string()
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
