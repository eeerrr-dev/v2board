use std::collections::HashSet;

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct ClickHouseMigration {
    pub version: u64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const CLICKHOUSE_MIGRATIONS: &[ClickHouseMigration] = &[
    ClickHouseMigration {
        version: 1,
        name: "schema_migration",
        sql: include_str!("../../../clickhouse-migrations/0001_schema_migration.sql"),
    },
    ClickHouseMigration {
        version: 2,
        name: "traffic_reported",
        sql: include_str!("../../../clickhouse-migrations/0002_traffic_reported.sql"),
    },
    ClickHouseMigration {
        version: 3,
        name: "traffic_accounted",
        sql: include_str!("../../../clickhouse-migrations/0003_traffic_accounted.sql"),
    },
    ClickHouseMigration {
        version: 4,
        name: "installation_binding",
        sql: include_str!("../../../clickhouse-migrations/0004_installation_binding.sql"),
    },
    ClickHouseMigration {
        version: 5,
        name: "traffic_reported_daily",
        sql: include_str!("../../../clickhouse-migrations/0005_traffic_reported_daily.sql"),
    },
    ClickHouseMigration {
        version: 6,
        name: "traffic_accounted_daily",
        sql: include_str!("../../../clickhouse-migrations/0006_traffic_accounted_daily.sql"),
    },
    ClickHouseMigration {
        version: 7,
        name: "retention_binding",
        sql: include_str!("../../../clickhouse-migrations/0007_retention_binding.sql"),
    },
];

const SUPPORTED_CLICKHOUSE_MAJOR: u64 = 26;
const SUPPORTED_CLICKHOUSE_MINOR: u64 = 3;
const REQUIRED_DEDUPLICATION_WINDOW: &str = "non_replicated_deduplication_window = 10000";

#[derive(Debug, thiserror::Error)]
pub enum ClickHouseMigrationError {
    #[error("ClickHouse migration versions must be strictly increasing")]
    InvalidOrder,
    #[error("ClickHouse migration {version} has conflicting ledger checksums")]
    ConflictingLedger { version: u64 },
    #[error("ClickHouse migration {version} checksum changed")]
    ChecksumMismatch { version: u64 },
    #[error("ClickHouse migration {version} ledger name changed")]
    NameMismatch { version: u64 },
    #[error("ClickHouse schema ledger contains unknown version {version}")]
    UnknownLedgerVersion { version: u64 },
    #[error("ClickHouse schema ledger contains duplicate version {version}")]
    DuplicateLedgerVersion { version: u64 },
    #[error("ClickHouse schema ledger has a gap at version {version}")]
    LedgerGap { version: u64 },
    #[error(
        "ClickHouse schema ledger is incomplete: expected {expected} versions, observed {observed}"
    )]
    IncompleteLineage { expected: usize, observed: usize },
    #[error("ClickHouse {observed} is unsupported; analytics requires 26.3.x")]
    UnsupportedVersion { observed: String },
    #[error("ClickHouse target database has {table_count} table(s) but no schema_migration ledger")]
    UnmanagedNonEmptyDatabase { table_count: u64 },
    #[error("ClickHouse target has a pre-existing schema ledger without a valid bootstrap entry")]
    IncompleteLedger,
    #[error("ClickHouse target database contains unexpected table {table}")]
    UnexpectedTable { table: String },
    #[error("ClickHouse installation binding is missing")]
    MissingInstallationBinding,
    #[error("ClickHouse installation binding is duplicated or malformed")]
    InvalidInstallationBinding,
    #[error("ClickHouse database is already bound to a different installation")]
    InstallationBindingConflict,
    #[error("ClickHouse retention binding is missing")]
    MissingRetentionBinding,
    #[error("ClickHouse retention binding is duplicated or malformed")]
    InvalidRetentionBinding,
    #[error("ClickHouse retention is already bound to different values or installation")]
    RetentionBindingConflict,
    #[error("ClickHouse retention must be nonzero, aggregate >= raw, and at most 36500 days")]
    InvalidRetention,
    #[error(
        "ClickHouse retention may be bound only while raw and aggregate analytics tables are empty"
    )]
    NonEmptyBeforeRetentionBinding,
    #[error("ClickHouse schema invariant failed for {table}: {detail}")]
    SchemaInvariant { table: String, detail: String },
    #[error("ClickHouse migration failed: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
}

#[derive(Debug, Deserialize, Row)]
struct LedgerRow {
    version: u64,
    name: String,
    checksum: String,
}

#[derive(Debug, Row, Serialize)]
struct AppliedMigration<'a> {
    version: u64,
    name: &'a str,
    checksum: &'a str,
    applied_at_unix: i64,
}

#[derive(Debug, Deserialize, Row)]
struct StringValue {
    value: String,
}

#[derive(Debug, Deserialize, Row)]
struct CountValue {
    value: u64,
}

#[derive(Debug, Deserialize, Row)]
struct TableState {
    engine: String,
    partition_key: String,
    sorting_key: String,
    create_table_query: String,
}

#[derive(Debug, Deserialize, Row, Eq, PartialEq)]
struct ColumnState {
    name: String,
    type_name: String,
}

#[derive(Debug, Deserialize, Row)]
struct IndexState {
    name: String,
    type_full: String,
    expr: String,
    granularity: u64,
}

#[derive(Debug, Deserialize, Row)]
struct TableName {
    name: String,
}

#[derive(Debug, Deserialize, Row)]
struct InstallationBindingRow {
    singleton: u8,
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
}

#[derive(Debug, Deserialize, Row)]
struct InstallationIdRow {
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
}

#[derive(Debug, Row, Serialize)]
struct NewInstallationBinding {
    singleton: u8,
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
    bound_at_unix: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Row, Serialize)]
