-- V2Board native PostgreSQL 18 append-only lineage v2.
-- Extends the immutable 0001 baseline with one-shot legacy lifecycle state,
-- durable traffic folding, gift-card timestamp provenance, and analytics
-- admission control.

-- The filesystem journal is authoritative before the target database exists.
-- Once bootstrap succeeds, every event is mirrored here in the same operation
-- identity so recovery does not depend on an ephemeral migration host.
CREATE TABLE v2_lifecycle_operation (
    operation_id UUID PRIMARY KEY,
    installation_id UUID NOT NULL UNIQUE
        REFERENCES v2_system_installation(installation_id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind = 'legacy_reference_migration'),
    manifest_binding_hmac_sha256 TEXT NOT NULL
        CHECK (manifest_binding_hmac_sha256 ~ '^[0-9a-f]{64}$'),
    inspect_review_sha256 TEXT NOT NULL
        CHECK (inspect_review_sha256 ~ '^[0-9a-f]{64}$'),
    authorized_snapshot_report_sha256 TEXT NOT NULL
        CHECK (authorized_snapshot_report_sha256 ~ '^[0-9a-f]{64}$'),
    authorized_snapshot_report_binding_hmac_sha256 TEXT NOT NULL
        CHECK (authorized_snapshot_report_binding_hmac_sha256 ~ '^[0-9a-f]{64}$'),
    authorization_binding_hmac_sha256 TEXT NOT NULL
        CHECK (authorization_binding_hmac_sha256 ~ '^[0-9a-f]{64}$'),
    authorization_file_sha256 TEXT NOT NULL
        CHECK (authorization_file_sha256 ~ '^[0-9a-f]{64}$'),
    source_fingerprint_sha256 TEXT NOT NULL
        CHECK (source_fingerprint_sha256 ~ '^[0-9a-f]{64}$'),
    converter_registry_sha256 TEXT NOT NULL
        CHECK (converter_registry_sha256 ~ '^[0-9a-f]{64}$'),
    target_lineage_sha256 TEXT NOT NULL
        CHECK (target_lineage_sha256 ~ '^[0-9a-f]{64}$'),
    state TEXT NOT NULL CHECK (
        state IN ('pending', 'running', 'verifying', 'needs_recovery', 'failed', 'completed')
    ),
    checkpoint SMALLINT NOT NULL CHECK (checkpoint BETWEEN 0 AND 15),
    journal_generation BIGINT NOT NULL CHECK (journal_generation >= 0),
    journal_event_sha256 TEXT NOT NULL
        CHECK (journal_event_sha256 ~ '^[0-9a-f]{64}$'),
    checkpoint_proof_sha256 TEXT
        CHECK (checkpoint_proof_sha256 IS NULL OR checkpoint_proof_sha256 ~ '^[0-9a-f]{64}$'),
    backup_reference TEXT,
    backup_restore_proof_sha256 TEXT
        CHECK (backup_restore_proof_sha256 IS NULL OR backup_restore_proof_sha256 ~ '^[0-9a-f]{64}$'),
    final_recheck_report_sha256 TEXT
        CHECK (final_recheck_report_sha256 IS NULL OR final_recheck_report_sha256 ~ '^[0-9a-f]{64}$'),
    native_authority_nodes_generation BIGINT
        CHECK (native_authority_nodes_generation IS NULL OR native_authority_nodes_generation >= 0),
    native_authority_nodes_event_sha256 TEXT
        CHECK (native_authority_nodes_event_sha256 IS NULL OR native_authority_nodes_event_sha256 ~ '^[0-9a-f]{64}$'),
    data_verification_report_sha256 TEXT
        CHECK (data_verification_report_sha256 IS NULL OR data_verification_report_sha256 ~ '^[0-9a-f]{64}$'),
    analytics_projection_report_sha256 TEXT
        CHECK (analytics_projection_report_sha256 IS NULL OR analytics_projection_report_sha256 ~ '^[0-9a-f]{64}$'),
    node_cutover_report_sha256 TEXT
        CHECK (node_cutover_report_sha256 IS NULL OR node_cutover_report_sha256 ~ '^[0-9a-f]{64}$'),
    source_retired BOOLEAN,
    mysql_reachable BOOLEAN,
    source_redis_reachable BOOLEAN,
    source_access_permanently_disabled BOOLEAN,
    legacy_runtime_compat BOOLEAN,
    cold_archive_reference TEXT,
    cold_archive_sha256 TEXT
        CHECK (cold_archive_sha256 IS NULL OR cold_archive_sha256 ~ '^[0-9a-f]{64}$'),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    completed_at BIGINT,
    CONSTRAINT chk_lifecycle_checkpoint_proof CHECK (
        (checkpoint IN (0, 5) AND checkpoint_proof_sha256 IS NULL)
        OR (checkpoint NOT IN (0, 5) AND checkpoint_proof_sha256 IS NOT NULL)
    ),
    CONSTRAINT chk_lifecycle_native_authority CHECK (
        (checkpoint < 12
            AND native_authority_nodes_generation IS NULL
            AND native_authority_nodes_event_sha256 IS NULL
            AND data_verification_report_sha256 IS NULL
            AND analytics_projection_report_sha256 IS NULL
            AND node_cutover_report_sha256 IS NULL)
        OR (checkpoint >= 12
            AND native_authority_nodes_generation IS NOT NULL
            AND native_authority_nodes_event_sha256 IS NOT NULL
            AND data_verification_report_sha256 IS NOT NULL
            AND analytics_projection_report_sha256 IS NOT NULL
            AND node_cutover_report_sha256 IS NOT NULL)
    ),
    CONSTRAINT chk_lifecycle_completed_proof CHECK (
        state <> 'completed'
        OR (
            checkpoint = 15
            AND backup_reference IS NOT NULL
            AND backup_restore_proof_sha256 IS NOT NULL
            AND final_recheck_report_sha256 IS NOT NULL
            AND data_verification_report_sha256 IS NOT NULL
            AND analytics_projection_report_sha256 IS NOT NULL
            AND node_cutover_report_sha256 IS NOT NULL
            AND source_retired IS TRUE
            AND mysql_reachable IS FALSE
            AND source_redis_reachable IS FALSE
            AND source_access_permanently_disabled IS TRUE
            AND legacy_runtime_compat IS FALSE
            AND cold_archive_reference IS NOT NULL
            AND cold_archive_sha256 IS NOT NULL
            AND completed_at IS NOT NULL
        )
    ),
    UNIQUE (operation_id, installation_id)
);

