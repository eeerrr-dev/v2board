-- V2Board native PostgreSQL 18 append-only lineage v2.
-- Extends the immutable 0001 baseline with gift-card timestamp provenance
-- and analytics admission control.

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
-- row but cannot alter it; trusted installation tooling supplies the policy
-- for the reserved installation identity.
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
    RAISE EXCEPTION 'analytics admission policy is immutable';
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
