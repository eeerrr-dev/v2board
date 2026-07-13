use std::collections::BTreeSet;

use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    ProvisionKind, ProvisionSpec,
    apply_journal::{
        ApplyCheckpoint, ApplyJournalSnapshot, ApplyJournalState, ApplyOutcomeCode,
        DurableTargetMutationPermit, NativeAuthorityBinding, backup_reference_sha256,
    },
    legacy_converter::{
        DERIVED_MAPPINGS, LegacyConversionStrategy, TABLE_MAPPINGS, TARGET_ONLY_TABLES,
        TARGET_POSTGRES_LINEAGE_SHA256, registry_sha256_for_strategy,
        target_postgres_lineage_sha256,
    },
};

static TARGET_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationAuditBinding {
    authorized_snapshot_report_sha256: String,
    authorized_snapshot_report_binding_hmac_sha256: String,
    authorization_binding_hmac_sha256: String,
    authorization_file_sha256: String,
}

impl AuthorizationAuditBinding {
    pub fn new(
        authorized_snapshot_report_sha256: impl Into<String>,
        authorized_snapshot_report_binding_hmac_sha256: impl Into<String>,
        authorization_binding_hmac_sha256: impl Into<String>,
        authorization_file_sha256: impl Into<String>,
    ) -> Result<Self, LifecycleLedgerError> {
        let binding = Self {
            authorized_snapshot_report_sha256: authorized_snapshot_report_sha256.into(),
            authorized_snapshot_report_binding_hmac_sha256:
                authorized_snapshot_report_binding_hmac_sha256.into(),
            authorization_binding_hmac_sha256: authorization_binding_hmac_sha256.into(),
            authorization_file_sha256: authorization_file_sha256.into(),
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn authorized_snapshot_report_sha256(&self) -> &str {
        &self.authorized_snapshot_report_sha256
    }

    pub fn authorized_snapshot_report_binding_hmac_sha256(&self) -> &str {
        &self.authorized_snapshot_report_binding_hmac_sha256
    }

    pub fn authorization_binding_hmac_sha256(&self) -> &str {
        &self.authorization_binding_hmac_sha256
    }

    pub fn authorization_file_sha256(&self) -> &str {
        &self.authorization_file_sha256
    }

    fn validate(&self) -> Result<(), LifecycleLedgerError> {
        if [
            &self.authorized_snapshot_report_sha256,
            &self.authorized_snapshot_report_binding_hmac_sha256,
            &self.authorization_binding_hmac_sha256,
            &self.authorization_file_sha256,
        ]
        .into_iter()
        .any(|value| !is_lower_sha256(value))
        {
            return Err(LifecycleLedgerError::InvalidAuthorizationAuditBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleLedgerBinding {
    source_fingerprint_sha256: String,
    backup_reference: String,
    authorization_audit: AuthorizationAuditBinding,
}

impl LifecycleLedgerBinding {
    pub fn new(
        source_fingerprint_sha256: impl Into<String>,
        backup_reference: impl Into<String>,
        authorization_audit: AuthorizationAuditBinding,
    ) -> Result<Self, LifecycleLedgerError> {
        let source_fingerprint_sha256 = source_fingerprint_sha256.into();
        let backup_reference = backup_reference.into();
        let binding = Self {
            source_fingerprint_sha256,
            backup_reference,
            authorization_audit,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn source_fingerprint_sha256(&self) -> &str {
        &self.source_fingerprint_sha256
    }

    pub fn backup_reference(&self) -> &str {
        &self.backup_reference
    }

    pub fn authorization_audit(&self) -> &AuthorizationAuditBinding {
        &self.authorization_audit
    }

    fn validate(&self) -> Result<(), LifecycleLedgerError> {
        if !is_lower_sha256(&self.source_fingerprint_sha256) {
            return Err(LifecycleLedgerError::InvalidSourceFingerprint);
        }
        if !valid_reference(&self.backup_reference) {
            return Err(LifecycleLedgerError::InvalidBackupReference);
        }
        self.authorization_audit.validate()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionProofBinding {
    data_verification_report_sha256: String,
    analytics_projection_report_sha256: String,
    node_cutover_report_sha256: String,
    native_runtime_report_sha256: String,
    native_runtime_running_and_verified: bool,
    cold_archive_reference: String,
    cold_archive_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeActivationProofBinding {
    data_verification_report_sha256: String,
    analytics_projection_report_sha256: String,
    node_cutover_report_sha256: String,
}

impl NativeActivationProofBinding {
    pub fn new(
        data_verification_report_sha256: impl Into<String>,
        analytics_projection_report_sha256: impl Into<String>,
        node_cutover_report_sha256: impl Into<String>,
    ) -> Result<Self, LifecycleLedgerError> {
        let binding = Self {
            data_verification_report_sha256: data_verification_report_sha256.into(),
            analytics_projection_report_sha256: analytics_projection_report_sha256.into(),
            node_cutover_report_sha256: node_cutover_report_sha256.into(),
        };
        binding.validate()?;
        Ok(binding)
    }

    fn validate(&self) -> Result<(), LifecycleLedgerError> {
        if [
            &self.data_verification_report_sha256,
            &self.analytics_projection_report_sha256,
            &self.node_cutover_report_sha256,
        ]
        .into_iter()
        .any(|value| !is_lower_sha256(value))
        {
            return Err(LifecycleLedgerError::InvalidActivationProof);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeActivationCommit {
    operation_id: Uuid,
    installation_id: Uuid,
    journal_generation: u64,
    journal_event_sha256: String,
    activated_at_unix: i64,
    native_authority: NativeAuthorityBinding,
}

impl NativeActivationCommit {
    pub const fn operation_id(&self) -> Uuid {
        self.operation_id
    }

    pub const fn installation_id(&self) -> Uuid {
        self.installation_id
    }

    pub const fn journal_generation(&self) -> u64 {
        self.journal_generation
    }

    pub fn journal_event_sha256(&self) -> &str {
        &self.journal_event_sha256
    }

    pub const fn activated_at_unix(&self) -> i64 {
        self.activated_at_unix
    }

    pub fn native_authority_binding(&self) -> &NativeAuthorityBinding {
        &self.native_authority
    }
}

impl CompletionProofBinding {
    pub fn new(
        data_verification_report_sha256: impl Into<String>,
        analytics_projection_report_sha256: impl Into<String>,
        node_cutover_report_sha256: impl Into<String>,
        native_runtime_report_sha256: impl Into<String>,
        native_runtime_running_and_verified: bool,
        cold_archive_reference: impl Into<String>,
        cold_archive_sha256: impl Into<String>,
    ) -> Result<Self, LifecycleLedgerError> {
        let cold_archive_reference = cold_archive_reference.into();
        let binding = Self {
            data_verification_report_sha256: data_verification_report_sha256.into(),
            analytics_projection_report_sha256: analytics_projection_report_sha256.into(),
            node_cutover_report_sha256: node_cutover_report_sha256.into(),
            native_runtime_report_sha256: native_runtime_report_sha256.into(),
            native_runtime_running_and_verified,
            cold_archive_reference,
            cold_archive_sha256: cold_archive_sha256.into(),
        };
        binding.validate()?;
        Ok(binding)
    }

    fn validate(&self) -> Result<(), LifecycleLedgerError> {
        if [
            &self.data_verification_report_sha256,
            &self.analytics_projection_report_sha256,
            &self.node_cutover_report_sha256,
            &self.native_runtime_report_sha256,
            &self.cold_archive_sha256,
        ]
        .into_iter()
        .any(|value| !is_lower_sha256(value))
            || !self.native_runtime_running_and_verified
        {
            return Err(LifecycleLedgerError::InvalidCompletionProof);
        }
        if !valid_reference(&self.cold_archive_reference) {
            return Err(LifecycleLedgerError::InvalidCompletionProof);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LifecycleLedgerError {
    #[error("lifecycle ledger supports only legacy_reference_migration")]
    WrongProvisionKind,
    #[error("target mutation permit does not bind the manifest and journal history")]
    BindingMismatch,
    #[error("source fingerprint must be a lowercase SHA-256")]
    InvalidSourceFingerprint,
    #[error("backup reference must be a bounded non-secret opaque identifier")]
    InvalidBackupReference,
    #[error("authorization audit identity must contain four lowercase SHA-256 values")]
    InvalidAuthorizationAuditBinding,
    #[error("completion proof contains an invalid hash or archive reference")]
    InvalidCompletionProof,
    #[error("native activation proof contains an invalid verification report hash")]
    InvalidActivationProof,
    #[error("native activation requires the current verifying/nodes_verified journal event")]
    ActivationCheckpointRequired,
    #[error("target PostgreSQL schema contains unexpected or missing tables")]
    UnexpectedTargetSchema,
    #[error("embedded PostgreSQL migration lineage does not match its frozen SHA-256 binding")]
    TargetLineageBindingMismatch,
    #[error("target PostgreSQL already contains conflicting lifecycle state")]
    ConflictingTargetState,
    #[error("PostgreSQL refused synchronous_commit=on for a durable lifecycle transaction")]
    DurabilitySettingRejected,
    #[error("journal history is empty, non-contiguous, or does not end at the mutation permit")]
    InvalidJournalHistory,
    #[error("completed journal snapshots require the completion ledger path")]
    CompletionPathRequired,
    #[error("PostgreSQL lifecycle migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("PostgreSQL lifecycle ledger failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("converter registry is invalid: {0}")]
    Converter(#[from] crate::legacy_converter::ConverterError),
    #[error("PostgreSQL runtime grant policy failed: {0}")]
    RuntimeGrants(#[from] crate::postgres_runtime_grants::PostgresRuntimeGrantError),
}

/// Applies the independent PostgreSQL migration lineage only to an empty target, or
/// verifies the exact lineage after a crash between schema commit and ledger
/// creation. The mutation permit is required before any DDL is attempted.
pub async fn bootstrap_postgres_schema(
    pool: &PgPool,
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
) -> Result<(), LifecycleLedgerError> {
    validate_permit(spec, permit)?;
    verify_target_lineage_binding()?;
    require_public_schema(pool).await?;
    let before = target_table_names(pool).await?;
    let migration_ledger_only = BTreeSet::from(["_sqlx_migrations".to_string()]);
    if before == expected_target_tables() {
        if !v2board_db::migrations_current(pool).await? {
            return Err(LifecycleLedgerError::UnexpectedTargetSchema);
        }
        crate::postgres_runtime_grants::apply_frozen_runtime_grants(pool, spec, permit).await?;
        return Ok(());
    }
    if !before.is_empty() && before != migration_ledger_only {
        // Each migration is transactional. Any partial application-table set,
        // or an unrelated table beside SQLx's ledger, is therefore drift rather
        // than a resumable migration-lineage bootstrap state.
        return Err(LifecycleLedgerError::UnexpectedTargetSchema);
    }
    let mut connection = pool.acquire().await?;
    sqlx::query("SET SESSION synchronous_commit = 'on'")
        .execute(&mut *connection)
        .await?;
    let synchronous_commit: String =
        sqlx::query_scalar("SELECT current_setting('synchronous_commit')")
            .fetch_one(&mut *connection)
            .await?;
    if synchronous_commit != "on" {
        return Err(LifecycleLedgerError::DurabilitySettingRejected);
    }
    TARGET_MIGRATOR.run(&mut *connection).await?;
    drop(connection);
    if !v2board_db::migrations_current(pool).await? {
        return Err(LifecycleLedgerError::UnexpectedTargetSchema);
    }
    let after = target_table_names(pool).await?;
    if after != expected_target_tables() {
        return Err(LifecycleLedgerError::UnexpectedTargetSchema);
    }
    crate::postgres_runtime_grants::apply_frozen_runtime_grants(pool, spec, permit).await?;
    Ok(())
}

/// Creates the pending installation and mirrors the complete verified
/// filesystem history after PostgreSQL bootstrap. Every conflict is compared;
/// retries never adopt a different operation.
pub async fn bootstrap_lifecycle_ledger(
    pool: &PgPool,
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    history: &[ApplyJournalSnapshot],
    binding: &LifecycleLedgerBinding,
) -> Result<(), LifecycleLedgerError> {
    validate_permit(spec, permit)?;
    binding.validate()?;
    validate_ledger_binding(permit, binding)?;
    verify_bootstrapped_schema(pool).await?;
    validate_history(permit, history)?;
    let head = history
        .last()
        .ok_or(LifecycleLedgerError::InvalidJournalHistory)?;
    let operation_id = parse_uuid(permit.operation_id())?;
    let installation_id = parse_uuid(permit.installation_id())?;
    let created_at = unix_seconds(history[0].recorded_at_unix_ms())?;
    let updated_at = unix_seconds(head.recorded_at_unix_ms())?;
    let converter_registry_sha256 = converter_registry_sha256_for_spec(spec)?;

    let mut tx = begin_durable_transaction(pool).await?;
    let installation_inserted = sqlx::query(
        r#"
        INSERT INTO v2_system_installation (
            singleton, installation_id, lineage, state, created_at, activated_at,
            source_fingerprint_sha256
        ) VALUES (1, $1, 'legacy_migrated', 'pending', $2, NULL, $3)
        ON CONFLICT (singleton) DO NOTHING
        "#,
    )
    .bind(installation_id)
    .bind(created_at)
    .bind(&binding.source_fingerprint_sha256)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if installation_inserted == 0
        && !installation_matches(
            &mut tx,
            installation_id,
            created_at,
            &binding.source_fingerprint_sha256,
        )
        .await?
    {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }

    let operation_inserted = sqlx::query(
        r#"
        INSERT INTO v2_lifecycle_operation (
            operation_id, installation_id, kind, manifest_binding_hmac_sha256,
            inspect_review_sha256, authorized_snapshot_report_sha256,
            authorized_snapshot_report_binding_hmac_sha256,
            authorization_binding_hmac_sha256, authorization_file_sha256,
            source_fingerprint_sha256,
            converter_registry_sha256, target_lineage_sha256, state, checkpoint,
            journal_generation, journal_event_sha256, checkpoint_proof_sha256, backup_reference,
            backup_restore_proof_sha256, final_recheck_report_sha256,
            created_at, updated_at
        ) VALUES (
            $1, $2, 'legacy_reference_migration', $3, $4, $5, $6, $7, $8,
            $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
        )
        ON CONFLICT (operation_id) DO NOTHING
        "#,
    )
    .bind(operation_id)
    .bind(installation_id)
    .bind(spec.manifest_binding_hmac_sha256())
    .bind(permit.inspect_review_sha256())
    .bind(
        &binding
            .authorization_audit
            .authorized_snapshot_report_sha256,
    )
    .bind(
        &binding
            .authorization_audit
            .authorized_snapshot_report_binding_hmac_sha256,
    )
    .bind(
        &binding
            .authorization_audit
            .authorization_binding_hmac_sha256,
    )
    .bind(&binding.authorization_audit.authorization_file_sha256)
    .bind(&binding.source_fingerprint_sha256)
    .bind(&converter_registry_sha256)
    .bind(TARGET_POSTGRES_LINEAGE_SHA256)
    .bind(state_text(head.state()))
    .bind(checkpoint_code(head.checkpoint()))
    .bind(i64_generation(head.generation())?)
    .bind(head.event_sha256())
    .bind(head.checkpoint_proof_sha256())
    .bind(&binding.backup_reference)
    .bind(head.backup_restore_proof_sha256())
    .bind(head.final_recheck_report_sha256())
    .bind(created_at)
    .bind(updated_at)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if operation_inserted == 0
        && !operation_head_matches(
            &mut tx,
            operation_id,
            installation_id,
            spec,
            permit,
            binding,
            &converter_registry_sha256,
            head,
            created_at,
            updated_at,
        )
        .await?
    {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    for snapshot in history {
        mirror_event_insert(&mut tx, operation_id, snapshot).await?;
    }
    let mirrored_event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2_lifecycle_event WHERE operation_id = $1")
            .bind(operation_id)
            .fetch_one(&mut *tx)
            .await?;
    let expected_event_count = u64::try_from(history.len())
        .map_err(|_| LifecycleLedgerError::InvalidJournalHistory)
        .and_then(i64_generation)?;
    if mirrored_event_count != expected_event_count {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    tx.commit().await?;
    Ok(())
}

/// Mirrors one newly fsync-durable, non-completed journal head with a locked
/// compare-and-swap against the permanent PostgreSQL head.
pub async fn mirror_lifecycle_snapshot(
    pool: &PgPool,
    spec: &ProvisionSpec,
    snapshot: &ApplyJournalSnapshot,
    authorization_audit: &AuthorizationAuditBinding,
) -> Result<(), LifecycleLedgerError> {
    authorization_audit.validate()?;
    if snapshot.state() == ApplyJournalState::Completed {
        return Err(LifecycleLedgerError::CompletionPathRequired);
    }
    if !snapshot_checkpoint_proof_is_valid(snapshot) {
        return Err(LifecycleLedgerError::InvalidJournalHistory);
    }
    verify_bootstrapped_schema(pool).await?;
    let operation_id = validate_snapshot_binding(spec, snapshot)?;
    let converter_registry_sha256 = converter_registry_sha256_for_spec(spec)?;
    let mut tx = begin_durable_transaction(pool).await?;
    let head = lock_operation_head(&mut tx, operation_id).await?;
    if !operation_binding_matches(
        &head,
        spec,
        snapshot,
        &converter_registry_sha256,
        authorization_audit,
    ) {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    validate_snapshot_native_authority(&mut tx, operation_id, snapshot).await?;
    if head.journal_generation == i64_generation(snapshot.generation())?
        && head.journal_event_sha256 == snapshot.event_sha256()
    {
        if !snapshot_matches_operation_head(&head, snapshot)? {
            return Err(LifecycleLedgerError::ConflictingTargetState);
        }
        mirror_event_insert(&mut tx, operation_id, snapshot).await?;
        tx.commit().await?;
        return Ok(());
    }
    if i64_generation(snapshot.generation())? != head.journal_generation + 1
        || snapshot.previous_event_sha256() != Some(head.journal_event_sha256.as_str())
        || unix_seconds(snapshot.recorded_at_unix_ms())? < head.updated_at
    {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    mirror_event_insert(&mut tx, operation_id, snapshot).await?;
    let changed = sqlx::query(
        r#"
        UPDATE v2_lifecycle_operation
        SET state = $1, checkpoint = $2, journal_generation = $3,
            journal_event_sha256 = $4, checkpoint_proof_sha256 = $5,
            native_authority_nodes_generation = $6,
            native_authority_nodes_event_sha256 = $7,
            data_verification_report_sha256 = $8,
            analytics_projection_report_sha256 = $9,
            node_cutover_report_sha256 = $10, updated_at = $11
        WHERE operation_id = $12 AND journal_generation = $13 AND journal_event_sha256 = $14
        "#,
    )
    .bind(state_text(snapshot.state()))
    .bind(checkpoint_code(snapshot.checkpoint()))
    .bind(i64_generation(snapshot.generation())?)
    .bind(snapshot.event_sha256())
    .bind(snapshot.checkpoint_proof_sha256())
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| i64_generation(binding.nodes_verified_generation()))
            .transpose()?,
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.nodes_verified_event_sha256().to_string()),
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.data_verification_report_sha256().to_string()),
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.analytics_projection_report_sha256().to_string()),
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.node_cutover_report_sha256().to_string()),
    )
    .bind(unix_seconds(snapshot.recorded_at_unix_ms())?)
    .bind(operation_id)
    .bind(head.journal_generation)
    .bind(&head.journal_event_sha256)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    tx.commit().await?;
    Ok(())
}

/// Binds the already-mirrored, fsync-durable `nodes_verified` journal head in an
/// append-only activation commit and activates the native installation in the
/// same transaction. The operation row is deliberately not advanced here:
/// `cutover_committed` remains a future filesystem journal event written only
/// after both native units are ready.
pub async fn commit_native_activation(
    pool: &PgPool,
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    snapshot: &ApplyJournalSnapshot,
    proof: &NativeActivationProofBinding,
    authorization_audit: &AuthorizationAuditBinding,
) -> Result<NativeActivationCommit, LifecycleLedgerError> {
    validate_permit(spec, permit)?;
    proof.validate()?;
    authorization_audit.validate()?;
    verify_bootstrapped_schema(pool).await?;
    if snapshot.state() != ApplyJournalState::Verifying
        || snapshot.checkpoint() != ApplyCheckpoint::NodesVerified
        || snapshot.outcome_code().is_some()
        || snapshot.binding().operation_id() != permit.operation_id()
        || snapshot.binding().inspect_review_sha256() != permit.inspect_review_sha256()
        || snapshot.generation() != permit.generation()
        || snapshot.event_sha256() != permit.event_sha256()
        || snapshot.installation_id() != Some(permit.installation_id())
        || snapshot.backup_restore_proof_sha256() != Some(permit.backup_restore_proof_sha256())
        || snapshot.backup_reference_sha256() != Some(permit.backup_reference_sha256())
        || snapshot.final_recheck_report_sha256() != Some(permit.final_recheck_report_sha256())
        || snapshot.source_fingerprint_sha256() != Some(permit.source_fingerprint_sha256())
        || snapshot.checkpoint_proof_sha256() != Some(proof.node_cutover_report_sha256.as_str())
        || snapshot.native_authority_binding().is_some()
    {
        return Err(LifecycleLedgerError::ActivationCheckpointRequired);
    }

    let operation_id = validate_snapshot_binding(spec, snapshot)?;
    let installation_id = parse_uuid(permit.installation_id())?;
    let converter_registry_sha256 = converter_registry_sha256_for_spec(spec)?;
    let mut tx = begin_durable_transaction(pool).await?;
    let head = lock_operation_head(&mut tx, operation_id).await?;
    if !operation_binding_matches(
        &head,
        spec,
        snapshot,
        &converter_registry_sha256,
        authorization_audit,
    ) {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    let installation = lock_installation(&mut tx, installation_id).await?;
    if !installation_binding_matches(&installation, &head) {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    if !snapshot_matches_operation_head(&head, snapshot)? || verification_fields_are_set(&head) {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    if !activation_stage_proofs_match(&mut tx, operation_id, snapshot, proof).await? {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    // A successful ordinary mirror is a prerequisite, not part of this
    // transaction. This no-op compare proves the referenced append-only event
    // row is present and byte-for-byte equal to the filesystem snapshot.
    mirror_event_insert(&mut tx, operation_id, snapshot).await?;
    let existing_commit = load_activation_commit(&mut tx, operation_id).await?;
    let activated_at_unix = match (
        installation.lineage.as_str(),
        installation.state.as_str(),
        installation.activated_at,
        existing_commit,
    ) {
        ("legacy_migrated", "pending", None, None) => {
            let minimum_activation_time = unix_seconds(snapshot.recorded_at_unix_ms())?;
            let committed_at = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT GREATEST(
                    $1,
                    FLOOR(EXTRACT(EPOCH FROM clock_timestamp()))::BIGINT
                )
                "#,
            )
            .bind(minimum_activation_time)
            .fetch_one(&mut *tx)
            .await?;
            insert_activation_commit(
                &mut tx,
                operation_id,
                installation_id,
                snapshot,
                proof,
                committed_at,
            )
            .await?;
            let activated_at = sqlx::query_scalar::<_, i64>(
                r#"
                UPDATE v2_system_installation
                SET lineage = 'native', state = 'active', activated_at = $1
                WHERE singleton = 1 AND installation_id = $2
                  AND lineage = 'legacy_migrated' AND state = 'pending'
                  AND activated_at IS NULL AND source_fingerprint_sha256 = $3
                RETURNING activated_at
                "#,
            )
            .bind(committed_at)
            .bind(installation_id)
            .bind(&head.source_fingerprint_sha256)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(LifecycleLedgerError::ConflictingTargetState)?;
            if activated_at != committed_at {
                return Err(LifecycleLedgerError::ConflictingTargetState);
            }
            activated_at
        }
        ("native", "active", Some(activated_at), Some(commit))
            if activation_commit_matches(&commit, installation_id, snapshot, proof)
                && commit.committed_at == activated_at =>
        {
            activated_at
        }
        _ => return Err(LifecycleLedgerError::ConflictingTargetState),
    };
    tx.commit().await?;
    let journal_generation = snapshot.generation();
    let native_authority = NativeAuthorityBinding::new(
        journal_generation,
        snapshot.event_sha256(),
        &proof.data_verification_report_sha256,
        &proof.analytics_projection_report_sha256,
        &proof.node_cutover_report_sha256,
    )
    .map_err(|_| LifecycleLedgerError::InvalidActivationProof)?;
    Ok(NativeActivationCommit {
        operation_id,
        installation_id,
        journal_generation,
        journal_event_sha256: snapshot.event_sha256().to_string(),
        activated_at_unix,
        native_authority,
    })
}

/// Observes an already-durable activation commit without locking or mutating
/// PostgreSQL. This is the recovery path for an indeterminate commit response:
/// callers may fsync the returned authority binding, but cannot use it as a
/// pre-commit mutation permit.
pub async fn observe_native_activation_commit(
    pool: &PgPool,
    spec: &ProvisionSpec,
    snapshot: &ApplyJournalSnapshot,
    authorization_audit: &AuthorizationAuditBinding,
) -> Result<Option<NativeActivationCommit>, LifecycleLedgerError> {
    authorization_audit.validate()?;
    verify_bootstrapped_schema(pool).await?;
    if snapshot.checkpoint() != ApplyCheckpoint::NodesVerified
        || !matches!(
            snapshot.state(),
            ApplyJournalState::Verifying | ApplyJournalState::NeedsRecovery
        )
        || snapshot.installation_id().is_none()
        || snapshot.native_authority_binding().is_some()
        || !snapshot_checkpoint_proof_is_valid(snapshot)
    {
        return Err(LifecycleLedgerError::ActivationCheckpointRequired);
    }
    let operation_id = validate_snapshot_binding(spec, snapshot)?;
    let converter_registry_sha256 = converter_registry_sha256_for_spec(spec)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let head = load_operation_head_readonly(&mut tx, operation_id).await?;
    if head.checkpoint != checkpoint_code(ApplyCheckpoint::NodesVerified)
        || head.journal_generation > i64_generation(snapshot.generation())?
        || !operation_binding_matches(
            &head,
            spec,
            snapshot,
            &converter_registry_sha256,
            authorization_audit,
        )
    {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    let installation_id = snapshot
        .installation_id()
        .ok_or(LifecycleLedgerError::ActivationCheckpointRequired)?;
    let installation = load_installation_readonly(&mut tx, parse_uuid(installation_id)?).await?;
    if !installation_binding_matches(&installation, &head) {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    let Some(commit) = load_activation_commit(&mut tx, operation_id).await? else {
        if installation.lineage != "legacy_migrated"
            || installation.state != "pending"
            || installation.activated_at.is_some()
        {
            return Err(LifecycleLedgerError::ConflictingTargetState);
        }
        tx.rollback().await?;
        return Ok(None);
    };
    let proof = NativeActivationProofBinding::new(
        &commit.data_verification_report_sha256,
        &commit.analytics_projection_report_sha256,
        &commit.node_cutover_report_sha256,
    )?;
    if installation.lineage != "native"
        || installation.state != "active"
        || installation.activated_at != Some(commit.committed_at)
        || commit.installation_id != installation.installation_id
        || commit.journal_generation > i64_generation(snapshot.generation())?
        || !activation_stage_proofs_match(&mut tx, operation_id, snapshot, &proof).await?
    {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    let authority = NativeAuthorityBinding::new(
        u64::try_from(commit.journal_generation)
            .map_err(|_| LifecycleLedgerError::ConflictingTargetState)?,
        &commit.journal_event_sha256,
        &commit.data_verification_report_sha256,
        &commit.analytics_projection_report_sha256,
        &commit.node_cutover_report_sha256,
    )
    .map_err(|_| LifecycleLedgerError::ConflictingTargetState)?;
    let anchor_matches: bool = sqlx::query_scalar(
        "SELECT COUNT(*) = 1 FROM v2_lifecycle_event \
         WHERE operation_id = $1 AND generation = $2 AND event_sha256 = $3 \
           AND state = 'verifying' AND checkpoint = 11 AND outcome_code IS NULL \
           AND installation_id = $4 AND checkpoint_proof_sha256 = $5 \
           AND native_authority_nodes_generation IS NULL \
           AND native_authority_nodes_event_sha256 IS NULL \
           AND data_verification_report_sha256 IS NULL \
           AND analytics_projection_report_sha256 IS NULL \
           AND node_cutover_report_sha256 IS NULL",
    )
    .bind(operation_id)
    .bind(commit.journal_generation)
    .bind(&commit.journal_event_sha256)
    .bind(commit.installation_id)
    .bind(&commit.node_cutover_report_sha256)
    .fetch_one(&mut *tx)
    .await?;
    if !anchor_matches {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    let receipt = NativeActivationCommit {
        operation_id,
        installation_id: commit.installation_id,
        journal_generation: authority.nodes_verified_generation(),
        journal_event_sha256: authority.nodes_verified_event_sha256().to_string(),
        activated_at_unix: commit.committed_at,
        native_authority: authority,
    };
    tx.rollback().await?;
    Ok(Some(receipt))
}

/// Completes the permanent ledger only after the filesystem journal has
/// reached `completed` and the source-retirement proof is available.
pub async fn complete_lifecycle_ledger(
    pool: &PgPool,
    spec: &ProvisionSpec,
    snapshot: &ApplyJournalSnapshot,
    proof: &CompletionProofBinding,
    authorization_audit: &AuthorizationAuditBinding,
) -> Result<(), LifecycleLedgerError> {
    proof.validate()?;
    authorization_audit.validate()?;
    verify_bootstrapped_schema(pool).await?;
    if snapshot.state() != ApplyJournalState::Completed
        || snapshot.checkpoint() != ApplyCheckpoint::CompletionVerified
        || !snapshot_checkpoint_proof_is_valid(snapshot)
        || snapshot.native_authority_binding().is_none()
    {
        return Err(LifecycleLedgerError::InvalidJournalHistory);
    }
    let operation_id = validate_snapshot_binding(spec, snapshot)?;
    let converter_registry_sha256 = converter_registry_sha256_for_spec(spec)?;
    let mut tx = begin_durable_transaction(pool).await?;
    let head = lock_operation_head(&mut tx, operation_id).await?;
    let installation = lock_installation(&mut tx, head.installation_id).await?;
    let activation_commit = load_activation_commit(&mut tx, operation_id)
        .await?
        .ok_or(LifecycleLedgerError::ConflictingTargetState)?;
    if !operation_binding_matches(
        &head,
        spec,
        snapshot,
        &converter_registry_sha256,
        authorization_audit,
    ) || !installation_binding_matches(&installation, &head)
        || installation.lineage != "native"
        || installation.state != "active"
        || installation.activated_at != Some(activation_commit.committed_at)
        || !snapshot_authority_matches_commit(snapshot, &activation_commit)
        || !activation_commit_matches_completion(
            &activation_commit,
            head.installation_id,
            i64_generation(snapshot.generation())?,
            proof,
        )
    {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    if head.journal_generation == i64_generation(snapshot.generation())?
        && head.journal_event_sha256 == snapshot.event_sha256()
    {
        if !completed_head_matches(&head, snapshot, proof)? {
            return Err(LifecycleLedgerError::ConflictingTargetState);
        }
        mirror_event_insert(&mut tx, operation_id, snapshot).await?;
        tx.commit().await?;
        return Ok(());
    }
    if i64_generation(snapshot.generation())? != head.journal_generation + 1
        || snapshot.previous_event_sha256() != Some(head.journal_event_sha256.as_str())
        || head.state != "verifying"
        || head.checkpoint != checkpoint_code(ApplyCheckpoint::SourceRetired)
        || retirement_fields_are_set(&head)
        || unix_seconds(snapshot.recorded_at_unix_ms())? < head.updated_at
    {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    mirror_event_insert(&mut tx, operation_id, snapshot).await?;
    let completed_at = unix_seconds(snapshot.recorded_at_unix_ms())?;
    let changed = sqlx::query(
        r#"
        UPDATE v2_lifecycle_operation
        SET state = 'completed', checkpoint = 15, journal_generation = $1,
            journal_event_sha256 = $2, checkpoint_proof_sha256 = $3,
            data_verification_report_sha256 = $4,
            analytics_projection_report_sha256 = $5, node_cutover_report_sha256 = $6,
            source_retired = TRUE, mysql_reachable = FALSE,
            source_redis_reachable = FALSE, source_access_permanently_disabled = TRUE,
            legacy_runtime_compat = FALSE,
            cold_archive_reference = $7, cold_archive_sha256 = $8,
            updated_at = $9, completed_at = $9
        WHERE operation_id = $10 AND journal_generation = $11 AND journal_event_sha256 = $12
        "#,
    )
    .bind(i64_generation(snapshot.generation())?)
    .bind(snapshot.event_sha256())
    .bind(snapshot.checkpoint_proof_sha256())
    .bind(&proof.data_verification_report_sha256)
    .bind(&proof.analytics_projection_report_sha256)
    .bind(&proof.node_cutover_report_sha256)
    .bind(&proof.cold_archive_reference)
    .bind(&proof.cold_archive_sha256)
    .bind(completed_at)
    .bind(operation_id)
    .bind(head.journal_generation)
    .bind(&head.journal_event_sha256)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    tx.commit().await?;
    Ok(())
}

fn retirement_fields_are_set(head: &OperationHeadRow) -> bool {
    head.source_retired.is_some()
        || head.mysql_reachable.is_some()
        || head.source_redis_reachable.is_some()
        || head.source_access_permanently_disabled.is_some()
        || head.legacy_runtime_compat.is_some()
        || head.cold_archive_reference.is_some()
        || head.cold_archive_sha256.is_some()
        || head.completed_at.is_some()
}

fn completed_head_matches(
    head: &OperationHeadRow,
    snapshot: &ApplyJournalSnapshot,
    proof: &CompletionProofBinding,
) -> Result<bool, LifecycleLedgerError> {
    Ok(snapshot_matches_operation_head(head, snapshot)?
        && head.state == "completed"
        && head.checkpoint == checkpoint_code(ApplyCheckpoint::CompletionVerified)
        && head.data_verification_report_sha256.as_deref()
            == Some(proof.data_verification_report_sha256.as_str())
        && head.analytics_projection_report_sha256.as_deref()
            == Some(proof.analytics_projection_report_sha256.as_str())
        && head.node_cutover_report_sha256.as_deref()
            == Some(proof.node_cutover_report_sha256.as_str())
        && head.source_retired == Some(true)
        && head.mysql_reachable == Some(false)
        && head.source_redis_reachable == Some(false)
        && head.source_access_permanently_disabled == Some(true)
        && head.legacy_runtime_compat == Some(false)
        && head.cold_archive_reference.as_deref() == Some(proof.cold_archive_reference.as_str())
        && head.cold_archive_sha256.as_deref() == Some(proof.cold_archive_sha256.as_str())
        && head.completed_at == Some(unix_seconds(snapshot.recorded_at_unix_ms())?))
}

fn validate_permit(
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
) -> Result<(), LifecycleLedgerError> {
    if spec.kind != ProvisionKind::LegacyReferenceMigration {
        return Err(LifecycleLedgerError::WrongProvisionKind);
    }
    if permit.operation_id() != spec.operation_id
        || parse_uuid(&spec.operation_id).is_err()
        || parse_uuid(permit.installation_id()).is_err()
        || !is_lower_sha256(spec.manifest_binding_hmac_sha256())
        || !is_lower_sha256(permit.inspect_review_sha256())
        || !is_lower_sha256(permit.event_sha256())
        || !is_lower_sha256(permit.backup_restore_proof_sha256())
        || !is_lower_sha256(permit.backup_reference_sha256())
        || !is_lower_sha256(permit.final_recheck_report_sha256())
        || !is_lower_sha256(permit.source_fingerprint_sha256())
    {
        return Err(LifecycleLedgerError::BindingMismatch);
    }
    Ok(())
}

fn validate_ledger_binding(
    permit: &DurableTargetMutationPermit,
    binding: &LifecycleLedgerBinding,
) -> Result<(), LifecycleLedgerError> {
    if binding.source_fingerprint_sha256 != permit.source_fingerprint_sha256()
        || backup_reference_sha256(&binding.backup_reference)
            .map_err(|_| LifecycleLedgerError::BindingMismatch)?
            != permit.backup_reference_sha256()
    {
        return Err(LifecycleLedgerError::BindingMismatch);
    }
    Ok(())
}

fn validate_history(
    permit: &DurableTargetMutationPermit,
    history: &[ApplyJournalSnapshot],
) -> Result<(), LifecycleLedgerError> {
    let Some(head) = history.last() else {
        return Err(LifecycleLedgerError::InvalidJournalHistory);
    };
    if history[0].generation() != 0
        || history[0].state() != ApplyJournalState::Pending
        || history[0].checkpoint() != ApplyCheckpoint::PendingDurable
        || history[0].previous_event_sha256().is_some()
        || history[0].outcome_code().is_some()
        || history[0].installation_id().is_some()
        || history[0].backup_restore_proof_sha256().is_some()
        || history[0].backup_reference_sha256().is_some()
        || history[0].final_recheck_report_sha256().is_some()
        || history[0].source_fingerprint_sha256().is_some()
        || history[0].checkpoint_proof_sha256().is_some()
        || history[0].native_authority_binding().is_some()
        || history.iter().any(|snapshot| {
            snapshot.binding().operation_id() != permit.operation_id()
                || snapshot.binding().inspect_review_sha256() != permit.inspect_review_sha256()
                || !is_lower_sha256(snapshot.event_sha256())
                || snapshot.recorded_at_unix_ms() == 0
                || snapshot.outcome_code().is_some()
                    != matches!(
                        snapshot.state(),
                        ApplyJournalState::NeedsRecovery | ApplyJournalState::Failed
                    )
                || !snapshot_evidence_matches_permit(snapshot, permit)
                || !snapshot_checkpoint_proof_is_valid(snapshot)
                || snapshot.native_authority_binding().is_some()
        })
        || history.windows(2).any(|pair| {
            pair[1].generation() != pair[0].generation().saturating_add(1)
                || pair[1].previous_event_sha256() != Some(pair[0].event_sha256())
                || pair[1].recorded_at_unix_ms() < pair[0].recorded_at_unix_ms()
        })
        || head.generation() != permit.generation()
        || head.event_sha256() != permit.event_sha256()
        || !matches!(
            head.state(),
            ApplyJournalState::Running | ApplyJournalState::Verifying
        )
        || head.checkpoint() < ApplyCheckpoint::InstallationIdentityReserved
        || head.installation_id() != Some(permit.installation_id())
        || head.backup_restore_proof_sha256() != Some(permit.backup_restore_proof_sha256())
        || head.backup_reference_sha256() != Some(permit.backup_reference_sha256())
        || head.final_recheck_report_sha256() != Some(permit.final_recheck_report_sha256())
        || head.source_fingerprint_sha256() != Some(permit.source_fingerprint_sha256())
        || head.checkpoint_proof_sha256() != permit.checkpoint_proof_sha256()
    {
        return Err(LifecycleLedgerError::InvalidJournalHistory);
    }
    Ok(())
}

fn snapshot_evidence_matches_permit(
    snapshot: &ApplyJournalSnapshot,
    permit: &DurableTargetMutationPermit,
) -> bool {
    let installation_matches =
        if snapshot.checkpoint() < ApplyCheckpoint::InstallationIdentityReserved {
            snapshot.installation_id().is_none()
        } else {
            snapshot.installation_id() == Some(permit.installation_id())
        };
    let backup_matches = if snapshot.checkpoint() < ApplyCheckpoint::BackupRestoreVerified {
        snapshot.backup_restore_proof_sha256().is_none()
            && snapshot.backup_reference_sha256().is_none()
    } else {
        snapshot.backup_restore_proof_sha256() == Some(permit.backup_restore_proof_sha256())
            && snapshot.backup_reference_sha256() == Some(permit.backup_reference_sha256())
    };
    let final_recheck_matches = if snapshot.checkpoint() < ApplyCheckpoint::FinalRecheckPassed {
        snapshot.final_recheck_report_sha256().is_none()
            && snapshot.source_fingerprint_sha256().is_none()
    } else {
        snapshot.final_recheck_report_sha256() == Some(permit.final_recheck_report_sha256())
            && snapshot.source_fingerprint_sha256() == Some(permit.source_fingerprint_sha256())
    };
    installation_matches && backup_matches && final_recheck_matches
}

fn snapshot_checkpoint_proof_is_valid(snapshot: &ApplyJournalSnapshot) -> bool {
    let proof = snapshot.checkpoint_proof_sha256();
    let shape_valid = if matches!(
        snapshot.checkpoint(),
        ApplyCheckpoint::PendingDurable | ApplyCheckpoint::InstallationIdentityReserved
    ) {
        proof.is_none()
    } else {
        proof.is_some_and(is_lower_sha256)
    };
    shape_valid
        && (snapshot.checkpoint() != ApplyCheckpoint::BackupRestoreVerified
            || proof == snapshot.backup_restore_proof_sha256())
        && (snapshot.checkpoint() != ApplyCheckpoint::FinalRecheckPassed
            || proof == snapshot.final_recheck_report_sha256())
}

fn verify_target_lineage_binding() -> Result<(), LifecycleLedgerError> {
    if target_postgres_lineage_sha256() != TARGET_POSTGRES_LINEAGE_SHA256 {
        return Err(LifecycleLedgerError::TargetLineageBindingMismatch);
    }
    Ok(())
}

/// Selects the converter policy already fixed by the typed manifest. Keeping
/// `PreserveAll` on schema v4 is compatibility-critical: its registry digest
/// is the original v1 digest, so existing durable ledger rows and checkpoints
/// retain their byte-for-byte identity. Schema v5 receives the distinct,
/// domain-separated discard-policy digest.
fn converter_registry_sha256_for_spec(
    spec: &ProvisionSpec,
) -> Result<String, LifecycleLedgerError> {
    if spec.legacy_apply_execution().is_none() {
        return Err(LifecycleLedgerError::BindingMismatch);
    }
    let strategy = LegacyConversionStrategy::for_schema_version(spec.schema_version)?;
    let registry_sha256 = registry_sha256_for_strategy(strategy)?;
    Ok(registry_sha256)
}

async fn require_public_schema(pool: &PgPool) -> Result<(), LifecycleLedgerError> {
    let schema = sqlx::query_scalar::<_, Option<String>>("SELECT current_schema()")
        .fetch_one(pool)
        .await?;
    if schema.as_deref() != Some("public") {
        return Err(LifecycleLedgerError::UnexpectedTargetSchema);
    }
    Ok(())
}

async fn verify_bootstrapped_schema(pool: &PgPool) -> Result<(), LifecycleLedgerError> {
    verify_target_lineage_binding()?;
    require_public_schema(pool).await?;
    if target_table_names(pool).await? != expected_target_tables()
        || !v2board_db::migrations_current(pool).await?
    {
        return Err(LifecycleLedgerError::UnexpectedTargetSchema);
    }
    Ok(())
}

async fn begin_durable_transaction(
    pool: &PgPool,
) -> Result<Transaction<'_, Postgres>, LifecycleLedgerError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL synchronous_commit = 'on'")
        .execute(&mut *tx)
        .await?;
    let synchronous_commit: String =
        sqlx::query_scalar("SELECT current_setting('synchronous_commit')")
            .fetch_one(&mut *tx)
            .await?;
    if synchronous_commit != "on" {
        return Err(LifecycleLedgerError::DurabilitySettingRejected);
    }
    Ok(tx)
}

async fn target_table_names(pool: &PgPool) -> Result<BTreeSet<String>, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect())
}

fn expected_target_tables() -> BTreeSet<String> {
    TABLE_MAPPINGS
        .iter()
        .map(|mapping| mapping.target.to_string())
        .chain(
            DERIVED_MAPPINGS
                .iter()
                .map(|mapping| mapping.target.to_string()),
        )
        .chain(TARGET_ONLY_TABLES.iter().map(|table| (*table).to_string()))
        .chain(["_sqlx_migrations".to_string()])
        .collect()
}

async fn installation_matches(
    tx: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
    created_at: i64,
    source_fingerprint: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*) = 1 FROM v2_system_installation
        WHERE singleton = 1 AND installation_id = $1 AND lineage = 'legacy_migrated'
          AND state = 'pending' AND created_at = $2 AND activated_at IS NULL
          AND source_fingerprint_sha256 = $3
        "#,
    )
    .bind(installation_id)
    .bind(created_at)
    .bind(source_fingerprint)
    .fetch_one(&mut **tx)
    .await
}

#[allow(clippy::too_many_arguments)]
async fn operation_head_matches(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
    installation_id: Uuid,
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
    binding: &LifecycleLedgerBinding,
    converter_registry_sha256: &str,
    head: &ApplyJournalSnapshot,
    created_at: i64,
    updated_at: i64,
) -> Result<bool, LifecycleLedgerError> {
    let generation = i64_generation(head.generation())?;
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*) = 1 FROM v2_lifecycle_operation
        WHERE operation_id = $1 AND installation_id = $2
          AND kind = 'legacy_reference_migration'
          AND manifest_binding_hmac_sha256 = $3 AND inspect_review_sha256 = $4
          AND authorized_snapshot_report_sha256 = $5
          AND authorized_snapshot_report_binding_hmac_sha256 = $6
          AND authorization_binding_hmac_sha256 = $7
          AND authorization_file_sha256 = $8
          AND source_fingerprint_sha256 = $9 AND converter_registry_sha256 = $10
          AND target_lineage_sha256 = $11 AND state = $12 AND checkpoint = $13
          AND journal_generation = $14 AND journal_event_sha256 = $15
          AND checkpoint_proof_sha256 IS NOT DISTINCT FROM $16
          AND backup_reference = $17
          AND backup_restore_proof_sha256 IS NOT DISTINCT FROM $18
          AND final_recheck_report_sha256 IS NOT DISTINCT FROM $19
          AND native_authority_nodes_generation IS NULL
          AND native_authority_nodes_event_sha256 IS NULL
          AND data_verification_report_sha256 IS NULL
          AND analytics_projection_report_sha256 IS NULL
          AND node_cutover_report_sha256 IS NULL
          AND source_retired IS NULL AND mysql_reachable IS NULL
          AND source_redis_reachable IS NULL
          AND source_access_permanently_disabled IS NULL
          AND legacy_runtime_compat IS NULL
          AND cold_archive_reference IS NULL AND cold_archive_sha256 IS NULL
          AND created_at = $20 AND updated_at = $21 AND completed_at IS NULL
        "#,
    )
    .bind(operation_id)
    .bind(installation_id)
    .bind(spec.manifest_binding_hmac_sha256())
    .bind(permit.inspect_review_sha256())
    .bind(
        &binding
            .authorization_audit
            .authorized_snapshot_report_sha256,
    )
    .bind(
        &binding
            .authorization_audit
            .authorized_snapshot_report_binding_hmac_sha256,
    )
    .bind(
        &binding
            .authorization_audit
            .authorization_binding_hmac_sha256,
    )
    .bind(&binding.authorization_audit.authorization_file_sha256)
    .bind(&binding.source_fingerprint_sha256)
    .bind(converter_registry_sha256)
    .bind(TARGET_POSTGRES_LINEAGE_SHA256)
    .bind(state_text(head.state()))
    .bind(checkpoint_code(head.checkpoint()))
    .bind(generation)
    .bind(head.event_sha256())
    .bind(head.checkpoint_proof_sha256())
    .bind(&binding.backup_reference)
    .bind(head.backup_restore_proof_sha256())
    .bind(head.final_recheck_report_sha256())
    .bind(created_at)
    .bind(updated_at)
    .fetch_one(&mut **tx)
    .await
    .map_err(LifecycleLedgerError::Database)
}

async fn mirror_event_insert(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
    snapshot: &ApplyJournalSnapshot,
) -> Result<(), LifecycleLedgerError> {
    let installation_id = snapshot.installation_id().map(parse_uuid).transpose()?;
    let inserted = sqlx::query(
        r#"
        INSERT INTO v2_lifecycle_event (
            operation_id, generation, state, checkpoint, outcome_code,
            previous_event_sha256, event_sha256, checkpoint_proof_sha256, installation_id,
            backup_restore_proof_sha256, backup_reference_sha256,
            final_recheck_report_sha256, source_fingerprint_sha256,
            native_authority_nodes_generation,
            native_authority_nodes_event_sha256,
            data_verification_report_sha256,
            analytics_projection_report_sha256, node_cutover_report_sha256,
            recorded_at_unix_ms
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)
        ON CONFLICT (operation_id, generation) DO NOTHING
        "#,
    )
    .bind(operation_id)
    .bind(i64_generation(snapshot.generation())?)
    .bind(state_text(snapshot.state()))
    .bind(checkpoint_code(snapshot.checkpoint()))
    .bind(snapshot.outcome_code().map(outcome_text))
    .bind(snapshot.previous_event_sha256())
    .bind(snapshot.event_sha256())
    .bind(snapshot.checkpoint_proof_sha256())
    .bind(installation_id)
    .bind(snapshot.backup_restore_proof_sha256())
    .bind(snapshot.backup_reference_sha256())
    .bind(snapshot.final_recheck_report_sha256())
    .bind(snapshot.source_fingerprint_sha256())
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| i64_generation(binding.nodes_verified_generation()))
            .transpose()?,
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.nodes_verified_event_sha256().to_string()),
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.data_verification_report_sha256().to_string()),
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.analytics_projection_report_sha256().to_string()),
    )
    .bind(
        snapshot
            .native_authority_binding()
            .map(|binding| binding.node_cutover_report_sha256().to_string()),
    )
    .bind(i64_generation(snapshot.recorded_at_unix_ms())?)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        let matches: bool = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) = 1 FROM v2_lifecycle_event
            WHERE operation_id=$1 AND generation=$2 AND state=$3 AND checkpoint=$4
              AND outcome_code IS NOT DISTINCT FROM $5
              AND previous_event_sha256 IS NOT DISTINCT FROM $6 AND event_sha256=$7
              AND checkpoint_proof_sha256 IS NOT DISTINCT FROM $8
              AND installation_id IS NOT DISTINCT FROM $9
              AND backup_restore_proof_sha256 IS NOT DISTINCT FROM $10
              AND backup_reference_sha256 IS NOT DISTINCT FROM $11
              AND final_recheck_report_sha256 IS NOT DISTINCT FROM $12
              AND source_fingerprint_sha256 IS NOT DISTINCT FROM $13
              AND native_authority_nodes_generation IS NOT DISTINCT FROM $14
              AND native_authority_nodes_event_sha256 IS NOT DISTINCT FROM $15
              AND data_verification_report_sha256 IS NOT DISTINCT FROM $16
              AND analytics_projection_report_sha256 IS NOT DISTINCT FROM $17
              AND node_cutover_report_sha256 IS NOT DISTINCT FROM $18
              AND recorded_at_unix_ms=$19
            "#,
        )
        .bind(operation_id)
        .bind(i64_generation(snapshot.generation())?)
        .bind(state_text(snapshot.state()))
        .bind(checkpoint_code(snapshot.checkpoint()))
        .bind(snapshot.outcome_code().map(outcome_text))
        .bind(snapshot.previous_event_sha256())
        .bind(snapshot.event_sha256())
        .bind(snapshot.checkpoint_proof_sha256())
        .bind(installation_id)
        .bind(snapshot.backup_restore_proof_sha256())
        .bind(snapshot.backup_reference_sha256())
        .bind(snapshot.final_recheck_report_sha256())
        .bind(snapshot.source_fingerprint_sha256())
        .bind(
            snapshot
                .native_authority_binding()
                .map(|binding| i64_generation(binding.nodes_verified_generation()))
                .transpose()?,
        )
        .bind(
            snapshot
                .native_authority_binding()
                .map(|binding| binding.nodes_verified_event_sha256().to_string()),
        )
        .bind(
            snapshot
                .native_authority_binding()
                .map(|binding| binding.data_verification_report_sha256().to_string()),
        )
        .bind(
            snapshot
                .native_authority_binding()
                .map(|binding| binding.analytics_projection_report_sha256().to_string()),
        )
        .bind(
            snapshot
                .native_authority_binding()
                .map(|binding| binding.node_cutover_report_sha256().to_string()),
        )
        .bind(i64_generation(snapshot.recorded_at_unix_ms())?)
        .fetch_one(&mut **tx)
        .await?;
        if !matches {
            return Err(LifecycleLedgerError::ConflictingTargetState);
        }
    }
    Ok(())
}

#[derive(FromRow)]
struct OperationHeadRow {
    installation_id: Uuid,
    kind: String,
    manifest_binding_hmac_sha256: String,
    inspect_review_sha256: String,
    authorized_snapshot_report_sha256: String,
    authorized_snapshot_report_binding_hmac_sha256: String,
    authorization_binding_hmac_sha256: String,
    authorization_file_sha256: String,
    source_fingerprint_sha256: String,
    converter_registry_sha256: String,
    target_lineage_sha256: String,
    state: String,
    checkpoint: i16,
    journal_generation: i64,
    journal_event_sha256: String,
    checkpoint_proof_sha256: Option<String>,
    backup_reference: Option<String>,
    backup_restore_proof_sha256: Option<String>,
    final_recheck_report_sha256: Option<String>,
    native_authority_nodes_generation: Option<i64>,
    native_authority_nodes_event_sha256: Option<String>,
    data_verification_report_sha256: Option<String>,
    analytics_projection_report_sha256: Option<String>,
    node_cutover_report_sha256: Option<String>,
    source_retired: Option<bool>,
    mysql_reachable: Option<bool>,
    source_redis_reachable: Option<bool>,
    source_access_permanently_disabled: Option<bool>,
    legacy_runtime_compat: Option<bool>,
    cold_archive_reference: Option<String>,
    cold_archive_sha256: Option<String>,
    created_at: i64,
    updated_at: i64,
    completed_at: Option<i64>,
}

#[derive(FromRow)]
struct InstallationRow {
    installation_id: Uuid,
    lineage: String,
    state: String,
    created_at: i64,
    activated_at: Option<i64>,
    source_fingerprint_sha256: Option<String>,
}

#[derive(FromRow)]
struct ActivationCommitRow {
    installation_id: Uuid,
    journal_generation: i64,
    journal_event_sha256: String,
    data_verification_report_sha256: String,
    analytics_projection_report_sha256: String,
    node_cutover_report_sha256: String,
    committed_at: i64,
}

async fn lock_operation_head(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
) -> Result<OperationHeadRow, LifecycleLedgerError> {
    sqlx::query_as(
        r#"
        SELECT installation_id, kind, manifest_binding_hmac_sha256,
               inspect_review_sha256, authorized_snapshot_report_sha256,
               authorized_snapshot_report_binding_hmac_sha256,
               authorization_binding_hmac_sha256, authorization_file_sha256,
               source_fingerprint_sha256,
               converter_registry_sha256, target_lineage_sha256, state,
               checkpoint, journal_generation, journal_event_sha256,
               checkpoint_proof_sha256, backup_reference,
               backup_restore_proof_sha256, final_recheck_report_sha256,
               native_authority_nodes_generation,
               native_authority_nodes_event_sha256, data_verification_report_sha256,
               analytics_projection_report_sha256, node_cutover_report_sha256,
               source_retired, mysql_reachable, source_redis_reachable,
               source_access_permanently_disabled, legacy_runtime_compat,
               cold_archive_reference,
               cold_archive_sha256, created_at, updated_at, completed_at
        FROM v2_lifecycle_operation
        WHERE operation_id = $1
        FOR UPDATE
        "#,
    )
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(LifecycleLedgerError::ConflictingTargetState)
}

async fn load_operation_head_readonly(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
) -> Result<OperationHeadRow, LifecycleLedgerError> {
    sqlx::query_as(
        r#"
        SELECT installation_id, kind, manifest_binding_hmac_sha256,
               inspect_review_sha256, authorized_snapshot_report_sha256,
               authorized_snapshot_report_binding_hmac_sha256,
               authorization_binding_hmac_sha256, authorization_file_sha256,
               source_fingerprint_sha256,
               converter_registry_sha256, target_lineage_sha256, state,
               checkpoint, journal_generation, journal_event_sha256,
               checkpoint_proof_sha256, backup_reference,
               backup_restore_proof_sha256, final_recheck_report_sha256,
               native_authority_nodes_generation,
               native_authority_nodes_event_sha256, data_verification_report_sha256,
               analytics_projection_report_sha256, node_cutover_report_sha256,
               source_retired, mysql_reachable, source_redis_reachable,
               source_access_permanently_disabled, legacy_runtime_compat,
               cold_archive_reference,
               cold_archive_sha256, created_at, updated_at, completed_at
        FROM v2_lifecycle_operation
        WHERE operation_id = $1
        "#,
    )
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(LifecycleLedgerError::ConflictingTargetState)
}

async fn lock_installation(
    tx: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
) -> Result<InstallationRow, LifecycleLedgerError> {
    sqlx::query_as(
        r#"
        SELECT installation_id, lineage, state, created_at, activated_at,
               source_fingerprint_sha256
        FROM v2_system_installation
        WHERE singleton = 1 AND installation_id = $1
        FOR UPDATE
        "#,
    )
    .bind(installation_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(LifecycleLedgerError::ConflictingTargetState)
}

async fn load_installation_readonly(
    tx: &mut Transaction<'_, Postgres>,
    installation_id: Uuid,
) -> Result<InstallationRow, LifecycleLedgerError> {
    sqlx::query_as(
        r#"
        SELECT installation_id, lineage, state, created_at, activated_at,
               source_fingerprint_sha256
        FROM v2_system_installation
        WHERE singleton = 1 AND installation_id = $1
        "#,
    )
    .bind(installation_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(LifecycleLedgerError::ConflictingTargetState)
}

async fn load_activation_commit(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
) -> Result<Option<ActivationCommitRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT installation_id, journal_generation, journal_event_sha256,
               data_verification_report_sha256,
               analytics_projection_report_sha256,
               node_cutover_report_sha256, committed_at
        FROM v2_lifecycle_activation_commit
        WHERE operation_id = $1
        "#,
    )
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await
}

async fn insert_activation_commit(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
    installation_id: Uuid,
    snapshot: &ApplyJournalSnapshot,
    proof: &NativeActivationProofBinding,
    committed_at: i64,
) -> Result<(), LifecycleLedgerError> {
    let inserted = sqlx::query(
        r#"
        INSERT INTO v2_lifecycle_activation_commit (
            operation_id, installation_id, journal_generation, journal_state,
            journal_checkpoint, journal_event_sha256,
            data_verification_report_sha256,
            analytics_projection_report_sha256, node_cutover_report_sha256,
            committed_at
        ) VALUES ($1,$2,$3,'verifying',11,$4,$5,$6,$7,$8)
        "#,
    )
    .bind(operation_id)
    .bind(installation_id)
    .bind(i64_generation(snapshot.generation())?)
    .bind(snapshot.event_sha256())
    .bind(&proof.data_verification_report_sha256)
    .bind(&proof.analytics_projection_report_sha256)
    .bind(&proof.node_cutover_report_sha256)
    .bind(committed_at)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if inserted != 1 {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    Ok(())
}

async fn validate_snapshot_native_authority(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
    snapshot: &ApplyJournalSnapshot,
) -> Result<(), LifecycleLedgerError> {
    if snapshot.checkpoint() < ApplyCheckpoint::NativeAuthorityCommitted {
        return if snapshot.native_authority_binding().is_none() {
            Ok(())
        } else {
            Err(LifecycleLedgerError::ConflictingTargetState)
        };
    }
    let authority = snapshot
        .native_authority_binding()
        .ok_or(LifecycleLedgerError::ConflictingTargetState)?;
    let commit = load_activation_commit(tx, operation_id)
        .await?
        .ok_or(LifecycleLedgerError::ConflictingTargetState)?;
    if !snapshot_authority_matches_commit(snapshot, &commit) {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    let proof = NativeActivationProofBinding::new(
        authority.data_verification_report_sha256(),
        authority.analytics_projection_report_sha256(),
        authority.node_cutover_report_sha256(),
    )?;
    if !activation_stage_proofs_match(tx, operation_id, snapshot, &proof).await? {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    let installation_active: bool = sqlx::query_scalar(
        "SELECT COUNT(*) = 1 FROM v2_system_installation \
         WHERE singleton = 1 AND installation_id = $1 AND lineage = 'native' \
           AND state = 'active' AND activated_at = $2",
    )
    .bind(commit.installation_id)
    .bind(commit.committed_at)
    .fetch_one(&mut **tx)
    .await?;
    let anchor_matches: bool = sqlx::query_scalar(
        "SELECT COUNT(*) = 1 FROM v2_lifecycle_event \
         WHERE operation_id = $1 AND generation = $2 AND event_sha256 = $3 \
           AND state = 'verifying' AND checkpoint = 11 AND outcome_code IS NULL \
           AND installation_id = $4 AND checkpoint_proof_sha256 = $5 \
           AND native_authority_nodes_generation IS NULL \
           AND native_authority_nodes_event_sha256 IS NULL \
           AND data_verification_report_sha256 IS NULL \
           AND analytics_projection_report_sha256 IS NULL \
           AND node_cutover_report_sha256 IS NULL",
    )
    .bind(operation_id)
    .bind(i64_generation(authority.nodes_verified_generation())?)
    .bind(authority.nodes_verified_event_sha256())
    .bind(commit.installation_id)
    .bind(authority.node_cutover_report_sha256())
    .fetch_one(&mut **tx)
    .await?;
    if !installation_active || !anchor_matches {
        return Err(LifecycleLedgerError::ConflictingTargetState);
    }
    Ok(())
}

fn installation_binding_matches(installation: &InstallationRow, head: &OperationHeadRow) -> bool {
    installation.installation_id == head.installation_id
        && installation.created_at == head.created_at
        && installation.source_fingerprint_sha256.as_deref()
            == Some(head.source_fingerprint_sha256.as_str())
}

fn verification_fields_are_set(head: &OperationHeadRow) -> bool {
    head.data_verification_report_sha256.is_some()
        || head.analytics_projection_report_sha256.is_some()
        || head.node_cutover_report_sha256.is_some()
}

fn activation_commit_matches(
    commit: &ActivationCommitRow,
    installation_id: Uuid,
    snapshot: &ApplyJournalSnapshot,
    proof: &NativeActivationProofBinding,
) -> bool {
    commit.installation_id == installation_id
        && i64::try_from(snapshot.generation()).ok() == Some(commit.journal_generation)
        && commit.journal_event_sha256 == snapshot.event_sha256()
        && commit.data_verification_report_sha256 == proof.data_verification_report_sha256
        && commit.analytics_projection_report_sha256 == proof.analytics_projection_report_sha256
        && commit.node_cutover_report_sha256 == proof.node_cutover_report_sha256
        && commit.committed_at > 0
}

fn activation_commit_matches_completion(
    commit: &ActivationCommitRow,
    installation_id: Uuid,
    completed_generation: i64,
    proof: &CompletionProofBinding,
) -> bool {
    commit.installation_id == installation_id
        && commit.journal_generation < completed_generation
        && is_lower_sha256(&commit.journal_event_sha256)
        && commit.data_verification_report_sha256 == proof.data_verification_report_sha256
        && commit.analytics_projection_report_sha256 == proof.analytics_projection_report_sha256
        && commit.node_cutover_report_sha256 == proof.node_cutover_report_sha256
        && commit.committed_at > 0
}

fn snapshot_authority_matches_commit(
    snapshot: &ApplyJournalSnapshot,
    commit: &ActivationCommitRow,
) -> bool {
    snapshot.native_authority_binding().is_some_and(|binding| {
        i64::try_from(binding.nodes_verified_generation()).ok() == Some(commit.journal_generation)
            && binding.nodes_verified_event_sha256() == commit.journal_event_sha256
            && binding.data_verification_report_sha256() == commit.data_verification_report_sha256
            && binding.analytics_projection_report_sha256()
                == commit.analytics_projection_report_sha256
            && binding.node_cutover_report_sha256() == commit.node_cutover_report_sha256
    })
}

async fn activation_stage_proofs_match(
    tx: &mut Transaction<'_, Postgres>,
    operation_id: Uuid,
    snapshot: &ApplyJournalSnapshot,
    proof: &NativeActivationProofBinding,
) -> Result<bool, LifecycleLedgerError> {
    let data: Option<String> = sqlx::query_scalar(
        "SELECT checkpoint_proof_sha256 FROM v2_lifecycle_event \
         WHERE operation_id = $1 AND checkpoint = 8 AND state = 'verifying' \
           AND outcome_code IS NULL AND generation <= $2 \
         ORDER BY generation DESC LIMIT 1",
    )
    .bind(operation_id)
    .bind(i64_generation(snapshot.generation())?)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    let analytics: Option<String> = sqlx::query_scalar(
        "SELECT checkpoint_proof_sha256 FROM v2_lifecycle_event \
         WHERE operation_id = $1 AND checkpoint = 9 AND state = 'verifying' \
           AND outcome_code IS NULL AND generation <= $2 \
         ORDER BY generation DESC LIMIT 1",
    )
    .bind(operation_id)
    .bind(i64_generation(snapshot.generation())?)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    Ok(
        data.as_deref() == Some(proof.data_verification_report_sha256.as_str())
            && analytics.as_deref() == Some(proof.analytics_projection_report_sha256.as_str()),
    )
}

fn validate_snapshot_binding(
    spec: &ProvisionSpec,
    snapshot: &ApplyJournalSnapshot,
) -> Result<Uuid, LifecycleLedgerError> {
    if spec.kind != ProvisionKind::LegacyReferenceMigration
        || snapshot.binding().operation_id() != spec.operation_id
        || !is_lower_sha256(snapshot.binding().inspect_review_sha256())
        || !is_lower_sha256(snapshot.event_sha256())
    {
        return Err(LifecycleLedgerError::BindingMismatch);
    }
    parse_uuid(&spec.operation_id)
}

fn operation_binding_matches(
    head: &OperationHeadRow,
    spec: &ProvisionSpec,
    snapshot: &ApplyJournalSnapshot,
    converter_registry_sha256: &str,
    authorization_audit: &AuthorizationAuditBinding,
) -> bool {
    let snapshot_checkpoint = checkpoint_code(snapshot.checkpoint());
    let proof_matches = head.checkpoint != snapshot_checkpoint
        || head.checkpoint_proof_sha256.as_deref() == snapshot.checkpoint_proof_sha256();
    let authority_matches = match (
        head_native_authority_binding(head),
        snapshot.native_authority_binding(),
    ) {
        (Ok(None), None) => true,
        (Ok(Some(head)), Some(snapshot)) => head == snapshot,
        (Ok(None), Some(_)) => {
            head.checkpoint == checkpoint_code(ApplyCheckpoint::NodesVerified)
                && snapshot.checkpoint() == ApplyCheckpoint::NativeAuthorityCommitted
        }
        _ => false,
    };
    snapshot
        .installation_id()
        .and_then(|value| Uuid::parse_str(value).ok())
        == Some(head.installation_id)
        && head.kind == "legacy_reference_migration"
        && head.manifest_binding_hmac_sha256 == spec.manifest_binding_hmac_sha256()
        && head.inspect_review_sha256 == snapshot.binding().inspect_review_sha256()
        && authorization_audit_fields_are_well_formed(head)
        && authorization_audit_matches(head, authorization_audit)
        && is_lower_sha256(&head.source_fingerprint_sha256)
        && head.converter_registry_sha256 == converter_registry_sha256
        && head.target_lineage_sha256 == TARGET_POSTGRES_LINEAGE_SHA256
        && proof_matches
        && authority_matches
        && head
            .backup_reference
            .as_deref()
            .is_some_and(valid_reference)
        && head
            .backup_reference
            .as_deref()
            .and_then(|reference| backup_reference_sha256(reference).ok())
            .as_deref()
            == snapshot.backup_reference_sha256()
        && snapshot.backup_restore_proof_sha256() == head.backup_restore_proof_sha256.as_deref()
        && snapshot.final_recheck_report_sha256() == head.final_recheck_report_sha256.as_deref()
        && snapshot.source_fingerprint_sha256() == Some(head.source_fingerprint_sha256.as_str())
        && verification_fields_are_well_formed(head)
        && head.created_at > 0
        && head.updated_at >= head.created_at
}

fn authorization_audit_fields_are_well_formed(head: &OperationHeadRow) -> bool {
    [
        &head.authorized_snapshot_report_sha256,
        &head.authorized_snapshot_report_binding_hmac_sha256,
        &head.authorization_binding_hmac_sha256,
        &head.authorization_file_sha256,
    ]
    .into_iter()
    .all(|value| is_lower_sha256(value))
}

fn authorization_audit_matches(
    head: &OperationHeadRow,
    binding: &AuthorizationAuditBinding,
) -> bool {
    head.authorized_snapshot_report_sha256 == binding.authorized_snapshot_report_sha256
        && head.authorized_snapshot_report_binding_hmac_sha256
            == binding.authorized_snapshot_report_binding_hmac_sha256
        && head.authorization_binding_hmac_sha256 == binding.authorization_binding_hmac_sha256
        && head.authorization_file_sha256 == binding.authorization_file_sha256
}

fn verification_fields_are_well_formed(head: &OperationHeadRow) -> bool {
    head_native_authority_binding(head).is_ok()
}

fn head_native_authority_binding(
    head: &OperationHeadRow,
) -> Result<Option<NativeAuthorityBinding>, LifecycleLedgerError> {
    match (
        head.native_authority_nodes_generation,
        head.native_authority_nodes_event_sha256.as_deref(),
        head.data_verification_report_sha256.as_deref(),
        head.analytics_projection_report_sha256.as_deref(),
        head.node_cutover_report_sha256.as_deref(),
    ) {
        (None, None, None, None, None) => Ok(None),
        (Some(generation), Some(event), Some(data), Some(analytics), Some(nodes)) => {
            let generation = u64::try_from(generation)
                .map_err(|_| LifecycleLedgerError::ConflictingTargetState)?;
            NativeAuthorityBinding::new(generation, event, data, analytics, nodes)
                .map(Some)
                .map_err(|_| LifecycleLedgerError::ConflictingTargetState)
        }
        _ => Err(LifecycleLedgerError::ConflictingTargetState),
    }
}

fn snapshot_matches_operation_head(
    head: &OperationHeadRow,
    snapshot: &ApplyJournalSnapshot,
) -> Result<bool, LifecycleLedgerError> {
    Ok(head.state == state_text(snapshot.state())
        && head.checkpoint == checkpoint_code(snapshot.checkpoint())
        && head.journal_generation == i64_generation(snapshot.generation())?
        && head.journal_event_sha256 == snapshot.event_sha256()
        && head.checkpoint_proof_sha256.as_deref() == snapshot.checkpoint_proof_sha256()
        && head_native_authority_binding(head)? == snapshot.native_authority_binding()
        && head.updated_at == unix_seconds(snapshot.recorded_at_unix_ms())?)
}

fn parse_uuid(value: &str) -> Result<Uuid, LifecycleLedgerError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or(LifecycleLedgerError::BindingMismatch)
}

fn state_text(state: ApplyJournalState) -> &'static str {
    match state {
        ApplyJournalState::Pending => "pending",
        ApplyJournalState::Running => "running",
        ApplyJournalState::Verifying => "verifying",
        ApplyJournalState::NeedsRecovery => "needs_recovery",
        ApplyJournalState::Failed => "failed",
        ApplyJournalState::Completed => "completed",
    }
}

fn checkpoint_code(checkpoint: ApplyCheckpoint) -> i16 {
    match checkpoint {
        ApplyCheckpoint::PendingDurable => 0,
        ApplyCheckpoint::MaintenanceFenced => 1,
        ApplyCheckpoint::SourceDrained => 2,
        ApplyCheckpoint::BackupRestoreVerified => 3,
        ApplyCheckpoint::FinalRecheckPassed => 4,
        ApplyCheckpoint::InstallationIdentityReserved => 5,
        ApplyCheckpoint::TargetsBootstrapped => 6,
        ApplyCheckpoint::PostgresBulkCopied => 7,
        ApplyCheckpoint::PostgresValueVerified => 8,
        ApplyCheckpoint::ClickhouseProjected => 9,
        ApplyCheckpoint::RuntimeMaterialized => 10,
        ApplyCheckpoint::NodesVerified => 11,
        ApplyCheckpoint::NativeAuthorityCommitted => 12,
        ApplyCheckpoint::CutoverCommitted => 13,
        ApplyCheckpoint::SourceRetired => 14,
        ApplyCheckpoint::CompletionVerified => 15,
    }
}

fn outcome_text(outcome: ApplyOutcomeCode) -> &'static str {
    match outcome {
        ApplyOutcomeCode::ProcessInterrupted => "process_interrupted",
        ApplyOutcomeCode::IoFailure => "io_failure",
        ApplyOutcomeCode::SourceDrift => "source_drift",
        ApplyOutcomeCode::TargetDrift => "target_drift",
        ApplyOutcomeCode::FenceUncertain => "fence_uncertain",
        ApplyOutcomeCode::DrainIncomplete => "drain_incomplete",
        ApplyOutcomeCode::BackupInvalid => "backup_invalid",
        ApplyOutcomeCode::ConversionFailed => "conversion_failed",
        ApplyOutcomeCode::VerificationMismatch => "verification_mismatch",
        ApplyOutcomeCode::ActivationFailed => "activation_failed",
        ApplyOutcomeCode::RetirementFailed => "retirement_failed",
        ApplyOutcomeCode::OperatorAbort => "operator_abort",
    }
}

fn i64_generation(value: u64) -> Result<i64, LifecycleLedgerError> {
    i64::try_from(value).map_err(|_| LifecycleLedgerError::InvalidJournalHistory)
}

fn unix_seconds(milliseconds: u64) -> Result<i64, LifecycleLedgerError> {
    i64_generation(milliseconds / 1_000)
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.contains('@')
        && !value.contains('?')
        && !value.contains('#')
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use sqlx::postgres::PgPoolOptions;

    use crate::{
        apply_journal::{ApplyJournal, ApplyJournalBinding},
        legacy_converter::{
            ConversionCheckpoint, ConversionPhase, ConversionRunBinding,
            LEGACY_SEMANTIC_SCHEMA_SHA256, LegacyConversionStrategy, registry_sha256,
            registry_sha256_for_strategy,
        },
        legacy_copy::{DurableCopyCheckpointSink, PostgresDurableCopyCheckpointSink},
        manifest::{ProvisionFlow, tests::legacy_spec_for_orchestration},
    };

    use super::*;

    const OPERATION_ID: &str = "40aa4a80-eb4b-4b25-9c3b-e17ed047873d";
    const INSTALLATION_ID: &str = "e0bb60eb-bb45-4393-8a04-18a3aa510497";
    const INSPECT_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const BACKUP_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const BACKUP_REFERENCE: &str = "backup-1042";
    const RECHECK_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SOURCE_FINGERPRINT: &str =
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const DATA_VERIFICATION_HASH: &str =
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const ANALYTICS_PROJECTION_HASH: &str =
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const NODE_CUTOVER_HASH: &str =
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    const RUNTIME_MATERIALIZED_HASH: &str =
        "8888888888888888888888888888888888888888888888888888888888888888";
    const COLD_ARCHIVE_HASH: &str =
        "1111111111111111111111111111111111111111111111111111111111111111";
    const AUTHORIZED_SNAPSHOT_HASH: &str =
        "2222222222222222222222222222222222222222222222222222222222222222";
    const AUTHORIZED_SNAPSHOT_HMAC: &str =
        "3333333333333333333333333333333333333333333333333333333333333333";
    const AUTHORIZATION_HMAC: &str =
        "4444444444444444444444444444444444444444444444444444444444444444";
    const AUTHORIZATION_FILE_HASH: &str =
        "5555555555555555555555555555555555555555555555555555555555555555";
    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "v2board-lifecycle-ledger-test-{}-{}",
                std::process::id(),
                TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            )))
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn authorization_audit_binding() -> AuthorizationAuditBinding {
        AuthorizationAuditBinding::new(
            AUTHORIZED_SNAPSHOT_HASH,
            AUTHORIZED_SNAPSHOT_HMAC,
            AUTHORIZATION_HMAC,
            AUTHORIZATION_FILE_HASH,
        )
        .expect("authorization audit binding")
    }

    fn durable_target_history() -> (
        TestRoot,
        ApplyJournal,
        DurableTargetMutationPermit,
        Vec<ApplyJournalSnapshot>,
    ) {
        let root = TestRoot::new();
        let binding = ApplyJournalBinding::new(OPERATION_ID, INSPECT_HASH).expect("binding");
        let (journal, pending) =
            ApplyJournal::create_pending(root.path(), binding).expect("pending journal");
        let mut current = journal.begin(&pending).expect("begin");
        for checkpoint in [
            ApplyCheckpoint::MaintenanceFenced,
            ApplyCheckpoint::SourceDrained,
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, INSPECT_HASH)
                .expect("advance source checkpoint");
        }
        let reference_hash = backup_reference_sha256(BACKUP_REFERENCE).expect("reference hash");
        current = journal
            .record_backup_restore_verified(&current, BACKUP_HASH, &reference_hash)
            .expect("backup proof");
        current = journal
            .record_final_recheck_passed(&current, RECHECK_HASH, SOURCE_FINGERPRINT)
            .expect("final recheck");
        current = journal
            .reserve_installation_identity(&current, INSTALLATION_ID)
            .expect("installation identity");
        let permit = journal
            .target_mutation_permit(&current)
            .expect("target permit");
        let history = journal.verified_history().expect("verified history");
        (root, journal, permit, history)
    }

    #[test]
    fn checkpoint_codes_match_the_frozen_journal_order() {
        assert_eq!(checkpoint_code(ApplyCheckpoint::PendingDurable), 0);
        assert_eq!(
            checkpoint_code(ApplyCheckpoint::InstallationIdentityReserved),
            5
        );
        assert_eq!(
            checkpoint_code(ApplyCheckpoint::NativeAuthorityCommitted),
            12
        );
        assert_eq!(checkpoint_code(ApplyCheckpoint::CompletionVerified), 15);
    }

    #[test]
    fn proof_references_are_bounded_and_cannot_embed_credentials() {
        assert!(valid_reference("backup:legacy/2026-07-12.snapshot"));
        assert!(!valid_reference("s3://user:password@example/bucket"));
        assert!(!valid_reference("backup?token=secret"));
        let binding = LifecycleLedgerBinding::new(
            "a".repeat(64),
            "backup-1042",
            authorization_audit_binding(),
        )
        .expect("valid canonical binding");
        assert_eq!(binding.backup_reference(), "backup-1042");
        assert_eq!(binding.source_fingerprint_sha256(), "a".repeat(64));
        assert!(
            LifecycleLedgerBinding::new(
                "a".repeat(64),
                " backup-1042",
                authorization_audit_binding(),
            )
            .is_err()
        );
    }

    #[test]
    fn authorization_audit_requires_four_canonical_hashes() {
        let binding = authorization_audit_binding();
        assert_eq!(
            binding.authorized_snapshot_report_sha256(),
            AUTHORIZED_SNAPSHOT_HASH
        );
        assert_eq!(
            binding.authorized_snapshot_report_binding_hmac_sha256(),
            AUTHORIZED_SNAPSHOT_HMAC
        );
        assert_eq!(
            binding.authorization_binding_hmac_sha256(),
            AUTHORIZATION_HMAC
        );
        assert_eq!(binding.authorization_file_sha256(), AUTHORIZATION_FILE_HASH);
        assert!(matches!(
            AuthorizationAuditBinding::new(
                "A".repeat(64),
                AUTHORIZED_SNAPSHOT_HMAC,
                AUTHORIZATION_HMAC,
                AUTHORIZATION_FILE_HASH,
            ),
            Err(LifecycleLedgerError::InvalidAuthorizationAuditBinding)
        ));
    }

    #[test]
    fn embedded_lineage_matches_the_frozen_converter_binding() {
        verify_target_lineage_binding().expect("lineage digest");
        assert_eq!(expected_target_tables().len(), 50);
    }

    #[test]
    fn lifecycle_registry_keeps_v4_identity_and_separates_v5_policy() {
        let mut spec = legacy_spec_for_orchestration();
        assert_eq!(spec.schema_version, 4);
        assert_eq!(
            converter_registry_sha256_for_spec(&spec).expect("schema-v4 registry"),
            registry_sha256().expect("frozen schema-v4 registry")
        );

        spec.schema_version = 5;
        assert_eq!(
            converter_registry_sha256_for_spec(&spec).expect("schema-v5 registry"),
            registry_sha256_for_strategy(
                LegacyConversionStrategy::DiscardNodesTrafficDetailsAndOperationalLogs,
            )
            .expect("discard-policy registry")
        );

        spec.schema_version = 3;
        assert!(matches!(
            converter_registry_sha256_for_spec(&spec),
            Err(LifecycleLedgerError::BindingMismatch)
        ));
    }

    #[test]
    fn target_history_requires_every_bound_hash_chain_event() {
        let (_root, _journal, permit, history) = durable_target_history();
        validate_history(&permit, &history).expect("valid target history");

        let mut missing_event = history.clone();
        missing_event.remove(2);
        assert!(matches!(
            validate_history(&permit, &missing_event),
            Err(LifecycleLedgerError::InvalidJournalHistory)
        ));
    }

    #[test]
    fn ledger_binding_must_match_the_durable_source_and_backup_reference() {
        let (_root, _journal, permit, _history) = durable_target_history();
        let exact = LifecycleLedgerBinding::new(
            SOURCE_FINGERPRINT,
            BACKUP_REFERENCE,
            authorization_audit_binding(),
        )
        .expect("exact binding");
        validate_ledger_binding(&permit, &exact).expect("permit-bound ledger values");

        let wrong_source = LifecycleLedgerBinding::new(
            "d".repeat(64),
            BACKUP_REFERENCE,
            authorization_audit_binding(),
        )
        .expect("alternate source");
        assert!(matches!(
            validate_ledger_binding(&permit, &wrong_source),
            Err(LifecycleLedgerError::BindingMismatch)
        ));
        let wrong_backup = LifecycleLedgerBinding::new(
            SOURCE_FINGERPRINT,
            "backup-1043",
            authorization_audit_binding(),
        )
        .expect("alternate backup");
        assert!(matches!(
            validate_ledger_binding(&permit, &wrong_backup),
            Err(LifecycleLedgerError::BindingMismatch)
        ));
    }

    #[test]
    fn completion_proof_is_canonical_and_fails_closed() {
        let proof = CompletionProofBinding::new(
            "c".repeat(64),
            "d".repeat(64),
            "e".repeat(64),
            "a".repeat(64),
            true,
            "archive:legacy/final.snapshot",
            "f".repeat(64),
        )
        .expect("completion proof");
        assert_eq!(
            proof.cold_archive_reference,
            "archive:legacy/final.snapshot"
        );
        assert!(matches!(
            CompletionProofBinding::new(
                "c".repeat(64),
                "d".repeat(64),
                "e".repeat(64),
                "a".repeat(64),
                false,
                "archive:legacy/final.snapshot",
                "f".repeat(64),
            ),
            Err(LifecycleLedgerError::InvalidCompletionProof)
        ));
        assert!(matches!(
            CompletionProofBinding::new(
                "c".repeat(64),
                "d".repeat(64),
                "e".repeat(64),
                "a".repeat(64),
                true,
                "archive?credential=secret",
                "f".repeat(64),
            ),
            Err(LifecycleLedgerError::InvalidCompletionProof)
        ));
    }

    #[tokio::test]
    #[ignore = "requires an empty PostgreSQL 18 database supplied by the Docker integration gate"]
    async fn postgres_lifecycle_ledger_is_atomic_idempotent_and_fail_closed() {
        let database_url = std::env::var("RUST_INTEGRATION_LIFECYCLE_DATABASE_URL")
            .expect("RUST_INTEGRATION_LIFECYCLE_DATABASE_URL must name an empty test database");
        let api_database_url = std::env::var("RUST_INTEGRATION_LIFECYCLE_API_DATABASE_URL")
            .expect("RUST_INTEGRATION_LIFECYCLE_API_DATABASE_URL must name the API role");
        let worker_database_url = std::env::var("RUST_INTEGRATION_LIFECYCLE_WORKER_DATABASE_URL")
            .expect("RUST_INTEGRATION_LIFECYCLE_WORKER_DATABASE_URL must name the worker role");
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await
            .expect("connect PostgreSQL lifecycle fixture");
        let server_version: String = sqlx::query_scalar("SELECT version()")
            .fetch_one(&pool)
            .await
            .expect("PostgreSQL version");
        assert!(
            server_version.starts_with("PostgreSQL 18."),
            "integration fixture must run PostgreSQL 18, got {server_version}"
        );
        let initial_tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'",
        )
        .fetch_one(&pool)
        .await
        .expect("empty target table count");
        assert_eq!(initial_tables, 0, "lifecycle target must start empty");

        let mut spec = legacy_spec_for_orchestration();
        let ProvisionFlow::LegacyReferenceMigration { target, .. } = &mut spec.flow else {
            panic!("shared lifecycle fixture must be the legacy flow");
        };
        target.postgres.migration_database_url = database_url.clone();
        target.postgres.api_database_url = api_database_url.clone();
        target.postgres.worker_database_url = worker_database_url.clone();
        assert_eq!(spec.operation_id, OPERATION_ID);
        let (root, journal, initial_permit, initial_history) = durable_target_history();
        let ledger_binding = LifecycleLedgerBinding::new(
            SOURCE_FINGERPRINT,
            BACKUP_REFERENCE,
            authorization_audit_binding(),
        )
        .expect("ledger binding");

        // A crash after the transactional migration lineage, and another after
        // ledger bootstrap, are both recovered by byte-for-byte idempotent retries.
        bootstrap_postgres_schema(&pool, &spec, &initial_permit)
            .await
            .expect("bootstrap empty PostgreSQL migration lineage");
        bootstrap_postgres_schema(&pool, &spec, &initial_permit)
            .await
            .expect("retry migration lineage after lost acknowledgement");
        let api_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&api_database_url)
            .await
            .expect("connect isolated API runtime role");
        let worker_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&worker_database_url)
            .await
            .expect("connect isolated worker runtime role");
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_user")
            .fetch_one(&api_pool)
            .await
            .expect("API role can read an allowed business table");
        assert!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_lifecycle_operation")
                .fetch_one(&api_pool)
                .await
                .is_err(),
            "API role must not read the protected lifecycle ledger"
        );
        assert!(
            sqlx::query("CREATE TABLE v2_runtime_acl_escape (id BIGINT)")
                .execute(&worker_pool)
                .await
                .is_err(),
            "worker role must not create schema objects"
        );
        bootstrap_lifecycle_ledger(
            &pool,
            &spec,
            &initial_permit,
            &initial_history,
            &ledger_binding,
        )
        .await
        .expect("bootstrap pending lifecycle ledger");
        bootstrap_lifecycle_ledger(
            &pool,
            &spec,
            &initial_permit,
            &initial_history,
            &ledger_binding,
        )
        .await
        .expect("retry pending ledger after lost acknowledgement");
        let tampered_authorization_audit = AuthorizationAuditBinding::new(
            "6".repeat(64),
            AUTHORIZED_SNAPSHOT_HMAC,
            AUTHORIZATION_HMAC,
            AUTHORIZATION_FILE_HASH,
        )
        .expect("well-formed but mismatched authorization audit binding");
        let tampered_ledger_binding = LifecycleLedgerBinding::new(
            SOURCE_FINGERPRINT,
            BACKUP_REFERENCE,
            tampered_authorization_audit.clone(),
        )
        .expect("well-formed alternate ledger binding");
        assert!(matches!(
            bootstrap_lifecycle_ledger(
                &pool,
                &spec,
                &initial_permit,
                &initial_history,
                &tampered_ledger_binding,
            )
            .await,
            Err(LifecycleLedgerError::ConflictingTargetState)
        ));
        assert!(matches!(
            mirror_lifecycle_snapshot(
                &pool,
                &spec,
                initial_history.last().expect("initial journal head"),
                &tampered_authorization_audit,
            )
            .await,
            Err(LifecycleLedgerError::ConflictingTargetState)
        ));

        let pending_installation: (String, String, Option<i64>) = sqlx::query_as(
            "SELECT lineage, state, activated_at FROM v2_system_installation WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("pending installation");
        assert_eq!(
            pending_installation,
            ("legacy_migrated".to_string(), "pending".to_string(), None)
        );

        drop(journal);
        let journal_binding =
            ApplyJournalBinding::new(OPERATION_ID, INSPECT_HASH).expect("journal binding");
        let (journal, mut current) = ApplyJournal::open(root.path(), journal_binding)
            .expect("reopen fsync journal after simulated process crash");
        assert_eq!(
            &current,
            initial_history.last().expect("initial journal head")
        );

        current = journal
            .checkpoint_with_proof(
                &current,
                ApplyCheckpoint::TargetsBootstrapped,
                "6".repeat(64),
            )
            .expect("targets bootstrapped");
        mirror_twice(&pool, &spec, &current).await;

        // The operation guard requires the converter's durable completion
        // record before accepting PostgresBulkCopied. This minimal empty-source
        // checkpoint uses the production CAS sink and is itself retried as if
        // its first commit acknowledgement had been lost.
        let conversion_binding = ConversionRunBinding {
            operation_id: OPERATION_ID.to_string(),
            target_installation_id: INSTALLATION_ID.to_string(),
            source_snapshot_sha256: SOURCE_FINGERPRINT.to_string(),
            source_schema_sha256: LEGACY_SEMANTIC_SCHEMA_SHA256.to_string(),
            registry_sha256: registry_sha256().expect("converter registry hash"),
            strategy: LegacyConversionStrategy::PreserveAll,
        };
        let complete_copy = ConversionCheckpoint {
            binding: conversion_binding,
            phase: ConversionPhase::Complete,
            table_order: 0,
            table: "empty_source".to_string(),
            last_source_id: 0,
            source_rows_seen: 0,
            target_rows_verified: 0,
            rolling_sha256: "2".repeat(64),
        };
        let mut checkpoint_sink = PostgresDurableCopyCheckpointSink::new(
            &pool,
            backup_reference_sha256(BACKUP_REFERENCE).expect("backup reference hash"),
        )
        .expect("PostgreSQL copy checkpoint sink");
        checkpoint_sink
            .compare_and_store(None, &complete_copy)
            .await
            .expect("durable empty-source copy completion");
        checkpoint_sink
            .compare_and_store(None, &complete_copy)
            .await
            .expect("retry copy completion after lost acknowledgement");

        current = journal
            .checkpoint_with_proof(
                &current,
                ApplyCheckpoint::PostgresBulkCopied,
                "7".repeat(64),
            )
            .expect("PostgreSQL copied");

        // The database guard must reject cp7 until the independently durable
        // Redis traffic fact has been folded and sealed. This fixture has an
        // empty legacy traffic delta, but still records a seal bound to the
        // exact SourceDrained journal event; zero traffic is a verified source
        // fact rather than permission to omit the fold ledger.
        assert!(
            mirror_lifecycle_snapshot(
                &pool,
                &spec,
                &current,
                ledger_binding.authorization_audit(),
            )
                .await
                .is_err(),
            "bulk-copy checkpoint must fail closed before the traffic fold seal"
        );
        let source_drained = initial_history
            .iter()
            .find(|snapshot| snapshot.checkpoint() == ApplyCheckpoint::SourceDrained)
            .expect("SourceDrained fixture event");
        sqlx::query(
            "INSERT INTO v2_legacy_traffic_fold (\
             operation_id, target_installation_id, source_default_run_id, \
             source_drain_receipt_sha256, source_drained_journal_generation, \
             source_drained_journal_event_sha256, source_drained_report_sha256, fenced_at, \
             upload_fields, download_fields, sorted_user_delta_count, \
             sorted_user_delta_sha256, upload_delta_sum, download_delta_sum, \
             fold_verification_sha256, seal_sha256, applied_at) VALUES (\
             $1, $2, $3, $4, $5, $6, $7, $8, \
             $9::numeric, $10::numeric, $11::numeric, $12, \
             $13::numeric, $14::numeric, $15, $16, $17)",
        )
        .bind(Uuid::parse_str(OPERATION_ID).expect("operation UUID"))
        .bind(Uuid::parse_str(INSTALLATION_ID).expect("installation UUID"))
        .bind("1".repeat(40))
        .bind("2".repeat(64))
        .bind(i64::try_from(source_drained.generation()).expect("journal generation fits i64"))
        .bind(source_drained.event_sha256())
        .bind(
            source_drained
                .checkpoint_proof_sha256()
                .expect("SourceDrained report hash"),
        )
        .bind(1_700_000_000_i64)
        .bind("0")
        .bind("0")
        .bind("0")
        .bind("3".repeat(64))
        .bind("0")
        .bind("0")
        .bind("4".repeat(64))
        .bind("5".repeat(64))
        .bind(1_700_000_001_i64)
        .execute(&pool)
        .await
        .expect("sealed zero-delta traffic fold");
        mirror_twice(&pool, &spec, &current).await;
        current = journal
            .enter_verification(&current)
            .expect("enter verification");
        mirror_twice(&pool, &spec, &current).await;
        for (checkpoint, proof) in [
            (
                ApplyCheckpoint::PostgresValueVerified,
                DATA_VERIFICATION_HASH,
            ),
            (
                ApplyCheckpoint::ClickhouseProjected,
                ANALYTICS_PROJECTION_HASH,
            ),
            (
                ApplyCheckpoint::RuntimeMaterialized,
                RUNTIME_MATERIALIZED_HASH,
            ),
            (ApplyCheckpoint::NodesVerified, NODE_CUTOVER_HASH),
        ] {
            current = journal
                .checkpoint_with_proof(&current, checkpoint, proof)
                .expect("advance verification checkpoint");
            mirror_twice(&pool, &spec, &current).await;
        }

        let fresh_permit = journal
            .target_mutation_permit(&current)
            .expect("fresh nodes-verified mutation permit");
        let activation_proof = NativeActivationProofBinding::new(
            DATA_VERIFICATION_HASH,
            ANALYTICS_PROJECTION_HASH,
            NODE_CUTOVER_HASH,
        )
        .expect("activation proof");
        assert!(
            observe_native_activation_commit(
                &pool,
                &spec,
                &current,
                ledger_binding.authorization_audit(),
            )
            .await
            .expect("observe absent activation")
            .is_none()
        );
        assert!(matches!(
            commit_native_activation(
                &pool,
                &spec,
                &initial_permit,
                &current,
                &activation_proof,
                ledger_binding.authorization_audit(),
            )
            .await,
            Err(LifecycleLedgerError::ActivationCheckpointRequired)
        ));
        let activation = commit_native_activation(
            &pool,
            &spec,
            &fresh_permit,
            &current,
            &activation_proof,
            ledger_binding.authorization_audit(),
        )
        .await
        .expect("atomic native activation");
        let activation_retry = commit_native_activation(
            &pool,
            &spec,
            &fresh_permit,
            &current,
            &activation_proof,
            ledger_binding.authorization_audit(),
        )
        .await
        .expect("retry activation after lost acknowledgement");
        assert_eq!(activation, activation_retry);

        let mismatched_activation = NativeActivationProofBinding::new(
            "3".repeat(64),
            ANALYTICS_PROJECTION_HASH,
            NODE_CUTOVER_HASH,
        )
        .expect("well-formed alternate activation proof");
        assert!(matches!(
            commit_native_activation(
                &pool,
                &spec,
                &fresh_permit,
                &current,
                &mismatched_activation,
                ledger_binding.authorization_audit(),
            )
            .await,
            Err(LifecycleLedgerError::ConflictingTargetState)
        ));

        current = journal
            .mark_needs_recovery(&current, ApplyOutcomeCode::ActivationFailed)
            .expect("persist indeterminate activation response");
        mirror_twice(&pool, &spec, &current).await;
        let observed_during_recovery = observe_native_activation_commit(
            &pool,
            &spec,
            &current,
            ledger_binding.authorization_audit(),
        )
        .await
        .expect("read-only activation observation")
        .expect("activation commit survived uncertain response");
        assert_eq!(observed_during_recovery, activation);
        current = journal.resume(&current).expect("resume nodes verification");
        mirror_twice(&pool, &spec, &current).await;
        let observed = observe_native_activation_commit(
            &pool,
            &spec,
            &current,
            ledger_binding.authorization_audit(),
        )
        .await
        .expect("observe committed activation after resume")
        .expect("activation commit");
        assert_eq!(observed, activation);
        current = journal
            .record_native_authority_committed(&current, observed.native_authority_binding())
            .expect("fsync observed native authority");
        mirror_twice(&pool, &spec, &current).await;
        assert!(matches!(
            journal.target_mutation_permit(&current),
            Err(crate::apply_journal::ApplyJournalError::TargetMutationNotAuthorized)
        ));

        drop(journal);
        let journal_binding =
            ApplyJournalBinding::new(OPERATION_ID, INSPECT_HASH).expect("journal binding");
        let (journal, reopened_authority) = ApplyJournal::open(root.path(), journal_binding)
            .expect("reopen after authority commit crash boundary");
        current = reopened_authority;
        let start_permit = journal
            .native_start_permit(&current)
            .expect("post-authority native start permit");
        assert_eq!(
            start_permit.native_authority_binding(),
            activation.native_authority_binding()
        );

        current = journal
            .checkpoint_with_proof(&current, ApplyCheckpoint::CutoverCommitted, "4".repeat(64))
            .expect("cutover committed journal event");
        mirror_twice(&pool, &spec, &current).await;
        current = journal
            .checkpoint_with_proof(&current, ApplyCheckpoint::SourceRetired, "5".repeat(64))
            .expect("source retired journal event");
        mirror_twice(&pool, &spec, &current).await;
        current = journal
            .complete(&current, "6".repeat(64))
            .expect("completion journal event");

        let mismatched_completion = CompletionProofBinding::new(
            "3".repeat(64),
            ANALYTICS_PROJECTION_HASH,
            NODE_CUTOVER_HASH,
            "a".repeat(64),
            true,
            "archive:legacy/final.snapshot",
            COLD_ARCHIVE_HASH,
        )
        .expect("well-formed alternate completion proof");
        assert!(matches!(
            complete_lifecycle_ledger(
                &pool,
                &spec,
                &current,
                &mismatched_completion,
                ledger_binding.authorization_audit(),
            )
            .await,
            Err(LifecycleLedgerError::ConflictingTargetState)
        ));

        let completion_proof = CompletionProofBinding::new(
            DATA_VERIFICATION_HASH,
            ANALYTICS_PROJECTION_HASH,
            NODE_CUTOVER_HASH,
            "a".repeat(64),
            true,
            "archive:legacy/final.snapshot",
            COLD_ARCHIVE_HASH,
        )
        .expect("completion proof");
        assert!(matches!(
            complete_lifecycle_ledger(
                &pool,
                &spec,
                &current,
                &completion_proof,
                &tampered_authorization_audit,
            )
            .await,
            Err(LifecycleLedgerError::ConflictingTargetState)
        ));
        complete_lifecycle_ledger(
            &pool,
            &spec,
            &current,
            &completion_proof,
            ledger_binding.authorization_audit(),
        )
        .await
        .expect("complete source retirement ledger");
        complete_lifecycle_ledger(
            &pool,
            &spec,
            &current,
            &completion_proof,
            ledger_binding.authorization_audit(),
        )
        .await
        .expect("retry completion after lost acknowledgement");
        assert!(matches!(
            complete_lifecycle_ledger(
                &pool,
                &spec,
                &current,
                &mismatched_completion,
                ledger_binding.authorization_audit(),
            )
            .await,
            Err(LifecycleLedgerError::ConflictingTargetState)
        ));

        let final_installation: (String, String, Option<i64>) = sqlx::query_as(
            "SELECT lineage, state, activated_at FROM v2_system_installation WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("final installation");
        assert_eq!(final_installation.0, "native");
        assert_eq!(final_installation.1, "active");
        assert_eq!(final_installation.2, Some(activation.activated_at_unix()));

        let final_operation: (String, i16, bool, bool, bool, bool, bool, String, String) =
            sqlx::query_as(
                "SELECT state, checkpoint, source_retired, mysql_reachable, \
                        source_redis_reachable, source_access_permanently_disabled, \
                        legacy_runtime_compat, cold_archive_reference, cold_archive_sha256 \
                 FROM v2_lifecycle_operation WHERE operation_id = $1",
            )
            .bind(Uuid::parse_str(OPERATION_ID).expect("operation UUID"))
            .fetch_one(&pool)
            .await
            .expect("completed lifecycle operation");
        assert_eq!(
            final_operation,
            (
                "completed".to_string(),
                15,
                true,
                false,
                false,
                true,
                false,
                "archive:legacy/final.snapshot".to_string(),
                COLD_ARCHIVE_HASH.to_string(),
            )
        );
        let final_authorization_audit: (String, String, String, String) = sqlx::query_as(
            "SELECT authorized_snapshot_report_sha256, \
                    authorized_snapshot_report_binding_hmac_sha256, \
                    authorization_binding_hmac_sha256, authorization_file_sha256 \
             FROM v2_lifecycle_operation WHERE operation_id = $1",
        )
        .bind(Uuid::parse_str(OPERATION_ID).expect("operation UUID"))
        .fetch_one(&pool)
        .await
        .expect("permanent authorization audit identity");
        assert_eq!(
            final_authorization_audit,
            (
                AUTHORIZED_SNAPSHOT_HASH.to_string(),
                AUTHORIZED_SNAPSHOT_HMAC.to_string(),
                AUTHORIZATION_HMAC.to_string(),
                AUTHORIZATION_FILE_HASH.to_string(),
            )
        );
        let activation_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM v2_lifecycle_activation_commit WHERE operation_id = $1",
        )
        .bind(Uuid::parse_str(OPERATION_ID).expect("operation UUID"))
        .fetch_one(&pool)
        .await
        .expect("activation commit count");
        assert_eq!(activation_count, 1);
        let event_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_lifecycle_event WHERE operation_id = $1")
                .bind(Uuid::parse_str(OPERATION_ID).expect("operation UUID"))
                .fetch_one(&pool)
                .await
                .expect("mirrored event count");
        assert_eq!(
            event_count,
            i64::try_from(journal.verified_history().expect("final history").len())
                .expect("history length")
        );
        api_pool.close().await;
        worker_pool.close().await;
        pool.close().await;
    }

    async fn mirror_twice(pool: &PgPool, spec: &ProvisionSpec, snapshot: &ApplyJournalSnapshot) {
        let authorization_audit = authorization_audit_binding();
        mirror_lifecycle_snapshot(pool, spec, snapshot, &authorization_audit)
            .await
            .expect("mirror lifecycle snapshot");
        mirror_lifecycle_snapshot(pool, spec, snapshot, &authorization_audit)
            .await
            .expect("retry lifecycle mirror after lost acknowledgement");
    }
}
