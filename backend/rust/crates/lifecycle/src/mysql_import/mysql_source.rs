use std::collections::{BTreeMap, BTreeSet};

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, Executor, MySql, MySqlConnection, QueryBuilder, Row, SqlSafeStr};
use url::Url;
use v2board_provision::mysql_import_converter::{
    DISCARDED_SOURCE_TABLES, SCALAR_REFERENCES, ScalarReferenceRule, TABLE_MAPPINGS, TableMapping,
};

use super::execute::DiscardedTableReport;

pub(crate) struct SourceSchemaInspection {
    pub(crate) imported_schema_sha256: String,
    pub(crate) discarded_tables: Vec<DiscardedTableReport>,
}

pub(crate) async fn verify_mysql_vendor_and_version(
    source: &mut MySqlConnection,
) -> anyhow::Result<()> {
    let (version, comment): (String, String) =
        sqlx::query_as("SELECT VERSION(), @@version_comment")
            .fetch_one(&mut *source)
            .await?;
    if !is_supported_oracle_mysql_version(&version, &comment) {
        anyhow::bail!(
            "legacy source engine must be Oracle MySQL 8.0.x, 8.3.x, or 8.4.x, observed {version} ({comment})"
        );
    }
    Ok(())
}

pub(crate) fn is_supported_oracle_mysql_version(version: &str, comment: &str) -> bool {
    let lowercase = format!("{version} {comment}").to_ascii_lowercase();
    (version.starts_with("8.0.") || version.starts_with("8.3.") || version.starts_with("8.4."))
        && !lowercase.contains("mariadb")
        && !lowercase.contains("percona")
        && lowercase.contains("mysql")
}

pub(crate) async fn verify_mysql_source_principal(
    source: &mut MySqlConnection,
    database_url: &str,
) -> anyhow::Result<()> {
    let url = Url::parse(database_url)?;
    let expected_username = percent_decode_str(url.username()).decode_utf8()?;
    let expected_database = percent_decode_str(url.path().trim_start_matches('/')).decode_utf8()?;
    let (current_user, current_database): (String, Option<String>) =
        sqlx::query_as("SELECT CURRENT_USER(), DATABASE()")
            .fetch_one(&mut *source)
            .await?;
    let authenticated_username = current_user
        .rsplit_once('@')
        .map(|(username, _)| username)
        .ok_or_else(|| anyhow::anyhow!("legacy MySQL returned an invalid CURRENT_USER identity"))?;
    if authenticated_username != expected_username
        || current_database.as_deref() != Some(&expected_database)
    {
        anyhow::bail!(
            "legacy MySQL authenticated identity or selected database differs from source.database_url"
        );
    }

    let enabled_roles = sqlx::query_as::<_, (String, String)>(
        "SELECT role_name, role_host FROM information_schema.enabled_roles",
    )
    .fetch_all(&mut *source)
    .await?;
    if !enabled_roles.is_empty() {
        anyhow::bail!("legacy MySQL source account must not have any enabled role");
    }

    let grant_rows = sqlx::query("SHOW GRANTS FOR CURRENT_USER")
        .fetch_all(&mut *source)
        .await?;
    let mut grants = Vec::with_capacity(grant_rows.len());
    for row in grant_rows {
        grants.push(row.try_get::<String, _>(0)?);
    }
    validate_mysql_source_grants(&grants, &expected_database)
}

pub(crate) fn validate_mysql_source_grants(
    grants: &[String],
    database: &str,
) -> anyhow::Result<()> {
    let usage_prefix = "GRANT USAGE ON *.* TO ";
    let select_prefix = format!("GRANT SELECT ON `{database}`.* TO ");
    let mut saw_usage = false;
    let mut saw_select = false;
    for grant in grants {
        let recipient = if let Some(recipient) = grant.strip_prefix(usage_prefix) {
            saw_usage = true;
            recipient
        } else if let Some(recipient) = grant.strip_prefix(&select_prefix) {
            saw_select = true;
            recipient
        } else {
            anyhow::bail!(
                "legacy MySQL source account must have only USAGE and database-level SELECT"
            );
        };
        if recipient.is_empty() || recipient.contains(" WITH ") {
            anyhow::bail!(
                "legacy MySQL source account must not have GRANT OPTION or account resource grants"
            );
        }
    }
    if !saw_usage || !saw_select {
        anyhow::bail!(
            "legacy MySQL source account must have exactly database-level SELECT plus implicit USAGE"
        );
    }
    Ok(())
}

