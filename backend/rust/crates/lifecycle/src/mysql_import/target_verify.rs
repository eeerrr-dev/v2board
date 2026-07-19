use std::time::Instant;

use anyhow::Context;
use futures_util::TryStreamExt;
use rust_decimal::Decimal;
use serde_json::value::RawValue;
use sqlx::{AssertSqlSafe, Column, PgPool, Row, SqlSafeStr, TypeInfo, ValueRef, types::Json};
use v2board_provision::mysql_import_converter::{
    CanonicalJson, CanonicalRow, CanonicalRowsHasher, CanonicalValue, DERIVED_MAPPINGS,
    DerivedMapping, IdentityWidth, TABLE_MAPPINGS, TableMapping, derived_target_verify_stream_sql,
    discarded_target_tables, sequence_reset_sql, target_columns_in_order, target_verify_stream_sql,
};

use super::{
    copy_stream::COPY_PROGRESS_ROWS,
    execute::ImportedTableReport,
    postgres_target::{execute_dynamic, postgres_identifier},
};

pub(crate) async fn finalize_and_verify_business_data(
    target: &PgPool,
    reports: &[ImportedTableReport],
) -> anyhow::Result<()> {
    for mapping in TABLE_MAPPINGS {
        ensure_sequence_headroom(target, mapping).await?;
    }
    eprintln!("mysql-import PostgreSQL sequence reset started");
    for mapping in TABLE_MAPPINGS {
        execute_dynamic(target, sequence_reset_sql(mapping)?).await?;
    }
    eprintln!("mysql-import PostgreSQL ANALYZE started");
    sqlx::query("ANALYZE").execute(target).await?;
    eprintln!("mysql-import PostgreSQL canonical verification started");

    for mapping in TABLE_MAPPINGS {
        let report = reports
            .iter()
            .find(|report| report.target == mapping.target)
            .ok_or_else(|| anyhow::anyhow!("converter report omitted target {}", mapping.target))?;
        verify_target_table(target, mapping, report).await?;
    }
    let derived = DERIVED_MAPPINGS
        .first()
        .ok_or_else(|| anyhow::anyhow!("converter registry omitted derived mapping"))?;
    let report = reports
        .iter()
        .find(|report| report.target == derived.target)
        .ok_or_else(|| anyhow::anyhow!("converter report omitted target {}", derived.target))?;
    verify_derived_target_table(target, derived, report).await?;
    validate_transformed_references(target).await?;

    for table in discarded_target_tables() {
        let sql = format!("SELECT COUNT(*) FROM {}", postgres_identifier(table));
        let count: i64 = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
            .fetch_one(target)
            .await?;
        if count != 0 {
            anyhow::bail!("fixed-discard target {table} contains {count} row(s)");
        }
    }
    Ok(())
}

async fn ensure_sequence_headroom(target: &PgPool, mapping: &TableMapping) -> anyhow::Result<()> {
    let sql = format!(
        "SELECT MAX(id)::bigint FROM {}",
        postgres_identifier(mapping.target)
    );
    let maximum: Option<i64> = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
        .fetch_one(target)
        .await?;
    validate_identity_headroom(mapping, maximum)
}

pub(crate) fn validate_identity_headroom(
    mapping: &TableMapping,
    maximum: Option<i64>,
) -> anyhow::Result<()> {
    let exhausted = match (mapping.identity_width, maximum) {
        (_, None) => false,
        (IdentityWidth::I32, Some(maximum)) => maximum >= i64::from(i32::MAX),
        (IdentityWidth::I64, Some(maximum)) => maximum == i64::MAX,
    };
    if exhausted {
        anyhow::bail!(
            "target identity sequence for {} is exhausted at imported maximum id {}; refuse to complete an installation that cannot allocate the next id",
            mapping.target,
            maximum.expect("exhausted identities have a maximum")
        );
    }
    Ok(())
}

async fn verify_target_table(
    target: &PgPool,
    mapping: &TableMapping,
    report: &ImportedTableReport,
) -> anyhow::Result<()> {
    let columns = target_columns_in_order(mapping);
    verify_target_stream(
        target,
        mapping.target,
        &columns,
        target_verify_stream_sql(mapping)?,
        report,
    )
    .await
}

async fn verify_derived_target_table(
    target: &PgPool,
    mapping: &DerivedMapping,
    report: &ImportedTableReport,
) -> anyhow::Result<()> {
    verify_target_stream(
        target,
        mapping.target,
        mapping.target_columns,
        derived_target_verify_stream_sql(mapping)?,
        report,
    )
    .await
}

