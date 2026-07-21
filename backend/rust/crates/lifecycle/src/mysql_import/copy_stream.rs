use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    time::Instant,
};

use anyhow::Context;
use futures_util::TryStreamExt;
use rust_decimal::Decimal;
use sqlx::{
    AssertSqlSafe, Column, MySqlConnection, PgPool, Row, SqlSafeStr, TypeInfo, ValueRef,
    mysql::MySqlRow,
};
use v2board_domain_model::PlanPricePeriod;
use v2board_provision::{
    mysql_import_converter::{
        CanonicalJson, CanonicalRow, CanonicalRowsHasher, CanonicalValue, DERIVED_MAPPINGS,
        DerivedMapping, DerivedMappingKind, LegacyGiftcardRedemptionRow, MysqlImportRowDisposition,
        SOURCE_ID_LOWER_BOUND, SourceRow, SourceValue, TABLE_MAPPINGS, TableMapping,
        copied_table_mappings, derived_target_copy_sql, expand_giftcard_redemptions,
        source_stream_sql, target_columns_in_order, target_copy_sql, transform_mysql_import_row,
    },
    mysql_import_policy::is_legacy_stripe_payment_driver,
};

use super::{execute::ImportedTableReport, mysql_source::mapping_has_source_column};

pub(crate) const COPY_SEND_BUFFER_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_COPY_ROW_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const COPY_PROGRESS_ROWS: u64 = 100_000;
pub(crate) const MAX_LEGACY_PAYMENT_METHODS: usize = 4_096;

pub(crate) async fn copy_business_data(
    source: &mut MySqlConnection,
    target: &PgPool,
    app_key: &str,
) -> anyhow::Result<Vec<ImportedTableReport>> {
    let mut known_payment_ids = BTreeSet::new();
    let mut stripe_payment_ids = BTreeSet::new();
    let mut reports = Vec::with_capacity(TABLE_MAPPINGS.len() + DERIVED_MAPPINGS.len());
    for mapping in copied_table_mappings() {
        // `audit_registry` proves that at most one derived COPY stream is
        // owned by a source table, so this lookup cannot silently skip a
        // second derived target while preserving the one-SELECT rule.
        if let Some(derived) = DERIVED_MAPPINGS
            .iter()
            .find(|derived| derived.source_tables.first() == Some(&mapping.source))
        {
            let (base, derived) = copy_base_and_derived(
                &mut *source,
                target,
                mapping,
                derived,
                app_key,
                &mut known_payment_ids,
                &mut stripe_payment_ids,
            )
            .await?;
            reports.push(base);
            reports.push(derived);
        } else {
            reports.push(
                copy_base_table(
                    &mut *source,
                    target,
                    mapping,
                    app_key,
                    &mut known_payment_ids,
                    &mut stripe_payment_ids,
                )
                .await?,
            );
        }
    }
    for derived in DERIVED_MAPPINGS {
        anyhow::ensure!(
            reports.iter().any(|report| report.target == derived.target),
            "converter registry did not execute derived target {}",
            derived.target
        );
    }
    Ok(reports)
}