pub(crate) async fn begin_mysql_read_snapshot(source: &mut MySqlConnection) -> anyhow::Result<()> {
    (&mut *source)
        .execute("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .await?;
    (&mut *source)
        .execute("SET SESSION TRANSACTION_READ_ONLY = 1")
        .await?;
    (&mut *source)
        .execute("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await?;
    let (isolation, read_only): (String, i64) =
        sqlx::query_as("SELECT @@transaction_isolation, @@transaction_read_only")
            .fetch_one(&mut *source)
            .await?;
    if !isolation.eq_ignore_ascii_case("REPEATABLE-READ") || read_only != 1 {
        anyhow::bail!("legacy MySQL did not enter the required read-only repeatable-read snapshot");
    }
    Ok(())
}

pub(crate) async fn commit_mysql_read_snapshot(source: &mut MySqlConnection) -> anyhow::Result<()> {
    (&mut *source).execute("COMMIT").await?;
    Ok(())
}

pub(crate) async fn inspect_source_schema(
    source: &mut MySqlConnection,
) -> anyhow::Result<SourceSchemaInspection> {
    let actual_tables = sqlx::query_scalar::<_, String>(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' ORDER BY table_name",
    )
    .fetch_all(&mut *source)
    .await?
    .into_iter()
    .collect::<BTreeSet<_>>();
    let imported_tables = validate_source_table_inventory(&actual_tables)?;
    let mut engines_query = QueryBuilder::<MySql>::new(
        "SELECT table_name, COALESCE(engine, '') FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' AND table_name IN (",
    );
    {
        let mut separated = engines_query.separated(", ");
        for table in &imported_tables {
            separated.push_bind(table);
        }
    }
    engines_query.push(") ORDER BY table_name");
    let imported_engines = engines_query
        .build_query_as::<(String, String)>()
        .fetch_all(&mut *source)
        .await?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    validate_source_table_engines(&imported_tables, &imported_engines)?;

    for mapping in TABLE_MAPPINGS {
        let actual = sqlx::query_scalar::<_, String>(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = DATABASE() AND table_name = ? ORDER BY ordinal_position",
        )
        .bind(mapping.source)
        .fetch_all(&mut *source)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
        let expected = mapping_source_columns(mapping)
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        if actual != expected {
            anyhow::bail!(
                "legacy source columns drifted for {}: expected={expected:?}, observed={actual:?}",
                mapping.source
            );
        }
    }

    type ColumnDescriptor = (
        String,
        u32,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    );
    let mut columns_query = QueryBuilder::<MySql>::new(
        "SELECT table_name, ordinal_position, column_name, column_type, is_nullable, \
                COALESCE(CAST(column_default AS CHAR), '<NULL>'), extra, \
                COALESCE(character_set_name, ''), COALESCE(collation_name, ''), column_key \
         FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name IN (",
    );
    {
        let mut separated = columns_query.separated(", ");
        for table in &imported_tables {
            separated.push_bind(table);
        }
    }
    columns_query.push(") ORDER BY table_name, ordinal_position");
    let columns = columns_query
        .build_query_as::<ColumnDescriptor>()
        .fetch_all(&mut *source)
        .await?;
    type IndexDescriptor = (String, String, i32, u32, String, i64, String);
    let mut indexes_query = QueryBuilder::<MySql>::new(
        "SELECT table_name, index_name, non_unique, seq_in_index, column_name, \
                COALESCE(sub_part, 0), index_type \
         FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name IN (",
    );
    {
        let mut separated = indexes_query.separated(", ");
        for table in &imported_tables {
            separated.push_bind(table);
        }
    }
    indexes_query.push(") ORDER BY table_name, index_name, seq_in_index");
    let indexes = indexes_query
        .build_query_as::<IndexDescriptor>()
        .fetch_all(&mut *source)
        .await?;
    let mut digest = Sha256::new();
    digest.update(b"v2board.mysql-import.source-schema.v1\0");
    digest.update(b"tables\0");
    for table in &imported_tables {
        schema_digest_fields(&mut digest, [table.clone(), "InnoDB".to_string()]);
    }
    digest.update(b"columns\0");
    for row in columns {
        schema_digest_fields(
            &mut digest,
            [
                row.0,
                row.1.to_string(),
                row.2,
                row.3,
                row.4,
                row.5,
                row.6,
                row.7,
                row.8,
                row.9,
            ],
        );
    }
    digest.update(b"indexes\0");
    for row in indexes {
        schema_digest_fields(
            &mut digest,
            [
                row.0,
                row.1,
                row.2.to_string(),
                row.3.to_string(),
                row.4,
                row.5.to_string(),
                row.6,
            ],
        );
    }
    let discarded_tables = DISCARDED_SOURCE_TABLES
        .iter()
        .map(|table| DiscardedTableReport {
            source: (*table).to_string(),
            present: actual_tables.contains(*table),
            rows_scanned: false,
            policy: "allowlisted_full_table_discard_without_row_scan",
        })
        .collect();
    Ok(SourceSchemaInspection {
        imported_schema_sha256: hex::encode(digest.finalize()),
        discarded_tables,
    })
}

pub(crate) fn validate_source_table_inventory(
    actual_tables: &BTreeSet<String>,
) -> anyhow::Result<BTreeSet<String>> {
    let imported_tables = TABLE_MAPPINGS
        .iter()
        .map(|mapping| mapping.source.to_string())
        .collect::<BTreeSet<_>>();
    let allowed_tables = imported_tables
        .iter()
        .cloned()
        .chain(
            DISCARDED_SOURCE_TABLES
                .iter()
                .map(|table| (*table).to_string()),
        )
        .collect::<BTreeSet<_>>();
    let missing = imported_tables
        .difference(actual_tables)
        .cloned()
        .collect::<Vec<_>>();
    let unexpected = actual_tables
        .difference(&allowed_tables)
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() || !unexpected.is_empty() {
        anyhow::bail!(
            "legacy source table inventory is unsupported; missing imported tables={missing:?}, unexpected tables={unexpected:?}"
        );
    }
    Ok(imported_tables)
}

pub(crate) fn validate_source_table_engines(
    imported_tables: &BTreeSet<String>,
    imported_engines: &BTreeMap<String, String>,
) -> anyhow::Result<()> {
    if imported_engines.keys().collect::<BTreeSet<_>>()
        != imported_tables.iter().collect::<BTreeSet<_>>()
    {
        anyhow::bail!("legacy source storage-engine inventory is incomplete");
    }
    for (table, engine) in imported_engines {
        if !engine.eq_ignore_ascii_case("InnoDB") {
            anyhow::bail!(
                "legacy source imported table {table} must use InnoDB for the consistent snapshot"
            );
        }
    }
    Ok(())
}

fn schema_digest_fields<const N: usize>(digest: &mut Sha256, fields: [String; N]) {
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
}

fn mapping_source_columns(mapping: &TableMapping) -> Vec<&str> {
    mapping
        .direct_columns
        .iter()
        .copied()
        .chain(
            mapping
                .transformed_columns
                .iter()
                .map(|column| column.source),
        )
        .chain(
            mapping
                .consumed_source_columns
                .iter()
                .map(|column| column.source),
        )
        .collect()
}

pub(crate) fn mapping_has_source_column(mapping: &TableMapping, name: &str) -> bool {
    mapping.direct_columns.contains(&name)
        || mapping
            .transformed_columns
            .iter()
            .any(|column| column.source == name)
        || mapping
            .consumed_source_columns
            .iter()
            .any(|column| column.source == name)
}

pub(crate) async fn validate_source_relationships(
    source: &mut MySqlConnection,
) -> anyhow::Result<()> {
    for mapping in TABLE_MAPPINGS {
        let sql = format!("SELECT COUNT(*) FROM `{}` WHERE `id` <= 0", mapping.source);
        let invalid: i64 = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
            .fetch_one(&mut *source)
            .await?;
        if invalid != 0 {
            anyhow::bail!(
                "legacy source table {} has {invalid} non-positive business identity row(s); native identities must be positive",
                mapping.source
            );
        }
    }

    for reference in SCALAR_REFERENCES {
        let predicate = match reference.rule {
            ScalarReferenceRule::Required => format!(
                "s.`{column}` IS NULL OR r.`id` IS NULL",
                column = reference.column
            ),
            ScalarReferenceRule::Nullable => format!(
                "s.`{column}` IS NOT NULL AND r.`id` IS NULL",
                column = reference.column
            ),
            ScalarReferenceRule::ZeroMeansNoReference => format!(
                "s.`{column}` <> 0 AND r.`id` IS NULL",
                column = reference.column
            ),
        };
        let sql = format!(
            "SELECT COUNT(*) FROM `{source_table}` AS s \
             LEFT JOIN `{referenced_table}` AS r ON r.`id` = s.`{column}` WHERE {predicate}",
            source_table = reference.source_table,
            referenced_table = reference.source_referenced_table,
            column = reference.column,
        );
        let invalid: i64 = sqlx::query_scalar(AssertSqlSafe(sql).into_sql_str())
            .fetch_one(&mut *source)
            .await?;
        if invalid != 0 {
            anyhow::bail!(
                "legacy source relationship {}.{} -> {} has {invalid} invalid row(s)",
                reference.source_table,
                reference.column,
                reference.source_referenced_table
            );
        }
    }
    Ok(())
}