pub struct ClickHouseProjectionCounts {
    pub reported_raw_rows: u64,
    pub accounted_raw_rows: u64,
    pub reported_daily_rows: u64,
    pub accounted_daily_rows: u64,
}

impl ClickHouseProjectionCounts {
    pub const fn is_empty(self) -> bool {
        self.reported_raw_rows == 0
            && self.accounted_raw_rows == 0
            && self.reported_daily_rows == 0
            && self.accounted_daily_rows == 0
    }
}

#[derive(Debug, Deserialize, Row)]
struct RetentionBindingRow {
    singleton: u8,
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
    raw_retention_days: u32,
    aggregate_retention_days: u32,
}

#[derive(Debug, Row, Serialize)]
struct NewRetentionBinding {
    singleton: u8,
    #[serde(with = "clickhouse::serde::uuid")]
    installation_id: Uuid,
    raw_retention_days: u32,
    aggregate_retention_days: u32,
    bound_at_unix: i64,
}

/// Apply the independent ClickHouse migration lineage.
///
/// The provisioner must serialize schema migration principals. The ledger
/// still detects checksum drift and conflicting concurrent history instead of
/// treating `IF NOT EXISTS` as proof that the expected table exists.
pub async fn migrate_clickhouse(
    client: &clickhouse::Client,
    now_unix: i64,
) -> Result<(), ClickHouseMigrationError> {
    if CLICKHOUSE_MIGRATIONS
        .iter()
        .enumerate()
        .any(|(index, migration)| migration.version != index as u64 + 1)
    {
        return Err(ClickHouseMigrationError::InvalidOrder);
    }

    ensure_supported_version(client).await?;

    let ledger_preexisting = table_exists(client, "schema_migration").await?;
    let preexisting_tables = table_names(client).await?;
    if !ledger_preexisting {
        if !preexisting_tables.is_empty() {
            return Err(ClickHouseMigrationError::UnmanagedNonEmptyDatabase {
                table_count: preexisting_tables.len() as u64,
            });
        }
    } else {
        reject_unknown_tables(&preexisting_tables)?;
    }

    // Bootstrap the ledger before it can describe itself.
    client.query(CLICKHOUSE_MIGRATIONS[0].sql).execute().await?;
    // ClickHouse has no transactional DDL. Make the bootstrap ledger itself
    // retry-idempotent before writing its first row, including recovery after
    // a crash between CREATE TABLE and the version-1 ledger insert.
    client
        .query(
            "ALTER TABLE schema_migration MODIFY SETTING \
             non_replicated_deduplication_window = 10000",
        )
        .execute()
        .await?;
    validate_ledger_table(client).await?;

    let initial_prefix = validate_ledger_lineage(client, false).await?;
    if ledger_preexisting && initial_prefix == 0 {
        let tables = table_names(client).await?;
        if tables.len() != 1 || tables[0] != "schema_migration" {
            return Err(ClickHouseMigrationError::IncompleteLedger);
        }
    }

    for (index, migration) in CLICKHOUSE_MIGRATIONS.iter().enumerate() {
        let prefix = validate_ledger_lineage(client, false).await?;
        if prefix > index {
            continue;
        }
        if prefix < index {
            return Err(ClickHouseMigrationError::LedgerGap {
                version: index as u64 + 1,
            });
        }

        client.query(migration.sql).execute().await?;
        validate_migration_effect(client, migration.version).await?;
        insert_migration_ledger_row(client, migration, now_unix).await?;
        let observed = validate_ledger_lineage(client, false).await?;
        if observed <= index {
            return Err(ClickHouseMigrationError::ConflictingLedger {
                version: migration.version,
            });
        }
    }
    validate_ledger_lineage(client, true).await?;
    validate_clickhouse_schema(client).await?;
    Ok(())
}

/// Validate the exact embedded schema and atomically bind an empty analytics
/// database to one PostgreSQL installation. On ordinary single-node
/// MergeTree, every contender uses the same deduplication token deliberately:
/// exactly one installation row can win and all losers fail verification.
pub async fn bind_clickhouse_installation(
    client: &clickhouse::Client,
    installation_id: Uuid,
    now_unix: i64,
) -> Result<(), ClickHouseMigrationError> {
    validate_runtime_schema_structure(client).await?;
    let rows = installation_binding_rows(client).await?;
    if rows.is_empty() {
        let fact_installations = raw_fact_installations(client).await?;
        if fact_installations.len() > 1
            || fact_installations
                .first()
                .is_some_and(|observed| *observed != installation_id)
        {
            return Err(ClickHouseMigrationError::InstallationBindingConflict);
        }
        let mut insert = client
            .insert::<NewInstallationBinding>("installation_binding")
            .await?
            .with_setting(
                "insert_deduplication_token",
                "v2board.analytics-installation-binding.v1",
            )
            .with_setting("async_insert", "0")
            .with_setting("wait_end_of_query", "1");
        insert
            .write(&NewInstallationBinding {
                singleton: 1,
                installation_id,
                bound_at_unix: now_unix,
            })
            .await?;
        insert.end().await?;
    }
    verify_installation_binding(client, installation_id).await
}