async fn copy_base_table(
    source: &mut MySqlConnection,
    target: &PgPool,
    mapping: &TableMapping,
    app_key: &str,
    known_payment_ids: &mut BTreeSet<i32>,
    stripe_payment_ids: &mut BTreeSet<i32>,
) -> anyhow::Result<ImportedTableReport> {
    let sql = source_stream_sql(mapping)?;
    let copy_sql = target_copy_sql(mapping)?;
    let columns = target_columns_in_order(mapping);
    let mut target_connection = target.acquire().await?;
    target_connection.close_on_drop();
    let mut copy = target_connection.copy_in_raw(&copy_sql).await?;
    if !copy.is_textual()
        || copy.num_columns() != columns.len()
        || (0..copy.num_columns()).any(|index| !copy.column_is_textual(index))
    {
        let _ = copy.abort("v2board import COPY format mismatch").await;
        anyhow::bail!("PostgreSQL COPY format mismatch for {}", mapping.target);
    }

    let mut previous_id = SOURCE_ID_LOWER_BOUND;
    let mut source_rows = 0_u64;
    let mut discarded_rows = 0_u64;
    let mut retained = CanonicalRowsHasher::for_mapping(mapping)?;
    let mut buffer = Vec::with_capacity(COPY_SEND_BUFFER_BYTES);
    let mut bytes_sent = 0_u64;
    let started_at = Instant::now();
    let mut next_progress = COPY_PROGRESS_ROWS;

    let stream_result: anyhow::Result<()> = async {
        let mut rows = sqlx::query(AssertSqlSafe(sql).into_sql_str()).fetch(&mut *source);
        while let Some(row) = rows.try_next().await? {
            let source_row = decode_mysql_row(mapping, &row)?;
            previous_id = next_source_id(mapping, previous_id, &source_row)?;
            source_rows = source_rows
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("source row count overflow"))?;
            index_legacy_payment(mapping, &source_row, known_payment_ids, stripe_payment_ids)?;

            match transform_mysql_import_row(
                mapping,
                &source_row,
                &*known_payment_ids,
                &*stripe_payment_ids,
            )? {
                MysqlImportRowDisposition::Discard => {
                    discarded_rows = discarded_rows.checked_add(1).ok_or_else(|| {
                        anyhow::anyhow!("discarded row count overflow for {}", mapping.source)
                    })?;
                }
                MysqlImportRowDisposition::Retain(mut row) => {
                    encrypt_retained_payment_config(mapping, app_key, &mut row)?;
                    let encoded = encode_copy_row(mapping.target, &columns, &row)?;
                    retained.update_row(&row)?;
                    if !buffer.is_empty()
                        && buffer.len().saturating_add(encoded.len()) > COPY_SEND_BUFFER_BYTES
                    {
                        bytes_sent = bytes_sent
                            .checked_add(buffer.len() as u64)
                            .ok_or_else(|| anyhow::anyhow!("COPY byte count overflow"))?;
                        copy.send(std::mem::take(&mut buffer)).await?;
                        buffer = Vec::with_capacity(COPY_SEND_BUFFER_BYTES);
                    }
                    if encoded.len() > COPY_SEND_BUFFER_BYTES {
                        bytes_sent = bytes_sent
                            .checked_add(encoded.len() as u64)
                            .ok_or_else(|| anyhow::anyhow!("COPY byte count overflow"))?;
                        copy.send(encoded).await?;
                    } else {
                        buffer.extend_from_slice(&encoded);
                    }
                }
            }
            if source_rows >= next_progress {
                report_copy_progress(mapping.target, source_rows, bytes_sent, started_at);
                next_progress = next_progress.saturating_add(COPY_PROGRESS_ROWS);
            }
        }
        drop(rows);
        if !buffer.is_empty() {
            bytes_sent = bytes_sent
                .checked_add(buffer.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("COPY byte count overflow"))?;
            copy.send(std::mem::take(&mut buffer)).await?;
        }
        Ok(())
    }
    .await;
    if let Err(error) = stream_result {
        let _ = copy.abort("v2board import COPY stream failed").await;
        return Err(error.context(format!("COPY {} -> {}", mapping.source, mapping.target)));
    }

    let (retained_rows, retained_sha256) = retained.finish();
    let copied_rows = copy.finish().await?;
    if copied_rows != retained_rows {
        anyhow::bail!(
            "PostgreSQL COPY row count mismatch for {}: expected {retained_rows}, observed {copied_rows}",
            mapping.target
        );
    }
    report_copy_progress(mapping.target, source_rows, bytes_sent, started_at);
    Ok(ImportedTableReport {
        source: mapping.source.to_string(),
        target: mapping.target.to_string(),
        source_rows,
        retained_rows,
        discarded_rows,
        retained_sha256,
    })
}