CREATE TABLE v2_lifecycle_event (
    operation_id UUID NOT NULL
        REFERENCES v2_lifecycle_operation(operation_id) ON DELETE RESTRICT,
    generation BIGINT NOT NULL CHECK (generation >= 0),
    state TEXT NOT NULL CHECK (
        state IN ('pending', 'running', 'verifying', 'needs_recovery', 'failed', 'completed')
    ),
    checkpoint SMALLINT NOT NULL CHECK (checkpoint BETWEEN 0 AND 15),
    outcome_code TEXT CHECK (
        outcome_code IS NULL OR outcome_code IN (
            'process_interrupted', 'io_failure', 'source_drift', 'target_drift',
            'fence_uncertain', 'drain_incomplete', 'backup_invalid', 'conversion_failed',
            'verification_mismatch', 'activation_failed', 'retirement_failed', 'operator_abort'
        )
    ),
    previous_event_sha256 TEXT
        CHECK (previous_event_sha256 IS NULL OR previous_event_sha256 ~ '^[0-9a-f]{64}$'),
    event_sha256 TEXT NOT NULL CHECK (event_sha256 ~ '^[0-9a-f]{64}$'),
    checkpoint_proof_sha256 TEXT
        CHECK (checkpoint_proof_sha256 IS NULL OR checkpoint_proof_sha256 ~ '^[0-9a-f]{64}$'),
    installation_id UUID,
    backup_restore_proof_sha256 TEXT
        CHECK (backup_restore_proof_sha256 IS NULL OR backup_restore_proof_sha256 ~ '^[0-9a-f]{64}$'),
    backup_reference_sha256 TEXT
        CHECK (backup_reference_sha256 IS NULL OR backup_reference_sha256 ~ '^[0-9a-f]{64}$'),
    final_recheck_report_sha256 TEXT
        CHECK (final_recheck_report_sha256 IS NULL OR final_recheck_report_sha256 ~ '^[0-9a-f]{64}$'),
    source_fingerprint_sha256 TEXT
        CHECK (source_fingerprint_sha256 IS NULL OR source_fingerprint_sha256 ~ '^[0-9a-f]{64}$'),
    native_authority_nodes_generation BIGINT
        CHECK (native_authority_nodes_generation IS NULL OR native_authority_nodes_generation >= 0),
    native_authority_nodes_event_sha256 TEXT
        CHECK (native_authority_nodes_event_sha256 IS NULL OR native_authority_nodes_event_sha256 ~ '^[0-9a-f]{64}$'),
    data_verification_report_sha256 TEXT
        CHECK (data_verification_report_sha256 IS NULL OR data_verification_report_sha256 ~ '^[0-9a-f]{64}$'),
    analytics_projection_report_sha256 TEXT
        CHECK (analytics_projection_report_sha256 IS NULL OR analytics_projection_report_sha256 ~ '^[0-9a-f]{64}$'),
    node_cutover_report_sha256 TEXT
        CHECK (node_cutover_report_sha256 IS NULL OR node_cutover_report_sha256 ~ '^[0-9a-f]{64}$'),
    recorded_at_unix_ms BIGINT NOT NULL CHECK (recorded_at_unix_ms > 0),
    CHECK (
        (generation = 0 AND previous_event_sha256 IS NULL
            AND state = 'pending' AND checkpoint = 0)
        OR (generation > 0 AND previous_event_sha256 IS NOT NULL)
    ),
    CHECK (
        (state IN ('needs_recovery', 'failed') AND outcome_code IS NOT NULL)
        OR (state NOT IN ('needs_recovery', 'failed') AND outcome_code IS NULL)
    ),
    CHECK (
        (checkpoint < 3 AND backup_restore_proof_sha256 IS NULL
            AND backup_reference_sha256 IS NULL)
        OR (checkpoint >= 3 AND backup_restore_proof_sha256 IS NOT NULL
            AND backup_reference_sha256 IS NOT NULL)
    ),
    CHECK (
        (checkpoint < 4 AND final_recheck_report_sha256 IS NULL
            AND source_fingerprint_sha256 IS NULL)
        OR (checkpoint >= 4 AND final_recheck_report_sha256 IS NOT NULL
            AND source_fingerprint_sha256 IS NOT NULL)
    ),
    CHECK (
        (checkpoint < 5 AND installation_id IS NULL)
        OR (checkpoint >= 5 AND installation_id IS NOT NULL)
    ),
    CHECK (
        (checkpoint IN (0, 5) AND checkpoint_proof_sha256 IS NULL)
        OR (checkpoint NOT IN (0, 5) AND checkpoint_proof_sha256 IS NOT NULL)
    ),
    CHECK (
        (checkpoint < 12
            AND native_authority_nodes_generation IS NULL
            AND native_authority_nodes_event_sha256 IS NULL
            AND data_verification_report_sha256 IS NULL
            AND analytics_projection_report_sha256 IS NULL
            AND node_cutover_report_sha256 IS NULL)
        OR (checkpoint >= 12
            AND native_authority_nodes_generation IS NOT NULL
            AND native_authority_nodes_event_sha256 IS NOT NULL
            AND data_verification_report_sha256 IS NOT NULL
            AND analytics_projection_report_sha256 IS NOT NULL
            AND node_cutover_report_sha256 IS NOT NULL)
    ),
    FOREIGN KEY (operation_id, installation_id)
        REFERENCES v2_lifecycle_operation(operation_id, installation_id)
        MATCH SIMPLE ON DELETE RESTRICT,
    PRIMARY KEY (operation_id, generation),
    UNIQUE (operation_id, event_sha256),
    UNIQUE (operation_id, generation, event_sha256)
);