/// Configure the exact raw/daily-aggregate retention contract and bind it to
/// the same installation as PostgreSQL. The first application is deliberately
/// restricted to an empty projection: ClickHouse DDL is not transactional, so
/// a crash is recovered by re-applying the idempotent TTL clauses before the
/// single deduplicated binding row is written. Once bound, changing either
/// value requires an explicit future schema change rather than silently
/// shortening retained history.
pub async fn configure_clickhouse_retention(
    client: &clickhouse::Client,
    installation_id: Uuid,
    raw_retention_days: u32,
    aggregate_retention_days: u32,
    now_unix: i64,
) -> Result<(), ClickHouseMigrationError> {
    validate_retention_days(raw_retention_days, aggregate_retention_days)?;
    validate_runtime_schema_structure(client).await?;
    verify_installation_binding(client, installation_id).await?;

    let bindings = retention_binding_rows(client).await?;
    if bindings.is_empty() {
        if !clickhouse_projection_counts(client).await?.is_empty() {
            return Err(ClickHouseMigrationError::NonEmptyBeforeRetentionBinding);
        }
        apply_retention_ttl(client, RAW_RETENTION_TABLES, raw_retention_days).await?;
        apply_retention_ttl(client, AGGREGATE_RETENTION_TABLES, aggregate_retention_days).await?;
        // A schema principal racing a writer is outside the supported topology,
        // but recheck immediately before sealing so such a violation fails
        // closed instead of blessing ungoverned rows.
        if !clickhouse_projection_counts(client).await?.is_empty() {
            return Err(ClickHouseMigrationError::NonEmptyBeforeRetentionBinding);
        }
        verify_retention_ttl(client, raw_retention_days, aggregate_retention_days).await?;
        let mut insert = client
            .insert::<NewRetentionBinding>("retention_binding")
            .await?
            .with_setting(
                "insert_deduplication_token",
                "v2board.analytics-retention-binding.v1",
            )
            .with_setting("async_insert", "0")
            .with_setting("wait_end_of_query", "1");
        insert
            .write(&NewRetentionBinding {
                singleton: 1,
                installation_id,
                raw_retention_days,
                aggregate_retention_days,
                bound_at_unix: now_unix,
            })
            .await?;
        insert.end().await?;
    }

    verify_retention_binding(
        client,
        installation_id,
        raw_retention_days,
        aggregate_retention_days,
    )
    .await?;
    verify_retention_ttl(client, raw_retention_days, aggregate_retention_days).await
}

/// Exact raw and immutable batch-daily row counts. The one-shot import verifies
/// all four tables are empty before native events can start; legacy daily
/// summaries remain in PostgreSQL instead of being misrepresented as native
/// raw events.
pub async fn clickhouse_projection_counts(
    client: &clickhouse::Client,
) -> Result<ClickHouseProjectionCounts, ClickHouseMigrationError> {
    Ok(client
        .query(
            "SELECT \
             coalesce((SELECT count() FROM traffic_reported), toUInt64(0)) \
                 AS reported_raw_rows, \
             coalesce((SELECT count() FROM traffic_accounted), toUInt64(0)) \
                 AS accounted_raw_rows, \
             coalesce((SELECT count() FROM traffic_reported_daily), toUInt64(0)) \
                 AS reported_daily_rows, \
             coalesce((SELECT count() FROM traffic_accounted_daily), toUInt64(0)) \
                 AS accounted_daily_rows",
        )
        .fetch_one::<ClickHouseProjectionCounts>()
        .await?)
}

/// Domain-separated digest of the exact embedded ClickHouse lineage. This is
/// stable across hosts and excludes application timestamps.
pub fn clickhouse_schema_lineage_sha256() -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.clickhouse-schema-lineage.v1\0");
    for migration in CLICKHOUSE_MIGRATIONS {
        digest.update(migration.version.to_be_bytes());
        digest_field(&mut digest, migration.name.as_bytes());
        digest_field(&mut digest, checksum(migration.sql.as_bytes()).as_bytes());
    }
    hex::encode(digest.finalize())
}

async fn raw_fact_installations(
    client: &clickhouse::Client,
) -> Result<Vec<Uuid>, ClickHouseMigrationError> {
    Ok(client
        .query(
            "SELECT DISTINCT installation_id FROM ( \
                 SELECT installation_id FROM traffic_reported \
                 UNION ALL \
                 SELECT installation_id FROM traffic_accounted \
             ) ORDER BY installation_id LIMIT 2",
        )
        .fetch_all::<InstallationIdRow>()
        .await?
        .into_iter()
        .map(|row| row.installation_id)
        .collect())
}

/// Per-batch fail-closed readiness check. This is intentionally read-only:
/// schema and installation binding belong exclusively to provisioning, never
/// to an API or worker runtime credential.
pub async fn verify_clickhouse_runtime_ready(
    client: &clickhouse::Client,
    installation_id: Uuid,
) -> Result<(), ClickHouseMigrationError> {
    validate_runtime_schema_structure(client).await?;
    verify_installation_binding(client, installation_id).await?;
    let binding = exact_retention_binding(client).await?;
    if binding.installation_id != installation_id {
        return Err(ClickHouseMigrationError::RetentionBindingConflict);
    }
    verify_retention_ttl(
        client,
        binding.raw_retention_days,
        binding.aggregate_retention_days,
    )
    .await
}

/// Read-only lifecycle verification of the exact manifest-bound ClickHouse
/// contract. Unlike configure/bind this function can never repair drift.
pub async fn verify_clickhouse_bound_contract(
    client: &clickhouse::Client,
    installation_id: Uuid,
    raw_retention_days: u32,
    aggregate_retention_days: u32,
) -> Result<(), ClickHouseMigrationError> {
    validate_runtime_schema_structure(client).await?;
    verify_installation_binding(client, installation_id).await?;
    verify_retention_binding(
        client,
        installation_id,
        raw_retention_days,
        aggregate_retention_days,
    )
    .await?;
    verify_retention_ttl(client, raw_retention_days, aggregate_retention_days).await
}

async fn validate_runtime_schema_structure(
    client: &clickhouse::Client,
) -> Result<(), ClickHouseMigrationError> {
    ensure_supported_version(client).await?;
    validate_ledger_lineage(client, true).await?;
    validate_clickhouse_schema(client).await
}