/// Seals the retained payment row's gateway config into the runtime at-rest
/// AES-256-GCM envelope before it is hashed and COPY-encoded, so the canonical
/// expectation, the stored JSONB column, and the post-COPY verification scan
/// all agree on the encrypted form. The deterministic nonce (derived from the
/// driver, uuid, and exact plaintext bytes) keeps the whole import
/// byte-reproducible across runs.
pub(crate) fn encrypt_retained_payment_config(
    mapping: &TableMapping,
    app_key: &str,
    row: &mut CanonicalRow,
) -> anyhow::Result<()> {
    if mapping.source != "v2_payment" {
        return Ok(());
    }
    let payment = match row.get("payment") {
        Some(CanonicalValue::Text(value)) => value.clone(),
        _ => anyhow::bail!("retained payment row has no text driver"),
    };
    let uuid = match row.get("uuid") {
        Some(CanonicalValue::Text(value)) => value.clone(),
        _ => anyhow::bail!("retained payment row has no text uuid"),
    };
    let plaintext = match row.get("config") {
        Some(CanonicalValue::Json(value)) => value.to_compact_json()?,
        _ => anyhow::bail!("retained payment row has no JSON config"),
    };
    let envelope = v2board_payment_adapters::payment_secrets::encrypt_payment_config_canonical(
        app_key,
        &payment,
        &uuid,
        plaintext.as_bytes(),
    )
    .map_err(|error| anyhow::anyhow!("payment config encryption failed: {error}"))?
    .to_string();
    row.insert(
        "config".to_string(),
        CanonicalValue::Json(CanonicalJson::parse(&envelope).map_err(|error| {
            anyhow::anyhow!("encrypted payment config is not canonical JSON: {error}")
        })?),
    );
    Ok(())
}

pub(crate) fn index_legacy_payment(
    mapping: &TableMapping,
    row: &SourceRow,
    known_payment_ids: &mut BTreeSet<i32>,
    stripe_payment_ids: &mut BTreeSet<i32>,
) -> anyhow::Result<()> {
    if mapping.source != "v2_payment" {
        return Ok(());
    }
    let id = match row.get("id") {
        Some(SourceValue::I64(value)) => i32::try_from(*value)?,
        Some(SourceValue::U64(value)) => i32::try_from(*value)?,
        _ => anyhow::bail!("legacy payment row has no integer id"),
    };
    let driver = match row.get("payment") {
        Some(SourceValue::Text(value)) => value,
        _ => anyhow::bail!("legacy payment row has no text driver"),
    };
    anyhow::ensure!(
        !known_payment_ids.contains(&id),
        "legacy payment stream contains duplicate id {id}"
    );
    anyhow::ensure!(
        known_payment_ids.len() < MAX_LEGACY_PAYMENT_METHODS,
        "legacy payment table exceeds the fixed {MAX_LEGACY_PAYMENT_METHODS}-row classification safety bound"
    );
    known_payment_ids.insert(id);
    if is_legacy_stripe_payment_driver(driver) {
        stripe_payment_ids.insert(id);
    }
    Ok(())
}

fn decode_mysql_row(mapping: &TableMapping, row: &MySqlRow) -> anyhow::Result<SourceRow> {
    let mut decoded = BTreeMap::new();
    for (index, column) in row.columns().iter().enumerate() {
        if !mapping_has_source_column(mapping, column.name()) {
            anyhow::bail!(
                "source query for {} returned unexpected column {}",
                mapping.source,
                column.name()
            );
        }
        decoded.insert(column.name().to_string(), decode_mysql_value(row, index)?);
    }
    Ok(decoded)
}