-- The activation commit is deliberately separate from the journal head. It
-- binds the last pre-start verification event and proofs in the same
-- transaction that activates the installation, without inventing a future
-- filesystem journal event merely to satisfy the operation update guard.
CREATE TABLE v2_lifecycle_activation_commit (
    operation_id UUID PRIMARY KEY
        REFERENCES v2_lifecycle_operation(operation_id) ON DELETE RESTRICT,
    installation_id UUID NOT NULL UNIQUE
        REFERENCES v2_system_installation(installation_id) ON DELETE RESTRICT,
    journal_generation BIGINT NOT NULL CHECK (journal_generation >= 0),
    journal_state TEXT NOT NULL CHECK (journal_state = 'verifying'),
    journal_checkpoint SMALLINT NOT NULL CHECK (journal_checkpoint = 11),
    journal_event_sha256 TEXT NOT NULL
        CHECK (journal_event_sha256 ~ '^[0-9a-f]{64}$'),
    data_verification_report_sha256 TEXT NOT NULL
        CHECK (data_verification_report_sha256 ~ '^[0-9a-f]{64}$'),
    analytics_projection_report_sha256 TEXT NOT NULL
        CHECK (analytics_projection_report_sha256 ~ '^[0-9a-f]{64}$'),
    node_cutover_report_sha256 TEXT NOT NULL
        CHECK (node_cutover_report_sha256 ~ '^[0-9a-f]{64}$'),
    committed_at BIGINT NOT NULL CHECK (committed_at > 0),
    FOREIGN KEY (operation_id, installation_id)
        REFERENCES v2_lifecycle_operation(operation_id, installation_id) ON DELETE RESTRICT,
    FOREIGN KEY (operation_id, journal_generation, journal_event_sha256)
        REFERENCES v2_lifecycle_event(operation_id, generation, event_sha256) ON DELETE RESTRICT,
    UNIQUE (
        operation_id, journal_generation, journal_event_sha256,
        data_verification_report_sha256, analytics_projection_report_sha256,
        node_cutover_report_sha256
    )
);

ALTER TABLE v2_lifecycle_event
    ADD CONSTRAINT fk_lifecycle_event_native_authority
    FOREIGN KEY (
        operation_id, native_authority_nodes_generation,
        native_authority_nodes_event_sha256, data_verification_report_sha256,
        analytics_projection_report_sha256, node_cutover_report_sha256
    ) REFERENCES v2_lifecycle_activation_commit (
        operation_id, journal_generation, journal_event_sha256,
        data_verification_report_sha256, analytics_projection_report_sha256,
        node_cutover_report_sha256
    ) MATCH SIMPLE ON DELETE RESTRICT;