async fn retention_binding_rows(
    client: &clickhouse::Client,
) -> Result<Vec<RetentionBindingRow>, ClickHouseMigrationError> {
    Ok(client
        .query(
            "SELECT singleton, installation_id, raw_retention_days, aggregate_retention_days \
             FROM retention_binding ORDER BY singleton, installation_id, raw_retention_days, \
             aggregate_retention_days",
        )
        .fetch_all::<RetentionBindingRow>()
        .await?)
}

async fn exact_retention_binding(
    client: &clickhouse::Client,
) -> Result<RetentionBindingRow, ClickHouseMigrationError> {
    let mut rows = retention_binding_rows(client).await?;
    if rows.is_empty() {
        return Err(ClickHouseMigrationError::MissingRetentionBinding);
    }
    if rows.len() != 1 || rows[0].singleton != 1 {
        return Err(ClickHouseMigrationError::InvalidRetentionBinding);
    }
    Ok(rows.remove(0))
}

async fn verify_retention_binding(
    client: &clickhouse::Client,
    installation_id: Uuid,
    raw_retention_days: u32,
    aggregate_retention_days: u32,
) -> Result<(), ClickHouseMigrationError> {
    let binding = exact_retention_binding(client).await?;
    if binding.installation_id != installation_id
        || binding.raw_retention_days != raw_retention_days
        || binding.aggregate_retention_days != aggregate_retention_days
    {
        return Err(ClickHouseMigrationError::RetentionBindingConflict);
    }
    Ok(())
}

fn validate_retention_days(raw: u32, aggregate: u32) -> Result<(), ClickHouseMigrationError> {
    if raw == 0 || aggregate < raw || aggregate > 36_500 {
        return Err(ClickHouseMigrationError::InvalidRetention);
    }
    Ok(())
}

async fn apply_retention_ttl(
    client: &clickhouse::Client,
    tables: &[&str],
    days: u32,
) -> Result<(), ClickHouseMigrationError> {
    for table in tables {
        client
            .query(&format!(
                "ALTER TABLE {table} MODIFY TTL accounting_date + toIntervalDay({days}) DELETE"
            ))
            .execute()
            .await?;
    }
    Ok(())
}

async fn verify_retention_ttl(
    client: &clickhouse::Client,
    raw_retention_days: u32,
    aggregate_retention_days: u32,
) -> Result<(), ClickHouseMigrationError> {
    validate_retention_days(raw_retention_days, aggregate_retention_days)?;
    for (tables, days) in [
        (RAW_RETENTION_TABLES, raw_retention_days),
        (AGGREGATE_RETENTION_TABLES, aggregate_retention_days),
    ] {
        let expected = format!("TTL accounting_date + toIntervalDay({days})");
        for table in tables {
            let state = table_state(client, table).await?;
            let normalized = normalize_sql(&state.create_table_query);
            if normalized.matches(" TTL ").count() != 1
                || !normalized.contains(&format!(" {expected} "))
            {
                return Err(schema_error(table, "retention TTL is missing or drifted"));
            }
        }
    }
    Ok(())
}

async fn installation_binding_rows(
    client: &clickhouse::Client,
) -> Result<Vec<InstallationBindingRow>, ClickHouseMigrationError> {
    Ok(client
        .query(
            "SELECT singleton, installation_id FROM installation_binding \
             ORDER BY singleton, installation_id",
        )
        .fetch_all::<InstallationBindingRow>()
        .await?)
}

async fn verify_installation_binding(
    client: &clickhouse::Client,
    installation_id: Uuid,
) -> Result<(), ClickHouseMigrationError> {
    let rows = installation_binding_rows(client).await?;
    if rows.is_empty() {
        return Err(ClickHouseMigrationError::MissingInstallationBinding);
    }
    if rows.len() != 1 || rows[0].singleton != 1 {
        return Err(ClickHouseMigrationError::InvalidInstallationBinding);
    }
    if rows[0].installation_id != installation_id {
        return Err(ClickHouseMigrationError::InstallationBindingConflict);
    }
    Ok(())
}

async fn ensure_supported_version(
    client: &clickhouse::Client,
) -> Result<(), ClickHouseMigrationError> {
    let observed = client
        .query("SELECT version() AS value")
        .fetch_one::<StringValue>()
        .await?
        .value;
    let supported = parse_version_major_minor(&observed)
        .is_some_and(|version| version == (SUPPORTED_CLICKHOUSE_MAJOR, SUPPORTED_CLICKHOUSE_MINOR));
    if !supported {
        return Err(ClickHouseMigrationError::UnsupportedVersion { observed });
    }
    Ok(())
}

