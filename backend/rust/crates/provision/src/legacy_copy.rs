//! Real datastore adapters for the one-shot legacy converter.
//!
//! The source is held in one read-only `REPEATABLE READ` consistent snapshot.
//! Target writes are batch-inserted and then read back in the same PostgreSQL
//! transaction. A duplicate primary key is accepted only when every canonical
//! value is identical. Checkpoints advance only after that comparison commits.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error as StdError,
    fs, io,
    time::Duration,
};

use redis::aio::MultiplexedConnection;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{MySql, MySqlPool, PgPool, Postgres, QueryBuilder, Row, Transaction, types::Json};
use tokio::time::timeout;
use url::Url;

use crate::legacy_converter::{
    CanonicalRow, CanonicalValue, ColumnRule, ConversionCheckpoint, ConversionPhase,
    ConversionRunBinding, ConverterError, DEFAULT_BATCH_SIZE, DERIVED_MAPPINGS,
    INITIAL_SOURCE_ID_CURSOR, LEGACY_SEMANTIC_SCHEMA_SHA256, LegacyGiftcardRedemptionRow,
    MAX_BATCH_SIZE, NODE_CREDENTIAL_SOURCES, SourceRow, SourceValue, TABLE_MAPPINGS,
    TARGET_GENERATED_COLUMNS, TARGET_POSTGRES_LINEAGE_SHA256, TableMapping, audit_registry,
    canonical_rows_sha256, expand_giftcard_redemptions, mapping_for_source,
    maximum_rows_per_target_insert, node_credential_rows_sql, registry_sha256,
    retry_accepts_existing, sequence_reset_sql, transform_row,
};
use crate::{
    apply_journal::{ApplyCheckpoint, ApplyJournalSnapshot, backup_reference_sha256},
    legacy_backup::load_verified_traffic_receipt_from_backup_archive,
    manifest::{ProvisionFlow, ProvisionSpec},
    native_legacy_source::{
        SourceError, VerifiedFrozenTrafficReceipt, verify_frozen_traffic_receipt,
    },
};

const SOURCE_JSON_ALIAS: &str = "v2board_source_row";
const TARGET_JSON_ALIAS: &str = "v2board_target_row";
const COPY_ROLLING_DOMAIN: &[u8] = b"v2board-legacy-copy-rolling-v1\0";
const FULL_ROW_DOMAIN: &[u8] = b"v2board-legacy-copy-full-row-v1\0";
const SOURCE_FINGERPRINT_DOMAIN: &[u8] = b"v2board-legacy-source-fingerprint-v1\0";
const PRIMARY_KEY_DOMAIN: &[u8] = b"v2board-legacy-copy-primary-key-v1\0";
const CHECKPOINT_RECORD_DOMAIN: &[u8] = b"v2board-legacy-copy-checkpoint-record-v1\0";
const FROZEN_TRAFFIC_DELTA_DOMAIN: &[u8] = b"v2board-frozen-traffic-user-deltas-v1\0";
const TRAFFIC_FOLD_ITEM_DOMAIN: &[u8] = b"v2board-legacy-traffic-fold-item-v1\0";
const TRAFFIC_FOLD_ITEMS_DOMAIN: &[u8] = b"v2board-legacy-traffic-fold-items-v1\0";
const TRAFFIC_FOLD_VERIFICATION_DOMAIN: &[u8] = b"v2board-legacy-traffic-fold-verification-v1\0";
const TRAFFIC_FOLD_SEAL_DOMAIN: &[u8] = b"v2board-legacy-traffic-fold-seal-v1\0";
const MAX_CHECKPOINT_RECORDS: usize = 1_000_000;
const TRAFFIC_REDIS_TIMEOUT: Duration = Duration::from_secs(10);
const TRAFFIC_WRITE_BATCH_SIZE: usize = 1_000;
const ARCHIVE_REDIS_SCAN_PAGE_SIZE: u32 = 4_096;
const MAX_ARCHIVE_REDIS_SCAN_PAGES: u64 = 1_000_000;

type BoxError = Box<dyn StdError + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum LegacyCopyError {
    #[error("source MySQL 8 operation failed")]
    Source(#[source] sqlx::Error),
    #[error("target PostgreSQL operation failed")]
    Target(#[source] sqlx::Error),
    #[error("legacy conversion contract failed")]
    Converter(#[from] ConverterError),
    #[error("durable copy checkpoint failed")]
    Checkpoint(#[source] BoxError),
    #[error("source transaction is not read-only REPEATABLE READ")]
    UnsafeSourceTransaction,
    #[error("source transaction has no selected database")]
    MissingSourceDatabase,
    #[error("source row JSON for {table} is invalid: {message}")]
    InvalidSourceJson { table: String, message: String },
    #[error("source row {table} id {selected_id} disagrees with JSON id")]
    SourceIdMismatch { table: String, selected_id: i64 },
    #[error("target schema is missing {table}.{column}")]
    MissingTargetColumn { table: String, column: String },
    #[error("target column {table}.{column} has unsupported type {udt_name}")]
    UnsupportedTargetType {
        table: String,
        column: String,
        udt_name: String,
    },
    #[error("canonical value for {table}.{column} is incompatible with PostgreSQL {expected}")]
    TargetTypeMismatch {
        table: String,
        column: String,
        expected: &'static str,
    },
    #[error("integer value for {table}.{column} is outside PostgreSQL {expected}")]
    TargetIntegerRange {
        table: String,
        column: String,
        expected: &'static str,
    },
    #[error(
        "text value for {table}.{column} contains NUL, which PostgreSQL text/jsonb cannot store"
    )]
    UnsupportedTextNul { table: String, column: String },
    #[error("target batch for {table} returned a different primary-key set")]
    TargetPrimaryKeyMismatch { table: String },
    #[error("target table {table} row count differs: source={source_count}, target={target_count}")]
    CountMismatch {
        table: String,
        source_count: u64,
        target_count: u64,
    },
    #[error("target table {table} canonical value digest differs")]
    ValueDigestMismatch { table: String },
    #[error("target table {table} primary-key digest differs")]
    PrimaryKeyDigestMismatch { table: String },
    #[error("copy checkpoint does not describe a supported resume position")]
    UnsupportedResumeCheckpoint,
    #[error("source snapshot prefix no longer matches its durable checkpoint")]
    ResumePrefixMismatch,
    #[error("source canonical fingerprint differs from the fenced and restored snapshot")]
    SourceFingerprintMismatch,
    #[error("giftcard id is outside the PostgreSQL integer range")]
    GiftcardIdRange,
    #[error("node credential derivation does not exactly match copied nodes")]
    NodeCredentialMismatch,
    #[error("generated target value {table}.{column} cannot be derived from the source row")]
    GeneratedValue { table: String, column: String },
    #[error(
        "target PostgreSQL must enable synchronous_commit, fsync, full_page_writes, and data_checksums"
    )]
    UnsafeTargetDurability,
    #[error("the frozen Redis traffic receipt is invalid")]
    TrafficReceipt(#[source] SourceError),
    #[error("the frozen Redis traffic source cannot be read safely")]
    TrafficRedis,
    #[error("the frozen Redis traffic differs from its HMAC-bound receipt")]
    TrafficReceiptMismatch,
    #[error("the frozen Redis traffic is not bound to the SourceDrained journal event")]
    TrafficJournalBinding,
    #[error("the frozen Redis traffic contains a negative or out-of-range value")]
    TrafficValueRange,
    #[error("the frozen Redis traffic names a user absent from the fenced MySQL snapshot")]
    TrafficUserMissing,
    #[error("folding frozen Redis traffic would overflow a PostgreSQL BIGINT user counter")]
    TrafficCounterOverflow,
    #[error("the append-only PostgreSQL traffic fold ledger conflicts with this operation")]
    TrafficLedgerConflict,
}

async fn target_durability_is_safe(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<bool, sqlx::Error> {
    sqlx::query("SET LOCAL synchronous_commit = 'on'")
        .execute(&mut **transaction)
        .await?;
    let synchronous_commit = sqlx::query_scalar::<_, String>("SHOW synchronous_commit")
        .fetch_one(&mut **transaction)
        .await?;
    let fsync = sqlx::query_scalar::<_, String>("SHOW fsync")
        .fetch_one(&mut **transaction)
        .await?;
    let full_page_writes = sqlx::query_scalar::<_, String>("SHOW full_page_writes")
        .fetch_one(&mut **transaction)
        .await?;
    let data_checksums = sqlx::query_scalar::<_, String>("SHOW data_checksums")
        .fetch_one(&mut **transaction)
        .await?;
    Ok(synchronous_commit == "on"
        && fsync == "on"
        && full_page_writes == "on"
        && data_checksums == "on")
}

async fn begin_durable_target_tx(
    pool: &PgPool,
) -> Result<Transaction<'_, Postgres>, LegacyCopyError> {
    let mut transaction = pool.begin().await.map_err(LegacyCopyError::Target)?;
    if !target_durability_is_safe(&mut transaction)
        .await
        .map_err(LegacyCopyError::Target)?
    {
        return Err(LegacyCopyError::UnsafeTargetDurability);
    }
    Ok(transaction)
}

#[allow(async_fn_in_trait)]
pub trait DurableCopyCheckpointSink {
    type Error: StdError + Send + Sync + 'static;

    /// Returns the fsync-durable CAS head for this exact conversion binding.
    async fn load(
        &mut self,
        binding: &ConversionRunBinding,
    ) -> Result<Option<ConversionCheckpoint>, Self::Error>;

    /// Atomically persists `next` only if `previous` is still the durable head.
    /// Implementations must durably commit the record before returning success
    /// and must never persist datastore URLs or row values.
    async fn compare_and_store(
        &mut self,
        previous: Option<&ConversionCheckpoint>,
        next: &ConversionCheckpoint,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum PostgresCheckpointError {
    #[error("PostgreSQL checkpoint query failed")]
    Database(#[from] sqlx::Error),
    #[error("lifecycle operation is missing or not at the converter mutation gate")]
    LifecycleGate,
    #[error(
        "PostgreSQL synchronous_commit, fsync, full_page_writes, or data_checksums is not durable"
    )]
    SynchronousCommit,
    #[error("checkpoint binding differs from the lifecycle operation")]
    Binding,
    #[error("checkpoint hash chain is malformed")]
    HashChain,
    #[error("checkpoint CAS head changed")]
    CompareAndStore,
    #[error("checkpoint value cannot be represented by the PostgreSQL schema")]
    ValueRange,
    #[error("checkpoint contract is invalid")]
    Converter(#[from] ConverterError),
}

#[derive(sqlx::FromRow)]
struct CheckpointRecordRow {
    sequence: i64,
    target_installation_id: String,
    source_snapshot_sha256: String,
    source_schema_sha256: String,
    registry_sha256: String,
    phase: String,
    table_order: i32,
    table_name: String,
    last_source_id: i64,
    source_rows_seen: String,
    target_rows_verified: String,
    rolling_sha256: String,
    previous_checkpoint_sha256: Option<String>,
    checkpoint_sha256: String,
    recorded_at: i64,
}

pub struct PostgresDurableCopyCheckpointSink<'a> {
    pool: &'a PgPool,
    backup_reference_sha256: String,
}

impl<'a> PostgresDurableCopyCheckpointSink<'a> {
    pub fn new(
        pool: &'a PgPool,
        backup_reference_sha256: impl Into<String>,
    ) -> Result<Self, PostgresCheckpointError> {
        let backup_reference_sha256 = backup_reference_sha256.into();
        if !is_lower_sha256(&backup_reference_sha256) {
            return Err(PostgresCheckpointError::Binding);
        }
        Ok(Self {
            pool,
            backup_reference_sha256,
        })
    }

    async fn lock_lifecycle_gate(
        transaction: &mut Transaction<'_, Postgres>,
        binding: &ConversionRunBinding,
        expected_backup_reference_sha256: &str,
    ) -> Result<(), PostgresCheckpointError> {
        let operation_id = uuid::Uuid::parse_str(&binding.operation_id)
            .map_err(|_| PostgresCheckpointError::Binding)?;
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                i16,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            "SELECT operation.installation_id::text, operation.kind, \
                    operation.converter_registry_sha256, \
                    operation.source_fingerprint_sha256, operation.state, \
                    operation.checkpoint, operation.target_lineage_sha256, \
                    operation.backup_reference, event.backup_reference_sha256, \
                    event.source_fingerprint_sha256 \
             FROM v2_lifecycle_operation operation \
             JOIN v2_lifecycle_event event \
               ON event.operation_id = operation.operation_id \
              AND event.generation = operation.journal_generation \
              AND event.event_sha256 = operation.journal_event_sha256 \
             WHERE operation.operation_id = $1 FOR UPDATE OF operation",
        )
        .bind(operation_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or(PostgresCheckpointError::LifecycleGate)?;
        let (
            installation_id,
            kind,
            converter_registry_sha256,
            source_fingerprint_sha256,
            state,
            checkpoint,
            target_lineage_sha256,
            backup_reference,
            event_backup_reference_sha256,
            event_source_fingerprint_sha256,
        ) = row;
        let actual_backup_reference_sha256 = backup_reference
            .as_deref()
            .map(backup_reference_sha256)
            .transpose()
            .map_err(|_| PostgresCheckpointError::Binding)?;
        if installation_id != binding.target_installation_id
            || kind != "legacy_reference_migration"
            || converter_registry_sha256 != binding.registry_sha256
            || source_fingerprint_sha256 != binding.source_snapshot_sha256
            || target_lineage_sha256 != TARGET_POSTGRES_LINEAGE_SHA256
            || actual_backup_reference_sha256.as_deref() != Some(expected_backup_reference_sha256)
            || event_backup_reference_sha256.as_deref() != Some(expected_backup_reference_sha256)
            || event_source_fingerprint_sha256.as_deref()
                != Some(binding.source_snapshot_sha256.as_str())
            || state != "running"
            || checkpoint != 6
        {
            return Err(PostgresCheckpointError::LifecycleGate);
        }
        Ok(())
    }

    async fn load_chain(
        transaction: &mut Transaction<'_, Postgres>,
        binding: &ConversionRunBinding,
    ) -> Result<Vec<(ConversionCheckpoint, String)>, PostgresCheckpointError> {
        let operation_id = uuid::Uuid::parse_str(&binding.operation_id)
            .map_err(|_| PostgresCheckpointError::Binding)?;
        let rows = sqlx::query_as::<_, CheckpointRecordRow>(
            "SELECT sequence, target_installation_id::text AS target_installation_id, \
                    source_snapshot_sha256, source_schema_sha256, registry_sha256, \
                    phase, table_order, table_name, last_source_id, \
                    source_rows_seen::text AS source_rows_seen, \
                    target_rows_verified::text AS target_rows_verified, rolling_sha256, \
                    previous_checkpoint_sha256, checkpoint_sha256, recorded_at \
             FROM v2_legacy_copy_checkpoint WHERE operation_id = $1 ORDER BY sequence",
        )
        .bind(operation_id)
        .fetch_all(&mut **transaction)
        .await?;
        if rows.len() > MAX_CHECKPOINT_RECORDS {
            return Err(PostgresCheckpointError::HashChain);
        }
        decode_checkpoint_chain(binding, rows)
    }

    async fn load_latest(
        transaction: &mut Transaction<'_, Postgres>,
        binding: &ConversionRunBinding,
    ) -> Result<Option<(ConversionCheckpoint, String, i64)>, PostgresCheckpointError> {
        let operation_id = uuid::Uuid::parse_str(&binding.operation_id)
            .map_err(|_| PostgresCheckpointError::Binding)?;
        let row = sqlx::query_as::<_, CheckpointRecordRow>(
            "SELECT sequence, target_installation_id::text AS target_installation_id, \
                    source_snapshot_sha256, source_schema_sha256, registry_sha256, \
                    phase, table_order, table_name, last_source_id, \
                    source_rows_seen::text AS source_rows_seen, \
                    target_rows_verified::text AS target_rows_verified, rolling_sha256, \
                    previous_checkpoint_sha256, checkpoint_sha256, recorded_at \
             FROM v2_legacy_copy_checkpoint WHERE operation_id = $1 \
             ORDER BY sequence DESC LIMIT 1",
        )
        .bind(operation_id)
        .fetch_optional(&mut **transaction)
        .await?;
        row.map(|row| decode_latest_checkpoint(binding, row))
            .transpose()
    }
}

impl DurableCopyCheckpointSink for PostgresDurableCopyCheckpointSink<'_> {
    type Error = PostgresCheckpointError;

    async fn load(
        &mut self,
        binding: &ConversionRunBinding,
    ) -> Result<Option<ConversionCheckpoint>, Self::Error> {
        binding.validate()?;
        let mut transaction = self.pool.begin().await?;
        Self::lock_lifecycle_gate(&mut transaction, binding, &self.backup_reference_sha256).await?;
        let chain = Self::load_chain(&mut transaction, binding).await?;
        transaction.rollback().await?;
        Ok(chain.last().map(|(checkpoint, _)| checkpoint.clone()))
    }

    async fn compare_and_store(
        &mut self,
        previous: Option<&ConversionCheckpoint>,
        next: &ConversionCheckpoint,
    ) -> Result<(), Self::Error> {
        next.binding.validate()?;
        let mut transaction = self.pool.begin().await?;
        if !target_durability_is_safe(&mut transaction).await? {
            return Err(PostgresCheckpointError::SynchronousCommit);
        }
        Self::lock_lifecycle_gate(
            &mut transaction,
            &next.binding,
            &self.backup_reference_sha256,
        )
        .await?;
        let head = Self::load_latest(&mut transaction, &next.binding).await?;
        if head
            .as_ref()
            .is_some_and(|(checkpoint, _, _)| checkpoint == next)
        {
            transaction.rollback().await?;
            return Ok(());
        }
        if head.as_ref().map(|(checkpoint, _, _)| checkpoint) != previous {
            return Err(PostgresCheckpointError::CompareAndStore);
        }
        next.validate_resume(&next.binding, previous)?;
        let sequence = head.as_ref().map_or(Ok(0_i64), |(_, _, sequence)| {
            sequence
                .checked_add(1)
                .ok_or(PostgresCheckpointError::ValueRange)
        })?;
        let previous_checkpoint_sha256 = head.as_ref().map(|(_, sha256, _)| sha256.as_str());
        let recorded_at = sqlx::query_scalar::<_, i64>(
            "SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if recorded_at <= 0 {
            return Err(PostgresCheckpointError::ValueRange);
        }
        let checkpoint_sha256 =
            checkpoint_record_sha256(sequence, next, previous_checkpoint_sha256, recorded_at)?;
        let operation_id = uuid::Uuid::parse_str(&next.binding.operation_id)
            .map_err(|_| PostgresCheckpointError::Binding)?;
        let installation_id = uuid::Uuid::parse_str(&next.binding.target_installation_id)
            .map_err(|_| PostgresCheckpointError::Binding)?;
        sqlx::query(
            "INSERT INTO v2_legacy_copy_checkpoint (\
                 operation_id, sequence, target_installation_id, source_snapshot_sha256, \
                 source_schema_sha256, registry_sha256, phase, table_order, table_name, \
                 last_source_id, source_rows_seen, target_rows_verified, rolling_sha256, \
                 previous_checkpoint_sha256, checkpoint_sha256, recorded_at\
             ) VALUES (\
                 $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, \
                 $11::numeric, $12::numeric, $13, $14, $15, $16\
             )",
        )
        .bind(operation_id)
        .bind(sequence)
        .bind(installation_id)
        .bind(&next.binding.source_snapshot_sha256)
        .bind(&next.binding.source_schema_sha256)
        .bind(&next.binding.registry_sha256)
        .bind(phase_name(next.phase))
        .bind(i32::from(next.table_order))
        .bind(&next.table)
        .bind(next.last_source_id)
        .bind(next.source_rows_seen.to_string())
        .bind(next.target_rows_verified.to_string())
        .bind(&next.rolling_sha256)
        .bind(previous_checkpoint_sha256)
        .bind(&checkpoint_sha256)
        .bind(recorded_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceSnapshotIdentity {
    pub database_name: String,
    pub server_version: String,
    pub connection_id: u64,
    pub transaction_isolation: String,
    pub transaction_read_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacySourceFingerprint {
    pub semantic_schema_sha256: String,
    pub converter_registry_sha256: String,
    pub table_count: usize,
    pub row_count: u64,
    pub canonical_sha256: String,
}

/// Immutable linkage between the HMAC-bound Redis receipt and the exact
/// filesystem journal event that declared the legacy source drained. The
/// formal MySQL fingerprint deliberately remains the canonical 27-table
/// fingerprint; this is an additional source fact, never folded into it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceDrainJournalBinding {
    pub generation: u64,
    pub event_sha256: String,
    pub report_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenTrafficDelta {
    user_id: i64,
    upload: i64,
    download: i64,
}

/// A verified, sorted union of both frozen Redis hashes. Construction is only
/// possible by verifying either the initial owner-only receipt or the sole age
/// archive, including its scoped HMAC and every count, sum, and digest. Before
/// archive creation Redis is mandatory. Afterwards an unreachable old Redis is
/// recoverable; if reachable, the selected DB may contain only the two exact
/// frozen hashes (whose values must match), every other logical DB and reachable
/// cache server must be empty, and live traffic keys remain forbidden.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFrozenTrafficBatch {
    receipt: VerifiedFrozenTrafficReceipt,
    source_drained: SourceDrainJournalBinding,
    deltas: Vec<FrozenTrafficDelta>,
}

impl VerifiedFrozenTrafficBatch {
    pub fn receipt(&self) -> &VerifiedFrozenTrafficReceipt {
        &self.receipt
    }

    pub fn source_drained(&self) -> &SourceDrainJournalBinding {
        &self.source_drained
    }

    pub fn user_count(&self) -> usize {
        self.deltas.len()
    }
}

/// Loads the independent frozen-traffic source fact. No PostgreSQL connection
/// is opened here, so archive/HMAC/keyspace/value failures necessarily happen
/// before the converter can mutate any target row.
pub async fn load_verified_frozen_traffic(
    spec: &ProvisionSpec,
    source_drained: &ApplyJournalSnapshot,
) -> Result<VerifiedFrozenTrafficBatch, LegacyCopyError> {
    let receipt_path = &spec
        .legacy_apply_execution()
        .ok_or(LegacyCopyError::TrafficJournalBinding)?
        .receipts
        .source_drain_path;
    let (receipt, archive_owned) = match fs::symlink_metadata(receipt_path) {
        Ok(_) => (
            verify_frozen_traffic_receipt(spec).map_err(LegacyCopyError::TrafficReceipt)?,
            false,
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (
            load_verified_traffic_receipt_from_backup_archive(spec)
                .await
                .map_err(|_| LegacyCopyError::TrafficReceiptMismatch)?,
            true,
        ),
        Err(_) => return Err(LegacyCopyError::TrafficReceiptMismatch),
    };
    let report_sha256 = source_drained
        .checkpoint_proof_sha256()
        .filter(|value| is_lower_sha256(value))
        .ok_or(LegacyCopyError::TrafficJournalBinding)?;
    if source_drained.checkpoint() != ApplyCheckpoint::SourceDrained
        || source_drained.binding().operation_id() != spec.operation_id
        || receipt.operation_id != spec.operation_id
        || report_sha256 != receipt.receipt_sha256
        || receipt.maintenance_fenced_generation.checked_add(1) != Some(source_drained.generation())
        || source_drained.previous_event_sha256()
            != Some(receipt.maintenance_fenced_event_sha256.as_str())
        || !is_lower_sha256(source_drained.event_sha256())
    {
        return Err(LegacyCopyError::TrafficJournalBinding);
    }
    let redis_url = match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, .. } => &source.redis_default_url,
        _ => return Err(LegacyCopyError::TrafficJournalBinding),
    };
    let client =
        redis::Client::open(redis_url.as_str()).map_err(|_| LegacyCopyError::TrafficRedis)?;
    let connection = timeout(
        TRAFFIC_REDIS_TIMEOUT,
        client.get_multiplexed_async_connection(),
    )
    .await;
    let mut connection = match connection {
        Ok(Ok(connection)) => Some(connection),
        Ok(Err(_)) | Err(_) if archive_owned => None,
        Ok(Err(_)) | Err(_) => return Err(LegacyCopyError::TrafficRedis),
    };
    let mut default_run_id = None;
    let deltas = if let Some(connection) = connection.as_mut() {
        let run_id_before = frozen_traffic_redis_run_id(connection).await?;
        require_live_traffic_keys_absent(spec, connection).await?;
        let (upload_exists, upload) =
            read_frozen_traffic_hash(connection, &receipt.frozen_upload_key).await?;
        let (download_exists, download) =
            read_frozen_traffic_hash(connection, &receipt.frozen_download_key).await?;
        if frozen_traffic_redis_run_id(connection).await? != run_id_before {
            return Err(LegacyCopyError::TrafficReceiptMismatch);
        }
        require_live_traffic_keys_absent(spec, connection).await?;
        let deltas = reconcile_durable_traffic_receipt(
            &receipt,
            upload_exists,
            &upload,
            download_exists,
            &download,
        )?;
        require_traffic_redis_keyspace_sealed(spec, &receipt, connection).await?;
        if frozen_traffic_redis_run_id(connection).await? != run_id_before {
            return Err(LegacyCopyError::TrafficReceiptMismatch);
        }
        default_run_id = Some(run_id_before);
        deltas
    } else {
        reconcile_durable_traffic_receipt(
            &receipt,
            false,
            &BTreeMap::new(),
            false,
            &BTreeMap::new(),
        )?
    };
    require_traffic_cache_keyspace_sealed(spec, default_run_id.as_deref(), archive_owned).await?;
    Ok(VerifiedFrozenTrafficBatch {
        receipt,
        source_drained: SourceDrainJournalBinding {
            generation: source_drained.generation(),
            event_sha256: source_drained.event_sha256().to_string(),
            report_sha256: report_sha256.to_string(),
        },
        deltas,
    })
}

fn reconcile_durable_traffic_receipt(
    receipt: &VerifiedFrozenTrafficReceipt,
    upload_exists: bool,
    upload: &BTreeMap<i64, i128>,
    download_exists: bool,
    download: &BTreeMap<i64, i128>,
) -> Result<Vec<FrozenTrafficDelta>, LegacyCopyError> {
    let mut durable_upload = BTreeMap::new();
    let mut durable_download = BTreeMap::new();
    for delta in &receipt.deltas {
        if let Some(upload) = delta.1 {
            durable_upload.insert(delta.0, i128::from(upload));
        }
        if let Some(download) = delta.2 {
            durable_download.insert(delta.0, i128::from(download));
        }
    }
    if (upload_exists && upload != &durable_upload)
        || (download_exists && download != &durable_download)
    {
        return Err(LegacyCopyError::TrafficReceiptMismatch);
    }
    let (deltas, digest, upload_sum, download_sum) =
        canonical_frozen_traffic_union(&durable_upload, &durable_download)?;
    let receipt_upload_sum = receipt
        .upload_delta_sum
        .parse::<i128>()
        .map_err(|_| LegacyCopyError::TrafficReceiptMismatch)?;
    let receipt_download_sum = receipt
        .download_delta_sum
        .parse::<i128>()
        .map_err(|_| LegacyCopyError::TrafficReceiptMismatch)?;
    if receipt.upload_fields != durable_upload.len() as u64
        || receipt.download_fields != durable_download.len() as u64
        || receipt.sorted_user_delta_count != deltas.len() as u64
        || receipt.sorted_user_delta_sha256 != digest
        || receipt_upload_sum != upload_sum
        || receipt_download_sum != download_sum
        || receipt.delta_applied_exactly_once != deltas.is_empty()
    {
        return Err(LegacyCopyError::TrafficReceiptMismatch);
    }
    Ok(deltas)
}

async fn require_live_traffic_keys_absent(
    spec: &ProvisionSpec,
    connection: &mut MultiplexedConnection,
) -> Result<(), LegacyCopyError> {
    let source = match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, .. } => source,
        _ => return Err(LegacyCopyError::TrafficJournalBinding),
    };
    for suffix in ["v2board_upload_traffic", "v2board_download_traffic"] {
        let key = format!("{}{suffix}", source.redis_connection_prefix);
        let kind = timeout(
            TRAFFIC_REDIS_TIMEOUT,
            redis::cmd("TYPE")
                .arg(&key)
                .query_async::<String>(&mut *connection),
        )
        .await
        .map_err(|_| LegacyCopyError::TrafficRedis)?
        .map_err(|_| LegacyCopyError::TrafficRedis)?;
        if kind != "none" {
            return Err(LegacyCopyError::TrafficReceiptMismatch);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RedisUrlIdentity {
    endpoint: (String, String, u16),
    database: u32,
}

fn redis_url_identity(value: &str) -> Result<RedisUrlIdentity, LegacyCopyError> {
    let url = Url::parse(value).map_err(|_| LegacyCopyError::TrafficRedis)?;
    let scheme = url.scheme();
    if !matches!(scheme, "redis" | "rediss") || url.fragment().is_some() {
        return Err(LegacyCopyError::TrafficRedis);
    }
    let host = url
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or(LegacyCopyError::TrafficRedis)?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    let port = url
        .port_or_known_default()
        .ok_or(LegacyCopyError::TrafficRedis)?;
    let path = url.path().trim_start_matches('/');
    if path.contains('/') {
        return Err(LegacyCopyError::TrafficRedis);
    }
    let database = if path.is_empty() {
        0
    } else {
        path.parse::<u32>()
            .map_err(|_| LegacyCopyError::TrafficRedis)?
    };
    Ok(RedisUrlIdentity {
        endpoint: (scheme.to_string(), host, port),
        database,
    })
}

async fn redis_dbsize(connection: &mut MultiplexedConnection) -> Result<u64, LegacyCopyError> {
    timeout(
        TRAFFIC_REDIS_TIMEOUT,
        redis::cmd("DBSIZE").query_async::<u64>(connection),
    )
    .await
    .map_err(|_| LegacyCopyError::TrafficRedis)?
    .map_err(|_| LegacyCopyError::TrafficRedis)
}

async fn redis_keyspace_counts(
    connection: &mut MultiplexedConnection,
) -> Result<BTreeMap<u32, u64>, LegacyCopyError> {
    let info = timeout(
        TRAFFIC_REDIS_TIMEOUT,
        redis::cmd("INFO")
            .arg("keyspace")
            .query_async::<String>(connection),
    )
    .await
    .map_err(|_| LegacyCopyError::TrafficRedis)?
    .map_err(|_| LegacyCopyError::TrafficRedis)?;
    parse_redis_keyspace_counts(&info)
}

fn parse_redis_keyspace_counts(info: &str) -> Result<BTreeMap<u32, u64>, LegacyCopyError> {
    let mut databases = BTreeMap::new();
    for line in info.lines().map(str::trim) {
        let Some((database, values)) = line.split_once(':') else {
            continue;
        };
        let Some(database) = database.strip_prefix("db") else {
            continue;
        };
        let database = database
            .parse::<u32>()
            .map_err(|_| LegacyCopyError::TrafficRedis)?;
        let keys = values
            .split(',')
            .find_map(|field| field.strip_prefix("keys="))
            .ok_or(LegacyCopyError::TrafficRedis)?
            .parse::<u64>()
            .map_err(|_| LegacyCopyError::TrafficRedis)?;
        if databases.insert(database, keys).is_some() {
            return Err(LegacyCopyError::TrafficRedis);
        }
    }
    Ok(databases)
}

fn only_selected_redis_database_has_keys(
    keyspaces: &BTreeMap<u32, u64>,
    selected_database: u32,
    selected_size: u64,
) -> bool {
    keyspaces.get(&selected_database).copied().unwrap_or(0) == selected_size
        && keyspaces
            .iter()
            .all(|(database, keys)| *database == selected_database || *keys == 0)
}

async fn require_only_exact_frozen_keys(
    connection: &mut MultiplexedConnection,
    receipt: &VerifiedFrozenTrafficReceipt,
    database_size: u64,
) -> Result<(), LegacyCopyError> {
    if database_size > 2 {
        return Err(LegacyCopyError::TrafficReceiptMismatch);
    }
    let mut cursor = 0_u64;
    let mut pages = 0_u64;
    let mut upload_seen = false;
    let mut download_seen = false;
    loop {
        pages = pages.saturating_add(1);
        if pages > MAX_ARCHIVE_REDIS_SCAN_PAGES {
            return Err(LegacyCopyError::TrafficRedis);
        }
        let (next, keys) = timeout(
            TRAFFIC_REDIS_TIMEOUT,
            redis::cmd("SCAN")
                .arg(cursor)
                .arg("COUNT")
                .arg(ARCHIVE_REDIS_SCAN_PAGE_SIZE)
                .query_async::<(u64, Vec<Vec<u8>>)>(connection),
        )
        .await
        .map_err(|_| LegacyCopyError::TrafficRedis)?
        .map_err(|_| LegacyCopyError::TrafficRedis)?;
        for key in keys {
            if key == receipt.frozen_upload_key.as_bytes() {
                upload_seen = true;
            } else if key == receipt.frozen_download_key.as_bytes() {
                download_seen = true;
            } else {
                return Err(LegacyCopyError::TrafficReceiptMismatch);
            }
        }
        if next == 0 {
            break;
        }
        if next == cursor {
            return Err(LegacyCopyError::TrafficRedis);
        }
        cursor = next;
    }
    if u64::from(upload_seen) + u64::from(download_seen) != database_size {
        return Err(LegacyCopyError::TrafficReceiptMismatch);
    }
    Ok(())
}

async fn require_traffic_redis_keyspace_sealed(
    spec: &ProvisionSpec,
    receipt: &VerifiedFrozenTrafficReceipt,
    connection: &mut MultiplexedConnection,
) -> Result<(), LegacyCopyError> {
    let source = match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, .. } => source,
        _ => return Err(LegacyCopyError::TrafficJournalBinding),
    };
    let default = redis_url_identity(&source.redis_default_url)?;
    let database_size = redis_dbsize(connection).await?;
    require_only_exact_frozen_keys(connection, receipt, database_size).await?;
    let keyspaces = redis_keyspace_counts(connection).await?;
    if !only_selected_redis_database_has_keys(&keyspaces, default.database, database_size) {
        return Err(LegacyCopyError::TrafficReceiptMismatch);
    }
    Ok(())
}

async fn require_traffic_cache_keyspace_sealed(
    spec: &ProvisionSpec,
    default_run_id: Option<&str>,
    allow_unreachable: bool,
) -> Result<(), LegacyCopyError> {
    let source = match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { source, .. } => source,
        _ => return Err(LegacyCopyError::TrafficJournalBinding),
    };
    let default = redis_url_identity(&source.redis_default_url)?;
    let cache = redis_url_identity(&source.redis_cache_url)?;
    let client = redis::Client::open(source.redis_cache_url.as_str())
        .map_err(|_| LegacyCopyError::TrafficRedis)?;
    let mut connection = match timeout(
        TRAFFIC_REDIS_TIMEOUT,
        client.get_multiplexed_async_connection(),
    )
    .await
    {
        Ok(Ok(connection)) => connection,
        Ok(Err(_)) | Err(_)
            if allow_unreachable
                && (default_run_id.is_none() || default.endpoint != cache.endpoint) =>
        {
            return Ok(());
        }
        Ok(Err(_)) | Err(_) => return Err(LegacyCopyError::TrafficRedis),
    };
    let cache_run_id = frozen_traffic_redis_run_id(&mut connection).await?;
    if default_run_id.is_some_and(|run_id| run_id == cache_run_id) {
        // The default connection already proved every other logical database,
        // including this selected cache DB, empty via INFO keyspace. Comparing
        // run IDs also handles equivalent hostname aliases safely.
        return Ok(());
    }
    if default.endpoint == cache.endpoint {
        // One endpoint cannot identify two different Redis server epochs in a
        // single short seal.
        return Err(LegacyCopyError::TrafficRedis);
    }
    if redis_dbsize(&mut connection).await? != 0
        || redis_keyspace_counts(&mut connection)
            .await?
            .values()
            .any(|keys| *keys != 0)
        || frozen_traffic_redis_run_id(&mut connection).await? != cache_run_id
    {
        return Err(LegacyCopyError::TrafficReceiptMismatch);
    }
    Ok(())
}

async fn frozen_traffic_redis_run_id(
    connection: &mut MultiplexedConnection,
) -> Result<String, LegacyCopyError> {
    let info = timeout(
        TRAFFIC_REDIS_TIMEOUT,
        redis::cmd("INFO")
            .arg("server")
            .query_async::<String>(connection),
    )
    .await
    .map_err(|_| LegacyCopyError::TrafficRedis)?
    .map_err(|_| LegacyCopyError::TrafficRedis)?;
    let run_id = info
        .lines()
        .find_map(|line| line.strip_prefix("run_id:"))
        .map(str::trim)
        .unwrap_or("");
    if run_id.len() != 40 || !run_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(LegacyCopyError::TrafficRedis);
    }
    Ok(run_id.to_ascii_lowercase())
}