fn next_source_id(mapping: &TableMapping, previous: i64, row: &SourceRow) -> anyhow::Result<i64> {
    let current = match row.get("id") {
        Some(SourceValue::I64(value)) => *value,
        Some(SourceValue::U64(value)) => i64::try_from(*value)?,
        _ => anyhow::bail!("source stream for {} has no integer id", mapping.source),
    };
    anyhow::ensure!(
        current > previous,
        "source stream for {} is not strictly ordered by id after {previous}",
        mapping.source
    );
    Ok(current)
}

fn decode_mysql_value(row: &MySqlRow, index: usize) -> anyhow::Result<SourceValue> {
    if row.try_get_raw(index)?.is_null() {
        return Ok(SourceValue::Null);
    }
    let type_name = row.column(index).type_info().name().to_ascii_uppercase();
    if type_name.contains("INT") || type_name == "YEAR" || type_name == "BOOLEAN" {
        if type_name.contains("UNSIGNED") {
            if let Ok(value) = row.try_get::<u64, _>(index) {
                return Ok(SourceValue::U64(value));
            }
            if let Ok(value) = row.try_get::<u32, _>(index) {
                return Ok(SourceValue::U64(u64::from(value)));
            }
            if let Ok(value) = row.try_get::<u16, _>(index) {
                return Ok(SourceValue::U64(u64::from(value)));
            }
            if let Ok(value) = row.try_get::<u8, _>(index) {
                return Ok(SourceValue::U64(u64::from(value)));
            }
        } else {
            if let Ok(value) = row.try_get::<i64, _>(index) {
                return Ok(SourceValue::I64(value));
            }
            if let Ok(value) = row.try_get::<i32, _>(index) {
                return Ok(SourceValue::I64(i64::from(value)));
            }
            if let Ok(value) = row.try_get::<i16, _>(index) {
                return Ok(SourceValue::I64(i64::from(value)));
            }
            if let Ok(value) = row.try_get::<i8, _>(index) {
                return Ok(SourceValue::I64(i64::from(value)));
            }
        }
    }
    if type_name.contains("DECIMAL") || type_name.contains("NUMERIC") {
        return Ok(SourceValue::Decimal(
            row.try_get::<Decimal, _>(index)?.normalize().to_string(),
        ));
    }
    if type_name.contains("BLOB") || type_name.contains("BINARY") || type_name == "BIT" {
        return Ok(SourceValue::Bytes(row.try_get(index)?));
    }
    if let Ok(value) = row.try_get::<String, _>(index) {
        return Ok(SourceValue::Text(value));
    }
    if let Ok(value) = row.try_get::<Vec<u8>, _>(index) {
        return Ok(SourceValue::Bytes(value));
    }
    anyhow::bail!(
        "unsupported legacy MySQL type {} at column {}",
        type_name,
        row.column(index).name()
    )
}

pub(crate) fn encode_copy_row(
    table: &str,
    columns: &[&str],
    row: &CanonicalRow,
) -> anyhow::Result<Vec<u8>> {
    if row.len() != columns.len() {
        anyhow::bail!(
            "canonical COPY row for {table} has {} columns; expected {}",
            row.len(),
            columns.len()
        );
    }
    let mut encoded = Vec::new();
    for (index, column) in columns.iter().enumerate() {
        if index != 0 {
            encoded.push(b',');
        }
        let value = row
            .get(*column)
            .ok_or_else(|| anyhow::anyhow!("canonical COPY row for {table} lacks {column}"))?;
        if matches!(value, CanonicalValue::Null) {
            encoded.extend_from_slice(b"\\N");
            continue;
        }
        let value = canonical_copy_text(table, column, value)?;
        encoded.push(b'"');
        for byte in value {
            if byte == b'"' {
                encoded.push(b'"');
            }
            encoded.push(byte);
        }
        encoded.push(b'"');
    }
    encoded.push(b'\n');
    if encoded.len() > MAX_COPY_ROW_BYTES {
        anyhow::bail!(
            "canonical COPY row for {table} exceeds the {}-byte safety limit",
            MAX_COPY_ROW_BYTES
        );
    }
    Ok(encoded)
}