async fn verify_target_stream(
    target: &PgPool,
    table: &str,
    columns: &[&str],
    sql: String,
    report: &ImportedTableReport,
) -> anyhow::Result<()> {
    let mut hasher = CanonicalRowsHasher::new(table, columns)?;
    let started_at = Instant::now();
    let mut next_progress = COPY_PROGRESS_ROWS;
    let mut rows = sqlx::query(AssertSqlSafe(sql).into_sql_str()).fetch(target);
    while let Some(row) = rows.try_next().await? {
        hasher.update_row(&decode_postgres_row(table, columns, &row)?)?;
        if hasher.rows() >= next_progress {
            report_verify_progress(table, hasher.rows(), started_at);
            next_progress = next_progress.saturating_add(COPY_PROGRESS_ROWS);
        }
    }
    let (rows, sha256) = hasher.finish();
    report_verify_progress(table, rows, started_at);
    anyhow::ensure!(
        rows == report.retained_rows,
        "target row count mismatch for {table}: expected {}, observed {rows}",
        report.retained_rows
    );
    anyhow::ensure!(
        sha256 == report.retained_sha256,
        "target canonical hash mismatch for {table}: COPY did not preserve the transformed source rows"
    );
    Ok(())
}

fn report_verify_progress(table: &str, rows: u64, started_at: Instant) {
    let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
    eprintln!(
        "mysql-import verify table={table} rows={rows} elapsed_s={elapsed:.1} rows_per_s={:.0}",
        rows as f64 / elapsed,
    );
}

fn decode_postgres_row(
    table: &str,
    columns: &[&str],
    row: &sqlx::postgres::PgRow,
) -> anyhow::Result<CanonicalRow> {
    anyhow::ensure!(
        row.len() == columns.len(),
        "target verification query for {table} returned an unexpected column count"
    );
    let mut decoded = CanonicalRow::new();
    for (index, column) in columns.iter().enumerate() {
        let value = if row.try_get_raw(index)?.is_null() {
            CanonicalValue::Null
        } else {
            match row
                .column(index)
                .type_info()
                .name()
                .to_ascii_uppercase()
                .as_str()
            {
                "INT2" | "INT4" | "INT8" => CanonicalValue::I64(postgres_integer(row, index)?),
                "NUMERIC" => CanonicalValue::Decimal(
                    row.try_get::<Decimal, _>(index)?.normalize().to_string(),
                ),
                "BYTEA" => CanonicalValue::Bytes(row.try_get(index)?),
                "JSON" | "JSONB" => {
                    let value = row.try_get::<Json<Box<RawValue>>, _>(index)?.0;
                    CanonicalValue::Json(CanonicalJson::parse(value.get()).map_err(|error| {
                        anyhow::anyhow!(
                            "could not decode exact PostgreSQL JSON for {table}.{column}: {error}"
                        )
                    })?)
                }
                _ => CanonicalValue::Text(row.try_get(index).with_context(|| {
                    format!(
                        "unsupported PostgreSQL type {} for {table}.{column}",
                        row.column(index).type_info().name()
                    )
                })?),
            }
        };
        decoded.insert((*column).to_string(), value);
    }
    Ok(decoded)
}

fn postgres_integer(row: &sqlx::postgres::PgRow, index: usize) -> anyhow::Result<i64> {
    if let Ok(value) = row.try_get::<i64, _>(index) {
        return Ok(value);
    }
    if let Ok(value) = row.try_get::<i32, _>(index) {
        return Ok(i64::from(value));
    }
    if let Ok(value) = row.try_get::<i16, _>(index) {
        return Ok(i64::from(value));
    }
    anyhow::bail!("target integer has an unsupported PostgreSQL type")
}

async fn validate_transformed_references(target: &PgPool) -> anyhow::Result<()> {
    let missing_coupon_plans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM coupon AS c \
         CROSS JOIN LATERAL jsonb_array_elements_text(c.limit_plan_ids) AS item(plan_id) \
         LEFT JOIN plan AS p ON p.id = item.plan_id::integer \
         WHERE c.limit_plan_ids IS NOT NULL AND p.id IS NULL",
    )
    .fetch_one(target)
    .await?;
    let missing_surplus_orders: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM orders AS o \
         CROSS JOIN LATERAL jsonb_array_elements_text(o.surplus_order_ids::jsonb) AS item(order_id) \
         LEFT JOIN orders AS referenced ON referenced.id = item.order_id::bigint \
         WHERE o.surplus_order_ids IS NOT NULL AND referenced.id IS NULL",
    )
    .fetch_one(target)
    .await?;
    if missing_coupon_plans != 0 || missing_surplus_orders != 0 {
        anyhow::bail!(
            "transformed array references are incomplete: coupon_plans={missing_coupon_plans}, surplus_orders={missing_surplus_orders}"
        );
    }
    Ok(())
}