-- Exact restart cursor for the one-shot MySQL snapshot converter. Rows are a
-- closed, hash-chained checkpoint log; the adapter locks the lifecycle
-- operation and performs the latest-head CAS before every insert.
CREATE TABLE v2_legacy_copy_checkpoint (
    operation_id UUID NOT NULL,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    target_installation_id UUID NOT NULL,
    source_snapshot_sha256 TEXT NOT NULL
        CHECK (source_snapshot_sha256 ~ '^[0-9a-f]{64}$'),
    source_schema_sha256 TEXT NOT NULL
        CHECK (source_schema_sha256 ~ '^[0-9a-f]{64}$'),
    registry_sha256 TEXT NOT NULL
        CHECK (registry_sha256 ~ '^[0-9a-f]{64}$'),
    phase TEXT NOT NULL CHECK (phase IN (
        'copy_base_tables', 'apply_deferred_references', 'build_derived_rows',
        'reset_sequences', 'fold_frozen_traffic', 'verify_all_values', 'complete'
    )),
    table_order INTEGER NOT NULL CHECK (table_order BETWEEN 0 AND 65535),
    table_name TEXT NOT NULL CHECK (
        table_name ~ '^[a-z][a-z0-9_]{0,62}$'
    ),
    last_source_id BIGINT NOT NULL,
    source_rows_seen NUMERIC(20, 0) NOT NULL CHECK (
        source_rows_seen BETWEEN 0 AND 18446744073709551615
    ),
    target_rows_verified NUMERIC(20, 0) NOT NULL CHECK (
        target_rows_verified BETWEEN 0 AND 18446744073709551615
    ),
    rolling_sha256 TEXT NOT NULL CHECK (rolling_sha256 ~ '^[0-9a-f]{64}$'),
    previous_checkpoint_sha256 TEXT
        CHECK (previous_checkpoint_sha256 IS NULL OR previous_checkpoint_sha256 ~ '^[0-9a-f]{64}$'),
    checkpoint_sha256 TEXT NOT NULL CHECK (checkpoint_sha256 ~ '^[0-9a-f]{64}$'),
    recorded_at BIGINT NOT NULL CHECK (recorded_at > 0),
    CHECK (
        (sequence = 0 AND previous_checkpoint_sha256 IS NULL)
        OR (sequence > 0 AND previous_checkpoint_sha256 IS NOT NULL)
    ),
    PRIMARY KEY (operation_id, sequence),
    UNIQUE (operation_id, checkpoint_sha256),
    FOREIGN KEY (operation_id, target_installation_id)
        REFERENCES v2_lifecycle_operation(operation_id, installation_id) ON DELETE RESTRICT,
    FOREIGN KEY (operation_id, previous_checkpoint_sha256)
        REFERENCES v2_legacy_copy_checkpoint(operation_id, checkpoint_sha256)
        MATCH SIMPLE ON DELETE RESTRICT
);

-- A frozen legacy Redis traffic receipt is a separate source fact from the
-- 27-table MySQL snapshot. Every per-user delta and the corresponding user
-- mutation commit in one durable transaction; the seal is inserted last and
-- makes an operation retry an exact comparison instead of a second addition.
CREATE TABLE v2_legacy_traffic_fold_item (
    operation_id UUID NOT NULL,
    target_installation_id UUID NOT NULL,
    user_id BIGINT NOT NULL,
    upload_delta BIGINT NOT NULL CHECK (upload_delta >= 0),
    download_delta BIGINT NOT NULL CHECK (download_delta >= 0),
    before_u BIGINT NOT NULL,
    before_d BIGINT NOT NULL,
    before_t BIGINT NOT NULL,
    before_updated_at BIGINT NOT NULL,
    after_u BIGINT NOT NULL,
    after_d BIGINT NOT NULL,
    after_t BIGINT NOT NULL CHECK (after_t > 0),
    after_updated_at BIGINT NOT NULL CHECK (after_updated_at > 0),
    item_sha256 TEXT NOT NULL CHECK (item_sha256 ~ '^[0-9a-f]{64}$'),
    PRIMARY KEY (operation_id, user_id),
    UNIQUE (operation_id, item_sha256),
    FOREIGN KEY (operation_id, target_installation_id)
        REFERENCES v2_lifecycle_operation(operation_id, installation_id) ON DELETE RESTRICT,
    CHECK (after_u::numeric = before_u::numeric + upload_delta::numeric),
    CHECK (after_d::numeric = before_d::numeric + download_delta::numeric),
    CHECK (after_t = after_updated_at)
);

CREATE TABLE v2_legacy_traffic_fold (
    operation_id UUID PRIMARY KEY,
    target_installation_id UUID NOT NULL,
    source_default_run_id TEXT NOT NULL
        CHECK (source_default_run_id ~ '^[0-9a-f]{40}$'),
    source_drain_receipt_sha256 TEXT NOT NULL
        CHECK (source_drain_receipt_sha256 ~ '^[0-9a-f]{64}$'),
    source_drained_journal_generation BIGINT NOT NULL
        CHECK (source_drained_journal_generation >= 0),
    source_drained_journal_event_sha256 TEXT NOT NULL
        CHECK (source_drained_journal_event_sha256 ~ '^[0-9a-f]{64}$'),
    source_drained_report_sha256 TEXT NOT NULL
        CHECK (source_drained_report_sha256 ~ '^[0-9a-f]{64}$'),
    fenced_at BIGINT NOT NULL CHECK (fenced_at > 0),
    upload_fields NUMERIC(20, 0) NOT NULL
        CHECK (upload_fields BETWEEN 0 AND 18446744073709551615),
    download_fields NUMERIC(20, 0) NOT NULL
        CHECK (download_fields BETWEEN 0 AND 18446744073709551615),
    sorted_user_delta_count NUMERIC(20, 0) NOT NULL
        CHECK (sorted_user_delta_count BETWEEN 0 AND 18446744073709551615),
    sorted_user_delta_sha256 TEXT NOT NULL
        CHECK (sorted_user_delta_sha256 ~ '^[0-9a-f]{64}$'),
    upload_delta_sum NUMERIC(39, 0) NOT NULL CHECK (upload_delta_sum >= 0),
    download_delta_sum NUMERIC(39, 0) NOT NULL CHECK (download_delta_sum >= 0),
    fold_verification_sha256 TEXT NOT NULL
        CHECK (fold_verification_sha256 ~ '^[0-9a-f]{64}$'),
    seal_sha256 TEXT NOT NULL CHECK (seal_sha256 ~ '^[0-9a-f]{64}$'),
    applied_at BIGINT NOT NULL CHECK (applied_at > 0),
    UNIQUE (source_default_run_id, source_drain_receipt_sha256),
    UNIQUE (operation_id, seal_sha256),
    FOREIGN KEY (operation_id, target_installation_id)
        REFERENCES v2_lifecycle_operation(operation_id, installation_id) ON DELETE RESTRICT
);

