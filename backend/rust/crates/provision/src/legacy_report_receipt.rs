//! Durable schema-v5 preimages for the one-shot migration verification reports.
//!
//! The filesystem journal and permanent PostgreSQL ledger intentionally retain
//! only report hashes. Schema v5 also keeps the exact JSON preimages in
//! root-owned, HMAC-bound, no-clobber receipts so the deliberate discard of
//! legacy node, traffic-detail, and operational-log tables remains independently
//! auditable after the source materialization has been destroyed.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    ProvisionSpec,
    apply_journal::{
        ApplyCheckpoint, ApplyJournalSnapshot, ApplyJournalState, DurableTargetMutationPermit,
    },
    legacy_clickhouse::{VerifiedLegacyClickHouseProjection, report_sha256_from_bytes},
    legacy_converter::{
        LEGACY_SEMANTIC_SCHEMA_SHA256, LegacyConversionStrategy, registry_sha256_for_strategy,
    },
    legacy_copy::{LegacyCopyVerification, legacy_copy_verification_sha256_from_bytes},
    manifest::LegacyRuntimeReceiptKind,
    native_legacy_source::{
        SourceError, existing_receipt_file, publish_owner_only_no_clobber, read_owner_only_file,
    },
};

const RECEIPT_VERSION: u32 = 1;
const MAX_DURABLE_REPORT_RECEIPT_BYTES: u64 = 64 * 1024;
const CLICKHOUSE_SCHEMA_V5_REPORT_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DurableReportKind {
    PostgresVerificationReport,
    ClickhouseProjectionReport,
}

impl DurableReportKind {
    const fn runtime_kind(self) -> LegacyRuntimeReceiptKind {
        match self {
            Self::PostgresVerificationReport => {
                LegacyRuntimeReceiptKind::PostgresVerificationReport
            }
            Self::ClickhouseProjectionReport => {
                LegacyRuntimeReceiptKind::ClickHouseProjectionReport
            }
        }
    }