fn parse_version_major_minor(value: &str) -> Option<(u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

async fn table_exists(
    client: &clickhouse::Client,
    table: &str,
) -> Result<bool, ClickHouseMigrationError> {
    let count = client
        .query(
            "SELECT count() AS value FROM system.tables \
             WHERE database = currentDatabase() AND name = ?",
        )
        .bind(table)
        .fetch_one::<CountValue>()
        .await?
        .value;
    Ok(count == 1)
}

async fn table_names(client: &clickhouse::Client) -> Result<Vec<String>, ClickHouseMigrationError> {
    Ok(client
        .query(
            "SELECT name FROM system.tables \
             WHERE database = currentDatabase() ORDER BY name",
        )
        .fetch_all::<TableName>()
        .await?
        .into_iter()
        .map(|row| row.name)
        .collect())
}

fn reject_unknown_tables(tables: &[String]) -> Result<(), ClickHouseMigrationError> {
    for table in tables {
        if !EXPECTED_TABLES.contains(&table.as_str()) {
            return Err(ClickHouseMigrationError::UnexpectedTable {
                table: table.clone(),
            });
        }
    }
    Ok(())
}

async fn ledger_rows(
    client: &clickhouse::Client,
) -> Result<Vec<LedgerRow>, ClickHouseMigrationError> {
    Ok(client
        .query(
            "SELECT version, name, checksum FROM schema_migration \
             ORDER BY version, applied_at_unix, checksum, name",
        )
        .fetch_all::<LedgerRow>()
        .await?)
}

async fn validate_ledger_lineage(
    client: &clickhouse::Client,
    exact: bool,
) -> Result<usize, ClickHouseMigrationError> {
    let rows = ledger_rows(client).await?;
    validate_ledger_rows(&rows, exact)
}

fn validate_ledger_rows(
    rows: &[LedgerRow],
    exact: bool,
) -> Result<usize, ClickHouseMigrationError> {
    let mut seen = HashSet::with_capacity(rows.len());
    for row in rows {
        if !seen.insert(row.version) {
            return Err(ClickHouseMigrationError::DuplicateLedgerVersion {
                version: row.version,
            });
        }
        let Some(expected) = CLICKHOUSE_MIGRATIONS
            .iter()
            .find(|migration| migration.version == row.version)
        else {
            return Err(ClickHouseMigrationError::UnknownLedgerVersion {
                version: row.version,
            });
        };
        if row.checksum != checksum(expected.sql.as_bytes()) {
            return Err(ClickHouseMigrationError::ChecksumMismatch {
                version: row.version,
            });
        }
        if row.name != expected.name {
            return Err(ClickHouseMigrationError::NameMismatch {
                version: row.version,
            });
        }
    }

    let mut prefix = 0_usize;
    for migration in CLICKHOUSE_MIGRATIONS {
        if seen.contains(&migration.version) {
            prefix += 1;
        } else {
            if rows.iter().any(|row| row.version > migration.version) {
                return Err(ClickHouseMigrationError::LedgerGap {
                    version: migration.version,
                });
            }
            break;
        }
    }
    if exact && prefix != CLICKHOUSE_MIGRATIONS.len() {
        return Err(ClickHouseMigrationError::IncompleteLineage {
            expected: CLICKHOUSE_MIGRATIONS.len(),
            observed: prefix,
        });
    }
    Ok(prefix)
}

async fn insert_migration_ledger_row(
    client: &clickhouse::Client,
    migration: &ClickHouseMigration,
    now_unix: i64,
) -> Result<(), ClickHouseMigrationError> {
    let checksum = checksum(migration.sql.as_bytes());
    let token = format!(
        "v2board.clickhouse-schema-ledger.v1.{}.{}",
        migration.version, checksum
    );
    let mut insert = client
        .insert::<AppliedMigration<'_>>("schema_migration")
        .await?
        .with_setting("insert_deduplication_token", token)
        .with_setting("async_insert", "0")
        .with_setting("wait_end_of_query", "1");
    insert
        .write(&AppliedMigration {
            version: migration.version,
            name: migration.name,
            checksum: &checksum,
            applied_at_unix: now_unix,
        })
        .await?;
    insert.end().await?;
    Ok(())
}

async fn validate_migration_effect(
    client: &clickhouse::Client,
    version: u64,
) -> Result<(), ClickHouseMigrationError> {
    match version {
        1 => validate_ledger_table(client).await,
        2 => validate_event_table(
            client,
            "traffic_reported",
            "installation_id, user_id, accounting_date, accepted_at_unix, event_id, ingest_batch_id, batch_row_number",
            REPORTED_COLUMNS,
        )
        .await,
        3 => validate_event_table(
            client,
            "traffic_accounted",
            "installation_id, user_id, accounting_date, accounted_at_unix, event_id, ingest_batch_id, batch_row_number",
            ACCOUNTED_COLUMNS,
        )
        .await,
        4 => validate_installation_binding_table(client).await,
        5 => validate_daily_aggregate_table(
            client,
            "traffic_reported_daily",
            "installation_id, schema_major, accounting_date, user_id, server_id, server_type, rate_text, rate_decimal_10_2, ingest_batch_id, batch_aggregate_row_number",
            REPORTED_DAILY_COLUMNS,
        )
        .await,
        6 => validate_daily_aggregate_table(
            client,
            "traffic_accounted_daily",
            "installation_id, schema_major, accounting_date, user_id, server_id, server_type, rate_text, rate_decimal_10_2, outcome, ingest_batch_id, batch_aggregate_row_number",
            ACCOUNTED_DAILY_COLUMNS,
        )
        .await,
        7 => validate_retention_binding_table(client).await,
        _ => Err(ClickHouseMigrationError::UnknownLedgerVersion { version }),
    }
}

async fn validate_clickhouse_schema(
    client: &clickhouse::Client,
) -> Result<(), ClickHouseMigrationError> {
    let tables = table_names(client).await?;
    reject_unknown_tables(&tables)?;
    if tables.len() != EXPECTED_TABLES.len() {
        return Err(ClickHouseMigrationError::SchemaInvariant {
            table: "currentDatabase()".to_owned(),
            detail: "managed table inventory is incomplete".to_owned(),
        });
    }
    validate_ledger_table(client).await?;
    validate_event_table(
        client,
        "traffic_reported",
        "installation_id, user_id, accounting_date, accepted_at_unix, event_id, ingest_batch_id, batch_row_number",
        REPORTED_COLUMNS,
    )
    .await?;
    validate_event_table(
        client,
        "traffic_accounted",
        "installation_id, user_id, accounting_date, accounted_at_unix, event_id, ingest_batch_id, batch_row_number",
        ACCOUNTED_COLUMNS,
    )
    .await?;
    validate_installation_binding_table(client).await?;
    validate_daily_aggregate_table(
        client,
        "traffic_reported_daily",
        "installation_id, schema_major, accounting_date, user_id, server_id, server_type, rate_text, rate_decimal_10_2, ingest_batch_id, batch_aggregate_row_number",
        REPORTED_DAILY_COLUMNS,
    )
    .await?;
    validate_daily_aggregate_table(
        client,
        "traffic_accounted_daily",
        "installation_id, schema_major, accounting_date, user_id, server_id, server_type, rate_text, rate_decimal_10_2, outcome, ingest_batch_id, batch_aggregate_row_number",
        ACCOUNTED_DAILY_COLUMNS,
    )
    .await?;
    validate_retention_binding_table(client).await?;
    Ok(())
}

async fn validate_ledger_table(
    client: &clickhouse::Client,
) -> Result<(), ClickHouseMigrationError> {
    let state = validate_table(
        client,
        "schema_migration",
        "MergeTree",
        "",
        "version, applied_at_unix, checksum",
        LEDGER_COLUMNS,
    )
    .await?;
    if !state
        .create_table_query
        .contains(REQUIRED_DEDUPLICATION_WINDOW)
    {
        return Err(schema_error(
            "schema_migration",
            "schema-ledger retry deduplication is not explicitly enabled",
        ));
    }
    Ok(())
}

async fn validate_event_table(
    client: &clickhouse::Client,
    table: &str,
    sorting_key: &str,
    expected_columns: &[(&str, &str)],
) -> Result<(), ClickHouseMigrationError> {
    validate_event_table_shape(client, table, sorting_key, expected_columns).await?;
    validate_event_deduplication(client, table).await?;
    validate_event_index(client, table).await
}

async fn validate_event_table_shape(
    client: &clickhouse::Client,
    table: &str,
    sorting_key: &str,
    expected_columns: &[(&str, &str)],
) -> Result<TableState, ClickHouseMigrationError> {
    validate_table(
        client,
        table,
        "MergeTree",
        "toYYYYMM(accounting_date)",
        sorting_key,
        expected_columns,
    )
    .await
}

async fn validate_event_deduplication(
    client: &clickhouse::Client,
    table: &str,
) -> Result<(), ClickHouseMigrationError> {
    let states = client
        .query(
            "SELECT engine, partition_key, sorting_key, create_table_query \
             FROM system.tables WHERE database = currentDatabase() AND name = ?",
        )
        .bind(table)
        .fetch_all::<TableState>()
        .await?;
    if states.len() != 1 {
        return Err(schema_error(table, "table is missing or duplicated"));
    }
    let state = &states[0];
    if !state
        .create_table_query
        .contains(REQUIRED_DEDUPLICATION_WINDOW)
    {
        return Err(schema_error(
            table,
            "non-replicated retry deduplication is not explicitly enabled",
        ));
    }
    Ok(())
}

async fn validate_event_index(
    client: &clickhouse::Client,
    table: &str,
) -> Result<(), ClickHouseMigrationError> {
    let indices = client
        .query(
            "SELECT name, type_full, expr, granularity \
             FROM system.data_skipping_indices \
             WHERE database = currentDatabase() AND table = ? AND name = 'idx_ingest_batch_id'",
        )
        .bind(table)
        .fetch_all::<IndexState>()
        .await?;
    if indices.len() != 1 {
        return Err(schema_error(
            table,
            "batch-id skipping index is missing or duplicated",
        ));
    }
    let index = &indices[0];
    if index.name != "idx_ingest_batch_id"
        || index.type_full != "bloom_filter(0.001)"
        || index.expr != "ingest_batch_id"
        || index.granularity != 1
    {
        return Err(schema_error(
            table,
            "batch-id skipping index definition drifted",
        ));
    }
    Ok(())
}

async fn validate_installation_binding_table(
    client: &clickhouse::Client,
) -> Result<(), ClickHouseMigrationError> {
    let state = validate_table(
        client,
        "installation_binding",
        "MergeTree",
        "",
        "singleton, installation_id",
        INSTALLATION_BINDING_COLUMNS,
    )
    .await?;
    if !state
        .create_table_query
        .contains(REQUIRED_DEDUPLICATION_WINDOW)
        || !state
            .create_table_query
            .contains("CONSTRAINT chk_single_installation_binding CHECK singleton = 1")
    {
        return Err(schema_error(
            "installation_binding",
            "singleton constraint or retry deduplication setting drifted",
        ));
    }
    Ok(())
}

async fn validate_retention_binding_table(
    client: &clickhouse::Client,
) -> Result<(), ClickHouseMigrationError> {
    let state = validate_table(
        client,
        "retention_binding",
        "MergeTree",
        "",
        "singleton, installation_id",
        RETENTION_BINDING_COLUMNS,
    )
    .await?;
    let create = normalize_sql(&state.create_table_query);
    for required in [
        REQUIRED_DEDUPLICATION_WINDOW,
        "CONSTRAINT chk_single_retention_binding CHECK singleton = 1",
        "CONSTRAINT chk_raw_retention_positive CHECK raw_retention_days > 0",
        "CONSTRAINT chk_aggregate_retention_order CHECK aggregate_retention_days >= raw_retention_days",
    ] {
        if !create.contains(required) {
            return Err(schema_error(
                "retention_binding",
                "retention constraints or retry deduplication setting drifted",
            ));
        }
    }
    Ok(())
}

async fn validate_daily_aggregate_table(
    client: &clickhouse::Client,
    table: &str,
    sorting_key: &str,
    columns: &[(&str, &str)],
) -> Result<(), ClickHouseMigrationError> {
    let state = validate_table(
        client,
        table,
        "MergeTree",
        "toYYYYMM(accounting_date)",
        sorting_key,
        columns,
    )
    .await?;
    let create = normalize_sql(&state.create_table_query);
    if !create.contains(REQUIRED_DEDUPLICATION_WINDOW) {
        return Err(schema_error(
            table,
            "daily batch aggregate retry deduplication drifted",
        ));
    }
    validate_event_index(client, table).await
}

async fn validate_table(
    client: &clickhouse::Client,
    table: &str,
    engine: &str,
    partition_key: &str,
    sorting_key: &str,
    expected_columns: &[(&str, &str)],
) -> Result<TableState, ClickHouseMigrationError> {
    let state = table_state(client, table).await?;
    if state.engine != engine
        || state.partition_key != partition_key
        || state.sorting_key != sorting_key
    {
        return Err(schema_error(
            table,
            "engine, partition key, or sorting key drifted",
        ));
    }
    let columns = client
        .query(
            "SELECT name, type AS type_name FROM system.columns \
             WHERE database = currentDatabase() AND table = ? ORDER BY position",
        )
        .bind(table)
        .fetch_all::<ColumnState>()
        .await?;
    let expected = expected_columns
        .iter()
        .map(|(name, type_name)| ColumnState {
            name: (*name).to_owned(),
            type_name: (*type_name).to_owned(),
        })
        .collect::<Vec<_>>();
    if columns != expected {
        return Err(schema_error(table, "column order, name, or type drifted"));
    }
    Ok(state)
}

async fn table_state(
    client: &clickhouse::Client,
    table: &str,
) -> Result<TableState, ClickHouseMigrationError> {
    let mut states = client
        .query(
            "SELECT engine, partition_key, sorting_key, create_table_query \
             FROM system.tables WHERE database = currentDatabase() AND name = ?",
        )
        .bind(table)
        .fetch_all::<TableState>()
        .await?;
    if states.len() != 1 {
        return Err(schema_error(table, "table is missing or duplicated"));
    }
    states
        .pop()
        .ok_or_else(|| schema_error(table, "table state disappeared during validation"))
}

fn schema_error(table: &str, detail: &str) -> ClickHouseMigrationError {
    ClickHouseMigrationError::SchemaInvariant {
        table: table.to_owned(),
        detail: detail.to_owned(),
    }
}

const EXPECTED_TABLES: &[&str] = &[
    "installation_binding",
    "retention_binding",
    "schema_migration",
    "traffic_accounted",
    "traffic_accounted_daily",
    "traffic_reported",
    "traffic_reported_daily",
];

const RAW_RETENTION_TABLES: &[&str] = &["traffic_reported", "traffic_accounted"];
const AGGREGATE_RETENTION_TABLES: &[&str] = &["traffic_reported_daily", "traffic_accounted_daily"];

const LEDGER_COLUMNS: &[(&str, &str)] = &[
    ("version", "UInt64"),
    ("name", "String"),
    ("checksum", "String"),
    ("applied_at_unix", "Int64"),
];

const INSTALLATION_BINDING_COLUMNS: &[(&str, &str)] = &[
    ("singleton", "UInt8"),
    ("installation_id", "UUID"),
    ("bound_at_unix", "Int64"),
];

const RETENTION_BINDING_COLUMNS: &[(&str, &str)] = &[
    ("singleton", "UInt8"),
    ("installation_id", "UUID"),
    ("raw_retention_days", "UInt32"),
    ("aggregate_retention_days", "UInt32"),
    ("bound_at_unix", "Int64"),
];

const REPORTED_COLUMNS: &[(&str, &str)] = &[
    ("event_id", "String"),
    ("schema_major", "UInt16"),
    ("installation_id", "UUID"),
    ("report_key", "String"),
    ("payload_hash", "String"),
    ("identity_kind", "LowCardinality(String)"),
    ("user_id", "UInt64"),
    ("traffic_epoch", "UInt64"),
    ("server_id", "UInt64"),
    ("server_type", "LowCardinality(String)"),
    ("rate_text", "String"),
    ("rate_decimal_10_2", "Decimal(10, 2)"),
    ("raw_u", "UInt64"),
    ("raw_d", "UInt64"),
    ("charged_u", "UInt64"),
    ("charged_d", "UInt64"),
    ("accepted_at_unix", "Int64"),
    ("accounting_date", "Date"),
    ("accounting_timezone", "LowCardinality(String)"),
    ("ingest_batch_id", "UUID"),
    ("batch_row_number", "UInt32"),
    ("outbox_payload_sha256", "String"),
    ("ingested_at_unix", "Int64"),
];

const ACCOUNTED_COLUMNS: &[(&str, &str)] = &[
    ("event_id", "String"),
    ("schema_major", "UInt16"),
    ("installation_id", "UUID"),
    ("report_key", "String"),
    ("payload_hash", "String"),
    ("identity_kind", "LowCardinality(String)"),
    ("user_id", "UInt64"),
    ("traffic_epoch", "UInt64"),
    ("server_id", "UInt64"),
    ("server_type", "LowCardinality(String)"),
    ("rate_text", "String"),
    ("rate_decimal_10_2", "Decimal(10, 2)"),
    ("raw_u", "UInt64"),
    ("raw_d", "UInt64"),
    ("charged_u", "UInt64"),
    ("charged_d", "UInt64"),
    ("accepted_at_unix", "Int64"),
    ("accounting_date", "Date"),
    ("accounting_timezone", "LowCardinality(String)"),
    ("accounted_at_unix", "Int64"),
    ("outcome", "LowCardinality(String)"),
    ("u_after", "Nullable(UInt64)"),
    ("d_after", "Nullable(UInt64)"),
    ("ingest_batch_id", "UUID"),
    ("batch_row_number", "UInt32"),
    ("outbox_payload_sha256", "String"),
    ("ingested_at_unix", "Int64"),
];

const REPORTED_DAILY_COLUMNS: &[(&str, &str)] = &[
    ("installation_id", "UUID"),
    ("schema_major", "UInt16"),
    ("accounting_date", "Date"),
    ("user_id", "UInt64"),
    ("server_id", "UInt64"),
    ("server_type", "LowCardinality(String)"),
    ("rate_text", "String"),
    ("rate_decimal_10_2", "Decimal(10, 2)"),
    ("ingest_batch_id", "UUID"),
    ("batch_aggregate_row_number", "UInt32"),
    ("event_count", "UInt64"),
    ("raw_u", "UInt64"),
    ("raw_d", "UInt64"),
    ("charged_u", "UInt64"),
    ("charged_d", "UInt64"),
];

const ACCOUNTED_DAILY_COLUMNS: &[(&str, &str)] = &[
    ("installation_id", "UUID"),
    ("schema_major", "UInt16"),
    ("accounting_date", "Date"),
    ("user_id", "UInt64"),
    ("server_id", "UInt64"),
    ("server_type", "LowCardinality(String)"),
    ("rate_text", "String"),
    ("rate_decimal_10_2", "Decimal(10, 2)"),
    ("outcome", "LowCardinality(String)"),
    ("ingest_batch_id", "UUID"),
    ("batch_aggregate_row_number", "UInt32"),
    ("event_count", "UInt64"),
    ("raw_u", "UInt64"),
    ("raw_d", "UInt64"),
    ("charged_u", "UInt64"),
    ("charged_d", "UInt64"),
];

fn checksum(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"v2board.clickhouse-migration.v1\0");
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
    hex::encode(digest.finalize())
}

