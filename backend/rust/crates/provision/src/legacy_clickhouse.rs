//! Exact ClickHouse stage for the offline, one-shot legacy migration.
//!
//! Legacy `v2_stat`, `v2_stat_server`, and `v2_stat_user` rows are durable
//! daily summaries, not native raw analytics events. They remain value-for-
//! value in PostgreSQL and are intentionally never expanded into fabricated
//! ClickHouse facts. ClickHouse starts at an explicitly empty native event
//! epoch and receives only events produced after native activation.

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use v2board_analytics::{
    AnalyticsAdmissionError, AnalyticsAdmissionPolicy, AnalyticsAdmissionSnapshot,
    AnalyticsPressureState, CLICKHOUSE_MIGRATIONS, ClickHouseMigrationError,
    ClickHouseProjectionCounts, analytics_admission_policy_sha256, bind_clickhouse_installation,
    clickhouse_client, clickhouse_projection_counts, clickhouse_schema_lineage_sha256,
    configure_clickhouse_retention, inspect_analytics_admission_exact,
    install_analytics_admission_policy, migrate_clickhouse, refresh_analytics_admission,
    verify_clickhouse_bound_contract, verify_clickhouse_runtime_ready,
};

use crate::{
    ProvisionSpec,
    apply_journal::DurableTargetMutationPermit,
    legacy_apply::{LegacyApplyError, VerifiedStageProof},
    legacy_converter::TABLE_MAPPINGS,
    legacy_copy::{
        LegacyCopyError, LegacyCopyVerification, TableVerification, legacy_copy_verification_sha256,
    },
    manifest::{AnalyticsAdmissionSpec, ProvisionFlow, TargetSpec},
};

const REPORT_DOMAIN: &[u8] = b"v2board.legacy-clickhouse-projection-report.v2\0";
const READBACK_DOMAIN: &[u8] = b"v2board.legacy-clickhouse-readback-report.v1\0";
const REPORT_VERSION: u32 = 2;
const LEGACY_STAT_TABLES: &[&str] = &["v2_stat", "v2_stat_server", "v2_stat_user"];

/// A typed return value is retained so any future production gap can disable
/// apply without changing the caller contract. The current implementation has
/// no known ClickHouse production blockers: both traffic producers reserve the
/// PostgreSQL admission singleton transactionally and the relay releases it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyClickHouseProductionBlocker {
    PostActivationOutboxBackpressurePolicyNotImplemented,
}