    const fn input_checkpoint(self) -> ApplyCheckpoint {
        match self {
            Self::PostgresVerificationReport => ApplyCheckpoint::PostgresBulkCopied,
            Self::ClickhouseProjectionReport => ApplyCheckpoint::PostgresValueVerified,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DurableReportReceiptPayload {
    document_kind: DurableReportKind,
    receipt_version: u32,
    operation_id: String,
    installation_id: String,
    manifest_binding_hmac_sha256: String,
    inspect_review_sha256: String,
    source_fingerprint_sha256: String,
    source_schema_sha256: String,
    conversion_strategy: LegacyConversionStrategy,
    converter_registry_sha256: String,
    input_journal_checkpoint: ApplyCheckpoint,
    input_journal_generation: u64,
    input_journal_event_sha256: String,
    input_checkpoint_proof_sha256: String,
    report_sha256: String,
    report_json_utf8: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DurableReportReceipt {
    payload: DurableReportReceiptPayload,
    hmac_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DurableReportReceiptError {
    #[error("durable report receipts are available only for schema-v5 legacy migrations")]
    WrongFlow,
    #[error("schema-v5 durable report receipt path is missing")]
    MissingPath,
    #[error("schema-v5 durable report receipt is missing")]
    Missing,
    #[error("schema-v5 durable report receipt serialization is invalid")]
    Serialization(#[from] serde_json::Error),
    #[error("schema-v5 durable report receipt filesystem state is invalid")]
    Filesystem(#[from] SourceError),
    #[error("schema-v5 durable report receipt binding is invalid")]
    Binding,
    #[error("schema-v5 PostgreSQL report conflicts with its durable preimage")]
    PostgresReportConflict,
    #[error("schema-v5 converter registry is invalid")]
    Registry,
}

#[derive(Deserialize)]
struct PostgresReportIdentity {
    strategy: LegacyConversionStrategy,
    traffic_fold: PostgresTrafficFoldIdentity,
}

#[derive(Deserialize)]
struct PostgresTrafficFoldIdentity {
    operation_id: String,
    target_installation_id: String,
}

#[derive(Deserialize)]
struct ClickhouseReportIdentity {
    report_version: u32,
    operation_id: String,
    installation_id: String,
    permit_generation: u64,
    permit_event_sha256: String,
    postgres_verification_report_sha256: String,
}

struct ReceiptExpectation<'a> {
    operation_id: &'a str,
    installation_id: &'a str,
    inspect_review_sha256: &'a str,
    source_fingerprint_sha256: &'a str,
    maximum_journal_generation: u64,
    report_sha256: Option<&'a str>,
}

/// Publish the exact independently regenerated PostgreSQL report before the
/// journal can advance to `PostgresValueVerified`. A retry may reuse only the
/// byte-identical preimage from an earlier clean attempt.
pub(crate) fn persist_postgres_verification_report(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    history: &[ApplyJournalSnapshot],
    verification: &LegacyCopyVerification,
) -> Result<String, DurableReportReceiptError> {
    require_current_permit_anchor(
        history,
        permit,
        DurableReportKind::PostgresVerificationReport,
    )?;
    let report_bytes = serde_json::to_vec(verification)?;
    let report_sha256 = legacy_copy_verification_sha256_from_bytes(&report_bytes);
    if permit.checkpoint_proof_sha256() != Some(report_sha256.as_str()) {
        return Err(DurableReportReceiptError::Binding);
    }
    let report_json_utf8 =
        String::from_utf8(report_bytes).map_err(|_| DurableReportReceiptError::Binding)?;
    persist_or_reconcile(
        spec,
        permit,
        history,
        DurableReportKind::PostgresVerificationReport,
        report_sha256,
        report_json_utf8,
        true,
    )
}

/// Publish the first complete ClickHouse projection report. If the projection
/// committed and the receipt was fsynced but the caller lost the acknowledgement,
/// a retry independently verifies the current projection and returns the first
/// stored hash even when fresh sampling fields produce different JSON.
pub(crate) fn persist_clickhouse_projection_report(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    history: &[ApplyJournalSnapshot],
    projection: &VerifiedLegacyClickHouseProjection,
) -> Result<String, DurableReportReceiptError> {
    require_current_permit_anchor(
        history,
        permit,
        DurableReportKind::ClickhouseProjectionReport,
    )?;
    let report = projection.report();
    let report_bytes = serde_json::to_vec(report)?;
    let report_sha256 = report_sha256_from_bytes(report.report_version, &report_bytes);
    if projection.report_sha256() != report_sha256
        || permit.checkpoint_proof_sha256()
            != Some(report.postgres_verification_report_sha256.as_str())
    {
        return Err(DurableReportReceiptError::Binding);
    }
    let report_json_utf8 =
        String::from_utf8(report_bytes).map_err(|_| DurableReportReceiptError::Binding)?;
    persist_or_reconcile(
        spec,
        permit,
        history,
        DurableReportKind::ClickhouseProjectionReport,
        report_sha256,
        report_json_utf8,
        false,
    )
}

/// Require the PostgreSQL preimage before the ClickHouse target is mutated.
pub(crate) fn verify_postgres_verification_receipt(
    spec: &ProvisionSpec,
    history: &[ApplyJournalSnapshot],
    installation_id: &str,
    source_fingerprint_sha256: &str,
    expected_report_sha256: &str,
) -> Result<(), DurableReportReceiptError> {
    let maximum_journal_generation = history
        .last()
        .map(ApplyJournalSnapshot::generation)
        .ok_or(DurableReportReceiptError::Binding)?;
    load_and_verify(
        spec,
        history,
        DurableReportKind::PostgresVerificationReport,
        ReceiptExpectation {
            operation_id: &spec.operation_id,
            installation_id,
            inspect_review_sha256: history[0].binding().inspect_review_sha256(),
            source_fingerprint_sha256,
            maximum_journal_generation,
            report_sha256: Some(expected_report_sha256),
        },
    )?;
    Ok(())
}

/// Close the journal/receipt/authority triangle. This performs no live target
/// comparison and therefore remains valid after native traffic has begun.
pub(crate) fn verify_durable_report_receipts(
    spec: &ProvisionSpec,
    history: &[ApplyJournalSnapshot],
    installation_id: &str,
    source_fingerprint_sha256: &str,
    postgres_report_sha256: &str,
    clickhouse_report_sha256: &str,
) -> Result<(), DurableReportReceiptError> {
    let first = history.first().ok_or(DurableReportReceiptError::Binding)?;
    let maximum_journal_generation = history
        .last()
        .map(ApplyJournalSnapshot::generation)
        .ok_or(DurableReportReceiptError::Binding)?;
    let common = |report_sha256| ReceiptExpectation {
        operation_id: &spec.operation_id,
        installation_id,
        inspect_review_sha256: first.binding().inspect_review_sha256(),
        source_fingerprint_sha256,
        maximum_journal_generation,
        report_sha256: Some(report_sha256),
    };
    load_and_verify(
        spec,
        history,
        DurableReportKind::PostgresVerificationReport,
        common(postgres_report_sha256),
    )?;
    load_and_verify(
        spec,
        history,
        DurableReportKind::ClickhouseProjectionReport,
        common(clickhouse_report_sha256),
    )?;
    Ok(())
}

fn persist_or_reconcile(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    history: &[ApplyJournalSnapshot],
    kind: DurableReportKind,
    current_report_sha256: String,
    current_report_json_utf8: String,
    require_identical_existing_report: bool,
) -> Result<String, DurableReportReceiptError> {
    let path = receipt_path(spec, kind)?;
    let expectation = ReceiptExpectation {
        operation_id: permit.operation_id(),
        installation_id: permit.installation_id(),
        inspect_review_sha256: permit.inspect_review_sha256(),
        source_fingerprint_sha256: permit.source_fingerprint_sha256(),
        maximum_journal_generation: permit.generation(),
        report_sha256: None,
    };
    if existing_receipt_file(path)?.is_some() {
        let (receipt, receipt_bytes) = load_and_verify(spec, history, kind, expectation)?;
        if require_identical_existing_report
            && (receipt.payload.report_sha256 != current_report_sha256
                || receipt.payload.report_json_utf8 != current_report_json_utf8)
        {
            return Err(DurableReportReceiptError::PostgresReportConflict);
        }
        publish_owner_only_no_clobber(path, &receipt_bytes, MAX_DURABLE_REPORT_RECEIPT_BYTES)?;
        return Ok(receipt.payload.report_sha256);
    }

    let strategy = required_strategy(spec)?;
    let payload = DurableReportReceiptPayload {
        document_kind: kind,
        receipt_version: RECEIPT_VERSION,
        operation_id: permit.operation_id().to_string(),
        installation_id: permit.installation_id().to_string(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        inspect_review_sha256: permit.inspect_review_sha256().to_string(),
        source_fingerprint_sha256: permit.source_fingerprint_sha256().to_string(),
        source_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.to_string(),
        conversion_strategy: strategy,
        converter_registry_sha256: registry_sha256_for_strategy(strategy)
            .map_err(|_| DurableReportReceiptError::Registry)?,
        input_journal_checkpoint: kind.input_checkpoint(),
        input_journal_generation: permit.generation(),
        input_journal_event_sha256: permit.event_sha256().to_string(),
        input_checkpoint_proof_sha256: permit
            .checkpoint_proof_sha256()
            .ok_or(DurableReportReceiptError::Binding)?
            .to_string(),
        report_sha256: current_report_sha256,
        report_json_utf8: current_report_json_utf8,
    };
    let payload_bytes = serde_json::to_vec(&payload)?;
    let hmac_sha256 = spec
        .source_receipt_binding_hmac_sha256(kind.runtime_kind(), &payload_bytes)
        .ok_or(DurableReportReceiptError::WrongFlow)?;
    let receipt = DurableReportReceipt {
        payload,
        hmac_sha256,
    };
    validate_receipt(spec, history, kind, &receipt, expectation)?;
    let receipt_bytes = serde_json::to_vec(&receipt)?;
    publish_owner_only_no_clobber(path, &receipt_bytes, MAX_DURABLE_REPORT_RECEIPT_BYTES)?;
    Ok(receipt.payload.report_sha256)
}

fn load_and_verify(
    spec: &ProvisionSpec,
    history: &[ApplyJournalSnapshot],
    kind: DurableReportKind,
    expectation: ReceiptExpectation<'_>,
) -> Result<(DurableReportReceipt, Vec<u8>), DurableReportReceiptError> {
    let path = receipt_path(spec, kind)?;
    let existing = existing_receipt_file(path)?.ok_or(DurableReportReceiptError::Missing)?;
    let bytes = read_owner_only_file(&existing, MAX_DURABLE_REPORT_RECEIPT_BYTES)?;
    let receipt: DurableReportReceipt = serde_json::from_slice(&bytes)?;
    if serde_json::to_vec(&receipt)? != bytes {
        return Err(DurableReportReceiptError::Binding);
    }
    let payload_bytes = serde_json::to_vec(&receipt.payload)?;
    if !spec.verify_source_receipt_binding_hmac_sha256(
        kind.runtime_kind(),
        &payload_bytes,
        &receipt.hmac_sha256,
    ) {
        return Err(DurableReportReceiptError::Binding);
    }
    validate_receipt(spec, history, kind, &receipt, expectation)?;
    publish_owner_only_no_clobber(path, &bytes, MAX_DURABLE_REPORT_RECEIPT_BYTES)?;
    Ok((receipt, bytes))
}

fn validate_receipt(
    spec: &ProvisionSpec,
    history: &[ApplyJournalSnapshot],
    kind: DurableReportKind,
    receipt: &DurableReportReceipt,
    expectation: ReceiptExpectation<'_>,
) -> Result<(), DurableReportReceiptError> {
    let payload = &receipt.payload;
    let strategy = required_strategy(spec)?;
    let registry =
        registry_sha256_for_strategy(strategy).map_err(|_| DurableReportReceiptError::Registry)?;
    if payload.document_kind != kind
        || payload.receipt_version != RECEIPT_VERSION
        || payload.operation_id != expectation.operation_id
        || payload.installation_id != expectation.installation_id
        || payload.manifest_binding_hmac_sha256 != spec.manifest_binding_hmac_sha256()
        || payload.inspect_review_sha256 != expectation.inspect_review_sha256
        || payload.source_fingerprint_sha256 != expectation.source_fingerprint_sha256
        || payload.source_schema_sha256 != LEGACY_SEMANTIC_SCHEMA_SHA256
        || payload.conversion_strategy != strategy
        || payload.converter_registry_sha256 != registry
        || payload.input_journal_checkpoint != kind.input_checkpoint()
        || payload.input_journal_generation == 0
        || payload.input_journal_generation > expectation.maximum_journal_generation
        || !is_lower_sha256(&payload.input_journal_event_sha256)
        || !is_lower_sha256(&payload.input_checkpoint_proof_sha256)
        || !is_lower_sha256(&payload.report_sha256)
        || !is_lower_sha256(&receipt.hmac_sha256)
        || expectation
            .report_sha256
            .is_some_and(|expected| payload.report_sha256 != expected)
    {
        return Err(DurableReportReceiptError::Binding);
    }
    validate_history_anchor(history, payload)?;
    validate_report_preimage(kind, payload)
}

fn validate_history_anchor(
    history: &[ApplyJournalSnapshot],
    payload: &DurableReportReceiptPayload,
) -> Result<(), DurableReportReceiptError> {
    let anchor = history
        .iter()
        .find(|snapshot| snapshot.generation() == payload.input_journal_generation)
        .ok_or(DurableReportReceiptError::Binding)?;
    if anchor.event_sha256() != payload.input_journal_event_sha256
        || anchor.state() != ApplyJournalState::Verifying
        || anchor.checkpoint() != payload.input_journal_checkpoint
        || anchor.outcome_code().is_some()
        || anchor.binding().operation_id() != payload.operation_id
        || anchor.binding().inspect_review_sha256() != payload.inspect_review_sha256
        || anchor.installation_id() != Some(payload.installation_id.as_str())
        || anchor.source_fingerprint_sha256() != Some(payload.source_fingerprint_sha256.as_str())
        || anchor.checkpoint_proof_sha256() != Some(payload.input_checkpoint_proof_sha256.as_str())
    {
        return Err(DurableReportReceiptError::Binding);
    }
    Ok(())
}
fn validate_report_preimage(
    kind: DurableReportKind,
    payload: &DurableReportReceiptPayload,
) -> Result<(), DurableReportReceiptError> {
    let bytes = payload.report_json_utf8.as_bytes();
    if bytes.is_empty() {
        return Err(DurableReportReceiptError::Binding);
    }
    match kind {
        DurableReportKind::PostgresVerificationReport => {
            if legacy_copy_verification_sha256_from_bytes(bytes) != payload.report_sha256
                || payload.input_checkpoint_proof_sha256 != payload.report_sha256
            {
                return Err(DurableReportReceiptError::Binding);
            }
            let identity: PostgresReportIdentity = serde_json::from_slice(bytes)?;
            if identity.strategy != payload.conversion_strategy
                || identity.traffic_fold.operation_id != payload.operation_id
                || identity.traffic_fold.target_installation_id != payload.installation_id
            {
                return Err(DurableReportReceiptError::Binding);
            }
        }
        DurableReportKind::ClickhouseProjectionReport => {
            let identity: ClickhouseReportIdentity = serde_json::from_slice(bytes)?;
            if identity.report_version != CLICKHOUSE_SCHEMA_V5_REPORT_VERSION
                || report_sha256_from_bytes(identity.report_version, bytes) != payload.report_sha256
                || identity.operation_id != payload.operation_id
                || identity.installation_id != payload.installation_id
                || identity.permit_generation != payload.input_journal_generation
                || identity.permit_event_sha256 != payload.input_journal_event_sha256
                || identity.postgres_verification_report_sha256
                    != payload.input_checkpoint_proof_sha256
            {
                return Err(DurableReportReceiptError::Binding);
            }
        }
    }
    Ok(())
}

fn require_current_permit_anchor(
    history: &[ApplyJournalSnapshot],
    permit: &DurableTargetMutationPermit,
    kind: DurableReportKind,
) -> Result<(), DurableReportReceiptError> {
    let current = history
        .iter()
        .find(|snapshot| snapshot.generation() == permit.generation())
        .ok_or(DurableReportReceiptError::Binding)?;
    if current.event_sha256() != permit.event_sha256()
        || current.state() != ApplyJournalState::Verifying
        || current.checkpoint() != kind.input_checkpoint()
        || current.outcome_code().is_some()
        || current.binding().operation_id() != permit.operation_id()
        || current.binding().inspect_review_sha256() != permit.inspect_review_sha256()
        || current.installation_id() != Some(permit.installation_id())
        || current.source_fingerprint_sha256() != Some(permit.source_fingerprint_sha256())
        || current.checkpoint_proof_sha256() != permit.checkpoint_proof_sha256()
        || history.last().map(ApplyJournalSnapshot::generation) != Some(permit.generation())
    {
        return Err(DurableReportReceiptError::Binding);
    }
    Ok(())
}

fn receipt_path(
    spec: &ProvisionSpec,
    kind: DurableReportKind,
) -> Result<&Path, DurableReportReceiptError> {
    if spec.schema_version != 5 {
        return Err(DurableReportReceiptError::WrongFlow);
    }
    let receipts = &spec
        .legacy_apply_execution()
        .ok_or(DurableReportReceiptError::WrongFlow)?
        .receipts;
    match kind {
        DurableReportKind::PostgresVerificationReport => {
            receipts.postgres_verification_path.as_deref()
        }
        DurableReportKind::ClickhouseProjectionReport => {
            receipts.clickhouse_projection_path.as_deref()
        }
    }
    .ok_or(DurableReportReceiptError::MissingPath)
}

fn required_strategy(
    spec: &ProvisionSpec,
) -> Result<LegacyConversionStrategy, DurableReportReceiptError> {
    if spec.schema_version != 5 {
        return Err(DurableReportReceiptError::WrongFlow);
    }
    let strategy = LegacyConversionStrategy::for_schema_version(spec.schema_version)
        .map_err(|_| DurableReportReceiptError::WrongFlow)?;
    if strategy != LegacyConversionStrategy::DiscardNodesTrafficDetailsAndOperationalLogs {
        return Err(DurableReportReceiptError::WrongFlow);
    }
    Ok(strategy)
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt},
        path::{Path, PathBuf},
    };

    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::{
        apply_journal::{ApplyJournal, ApplyJournalBinding},
        manifest::ProvisionFlow,
    };

    const OPERATION_ID: &str = "40aa4a80-eb4b-4b25-9c3b-e17ed047873d";
    const INSTALLATION_ID: &str = "e0bb60eb-bb45-4393-8a04-18a3aa510497";
    const INSPECT_REVIEW_SHA256: &str =
        "1111111111111111111111111111111111111111111111111111111111111111";
    const SOURCE_FINGERPRINT_SHA256: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "v2board-{label}-{}-{}",
                std::process::id(),
                Uuid::new_v4()
            ));
            fs::create_dir(&path).expect("create private report-receipt test root");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .expect("make report-receipt test root private");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct PostgresAnchor {
        journal: ApplyJournal,
        head: ApplyJournalSnapshot,
        permit: DurableTargetMutationPermit,
        history: Vec<ApplyJournalSnapshot>,
    }

    fn v5_spec(root: &Path) -> ProvisionSpec {
        let mut spec =
            crate::manifest::tests::legacy_v5_spec_for_orchestration_operation(OPERATION_ID);
        let ProvisionFlow::LegacyReferenceMigration {
            execution: Some(execution),
            ..
        } = &mut spec.flow
        else {
            panic!("schema-v5 legacy execution");
        };
        execution.receipts.postgres_verification_path =
            Some(root.join("postgres-verification-report.json"));
        execution.receipts.clickhouse_projection_path =
            Some(root.join("clickhouse-projection-report.json"));
        spec
    }

    fn postgres_report(sample: u64) -> String {
        serde_json::to_string(&json!({
            "strategy": "discard_nodes_traffic_details_and_operational_logs",
            "traffic_fold": {
                "operation_id": OPERATION_ID,
                "target_installation_id": INSTALLATION_ID
            },
            "test_sample": sample
        }))
        .expect("serialize PostgreSQL report preimage")
    }

    fn postgres_anchor(root: &Path, report_sha256: &str) -> PostgresAnchor {
        let binding = ApplyJournalBinding::new(OPERATION_ID, INSPECT_REVIEW_SHA256)
            .expect("valid journal binding");
        let (journal, pending) = ApplyJournal::create_pending(root.join("journal"), binding)
            .expect("create report-receipt test journal");
        let mut head = journal.begin(&pending).expect("begin test journal");
        head = journal
            .checkpoint_with_proof(&head, ApplyCheckpoint::MaintenanceFenced, "2".repeat(64))
            .expect("maintenance fenced");
        head = journal
            .checkpoint_with_proof(&head, ApplyCheckpoint::SourceDrained, "3".repeat(64))
            .expect("source drained");
        head = journal
            .record_backup_restore_verified(&head, "4".repeat(64), "5".repeat(64))
            .expect("backup restore verified");
        head = journal
            .record_final_recheck_passed(&head, "6".repeat(64), SOURCE_FINGERPRINT_SHA256)
            .expect("final recheck passed");
        head = journal
            .reserve_installation_identity(&head, INSTALLATION_ID)
            .expect("installation identity reserved");
        head = journal
            .checkpoint_with_proof(&head, ApplyCheckpoint::TargetsBootstrapped, "7".repeat(64))
            .expect("targets bootstrapped");
        head = journal
            .checkpoint_with_proof(&head, ApplyCheckpoint::PostgresBulkCopied, report_sha256)
            .expect("PostgreSQL bulk copied");
        head = journal
            .enter_verification(&head)
            .expect("enter verification");
        let permit = journal
            .target_mutation_permit(&head)
            .expect("PostgreSQL verification permit");
        let history = journal
            .verified_history()
            .expect("verified journal history");
        PostgresAnchor {
            journal,
            head,
            permit,
            history,
        }
    }

    fn expectation<'a>(
        permit: &'a DurableTargetMutationPermit,
        report_sha256: Option<&'a str>,
    ) -> ReceiptExpectation<'a> {
        ReceiptExpectation {
            operation_id: permit.operation_id(),
            installation_id: permit.installation_id(),
            inspect_review_sha256: permit.inspect_review_sha256(),
            source_fingerprint_sha256: permit.source_fingerprint_sha256(),
            maximum_journal_generation: permit.generation(),
            report_sha256,
        }
    }

    #[test]
    fn postgres_receipt_is_owner_only_idempotent_and_conflicts_on_new_preimage() {
        let root = TestRoot::new("postgres-report-receipt");
        let spec = v5_spec(root.path());
        let report_json = postgres_report(1);
        let report_sha256 = legacy_copy_verification_sha256_from_bytes(report_json.as_bytes());
        let anchor = postgres_anchor(root.path(), &report_sha256);

        let first = persist_or_reconcile(
            &spec,
            &anchor.permit,
            &anchor.history,
            DurableReportKind::PostgresVerificationReport,
            report_sha256.clone(),
            report_json.clone(),
            true,
        )
        .expect("persist PostgreSQL report receipt");
        assert_eq!(first, report_sha256);

        let path = receipt_path(&spec, DurableReportKind::PostgresVerificationReport)
            .expect("PostgreSQL receipt path");
        let first_bytes = fs::read(path).expect("read PostgreSQL report receipt");
        let metadata = fs::metadata(path).expect("stat PostgreSQL report receipt");
        assert_eq!(metadata.uid(), 0);
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);

        let retry = persist_or_reconcile(
            &spec,
            &anchor.permit,
            &anchor.history,
            DurableReportKind::PostgresVerificationReport,
            report_sha256.clone(),
            report_json,
            true,
        )
        .expect("reconcile identical PostgreSQL report receipt");
        assert_eq!(retry, report_sha256);
        assert_eq!(fs::read(path).unwrap(), first_bytes);

        let conflicting_json = postgres_report(2);
        let conflicting_sha256 =
            legacy_copy_verification_sha256_from_bytes(conflicting_json.as_bytes());
        assert_ne!(conflicting_sha256, report_sha256);
        assert!(matches!(
            persist_or_reconcile(
                &spec,
                &anchor.permit,
                &anchor.history,
                DurableReportKind::PostgresVerificationReport,
                conflicting_sha256,
                conflicting_json,
                true,
            ),
            Err(DurableReportReceiptError::PostgresReportConflict)
        ));
        assert_eq!(fs::read(path).unwrap(), first_bytes);
    }