fn digest_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn normalize_sql(sql: &str) -> String {
    format!(" {} ", sql.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_versions_are_monotonic_and_checksums_are_stable() {
        for (index, migration) in CLICKHOUSE_MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version, index as u64 + 1);
            assert!(!migration.sql.trim().is_empty());
            assert_eq!(checksum(migration.sql.as_bytes()).len(), 64);
        }
    }

    #[test]
    fn raw_tables_do_not_use_eventual_replacing_merge_correctness() {
        for migration in &CLICKHOUSE_MIGRATIONS[1..=2] {
            assert!(migration.sql.contains("ENGINE = MergeTree"));
            assert!(!migration.sql.contains("ReplacingMergeTree"));
            assert!(migration.sql.contains("PARTITION BY toYYYYMM"));
            assert!(
                migration
                    .sql
                    .contains("non_replicated_deduplication_window")
            );
            assert!(migration.sql.contains("idx_ingest_batch_id"));
        }
        assert!(
            CLICKHOUSE_MIGRATIONS[3]
                .sql
                .contains("installation_binding")
        );
        for migration in &CLICKHOUSE_MIGRATIONS[4..=5] {
            assert!(migration.sql.contains("ENGINE = MergeTree"));
            assert!(!migration.sql.contains("SummingMergeTree"));
            assert!(migration.sql.contains("schema_major UInt16"));
            assert!(migration.sql.contains("idx_ingest_batch_id"));
        }
        assert!(CLICKHOUSE_MIGRATIONS[6].sql.contains("retention_binding"));
        assert_eq!(clickhouse_schema_lineage_sha256().len(), 64);
    }

    #[test]
    fn retention_values_are_fail_closed() {
        assert!(validate_retention_days(90, 730).is_ok());
        assert!(matches!(
            validate_retention_days(0, 730),
            Err(ClickHouseMigrationError::InvalidRetention)
        ));
        assert!(matches!(
            validate_retention_days(731, 730),
            Err(ClickHouseMigrationError::InvalidRetention)
        ));
        assert!(matches!(
            validate_retention_days(90, 36_501),
            Err(ClickHouseMigrationError::InvalidRetention)
        ));
    }

    #[test]
    fn clickhouse_version_gate_accepts_only_the_pinned_lts_line() {
        assert_eq!(parse_version_major_minor("26.3.17.4"), Some((26, 3)));
        assert_eq!(parse_version_major_minor("26.3"), Some((26, 3)));
        assert_ne!(parse_version_major_minor("26.4.1.1"), Some((26, 3)));
        assert_ne!(parse_version_major_minor("25.8.12.1"), Some((26, 3)));
        assert_eq!(parse_version_major_minor("not-a-version"), None);
    }

    #[test]
    fn ledger_lineage_rejects_future_gap_duplicate_and_incomplete_history() {
        let row = |index: usize| LedgerRow {
            version: CLICKHOUSE_MIGRATIONS[index].version,
            name: CLICKHOUSE_MIGRATIONS[index].name.to_owned(),
            checksum: checksum(CLICKHOUSE_MIGRATIONS[index].sql.as_bytes()),
        };
        let exact = (0..CLICKHOUSE_MIGRATIONS.len())
            .map(row)
            .collect::<Vec<_>>();
        assert_eq!(validate_ledger_rows(&exact, true).unwrap(), exact.len());

        let mut future = exact
            .iter()
            .map(|item| LedgerRow {
                version: item.version,
                name: item.name.clone(),
                checksum: item.checksum.clone(),
            })
            .collect::<Vec<_>>();
        future.push(LedgerRow {
            version: 999,
            name: "future".into(),
            checksum: "0".repeat(64),
        });
        assert!(matches!(
            validate_ledger_rows(&future, false),
            Err(ClickHouseMigrationError::UnknownLedgerVersion { version: 999 })
        ));

        let gap = vec![row(0), row(2)];
        assert!(matches!(
            validate_ledger_rows(&gap, false),
            Err(ClickHouseMigrationError::LedgerGap { version: 2 })
        ));

        let duplicate = vec![row(0), row(0)];
        assert!(matches!(
            validate_ledger_rows(&duplicate, false),
            Err(ClickHouseMigrationError::DuplicateLedgerVersion { version: 1 })
        ));

        assert!(matches!(
            validate_ledger_rows(&[row(0)], true),
            Err(ClickHouseMigrationError::IncompleteLineage { .. })
        ));
    }
}