pub const fn legacy_clickhouse_production_blockers() -> &'static [LegacyClickHouseProductionBlocker]
{
    &[]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PostgresAnalyticsLedgerCounts {
    pub outbox_rows: u64,
    pub delivery_batch_rows: u64,
}

impl PostgresAnalyticsLedgerCounts {
    const fn is_empty(self) -> bool {
        self.outbox_rows == 0 && self.delivery_batch_rows == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacyStatisticsProof {
    pub table: String,
    pub row_count: u64,
    pub primary_key_sha256: String,
    pub canonical_sha256: String,
    pub authority: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AnalyticsAdmissionProof {
    pub installation_id: String,
    pub policy_sha256: String,
    pub pressure_state: String,
    pub generation: u64,
    pub sampled_at: i64,
    pub sample_age_seconds: u64,
    pub sample_fresh: bool,
    pub pending_rows: u64,
    pub accounted_pending_rows: u64,
    pub oldest_pending_age_seconds: Option<u64>,
    pub relation_heap_bytes: u64,
    pub relation_index_bytes: u64,
    pub relation_toast_bytes: u64,
    pub relation_total_bytes: u64,
    pub accounted_relation_bytes: u64,
    pub database_bytes: u64,
    pub capacity_headroom_bytes: i64,
    pub last_transition_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacyClickHouseProjectionReport {
    pub report_version: u32,
    pub operation_id: String,
    pub installation_id: String,
    pub permit_generation: u64,
    pub permit_event_sha256: String,
    pub postgres_verification_report_sha256: String,
    pub clickhouse_schema_migration_count: usize,
    pub clickhouse_schema_lineage_sha256: String,
    pub raw_retention_days: u32,
    pub aggregate_retention_days: u32,
    pub analytics_admission_policy: AnalyticsAdmissionPolicy,
    pub analytics_admission_policy_sha256: String,
    pub analytics_admission_before: AnalyticsAdmissionProof,
    pub analytics_admission_after: AnalyticsAdmissionProof,
    pub postgres_ledgers_before: PostgresAnalyticsLedgerCounts,
    pub postgres_ledgers_after: PostgresAnalyticsLedgerCounts,
    pub clickhouse_before_binding: ClickHouseProjectionCounts,
    pub clickhouse_after_binding: ClickHouseProjectionCounts,
    pub legacy_statistics: Vec<LegacyStatisticsProof>,
    pub legacy_daily_statistics_preserved_in_postgres: bool,
    pub historical_clickhouse_raw_events_synthesized: bool,
    pub clickhouse_native_event_epoch_starts_empty: bool,
    pub offline_outbox_admission_is_read_only_and_empty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLegacyClickHouseProjection {
    report: LegacyClickHouseProjectionReport,
    report_sha256: String,
}

impl VerifiedLegacyClickHouseProjection {
    pub fn report(&self) -> &LegacyClickHouseProjectionReport {
        &self.report
    }

    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }

    pub fn stage_proof(&self) -> Result<VerifiedStageProof, LegacyApplyError> {
        VerifiedStageProof::new(self.report_sha256.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LegacyClickHouseReadbackReport {
    pub report_version: u32,
    pub operation_id: String,
    pub installation_id: String,
    pub original_projection_report_sha256: String,
    pub clickhouse_schema_migration_count: usize,
    pub clickhouse_schema_lineage_sha256: String,
    pub raw_retention_days: u32,
    pub aggregate_retention_days: u32,
    pub analytics_admission_policy_sha256: String,
    pub analytics_admission: AnalyticsAdmissionProof,
    pub postgres_ledgers: PostgresAnalyticsLedgerCounts,
    pub clickhouse_counts: ClickHouseProjectionCounts,
    pub postgres_snapshot_is_read_only: bool,
    pub clickhouse_verification_is_read_only: bool,
    pub native_event_epoch_is_still_empty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLegacyClickHouseReadback {
    report: LegacyClickHouseReadbackReport,
    report_sha256: String,
}

impl VerifiedLegacyClickHouseReadback {
    pub fn report(&self) -> &LegacyClickHouseReadbackReport {
        &self.report
    }

    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LegacyClickHouseStageError {
    #[error("ClickHouse projection requires the schema-v4 legacy flow")]
    WrongFlow,
    #[error("ClickHouse projection permit does not match the manifest or PostgreSQL checkpoint")]
    PermitBinding,
    #[error("PostgreSQL value-verification report is incomplete or inconsistent")]
    PostgresVerification,
    #[error("PostgreSQL analytics outbox/delivery ledgers are not empty")]
    PostgresAnalyticsNotEmpty,
    #[error("target PostgreSQL installation reservation is missing or inconsistent")]
    InstallationReservation,
    #[error("ClickHouse projection is not empty")]
    ClickHouseNotEmpty,
    #[error("legacy copy verification failed: {0}")]
    Copy(#[from] LegacyCopyError),
    #[error("PostgreSQL projection admission query failed: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("PostgreSQL analytics admission policy/state failed: {0}")]
    AnalyticsAdmission(#[from] AnalyticsAdmissionError),
    #[error("PostgreSQL analytics admission is not initially empty, fresh, and normal")]
    AnalyticsAdmissionUnsafe,
    #[error("original ClickHouse projection proof is not a lowercase SHA-256")]
    InvalidOriginalProjectionProof,
    #[error("ClickHouse projection failed: {0}")]
    ClickHouse(#[from] ClickHouseMigrationError),
    #[error("ClickHouse projection report serialization failed: {0}")]
    Report(#[from] serde_json::Error),
}

/// Apply and verify the exact ClickHouse lineage using only the schema
/// principal from the HMAC-bound manifest. The caller must already hold the
/// durable target-mutation permit at `PostgresValueVerified`; this function
/// never starts a producer or synthesizes historical raw events.
pub async fn project_legacy_clickhouse(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    postgres_verification: &LegacyCopyVerification,
) -> Result<VerifiedLegacyClickHouseProjection, LegacyClickHouseStageError> {
    let target = legacy_target(spec)?;
    if spec.schema_version != 4
        || spec.legacy_apply_execution().is_none()
        || spec.operation_id != permit.operation_id()
        || Uuid::parse_str(permit.installation_id()).is_err()
    {
        return Err(LegacyClickHouseStageError::PermitBinding);
    }
    let postgres_verification_sha256 = legacy_copy_verification_sha256(postgres_verification)?;
    if permit.checkpoint_proof_sha256() != Some(postgres_verification_sha256.as_str()) {
        return Err(LegacyClickHouseStageError::PermitBinding);
    }
    let legacy_statistics = verify_postgres_report(postgres_verification)?;

    let postgres = PgPool::connect(&target.postgres.migration_database_url).await?;
    verify_installation_reservation(&postgres, permit.installation_id()).await?;
    let postgres_ledgers_before = postgres_analytics_ledger_counts(&postgres).await?;
    if !postgres_ledgers_before.is_empty() {
        return Err(LegacyClickHouseStageError::PostgresAnalyticsNotEmpty);
    }
    let now_unix: i64 =
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint")
            .fetch_one(&postgres)
            .await?;
    let installation_id = Uuid::parse_str(permit.installation_id())
        .map_err(|_| LegacyClickHouseStageError::PermitBinding)?;
    let analytics_admission_policy = analytics_admission_policy(&target.analytics_admission);
    let analytics_admission_policy_sha256 = install_analytics_admission_policy(
        &postgres,
        installation_id,
        &analytics_admission_policy,
        now_unix,
    )
    .await?;
    let analytics_admission_before =
        admission_proof(refresh_analytics_admission(&postgres).await?.snapshot)?;
    if analytics_admission_before.installation_id != installation_id.to_string()
        || analytics_admission_before.policy_sha256 != analytics_admission_policy_sha256
    {
        return Err(LegacyClickHouseStageError::AnalyticsAdmissionUnsafe);
    }

    let clickhouse_target = &target.clickhouse;
    let client = clickhouse_client(
        &clickhouse_target.endpoint,
        &clickhouse_target.database,
        &clickhouse_target.schema_principal.username,
        Some(clickhouse_target.schema_principal.password()),
    );
    migrate_clickhouse(&client, now_unix).await?;
    let clickhouse_before_binding = clickhouse_projection_counts(&client).await?;
    if !clickhouse_before_binding.is_empty() {
        return Err(LegacyClickHouseStageError::ClickHouseNotEmpty);
    }
    bind_clickhouse_installation(&client, installation_id, now_unix).await?;
    configure_clickhouse_retention(
        &client,
        installation_id,
        clickhouse_target.raw_retention_days,
        clickhouse_target.aggregate_retention_days,
        now_unix,
    )
    .await?;
    verify_clickhouse_runtime_ready(&client, installation_id).await?;
    let clickhouse_after_binding = clickhouse_projection_counts(&client).await?;
    if !clickhouse_after_binding.is_empty() {
        return Err(LegacyClickHouseStageError::ClickHouseNotEmpty);
    }

    verify_installation_reservation(&postgres, permit.installation_id()).await?;
    let postgres_ledgers_after = postgres_analytics_ledger_counts(&postgres).await?;
    if !postgres_ledgers_after.is_empty() {
        return Err(LegacyClickHouseStageError::PostgresAnalyticsNotEmpty);
    }
    let analytics_admission_after =
        admission_proof(refresh_analytics_admission(&postgres).await?.snapshot)?;
    if analytics_admission_after.installation_id != installation_id.to_string()
        || analytics_admission_after.policy_sha256 != analytics_admission_policy_sha256
    {
        return Err(LegacyClickHouseStageError::AnalyticsAdmissionUnsafe);
    }

    let report = LegacyClickHouseProjectionReport {
        report_version: REPORT_VERSION,
        operation_id: spec.operation_id.clone(),
        installation_id: permit.installation_id().to_string(),
        permit_generation: permit.generation(),
        permit_event_sha256: permit.event_sha256().to_string(),
        postgres_verification_report_sha256: postgres_verification_sha256,
        clickhouse_schema_migration_count: CLICKHOUSE_MIGRATIONS.len(),
        clickhouse_schema_lineage_sha256: clickhouse_schema_lineage_sha256(),
        raw_retention_days: clickhouse_target.raw_retention_days,
        aggregate_retention_days: clickhouse_target.aggregate_retention_days,
        analytics_admission_policy,
        analytics_admission_policy_sha256,
        analytics_admission_before,
        analytics_admission_after,
        postgres_ledgers_before,
        postgres_ledgers_after,
        clickhouse_before_binding,
        clickhouse_after_binding,
        legacy_statistics,
        legacy_daily_statistics_preserved_in_postgres: true,
        historical_clickhouse_raw_events_synthesized: false,
        clickhouse_native_event_epoch_starts_empty: true,
        offline_outbox_admission_is_read_only_and_empty: true,
    };
    let report_sha256 = report_sha256(&report)?;
    Ok(VerifiedLegacyClickHouseProjection {
        report,
        report_sha256,
    })
}

/// Re-read every analytics authority fact immediately before native authority
/// commit. This path never migrates, binds, alters TTL, refreshes admission
/// state, or writes a receipt.
pub async fn verify_legacy_clickhouse_projection_read_only(
    spec: &ProvisionSpec,
    expected_installation_id: &str,
    original_projection_report_sha256: &str,
) -> Result<VerifiedLegacyClickHouseReadback, LegacyClickHouseStageError> {
    let target = legacy_target(spec)?;
    if spec.schema_version != 4
        || spec.legacy_apply_execution().is_none()
        || !is_lower_hex_sha256(original_projection_report_sha256)
    {
        return Err(LegacyClickHouseStageError::InvalidOriginalProjectionProof);
    }
    let installation_id = Uuid::parse_str(expected_installation_id)
        .map_err(|_| LegacyClickHouseStageError::InstallationReservation)?;
    let postgres = PgPool::connect(&target.postgres.migration_database_url).await?;
    verify_installation_reservation(&postgres, expected_installation_id).await?;
    let postgres_ledgers = postgres_analytics_ledger_counts(&postgres).await?;
    if !postgres_ledgers.is_empty() {
        return Err(LegacyClickHouseStageError::PostgresAnalyticsNotEmpty);
    }
    let expected_policy = analytics_admission_policy(&target.analytics_admission);
    let expected_policy_sha256 = analytics_admission_policy_sha256(&expected_policy)?;
    let analytics_admission = admission_proof(inspect_analytics_admission_exact(&postgres).await?)?;
    if analytics_admission.installation_id != expected_installation_id
        || analytics_admission.policy_sha256 != expected_policy_sha256
    {
        return Err(LegacyClickHouseStageError::AnalyticsAdmissionUnsafe);
    }

    let clickhouse_target = &target.clickhouse;
    let client = clickhouse_client(
        &clickhouse_target.endpoint,
        &clickhouse_target.database,
        &clickhouse_target.schema_principal.username,
        Some(clickhouse_target.schema_principal.password()),
    );
    verify_clickhouse_bound_contract(
        &client,
        installation_id,
        clickhouse_target.raw_retention_days,
        clickhouse_target.aggregate_retention_days,
    )
    .await?;
    let clickhouse_counts = clickhouse_projection_counts(&client).await?;
    if !clickhouse_counts.is_empty() {
        return Err(LegacyClickHouseStageError::ClickHouseNotEmpty);
    }

    let report = LegacyClickHouseReadbackReport {
        report_version: 1,
        operation_id: spec.operation_id.clone(),
        installation_id: expected_installation_id.to_owned(),
        original_projection_report_sha256: original_projection_report_sha256.to_owned(),
        clickhouse_schema_migration_count: CLICKHOUSE_MIGRATIONS.len(),
        clickhouse_schema_lineage_sha256: clickhouse_schema_lineage_sha256(),
        raw_retention_days: clickhouse_target.raw_retention_days,
        aggregate_retention_days: clickhouse_target.aggregate_retention_days,
        analytics_admission_policy_sha256: expected_policy_sha256,
        analytics_admission,
        postgres_ledgers,
        clickhouse_counts,
        postgres_snapshot_is_read_only: true,
        clickhouse_verification_is_read_only: true,
        native_event_epoch_is_still_empty: true,
    };
    let report_sha256 = readback_report_sha256(&report)?;
    Ok(VerifiedLegacyClickHouseReadback {
        report,
        report_sha256,
    })
}

pub fn readback_report_sha256(
    report: &LegacyClickHouseReadbackReport,
) -> Result<String, serde_json::Error> {
    let encoded = serde_json::to_vec(report)?;
    let mut digest = Sha256::new();
    digest.update(READBACK_DOMAIN);
    digest.update((encoded.len() as u64).to_be_bytes());
    digest.update(encoded);
    Ok(hex::encode(digest.finalize()))
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn report_sha256(
    report: &LegacyClickHouseProjectionReport,
) -> Result<String, serde_json::Error> {
    let encoded = serde_json::to_vec(report)?;
    let mut digest = Sha256::new();
    digest.update(REPORT_DOMAIN);
    digest.update((encoded.len() as u64).to_be_bytes());
    digest.update(encoded);
    Ok(hex::encode(digest.finalize()))
}

fn legacy_target(spec: &ProvisionSpec) -> Result<&TargetSpec, LegacyClickHouseStageError> {
    match &spec.flow {
        ProvisionFlow::LegacyReferenceMigration { target, .. } => Ok(target),
        _ => Err(LegacyClickHouseStageError::WrongFlow),
    }
}

fn analytics_admission_policy(spec: &AnalyticsAdmissionSpec) -> AnalyticsAdmissionPolicy {
    AnalyticsAdmissionPolicy {
        recovery_pending_rows: spec.recovery_pending_rows,
        soft_pending_rows: spec.soft_pending_rows,
        hard_pending_rows: spec.hard_pending_rows,
        recovery_relation_bytes: spec.recovery_relation_bytes,
        soft_relation_bytes: spec.soft_relation_bytes,
        hard_relation_bytes: spec.hard_relation_bytes,
        recovery_oldest_age_seconds: spec.recovery_oldest_age_seconds,
        soft_oldest_age_seconds: spec.soft_oldest_age_seconds,
        hard_oldest_age_seconds: spec.hard_oldest_age_seconds,
        database_capacity_bytes: spec.database_capacity_bytes,
        hard_min_headroom_bytes: spec.hard_min_headroom_bytes,
        soft_min_headroom_bytes: spec.soft_min_headroom_bytes,
        recovery_min_headroom_bytes: spec.recovery_min_headroom_bytes,
        event_reservation_bytes: spec.event_reservation_bytes,
        soft_max_new_rows_per_second: spec.soft_max_new_rows_per_second,
        sample_interval_seconds: spec.sample_interval_seconds,
        stale_after_seconds: spec.stale_after_seconds,
        capacity_evidence: spec.capacity_evidence.clone(),
    }
}

fn admission_proof(
    snapshot: AnalyticsAdmissionSnapshot,
) -> Result<AnalyticsAdmissionProof, LegacyClickHouseStageError> {
    if !snapshot.sample_fresh
        || snapshot.pressure_state != AnalyticsPressureState::Normal
        || snapshot.pending_rows != 0
        || snapshot.accounted_pending_rows != 0
        || snapshot.oldest_pending_age_seconds.is_some()
    {
        return Err(LegacyClickHouseStageError::AnalyticsAdmissionUnsafe);
    }
    Ok(AnalyticsAdmissionProof {
        installation_id: snapshot.installation_id.to_string(),
        policy_sha256: snapshot.policy_sha256,
        pressure_state: snapshot.pressure_state.as_str().to_owned(),
        generation: snapshot.generation,
        sampled_at: snapshot.sampled_at,
        sample_age_seconds: snapshot.sample_age_seconds,
        sample_fresh: snapshot.sample_fresh,
        pending_rows: snapshot.pending_rows,
        accounted_pending_rows: snapshot.accounted_pending_rows,
        oldest_pending_age_seconds: snapshot.oldest_pending_age_seconds,
        relation_heap_bytes: snapshot.relation_heap_bytes,
        relation_index_bytes: snapshot.relation_index_bytes,
        relation_toast_bytes: snapshot.relation_toast_bytes,
        relation_total_bytes: snapshot.relation_total_bytes,
        accounted_relation_bytes: snapshot.accounted_relation_bytes,
        database_bytes: snapshot.database_bytes,
        capacity_headroom_bytes: snapshot.capacity_headroom_bytes,
        last_transition_reason: snapshot.last_transition_reason,
    })
}

fn verify_postgres_report(
    verification: &LegacyCopyVerification,
) -> Result<Vec<LegacyStatisticsProof>, LegacyClickHouseStageError> {
    if verification.base_tables.len() != TABLE_MAPPINGS.len()
        || verification.derived_tables.iter().any(|table| {
            table.expected_count != table.target_count
                || table.expected_sha256 != table.target_sha256
        })
        || !verification.traffic_fold.applied_exactly_once
    {
        return Err(LegacyClickHouseStageError::PostgresVerification);
    }
    for (observed, mapping) in verification.base_tables.iter().zip(TABLE_MAPPINGS) {
        if observed.table != mapping.target
            || observed.source_count != observed.target_count
            || observed.source_primary_key_sha256 != observed.target_primary_key_sha256
            || observed.source_canonical_sha256 != observed.target_canonical_sha256
        {
            return Err(LegacyClickHouseStageError::PostgresVerification);
        }
    }
    LEGACY_STAT_TABLES
        .iter()
        .map(|name| {
            let table = exact_table(&verification.base_tables, name)?;
            Ok(LegacyStatisticsProof {
                table: table.table.clone(),
                row_count: table.target_count,
                primary_key_sha256: table.target_primary_key_sha256.clone(),
                canonical_sha256: table.target_canonical_sha256.clone(),
                authority: "postgresql_value_verified_legacy_daily_summary",
            })
        })
        .collect()
}

fn exact_table<'a>(
    tables: &'a [TableVerification],
    name: &str,
) -> Result<&'a TableVerification, LegacyClickHouseStageError> {
    let mut matching = tables.iter().filter(|table| table.table == name);
    let Some(table) = matching.next() else {
        return Err(LegacyClickHouseStageError::PostgresVerification);
    };
    if matching.next().is_some() {
        return Err(LegacyClickHouseStageError::PostgresVerification);
    }
    Ok(table)
}

async fn postgres_analytics_ledger_counts(
    pool: &PgPool,
) -> Result<PostgresAnalyticsLedgerCounts, LegacyClickHouseStageError> {
    let (outbox_rows, delivery_batch_rows): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM v2_analytics_outbox)::bigint, \
                (SELECT count(*) FROM v2_analytics_delivery_batch)::bigint",
    )
    .fetch_one(pool)
    .await?;
    Ok(PostgresAnalyticsLedgerCounts {
        outbox_rows: u64::try_from(outbox_rows)
            .map_err(|_| LegacyClickHouseStageError::PostgresVerification)?,
        delivery_batch_rows: u64::try_from(delivery_batch_rows)
            .map_err(|_| LegacyClickHouseStageError::PostgresVerification)?,
    })
}

async fn verify_installation_reservation(
    pool: &PgPool,
    expected_installation_id: &str,
) -> Result<(), LegacyClickHouseStageError> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT installation_id::text, lineage, state \
         FROM v2_system_installation ORDER BY installation_id",
    )
    .fetch_all(pool)
    .await?;
    if rows.as_slice()
        != [(
            expected_installation_id.to_string(),
            "legacy_migrated".to_string(),
            "pending".to_string(),
        )]
    {
        return Err(LegacyClickHouseStageError::InstallationReservation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_blockers_are_empty_after_all_producers_use_admission() {
        assert!(legacy_clickhouse_production_blockers().is_empty());
        let outbox = include_str!("../../analytics/src/outbox.rs");
        let api_producer = include_str!("../../api/src/server_api/traffic.rs");
        let worker_producer = include_str!("../../workers/src/traffic.rs");
        assert!(outbox.contains("admit_analytics_rows(tx, inserted_rows, created_at).await?"));
        assert!(
            outbox
                .matches("release_terminal_rows(&mut tx, batch.rows.len())")
                .count()
                >= 2
        );
        assert!(api_producer.contains("enqueue_events(&mut tx, &analytics_events, now)"));
        assert!(worker_producer.contains("enqueue_events(tx, &events, accounted_at)"));
    }

    #[test]
    fn projection_report_hash_is_domain_separated_and_deterministic() {
        let report = LegacyClickHouseProjectionReport {
            report_version: REPORT_VERSION,
            operation_id: Uuid::from_u128(1).to_string(),
            installation_id: Uuid::from_u128(2).to_string(),
            permit_generation: 8,
            permit_event_sha256: "a".repeat(64),
            postgres_verification_report_sha256: "b".repeat(64),
            clickhouse_schema_migration_count: CLICKHOUSE_MIGRATIONS.len(),
            clickhouse_schema_lineage_sha256: clickhouse_schema_lineage_sha256(),
            raw_retention_days: 90,
            aggregate_retention_days: 730,
            analytics_admission_policy: test_admission_policy(),
            analytics_admission_policy_sha256: "c".repeat(64),
            analytics_admission_before: test_admission_proof(),
            analytics_admission_after: test_admission_proof(),
            postgres_ledgers_before: PostgresAnalyticsLedgerCounts {
                outbox_rows: 0,
                delivery_batch_rows: 0,
            },
            postgres_ledgers_after: PostgresAnalyticsLedgerCounts {
                outbox_rows: 0,
                delivery_batch_rows: 0,
            },
            clickhouse_before_binding: ClickHouseProjectionCounts {
                reported_raw_rows: 0,
                accounted_raw_rows: 0,
                reported_daily_rows: 0,
                accounted_daily_rows: 0,
            },
            clickhouse_after_binding: ClickHouseProjectionCounts {
                reported_raw_rows: 0,
                accounted_raw_rows: 0,
                reported_daily_rows: 0,
                accounted_daily_rows: 0,
            },
            legacy_statistics: Vec::new(),
            legacy_daily_statistics_preserved_in_postgres: true,
            historical_clickhouse_raw_events_synthesized: false,
            clickhouse_native_event_epoch_starts_empty: true,
            offline_outbox_admission_is_read_only_and_empty: true,
        };
        assert_eq!(
            report_sha256(&report).unwrap(),
            report_sha256(&report).unwrap()
        );
        assert_eq!(report_sha256(&report).unwrap().len(), 64);
    }

    fn test_admission_policy() -> AnalyticsAdmissionPolicy {
        AnalyticsAdmissionPolicy {
            recovery_pending_rows: 750_000,
            soft_pending_rows: 1_000_000,
            hard_pending_rows: 2_000_000,
            recovery_relation_bytes: 3 * 1024 * 1024 * 1024,
            soft_relation_bytes: 4 * 1024 * 1024 * 1024,
            hard_relation_bytes: 8 * 1024 * 1024 * 1024,
            recovery_oldest_age_seconds: 120,
            soft_oldest_age_seconds: 300,
            hard_oldest_age_seconds: 1_800,
            database_capacity_bytes: 64 * 1024 * 1024 * 1024,
            hard_min_headroom_bytes: 8 * 1024 * 1024 * 1024,
            soft_min_headroom_bytes: 16 * 1024 * 1024 * 1024,
            recovery_min_headroom_bytes: 20 * 1024 * 1024 * 1024,
            event_reservation_bytes: 4_096,
            soft_max_new_rows_per_second: 100_000,
            sample_interval_seconds: 1,
            stale_after_seconds: 10,
            capacity_evidence: "dedicated PostgreSQL volume quota".to_owned(),
        }
    }

    fn test_admission_proof() -> AnalyticsAdmissionProof {
        AnalyticsAdmissionProof {
            installation_id: Uuid::from_u128(2).to_string(),
            policy_sha256: "c".repeat(64),
            pressure_state: "normal".to_owned(),
            generation: 1,
            sampled_at: 1_700_000_000,
            sample_age_seconds: 0,
            sample_fresh: true,
            pending_rows: 0,
            accounted_pending_rows: 0,
            oldest_pending_age_seconds: None,
            relation_heap_bytes: 0,
            relation_index_bytes: 0,
            relation_toast_bytes: 0,
            relation_total_bytes: 0,
            accounted_relation_bytes: 0,
            database_bytes: 1,
            capacity_headroom_bytes: 64 * 1024 * 1024 * 1024 - 1,
            last_transition_reason: "exact_sample".to_owned(),
        }
    }
}