    #[test]
    fn report_receipt_rejects_tamper_and_valid_hmac_wrong_binding() {
        let root = TestRoot::new("tampered-report-receipt");
        let spec = v5_spec(root.path());
        let report_json = postgres_report(1);
        let report_sha256 = legacy_copy_verification_sha256_from_bytes(report_json.as_bytes());
        let anchor = postgres_anchor(root.path(), &report_sha256);
        persist_or_reconcile(
            &spec,
            &anchor.permit,
            &anchor.history,
            DurableReportKind::PostgresVerificationReport,
            report_sha256.clone(),
            report_json,
            true,
        )
        .expect("persist report before tamper tests");

        let path = receipt_path(&spec, DurableReportKind::PostgresVerificationReport)
            .expect("PostgreSQL receipt path");
        let original = fs::read(path).expect("read original receipt");
        let mut tampered: DurableReportReceipt =
            serde_json::from_slice(&original).expect("parse original receipt");
        tampered.payload.report_json_utf8.push(' ');
        fs::write(path, serde_json::to_vec(&tampered).unwrap()).expect("write HMAC tamper");
        assert!(matches!(
            load_and_verify(
                &spec,
                &anchor.history,
                DurableReportKind::PostgresVerificationReport,
                expectation(&anchor.permit, Some(&report_sha256)),
            ),
            Err(DurableReportReceiptError::Binding)
        ));

        let mut wrong_binding: DurableReportReceipt =
            serde_json::from_slice(&original).expect("parse original receipt again");
        wrong_binding.payload.installation_id = Uuid::from_u128(3).to_string();
        let payload_bytes = serde_json::to_vec(&wrong_binding.payload).unwrap();
        wrong_binding.hmac_sha256 = spec
            .source_receipt_binding_hmac_sha256(
                LegacyRuntimeReceiptKind::PostgresVerificationReport,
                &payload_bytes,
            )
            .expect("mint test HMAC");
        assert!(spec.verify_source_receipt_binding_hmac_sha256(
            LegacyRuntimeReceiptKind::PostgresVerificationReport,
            &payload_bytes,
            &wrong_binding.hmac_sha256,
        ));
        fs::write(path, serde_json::to_vec(&wrong_binding).unwrap())
            .expect("write valid-HMAC wrong binding");
        assert!(matches!(
            load_and_verify(
                &spec,
                &anchor.history,
                DurableReportKind::PostgresVerificationReport,
                expectation(&anchor.permit, Some(&report_sha256)),
            ),
            Err(DurableReportReceiptError::Binding)
        ));
    }