fn canonical_copy_text(
    table: &str,
    column: &str,
    value: &CanonicalValue,
) -> anyhow::Result<Vec<u8>> {
    let bytes = match value {
        CanonicalValue::Null => unreachable!("NULL has dedicated COPY framing"),
        CanonicalValue::Bool(value) => {
            if *value {
                b"true".to_vec()
            } else {
                b"false".to_vec()
            }
        }
        CanonicalValue::I64(value) => value.to_string().into_bytes(),
        CanonicalValue::U64(value) => i64::try_from(*value)
            .with_context(|| format!("{table}.{column} exceeds PostgreSQL BIGINT"))?
            .to_string()
            .into_bytes(),
        CanonicalValue::Decimal(value) => {
            let normalized = Decimal::from_str(value)?.normalize().to_string();
            anyhow::ensure!(
                normalized == *value,
                "{table}.{column} is not a canonical exact decimal"
            );
            normalized.into_bytes()
        }
        CanonicalValue::Text(value) => {
            ensure_no_nul(table, column, value)?;
            value.as_bytes().to_vec()
        }
        CanonicalValue::Bytes(value) => format!("\\x{}", hex::encode(value)).into_bytes(),
        CanonicalValue::Json(value) => {
            ensure_json_has_no_nul(table, column, value)?;
            value.to_compact_json()?.into_bytes()
        }
    };
    Ok(bytes)
}

fn ensure_no_nul(table: &str, column: &str, value: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.contains('\0'),
        "{table}.{column} contains U+0000, which PostgreSQL text cannot store"
    );
    Ok(())
}

fn ensure_json_has_no_nul(table: &str, column: &str, value: &CanonicalJson) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.contains_nul(),
        "{table}.{column} contains U+0000, which PostgreSQL JSONB cannot store"
    );
    Ok(())
}

fn report_copy_progress(table: &str, rows: u64, bytes: u64, started_at: Instant) {
    let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
    eprintln!(
        "mysql-import COPY table={table} rows={rows} mib={:.1} elapsed_s={:.1} rows_per_s={:.0}",
        bytes as f64 / (1024.0 * 1024.0),
        elapsed,
        rows as f64 / elapsed,
    );
}

