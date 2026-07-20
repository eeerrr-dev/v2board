use super::audit::audit_registry;
use super::mappings::{DerivedMapping, DerivedMappingKind, TableMapping};
use super::transform::source_columns_in_order;
use super::values::ConverterError;

pub fn source_stream_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    let columns = source_columns_in_order(mapping)
        .into_iter()
        .map(mysql_identifier)
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "SELECT {columns} FROM {} ORDER BY `id` ASC",
        mysql_identifier(mapping.source)
    ))
}

pub fn target_copy_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    Ok(copy_sql(mapping.target, &target_columns_in_order(mapping)))
}

pub fn derived_target_copy_sql(mapping: &DerivedMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    Ok(copy_sql(mapping.target, mapping.target_columns))
}

fn copy_sql(table: &str, columns: &[&str]) -> String {
    let quoted = columns
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "COPY {} ({quoted}) FROM STDIN WITH (FORMAT csv, DELIMITER ',', QUOTE '\"', ESCAPE '\"', NULL E'\\\\N', HEADER false, ENCODING 'UTF8', ON_ERROR stop)",
        postgres_identifier(table)
    )
}

pub fn target_verify_stream_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    Ok(verify_stream_sql(
        mapping.target,
        &target_columns_in_order(mapping),
        &["id"],
    ))
}

pub fn derived_target_verify_stream_sql(
    mapping: &DerivedMapping,
) -> Result<String, ConverterError> {
    audit_registry()?;
    let columns = mapping
        .target_columns
        .iter()
        .map(|column| match (mapping.kind, *column) {
            // SQLx deliberately does not decode a PostgreSQL user-defined
            // enum as `String`. Verification is over the enum's canonical
            // textual label, so cast only this adapter-owned representation.
            (DerivedMappingKind::PlanPrices, "period") => {
                "\"period\"::text AS \"period\"".to_string()
            }
            _ => postgres_identifier(column),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Ok(verify_stream_sql_with_projection(
        mapping.target,
        &columns,
        mapping.key_columns,
    ))
}

fn verify_stream_sql(table: &str, columns: &[&str], key_columns: &[&str]) -> String {
    let columns = columns
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    verify_stream_sql_with_projection(table, &columns, key_columns)
}

fn verify_stream_sql_with_projection(table: &str, columns: &str, key_columns: &[&str]) -> String {
    // PostgreSQL resolves an unqualified ORDER BY name against output aliases
    // before input columns.  A verification projection such as
    // `period::text AS period` would therefore sort the textual adapter value
    // instead of the native enum/primary-key value.  Qualifying every key
    // keeps the scan in the source-derived canonical key order even when a
    // projected representation deliberately reuses the column name.
    let order = key_columns
        .iter()
        .map(|column| {
            format!(
                "{}.{} ASC",
                postgres_identifier(table),
                postgres_identifier(column)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SELECT {columns} FROM {} ORDER BY {order}",
        postgres_identifier(table)
    )
}

pub fn sequence_reset_sql(mapping: &TableMapping) -> Result<String, ConverterError> {
    audit_registry()?;
    Ok(format!(
        "SELECT setval(pg_get_serial_sequence('{table}', 'id'), GREATEST(COALESCE(MAX(id), 1), 1), MAX(id) IS NOT NULL AND MAX(id) >= 1) FROM {quoted}",
        table = mapping.target,
        quoted = postgres_identifier(mapping.target),
    ))
}

pub fn target_columns_in_order(mapping: &TableMapping) -> Vec<&str> {
    mapping
        .direct_columns
        .iter()
        .copied()
        .chain(
            mapping
                .transformed_columns
                .iter()
                .map(|column| column.target),
        )
        .chain(mapping.added_columns.iter().map(|column| column.target))
        .collect()
}

fn mysql_identifier(identifier: &str) -> String {
    format!("`{identifier}`")
}

fn postgres_identifier(identifier: &str) -> String {
    format!("\"{identifier}\"")
}
