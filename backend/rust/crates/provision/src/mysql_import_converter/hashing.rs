use std::collections::BTreeSet;

use sha2::{Digest, Sha256, Sha384};

use crate::mysql_import_policy::MYSQL_IMPORT_POLICY_MARKER;

use super::audit::{audit_registry, registry_error, validate_identifier};
use super::mappings::{
    DERIVED_MAPPINGS, DISCARDED_SOURCE_TABLES, DISCARDED_TARGET_TABLES, SCALAR_REFERENCES,
    TABLE_MAPPINGS, TARGET_GENERATED_COLUMNS, TableMapping,
};
use super::sql::target_columns_in_order;
use super::values::{CanonicalRow, CanonicalValue, ConverterError, ExactJsonValue};
use super::{
    MYSQL_IMPORT_REGISTRY_VERSION, MYSQL_IMPORT_SOURCE_PROFILE,
    MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256, MYSQL_SOURCE_INSTALL_SQL_SHA256,
    TARGET_POSTGRES_MIGRATIONS, TARGET_POSTGRES_SCHEMA_ID,
};

/// Domain-separated identity of the current pre-release PostgreSQL schema.
/// Each entry binds its version, filename, and SQLx-style SHA-384 checksum.
pub fn target_postgres_schema_sha256() -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.postgres-schema.v1\0");
    for (version, name, sql) in TARGET_POSTGRES_MIGRATIONS {
        digest.update(version.to_be_bytes());
        digest_field(&mut digest, name.as_bytes());
        digest_field(&mut digest, &Sha384::digest(sql));
    }
    hex::encode(digest.finalize())
}

pub fn registry_sha256() -> Result<String, ConverterError> {
    audit_registry()?;
    let mut digest = Sha256::new();
    digest.update(b"v2board-mysql-import-registry-v1\0");
    digest.update(MYSQL_IMPORT_REGISTRY_VERSION.to_be_bytes());
    digest_field(&mut digest, MYSQL_IMPORT_SOURCE_PROFILE.as_bytes());
    digest_field(&mut digest, MYSQL_IMPORTED_SOURCE_SCHEMA_SHA256.as_bytes());
    digest_field(&mut digest, MYSQL_SOURCE_INSTALL_SQL_SHA256.as_bytes());
    digest_field(&mut digest, TARGET_POSTGRES_SCHEMA_ID.as_bytes());
    digest_field(&mut digest, target_postgres_schema_sha256().as_bytes());
    for mapping in TABLE_MAPPINGS {
        digest_field(&mut digest, mapping.order.to_string().as_bytes());
        digest_field(&mut digest, mapping.source.as_bytes());
        digest_field(&mut digest, mapping.target.as_bytes());
        digest_field(
            &mut digest,
            format!("{:?}", mapping.identity_width).as_bytes(),
        );
        for column in mapping.direct_columns {
            digest_field(&mut digest, b"direct");
            digest_field(&mut digest, column.as_bytes());
        }
        for column in mapping.transformed_columns {
            digest_field(&mut digest, b"transform");
            digest_field(&mut digest, column.source.as_bytes());
            digest_field(&mut digest, column.target.as_bytes());
            digest_field(&mut digest, format!("{:?}", column.rule).as_bytes());
            digest_field(
                &mut digest,
                column.source_referenced_table.unwrap_or("").as_bytes(),
            );
            digest_field(
                &mut digest,
                column.referenced_target_table.unwrap_or("").as_bytes(),
            );
        }
        for column in mapping.added_columns {
            digest_field(&mut digest, b"added");
            digest_field(&mut digest, column.target.as_bytes());
            digest_field(&mut digest, format!("{:?}", column.value).as_bytes());
            digest_field(&mut digest, column.provenance.as_bytes());
        }
        for column in mapping.consumed_source_columns {
            digest_field(&mut digest, b"consumed");
            digest_field(&mut digest, column.source.as_bytes());
            digest_field(&mut digest, column.reason.as_bytes());
        }
    }
    for mapping in DERIVED_MAPPINGS {
        digest_field(&mut digest, mapping.order.to_string().as_bytes());
        digest_field(&mut digest, mapping.target.as_bytes());
        digest_field(&mut digest, format!("{:?}", mapping.kind).as_bytes());
        for table in mapping.source_tables {
            digest_field(&mut digest, table.as_bytes());
        }
        for column in mapping.target_columns {
            digest_field(&mut digest, b"target-column");
            digest_field(&mut digest, column.as_bytes());
        }
        for column in mapping.key_columns {
            digest_field(&mut digest, b"key-column");
            digest_field(&mut digest, column.as_bytes());
        }
        digest_field(&mut digest, mapping.rule.as_bytes());
    }
    digest_field(&mut digest, b"mysql-import-schema-v1");
    digest_field(&mut digest, MYSQL_IMPORT_POLICY_MARKER.as_bytes());
    for table in DISCARDED_SOURCE_TABLES {
        digest_field(&mut digest, b"discard-source-table");
        digest_field(&mut digest, table.as_bytes());
    }
    for table in DISCARDED_TARGET_TABLES {
        digest_field(&mut digest, b"empty-target-table");
        digest_field(&mut digest, table.as_bytes());
    }
    for reference in SCALAR_REFERENCES {
        digest_field(&mut digest, b"scalar-reference");
        digest_field(&mut digest, reference.source_table.as_bytes());
        digest_field(&mut digest, reference.target_table.as_bytes());
        digest_field(&mut digest, reference.column.as_bytes());
        digest_field(&mut digest, reference.source_referenced_table.as_bytes());
        digest_field(&mut digest, reference.target_referenced_table.as_bytes());
        digest_field(&mut digest, format!("{:?}", reference.rule).as_bytes());
    }
    for (table, columns) in TARGET_GENERATED_COLUMNS {
        digest_field(&mut digest, b"generated-target-columns");
        digest_field(&mut digest, table.as_bytes());
        for column in *columns {
            digest_field(&mut digest, column.as_bytes());
        }
    }
    digest_field(
        &mut digest,
        b"preserve-mysql-persisted-v2-user-u-d-and-permanent-token;never-fold-legacy-redis",
    );
    Ok(hex::encode(digest.finalize()))
}