async fn read_frozen_traffic_hash(
    connection: &mut MultiplexedConnection,
    key: &str,
) -> Result<(bool, BTreeMap<i64, i128>), LegacyCopyError> {
    let kind = timeout(
        TRAFFIC_REDIS_TIMEOUT,
        redis::cmd("TYPE")
            .arg(key)
            .query_async::<String>(connection),
    )
    .await
    .map_err(|_| LegacyCopyError::TrafficRedis)?
    .map_err(|_| LegacyCopyError::TrafficRedis)?;
    if !frozen_traffic_hash_requires_scan(&kind)? {
        return Ok((false, BTreeMap::new()));
    }
    let mut values = BTreeMap::new();
    let mut cursor = 0_u64;
    loop {
        let (next, batch) = timeout(
            TRAFFIC_REDIS_TIMEOUT,
            redis::cmd("HSCAN")
                .arg(key)
                .arg(cursor)
                .arg("COUNT")
                .arg(1_000_u32)
                .query_async::<(u64, Vec<(Vec<u8>, Vec<u8>)>)>(connection),
        )
        .await
        .map_err(|_| LegacyCopyError::TrafficRedis)?
        .map_err(|_| LegacyCopyError::TrafficRedis)?;
        for (field, value) in batch {
            let user_id = std::str::from_utf8(&field)
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|value| *value > 0)
                .ok_or(LegacyCopyError::TrafficValueRange)?;
            let delta = std::str::from_utf8(&value)
                .ok()
                .and_then(|value| value.parse::<i128>().ok())
                .filter(|value| *value >= 0)
                .ok_or(LegacyCopyError::TrafficValueRange)?;
            if values.insert(user_id, delta).is_some() {
                return Err(LegacyCopyError::TrafficReceiptMismatch);
            }
        }
        if next == 0 {
            break;
        }
        cursor = next;
    }
    Ok((true, values))
}

fn frozen_traffic_hash_requires_scan(kind: &str) -> Result<bool, LegacyCopyError> {
    match kind {
        "none" => Ok(false),
        "hash" => Ok(true),
        _ => Err(LegacyCopyError::TrafficReceiptMismatch),
    }
}

fn canonical_frozen_traffic_union(
    upload: &BTreeMap<i64, i128>,
    download: &BTreeMap<i64, i128>,
) -> Result<(Vec<FrozenTrafficDelta>, String, i128, i128), LegacyCopyError> {
    let mut users = BTreeSet::new();
    users.extend(upload.keys().copied());
    users.extend(download.keys().copied());
    let mut digest = Sha256::new();
    digest.update(FROZEN_TRAFFIC_DELTA_DOMAIN);
    let mut upload_sum = 0_i128;
    let mut download_sum = 0_i128;
    let mut deltas = Vec::with_capacity(users.len());
    for user_id in users {
        let upload = upload.get(&user_id).copied().unwrap_or(0);
        let download = download.get(&user_id).copied().unwrap_or(0);
        if upload < 0 || download < 0 {
            return Err(LegacyCopyError::TrafficValueRange);
        }
        let upload_i64 = i64::try_from(upload).map_err(|_| LegacyCopyError::TrafficValueRange)?;
        let download_i64 =
            i64::try_from(download).map_err(|_| LegacyCopyError::TrafficValueRange)?;
        upload_sum = upload_sum
            .checked_add(upload)
            .ok_or(LegacyCopyError::TrafficValueRange)?;
        download_sum = download_sum
            .checked_add(download)
            .ok_or(LegacyCopyError::TrafficValueRange)?;
        digest_field(&mut digest, user_id.to_string().as_bytes());
        digest_field(&mut digest, upload.to_string().as_bytes());
        digest_field(&mut digest, download.to_string().as_bytes());
        deltas.push(FrozenTrafficDelta {
            user_id,
            upload: upload_i64,
            download: download_i64,
        });
    }
    Ok((
        deltas,
        hex::encode(digest.finalize()),
        upload_sum,
        download_sum,
    ))
}

/// Owns the sole source connection used from first copy read through final
/// verification. Dropping an uncommitted SQLx transaction starts a rollback.
pub struct LegacySourceSnapshot {
    transaction: Transaction<'static, MySql>,
    identity: SourceSnapshotIdentity,
}

impl LegacySourceSnapshot {
    pub async fn begin(
        pool: &MySqlPool,
        binding: &ConversionRunBinding,
    ) -> Result<Self, LegacyCopyError> {
        binding.validate()?;
        Self::begin_read_only(pool).await
    }

    async fn begin_read_only(pool: &MySqlPool) -> Result<Self, LegacyCopyError> {
        audit_registry()?;
        let mut transaction = pool
            .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
            .await
            .map_err(LegacyCopyError::Source)?;

        let transaction_isolation =
            sqlx::query_scalar::<_, String>("SELECT @@SESSION.transaction_isolation")
                .fetch_one(&mut *transaction)
                .await
                .map_err(LegacyCopyError::Source)?;
        if !transaction_isolation.eq_ignore_ascii_case("REPEATABLE-READ") {
            return Err(LegacyCopyError::UnsafeSourceTransaction);
        }

        let (database_name, server_version, connection_id): (Option<String>, String, u64) =
            sqlx::query_as("SELECT DATABASE(), @@version, CONNECTION_ID()")
                .fetch_one(&mut *transaction)
                .await
                .map_err(LegacyCopyError::Source)?;
        let database_name = database_name.ok_or(LegacyCopyError::MissingSourceDatabase)?;

        // The first consistent read establishes the snapshot immediately; no
        // later read may accidentally become the snapshot anchor.
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_server_group")
            .fetch_one(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Source)?;

        Ok(Self {
            transaction,
            identity: SourceSnapshotIdentity {
                database_name,
                server_version,
                connection_id,
                transaction_isolation,
                // This is evidence about the transaction command accepted by
                // the server, not @@SESSION.transaction_read_only. MySQL 8
                // leaves that session variable at the default for the *next*
                // transaction even while an explicit READ ONLY transaction is
                // active. Requiring INNODB_TRX instead would unnecessarily
                // grant the source inspector global PROCESS.
                transaction_read_only: true,
            },
        })
    }

    pub fn identity(&self) -> &SourceSnapshotIdentity {
        &self.identity
    }

    pub async fn commit(self) -> Result<(), LegacyCopyError> {
        self.transaction
            .commit()
            .await
            .map_err(LegacyCopyError::Source)
    }

    pub async fn rollback(self) -> Result<(), LegacyCopyError> {
        self.transaction
            .rollback()
            .await
            .map_err(LegacyCopyError::Source)
    }

    pub async fn read_batch(
        &mut self,
        mapping: &TableMapping,
        after_id: i64,
        batch_size: u32,
    ) -> Result<Vec<SourceRow>, LegacyCopyError> {
        if batch_size == 0 || batch_size > MAX_BATCH_SIZE {
            return Err(ConverterError::InvalidBatchSize(batch_size).into());
        }
        let sql = source_json_batch_sql(mapping)?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(after_id)
            .bind(i64::from(batch_size))
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(LegacyCopyError::Source)?;
        rows.into_iter()
            .map(|row| {
                let selected_id = row
                    .try_get::<i64, _>("v2board_selected_id")
                    .map_err(LegacyCopyError::Source)?;
                let payload = row
                    .try_get::<String, _>(SOURCE_JSON_ALIAS)
                    .map_err(LegacyCopyError::Source)?;
                decode_source_json_row(mapping, selected_id, &payload)
            })
            .collect()
    }

    pub async fn table_count(&mut self, mapping: &TableMapping) -> Result<u64, LegacyCopyError> {
        let sql = format!("SELECT COUNT(*) FROM {}", mysql_identifier(mapping.source)?);
        let count = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql))
            .fetch_one(&mut *self.transaction)
            .await
            .map_err(LegacyCopyError::Source)?;
        u64::try_from(count).map_err(|_| LegacyCopyError::InvalidSourceJson {
            table: mapping.source.to_string(),
            message: "negative COUNT(*)".to_string(),
        })
    }

    async fn canonical_fingerprint(
        &mut self,
        batch_size: u32,
    ) -> Result<LegacySourceFingerprint, LegacyCopyError> {
        if batch_size == 0 || batch_size > MAX_BATCH_SIZE {
            return Err(ConverterError::InvalidBatchSize(batch_size).into());
        }
        audit_registry()?;
        let registry = registry_sha256()?;
        let mut digest = Sha256::new();
        digest.update(SOURCE_FINGERPRINT_DOMAIN);
        digest_field(&mut digest, LEGACY_SEMANTIC_SCHEMA_SHA256.as_bytes());
        digest_field(&mut digest, registry.as_bytes());
        let mut row_count = 0_u64;
        for mapping in TABLE_MAPPINGS {
            digest_field(&mut digest, mapping.source.as_bytes());
            digest_field(&mut digest, mapping.target.as_bytes());
            let mut table_rows = 0_u64;
            let mut after = INITIAL_SOURCE_ID_CURSOR;
            loop {
                let source_rows = self.read_batch(mapping, after, batch_size).await?;
                if source_rows.is_empty() {
                    break;
                }
                for source in &source_rows {
                    let expected = expected_full_row(mapping, source)?;
                    validate_canonical_row_for_postgres(mapping.target, &expected)?;
                    digest_serializable(&mut digest, &expected)?;
                    if mapping.source == "v2_giftcard" {
                        let used = source.get("used_user_ids").ok_or_else(|| {
                            ConverterError::MissingColumn {
                                table: mapping.source.to_string(),
                                column: "used_user_ids".to_string(),
                            }
                        })?;
                        let candidates = giftcard_candidate_user_ids(used)?;
                        let known = self.existing_user_ids(&candidates).await?;
                        let giftcard_id = i32::try_from(source_i64(source, "id", mapping.source)?)
                            .map_err(|_| LegacyCopyError::GiftcardIdRange)?;
                        for redemption in expand_giftcard_redemptions(giftcard_id, used, &known)? {
                            digest_serializable(&mut digest, &redemption)?;
                        }
                    }
                }
                let batch_len = u64::try_from(source_rows.len())
                    .map_err(|_| LegacyCopyError::SourceFingerprintMismatch)?;
                table_rows = table_rows
                    .checked_add(batch_len)
                    .ok_or(LegacyCopyError::SourceFingerprintMismatch)?;
                row_count = row_count
                    .checked_add(batch_len)
                    .ok_or(LegacyCopyError::SourceFingerprintMismatch)?;
                after = source_i64(
                    source_rows
                        .last()
                        .ok_or(LegacyCopyError::SourceFingerprintMismatch)?,
                    "id",
                    mapping.source,
                )?;
            }
            digest_field(&mut digest, table_rows.to_string().as_bytes());
        }
        Ok(LegacySourceFingerprint {
            semantic_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.to_string(),
            converter_registry_sha256: registry,
            table_count: TABLE_MAPPINGS.len(),
            row_count,
            canonical_sha256: hex::encode(digest.finalize()),
        })
    }

    async fn existing_user_ids(
        &mut self,
        candidates: &BTreeSet<u64>,
    ) -> Result<BTreeSet<u64>, LegacyCopyError> {
        if candidates.is_empty() {
            return Ok(BTreeSet::new());
        }
        let ids = candidates
            .iter()
            .map(|id| i64::try_from(*id).map_err(|_| LegacyCopyError::GiftcardIdRange))
            .collect::<Result<Vec<_>, _>>()?;
        let mut query = QueryBuilder::<MySql>::new("SELECT id FROM v2_user WHERE id IN (");
        let mut separated = query.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(") ORDER BY id");
        let rows = query
            .build_query_scalar::<i64>()
            .fetch_all(&mut *self.transaction)
            .await
            .map_err(LegacyCopyError::Source)?;
        rows.into_iter()
            .map(|id| u64::try_from(id).map_err(|_| LegacyCopyError::GiftcardIdRange))
            .collect()
    }

    async fn prevalidate_frozen_traffic(
        &mut self,
        binding: &ConversionRunBinding,
        traffic: &VerifiedFrozenTrafficBatch,
    ) -> Result<TrafficFoldPlan, LegacyCopyError> {
        if traffic.receipt.operation_id != binding.operation_id
            || traffic.source_drained.generation == 0
            || !is_lower_sha256(&traffic.source_drained.event_sha256)
            || !is_lower_sha256(&traffic.source_drained.report_sha256)
            || !is_lower_sha256(&traffic.receipt.receipt_sha256)
            || !is_lower_sha256(&traffic.receipt.sorted_user_delta_sha256)
            || traffic.receipt.source_default_run_id.len() != 40
            || !traffic
                .receipt
                .source_default_run_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(LegacyCopyError::TrafficJournalBinding);
        }
        let mut source_users = BTreeMap::new();
        for chunk in traffic.deltas.chunks(TRAFFIC_WRITE_BATCH_SIZE) {
            let mut query = QueryBuilder::<MySql>::new(
                "SELECT id, u, d, t, updated_at FROM v2_user WHERE id IN (",
            );
            let mut ids = query.separated(", ");
            for delta in chunk {
                ids.push_bind(delta.user_id);
            }
            ids.push_unseparated(") ORDER BY id");
            let rows = query
                .build_query_as::<(i64, i64, i64, i64, i64)>()
                .fetch_all(&mut *self.transaction)
                .await
                .map_err(LegacyCopyError::Source)?;
            for (id, u, d, t, updated_at) in rows {
                if source_users.insert(id, (u, d, t, updated_at)).is_some() {
                    return Err(LegacyCopyError::TrafficReceiptMismatch);
                }
            }
        }
        build_traffic_fold_plan(binding, traffic, &source_users)
    }
}