async fn copy_base_and_derived(
    source: &mut MySqlConnection,
    target: &PgPool,
    mapping: &TableMapping,
    derived: &DerivedMapping,
    app_key: &str,
    known_payment_ids: &mut BTreeSet<i32>,
    stripe_payment_ids: &mut BTreeSet<i32>,
) -> anyhow::Result<(ImportedTableReport, ImportedTableReport)> {
    anyhow::ensure!(
        derived.source_tables.first() == Some(&mapping.source),
        "derived COPY {} is not owned by source {}",
        derived.target,
        mapping.source,
    );
    let columns = target_columns_in_order(mapping);
    let base_copy_sql = target_copy_sql(mapping)?;
    let derived_copy_sql = derived_target_copy_sql(derived)?;
    let mut base_connection = target.acquire().await?;
    base_connection.close_on_drop();
    let mut derived_connection = target.acquire().await?;
    derived_connection.close_on_drop();
    let mut base_copy = base_connection.copy_in_raw(&base_copy_sql).await?;
    let mut derived_copy = derived_connection.copy_in_raw(&derived_copy_sql).await?;
    if !base_copy.is_textual()
        || base_copy.num_columns() != columns.len()
        || (0..base_copy.num_columns()).any(|index| !base_copy.column_is_textual(index))
    {
        let _ = base_copy.abort("v2board import COPY format mismatch").await;
        let _ = derived_copy
            .abort("v2board import COPY format mismatch")
            .await;
        anyhow::bail!("PostgreSQL COPY format mismatch for {}", mapping.target);
    }
    if !derived_copy.is_textual()
        || derived_copy.num_columns() != derived.target_columns.len()
        || (0..derived_copy.num_columns()).any(|index| !derived_copy.column_is_textual(index))
    {
        let _ = base_copy.abort("v2board import COPY format mismatch").await;
        let _ = derived_copy
            .abort("v2board import COPY format mismatch")
            .await;
        anyhow::bail!("PostgreSQL COPY format mismatch for {}", derived.target);
    }

    let mut base_retained = CanonicalRowsHasher::for_mapping(mapping)?;
    let mut derived_retained = CanonicalRowsHasher::new(derived.target, derived.target_columns)?;
    let mut previous_id = SOURCE_ID_LOWER_BOUND;
    let mut source_rows = 0_u64;
    let mut base_buffer = Vec::with_capacity(COPY_SEND_BUFFER_BYTES);
    let mut derived_buffer = Vec::with_capacity(COPY_SEND_BUFFER_BYTES);
    let mut base_bytes_sent = 0_u64;
    let mut derived_bytes_sent = 0_u64;
    let started_at = Instant::now();
    let mut next_progress = COPY_PROGRESS_ROWS;

    let stream_result: anyhow::Result<()> = async {
        let sql = source_stream_sql(mapping)?;
        let mut rows = sqlx::query(AssertSqlSafe(sql).into_sql_str()).fetch(&mut *source);
        while let Some(row) = rows.try_next().await? {
            let source_row = decode_mysql_row(mapping, &row)?;
            previous_id = next_source_id(mapping, previous_id, &source_row)?;
            source_rows = source_rows
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("{} source row count overflow", mapping.source))?;

            index_legacy_payment(mapping, &source_row, known_payment_ids, stripe_payment_ids)?;

            let mut base_row = match transform_mysql_import_row(
                mapping,
                &source_row,
                &*known_payment_ids,
                &*stripe_payment_ids,
            )? {
                MysqlImportRowDisposition::Retain(row) => row,
                MysqlImportRowDisposition::Discard => {
                    anyhow::bail!(
                        "derived source {} unexpectedly discarded its base row",
                        mapping.source
                    )
                }
            };
            encrypt_retained_payment_config(mapping, app_key, &mut base_row)?;
            let encoded = encode_copy_row(mapping.target, &columns, &base_row)?;
            base_retained.update_row(&base_row)?;
            if !base_buffer.is_empty()
                && base_buffer.len().saturating_add(encoded.len()) > COPY_SEND_BUFFER_BYTES
            {
                base_bytes_sent = base_bytes_sent
                    .checked_add(base_buffer.len() as u64)
                    .ok_or_else(|| {
                        anyhow::anyhow!("{} COPY byte count overflow", mapping.target)
                    })?;
                base_copy.send(std::mem::take(&mut base_buffer)).await?;
                base_buffer = Vec::with_capacity(COPY_SEND_BUFFER_BYTES);
            }
            if encoded.len() > COPY_SEND_BUFFER_BYTES {
                base_bytes_sent = base_bytes_sent
                    .checked_add(encoded.len() as u64)
                    .ok_or_else(|| {
                        anyhow::anyhow!("{} COPY byte count overflow", mapping.target)
                    })?;
                base_copy.send(encoded).await?;
            } else {
                base_buffer.extend_from_slice(&encoded);
            }

            for row in derived_rows_for_source(derived, &source_row)? {
                let encoded = encode_copy_row(derived.target, derived.target_columns, &row)?;
                derived_retained.update_row(&row)?;
                if !derived_buffer.is_empty()
                    && derived_buffer.len().saturating_add(encoded.len()) > COPY_SEND_BUFFER_BYTES
                {
                    derived_bytes_sent = derived_bytes_sent
                        .checked_add(derived_buffer.len() as u64)
                        .ok_or_else(|| {
                            anyhow::anyhow!("{} COPY byte count overflow", derived.target)
                        })?;
                    derived_copy
                        .send(std::mem::take(&mut derived_buffer))
                        .await?;
                    derived_buffer = Vec::with_capacity(COPY_SEND_BUFFER_BYTES);
                }
                if encoded.len() > COPY_SEND_BUFFER_BYTES {
                    derived_bytes_sent = derived_bytes_sent
                        .checked_add(encoded.len() as u64)
                        .ok_or_else(|| {
                            anyhow::anyhow!("{} COPY byte count overflow", derived.target)
                        })?;
                    derived_copy.send(encoded).await?;
                } else {
                    derived_buffer.extend_from_slice(&encoded);
                }
            }
            if source_rows >= next_progress {
                report_copy_progress(mapping.target, source_rows, base_bytes_sent, started_at);
                next_progress = next_progress.saturating_add(COPY_PROGRESS_ROWS);
            }
        }
        drop(rows);
        if !base_buffer.is_empty() {
            base_bytes_sent = base_bytes_sent
                .checked_add(base_buffer.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("{} COPY byte count overflow", mapping.target))?;
            base_copy.send(std::mem::take(&mut base_buffer)).await?;
        }
        if !derived_buffer.is_empty() {
            derived_bytes_sent = derived_bytes_sent
                .checked_add(derived_buffer.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("{} COPY byte count overflow", derived.target))?;
            derived_copy
                .send(std::mem::take(&mut derived_buffer))
                .await?;
        }
        Ok(())
    }
    .await;
    if let Err(error) = stream_result {
        let _ = base_copy.abort("v2board import COPY stream failed").await;
        let _ = derived_copy
            .abort("v2board import COPY stream failed")
            .await;
        return Err(error.context(format!(
            "COPY {} -> {} and {}",
            mapping.source, mapping.target, derived.target
        )));
    }
    let (base_retained_rows, base_retained_sha256) = base_retained.finish();
    let (derived_retained_rows, derived_retained_sha256) = derived_retained.finish();
    let (base_copied_rows, derived_copied_rows) =
        tokio::join!(base_copy.finish(), derived_copy.finish());
    let base_copied_rows = base_copied_rows?;
    let derived_copied_rows = derived_copied_rows?;
    anyhow::ensure!(
        base_copied_rows == base_retained_rows,
        "{} COPY row count mismatch: expected {base_retained_rows}, observed {base_copied_rows}",
        mapping.target,
    );
    anyhow::ensure!(
        derived_copied_rows == derived_retained_rows,
        "{} COPY row count mismatch: expected {derived_retained_rows}, observed {derived_copied_rows}",
        derived.target,
    );
    report_copy_progress(mapping.target, source_rows, base_bytes_sent, started_at);
    report_copy_progress(
        derived.target,
        derived_retained_rows,
        derived_bytes_sent,
        started_at,
    );
    Ok((
        ImportedTableReport {
            source: mapping.source.to_string(),
            target: mapping.target.to_string(),
            source_rows,
            retained_rows: base_retained_rows,
            discarded_rows: 0,
            retained_sha256: base_retained_sha256,
        },
        ImportedTableReport {
            source: derived_source_label(derived).to_string(),
            target: derived.target.to_string(),
            source_rows,
            retained_rows: derived_retained_rows,
            discarded_rows: 0,
            retained_sha256: derived_retained_sha256,
        },
    ))
}