/// Streaming canonical digest for the typed rows written to and read back from
/// PostgreSQL. Transport framing is deliberately excluded.
pub struct CanonicalRowsHasher {
    table: String,
    columns: Vec<String>,
    rows: u64,
    digest: Sha256,
}

impl CanonicalRowsHasher {
    pub fn for_mapping(mapping: &TableMapping) -> Result<Self, ConverterError> {
        audit_registry()?;
        Self::new(mapping.target, &target_columns_in_order(mapping))
    }

    pub fn new(table: &str, columns: &[&str]) -> Result<Self, ConverterError> {
        validate_identifier(table)?;
        if columns.is_empty() {
            return registry_error(format!("target table {table} has no COPY columns"));
        }
        let mut seen = BTreeSet::new();
        let mut digest = Sha256::new();
        digest.update(b"v2board-mysql-import-canonical-rows-v2\0");
        digest_field(&mut digest, table.as_bytes());
        for column in columns {
            validate_identifier(column)?;
            if !seen.insert(*column) {
                return registry_error(format!("target table {table} repeats column {column}"));
            }
            digest_field(&mut digest, column.as_bytes());
        }
        Ok(Self {
            table: table.to_string(),
            columns: columns.iter().map(|column| (*column).to_string()).collect(),
            rows: 0,
            digest,
        })
    }

    pub fn update_row(&mut self, row: &CanonicalRow) -> Result<(), ConverterError> {
        if row.len() != self.columns.len() {
            return registry_error(format!(
                "target row for {} has {} columns; expected {}",
                self.table,
                row.len(),
                self.columns.len()
            ));
        }
        digest_field(&mut self.digest, b"row");
        for column in &self.columns {
            let value = row
                .get(column)
                .ok_or_else(|| ConverterError::MissingColumn {
                    table: self.table.clone(),
                    column: column.clone(),
                })?;
            digest_canonical_value(&mut self.digest, &self.table, column, value)?;
        }
        self.rows = self.rows.checked_add(1).ok_or_else(|| {
            ConverterError::Registry(format!("row count overflow for {}", self.table))
        })?;
        Ok(())
    }

    pub fn rows(&self) -> u64 {
        self.rows
    }

    pub fn finish(mut self) -> (u64, String) {
        digest_field(&mut self.digest, b"end");
        digest_field(&mut self.digest, &self.rows.to_be_bytes());
        (self.rows, hex::encode(self.digest.finalize()))
    }
}

fn digest_canonical_value(
    digest: &mut Sha256,
    table: &str,
    column: &str,
    value: &CanonicalValue,
) -> Result<(), ConverterError> {
    match value {
        CanonicalValue::Null => digest_field(digest, b"null"),
        CanonicalValue::Bool(value) => {
            digest_field(digest, b"boolean");
            digest_field(digest, if *value { b"true" } else { b"false" });
        }
        CanonicalValue::I64(value) => {
            digest_field(digest, b"integer");
            digest_field(digest, value.to_string().as_bytes());
        }
        CanonicalValue::U64(value) => {
            let value = i64::try_from(*value).map_err(|_| ConverterError::IntegerOutOfRange {
                table: table.to_string(),
                column: column.to_string(),
                value: *value,
            })?;
            digest_field(digest, b"integer");
            digest_field(digest, value.to_string().as_bytes());
        }
        CanonicalValue::Decimal(value) => digest_field(digest, format!("d:{value}").as_bytes()),
        CanonicalValue::Text(value) => {
            digest_field(digest, b"text");
            digest_field(digest, value.as_bytes());
        }
        CanonicalValue::Bytes(value) => {
            digest_field(digest, b"bytes");
            digest_field(digest, value);
        }
        CanonicalValue::Json(value) => {
            digest_field(digest, b"json");
            digest_json_value(digest, &value.0);
        }
    }
    Ok(())
}

fn digest_json_value(digest: &mut Sha256, value: &ExactJsonValue) {
    match value {
        ExactJsonValue::Null => digest_field(digest, b"null"),
        ExactJsonValue::Bool(value) => {
            digest_field(digest, if *value { b"true" } else { b"false" });
        }
        ExactJsonValue::Number(value) => {
            digest_field(digest, b"number");
            digest_field(digest, value.as_bytes());
        }
        ExactJsonValue::String(value) => {
            digest_field(digest, b"string");
            digest_field(digest, value.as_bytes());
        }
        ExactJsonValue::Array(values) => {
            digest_field(digest, b"array");
            digest_field(digest, &(values.len() as u64).to_be_bytes());
            for value in values {
                digest_json_value(digest, value);
            }
        }
        ExactJsonValue::Object(values) => {
            digest_field(digest, b"object");
            digest_field(digest, &(values.len() as u64).to_be_bytes());
            for (key, value) in values {
                digest_field(digest, key.as_bytes());
                digest_json_value(digest, value);
            }
        }
    }
}

fn digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
