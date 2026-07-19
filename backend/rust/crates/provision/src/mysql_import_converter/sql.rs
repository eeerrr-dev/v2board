use super::audit::audit_registry;
use super::mappings::{DerivedMapping, TableMapping};
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
    Ok(verify_stream_sql(
        mapping.target,
        mapping.target_columns,
        mapping.key_columns,
    ))
}

fn verify_stream_sql(table: &str, columns: &[&str], key_columns: &[&str]) -> String {
    let columns = columns
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let order = key_columns
        .iter()
        .map(|column| format!("{} ASC", postgres_identifier(column)))
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