fn derived_rows_for_source(
    mapping: &DerivedMapping,
    source: &SourceRow,
) -> anyhow::Result<Vec<CanonicalRow>> {
    match mapping.kind {
        DerivedMappingKind::PlanPrices => {
            let plan_id = required_source_i32(source, "id", "v2_plan")?;
            let mut rows = Vec::with_capacity(8);
            for period in PlanPricePeriod::ALL {
                let source_column = legacy_plan_price_column(period);
                let Some(amount_minor) = optional_source_i32(source, source_column, "v2_plan")?
                else {
                    continue;
                };
                rows.push(CanonicalRow::from([
                    (
                        "plan_id".to_string(),
                        CanonicalValue::I64(i64::from(plan_id)),
                    ),
                    (
                        "period".to_string(),
                        CanonicalValue::Text(native_plan_price_period(period).to_string()),
                    ),
                    (
                        "amount_minor".to_string(),
                        CanonicalValue::I64(i64::from(amount_minor)),
                    ),
                ]));
            }
            Ok(rows)
        }
        DerivedMappingKind::GiftcardRedemptions => {
            let giftcard_id = required_source_i32(source, "id", "v2_giftcard")?;
            let used_user_ids = source
                .get("used_user_ids")
                .ok_or_else(|| anyhow::anyhow!("v2_giftcard source row lacks used_user_ids"))?;
            expand_giftcard_redemptions(giftcard_id, used_user_ids)?
                .into_iter()
                .map(|redemption| Ok(giftcard_redemption_canonical(&redemption)))
                .collect()
        }
    }
}