fn build_traffic_fold_plan(
    binding: &ConversionRunBinding,
    traffic: &VerifiedFrozenTrafficBatch,
    source_users: &BTreeMap<i64, (i64, i64, i64, i64)>,
) -> Result<TrafficFoldPlan, LegacyCopyError> {
    if traffic.receipt.operation_id != binding.operation_id
        || source_users.len() != traffic.deltas.len()
    {
        return Err(LegacyCopyError::TrafficUserMissing);
    }
    let after_time = traffic.receipt.fenced_at_unix;
    if after_time <= 0 {
        return Err(LegacyCopyError::TrafficValueRange);
    }
    let mut items = Vec::with_capacity(traffic.deltas.len());
    let mut items_digest = Sha256::new();
    items_digest.update(TRAFFIC_FOLD_ITEMS_DOMAIN);
    for delta in &traffic.deltas {
        if delta.upload < 0 || delta.download < 0 {
            return Err(LegacyCopyError::TrafficValueRange);
        }
        let (before_u, before_d, before_t, before_updated_at) = source_users
            .get(&delta.user_id)
            .copied()
            .ok_or(LegacyCopyError::TrafficUserMissing)?;
        let after_u = before_u
            .checked_add(delta.upload)
            .ok_or(LegacyCopyError::TrafficCounterOverflow)?;
        let after_d = before_d
            .checked_add(delta.download)
            .ok_or(LegacyCopyError::TrafficCounterOverflow)?;
        let mut item = TrafficFoldItemPlan {
            user_id: delta.user_id,
            upload_delta: delta.upload,
            download_delta: delta.download,
            before_u,
            before_d,
            before_t,
            before_updated_at,
            after_u,
            after_d,
            after_t: after_time,
            after_updated_at: after_time,
            item_sha256: String::new(),
        };
        item.item_sha256 =
            traffic_fold_item_sha256(binding, &traffic.receipt.receipt_sha256, &item)?;
        digest_field(&mut items_digest, item.item_sha256.as_bytes());
        items.push(item);
    }
    let items_sha256 = hex::encode(items_digest.finalize());
    let mut plan = TrafficFoldPlan {
        binding: binding.clone(),
        receipt: traffic.receipt.clone(),
        source_drained: traffic.source_drained.clone(),
        items,
        items_sha256,
        fold_verification_sha256: String::new(),
    };
    plan.fold_verification_sha256 = traffic_fold_verification_sha256(&plan)?;
    Ok(plan)
}

/// Computes the exact value fingerprint used by backup/restore verification,
/// the fenced final recheck, and the copy binding. Server identity and database
/// name are deliberately reported separately: an isolated restore uses a
/// different server/database but must produce identical canonical contents.
pub async fn fingerprint_legacy_source(
    pool: &MySqlPool,
    batch_size: u32,
) -> Result<(SourceSnapshotIdentity, LegacySourceFingerprint), LegacyCopyError> {
    let mut snapshot = LegacySourceSnapshot::begin_read_only(pool).await?;
    let identity = snapshot.identity().clone();
    let fingerprint = snapshot.canonical_fingerprint(batch_size).await?;
    snapshot.commit().await?;
    Ok((identity, fingerprint))
}

fn source_json_batch_sql(mapping: &TableMapping) -> Result<String, LegacyCopyError> {
    audit_registry()?;
    let columns = source_columns(mapping);
    let mut object_fields = Vec::with_capacity(columns.len() * 2);
    for column in columns {
        object_fields.push(format!("'{}'", column));
        let identifier = mysql_identifier(column)?;
        let exact_decimal = mapping.transformed_columns.iter().any(|candidate| {
            candidate.source == column && candidate.rule == ColumnRule::ExactDecimal
        });
        if exact_decimal {
            object_fields.push(format!("CAST({identifier} AS CHAR)"));
        } else {
            object_fields.push(identifier);
        }
    }
    Ok(format!(
        "SELECT `id` AS `v2board_selected_id`, CAST(JSON_OBJECT({}) AS CHAR) AS `{SOURCE_JSON_ALIAS}` \
         FROM {} WHERE `id` > ? ORDER BY `id` ASC LIMIT ?",
        object_fields.join(", "),
        mysql_identifier(mapping.source)?,
    ))
}

fn source_columns(mapping: &TableMapping) -> Vec<&str> {
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
        .chain(mapping.deferred_columns.iter().map(|column| column.source))
        .chain(
            mapping
                .consumed_source_columns
                .iter()
                .map(|column| column.source),
        )
        .collect()
}

fn decode_source_json_row(
    mapping: &TableMapping,
    selected_id: i64,
    payload: &str,
) -> Result<SourceRow, LegacyCopyError> {
    let value = serde_json::from_str::<Value>(payload).map_err(|error| {
        LegacyCopyError::InvalidSourceJson {
            table: mapping.source.to_string(),
            message: error.to_string(),
        }
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| LegacyCopyError::InvalidSourceJson {
            table: mapping.source.to_string(),
            message: "JSON_OBJECT did not return an object".to_string(),
        })?;
    let expected = source_columns(mapping).into_iter().collect::<BTreeSet<_>>();
    if object.len() != expected.len()
        || object
            .keys()
            .any(|column| !expected.contains(column.as_str()))
    {
        return Err(LegacyCopyError::InvalidSourceJson {
            table: mapping.source.to_string(),
            message: "source JSON keys differ from the converter registry".to_string(),
        });
    }
    let mut source = SourceRow::new();
    for column in expected {
        let value = object
            .get(column)
            .ok_or_else(|| LegacyCopyError::InvalidSourceJson {
                table: mapping.source.to_string(),
                message: format!("missing JSON key {column}"),
            })?;
        let value = match value {
            Value::Null => SourceValue::Null,
            Value::String(value) => SourceValue::Text(value.clone()),
            Value::Number(value) => {
                if let Some(value) = value.as_i64() {
                    SourceValue::I64(value)
                } else if let Some(value) = value.as_u64() {
                    SourceValue::U64(value)
                } else {
                    return Err(LegacyCopyError::InvalidSourceJson {
                        table: mapping.source.to_string(),
                        message: format!("non-integral JSON number in {column}"),
                    });
                }
            }
            Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
                return Err(LegacyCopyError::InvalidSourceJson {
                    table: mapping.source.to_string(),
                    message: format!("unexpected typed JSON value in {column}"),
                });
            }
        };
        source.insert(column.to_string(), value);
    }
    if source_i64(&source, "id", mapping.source)? != selected_id {
        return Err(LegacyCopyError::SourceIdMismatch {
            table: mapping.source.to_string(),
            selected_id,
        });
    }
    Ok(source)
}

fn mysql_identifier(identifier: &str) -> Result<String, LegacyCopyError> {
    validate_identifier(identifier)?;
    Ok(format!("`{identifier}`"))
}

fn postgres_identifier(identifier: &str) -> Result<String, LegacyCopyError> {
    validate_identifier(identifier)?;
    Ok(format!("\"{identifier}\""))
}

fn validate_identifier(identifier: &str) -> Result<(), LegacyCopyError> {
    if !identifier.is_empty()
        && identifier
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
    {
        Ok(())
    } else {
        Err(ConverterError::Registry(format!("unsafe SQL identifier {identifier:?}")).into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PgColumnKind {
    SmallInt,
    Integer,
    BigInt,
    Numeric,
    Text,
    Jsonb,
    Bytea,
}

impl PgColumnKind {
    fn from_udt_name(udt_name: &str) -> Option<Self> {
        Some(match udt_name {
            "int2" => Self::SmallInt,
            "int4" => Self::Integer,
            "int8" => Self::BigInt,
            "numeric" => Self::Numeric,
            "varchar" | "bpchar" | "text" => Self::Text,
            "jsonb" => Self::Jsonb,
            "bytea" => Self::Bytea,
            _ => return None,
        })
    }

    const fn cast(self) -> &'static str {
        match self {
            Self::SmallInt => "::smallint",
            Self::Integer => "::integer",
            Self::BigInt => "::bigint",
            Self::Numeric => "::numeric",
            Self::Text => "::text",
            Self::Jsonb => "::jsonb",
            Self::Bytea => "::bytea",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::SmallInt => "smallint",
            Self::Integer => "integer",
            Self::BigInt => "bigint",
            Self::Numeric => "numeric",
            Self::Text => "text",
            Self::Jsonb => "jsonb",
            Self::Bytea => "bytea",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PostgresTargetSchema {
    columns: BTreeMap<(String, String), PgColumnKind>,
}

impl PostgresTargetSchema {
    pub async fn inspect(pool: &PgPool) -> Result<Self, LegacyCopyError> {
        audit_registry()?;
        let tables = TABLE_MAPPINGS
            .iter()
            .map(|mapping| mapping.target.to_string())
            .chain(
                DERIVED_MAPPINGS
                    .iter()
                    .map(|mapping| mapping.target.to_string()),
            )
            .collect::<Vec<_>>();
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT table_name, column_name, udt_name \
             FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = ANY($1) \
             ORDER BY table_name, ordinal_position",
        )
        .bind(&tables)
        .fetch_all(pool)
        .await
        .map_err(LegacyCopyError::Target)?;
        let mut columns = BTreeMap::new();
        for (table, column, udt_name) in rows {
            let kind = PgColumnKind::from_udt_name(&udt_name).ok_or_else(|| {
                LegacyCopyError::UnsupportedTargetType {
                    table: table.clone(),
                    column: column.clone(),
                    udt_name,
                }
            })?;
            columns.insert((table, column), kind);
        }
        let schema = Self { columns };
        for mapping in TABLE_MAPPINGS {
            for column in initial_target_columns(mapping) {
                schema.kind(mapping.target, column)?;
            }
            for column in mapping.deferred_columns {
                schema.kind(mapping.target, column.target)?;
            }
            for (table, generated) in TARGET_GENERATED_COLUMNS {
                if *table == mapping.target {
                    for column in *generated {
                        schema.kind(table, column)?;
                    }
                }
            }
        }
        for (table, columns) in [
            (
                "v2_giftcard_redemption",
                &[
                    "giftcard_id",
                    "user_id",
                    "created_at",
                    "created_at_provenance",
                ][..],
            ),
            (
                "v2_server_credential",
                &["node_type", "node_id", "credential_epoch", "updated_at"][..],
            ),
        ] {
            for column in columns {
                schema.kind(table, column)?;
            }
        }
        Ok(schema)
    }

    fn kind(&self, table: &str, column: &str) -> Result<PgColumnKind, LegacyCopyError> {
        self.columns
            .get(&(table.to_string(), column.to_string()))
            .copied()
            .ok_or_else(|| LegacyCopyError::MissingTargetColumn {
                table: table.to_string(),
                column: column.to_string(),
            })
    }
}

pub struct PostgresCopyTarget<'a> {
    pool: &'a PgPool,
    schema: PostgresTargetSchema,
}

impl<'a> PostgresCopyTarget<'a> {
    pub async fn new(pool: &'a PgPool) -> Result<Self, LegacyCopyError> {
        Ok(Self {
            pool,
            schema: PostgresTargetSchema::inspect(pool).await?,
        })
    }

    pub async fn insert_or_compare_batch(
        &self,
        mapping: &TableMapping,
        expected: &[CanonicalRow],
    ) -> Result<(), LegacyCopyError> {
        if expected.is_empty() {
            return Ok(());
        }
        let maximum = maximum_rows_per_target_insert(mapping)?;
        for chunk in expected.chunks(maximum.min(DEFAULT_BATCH_SIZE as usize)) {
            self.insert_or_compare_chunk(mapping, chunk).await?;
        }
        Ok(())
    }

    async fn insert_or_compare_chunk(
        &self,
        mapping: &TableMapping,
        expected: &[CanonicalRow],
    ) -> Result<(), LegacyCopyError> {
        let columns = initial_target_columns(mapping);
        let bind_rows = expected
            .iter()
            .map(|row| self.bind_row(mapping.target, &columns, row))
            .collect::<Result<Vec<_>, _>>()?;
        let mut transaction = begin_durable_target_tx(self.pool).await?;
        let id_identifier = postgres_identifier("id")?;
        let mut insert = QueryBuilder::<Postgres>::new(format!(
            "INSERT INTO {} ({}) ",
            postgres_identifier(mapping.target)?,
            columns
                .iter()
                .map(|column| postgres_identifier(column))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        ));
        insert.push_values(&bind_rows, |mut row, values| {
            for (value, kind) in values {
                row.push_bind(value.as_deref());
                row.push_unseparated(kind.cast());
            }
        });
        insert.push(" ON CONFLICT (");
        insert.push(id_identifier);
        insert.push(") DO NOTHING");
        insert
            .build()
            .execute(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?;
        let stored = self
            .fetch_rows(&mut transaction, mapping.target, &columns, expected)
            .await?;
        compare_expected_rows(mapping.target, expected, &stored)?;
        transaction.commit().await.map_err(LegacyCopyError::Target)
    }

    fn bind_row(
        &self,
        table: &str,
        columns: &[&str],
        row: &CanonicalRow,
    ) -> Result<Vec<(Option<String>, PgColumnKind)>, LegacyCopyError> {
        columns
            .iter()
            .map(|column| {
                let kind = self.schema.kind(table, column)?;
                let value = row
                    .get(*column)
                    .ok_or_else(|| ConverterError::MissingColumn {
                        table: table.to_string(),
                        column: (*column).to_string(),
                    })?;
                Ok((canonical_bind_text(table, column, value, kind)?, kind))
            })
            .collect()
    }

    async fn fetch_rows(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        table: &str,
        columns: &[&str],
        expected: &[CanonicalRow],
    ) -> Result<Vec<CanonicalRow>, LegacyCopyError> {
        let ids = expected
            .iter()
            .map(|row| canonical_i64(row, "id", table))
            .collect::<Result<Vec<_>, _>>()?;
        self.fetch_rows_by_ids(transaction, table, columns, &ids)
            .await
    }

    async fn fetch_rows_by_ids(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        table: &str,
        columns: &[&str],
        ids: &[i64],
    ) -> Result<Vec<CanonicalRow>, LegacyCopyError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut query = QueryBuilder::<Postgres>::new("SELECT id::bigint, jsonb_build_object(");
        let mut fields = query.separated(", ");
        for column in columns {
            fields.push(format!("'{}'", column));
            let identifier = postgres_identifier(column)?;
            match self.schema.kind(table, column)? {
                PgColumnKind::Bytea => fields.push(format!("encode({identifier}, 'hex')")),
                PgColumnKind::Numeric => fields.push(format!("{identifier}::text")),
                _ => fields.push(identifier),
            };
        }
        query.push(format!(
            ") AS {TARGET_JSON_ALIAS} FROM {} WHERE id IN (",
            postgres_identifier(table)?
        ));
        let mut separated = query.separated(", ");
        for id in ids {
            separated.push_bind(*id);
        }
        separated.push_unseparated(") ORDER BY id FOR SHARE");
        let rows = query
            .build_query_as::<(i64, Json<Value>)>()
            .fetch_all(&mut **transaction)
            .await
            .map_err(LegacyCopyError::Target)?;
        rows.into_iter()
            .map(|(id, Json(value))| self.decode_target_row(table, columns, id, value))
            .collect()
    }

    fn decode_target_row(
        &self,
        table: &str,
        columns: &[&str],
        id: i64,
        value: Value,
    ) -> Result<CanonicalRow, LegacyCopyError> {
        let object =
            value
                .as_object()
                .ok_or_else(|| LegacyCopyError::TargetPrimaryKeyMismatch {
                    table: table.to_string(),
                })?;
        let mut row = CanonicalRow::new();
        for column in columns {
            let value =
                object
                    .get(*column)
                    .ok_or_else(|| LegacyCopyError::MissingTargetColumn {
                        table: table.to_string(),
                        column: (*column).to_string(),
                    })?;
            row.insert(
                (*column).to_string(),
                decode_target_value(table, column, value, self.schema.kind(table, column)?)?,
            );
        }
        if canonical_i64(&row, "id", table)? != id {
            return Err(LegacyCopyError::TargetPrimaryKeyMismatch {
                table: table.to_string(),
            });
        }
        Ok(row)
    }

    async fn apply_frozen_traffic(
        &self,
        plan: &TrafficFoldPlan,
    ) -> Result<TrafficFoldVerification, LegacyCopyError> {
        let mut transaction = begin_durable_target_tx(self.pool).await?;
        lock_traffic_fold_gate(&mut transaction, plan).await?;
        if traffic_fold_seal_exists(&mut transaction, plan).await? {
            let report = verify_traffic_fold_in_transaction(&mut transaction, plan).await?;
            transaction
                .rollback()
                .await
                .map_err(LegacyCopyError::Target)?;
            return Ok(report);
        }
        let operation_id = parse_uuid(&plan.binding.operation_id)?;
        let installation_id = parse_uuid(&plan.binding.target_installation_id)?;
        let orphan_items = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM v2_legacy_traffic_fold_item WHERE operation_id = $1",
        )
        .bind(operation_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(LegacyCopyError::Target)?;
        let replayed_receipt = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM v2_legacy_traffic_fold \
             WHERE source_default_run_id = $1 AND source_drain_receipt_sha256 = $2)",
        )
        .bind(&plan.receipt.source_default_run_id)
        .bind(&plan.receipt.receipt_sha256)
        .fetch_one(&mut *transaction)
        .await
        .map_err(LegacyCopyError::Target)?;
        if orphan_items != 0 || replayed_receipt {
            return Err(LegacyCopyError::TrafficLedgerConflict);
        }

        // Lock and compare every target user before the first INSERT/UPDATE.
        let ids = plan
            .items
            .iter()
            .map(|item| item.user_id)
            .collect::<Vec<_>>();
        let target_users = if ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
                "SELECT id, u, d, t, updated_at FROM v2_user \
                 WHERE id = ANY($1) ORDER BY id FOR UPDATE",
            )
            .bind(&ids)
            .fetch_all(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?
        };
        if target_users.len() != plan.items.len()
            || target_users
                .iter()
                .zip(&plan.items)
                .any(|((id, u, d, t, updated_at), expected)| {
                    *id != expected.user_id
                        || *u != expected.before_u
                        || *d != expected.before_d
                        || *t != expected.before_t
                        || *updated_at != expected.before_updated_at
                })
        {
            return Err(LegacyCopyError::TrafficLedgerConflict);
        }

        for chunk in plan.items.chunks(TRAFFIC_WRITE_BATCH_SIZE) {
            let mut insert = QueryBuilder::<Postgres>::new(
                "INSERT INTO v2_legacy_traffic_fold_item (\
                 operation_id, target_installation_id, user_id, upload_delta, download_delta, \
                 before_u, before_d, before_t, before_updated_at, after_u, after_d, after_t, \
                 after_updated_at, item_sha256) ",
            );
            insert.push_values(chunk, |mut row, item| {
                row.push_bind(operation_id)
                    .push_bind(installation_id)
                    .push_bind(item.user_id)
                    .push_bind(item.upload_delta)
                    .push_bind(item.download_delta)
                    .push_bind(item.before_u)
                    .push_bind(item.before_d)
                    .push_bind(item.before_t)
                    .push_bind(item.before_updated_at)
                    .push_bind(item.after_u)
                    .push_bind(item.after_d)
                    .push_bind(item.after_t)
                    .push_bind(item.after_updated_at)
                    .push_bind(&item.item_sha256);
            });
            insert
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(LegacyCopyError::Target)?;

            let mut update = QueryBuilder::<Postgres>::new(
                "UPDATE v2_user AS users SET u = delta.after_u, d = delta.after_d, \
                 t = delta.after_t, updated_at = delta.after_updated_at FROM (",
            );
            update.push_values(chunk, |mut row, item| {
                row.push_bind(item.user_id)
                    .push_bind(item.after_u)
                    .push_bind(item.after_d)
                    .push_bind(item.after_t)
                    .push_bind(item.after_updated_at);
            });
            update.push(
                ") AS delta(user_id, after_u, after_d, after_t, after_updated_at) \
                 WHERE users.id = delta.user_id",
            );
            let updated = update
                .build()
                .execute(&mut *transaction)
                .await
                .map_err(LegacyCopyError::Target)?
                .rows_affected();
            if updated != chunk.len() as u64 {
                return Err(LegacyCopyError::TrafficUserMissing);
            }
        }
        let applied_at = sqlx::query_scalar::<_, i64>(
            "SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint",
        )
        .fetch_one(&mut *transaction)
        .await
        .map_err(LegacyCopyError::Target)?;
        if applied_at <= 0 {
            return Err(LegacyCopyError::TrafficValueRange);
        }
        let seal_sha256 = traffic_fold_seal_sha256(plan, applied_at)?;
        let source_drained_generation = i64::try_from(plan.source_drained.generation)
            .map_err(|_| LegacyCopyError::TrafficValueRange)?;
        sqlx::query(
            "INSERT INTO v2_legacy_traffic_fold (\
             operation_id, target_installation_id, source_default_run_id, \
             source_drain_receipt_sha256, source_drained_journal_generation, \
             source_drained_journal_event_sha256, source_drained_report_sha256, fenced_at, \
             upload_fields, download_fields, sorted_user_delta_count, \
             sorted_user_delta_sha256, upload_delta_sum, download_delta_sum, \
             fold_verification_sha256, seal_sha256, applied_at) VALUES (\
             $1, $2, $3, $4, $5, $6, $7, $8, $9::numeric, $10::numeric, $11::numeric, \
             $12, $13::numeric, $14::numeric, $15, $16, $17)",
        )
        .bind(operation_id)
        .bind(installation_id)
        .bind(&plan.receipt.source_default_run_id)
        .bind(&plan.receipt.receipt_sha256)
        .bind(source_drained_generation)
        .bind(&plan.source_drained.event_sha256)
        .bind(&plan.source_drained.report_sha256)
        .bind(plan.receipt.fenced_at_unix)
        .bind(plan.receipt.upload_fields.to_string())
        .bind(plan.receipt.download_fields.to_string())
        .bind(plan.receipt.sorted_user_delta_count.to_string())
        .bind(&plan.receipt.sorted_user_delta_sha256)
        .bind(&plan.receipt.upload_delta_sum)
        .bind(&plan.receipt.download_delta_sum)
        .bind(&plan.fold_verification_sha256)
        .bind(&seal_sha256)
        .bind(applied_at)
        .execute(&mut *transaction)
        .await
        .map_err(LegacyCopyError::Target)?;
        let report = verify_traffic_fold_in_transaction(&mut transaction, plan).await?;
        transaction
            .commit()
            .await
            .map_err(LegacyCopyError::Target)?;
        Ok(report)
    }

    async fn verify_frozen_traffic(
        &self,
        plan: &TrafficFoldPlan,
    ) -> Result<TrafficFoldVerification, LegacyCopyError> {
        let mut transaction = begin_durable_target_tx(self.pool).await?;
        lock_traffic_fold_gate(&mut transaction, plan).await?;
        let report = verify_traffic_fold_in_transaction(&mut transaction, plan).await?;
        transaction
            .rollback()
            .await
            .map_err(LegacyCopyError::Target)?;
        Ok(report)
    }

    pub async fn table_count(&self, table: &str) -> Result<u64, LegacyCopyError> {
        let sql = format!("SELECT COUNT(*) FROM {}", postgres_identifier(table)?);
        let count = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql))
            .fetch_one(self.pool)
            .await
            .map_err(LegacyCopyError::Target)?;
        u64::try_from(count).map_err(|_| LegacyCopyError::CountMismatch {
            table: table.to_string(),
            source_count: 0,
            target_count: 0,
        })
    }

    pub async fn apply_user_inviter_batch(
        &self,
        source_rows: &[SourceRow],
    ) -> Result<(), LegacyCopyError> {
        if source_rows.is_empty() {
            return Ok(());
        }
        let mut transaction = begin_durable_target_tx(self.pool).await?;
        for source in source_rows {
            let id = source_i64(source, "id", "v2_user")?;
            let inviter = optional_source_i64(source, "invite_user_id", "v2_user")?;
            sqlx::query(
                "UPDATE v2_user SET invite_user_id = $2 \
                 WHERE id = $1 AND invite_user_id IS NULL",
            )
            .bind(id)
            .bind(inviter)
            .execute(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?;
            let stored = sqlx::query_scalar::<_, Option<i64>>(
                "SELECT invite_user_id FROM v2_user WHERE id = $1 FOR SHARE",
            )
            .bind(id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?
            .ok_or_else(|| LegacyCopyError::TargetPrimaryKeyMismatch {
                table: "v2_user".to_string(),
            })?;
            if stored != inviter {
                return Err(ConverterError::RetryConflict.into());
            }
        }
        transaction.commit().await.map_err(LegacyCopyError::Target)
    }

    pub async fn insert_giftcard_redemptions(
        &self,
        rows: &[LegacyGiftcardRedemptionRow],
    ) -> Result<(), LegacyCopyError> {
        if rows.is_empty() {
            return Ok(());
        }
        let mut transaction = begin_durable_target_tx(self.pool).await?;
        let mut insert = QueryBuilder::<Postgres>::new(
            "INSERT INTO v2_giftcard_redemption \
             (giftcard_id, user_id, created_at, created_at_provenance) ",
        );
        insert.push_values(rows, |mut row, value| {
            row.push_bind(value.giftcard_id)
                .push_bind(value.user_id)
                .push_bind(value.created_at)
                .push_bind(&value.created_at_provenance);
        });
        insert.push(" ON CONFLICT (giftcard_id, user_id) DO NOTHING");
        insert
            .build()
            .execute(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?;
        for expected in rows {
            let stored = sqlx::query_as::<_, (i64, String)>(
                "SELECT created_at, created_at_provenance \
                 FROM v2_giftcard_redemption \
                 WHERE giftcard_id = $1 AND user_id = $2 FOR SHARE",
            )
            .bind(expected.giftcard_id)
            .bind(expected.user_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?;
            if stored.as_ref()
                != Some(&(expected.created_at, expected.created_at_provenance.clone()))
            {
                return Err(ConverterError::RetryConflict.into());
            }
        }
        transaction.commit().await.map_err(LegacyCopyError::Target)
    }

    pub async fn derive_and_verify_node_credentials(&self) -> Result<(), LegacyCopyError> {
        let sql = node_credential_rows_sql()?;
        let mut transaction = begin_durable_target_tx(self.pool).await?;
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?;
        let union = NODE_CREDENTIAL_SOURCES
            .iter()
            .map(|(node_type, table)| {
                format!(
                    "SELECT '{node_type}'::text AS node_type, id AS node_id, \
                     0::bigint AS credential_epoch, updated_at FROM {table}"
                )
            })
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        let mismatch_sql = format!(
            "SELECT COUNT(*) FROM ({union}) expected \
             FULL OUTER JOIN v2_server_credential actual \
             USING (node_type, node_id) \
             WHERE expected.node_id IS NULL OR actual.node_id IS NULL \
                OR expected.credential_epoch IS DISTINCT FROM actual.credential_epoch \
                OR expected.updated_at IS DISTINCT FROM actual.updated_at"
        );
        let mismatches = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(mismatch_sql))
            .fetch_one(&mut *transaction)
            .await
            .map_err(LegacyCopyError::Target)?;
        if mismatches != 0 {
            return Err(LegacyCopyError::NodeCredentialMismatch);
        }
        transaction.commit().await.map_err(LegacyCopyError::Target)
    }

    pub async fn reset_sequences(&self) -> Result<(), LegacyCopyError> {
        let mut transaction = begin_durable_target_tx(self.pool).await?;
        for mapping in TABLE_MAPPINGS {
            let sql = sequence_reset_sql(mapping)?;
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql))
                .fetch_one(&mut *transaction)
                .await
                .map_err(LegacyCopyError::Target)?;
        }
        transaction.commit().await.map_err(LegacyCopyError::Target)
    }
}