CREATE FUNCTION v2_guard_lifecycle_operation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'lifecycle operation history cannot be deleted';
    END IF;
    IF NEW.operation_id IS DISTINCT FROM OLD.operation_id
       OR NEW.installation_id IS DISTINCT FROM OLD.installation_id
       OR NEW.kind IS DISTINCT FROM OLD.kind
       OR NEW.manifest_binding_hmac_sha256 IS DISTINCT FROM OLD.manifest_binding_hmac_sha256
       OR NEW.inspect_review_sha256 IS DISTINCT FROM OLD.inspect_review_sha256
       OR NEW.authorized_snapshot_report_sha256 IS DISTINCT FROM OLD.authorized_snapshot_report_sha256
       OR NEW.authorized_snapshot_report_binding_hmac_sha256 IS DISTINCT FROM OLD.authorized_snapshot_report_binding_hmac_sha256
       OR NEW.authorization_binding_hmac_sha256 IS DISTINCT FROM OLD.authorization_binding_hmac_sha256
       OR NEW.authorization_file_sha256 IS DISTINCT FROM OLD.authorization_file_sha256
       OR NEW.source_fingerprint_sha256 IS DISTINCT FROM OLD.source_fingerprint_sha256
       OR NEW.converter_registry_sha256 IS DISTINCT FROM OLD.converter_registry_sha256
       OR NEW.target_lineage_sha256 IS DISTINCT FROM OLD.target_lineage_sha256
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'immutable lifecycle operation binding changed';
    END IF;
    IF OLD.state IN ('failed', 'completed') THEN
        RAISE EXCEPTION 'terminal lifecycle operation cannot change';
    END IF;
    IF NEW.journal_generation <> OLD.journal_generation + 1
       OR NEW.checkpoint < OLD.checkpoint
       OR NEW.checkpoint > OLD.checkpoint + 1
       OR NEW.updated_at < OLD.updated_at
       OR NEW.journal_event_sha256 IS NOT DISTINCT FROM OLD.journal_event_sha256 THEN
        RAISE EXCEPTION 'lifecycle journal head did not advance monotonically';
    END IF;
    IF NOT (
        (OLD.state = 'pending' AND NEW.state IN ('running', 'needs_recovery', 'failed'))
        OR (OLD.state = 'running' AND NEW.state IN ('running', 'verifying', 'needs_recovery', 'failed'))
        OR (OLD.state = 'verifying' AND NEW.state IN ('verifying', 'needs_recovery', 'failed', 'completed'))
        OR (OLD.state = 'needs_recovery' AND NEW.state IN ('running', 'verifying', 'failed'))
    ) THEN
        RAISE EXCEPTION 'invalid lifecycle state transition: % -> %', OLD.state, NEW.state;
    END IF;
    IF NEW.state = 'completed'
       AND NOT (
           OLD.state = 'verifying' AND OLD.checkpoint = 14
           AND NEW.checkpoint = 15
       ) THEN
        RAISE EXCEPTION 'lifecycle completion requires the source_retired predecessor';
    END IF;
    IF OLD.checkpoint < 7 AND NEW.checkpoint >= 7 AND NOT EXISTS (
        SELECT 1 FROM v2_legacy_copy_checkpoint
        WHERE operation_id = OLD.operation_id
          AND target_installation_id = OLD.installation_id
          AND registry_sha256 = OLD.converter_registry_sha256
          AND phase = 'complete'
    ) THEN
        RAISE EXCEPTION 'postgres bulk-copy checkpoint requires a complete converter checkpoint';
    END IF;
    IF OLD.checkpoint < 7 AND NEW.checkpoint >= 7 AND NOT EXISTS (
        SELECT 1 FROM v2_legacy_traffic_fold
        WHERE operation_id = OLD.operation_id
          AND target_installation_id = OLD.installation_id
    ) THEN
        RAISE EXCEPTION 'postgres bulk-copy checkpoint requires a sealed legacy traffic fold';
    END IF;
    IF NEW.checkpoint = OLD.checkpoint
       AND NEW.checkpoint_proof_sha256 IS DISTINCT FROM OLD.checkpoint_proof_sha256 THEN
        RAISE EXCEPTION 'same lifecycle checkpoint cannot replace its proof';
    END IF;
    IF OLD.checkpoint < 12 AND NEW.checkpoint >= 12 AND NOT EXISTS (
        SELECT 1 FROM v2_lifecycle_activation_commit
        WHERE operation_id = OLD.operation_id
          AND installation_id = OLD.installation_id
          AND journal_generation = NEW.native_authority_nodes_generation
          AND journal_event_sha256 = NEW.native_authority_nodes_event_sha256
          AND data_verification_report_sha256 = NEW.data_verification_report_sha256
          AND analytics_projection_report_sha256 = NEW.analytics_projection_report_sha256
          AND node_cutover_report_sha256 = NEW.node_cutover_report_sha256
    ) THEN
        RAISE EXCEPTION 'native authority checkpoint does not match the immutable activation commit';
    END IF;
    IF (OLD.backup_reference IS NOT NULL AND NEW.backup_reference IS DISTINCT FROM OLD.backup_reference)
       OR (OLD.backup_restore_proof_sha256 IS NOT NULL AND NEW.backup_restore_proof_sha256 IS DISTINCT FROM OLD.backup_restore_proof_sha256)
       OR (OLD.final_recheck_report_sha256 IS NOT NULL AND NEW.final_recheck_report_sha256 IS DISTINCT FROM OLD.final_recheck_report_sha256)
       OR (OLD.native_authority_nodes_generation IS NOT NULL AND NEW.native_authority_nodes_generation IS DISTINCT FROM OLD.native_authority_nodes_generation)
       OR (OLD.native_authority_nodes_event_sha256 IS NOT NULL AND NEW.native_authority_nodes_event_sha256 IS DISTINCT FROM OLD.native_authority_nodes_event_sha256)
       OR (OLD.data_verification_report_sha256 IS NOT NULL AND NEW.data_verification_report_sha256 IS DISTINCT FROM OLD.data_verification_report_sha256)
       OR (OLD.analytics_projection_report_sha256 IS NOT NULL AND NEW.analytics_projection_report_sha256 IS DISTINCT FROM OLD.analytics_projection_report_sha256)
       OR (OLD.node_cutover_report_sha256 IS NOT NULL AND NEW.node_cutover_report_sha256 IS DISTINCT FROM OLD.node_cutover_report_sha256)
       OR (OLD.source_retired IS NOT NULL AND NEW.source_retired IS DISTINCT FROM OLD.source_retired)
       OR (OLD.mysql_reachable IS NOT NULL AND NEW.mysql_reachable IS DISTINCT FROM OLD.mysql_reachable)
       OR (OLD.source_redis_reachable IS NOT NULL AND NEW.source_redis_reachable IS DISTINCT FROM OLD.source_redis_reachable)
       OR (OLD.source_access_permanently_disabled IS NOT NULL AND NEW.source_access_permanently_disabled IS DISTINCT FROM OLD.source_access_permanently_disabled)
       OR (OLD.legacy_runtime_compat IS NOT NULL AND NEW.legacy_runtime_compat IS DISTINCT FROM OLD.legacy_runtime_compat)
       OR (OLD.cold_archive_reference IS NOT NULL AND NEW.cold_archive_reference IS DISTINCT FROM OLD.cold_archive_reference)
       OR (OLD.cold_archive_sha256 IS NOT NULL AND NEW.cold_archive_sha256 IS DISTINCT FROM OLD.cold_archive_sha256) THEN
        RAISE EXCEPTION 'lifecycle proof binding cannot change once recorded';
    END IF;
    IF NEW.state = 'completed' AND NOT EXISTS (
        SELECT 1 FROM v2_lifecycle_activation_commit
        WHERE operation_id = OLD.operation_id
          AND installation_id = OLD.installation_id
          AND journal_generation = NEW.native_authority_nodes_generation
          AND journal_event_sha256 = NEW.native_authority_nodes_event_sha256
          AND data_verification_report_sha256 = NEW.data_verification_report_sha256
          AND analytics_projection_report_sha256 = NEW.analytics_projection_report_sha256
          AND node_cutover_report_sha256 = NEW.node_cutover_report_sha256
    ) THEN
        RAISE EXCEPTION 'completed lifecycle proofs do not match the activation commit';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_lifecycle_operation_guard