fn legacy_plan_price_column(period: PlanPricePeriod) -> &'static str {
    match period {
        PlanPricePeriod::Month => "month_price",
        PlanPricePeriod::Quarter => "quarter_price",
        PlanPricePeriod::HalfYear => "half_year_price",
        PlanPricePeriod::Year => "year_price",
        PlanPricePeriod::TwoYear => "two_year_price",
        PlanPricePeriod::ThreeYear => "three_year_price",
        PlanPricePeriod::OneTime => "onetime_price",
        PlanPricePeriod::Reset => "reset_price",
    }
}

fn native_plan_price_period(period: PlanPricePeriod) -> &'static str {
    match period {
        PlanPricePeriod::Month => "month",
        PlanPricePeriod::Quarter => "quarter",
        PlanPricePeriod::HalfYear => "half_year",
        PlanPricePeriod::Year => "year",
        PlanPricePeriod::TwoYear => "two_year",
        PlanPricePeriod::ThreeYear => "three_year",
        PlanPricePeriod::OneTime => "one_time",
        PlanPricePeriod::Reset => "reset",
    }
}

fn required_source_i32(source: &SourceRow, column: &str, table: &str) -> anyhow::Result<i32> {
    optional_source_i32(source, column, table)?
        .ok_or_else(|| anyhow::anyhow!("{table}.{column} must not be NULL"))
}

fn optional_source_i32(
    source: &SourceRow,
    column: &str,
    table: &str,
) -> anyhow::Result<Option<i32>> {
    match source.get(column) {
        Some(SourceValue::Null) => Ok(None),
        Some(SourceValue::I64(value)) => i32::try_from(*value)
            .map(Some)
            .with_context(|| format!("{table}.{column} is outside PostgreSQL INTEGER range")),
        Some(SourceValue::U64(value)) => i32::try_from(*value)
            .map(Some)
            .with_context(|| format!("{table}.{column} is outside PostgreSQL INTEGER range")),
        Some(_) => anyhow::bail!("{table}.{column} is not an integer"),
        None => anyhow::bail!("{table} source row lacks {column}"),
    }
}

fn derived_source_label(mapping: &DerivedMapping) -> &'static str {
    match mapping.kind {
        DerivedMappingKind::PlanPrices => "v2_plan.price_columns",
        DerivedMappingKind::GiftcardRedemptions => "v2_giftcard.used_user_ids",
    }
}

fn giftcard_redemption_canonical(row: &LegacyGiftcardRedemptionRow) -> CanonicalRow {
    CanonicalRow::from([
        (
            "giftcard_id".to_string(),
            CanonicalValue::I64(i64::from(row.giftcard_id)),
        ),
        ("user_id".to_string(), CanonicalValue::I64(row.user_id)),
        (
            "created_at".to_string(),
            CanonicalValue::I64(row.created_at),
        ),
        (
            "created_at_provenance".to_string(),
            CanonicalValue::Text(row.created_at_provenance.clone()),
        ),
    ])
}