fn initial_target_columns(mapping: &TableMapping) -> Vec<&str> {
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

fn full_target_columns(mapping: &TableMapping) -> Vec<&str> {
    initial_target_columns(mapping)
        .into_iter()
        .chain(mapping.deferred_columns.iter().map(|column| column.target))
        .chain(
            TARGET_GENERATED_COLUMNS
                .iter()
                .filter(|(table, _)| *table == mapping.target)
                .flat_map(|(_, columns)| columns.iter().copied()),
        )
        .collect()
}

fn canonical_bind_text(
    table: &str,
    column: &str,
    value: &CanonicalValue,
    kind: PgColumnKind,
) -> Result<Option<String>, LegacyCopyError> {
    if matches!(value, CanonicalValue::Null) {
        return Ok(None);
    }
    let compatible = match (value, kind) {
        (CanonicalValue::I64(value), PgColumnKind::SmallInt) => checked_signed_integer(
            table,
            column,
            *value,
            i64::from(i16::MIN),
            i64::from(i16::MAX),
            kind,
        )?,
        (CanonicalValue::I64(value), PgColumnKind::Integer) => checked_signed_integer(
            table,
            column,
            *value,
            i64::from(i32::MIN),
            i64::from(i32::MAX),
            kind,
        )?,
        (CanonicalValue::I64(value), PgColumnKind::BigInt) => value.to_string(),
        (CanonicalValue::U64(value), PgColumnKind::SmallInt) => {
            checked_unsigned_integer(table, column, *value, i16::MAX as u64, kind)?
        }
        (CanonicalValue::U64(value), PgColumnKind::Integer) => {
            checked_unsigned_integer(table, column, *value, i32::MAX as u64, kind)?
        }
        (CanonicalValue::U64(value), PgColumnKind::BigInt) => {
            checked_unsigned_integer(table, column, *value, i64::MAX as u64, kind)?
        }
        (CanonicalValue::Decimal(value), PgColumnKind::Numeric) => value.clone(),
        (CanonicalValue::Text(value), PgColumnKind::Text) => {
            reject_nul(table, column, value)?;
            value.clone()
        }
        (CanonicalValue::Json(value), PgColumnKind::Jsonb) => {
            if json_contains_nul(value) {
                return Err(LegacyCopyError::UnsupportedTextNul {
                    table: table.to_string(),
                    column: column.to_string(),
                });
            }
            value.to_string()
        }
        (CanonicalValue::Bytes(value), PgColumnKind::Bytea) => format!("\\x{}", hex::encode(value)),
        _ => {
            return Err(LegacyCopyError::TargetTypeMismatch {
                table: table.to_string(),
                column: column.to_string(),
                expected: kind.label(),
            });
        }
    };
    Ok(Some(compatible))
}

fn checked_signed_integer(
    table: &str,
    column: &str,
    value: i64,
    minimum: i64,
    maximum: i64,
    kind: PgColumnKind,
) -> Result<String, LegacyCopyError> {
    if (minimum..=maximum).contains(&value) {
        Ok(value.to_string())
    } else {
        Err(LegacyCopyError::TargetIntegerRange {
            table: table.to_string(),
            column: column.to_string(),
            expected: kind.label(),
        })
    }
}

fn checked_unsigned_integer(
    table: &str,
    column: &str,
    value: u64,
    maximum: u64,
    kind: PgColumnKind,
) -> Result<String, LegacyCopyError> {
    if value <= maximum {
        Ok(value.to_string())
    } else {
        Err(LegacyCopyError::TargetIntegerRange {
            table: table.to_string(),
            column: column.to_string(),
            expected: kind.label(),
        })
    }
}

fn reject_nul(table: &str, column: &str, value: &str) -> Result<(), LegacyCopyError> {
    if value.contains('\0') {
        Err(LegacyCopyError::UnsupportedTextNul {
            table: table.to_string(),
            column: column.to_string(),
        })
    } else {
        Ok(())
    }
}

fn validate_canonical_row_for_postgres(
    table: &str,
    row: &CanonicalRow,
) -> Result<(), LegacyCopyError> {
    for (column, value) in row {
        match value {
            CanonicalValue::Text(value) | CanonicalValue::Decimal(value) => {
                reject_nul(table, column, value)?;
            }
            CanonicalValue::Json(value) if json_contains_nul(value) => {
                return Err(LegacyCopyError::UnsupportedTextNul {
                    table: table.to_string(),
                    column: column.to_string(),
                });
            }
            CanonicalValue::Null
            | CanonicalValue::I64(_)
            | CanonicalValue::U64(_)
            | CanonicalValue::Bytes(_)
            | CanonicalValue::Json(_) => {}
        }
    }
    Ok(())
}

fn json_contains_nul(value: &Value) -> bool {
    match value {
        Value::String(value) => value.contains('\0'),
        Value::Array(values) => values.iter().any(json_contains_nul),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| key.contains('\0') || json_contains_nul(value)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn decode_target_value(
    table: &str,
    column: &str,
    value: &Value,
    kind: PgColumnKind,
) -> Result<CanonicalValue, LegacyCopyError> {
    if value.is_null() {
        return Ok(CanonicalValue::Null);
    }
    let mismatch = || LegacyCopyError::TargetTypeMismatch {
        table: table.to_string(),
        column: column.to_string(),
        expected: kind.label(),
    };
    match kind {
        PgColumnKind::SmallInt | PgColumnKind::Integer | PgColumnKind::BigInt => value
            .as_i64()
            .map(CanonicalValue::I64)
            .or_else(|| value.as_u64().map(CanonicalValue::U64))
            .ok_or_else(mismatch),
        PgColumnKind::Numeric => {
            let text = value.as_str().ok_or_else(mismatch)?;
            let normalized = normalize_decimal(text).ok_or_else(mismatch)?;
            Ok(CanonicalValue::Decimal(normalized))
        }
        PgColumnKind::Text => value
            .as_str()
            .map(|value| CanonicalValue::Text(value.to_string()))
            .ok_or_else(mismatch),
        PgColumnKind::Jsonb => Ok(CanonicalValue::Json(value.clone())),
        PgColumnKind::Bytea => {
            let value = value.as_str().ok_or_else(mismatch)?;
            let bytes = hex::decode(value).map_err(|_| mismatch())?;
            Ok(CanonicalValue::Bytes(bytes))
        }
    }
}

fn normalize_decimal(value: &str) -> Option<String> {
    if value.is_empty()
        || value.starts_with('+')
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return None;
    }
    let (negative, unsigned) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    let split = unsigned.split_once('.');
    let (integer, fraction) = split.unwrap_or((unsigned, ""));
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || unsigned.matches('.').count() > 1
        || split.is_some_and(|_| fraction.is_empty())
    {
        return None;
    }
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction.trim_end_matches('0');
    let sign = if negative && (integer != "0" || !fraction.is_empty()) {
        "-"
    } else {
        ""
    };
    if fraction.is_empty() {
        Some(format!("{sign}{integer}"))
    } else {
        Some(format!("{sign}{integer}.{fraction}"))
    }
}

fn compare_expected_rows(
    table: &str,
    expected: &[CanonicalRow],
    stored: &[CanonicalRow],
) -> Result<(), LegacyCopyError> {
    if expected.len() != stored.len() {
        return Err(LegacyCopyError::TargetPrimaryKeyMismatch {
            table: table.to_string(),
        });
    }
    let expected = expected
        .iter()
        .map(|row| Ok((canonical_i64(row, "id", table)?, row)))
        .collect::<Result<BTreeMap<_, _>, LegacyCopyError>>()?;
    let stored = stored
        .iter()
        .map(|row| Ok((canonical_i64(row, "id", table)?, row)))
        .collect::<Result<BTreeMap<_, _>, LegacyCopyError>>()?;
    if expected.keys().ne(stored.keys()) {
        return Err(LegacyCopyError::TargetPrimaryKeyMismatch {
            table: table.to_string(),
        });
    }
    for (id, expected) in expected {
        let stored = stored
            .get(&id)
            .ok_or_else(|| LegacyCopyError::TargetPrimaryKeyMismatch {
                table: table.to_string(),
            })?;
        retry_accepts_existing(expected, stored)?;
    }
    Ok(())
}

fn source_i64(row: &SourceRow, column: &str, table: &str) -> Result<i64, LegacyCopyError> {
    match row.get(column) {
        Some(SourceValue::I64(value)) => Ok(*value),
        Some(SourceValue::U64(value)) => {
            i64::try_from(*value).map_err(|_| LegacyCopyError::InvalidSourceJson {
                table: table.to_string(),
                message: format!("{column} exceeds i64"),
            })
        }
        _ => Err(LegacyCopyError::InvalidSourceJson {
            table: table.to_string(),
            message: format!("{column} is not an integer"),
        }),
    }
}

fn optional_source_i64(
    row: &SourceRow,
    column: &str,
    table: &str,
) -> Result<Option<i64>, LegacyCopyError> {
    match row.get(column) {
        Some(SourceValue::Null) => Ok(None),
        _ => source_i64(row, column, table).map(Some),
    }
}

fn canonical_i64(row: &CanonicalRow, column: &str, table: &str) -> Result<i64, LegacyCopyError> {
    match row.get(column) {
        Some(CanonicalValue::I64(value)) => Ok(*value),
        Some(CanonicalValue::U64(value)) => {
            i64::try_from(*value).map_err(|_| LegacyCopyError::TargetPrimaryKeyMismatch {
                table: table.to_string(),
            })
        }
        _ => Err(LegacyCopyError::TargetPrimaryKeyMismatch {
            table: table.to_string(),
        }),
    }
}

fn canonical_direct(value: &SourceValue) -> CanonicalValue {
    match value {
        SourceValue::Null => CanonicalValue::Null,
        SourceValue::I64(value) => CanonicalValue::I64(*value),
        SourceValue::U64(value) => CanonicalValue::U64(*value),
        SourceValue::Decimal(value) => CanonicalValue::Decimal(value.clone()),
        SourceValue::Text(value) => CanonicalValue::Text(value.clone()),
        SourceValue::Bytes(value) => CanonicalValue::Bytes(value.clone()),
    }
}

fn expected_full_row(
    mapping: &TableMapping,
    source: &SourceRow,
) -> Result<CanonicalRow, LegacyCopyError> {
    let mut row = transform_row(mapping, source)?;
    for deferred in mapping.deferred_columns {
        let value = source
            .get(deferred.source)
            .ok_or_else(|| ConverterError::MissingColumn {
                table: mapping.source.to_string(),
                column: deferred.source.to_string(),
            })?;
        row.insert(deferred.target.to_string(), canonical_direct(value));
    }
    match mapping.target {
        "v2_order" => {
            let plan_id = canonical_i64(&row, "plan_id", mapping.target)?;
            row.insert(
                "referenced_plan_id".to_string(),
                if plan_id == 0 {
                    CanonicalValue::Null
                } else {
                    CanonicalValue::I64(plan_id)
                },
            );
            let status = canonical_i64(&row, "status", mapping.target)?;
            let user_id = canonical_i64(&row, "user_id", mapping.target)?;
            row.insert(
                "unfinished_user_id".to_string(),
                if matches!(status, 0 | 1) {
                    CanonicalValue::I64(user_id)
                } else {
                    CanonicalValue::Null
                },
            );
        }
        "v2_ticket" => {
            let status = canonical_i64(&row, "status", mapping.target)?;
            let user_id = canonical_i64(&row, "user_id", mapping.target)?;
            row.insert(
                "open_user_id".to_string(),
                if status == 0 {
                    CanonicalValue::I64(user_id)
                } else {
                    CanonicalValue::Null
                },
            );
        }
        _ => {}
    }
    for (table, columns) in TARGET_GENERATED_COLUMNS {
        if *table == mapping.target {
            for column in *columns {
                if !row.contains_key(*column) {
                    return Err(LegacyCopyError::GeneratedValue {
                        table: mapping.target.to_string(),
                        column: (*column).to_string(),
                    });
                }
            }
        }
    }
    Ok(row)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TableVerification {
    pub table: String,
    pub source_count: u64,
    pub target_count: u64,
    pub source_primary_key_sha256: String,
    pub target_primary_key_sha256: String,
    pub source_canonical_sha256: String,
    pub target_canonical_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DerivedVerification {
    pub target: String,
    pub expected_count: u64,
    pub target_count: u64,
    pub expected_sha256: String,
    pub target_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TrafficFoldVerification {
    pub operation_id: String,
    pub target_installation_id: String,
    pub source_default_run_id: String,
    pub source_drain_receipt_sha256: String,
    pub source_drained_journal_generation: u64,
    pub source_drained_journal_event_sha256: String,
    pub source_drained_report_sha256: String,
    pub fenced_at: i64,
    pub upload_fields: u64,
    pub download_fields: u64,
    pub sorted_user_delta_count: u64,
    pub sorted_user_delta_sha256: String,
    pub upload_delta_sum: String,
    pub download_delta_sum: String,
    pub ledger_item_count: u64,
    pub ledger_items_sha256: String,
    pub fold_verification_sha256: String,
    pub seal_sha256: String,
    pub applied_at: i64,
    pub applied_exactly_once: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacyCopyVerification {
    pub base_tables: Vec<TableVerification>,
    pub derived_tables: Vec<DerivedVerification>,
    pub traffic_fold: TrafficFoldVerification,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct TrafficFoldItemPlan {
    user_id: i64,
    upload_delta: i64,
    download_delta: i64,
    before_u: i64,
    before_d: i64,
    before_t: i64,
    before_updated_at: i64,
    after_u: i64,
    after_d: i64,
    after_t: i64,
    after_updated_at: i64,
    item_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrafficFoldPlan {
    binding: ConversionRunBinding,
    receipt: VerifiedFrozenTrafficReceipt,
    source_drained: SourceDrainJournalBinding,
    items: Vec<TrafficFoldItemPlan>,
    items_sha256: String,
    fold_verification_sha256: String,
}

#[derive(sqlx::FromRow)]
struct TrafficFoldSealRow {
    target_installation_id: String,
    source_default_run_id: String,
    source_drain_receipt_sha256: String,
    source_drained_journal_generation: i64,
    source_drained_journal_event_sha256: String,
    source_drained_report_sha256: String,
    fenced_at: i64,
    upload_fields: String,
    download_fields: String,
    sorted_user_delta_count: String,
    sorted_user_delta_sha256: String,
    upload_delta_sum: String,
    download_delta_sum: String,
    fold_verification_sha256: String,
    seal_sha256: String,
    applied_at: i64,
}

#[derive(Debug, Eq, PartialEq, sqlx::FromRow)]
struct TrafficFoldItemRow {
    user_id: i64,
    upload_delta: i64,
    download_delta: i64,
    before_u: i64,
    before_d: i64,
    before_t: i64,
    before_updated_at: i64,
    after_u: i64,
    after_d: i64,
    after_t: i64,
    after_updated_at: i64,
    item_sha256: String,
}

pub struct LegacyCopyAdapter<'a> {
    source: LegacySourceSnapshot,
    target: PostgresCopyTarget<'a>,
    binding: ConversionRunBinding,
    batch_size: u32,
}

impl<'a> LegacyCopyAdapter<'a> {
    pub async fn new(
        source_pool: &MySqlPool,
        target_pool: &'a PgPool,
        binding: ConversionRunBinding,
        batch_size: u32,
    ) -> Result<Self, LegacyCopyError> {
        if batch_size == 0 || batch_size > MAX_BATCH_SIZE {
            return Err(ConverterError::InvalidBatchSize(batch_size).into());
        }
        binding.validate()?;
        Ok(Self {
            source: LegacySourceSnapshot::begin(source_pool, &binding).await?,
            target: PostgresCopyTarget::new(target_pool).await?,
            binding,
            batch_size,
        })
    }

    pub fn source_identity(&self) -> &SourceSnapshotIdentity {
        self.source.identity()
    }

    async fn copy_base_tables<S>(
        &mut self,
        sink: &mut S,
        traffic: &TrafficFoldPlan,
    ) -> Result<ConversionCheckpoint, LegacyCopyError>
    where
        S: DurableCopyCheckpointSink,
    {
        // Recompute the same full value fingerprint on every process start,
        // including resume. A row-value change with identical counts must not
        // be hidden behind a previously durable copy prefix.
        let observed_source = self.source.canonical_fingerprint(self.batch_size).await?;
        if observed_source.canonical_sha256 != self.binding.source_snapshot_sha256
            || observed_source.semantic_schema_sha256 != self.binding.source_schema_sha256
            || observed_source.converter_registry_sha256 != self.binding.registry_sha256
        {
            return Err(LegacyCopyError::SourceFingerprintMismatch);
        }
        let durable = sink
            .load(&self.binding)
            .await
            .map_err(|error| LegacyCopyError::Checkpoint(Box::new(error)))?;
        let mut checkpoint = match durable {
            Some(checkpoint) => {
                checkpoint.validate_resume(&self.binding, None)?;
                self.verify_copy_prefix(&checkpoint).await?;
                checkpoint
            }
            None => {
                self.prevalidate_source().await?;
                let checkpoint = initial_checkpoint(&self.binding)?;
                sink.compare_and_store(None, &checkpoint)
                    .await
                    .map_err(|error| LegacyCopyError::Checkpoint(Box::new(error)))?;
                checkpoint
            }
        };
        if checkpoint.phase > ConversionPhase::CopyBaseTables {
            let mut transaction = begin_durable_target_tx(self.target.pool).await?;
            lock_traffic_fold_gate(&mut transaction, traffic).await?;
            let folded = traffic_fold_seal_exists(&mut transaction, traffic).await?;
            transaction
                .rollback()
                .await
                .map_err(LegacyCopyError::Target)?;
            self.verify_initial_copy_all(folded.then_some(traffic))
                .await?;
            return Ok(checkpoint);
        }
        if checkpoint.phase != ConversionPhase::CopyBaseTables {
            return Err(LegacyCopyError::UnsupportedResumeCheckpoint);
        }

        for (index, mapping) in TABLE_MAPPINGS.iter().enumerate() {
            if mapping.order < checkpoint.table_order {
                continue;
            }
            let mut after_id = if mapping.order == checkpoint.table_order {
                if checkpoint.table != mapping.source {
                    return Err(LegacyCopyError::UnsupportedResumeCheckpoint);
                }
                checkpoint.last_source_id
            } else {
                INITIAL_SOURCE_ID_CURSOR
            };
            loop {
                let source_rows = self
                    .source
                    .read_batch(mapping, after_id, self.batch_size)
                    .await?;
                if source_rows.is_empty() {
                    break;
                }
                let expected = source_rows
                    .iter()
                    .map(|row| transform_row(mapping, row).map_err(LegacyCopyError::from))
                    .collect::<Result<Vec<_>, _>>()?;
                self.target
                    .insert_or_compare_batch(mapping, &expected)
                    .await?;
                let last_id = source_i64(
                    source_rows
                        .last()
                        .ok_or_else(|| LegacyCopyError::UnsupportedResumeCheckpoint)?,
                    "id",
                    mapping.source,
                )?;
                let batch_sha = canonical_rows_sha256(mapping, &expected)?;
                let next = ConversionCheckpoint {
                    binding: self.binding.clone(),
                    phase: ConversionPhase::CopyBaseTables,
                    table_order: mapping.order,
                    table: mapping.source.to_string(),
                    last_source_id: last_id,
                    source_rows_seen: checkpoint
                        .source_rows_seen
                        .saturating_add(source_rows.len() as u64),
                    target_rows_verified: checkpoint
                        .target_rows_verified
                        .saturating_add(expected.len() as u64),
                    rolling_sha256: advance_rolling_sha256(
                        &checkpoint.rolling_sha256,
                        mapping.source,
                        last_id,
                        &batch_sha,
                    )?,
                };
                next.validate_resume(&self.binding, Some(&checkpoint))?;
                sink.compare_and_store(Some(&checkpoint), &next)
                    .await
                    .map_err(|error| LegacyCopyError::Checkpoint(Box::new(error)))?;
                checkpoint = next;
                after_id = last_id;
            }
            let next_mapping = TABLE_MAPPINGS.get(index + 1);
            let next = if let Some(next_mapping) = next_mapping {
                ConversionCheckpoint {
                    binding: self.binding.clone(),
                    phase: ConversionPhase::CopyBaseTables,
                    table_order: next_mapping.order,
                    table: next_mapping.source.to_string(),
                    last_source_id: INITIAL_SOURCE_ID_CURSOR,
                    source_rows_seen: checkpoint.source_rows_seen,
                    target_rows_verified: checkpoint.target_rows_verified,
                    rolling_sha256: checkpoint.rolling_sha256.clone(),
                }
            } else {
                ConversionCheckpoint {
                    binding: self.binding.clone(),
                    phase: ConversionPhase::ApplyDeferredReferences,
                    table_order: 50,
                    table: "v2_user".to_string(),
                    last_source_id: INITIAL_SOURCE_ID_CURSOR,
                    source_rows_seen: checkpoint.source_rows_seen,
                    target_rows_verified: checkpoint.target_rows_verified,
                    rolling_sha256: checkpoint.rolling_sha256.clone(),
                }
            };
            next.validate_resume(&self.binding, Some(&checkpoint))?;
            sink.compare_and_store(Some(&checkpoint), &next)
                .await
                .map_err(|error| LegacyCopyError::Checkpoint(Box::new(error)))?;
            checkpoint = next;
        }
        Ok(checkpoint)
    }

    async fn prevalidate_source(&mut self) -> Result<(), LegacyCopyError> {
        for mapping in TABLE_MAPPINGS {
            let columns = full_target_columns(mapping);
            let mut after = INITIAL_SOURCE_ID_CURSOR;
            loop {
                let source_rows = self
                    .source
                    .read_batch(mapping, after, self.batch_size)
                    .await?;
                if source_rows.is_empty() {
                    break;
                }
                for source in &source_rows {
                    let expected = expected_full_row(mapping, source)?;
                    self.target.bind_row(mapping.target, &columns, &expected)?;
                    if mapping.source == "v2_giftcard" {
                        let used = source.get("used_user_ids").ok_or_else(|| {
                            ConverterError::MissingColumn {
                                table: mapping.source.to_string(),
                                column: "used_user_ids".to_string(),
                            }
                        })?;
                        let candidates = giftcard_candidate_user_ids(used)?;
                        let known = self.source.existing_user_ids(&candidates).await?;
                        let giftcard_id = i32::try_from(source_i64(source, "id", mapping.source)?)
                            .map_err(|_| LegacyCopyError::GiftcardIdRange)?;
                        expand_giftcard_redemptions(giftcard_id, used, &known)?;
                    }
                }
                after = source_i64(
                    source_rows
                        .last()
                        .ok_or(LegacyCopyError::UnsupportedResumeCheckpoint)?,
                    "id",
                    mapping.source,
                )?;
            }
        }
        Ok(())
    }

    async fn verify_initial_copy_all(
        &mut self,
        traffic: Option<&TrafficFoldPlan>,
    ) -> Result<(), LegacyCopyError> {
        for mapping in TABLE_MAPPINGS {
            let mut after = INITIAL_SOURCE_ID_CURSOR;
            let mut source_count = 0_u64;
            loop {
                let source_rows = self
                    .source
                    .read_batch(mapping, after, self.batch_size)
                    .await?;
                if source_rows.is_empty() {
                    break;
                }
                let mut expected = source_rows
                    .iter()
                    .map(|row| transform_row(mapping, row).map_err(LegacyCopyError::from))
                    .collect::<Result<Vec<_>, _>>()?;
                if let Some(traffic) = traffic {
                    overlay_frozen_traffic(mapping, &mut expected, traffic)?;
                }
                let mut transaction = self
                    .target
                    .pool
                    .begin()
                    .await
                    .map_err(LegacyCopyError::Target)?;
                let stored = self
                    .target
                    .fetch_rows(
                        &mut transaction,
                        mapping.target,
                        &initial_target_columns(mapping),
                        &expected,
                    )
                    .await?;
                compare_expected_rows(mapping.target, &expected, &stored)?;
                transaction
                    .rollback()
                    .await
                    .map_err(LegacyCopyError::Target)?;
                source_count = source_count.saturating_add(expected.len() as u64);
                after = source_i64(
                    source_rows
                        .last()
                        .ok_or(LegacyCopyError::ResumePrefixMismatch)?,
                    "id",
                    mapping.source,
                )?;
            }
            let target_count = self.target.table_count(mapping.target).await?;
            if source_count != target_count {
                return Err(LegacyCopyError::CountMismatch {
                    table: mapping.target.to_string(),
                    source_count,
                    target_count,
                });
            }
        }
        Ok(())
    }

    /// Executes every PostgreSQL conversion stage inside the one source
    /// snapshot. Stage work is idempotent and a durable CAS checkpoint is
    /// written only after its target verification succeeds. A crash inside a
    /// stage replays that whole stage; it never skips partially written data.
    pub async fn execute<S>(
        &mut self,
        sink: &mut S,
        traffic: &VerifiedFrozenTrafficBatch,
    ) -> Result<(ConversionCheckpoint, LegacyCopyVerification), LegacyCopyError>
    where
        S: DurableCopyCheckpointSink,
    {
        // This validates receipt/run-id/deltas against the same fenced MySQL
        // snapshot before copy_base_tables can perform its first target write.
        let traffic_plan = self
            .source
            .prevalidate_frozen_traffic(&self.binding, traffic)
            .await?;
        let first_mapping = TABLE_MAPPINGS
            .first()
            .ok_or_else(|| ConverterError::Registry("empty converter registry".to_string()))?;
        let mut checkpoint = self.copy_base_tables(sink, &traffic_plan).await?;

        if checkpoint.phase == ConversionPhase::ApplyDeferredReferences {
            self.apply_deferred_user_references().await?;
            checkpoint = self
                .advance_stage_checkpoint(
                    sink,
                    checkpoint,
                    ConversionPhase::BuildDerivedRows,
                    280,
                    "v2_giftcard_redemption",
                    "deferred_user_references_verified",
                )
                .await?;
        }
        if checkpoint.phase == ConversionPhase::BuildDerivedRows {
            self.copy_giftcard_redemptions().await?;
            self.verify_giftcard_redemptions().await?;
            self.derive_node_credentials().await?;
            self.verify_node_credentials().await?;
            checkpoint = self
                .advance_stage_checkpoint(
                    sink,
                    checkpoint,
                    ConversionPhase::ResetSequences,
                    first_mapping.order,
                    first_mapping.target,
                    "derived_rows_verified",
                )
                .await?;
        }
        if checkpoint.phase == ConversionPhase::ResetSequences {
            self.reset_sequences().await?;
            checkpoint = self
                .advance_stage_checkpoint(
                    sink,
                    checkpoint,
                    ConversionPhase::FoldFrozenTraffic,
                    first_mapping.order,
                    first_mapping.target,
                    "sequences_reset",
                )
                .await?;
        }
        if checkpoint.phase == ConversionPhase::FoldFrozenTraffic {
            let traffic_verification = self.target.apply_frozen_traffic(&traffic_plan).await?;
            checkpoint = self
                .advance_stage_checkpoint(
                    sink,
                    checkpoint,
                    ConversionPhase::VerifyAllValues,
                    first_mapping.order,
                    first_mapping.target,
                    &traffic_verification.fold_verification_sha256,
                )
                .await?;
        }
        let verification = self.verify_all(&traffic_plan).await?;
        if checkpoint.phase == ConversionPhase::VerifyAllValues {
            let verification_sha256 = legacy_copy_verification_sha256(&verification)?;
            checkpoint = self
                .advance_stage_checkpoint(
                    sink,
                    checkpoint,
                    ConversionPhase::Complete,
                    u16::MAX,
                    "complete",
                    &verification_sha256,
                )
                .await?;
        }
        if checkpoint.phase != ConversionPhase::Complete {
            return Err(LegacyCopyError::UnsupportedResumeCheckpoint);
        }
        Ok((checkpoint, verification))
    }

    /// Recomputes the complete source-to-target value report without
    /// advancing the converter checkpoint. This is the independent
    /// `postgres_bulk_copied -> postgres_value_verified` gate and is also used
    /// to reconstruct the exact report on a later ClickHouse-stage retry.
    pub async fn verify_completed(
        &mut self,
        traffic: &VerifiedFrozenTrafficBatch,
    ) -> Result<LegacyCopyVerification, LegacyCopyError> {
        let traffic_plan = self
            .source
            .prevalidate_frozen_traffic(&self.binding, traffic)
            .await?;
        self.verify_all(&traffic_plan).await
    }

    async fn advance_stage_checkpoint<S>(
        &self,
        sink: &mut S,
        previous: ConversionCheckpoint,
        phase: ConversionPhase,
        table_order: u16,
        table: &str,
        stage_proof: &str,
    ) -> Result<ConversionCheckpoint, LegacyCopyError>
    where
        S: DurableCopyCheckpointSink,
    {
        let proof_sha256 = hex::encode(Sha256::digest(stage_proof.as_bytes()));
        let next = ConversionCheckpoint {
            binding: self.binding.clone(),
            phase,
            table_order,
            table: table.to_string(),
            last_source_id: INITIAL_SOURCE_ID_CURSOR,
            source_rows_seen: previous.source_rows_seen,
            target_rows_verified: previous.target_rows_verified,
            rolling_sha256: advance_rolling_sha256(
                &previous.rolling_sha256,
                table,
                INITIAL_SOURCE_ID_CURSOR,
                &proof_sha256,
            )?,
        };
        next.validate_resume(&self.binding, Some(&previous))?;
        sink.compare_and_store(Some(&previous), &next)
            .await
            .map_err(|error| LegacyCopyError::Checkpoint(Box::new(error)))?;
        Ok(next)
    }

    async fn verify_copy_prefix(
        &mut self,
        checkpoint: &ConversionCheckpoint,
    ) -> Result<(), LegacyCopyError> {
        if checkpoint.phase != ConversionPhase::CopyBaseTables {
            return Ok(());
        }
        let mut rolling = initial_rolling_sha256(&self.binding)?;
        let mut rows_seen = 0_u64;
        for mapping in TABLE_MAPPINGS {
            if mapping.order > checkpoint.table_order {
                break;
            }
            let upper = if mapping.order == checkpoint.table_order {
                checkpoint.last_source_id
            } else {
                i64::MAX
            };
            if upper == INITIAL_SOURCE_ID_CURSOR {
                break;
            }
            let mut after = INITIAL_SOURCE_ID_CURSOR;
            loop {
                let source_rows = self
                    .source
                    .read_batch(mapping, after, self.batch_size)
                    .await?;
                let source_rows = source_rows
                    .into_iter()
                    .take_while(|row| {
                        source_i64(row, "id", mapping.source).is_ok_and(|id| id <= upper)
                    })
                    .collect::<Vec<_>>();
                if source_rows.is_empty() {
                    break;
                }
                let expected = source_rows
                    .iter()
                    .map(|row| transform_row(mapping, row).map_err(LegacyCopyError::from))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut transaction = self
                    .target
                    .pool
                    .begin()
                    .await
                    .map_err(LegacyCopyError::Target)?;
                let stored = self
                    .target
                    .fetch_rows(
                        &mut transaction,
                        mapping.target,
                        &initial_target_columns(mapping),
                        &expected,
                    )
                    .await?;
                compare_expected_rows(mapping.target, &expected, &stored)?;
                transaction
                    .rollback()
                    .await
                    .map_err(LegacyCopyError::Target)?;
                let last_id = source_i64(
                    source_rows
                        .last()
                        .ok_or(LegacyCopyError::ResumePrefixMismatch)?,
                    "id",
                    mapping.source,
                )?;
                let batch_sha = canonical_rows_sha256(mapping, &expected)?;
                rolling = advance_rolling_sha256(&rolling, mapping.source, last_id, &batch_sha)?;
                rows_seen = rows_seen.saturating_add(source_rows.len() as u64);
                after = last_id;
                if last_id >= upper {
                    break;
                }
            }
        }
        if rolling != checkpoint.rolling_sha256
            || rows_seen != checkpoint.source_rows_seen
            || rows_seen != checkpoint.target_rows_verified
        {
            return Err(LegacyCopyError::ResumePrefixMismatch);
        }
        Ok(())
    }

    pub async fn apply_deferred_user_references(&mut self) -> Result<(), LegacyCopyError> {
        let mapping = mapping_for_source("v2_user")
            .ok_or_else(|| ConverterError::UnknownTable("v2_user".to_string()))?;
        let mut after = INITIAL_SOURCE_ID_CURSOR;
        loop {
            let rows = self
                .source
                .read_batch(mapping, after, self.batch_size)
                .await?;
            if rows.is_empty() {
                break;
            }
            self.target.apply_user_inviter_batch(&rows).await?;
            after = source_i64(
                rows.last()
                    .ok_or(LegacyCopyError::UnsupportedResumeCheckpoint)?,
                "id",
                mapping.source,
            )?;
        }
        Ok(())
    }

    pub async fn copy_giftcard_redemptions(&mut self) -> Result<(), LegacyCopyError> {
        let mapping = mapping_for_source("v2_giftcard")
            .ok_or_else(|| ConverterError::UnknownTable("v2_giftcard".to_string()))?;
        let mut after = INITIAL_SOURCE_ID_CURSOR;
        loop {
            let source_rows = self
                .source
                .read_batch(mapping, after, self.batch_size)
                .await?;
            if source_rows.is_empty() {
                break;
            }
            for source in &source_rows {
                let giftcard_id = i32::try_from(source_i64(source, "id", mapping.source)?)
                    .map_err(|_| LegacyCopyError::GiftcardIdRange)?;
                let used =
                    source
                        .get("used_user_ids")
                        .ok_or_else(|| ConverterError::MissingColumn {
                            table: mapping.source.to_string(),
                            column: "used_user_ids".to_string(),
                        })?;
                let candidates = giftcard_candidate_user_ids(used)?;
                let known = self.target.existing_user_ids(&candidates).await?;
                let rows = expand_giftcard_redemptions(giftcard_id, used, &known)?;
                self.target.insert_giftcard_redemptions(&rows).await?;
            }
            after = source_i64(
                source_rows
                    .last()
                    .ok_or(LegacyCopyError::UnsupportedResumeCheckpoint)?,
                "id",
                mapping.source,
            )?;
        }
        Ok(())
    }

    pub async fn derive_node_credentials(&self) -> Result<(), LegacyCopyError> {
        self.target.derive_and_verify_node_credentials().await
    }

    pub async fn verify_giftcard_redemptions(
        &mut self,
    ) -> Result<DerivedVerification, LegacyCopyError> {
        let mapping = mapping_for_source("v2_giftcard")
            .ok_or_else(|| ConverterError::UnknownTable("v2_giftcard".to_string()))?;
        let mut after = INITIAL_SOURCE_ID_CURSOR;
        let mut expected_count = 0_u64;
        let mut expected_digest = Sha256::new();
        let mut target_digest = Sha256::new();
        expected_digest.update(FULL_ROW_DOMAIN);
        target_digest.update(FULL_ROW_DOMAIN);
        loop {
            let source_rows = self
                .source
                .read_batch(mapping, after, self.batch_size)
                .await?;
            if source_rows.is_empty() {
                break;
            }
            for source in &source_rows {
                let giftcard_id = i32::try_from(source_i64(source, "id", mapping.source)?)
                    .map_err(|_| LegacyCopyError::GiftcardIdRange)?;
                let used =
                    source
                        .get("used_user_ids")
                        .ok_or_else(|| ConverterError::MissingColumn {
                            table: mapping.source.to_string(),
                            column: "used_user_ids".to_string(),
                        })?;
                let candidates = giftcard_candidate_user_ids(used)?;
                let known = self.target.existing_user_ids(&candidates).await?;
                let expected = expand_giftcard_redemptions(giftcard_id, used, &known)?;
                let stored = self.target.giftcard_redemptions_for(giftcard_id).await?;
                if expected != stored {
                    return Err(LegacyCopyError::ValueDigestMismatch {
                        table: "v2_giftcard_redemption".to_string(),
                    });
                }
                for row in &expected {
                    digest_serializable(&mut expected_digest, row)?;
                }
                for row in &stored {
                    digest_serializable(&mut target_digest, row)?;
                }
                expected_count = expected_count.saturating_add(expected.len() as u64);
            }
            after = source_i64(
                source_rows
                    .last()
                    .ok_or(LegacyCopyError::UnsupportedResumeCheckpoint)?,
                "id",
                mapping.source,
            )?;
        }
        let target_count = self.target.table_count("v2_giftcard_redemption").await?;
        if expected_count != target_count {
            return Err(LegacyCopyError::CountMismatch {
                table: "v2_giftcard_redemption".to_string(),
                source_count: expected_count,
                target_count,
            });
        }
        let expected_sha256 = hex::encode(expected_digest.finalize());
        let target_sha256 = hex::encode(target_digest.finalize());
        if expected_sha256 != target_sha256 {
            return Err(LegacyCopyError::ValueDigestMismatch {
                table: "v2_giftcard_redemption".to_string(),
            });
        }
        Ok(DerivedVerification {
            target: "v2_giftcard_redemption".to_string(),
            expected_count,
            target_count,
            expected_sha256,
            target_sha256,
        })
    }

    pub async fn verify_node_credentials(&self) -> Result<DerivedVerification, LegacyCopyError> {
        self.target.verify_node_credentials().await
    }

    pub async fn reset_sequences(&self) -> Result<(), LegacyCopyError> {
        self.target.reset_sequences().await
    }

    async fn verify_all_base_tables(
        &mut self,
        traffic: &TrafficFoldPlan,
    ) -> Result<Vec<TableVerification>, LegacyCopyError> {
        let mut reports = Vec::with_capacity(TABLE_MAPPINGS.len());
        for mapping in TABLE_MAPPINGS {
            reports.push(self.verify_base_table(mapping, traffic).await?);
        }
        Ok(reports)
    }

    async fn verify_all(
        &mut self,
        traffic: &TrafficFoldPlan,
    ) -> Result<LegacyCopyVerification, LegacyCopyError> {
        let base_tables = self.verify_all_base_tables(traffic).await?;
        let derived_tables = vec![
            self.verify_giftcard_redemptions().await?,
            self.verify_node_credentials().await?,
        ];
        let traffic_fold = self.target.verify_frozen_traffic(traffic).await?;
        Ok(LegacyCopyVerification {
            base_tables,
            derived_tables,
            traffic_fold,
        })
    }

    async fn verify_base_table(
        &mut self,
        mapping: &TableMapping,
        traffic: &TrafficFoldPlan,
    ) -> Result<TableVerification, LegacyCopyError> {
        let columns = full_target_columns(mapping);
        let mut after = INITIAL_SOURCE_ID_CURSOR;
        let mut source_count = 0_u64;
        let mut source_rows_digest = Sha256::new();
        let mut target_rows_digest = Sha256::new();
        let mut source_pk_digest = Sha256::new();
        let mut target_pk_digest = Sha256::new();
        source_rows_digest.update(FULL_ROW_DOMAIN);
        target_rows_digest.update(FULL_ROW_DOMAIN);
        source_pk_digest.update(PRIMARY_KEY_DOMAIN);
        target_pk_digest.update(PRIMARY_KEY_DOMAIN);
        loop {
            let source_rows = self
                .source
                .read_batch(mapping, after, self.batch_size)
                .await?;
            if source_rows.is_empty() {
                break;
            }
            let mut expected = source_rows
                .iter()
                .map(|row| expected_full_row(mapping, row))
                .collect::<Result<Vec<_>, _>>()?;
            overlay_frozen_traffic(mapping, &mut expected, traffic)?;
            let mut transaction = self
                .target
                .pool
                .begin()
                .await
                .map_err(LegacyCopyError::Target)?;
            let stored = self
                .target
                .fetch_rows(&mut transaction, mapping.target, &columns, &expected)
                .await?;
            transaction
                .rollback()
                .await
                .map_err(LegacyCopyError::Target)?;
            compare_expected_rows(mapping.target, &expected, &stored)?;
            digest_rows(&mut source_rows_digest, &expected)?;
            digest_rows(&mut target_rows_digest, &stored)?;
            digest_primary_keys(&mut source_pk_digest, mapping.target, &expected)?;
            digest_primary_keys(&mut target_pk_digest, mapping.target, &stored)?;
            source_count = source_count.saturating_add(expected.len() as u64);
            after = source_i64(
                source_rows
                    .last()
                    .ok_or(LegacyCopyError::TargetPrimaryKeyMismatch {
                        table: mapping.target.to_string(),
                    })?,
                "id",
                mapping.source,
            )?;
        }
        let source_count_query = self.source.table_count(mapping).await?;
        let target_count = self.target.table_count(mapping.target).await?;
        if source_count != source_count_query || source_count != target_count {
            return Err(LegacyCopyError::CountMismatch {
                table: mapping.target.to_string(),
                source_count: source_count_query,
                target_count,
            });
        }
        let source_primary_key_sha256 = hex::encode(source_pk_digest.finalize());
        let target_primary_key_sha256 = hex::encode(target_pk_digest.finalize());
        if source_primary_key_sha256 != target_primary_key_sha256 {
            return Err(LegacyCopyError::PrimaryKeyDigestMismatch {
                table: mapping.target.to_string(),
            });
        }
        let source_canonical_sha256 = hex::encode(source_rows_digest.finalize());
        let target_canonical_sha256 = hex::encode(target_rows_digest.finalize());
        if source_canonical_sha256 != target_canonical_sha256 {
            return Err(LegacyCopyError::ValueDigestMismatch {
                table: mapping.target.to_string(),
            });
        }
        Ok(TableVerification {
            table: mapping.target.to_string(),
            source_count,
            target_count,
            source_primary_key_sha256,
            target_primary_key_sha256,
            source_canonical_sha256,
            target_canonical_sha256,
        })
    }

    pub async fn finish_source_snapshot(self) -> Result<(), LegacyCopyError> {
        self.source.commit().await
    }
}

impl PostgresCopyTarget<'_> {
    async fn existing_user_ids(
        &self,
        candidates: &BTreeSet<u64>,
    ) -> Result<BTreeSet<u64>, LegacyCopyError> {
        if candidates.is_empty() {
            return Ok(BTreeSet::new());
        }
        let ids = candidates
            .iter()
            .map(|id| i64::try_from(*id).map_err(|_| LegacyCopyError::GiftcardIdRange))
            .collect::<Result<Vec<_>, _>>()?;
        let rows =
            sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE id = ANY($1) ORDER BY id")
                .bind(&ids)
                .fetch_all(self.pool)
                .await
                .map_err(LegacyCopyError::Target)?;
        rows.into_iter()
            .map(|id| u64::try_from(id).map_err(|_| LegacyCopyError::GiftcardIdRange))
            .collect()
    }

    async fn giftcard_redemptions_for(
        &self,
        giftcard_id: i32,
    ) -> Result<Vec<LegacyGiftcardRedemptionRow>, LegacyCopyError> {
        sqlx::query_as::<_, (i32, i64, i64, String)>(
            "SELECT giftcard_id, user_id, created_at, created_at_provenance \
             FROM v2_giftcard_redemption \
             WHERE giftcard_id = $1 ORDER BY user_id",
        )
        .bind(giftcard_id)
        .fetch_all(self.pool)
        .await
        .map_err(LegacyCopyError::Target)
        .map(|rows| {
            rows.into_iter()
                .map(
                    |(giftcard_id, user_id, created_at, created_at_provenance)| {
                        LegacyGiftcardRedemptionRow {
                            giftcard_id,
                            user_id,
                            created_at,
                            created_at_provenance,
                        }
                    },
                )
                .collect()
        })
    }

    async fn verify_node_credentials(&self) -> Result<DerivedVerification, LegacyCopyError> {
        let union = NODE_CREDENTIAL_SOURCES
            .iter()
            .map(|(node_type, table)| {
                format!(
                    "SELECT '{node_type}'::text AS node_type, id AS node_id, \
                     0::bigint AS credential_epoch, updated_at FROM {table}"
                )
            })
            .collect::<Vec<_>>()
            .join(" UNION ALL ");
        let expected_sql = format!(
            "SELECT node_type, node_id, credential_epoch, updated_at \
             FROM ({union}) expected ORDER BY node_type, node_id"
        );
        let expected =
            sqlx::query_as::<_, (String, i32, i64, i64)>(sqlx::AssertSqlSafe(expected_sql))
                .fetch_all(self.pool)
                .await
                .map_err(LegacyCopyError::Target)?;
        let stored = sqlx::query_as::<_, (String, i32, i64, i64)>(
            "SELECT node_type, node_id, credential_epoch, updated_at \
             FROM v2_server_credential ORDER BY node_type, node_id",
        )
        .fetch_all(self.pool)
        .await
        .map_err(LegacyCopyError::Target)?;
        if expected != stored {
            return Err(LegacyCopyError::NodeCredentialMismatch);
        }
        let mut expected_digest = Sha256::new();
        let mut target_digest = Sha256::new();
        expected_digest.update(FULL_ROW_DOMAIN);
        target_digest.update(FULL_ROW_DOMAIN);
        for row in &expected {
            digest_serializable(&mut expected_digest, row)?;
        }
        for row in &stored {
            digest_serializable(&mut target_digest, row)?;
        }
        let expected_sha256 = hex::encode(expected_digest.finalize());
        let target_sha256 = hex::encode(target_digest.finalize());
        if expected_sha256 != target_sha256 {
            return Err(LegacyCopyError::NodeCredentialMismatch);
        }
        Ok(DerivedVerification {
            target: "v2_server_credential".to_string(),
            expected_count: expected.len() as u64,
            target_count: stored.len() as u64,
            expected_sha256,
            target_sha256,
        })
    }
}

fn giftcard_candidate_user_ids(
    used_user_ids: &SourceValue,
) -> Result<BTreeSet<u64>, LegacyCopyError> {
    let SourceValue::Text(text) = used_user_ids else {
        if matches!(used_user_ids, SourceValue::Null) {
            return Ok(BTreeSet::new());
        }
        return Err(LegacyCopyError::InvalidSourceJson {
            table: "v2_giftcard".to_string(),
            message: "used_user_ids is not text or NULL".to_string(),
        });
    };
    let value = serde_json::from_str::<Value>(text).map_err(|error| {
        LegacyCopyError::InvalidSourceJson {
            table: "v2_giftcard".to_string(),
            message: error.to_string(),
        }
    })?;
    let members = value
        .as_array()
        .ok_or_else(|| LegacyCopyError::InvalidSourceJson {
            table: "v2_giftcard".to_string(),
            message: "used_user_ids is not an array".to_string(),
        })?;
    members
        .iter()
        .map(|member| {
            let id = match member {
                Value::Number(number) => number.as_u64(),
                Value::String(value)
                    if !value.is_empty()
                        && (b'1'..=b'9').contains(&value.as_bytes()[0])
                        && value.as_bytes()[1..].iter().all(u8::is_ascii_digit) =>
                {
                    value.parse::<u64>().ok()
                }
                _ => None,
            }
            .filter(|id| *id > 0 && *id <= i64::MAX as u64)
            .ok_or_else(|| LegacyCopyError::InvalidSourceJson {
                table: "v2_giftcard".to_string(),
                message: "used_user_ids has an invalid member".to_string(),
            })?;
            Ok(id)
        })
        .collect()
}

fn overlay_frozen_traffic(
    mapping: &TableMapping,
    rows: &mut [CanonicalRow],
    traffic: &TrafficFoldPlan,
) -> Result<(), LegacyCopyError> {
    if mapping.target != "v2_user" {
        return Ok(());
    }
    for row in rows {
        let user_id = canonical_i64(row, "id", mapping.target)?;
        let Ok(index) = traffic
            .items
            .binary_search_by_key(&user_id, |item| item.user_id)
        else {
            continue;
        };
        let item = &traffic.items[index];
        row.insert("u".to_string(), CanonicalValue::I64(item.after_u));
        row.insert("d".to_string(), CanonicalValue::I64(item.after_d));
        row.insert("t".to_string(), CanonicalValue::I64(item.after_t));
        row.insert(
            "updated_at".to_string(),
            CanonicalValue::I64(item.after_updated_at),
        );
    }
    Ok(())
}

fn parse_uuid(value: &str) -> Result<uuid::Uuid, LegacyCopyError> {
    uuid::Uuid::parse_str(value)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or(LegacyCopyError::TrafficJournalBinding)
}

async fn lock_traffic_fold_gate(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &TrafficFoldPlan,
) -> Result<(), LegacyCopyError> {
    let operation_id = parse_uuid(&plan.binding.operation_id)?;
    let generation = i64::try_from(plan.source_drained.generation)
        .map_err(|_| LegacyCopyError::TrafficJournalBinding)?;
    let row = sqlx::query_as::<_, (String, String, String, String, String, String, i16, bool)>(
        "SELECT operation.installation_id::text, operation.kind, \
         operation.source_fingerprint_sha256, operation.converter_registry_sha256, \
         operation.target_lineage_sha256, operation.state, operation.checkpoint, \
         (EXISTS (SELECT 1 FROM v2_lifecycle_event event \
             WHERE event.operation_id = operation.operation_id \
               AND event.generation = $2 AND event.event_sha256 = $3 \
               AND event.checkpoint = 2 AND event.state = 'running' \
               AND event.outcome_code IS NULL AND event.checkpoint_proof_sha256 = $4) \
          AND EXISTS (SELECT 1 FROM v2_lifecycle_event current_event \
             WHERE current_event.operation_id = operation.operation_id \
               AND current_event.generation = operation.journal_generation \
               AND current_event.event_sha256 = operation.journal_event_sha256 \
               AND current_event.checkpoint = operation.checkpoint \
               AND current_event.state = operation.state \
               AND current_event.outcome_code IS NULL)) \
         FROM v2_lifecycle_operation operation \
         WHERE operation.operation_id = $1 FOR UPDATE OF operation",
    )
    .bind(operation_id)
    .bind(generation)
    .bind(&plan.source_drained.event_sha256)
    .bind(&plan.source_drained.report_sha256)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(LegacyCopyError::Target)?
    .ok_or(LegacyCopyError::TrafficJournalBinding)?;
    if row.0 != plan.binding.target_installation_id
        || row.1 != "legacy_reference_migration"
        || row.2 != plan.binding.source_snapshot_sha256
        || row.3 != plan.binding.registry_sha256
        || row.4 != TARGET_POSTGRES_LINEAGE_SHA256
        || !matches!(row.5.as_str(), "running" | "verifying")
        || !(6..=8).contains(&row.6)
        || !row.7
    {
        return Err(LegacyCopyError::TrafficJournalBinding);
    }
    Ok(())
}

async fn traffic_fold_seal_exists(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &TrafficFoldPlan,
) -> Result<bool, LegacyCopyError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM v2_legacy_traffic_fold WHERE operation_id = $1)",
    )
    .bind(parse_uuid(&plan.binding.operation_id)?)
    .fetch_one(&mut **transaction)
    .await
    .map_err(LegacyCopyError::Target)
}

async fn verify_traffic_fold_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    plan: &TrafficFoldPlan,
) -> Result<TrafficFoldVerification, LegacyCopyError> {
    let operation_id = parse_uuid(&plan.binding.operation_id)?;
    let seal = sqlx::query_as::<_, TrafficFoldSealRow>(
        "SELECT target_installation_id::text AS target_installation_id, \
         source_default_run_id, source_drain_receipt_sha256, \
         source_drained_journal_generation, source_drained_journal_event_sha256, \
         source_drained_report_sha256, fenced_at, upload_fields::text AS upload_fields, \
         download_fields::text AS download_fields, \
         sorted_user_delta_count::text AS sorted_user_delta_count, \
         sorted_user_delta_sha256, upload_delta_sum::text AS upload_delta_sum, \
         download_delta_sum::text AS download_delta_sum, fold_verification_sha256, \
         seal_sha256, applied_at FROM v2_legacy_traffic_fold WHERE operation_id = $1",
    )
    .bind(operation_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(LegacyCopyError::Target)?
    .ok_or(LegacyCopyError::TrafficLedgerConflict)?;
    let generation = i64::try_from(plan.source_drained.generation)
        .map_err(|_| LegacyCopyError::TrafficJournalBinding)?;
    if seal.target_installation_id != plan.binding.target_installation_id
        || seal.source_default_run_id != plan.receipt.source_default_run_id
        || seal.source_drain_receipt_sha256 != plan.receipt.receipt_sha256
        || seal.source_drained_journal_generation != generation
        || seal.source_drained_journal_event_sha256 != plan.source_drained.event_sha256
        || seal.source_drained_report_sha256 != plan.source_drained.report_sha256
        || seal.fenced_at != plan.receipt.fenced_at_unix
        || seal.upload_fields != plan.receipt.upload_fields.to_string()
        || seal.download_fields != plan.receipt.download_fields.to_string()
        || seal.sorted_user_delta_count != plan.receipt.sorted_user_delta_count.to_string()
        || seal.sorted_user_delta_sha256 != plan.receipt.sorted_user_delta_sha256
        || seal.upload_delta_sum != plan.receipt.upload_delta_sum
        || seal.download_delta_sum != plan.receipt.download_delta_sum
        || seal.fold_verification_sha256 != plan.fold_verification_sha256
        || seal.applied_at <= 0
        || seal.seal_sha256 != traffic_fold_seal_sha256(plan, seal.applied_at)?
    {
        return Err(LegacyCopyError::TrafficLedgerConflict);
    }
    let stored_items = sqlx::query_as::<_, TrafficFoldItemRow>(
        "SELECT user_id, upload_delta, download_delta, before_u, before_d, before_t, \
         before_updated_at, after_u, after_d, after_t, after_updated_at, item_sha256 \
         FROM v2_legacy_traffic_fold_item WHERE operation_id = $1 ORDER BY user_id",
    )
    .bind(operation_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(LegacyCopyError::Target)?;
    if stored_items.len() != plan.items.len()
        || stored_items
            .iter()
            .zip(&plan.items)
            .any(|(stored, expected)| {
                *stored
                    != (TrafficFoldItemRow {
                        user_id: expected.user_id,
                        upload_delta: expected.upload_delta,
                        download_delta: expected.download_delta,
                        before_u: expected.before_u,
                        before_d: expected.before_d,
                        before_t: expected.before_t,
                        before_updated_at: expected.before_updated_at,
                        after_u: expected.after_u,
                        after_d: expected.after_d,
                        after_t: expected.after_t,
                        after_updated_at: expected.after_updated_at,
                        item_sha256: expected.item_sha256.clone(),
                    })
            })
    {
        return Err(LegacyCopyError::TrafficLedgerConflict);
    }
    let ids = plan
        .items
        .iter()
        .map(|item| item.user_id)
        .collect::<Vec<_>>();
    let users = if ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT id, u, d, t, updated_at FROM v2_user \
             WHERE id = ANY($1) ORDER BY id FOR SHARE",
        )
        .bind(&ids)
        .fetch_all(&mut **transaction)
        .await
        .map_err(LegacyCopyError::Target)?
    };
    if users.len() != plan.items.len()
        || users.iter().zip(&plan.items).any(|(stored, expected)| {
            stored.0 != expected.user_id
                || stored.1 != expected.after_u
                || stored.2 != expected.after_d
                || stored.3 != expected.after_t
                || stored.4 != expected.after_updated_at
        })
    {
        return Err(LegacyCopyError::TrafficLedgerConflict);
    }
    Ok(TrafficFoldVerification {
        operation_id: plan.binding.operation_id.clone(),
        target_installation_id: plan.binding.target_installation_id.clone(),
        source_default_run_id: plan.receipt.source_default_run_id.clone(),
        source_drain_receipt_sha256: plan.receipt.receipt_sha256.clone(),
        source_drained_journal_generation: plan.source_drained.generation,
        source_drained_journal_event_sha256: plan.source_drained.event_sha256.clone(),
        source_drained_report_sha256: plan.source_drained.report_sha256.clone(),
        fenced_at: plan.receipt.fenced_at_unix,
        upload_fields: plan.receipt.upload_fields,
        download_fields: plan.receipt.download_fields,
        sorted_user_delta_count: plan.receipt.sorted_user_delta_count,
        sorted_user_delta_sha256: plan.receipt.sorted_user_delta_sha256.clone(),
        upload_delta_sum: plan.receipt.upload_delta_sum.clone(),
        download_delta_sum: plan.receipt.download_delta_sum.clone(),
        ledger_item_count: plan.items.len() as u64,
        ledger_items_sha256: plan.items_sha256.clone(),
        fold_verification_sha256: plan.fold_verification_sha256.clone(),
        seal_sha256: seal.seal_sha256,
        applied_at: seal.applied_at,
        applied_exactly_once: true,
    })
}

fn traffic_fold_item_sha256(
    binding: &ConversionRunBinding,
    receipt_sha256: &str,
    item: &TrafficFoldItemPlan,
) -> Result<String, LegacyCopyError> {
    if !is_lower_sha256(receipt_sha256) {
        return Err(LegacyCopyError::TrafficReceiptMismatch);
    }
    let mut digest = Sha256::new();
    digest.update(TRAFFIC_FOLD_ITEM_DOMAIN);
    for field in [
        binding.operation_id.as_bytes(),
        binding.target_installation_id.as_bytes(),
        receipt_sha256.as_bytes(),
        item.user_id.to_string().as_bytes(),
        item.upload_delta.to_string().as_bytes(),
        item.download_delta.to_string().as_bytes(),
        item.before_u.to_string().as_bytes(),
        item.before_d.to_string().as_bytes(),
        item.before_t.to_string().as_bytes(),
        item.before_updated_at.to_string().as_bytes(),
        item.after_u.to_string().as_bytes(),
        item.after_d.to_string().as_bytes(),
        item.after_t.to_string().as_bytes(),
        item.after_updated_at.to_string().as_bytes(),
    ] {
        digest_field(&mut digest, field);
    }
    Ok(hex::encode(digest.finalize()))
}

fn traffic_fold_verification_sha256(plan: &TrafficFoldPlan) -> Result<String, LegacyCopyError> {
    if !is_lower_sha256(&plan.items_sha256) {
        return Err(LegacyCopyError::TrafficLedgerConflict);
    }
    let mut digest = Sha256::new();
    digest.update(TRAFFIC_FOLD_VERIFICATION_DOMAIN);
    let fields = [
        plan.binding.operation_id.clone(),
        plan.binding.target_installation_id.clone(),
        plan.receipt.source_default_run_id.clone(),
        plan.receipt.receipt_sha256.clone(),
        plan.source_drained.generation.to_string(),
        plan.source_drained.event_sha256.clone(),
        plan.source_drained.report_sha256.clone(),
        plan.receipt.fenced_at_unix.to_string(),
        plan.receipt.upload_fields.to_string(),
        plan.receipt.download_fields.to_string(),
        plan.receipt.sorted_user_delta_count.to_string(),
        plan.receipt.sorted_user_delta_sha256.clone(),
        plan.receipt.upload_delta_sum.clone(),
        plan.receipt.download_delta_sum.clone(),
        plan.items.len().to_string(),
        plan.items_sha256.clone(),
    ];
    for field in fields {
        digest_field(&mut digest, field.as_bytes());
    }
    Ok(hex::encode(digest.finalize()))
}

fn traffic_fold_seal_sha256(
    plan: &TrafficFoldPlan,
    applied_at: i64,
) -> Result<String, LegacyCopyError> {
    if applied_at <= 0 || !is_lower_sha256(&plan.fold_verification_sha256) {
        return Err(LegacyCopyError::TrafficLedgerConflict);
    }
    let mut digest = Sha256::new();
    digest.update(TRAFFIC_FOLD_SEAL_DOMAIN);
    for field in [
        plan.binding.operation_id.as_bytes(),
        plan.binding.target_installation_id.as_bytes(),
        plan.fold_verification_sha256.as_bytes(),
        applied_at.to_string().as_bytes(),
    ] {
        digest_field(&mut digest, field);
    }
    Ok(hex::encode(digest.finalize()))
}

fn initial_checkpoint(
    binding: &ConversionRunBinding,
) -> Result<ConversionCheckpoint, LegacyCopyError> {
    let first = TABLE_MAPPINGS
        .first()
        .ok_or_else(|| ConverterError::Registry("empty converter registry".to_string()))?;
    Ok(ConversionCheckpoint {
        binding: binding.clone(),
        phase: ConversionPhase::CopyBaseTables,
        table_order: first.order,
        table: first.source.to_string(),
        last_source_id: INITIAL_SOURCE_ID_CURSOR,
        source_rows_seen: 0,
        target_rows_verified: 0,
        rolling_sha256: initial_rolling_sha256(binding)?,
    })
}

fn phase_name(phase: ConversionPhase) -> &'static str {
    match phase {
        ConversionPhase::CopyBaseTables => "copy_base_tables",
        ConversionPhase::ApplyDeferredReferences => "apply_deferred_references",
        ConversionPhase::BuildDerivedRows => "build_derived_rows",
        ConversionPhase::ResetSequences => "reset_sequences",
        ConversionPhase::FoldFrozenTraffic => "fold_frozen_traffic",
        ConversionPhase::VerifyAllValues => "verify_all_values",
        ConversionPhase::Complete => "complete",
    }
}

fn parse_phase(value: &str) -> Result<ConversionPhase, PostgresCheckpointError> {
    match value {
        "copy_base_tables" => Ok(ConversionPhase::CopyBaseTables),
        "apply_deferred_references" => Ok(ConversionPhase::ApplyDeferredReferences),
        "build_derived_rows" => Ok(ConversionPhase::BuildDerivedRows),
        "reset_sequences" => Ok(ConversionPhase::ResetSequences),
        "fold_frozen_traffic" => Ok(ConversionPhase::FoldFrozenTraffic),
        "verify_all_values" => Ok(ConversionPhase::VerifyAllValues),
        "complete" => Ok(ConversionPhase::Complete),
        _ => Err(PostgresCheckpointError::HashChain),
    }
}

fn decode_checkpoint_chain(
    expected_binding: &ConversionRunBinding,
    rows: Vec<CheckpointRecordRow>,
) -> Result<Vec<(ConversionCheckpoint, String)>, PostgresCheckpointError> {
    let mut decoded = Vec::with_capacity(rows.len());
    let mut previous_sha256: Option<String> = None;
    let mut previous_checkpoint: Option<ConversionCheckpoint> = None;
    for (expected_sequence, row) in rows.into_iter().enumerate() {
        if row.sequence
            != i64::try_from(expected_sequence).map_err(|_| PostgresCheckpointError::ValueRange)?
            || row.previous_checkpoint_sha256 != previous_sha256
            || row.target_installation_id != expected_binding.target_installation_id
            || row.source_snapshot_sha256 != expected_binding.source_snapshot_sha256
            || row.source_schema_sha256 != expected_binding.source_schema_sha256
            || row.registry_sha256 != expected_binding.registry_sha256
            || row.recorded_at <= 0
        {
            return Err(PostgresCheckpointError::HashChain);
        }
        let table_order =
            u16::try_from(row.table_order).map_err(|_| PostgresCheckpointError::ValueRange)?;
        let source_rows_seen = row
            .source_rows_seen
            .parse::<u64>()
            .map_err(|_| PostgresCheckpointError::ValueRange)?;
        let target_rows_verified = row
            .target_rows_verified
            .parse::<u64>()
            .map_err(|_| PostgresCheckpointError::ValueRange)?;
        let checkpoint = ConversionCheckpoint {
            binding: expected_binding.clone(),
            phase: parse_phase(&row.phase)?,
            table_order,
            table: row.table_name,
            last_source_id: row.last_source_id,
            source_rows_seen,
            target_rows_verified,
            rolling_sha256: row.rolling_sha256,
        };
        checkpoint.validate_resume(expected_binding, previous_checkpoint.as_ref())?;
        let computed = checkpoint_record_sha256(
            row.sequence,
            &checkpoint,
            previous_sha256.as_deref(),
            row.recorded_at,
        )?;
        if computed != row.checkpoint_sha256 {
            return Err(PostgresCheckpointError::HashChain);
        }
        previous_sha256 = Some(row.checkpoint_sha256.clone());
        previous_checkpoint = Some(checkpoint.clone());
        decoded.push((checkpoint, row.checkpoint_sha256));
    }
    Ok(decoded)
}

fn decode_latest_checkpoint(
    expected_binding: &ConversionRunBinding,
    row: CheckpointRecordRow,
) -> Result<(ConversionCheckpoint, String, i64), PostgresCheckpointError> {
    if row.sequence < 0
        || row.target_installation_id != expected_binding.target_installation_id
        || row.source_snapshot_sha256 != expected_binding.source_snapshot_sha256
        || row.source_schema_sha256 != expected_binding.source_schema_sha256
        || row.registry_sha256 != expected_binding.registry_sha256
        || row.recorded_at <= 0
        || (row.sequence == 0) != row.previous_checkpoint_sha256.is_none()
    {
        return Err(PostgresCheckpointError::HashChain);
    }
    let checkpoint = ConversionCheckpoint {
        binding: expected_binding.clone(),
        phase: parse_phase(&row.phase)?,
        table_order: u16::try_from(row.table_order)
            .map_err(|_| PostgresCheckpointError::ValueRange)?,
        table: row.table_name,
        last_source_id: row.last_source_id,
        source_rows_seen: row
            .source_rows_seen
            .parse::<u64>()
            .map_err(|_| PostgresCheckpointError::ValueRange)?,
        target_rows_verified: row
            .target_rows_verified
            .parse::<u64>()
            .map_err(|_| PostgresCheckpointError::ValueRange)?,
        rolling_sha256: row.rolling_sha256,
    };
    checkpoint.validate_resume(expected_binding, None)?;
    let computed = checkpoint_record_sha256(
        row.sequence,
        &checkpoint,
        row.previous_checkpoint_sha256.as_deref(),
        row.recorded_at,
    )?;
    if computed != row.checkpoint_sha256 {
        return Err(PostgresCheckpointError::HashChain);
    }
    Ok((checkpoint, row.checkpoint_sha256, row.sequence))
}

fn checkpoint_record_sha256(
    sequence: i64,
    checkpoint: &ConversionCheckpoint,
    previous_checkpoint_sha256: Option<&str>,
    recorded_at: i64,
) -> Result<String, PostgresCheckpointError> {
    checkpoint.binding.validate()?;
    if sequence < 0
        || recorded_at <= 0
        || previous_checkpoint_sha256.is_some_and(|value| !is_lower_sha256(value))
        || !is_lower_sha256(&checkpoint.rolling_sha256)
    {
        return Err(PostgresCheckpointError::HashChain);
    }
    let mut digest = Sha256::new();
    digest.update(CHECKPOINT_RECORD_DOMAIN);
    digest_field(&mut digest, checkpoint.binding.operation_id.as_bytes());
    digest_field(&mut digest, sequence.to_string().as_bytes());
    digest_field(
        &mut digest,
        checkpoint.binding.target_installation_id.as_bytes(),
    );
    digest_field(
        &mut digest,
        checkpoint.binding.source_snapshot_sha256.as_bytes(),
    );
    digest_field(
        &mut digest,
        checkpoint.binding.source_schema_sha256.as_bytes(),
    );
    digest_field(&mut digest, checkpoint.binding.registry_sha256.as_bytes());
    digest_field(&mut digest, phase_name(checkpoint.phase).as_bytes());
    digest_field(&mut digest, checkpoint.table_order.to_string().as_bytes());
    digest_field(&mut digest, checkpoint.table.as_bytes());
    digest_field(
        &mut digest,
        checkpoint.last_source_id.to_string().as_bytes(),
    );
    digest_field(
        &mut digest,
        checkpoint.source_rows_seen.to_string().as_bytes(),
    );
    digest_field(
        &mut digest,
        checkpoint.target_rows_verified.to_string().as_bytes(),
    );
    digest_field(&mut digest, checkpoint.rolling_sha256.as_bytes());
    digest_field(
        &mut digest,
        previous_checkpoint_sha256.unwrap_or("").as_bytes(),
    );
    digest_field(&mut digest, recorded_at.to_string().as_bytes());
    Ok(hex::encode(digest.finalize()))
}

fn initial_rolling_sha256(binding: &ConversionRunBinding) -> Result<String, LegacyCopyError> {
    let mut digest = Sha256::new();
    digest.update(COPY_ROLLING_DOMAIN);
    digest_field(&mut digest, binding.operation_id.as_bytes());
    digest_field(&mut digest, binding.target_installation_id.as_bytes());
    digest_field(&mut digest, binding.source_snapshot_sha256.as_bytes());
    digest_field(&mut digest, registry_sha256()?.as_bytes());
    Ok(hex::encode(digest.finalize()))
}

fn advance_rolling_sha256(
    previous: &str,
    table: &str,
    last_id: i64,
    batch_sha256: &str,
) -> Result<String, LegacyCopyError> {
    if !is_lower_sha256(previous) || !is_lower_sha256(batch_sha256) {
        return Err(ConverterError::CheckpointBindingMismatch.into());
    }
    let mut digest = Sha256::new();
    digest.update(COPY_ROLLING_DOMAIN);
    digest_field(&mut digest, previous.as_bytes());
    digest_field(&mut digest, table.as_bytes());
    digest_field(&mut digest, last_id.to_string().as_bytes());
    digest_field(&mut digest, batch_sha256.as_bytes());
    Ok(hex::encode(digest.finalize()))
}

fn digest_rows(digest: &mut Sha256, rows: &[CanonicalRow]) -> Result<(), LegacyCopyError> {
    for row in rows {
        let encoded =
            serde_json::to_vec(row).map_err(|error| LegacyCopyError::InvalidSourceJson {
                table: "canonical_verification".to_string(),
                message: error.to_string(),
            })?;
        digest_field(digest, &encoded);
    }
    Ok(())
}

fn digest_serializable(digest: &mut Sha256, value: &impl Serialize) -> Result<(), LegacyCopyError> {
    let encoded =
        serde_json::to_vec(value).map_err(|error| LegacyCopyError::InvalidSourceJson {
            table: "canonical_verification".to_string(),
            message: error.to_string(),
        })?;
    digest_field(digest, &encoded);
    Ok(())
}

/// Canonical cp8 report hash. It covers all 27 base tables, both derived
/// tables, and the independently sealed Redis traffic fold.
pub fn legacy_copy_verification_sha256(
    verification: &LegacyCopyVerification,
) -> Result<String, LegacyCopyError> {
    let encoded =
        serde_json::to_vec(verification).map_err(|error| LegacyCopyError::InvalidSourceJson {
            table: "copy_verification".to_string(),
            message: error.to_string(),
        })?;
    let mut digest = Sha256::new();
    digest.update(FULL_ROW_DOMAIN);
    digest_field(&mut digest, &encoded);
    Ok(hex::encode(digest.finalize()))
}

fn digest_primary_keys(
    digest: &mut Sha256,
    table: &str,
    rows: &[CanonicalRow],
) -> Result<(), LegacyCopyError> {
    for row in rows {
        digest_field(
            digest,
            canonical_i64(row, "id", table)?.to_string().as_bytes(),
        );
    }
    Ok(())
}

fn digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_journal::{ApplyJournal, ApplyJournalBinding, DurableTargetMutationPermit};
    use uuid::Uuid;

    #[test]
    fn archive_redis_identity_and_keyspace_info_are_strict() {
        let identity =
            redis_url_identity("rediss://user:secret@[::1]:6380/12").expect("Redis identity");
        assert_eq!(
            identity,
            RedisUrlIdentity {
                endpoint: ("rediss".into(), "::1".into(), 6380),
                database: 12,
            }
        );
        assert_eq!(
            parse_redis_keyspace_counts(
                "# Keyspace\r\ndb0:keys=2,expires=0,avg_ttl=0\r\ndb12:keys=0,expires=0\r\n"
            )
            .expect("keyspace")
            .into_iter()
            .collect::<Vec<_>>(),
            vec![(0, 2), (12, 0)]
        );
        assert!(parse_redis_keyspace_counts("db0:expires=0").is_err());
        assert!(parse_redis_keyspace_counts("db0:keys=1\ndb0:keys=1").is_err());
        assert!(redis_url_identity("redis://127.0.0.1/not-a-db").is_err());
        assert!(only_selected_redis_database_has_keys(
            &BTreeMap::from([(0, 2), (12, 0)]),
            0,
            2
        ));
        assert!(!only_selected_redis_database_has_keys(
            &BTreeMap::from([(0, 2), (12, 1)]),
            0,
            2
        ));
    }

    fn binding() -> ConversionRunBinding {
        ConversionRunBinding {
            operation_id: uuid::Uuid::from_u128(1).to_string(),
            target_installation_id: uuid::Uuid::from_u128(2).to_string(),
            source_snapshot_sha256: "a".repeat(64),
            source_schema_sha256: crate::legacy_converter::LEGACY_SEMANTIC_SCHEMA_SHA256
                .to_string(),
            registry_sha256: registry_sha256().expect("registry"),
        }
    }

    fn clickhouse_projection_permit(
        binding: &ConversionRunBinding,
        verification: &LegacyCopyVerification,
    ) -> DurableTargetMutationPermit {
        let root = std::env::temp_dir().join(format!(
            "v2board-legacy-clickhouse-journal-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let journal_binding = ApplyJournalBinding::new(&binding.operation_id, "1".repeat(64))
            .expect("ClickHouse test journal binding");
        let (journal, pending) =
            ApplyJournal::create_pending(&root, journal_binding).expect("create test journal");
        let mut current = journal.begin(&pending).expect("begin test journal");
        current = journal
            .checkpoint_with_proof(&current, ApplyCheckpoint::MaintenanceFenced, "2".repeat(64))
            .expect("maintenance fenced");
        current = journal
            .checkpoint_with_proof(&current, ApplyCheckpoint::SourceDrained, "3".repeat(64))
            .expect("source drained");
        current = journal
            .record_backup_restore_verified(&current, "4".repeat(64), "5".repeat(64))
            .expect("backup verified");
        current = journal
            .record_final_recheck_passed(&current, "6".repeat(64), &binding.source_snapshot_sha256)
            .expect("final recheck");
        current = journal
            .reserve_installation_identity(&current, &binding.target_installation_id)
            .expect("installation identity");
        current = journal
            .checkpoint_with_proof(
                &current,
                ApplyCheckpoint::TargetsBootstrapped,
                "7".repeat(64),
            )
            .expect("targets bootstrapped");
        current = journal
            .checkpoint_with_proof(
                &current,
                ApplyCheckpoint::PostgresBulkCopied,
                "8".repeat(64),
            )
            .expect("PostgreSQL copied");
        current = journal
            .enter_verification(&current)
            .expect("enter verification");
        let verification_sha256 =
            legacy_copy_verification_sha256(verification).expect("verification digest");
        current = journal
            .checkpoint_with_proof(
                &current,
                ApplyCheckpoint::PostgresValueVerified,
                verification_sha256,
            )
            .expect("PostgreSQL value verified");
        let permit = journal
            .target_mutation_permit(&current)
            .expect("ClickHouse target mutation permit");
        fs::remove_dir_all(root).expect("remove ClickHouse test journal");
        permit
    }

    #[test]
    fn source_sql_casts_only_exact_decimals_and_never_float() {
        let payment = mapping_for_source("v2_payment").expect("payment");
        let sql = source_json_batch_sql(payment).expect("source SQL");
        assert!(sql.contains("CAST(`handling_fee_percent` AS CHAR)"));
        assert!(sql.contains("CAST(JSON_OBJECT("));
        assert!(!sql.to_ascii_lowercase().contains("float"));
        assert!(!sql.contains("WITH CONSISTENT SNAPSHOT"));
        assert!(sql.contains("WHERE `id` > ? ORDER BY `id` ASC LIMIT ?"));
    }

    #[test]
    fn source_json_keeps_large_integers_and_text_distinct() {
        let user = mapping_for_source("v2_user").expect("user");
        let mut object = serde_json::Map::new();
        for column in source_columns(user) {
            object.insert(column.to_string(), Value::Null);
        }
        object.insert(
            "id".to_string(),
            serde_json::json!(9_007_199_254_740_993_i64),
        );
        object.insert("email".to_string(), Value::String("0001".to_string()));
        object.insert("telegram_id".to_string(), serde_json::json!(u64::MAX));
        object.insert("remarks".to_string(), Value::String("雪\0é".to_string()));
        let payload = Value::Object(object).to_string();
        let row =
            decode_source_json_row(user, 9_007_199_254_740_993, &payload).expect("source row");
        assert_eq!(
            row.get("id"),
            Some(&SourceValue::I64(9_007_199_254_740_993))
        );
        assert_eq!(
            row.get("email"),
            Some(&SourceValue::Text("0001".to_string()))
        );
        assert_eq!(row.get("telegram_id"), Some(&SourceValue::U64(u64::MAX)));
        assert_eq!(
            row.get("remarks"),
            Some(&SourceValue::Text("雪\0é".to_string()))
        );
    }

    #[test]
    fn bind_conversion_is_exact_and_typed() {
        assert_eq!(
            canonical_bind_text(
                "v2_stat_user",
                "server_rate",
                &CanonicalValue::Decimal("9007199254740993.25".to_string()),
                PgColumnKind::Numeric,
            )
            .expect("decimal"),
            Some("9007199254740993.25".to_string())
        );
        assert!(
            canonical_bind_text(
                "v2_stat_user",
                "server_rate",
                &CanonicalValue::Text("1.0".to_string()),
                PgColumnKind::Numeric,
            )
            .is_err()
        );
        assert!(
            canonical_bind_text(
                "v2_user",
                "telegram_id",
                &CanonicalValue::U64(u64::MAX),
                PgColumnKind::BigInt,
            )
            .is_err()
        );
        assert_eq!(
            decode_target_value(
                "v2_stat_user",
                "server_rate",
                &Value::String("9007199254740993.25".to_string()),
                PgColumnKind::Numeric,
            )
            .expect("target decimal"),
            CanonicalValue::Decimal("9007199254740993.25".to_string())
        );
        assert!(
            canonical_bind_text(
                "v2_user",
                "remarks",
                &CanonicalValue::Text("雪\0é".to_string()),
                PgColumnKind::Text,
            )
            .is_err()
        );
    }

    #[test]
    fn generated_columns_are_derived_from_preserved_values() {
        let order = mapping_for_source("v2_order").expect("order");
        let mut source = SourceRow::new();
        for column in source_columns(order) {
            source.insert(column.to_string(), SourceValue::Null);
        }
        for (column, value) in [
            ("id", 1),
            ("invite_user_id", 0),
            ("user_id", 7),
            ("plan_id", 0),
            ("type", 9),
            ("total_amount", 100),
            ("status", 0),
            ("commission_status", 0),
            ("commission_balance", 0),
            ("created_at", 10),
            ("updated_at", 10),
        ] {
            source.insert(column.to_string(), SourceValue::I64(value));
        }
        for (column, value) in [("period", "deposit"), ("trade_no", "trade")] {
            source.insert(column.to_string(), SourceValue::Text(value.to_string()));
        }
        let row = expected_full_row(order, &source).expect("full row");
        assert_eq!(row.get("referenced_plan_id"), Some(&CanonicalValue::Null));
        assert_eq!(row.get("unfinished_user_id"), Some(&CanonicalValue::I64(7)));
    }

    #[test]
    fn checkpoint_hash_is_bound_and_deterministic() {
        let binding = binding();
        let initial = initial_rolling_sha256(&binding).expect("initial");
        let next = advance_rolling_sha256(&initial, "v2_user", 9, &"b".repeat(64)).expect("next");
        assert_ne!(initial, next);
        assert_eq!(next.len(), 64);
        assert_eq!(
            next,
            advance_rolling_sha256(&initial, "v2_user", 9, &"b".repeat(64)).expect("same")
        );
    }

    #[test]
    fn postgres_checkpoint_record_hash_binds_previous_head_and_every_value() {
        let binding = binding();
        let checkpoint = initial_checkpoint(&binding).expect("checkpoint");
        let first =
            checkpoint_record_sha256(0, &checkpoint, None, 1_000).expect("first checkpoint hash");
        let second = checkpoint_record_sha256(1, &checkpoint, Some(&first), 1_001)
            .expect("second checkpoint hash");
        assert_ne!(first, second);

        let row = CheckpointRecordRow {
            sequence: 0,
            target_installation_id: binding.target_installation_id.clone(),
            source_snapshot_sha256: binding.source_snapshot_sha256.clone(),
            source_schema_sha256: binding.source_schema_sha256.clone(),
            registry_sha256: binding.registry_sha256.clone(),
            phase: phase_name(checkpoint.phase).to_string(),
            table_order: i32::from(checkpoint.table_order),
            table_name: checkpoint.table.clone(),
            last_source_id: checkpoint.last_source_id,
            source_rows_seen: checkpoint.source_rows_seen.to_string(),
            target_rows_verified: checkpoint.target_rows_verified.to_string(),
            rolling_sha256: checkpoint.rolling_sha256.clone(),
            previous_checkpoint_sha256: None,
            checkpoint_sha256: first.clone(),
            recorded_at: 1_000,
        };
        let decoded = decode_checkpoint_chain(&binding, vec![row]).expect("chain");
        assert_eq!(decoded, vec![(checkpoint, first)]);
    }

    #[derive(Default)]
    struct MemorySink {
        head: Option<ConversionCheckpoint>,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("checkpoint CAS mismatch")]
    struct MemorySinkError;

    impl DurableCopyCheckpointSink for MemorySink {
        type Error = MemorySinkError;

        async fn load(
            &mut self,
            _binding: &ConversionRunBinding,
        ) -> Result<Option<ConversionCheckpoint>, Self::Error> {
            Ok(self.head.clone())
        }

        async fn compare_and_store(
            &mut self,
            previous: Option<&ConversionCheckpoint>,
            next: &ConversionCheckpoint,
        ) -> Result<(), Self::Error> {
            if self.head.as_ref() != previous {
                return Err(MemorySinkError);
            }
            self.head = Some(next.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn checkpoint_sink_contract_uses_compare_and_store() {
        let binding = binding();
        let checkpoint = initial_checkpoint(&binding).expect("checkpoint");
        let mut sink = MemorySink::default();
        sink.compare_and_store(None, &checkpoint)
            .await
            .expect("first CAS");
        assert!(sink.compare_and_store(None, &checkpoint).await.is_err());
        assert_eq!(sink.load(&binding).await.expect("load"), Some(checkpoint));
    }

    fn verified_traffic_batch(
        upload: BTreeMap<i64, i128>,
        download: BTreeMap<i64, i128>,
    ) -> VerifiedFrozenTrafficBatch {
        let mut users = BTreeSet::new();
        users.extend(upload.keys().copied());
        users.extend(download.keys().copied());
        let durable_deltas = users
            .into_iter()
            .map(|user_id| {
                crate::native_legacy_source::FrozenTrafficDeltaRecord(
                    user_id,
                    upload
                        .get(&user_id)
                        .copied()
                        .map(i64::try_from)
                        .transpose()
                        .expect("test upload fits i64"),
                    download
                        .get(&user_id)
                        .copied()
                        .map(i64::try_from)
                        .transpose()
                        .expect("test download fits i64"),
                )
            })
            .collect();
        let (deltas, digest, upload_sum, download_sum) =
            canonical_frozen_traffic_union(&upload, &download).expect("canonical traffic");
        let operation_id = binding().operation_id;
        VerifiedFrozenTrafficBatch {
            receipt: VerifiedFrozenTrafficReceipt {
                operation_id,
                maintenance_fenced_generation: 1,
                maintenance_fenced_event_sha256: "5".repeat(64),
                source_default_run_id: "1".repeat(40),
                frozen_upload_key: "prefix:frozen_upload".to_string(),
                frozen_download_key: "prefix:frozen_download".to_string(),
                fenced_at_unix: 1_700_000_000,
                upload_fields: upload.len() as u64,
                download_fields: download.len() as u64,
                sorted_user_delta_count: deltas.len() as u64,
                sorted_user_delta_sha256: digest,
                upload_delta_sum: upload_sum.to_string(),
                download_delta_sum: download_sum.to_string(),
                deltas: durable_deltas,
                delta_applied_exactly_once: deltas.is_empty(),
                receipt_sha256: "2".repeat(64),
            },
            source_drained: SourceDrainJournalBinding {
                generation: 2,
                event_sha256: "3".repeat(64),
                report_sha256: "4".repeat(64),
            },
            deltas,
        }
    }

    async fn install_converter_lifecycle_fixture(
        pool: &PgPool,
        binding: &ConversionRunBinding,
    ) -> String {
        let operation_id = parse_uuid(&binding.operation_id).expect("operation UUID");
        let installation_id =
            parse_uuid(&binding.target_installation_id).expect("installation UUID");
        let backup_reference = "backup:converter-e2e";
        let backup_reference_sha256 =
            backup_reference_sha256(backup_reference).expect("backup reference digest");
        sqlx::query(
            "INSERT INTO v2_system_installation (\
             singleton, installation_id, lineage, state, created_at, source_fingerprint_sha256) \
             VALUES (1, $1, 'legacy_migrated', 'pending', 1000, $2)",
        )
        .bind(installation_id)
        .bind(&binding.source_snapshot_sha256)
        .execute(pool)
        .await
        .expect("converter installation fixture");
        sqlx::query(
            "INSERT INTO v2_lifecycle_operation (\
             operation_id, installation_id, kind, manifest_binding_hmac_sha256, \
             inspect_review_sha256, authorized_snapshot_report_sha256, \
             authorized_snapshot_report_binding_hmac_sha256, authorization_binding_hmac_sha256, \
             authorization_file_sha256, source_fingerprint_sha256, converter_registry_sha256, \
             target_lineage_sha256, state, checkpoint, journal_generation, \
             journal_event_sha256, checkpoint_proof_sha256, backup_reference, \
             backup_restore_proof_sha256, final_recheck_report_sha256, created_at, updated_at) \
             VALUES ($1, $2, 'legacy_reference_migration', $3, $4, $5, $6, $7, $8, $9, \
             $10, $11, 'running', 6, 6, $12, $13, $14, $15, $16, 1000, 1000)",
        )
        .bind(operation_id)
        .bind(installation_id)
        .bind("1".repeat(64))
        .bind("2".repeat(64))
        .bind("3".repeat(64))
        .bind("4".repeat(64))
        .bind("5".repeat(64))
        .bind("6".repeat(64))
        .bind(&binding.source_snapshot_sha256)
        .bind(&binding.registry_sha256)
        .bind(TARGET_POSTGRES_LINEAGE_SHA256)
        .bind("6".repeat(64))
        .bind("7".repeat(64))
        .bind(backup_reference)
        .bind("8".repeat(64))
        .bind("9".repeat(64))
        .execute(pool)
        .await
        .expect("converter lifecycle operation fixture");
        sqlx::query(
            "INSERT INTO v2_lifecycle_event (\
             operation_id, generation, state, checkpoint, previous_event_sha256, \
             event_sha256, checkpoint_proof_sha256, recorded_at_unix_ms) \
             VALUES ($1, 2, 'running', 2, $2, $3, $4, 1000)",
        )
        .bind(operation_id)
        .bind("0".repeat(64))
        .bind("3".repeat(64))
        .bind("4".repeat(64))
        .execute(pool)
        .await
        .expect("converter SourceDrained fixture event");
        sqlx::query(
            "INSERT INTO v2_lifecycle_event (\
             operation_id, generation, state, checkpoint, previous_event_sha256, event_sha256, \
             checkpoint_proof_sha256, installation_id, backup_restore_proof_sha256, \
             backup_reference_sha256, final_recheck_report_sha256, source_fingerprint_sha256, \
             recorded_at_unix_ms) VALUES (\
             $1, 6, 'running', 6, $2, $3, $4, $5, $6, $7, $8, $9, 1001)",
        )
        .bind(operation_id)
        .bind("5".repeat(64))
        .bind("6".repeat(64))
        .bind("7".repeat(64))
        .bind(installation_id)
        .bind("8".repeat(64))
        .bind(&backup_reference_sha256)
        .bind("9".repeat(64))
        .bind(&binding.source_snapshot_sha256)
        .execute(pool)
        .await
        .expect("converter current fixture event");
        backup_reference_sha256
    }

    #[test]
    fn frozen_traffic_union_is_sorted_and_keeps_upload_only_users() {
        let upload = BTreeMap::from([(7_i64, 11_i128), (10, 13)]);
        let download = BTreeMap::from([(9_i64, 17_i128), (10, 19)]);
        let batch = verified_traffic_batch(upload, download);
        assert_eq!(batch.deltas.len(), 3);
        assert_eq!(batch.deltas[0].user_id, 7);
        assert_eq!(batch.deltas[0].upload, 11);
        assert_eq!(batch.deltas[0].download, 0);
        assert_eq!(batch.deltas[1].user_id, 9);
        assert_eq!(batch.deltas[1].upload, 0);
        assert_eq!(batch.deltas[1].download, 17);
        assert_eq!(batch.deltas[2].user_id, 10);
        assert_eq!(batch.receipt.upload_delta_sum, "24");
        assert_eq!(batch.receipt.download_delta_sum, "36");
    }

    #[test]
    fn absent_frozen_hashes_are_the_canonical_zero_delta_receipt() {
        assert!(!frozen_traffic_hash_requires_scan("none").expect("absent hash"));
        assert!(frozen_traffic_hash_requires_scan("hash").expect("present hash"));
        assert!(frozen_traffic_hash_requires_scan("string").is_err());
        let batch = verified_traffic_batch(BTreeMap::new(), BTreeMap::new());
        assert_eq!(batch.user_count(), 0);
        assert_eq!(batch.receipt.upload_fields, 0);
        assert_eq!(batch.receipt.download_fields, 0);
        assert_eq!(batch.receipt.upload_delta_sum, "0");
        assert_eq!(batch.receipt.download_delta_sum, "0");
        assert!(batch.receipt.delta_applied_exactly_once);
    }

    #[test]
    fn durable_receipt_survives_loss_of_both_frozen_redis_hashes() {
        let expected_upload = BTreeMap::from([(7_i64, 11_i128), (10, 13)]);
        let expected_download = BTreeMap::from([(9_i64, 17_i128), (10, 19)]);
        let batch = verified_traffic_batch(expected_upload.clone(), expected_download.clone());

        let recovered = reconcile_durable_traffic_receipt(
            &batch.receipt,
            false,
            &BTreeMap::new(),
            false,
            &BTreeMap::new(),
        )
        .expect("the HMAC receipt, not volatile Redis, is the durable traffic authority");
        assert_eq!(recovered, batch.deltas);

        let mut tampered_upload = expected_upload;
        tampered_upload.insert(7, 12);
        assert!(matches!(
            reconcile_durable_traffic_receipt(
                &batch.receipt,
                true,
                &tampered_upload,
                true,
                &expected_download,
            ),
            Err(LegacyCopyError::TrafficReceiptMismatch)
        ));
    }

    #[test]
    fn traffic_preflight_rejects_missing_users_and_overflow_before_a_plan_exists() {
        let batch = verified_traffic_batch(BTreeMap::from([(7_i64, 1_i128)]), BTreeMap::new());
        assert!(matches!(
            build_traffic_fold_plan(&binding(), &batch, &BTreeMap::new()),
            Err(LegacyCopyError::TrafficUserMissing)
        ));
        let source = BTreeMap::from([(7_i64, (i64::MAX, 0_i64, 10_i64, 10_i64))]);
        assert!(matches!(
            build_traffic_fold_plan(&binding(), &batch, &source),
            Err(LegacyCopyError::TrafficCounterOverflow)
        ));
        assert!(matches!(
            canonical_frozen_traffic_union(&BTreeMap::from([(7_i64, -1_i128)]), &BTreeMap::new()),
            Err(LegacyCopyError::TrafficValueRange)
        ));
    }

    #[test]
    fn final_user_verification_overlays_the_separate_traffic_fact() {
        let batch = verified_traffic_batch(BTreeMap::from([(7_i64, 11_i128)]), BTreeMap::new());
        let source = BTreeMap::from([(7_i64, (100_i64, 200_i64, 30_i64, 40_i64))]);
        let plan = build_traffic_fold_plan(&binding(), &batch, &source).expect("traffic plan");
        let mut row = CanonicalRow::from([
            ("id".to_string(), CanonicalValue::I64(7)),
            ("u".to_string(), CanonicalValue::I64(100)),
            ("d".to_string(), CanonicalValue::I64(200)),
            ("t".to_string(), CanonicalValue::I64(30)),
            ("updated_at".to_string(), CanonicalValue::I64(40)),
        ]);
        overlay_frozen_traffic(
            mapping_for_source("v2_user").expect("user mapping"),
            std::slice::from_mut(&mut row),
            &plan,
        )
        .expect("traffic overlay");
        assert_eq!(row.get("u"), Some(&CanonicalValue::I64(111)));
        assert_eq!(row.get("d"), Some(&CanonicalValue::I64(200)));
        assert_eq!(row.get("t"), Some(&CanonicalValue::I64(1_700_000_000)));
        assert_eq!(
            row.get("updated_at"),
            Some(&CanonicalValue::I64(1_700_000_000))
        );
    }

    #[test]
    fn cp8_hash_covers_the_independent_traffic_fold_report() {
        let batch = verified_traffic_batch(BTreeMap::from([(7_i64, 1_i128)]), BTreeMap::new());
        let source = BTreeMap::from([(7_i64, (10_i64, 20_i64, 30_i64, 40_i64))]);
        let plan = build_traffic_fold_plan(&binding(), &batch, &source).expect("traffic plan");
        let mut traffic = TrafficFoldVerification {
            operation_id: plan.binding.operation_id.clone(),
            target_installation_id: plan.binding.target_installation_id.clone(),
            source_default_run_id: plan.receipt.source_default_run_id.clone(),
            source_drain_receipt_sha256: plan.receipt.receipt_sha256.clone(),
            source_drained_journal_generation: plan.source_drained.generation,
            source_drained_journal_event_sha256: plan.source_drained.event_sha256.clone(),
            source_drained_report_sha256: plan.source_drained.report_sha256.clone(),
            fenced_at: plan.receipt.fenced_at_unix,
            upload_fields: plan.receipt.upload_fields,
            download_fields: plan.receipt.download_fields,
            sorted_user_delta_count: plan.receipt.sorted_user_delta_count,
            sorted_user_delta_sha256: plan.receipt.sorted_user_delta_sha256.clone(),
            upload_delta_sum: plan.receipt.upload_delta_sum.clone(),
            download_delta_sum: plan.receipt.download_delta_sum.clone(),
            ledger_item_count: 1,
            ledger_items_sha256: plan.items_sha256.clone(),
            fold_verification_sha256: plan.fold_verification_sha256.clone(),
            seal_sha256: "5".repeat(64),
            applied_at: 1_700_000_001_000,
            applied_exactly_once: true,
        };
        let first = legacy_copy_verification_sha256(&LegacyCopyVerification {
            base_tables: Vec::new(),
            derived_tables: Vec::new(),
            traffic_fold: traffic.clone(),
        })
        .expect("first report hash");
        traffic.source_drain_receipt_sha256 = "6".repeat(64);
        let second = legacy_copy_verification_sha256(&LegacyCopyVerification {
            base_tables: Vec::new(),
            derived_tables: Vec::new(),
            traffic_fold: traffic,
        })
        .expect("second report hash");
        assert_ne!(first, second);
    }

    #[test]
    fn formal_mysql_fingerprint_registry_remains_exactly_twenty_seven_tables() {
        assert_eq!(TABLE_MAPPINGS.len(), 27);
        assert!(
            TABLE_MAPPINGS
                .iter()
                .all(|mapping| !mapping.target.starts_with("v2_legacy_traffic_fold"))
        );
        assert!(
            crate::legacy_converter::TARGET_ONLY_TABLES.contains(&"v2_legacy_traffic_fold_item")
        );
        assert!(crate::legacy_converter::TARGET_ONLY_TABLES.contains(&"v2_legacy_traffic_fold"));
    }

    #[tokio::test]
    #[ignore = "requires V2BOARD_LEGACY_FIXTURE_DATABASE_URL pointing at disposable MySQL 8"]
    async fn mysql_json_object_preserves_integer_decimal_unicode_and_nul_fixture() {
        let database_url =
            std::env::var("V2BOARD_LEGACY_FIXTURE_DATABASE_URL").expect("fixture database URL");
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("fixture connection");
        let mut connection = pool.acquire().await.expect("fixture acquire");
        sqlx::query(
            "CREATE TEMPORARY TABLE v2board_copy_fixture (\
                 signed_value BIGINT NOT NULL, \
                 unsigned_value BIGINT UNSIGNED NOT NULL, \
                 exact_decimal DECIMAL(30,10) NOT NULL, \
                 body LONGTEXT CHARACTER SET utf8mb4 NOT NULL\
             )",
        )
        .execute(&mut *connection)
        .await
        .expect("fixture table");
        let body = "雪\0é";
        sqlx::query(
            "INSERT INTO v2board_copy_fixture \
             (signed_value, unsigned_value, exact_decimal, body) \
             VALUES (?, ?, CAST(? AS DECIMAL(30,10)), ?)",
        )
        .bind(9_007_199_254_740_993_i64)
        .bind(u64::MAX)
        .bind("12345678901234567890.1234567890")
        .bind(body)
        .execute(&mut *connection)
        .await
        .expect("fixture insert");
        let payload = sqlx::query_scalar::<_, String>(
            "SELECT CAST(JSON_OBJECT(\
                 'signed', signed_value, \
                 'unsigned', unsigned_value, \
                 'decimal', CAST(exact_decimal AS CHAR), \
                 'body', body\
             ) AS CHAR) FROM v2board_copy_fixture",
        )
        .fetch_one(&mut *connection)
        .await
        .expect("fixture payload");
        let value = serde_json::from_str::<Value>(&payload).expect("fixture JSON");
        assert_eq!(
            value.get("signed").and_then(Value::as_i64),
            Some(9_007_199_254_740_993)
        );
        assert_eq!(
            value.get("unsigned").and_then(Value::as_u64),
            Some(u64::MAX)
        );
        assert_eq!(
            value.get("decimal").and_then(Value::as_str),
            Some("12345678901234567890.1234567890")
        );
        assert_eq!(value.get("body").and_then(Value::as_str), Some(body));
    }

    #[tokio::test]
    #[ignore = "requires seeded disposable MySQL 8 plus empty PostgreSQL 18 and ClickHouse fixture databases"]
    async fn all_legacy_tables_copy_to_postgres_project_clickhouse_and_retry_exactly() {
        let mysql_url = std::env::var("V2BOARD_LEGACY_CONVERTER_MYSQL_URL")
            .expect("V2BOARD_LEGACY_CONVERTER_MYSQL_URL");
        let postgres_url = std::env::var("V2BOARD_LEGACY_CONVERTER_POSTGRES_URL")
            .expect("V2BOARD_LEGACY_CONVERTER_POSTGRES_URL");
        let clickhouse_endpoint = std::env::var("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_URL")
            .expect("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_URL");
        let clickhouse_database = std::env::var("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_DATABASE")
            .expect("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_DATABASE");
        let clickhouse_username = std::env::var("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_USERNAME")
            .expect("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_USERNAME");
        let clickhouse_password = std::env::var("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_PASSWORD")
            .expect("V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_PASSWORD");
        let source = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(2)
            .connect(&mysql_url)
            .await
            .expect("seeded MySQL source");
        let target = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&postgres_url)
            .await
            .expect("empty PostgreSQL target");
        sqlx::migrate!("../../migrations-postgres")
            .run(&target)
            .await
            .expect("apply PostgreSQL converter baseline");

        let (_, source_fingerprint) = fingerprint_legacy_source(&source, 2)
            .await
            .expect("fingerprint all seeded source rows");
        assert_eq!(source_fingerprint.table_count, TABLE_MAPPINGS.len());
        assert_eq!(source_fingerprint.row_count, 28);
        let mut conversion_binding = binding();
        conversion_binding.source_snapshot_sha256 = source_fingerprint.canonical_sha256;
        let backup_reference_sha256 =
            install_converter_lifecycle_fixture(&target, &conversion_binding).await;
        let traffic = verified_traffic_batch(BTreeMap::new(), BTreeMap::new());

        let mut first_sink =
            PostgresDurableCopyCheckpointSink::new(&target, backup_reference_sha256.clone())
                .expect("durable PostgreSQL checkpoint sink");
        let mut first = LegacyCopyAdapter::new(&source, &target, conversion_binding.clone(), 1)
            .await
            .expect("first real converter adapter");
        let (first_checkpoint, first_verification) = first
            .execute(&mut first_sink, &traffic)
            .await
            .expect("copy every seeded legacy table");
        assert_eq!(first_checkpoint.phase, ConversionPhase::Complete);
        assert_eq!(first_verification.base_tables.len(), TABLE_MAPPINGS.len());
        assert!(first_verification.base_tables.iter().all(|table| {
            table.source_count > 0
                && table.source_count == table.target_count
                && table.source_primary_key_sha256 == table.target_primary_key_sha256
                && table.source_canonical_sha256 == table.target_canonical_sha256
        }));
        assert_eq!(
            first_verification
                .derived_tables
                .iter()
                .map(|table| (table.target.as_str(), table.target_count))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([("v2_giftcard_redemption", 2), ("v2_server_credential", 8)])
        );
        assert!(first_verification.traffic_fold.applied_exactly_once);
        first
            .finish_source_snapshot()
            .await
            .expect("finish first source snapshot");

        let users = sqlx::query_as::<_, (i64, Option<i64>, String, String)>(
            "SELECT id, invite_user_id, token, remarks FROM v2_user ORDER BY id",
        )
        .fetch_all(&target)
        .await
        .expect("read converted users");
        assert_eq!(users.len(), 2);
        assert_eq!(users[1].1, Some(1));
        assert_eq!(users[0].2, "00000000000000000000000000000001");
        assert_eq!(users[0].3, "主用户 ☃");
        let payment_config: serde_json::Value =
            sqlx::query_scalar("SELECT config FROM v2_payment WHERE id = 1")
                .fetch_one(&target)
                .await
                .expect("read converted payment JSON");
        assert_eq!(
            payment_config,
            serde_json::json!({"merchant": "legacy", "enabled": true})
        );
        let coupon_plans: serde_json::Value =
            sqlx::query_scalar("SELECT limit_plan_ids FROM v2_coupon WHERE id = 1")
                .fetch_one(&target)
                .await
                .expect("read normalized coupon ids");
        assert_eq!(coupon_plans, serde_json::json!([1]));
        let redemptions = sqlx::query_as::<_, (i32, i64)>(
            "SELECT giftcard_id, user_id FROM v2_giftcard_redemption ORDER BY user_id",
        )
        .fetch_all(&target)
        .await
        .expect("read derived gift-card redemptions");
        assert_eq!(redemptions, vec![(1, 1), (1, 2)]);
        let checkpoint_rows_before: i64 =
            sqlx::query_scalar("SELECT count(*) FROM v2_legacy_copy_checkpoint")
                .fetch_one(&target)
                .await
                .expect("count durable converter checkpoints");
        assert!(checkpoint_rows_before > i64::from(TABLE_MAPPINGS.len() as u32));

        let mut retry_sink =
            PostgresDurableCopyCheckpointSink::new(&target, backup_reference_sha256)
                .expect("reconstructed durable checkpoint sink");
        let mut retry = LegacyCopyAdapter::new(&source, &target, conversion_binding.clone(), 3)
            .await
            .expect("reconstructed real converter adapter");
        let (retry_checkpoint, retry_verification) = retry
            .execute(&mut retry_sink, &traffic)
            .await
            .expect("retry reconciles without duplicate data");
        assert_eq!(retry_checkpoint, first_checkpoint);
        assert_eq!(retry_verification, first_verification);
        retry
            .finish_source_snapshot()
            .await
            .expect("finish retry source snapshot");
        let checkpoint_rows_after: i64 =
            sqlx::query_scalar("SELECT count(*) FROM v2_legacy_copy_checkpoint")
                .fetch_one(&target)
                .await
                .expect("recount durable converter checkpoints");
        assert_eq!(checkpoint_rows_after, checkpoint_rows_before);

        let mut spec = crate::manifest::tests::legacy_spec_for_orchestration_operation(
            &conversion_binding.operation_id,
        );
        let ProvisionFlow::LegacyReferenceMigration {
            target: target_spec,
            ..
        } = &mut spec.flow
        else {
            panic!("legacy integration spec");
        };
        target_spec.postgres.migration_database_url = postgres_url.clone();
        target_spec.clickhouse.endpoint = clickhouse_endpoint;
        target_spec.clickhouse.database = clickhouse_database;
        target_spec.clickhouse.schema_principal = serde_json::from_value(serde_json::json!({
            "username": clickhouse_username,
            "password": clickhouse_password,
        }))
        .expect("ClickHouse schema principal");
        let permit = clickhouse_projection_permit(&conversion_binding, &first_verification);
        let projection = crate::legacy_clickhouse::project_legacy_clickhouse(
            &spec,
            &permit,
            &first_verification,
        )
        .await
        .expect("project the empty native ClickHouse epoch");
        assert_eq!(projection.report().legacy_statistics.len(), 3);
        assert!(
            projection
                .report()
                .legacy_statistics
                .iter()
                .all(|proof| proof.row_count == 1)
        );
        assert!(
            projection
                .report()
                .legacy_daily_statistics_preserved_in_postgres
        );
        assert!(
            !projection
                .report()
                .historical_clickhouse_raw_events_synthesized
        );
        assert!(
            projection
                .report()
                .clickhouse_native_event_epoch_starts_empty
        );
        let readback = crate::legacy_clickhouse::verify_legacy_clickhouse_projection_read_only(
            &spec,
            &conversion_binding.target_installation_id,
            projection.report_sha256(),
        )
        .await
        .expect("read back the bound empty ClickHouse epoch");
        assert_eq!(
            readback.report().original_projection_report_sha256,
            projection.report_sha256()
        );
        assert!(readback.report().native_event_epoch_is_still_empty);

        source.close().await;
        target.close().await;
    }

    #[tokio::test]
    #[ignore = "requires V2BOARD_TRAFFIC_FOLD_POSTGRES_URL pointing at an empty disposable PostgreSQL 18 database with the final baseline applied"]
    async fn postgres_traffic_fold_is_atomic_append_only_and_retry_exact() {
        let database_url =
            std::env::var("V2BOARD_TRAFFIC_FOLD_POSTGRES_URL").expect("traffic fold database URL");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("PostgreSQL fixture connection");
        sqlx::migrate!("../../migrations-postgres")
            .run(&pool)
            .await
            .expect("apply the exact PostgreSQL baseline and SQLx ledger");
        let binding = binding();
        let operation_id = parse_uuid(&binding.operation_id).expect("operation UUID");
        let installation_id =
            parse_uuid(&binding.target_installation_id).expect("installation UUID");
        sqlx::query(
            "INSERT INTO v2_system_installation (\
             singleton, installation_id, lineage, state, created_at, source_fingerprint_sha256) \
             VALUES (1, $1, 'legacy_migrated', 'pending', 1000, $2)",
        )
        .bind(installation_id)
        .bind(&binding.source_snapshot_sha256)
        .execute(&pool)
        .await
        .expect("installation");
        sqlx::query(
            "INSERT INTO v2_lifecycle_operation (\
             operation_id, installation_id, kind, manifest_binding_hmac_sha256, \
             inspect_review_sha256, authorized_snapshot_report_sha256, \
             authorized_snapshot_report_binding_hmac_sha256, authorization_binding_hmac_sha256, \
             authorization_file_sha256, source_fingerprint_sha256, converter_registry_sha256, \
             target_lineage_sha256, state, checkpoint, journal_generation, \
             journal_event_sha256, checkpoint_proof_sha256, backup_reference, \
             backup_restore_proof_sha256, final_recheck_report_sha256, created_at, updated_at) \
             VALUES ($1, $2, 'legacy_reference_migration', $3, $4, $5, $6, $7, $8, $9, \
             $10, $11, 'running', 6, 6, $12, $13, 'backup:test', $14, $15, 1000, 1000)",
        )
        .bind(operation_id)
        .bind(installation_id)
        .bind("1".repeat(64))
        .bind("2".repeat(64))
        .bind("3".repeat(64))
        .bind("4".repeat(64))
        .bind("5".repeat(64))
        .bind("6".repeat(64))
        .bind(&binding.source_snapshot_sha256)
        .bind(&binding.registry_sha256)
        .bind(TARGET_POSTGRES_LINEAGE_SHA256)
        .bind("6".repeat(64))
        .bind("7".repeat(64))
        .bind("8".repeat(64))
        .bind("9".repeat(64))
        .execute(&pool)
        .await
        .expect("operation");
        sqlx::query(
            "INSERT INTO v2_lifecycle_event (\
             operation_id, generation, state, checkpoint, previous_event_sha256, \
             event_sha256, checkpoint_proof_sha256, recorded_at_unix_ms) \
             VALUES ($1, 2, 'running', 2, $2, $3, $4, 1000)",
        )
        .bind(operation_id)
        .bind("0".repeat(64))
        .bind("3".repeat(64))
        .bind("4".repeat(64))
        .execute(&pool)
        .await
        .expect("SourceDrained event");
        sqlx::query(
            "INSERT INTO v2_lifecycle_event (\
             operation_id, generation, state, checkpoint, previous_event_sha256, event_sha256, \
             checkpoint_proof_sha256, installation_id, backup_restore_proof_sha256, \
             backup_reference_sha256, final_recheck_report_sha256, source_fingerprint_sha256, \
             recorded_at_unix_ms) VALUES (\
             $1, 6, 'running', 6, $2, $3, $4, $5, $6, $7, $8, $9, 1001)",
        )
        .bind(operation_id)
        .bind("5".repeat(64))
        .bind("6".repeat(64))
        .bind("7".repeat(64))
        .bind(installation_id)
        .bind("8".repeat(64))
        .bind("a".repeat(64))
        .bind("9".repeat(64))
        .bind(&binding.source_snapshot_sha256)
        .execute(&pool)
        .await
        .expect("current event");
        for (id, email, token, u, d) in [
            (
                7_i64,
                "traffic7@example.test",
                "00000000000000000000000000000007",
                100_i64,
                200_i64,
            ),
            (
                9_i64,
                "traffic9@example.test",
                "00000000000000000000000000000009",
                300_i64,
                400_i64,
            ),
        ] {
            sqlx::query(
                "INSERT INTO v2_user (id, email, password, uuid, token, u, d, t, created_at, updated_at) \
                 VALUES ($1, $2, 'hash', $3, $4, $5, $6, 30, 10, 40)",
            )
            .bind(id)
            .bind(email)
            .bind(uuid::Uuid::from_u128(id as u128).to_string())
            .bind(token)
            .bind(u)
            .bind(d)
            .execute(&pool)
            .await
            .expect("fixture user");
        }
        let batch = verified_traffic_batch(
            BTreeMap::from([(7_i64, 11_i128)]),
            BTreeMap::from([(9_i64, 17_i128)]),
        );
        let source_users = BTreeMap::from([
            (7_i64, (100_i64, 200_i64, 30_i64, 40_i64)),
            (9_i64, (300_i64, 400_i64, 30_i64, 40_i64)),
        ]);
        let plan = build_traffic_fold_plan(&binding, &batch, &source_users).expect("traffic plan");
        let target = PostgresCopyTarget::new(&pool).await.expect("copy target");
        let first = target
            .apply_frozen_traffic(&plan)
            .await
            .expect("first traffic fold");
        let retry = target
            .apply_frozen_traffic(&plan)
            .await
            .expect("exact retry");
        assert_eq!(retry, first);
        assert!(first.applied_exactly_once);
        assert_eq!(first.ledger_item_count, 2);
        let users = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT id, u, d, t, updated_at FROM v2_user WHERE id IN (7, 9) ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .expect("folded users");
        assert_eq!(users[0], (7, 111, 200, 1_700_000_000, 1_700_000_000));
        assert_eq!(users[1], (9, 300, 417, 1_700_000_000, 1_700_000_000));
        assert!(
            sqlx::query(
                "UPDATE v2_legacy_traffic_fold_item SET upload_delta = upload_delta + 1 \
                 WHERE operation_id = $1 AND user_id = 7",
            )
            .bind(operation_id)
            .execute(&pool)
            .await
            .is_err()
        );
        pool.close().await;
    }

    #[test]
    fn giftcard_candidate_ids_are_strict_and_deduplicated() {
        let ids = giftcard_candidate_user_ids(&SourceValue::Text(r#"[9,"7",9]"#.to_string()))
            .expect("ids");
        assert_eq!(ids, [7_u64, 9].into_iter().collect());
        assert!(giftcard_candidate_user_ids(&SourceValue::Text(r#"["07"]"#.to_string())).is_err());
    }
}