BEFORE UPDATE OR DELETE ON v2_lifecycle_operation
FOR EACH ROW EXECUTE FUNCTION v2_guard_lifecycle_operation();

CREATE FUNCTION v2_guard_lifecycle_event()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'lifecycle events are append-only';
END;
$$;

CREATE TRIGGER trg_lifecycle_event_append_only
BEFORE UPDATE OR DELETE ON v2_lifecycle_event
FOR EACH ROW EXECUTE FUNCTION v2_guard_lifecycle_event();

CREATE TRIGGER trg_lifecycle_activation_commit_append_only
BEFORE UPDATE OR DELETE ON v2_lifecycle_activation_commit
FOR EACH ROW EXECUTE FUNCTION v2_guard_lifecycle_event();

CREATE TRIGGER trg_legacy_copy_checkpoint_append_only
BEFORE UPDATE OR DELETE ON v2_legacy_copy_checkpoint
FOR EACH ROW EXECUTE FUNCTION v2_guard_lifecycle_event();

CREATE TRIGGER trg_legacy_traffic_fold_item_append_only
BEFORE UPDATE OR DELETE ON v2_legacy_traffic_fold_item
FOR EACH ROW EXECUTE FUNCTION v2_guard_lifecycle_event();

CREATE TRIGGER trg_legacy_traffic_fold_append_only
BEFORE UPDATE OR DELETE ON v2_legacy_traffic_fold
FOR EACH ROW EXECUTE FUNCTION v2_guard_lifecycle_event();