    #[test]
    fn clickhouse_lost_ack_keeps_original_report_hash() {
        let root = TestRoot::new("clickhouse-report-lost-ack");
        let spec = v5_spec(root.path());
        let postgres_json = postgres_report(1);
        let postgres_sha256 = legacy_copy_verification_sha256_from_bytes(postgres_json.as_bytes());
        let mut anchor = postgres_anchor(root.path(), &postgres_sha256);
        anchor.head = anchor
            .journal
            .checkpoint_with_proof(
                &anchor.head,
                ApplyCheckpoint::PostgresValueVerified,
                &postgres_sha256,
            )
            .expect("PostgreSQL value verified");
        anchor.permit = anchor
            .journal
            .target_mutation_permit(&anchor.head)
            .expect("ClickHouse projection permit");
        anchor.history = anchor
            .journal
            .verified_history()
            .expect("verified ClickHouse journal history");

        let clickhouse_report = |permit: &DurableTargetMutationPermit, sample| {
            serde_json::to_string(&json!({
                "report_version": CLICKHOUSE_SCHEMA_V5_REPORT_VERSION,
                "operation_id": OPERATION_ID,
                "installation_id": INSTALLATION_ID,
                "permit_generation": permit.generation(),
                "permit_event_sha256": permit.event_sha256(),
                "postgres_verification_report_sha256": postgres_sha256,
                "test_sample": sample
            }))
            .expect("serialize ClickHouse report preimage")
        };
        let first_json = clickhouse_report(&anchor.permit, 1);
        let first_sha256 =
            report_sha256_from_bytes(CLICKHOUSE_SCHEMA_V5_REPORT_VERSION, first_json.as_bytes());
        let first = persist_or_reconcile(
            &spec,
            &anchor.permit,
            &anchor.history,
            DurableReportKind::ClickhouseProjectionReport,
            first_sha256.clone(),
            first_json,
            false,
        )
        .expect("persist first ClickHouse report receipt");
        assert_eq!(first, first_sha256);

        let path = receipt_path(&spec, DurableReportKind::ClickhouseProjectionReport)
            .expect("ClickHouse receipt path");
        let first_receipt_bytes = fs::read(path).expect("read first ClickHouse receipt");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("safe ClickHouse receipt name");
        let partial = path.with_file_name(format!(".{name}.partial"));
        fs::hard_link(path, &partial).expect("simulate lost ack after final hard link");
        assert_eq!(fs::metadata(path).unwrap().nlink(), 2);

        let first_generation = anchor.permit.generation();
        anchor.head = anchor
            .journal
            .mark_needs_recovery(
                &anchor.head,
                crate::apply_journal::ApplyOutcomeCode::ProcessInterrupted,
            )
            .expect("record lost acknowledgement");
        anchor.head = anchor
            .journal
            .resume(&anchor.head)
            .expect("resume ClickHouse projection");
        anchor.permit = anchor
            .journal
            .target_mutation_permit(&anchor.head)
            .expect("resumed ClickHouse projection permit");
        anchor.history = anchor
            .journal
            .verified_history()
            .expect("verified resumed ClickHouse history");
        assert!(anchor.permit.generation() > first_generation);

        let retry_json = clickhouse_report(&anchor.permit, 2);
        let retry_sha256 =
            report_sha256_from_bytes(CLICKHOUSE_SCHEMA_V5_REPORT_VERSION, retry_json.as_bytes());
        assert_ne!(retry_sha256, first_sha256);
        let retry = persist_or_reconcile(
            &spec,
            &anchor.permit,
            &anchor.history,
            DurableReportKind::ClickhouseProjectionReport,
            retry_sha256,
            retry_json,
            false,
        )
        .expect("reconcile ClickHouse lost-ack receipt");

        assert_eq!(retry, first_sha256);
        assert_eq!(fs::read(path).unwrap(), first_receipt_bytes);
        assert!(!partial.exists());
        assert_eq!(fs::metadata(path).unwrap().nlink(), 1);
    }
}