ALTER TABLE v2_system_installation
    ADD CONSTRAINT chk_system_installation_legacy_pending CHECK (
        lineage <> 'legacy_migrated' OR state = 'pending'
    );

CREATE OR REPLACE FUNCTION v2_guard_system_installation()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'installation identity cannot be deleted';
    END IF;
    IF NEW.singleton IS DISTINCT FROM OLD.singleton
       OR NEW.installation_id IS DISTINCT FROM OLD.installation_id
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'installation identity cannot be changed';
    END IF;
    IF NEW.lineage IS DISTINCT FROM OLD.lineage
       AND NOT (OLD.lineage = 'legacy_migrated' AND NEW.lineage = 'native') THEN
        RAISE EXCEPTION 'invalid installation lineage transition: % -> %', OLD.lineage, NEW.lineage;
    END IF;
    IF NEW.state IS DISTINCT FROM OLD.state
       AND NOT (
           (OLD.state = 'pending' AND NEW.state = 'active')
           OR (OLD.state = 'active' AND NEW.state = 'retired')
       ) THEN
        RAISE EXCEPTION 'invalid installation state transition: % -> %', OLD.state, NEW.state;
    END IF;
    IF OLD.activated_at IS NOT NULL
       AND NEW.activated_at IS DISTINCT FROM OLD.activated_at THEN
        RAISE EXCEPTION 'installation activation time cannot be changed';
    END IF;
    IF OLD.source_fingerprint_sha256 IS NOT NULL
       AND NEW.source_fingerprint_sha256 IS DISTINCT FROM OLD.source_fingerprint_sha256 THEN
        RAISE EXCEPTION 'installation source fingerprint cannot be changed once recorded';
    END IF;
    IF OLD.lineage = 'legacy_migrated' AND OLD.state = 'pending'
       AND (
           NEW.lineage IS DISTINCT FROM OLD.lineage
           OR NEW.state IS DISTINCT FROM OLD.state
       ) AND NOT (
           NEW.lineage = 'native' AND NEW.state = 'active'
           AND EXISTS (
               SELECT 1 FROM v2_lifecycle_activation_commit
               WHERE installation_id = OLD.installation_id
           )
       ) THEN
        RAISE EXCEPTION 'legacy installation lineage and state must activate together with an immutable commit';
    END IF;
    RETURN NEW;
END;
$$;

ALTER TABLE v2_legacy_traffic_fold_item
    ADD CONSTRAINT fk_legacy_traffic_fold_item_user
    FOREIGN KEY (user_id) REFERENCES v2_user(id) ON DELETE RESTRICT;

ALTER TABLE v2_giftcard_redemption
    ADD COLUMN created_at_provenance TEXT NOT NULL DEFAULT 'native'
        CHECK (created_at_provenance IN ('native', 'legacy_unknown'));

UPDATE v2_giftcard_redemption
SET created_at_provenance = 'legacy_unknown'
WHERE created_at = 0;

ALTER TABLE v2_giftcard_redemption
    ADD CONSTRAINT chk_giftcard_redemption_created_at_provenance CHECK (
        (created_at_provenance = 'native' AND created_at > 0)
        OR (created_at_provenance = 'legacy_unknown' AND created_at = 0)
    );

-- Machine-bound analytics admission policy. Runtime principals may read this
-- row but cannot alter it; only a lifecycle operation may install a policy for
-- the reserved installation identity.
CREATE TABLE v2_analytics_admission_policy (
    singleton SMALLINT PRIMARY KEY CHECK (singleton = 1),
    installation_id UUID NOT NULL UNIQUE
        REFERENCES v2_system_installation(installation_id) ON DELETE RESTRICT,
    policy_sha256 TEXT NOT NULL UNIQUE CHECK (policy_sha256 ~ '^[0-9a-f]{64}$'),
    recovery_pending_rows BIGINT NOT NULL CHECK (recovery_pending_rows >= 0),
    soft_pending_rows BIGINT NOT NULL CHECK (soft_pending_rows > recovery_pending_rows),
    hard_pending_rows BIGINT NOT NULL CHECK (hard_pending_rows > soft_pending_rows),
    recovery_relation_bytes BIGINT NOT NULL CHECK (recovery_relation_bytes >= 0),
    soft_relation_bytes BIGINT NOT NULL CHECK (soft_relation_bytes > recovery_relation_bytes),
    hard_relation_bytes BIGINT NOT NULL CHECK (hard_relation_bytes > soft_relation_bytes),
    recovery_oldest_age_seconds BIGINT NOT NULL CHECK (recovery_oldest_age_seconds >= 0),
    soft_oldest_age_seconds BIGINT NOT NULL CHECK (soft_oldest_age_seconds > recovery_oldest_age_seconds),
    hard_oldest_age_seconds BIGINT NOT NULL CHECK (hard_oldest_age_seconds > soft_oldest_age_seconds),
    database_capacity_bytes BIGINT NOT NULL CHECK (database_capacity_bytes > 0),
    hard_min_headroom_bytes BIGINT NOT NULL CHECK (hard_min_headroom_bytes >= 0),
    soft_min_headroom_bytes BIGINT NOT NULL CHECK (soft_min_headroom_bytes > hard_min_headroom_bytes),
    recovery_min_headroom_bytes BIGINT NOT NULL CHECK (recovery_min_headroom_bytes > soft_min_headroom_bytes),
    event_reservation_bytes BIGINT NOT NULL CHECK (event_reservation_bytes > 0),
    soft_max_new_rows_per_second BIGINT NOT NULL CHECK (
        soft_max_new_rows_per_second BETWEEN 100000 AND 10000000
    ),
    sample_interval_seconds BIGINT NOT NULL CHECK (sample_interval_seconds BETWEEN 1 AND 60),
    stale_after_seconds BIGINT NOT NULL CHECK (
        stale_after_seconds BETWEEN sample_interval_seconds * 2 AND 600
    ),
    capacity_evidence TEXT NOT NULL CHECK (length(btrim(capacity_evidence)) BETWEEN 8 AND 1024),
    installed_at BIGINT NOT NULL CHECK (installed_at > 0),
    CONSTRAINT chk_analytics_admission_capacity_order CHECK (
        database_capacity_bytes > recovery_min_headroom_bytes
    )
);

CREATE FUNCTION v2_guard_analytics_admission_policy()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'analytics admission policy is lifecycle-immutable';
END;
$$;

CREATE TRIGGER trg_analytics_admission_policy_immutable
BEFORE UPDATE OR DELETE ON v2_analytics_admission_policy
FOR EACH ROW EXECUTE FUNCTION v2_guard_analytics_admission_policy();

-- This singleton is the cheap hot-path admission state. The worker refreshes
-- exact PostgreSQL measurements; every producer serializes reservations here
-- in the same transaction as its business mutation and outbox insert.
CREATE TABLE v2_analytics_admission_state (
    singleton SMALLINT PRIMARY KEY CHECK (singleton = 1),
    installation_id UUID NOT NULL UNIQUE
        REFERENCES v2_analytics_admission_policy(installation_id) ON DELETE RESTRICT,
    pressure_state TEXT NOT NULL CHECK (pressure_state IN ('normal', 'soft_pressure', 'hard_stop')),
    generation BIGINT NOT NULL CHECK (generation >= 0),
    sampled_at BIGINT NOT NULL CHECK (sampled_at > 0),
    state_changed_at BIGINT NOT NULL CHECK (state_changed_at > 0),
    pending_rows BIGINT NOT NULL CHECK (pending_rows >= 0),
    oldest_pending_created_at BIGINT,
    relation_heap_bytes BIGINT NOT NULL CHECK (relation_heap_bytes >= 0),
    relation_index_bytes BIGINT NOT NULL CHECK (relation_index_bytes >= 0),
    relation_toast_bytes BIGINT NOT NULL CHECK (relation_toast_bytes >= 0),
    relation_total_bytes BIGINT NOT NULL CHECK (relation_total_bytes >= 0),
    database_bytes BIGINT NOT NULL CHECK (database_bytes >= 0),
    capacity_headroom_bytes BIGINT NOT NULL,
    accounted_pending_rows BIGINT NOT NULL CHECK (accounted_pending_rows >= 0),
    accounted_relation_bytes BIGINT NOT NULL CHECK (accounted_relation_bytes >= 0),
    soft_window_started_at BIGINT NOT NULL CHECK (soft_window_started_at > 0),
    soft_window_admitted_rows BIGINT NOT NULL CHECK (soft_window_admitted_rows >= 0),
    last_transition_reason TEXT NOT NULL CHECK (length(last_transition_reason) BETWEEN 1 AND 128),
    CONSTRAINT chk_analytics_admission_relation_parts CHECK (
        relation_total_bytes >= relation_heap_bytes + relation_index_bytes
        AND relation_toast_bytes = relation_total_bytes - relation_heap_bytes - relation_index_bytes
    )
);

CREATE FUNCTION v2_guard_analytics_admission_state()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'analytics admission state cannot be deleted';
    END IF;
    IF NEW.singleton IS DISTINCT FROM OLD.singleton
       OR NEW.installation_id IS DISTINCT FROM OLD.installation_id THEN
        RAISE EXCEPTION 'analytics admission state identity cannot change';
    END IF;
    IF NEW.generation <> OLD.generation + 1 THEN
        RAISE EXCEPTION 'analytics admission generation must advance exactly once';
    END IF;
    IF NEW.sampled_at < OLD.sampled_at OR NEW.state_changed_at < OLD.state_changed_at THEN
        RAISE EXCEPTION 'analytics admission timestamps cannot regress';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_analytics_admission_state_guard
BEFORE UPDATE OR DELETE ON v2_analytics_admission_state
FOR EACH ROW EXECUTE FUNCTION v2_guard_analytics_admission_state();
