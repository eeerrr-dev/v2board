-- Versioned, encrypted operator configuration authority.
--
-- Runtime role files remain the bootstrap source for datastore credentials and
-- the application key.  Mutable operator settings are committed here once and
-- consumed by both the API and worker.  Revision rows are immutable; the
-- singleton state pointer is the only activation boundary.

CREATE TABLE v2_operator_config_revision (
    revision BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    revision_id UUID NOT NULL UNIQUE,
    format_version SMALLINT NOT NULL,
    installation_id UUID NOT NULL,
    public_config JSONB NOT NULL,
    secret_nonce BYTEA NOT NULL,
    secret_ciphertext BYTEA NOT NULL,
    secret_tag BYTEA NOT NULL,
    config_hmac_sha256 VARCHAR(64) NOT NULL,
    created_by VARCHAR(64) NOT NULL,
    created_at BIGINT NOT NULL,
    CONSTRAINT fk_operator_config_revision_installation
        FOREIGN KEY (installation_id)
        REFERENCES v2_system_installation(installation_id) ON DELETE RESTRICT,
    CONSTRAINT uniq_operator_config_revision_installation
        UNIQUE (revision, installation_id),
    CONSTRAINT chk_operator_config_public_object
        CHECK (jsonb_typeof(public_config) = 'object'),
    CONSTRAINT chk_operator_config_format_version
        CHECK (format_version = 1),
    CONSTRAINT chk_operator_config_public_has_no_secrets
        CHECK (NOT (public_config ?| ARRAY[
            'server_token',
            'email_password',
            'telegram_bot_token',
            'recaptcha_key'
        ])),
    CONSTRAINT chk_operator_config_nonce_length
        CHECK (octet_length(secret_nonce) = 12),
    CONSTRAINT chk_operator_config_ciphertext_present
        CHECK (octet_length(secret_ciphertext) > 0),
    CONSTRAINT chk_operator_config_tag_length
        CHECK (octet_length(secret_tag) = 16),
    CONSTRAINT chk_operator_config_hmac
        CHECK (config_hmac_sha256 ~ '^[0-9a-f]{64}$'),
    CONSTRAINT chk_operator_config_actor_present
        CHECK (length(btrim(created_by)) > 0)
);

CREATE FUNCTION v2_guard_operator_config_revision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'operator configuration revisions are immutable';
END;
$$;

CREATE TRIGGER trg_operator_config_revision_immutable
BEFORE UPDATE OR DELETE ON v2_operator_config_revision
FOR EACH ROW EXECUTE FUNCTION v2_guard_operator_config_revision();

CREATE TABLE v2_operator_config_state (
    singleton SMALLINT PRIMARY KEY CHECK (singleton = 1),
    installation_id UUID NOT NULL,
    active_revision BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CONSTRAINT fk_operator_config_state_revision
        FOREIGN KEY (active_revision, installation_id)
        REFERENCES v2_operator_config_revision(revision, installation_id)
        ON DELETE RESTRICT
);

CREATE FUNCTION v2_guard_operator_config_state()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'operator configuration state cannot be deleted';
    END IF;
    IF NEW.singleton IS DISTINCT FROM OLD.singleton
       OR NEW.installation_id IS DISTINCT FROM OLD.installation_id THEN
        RAISE EXCEPTION 'operator configuration state identity cannot change';
    END IF;
    IF NEW.active_revision < OLD.active_revision THEN
        RAISE EXCEPTION 'operator configuration revision cannot move backwards';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_operator_config_state_guard
BEFORE UPDATE OR DELETE ON v2_operator_config_state
FOR EACH ROW EXECUTE FUNCTION v2_guard_operator_config_state();

CREATE FUNCTION v2_guard_operator_config_ack()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'operator configuration acknowledgement cannot be deleted';
    END IF;
    IF NEW.singleton IS DISTINCT FROM OLD.singleton
       OR NEW.installation_id IS DISTINCT FROM OLD.installation_id THEN
        RAISE EXCEPTION 'operator configuration acknowledgement identity cannot change';
    END IF;
    IF NEW.observed_revision < OLD.observed_revision
       OR COALESCE(NEW.applied_revision, 0) < COALESCE(OLD.applied_revision, 0) THEN
        RAISE EXCEPTION 'operator configuration acknowledgement cannot move backwards';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TABLE v2_operator_config_api_ack (
    singleton SMALLINT PRIMARY KEY CHECK (singleton = 1),
    installation_id UUID NOT NULL,
    observed_revision BIGINT NOT NULL,
    applied_revision BIGINT,
    status VARCHAR(16) NOT NULL CHECK (status IN ('applied', 'rejected')),
    error_code VARCHAR(64),
    observed_at BIGINT NOT NULL,
    CONSTRAINT fk_operator_config_api_ack_observed
        FOREIGN KEY (observed_revision, installation_id)
        REFERENCES v2_operator_config_revision(revision, installation_id)
        ON DELETE RESTRICT,
    CONSTRAINT fk_operator_config_api_ack_applied
        FOREIGN KEY (applied_revision, installation_id)
        REFERENCES v2_operator_config_revision(revision, installation_id)
        ON DELETE RESTRICT,
    CONSTRAINT chk_operator_config_api_ack_status CHECK (
        (status = 'applied' AND applied_revision = observed_revision AND error_code IS NULL)
        OR
        (status = 'rejected'
         AND (applied_revision IS NULL OR applied_revision < observed_revision)
         AND error_code ~ '^[a-z0-9_]{1,64}$')
    )
);

CREATE TRIGGER trg_operator_config_api_ack_guard
BEFORE UPDATE OR DELETE ON v2_operator_config_api_ack
FOR EACH ROW EXECUTE FUNCTION v2_guard_operator_config_ack();

CREATE TABLE v2_operator_config_worker_ack (
    singleton SMALLINT PRIMARY KEY CHECK (singleton = 1),
    installation_id UUID NOT NULL,
    observed_revision BIGINT NOT NULL,
    applied_revision BIGINT,
    status VARCHAR(16) NOT NULL CHECK (status IN ('applied', 'rejected')),
    error_code VARCHAR(64),
    observed_at BIGINT NOT NULL,
    CONSTRAINT fk_operator_config_worker_ack_observed
        FOREIGN KEY (observed_revision, installation_id)
        REFERENCES v2_operator_config_revision(revision, installation_id)
        ON DELETE RESTRICT,
    CONSTRAINT fk_operator_config_worker_ack_applied
        FOREIGN KEY (applied_revision, installation_id)
        REFERENCES v2_operator_config_revision(revision, installation_id)
        ON DELETE RESTRICT,
    CONSTRAINT chk_operator_config_worker_ack_status CHECK (
        (status = 'applied' AND applied_revision = observed_revision AND error_code IS NULL)
        OR
        (status = 'rejected'
         AND (applied_revision IS NULL OR applied_revision < observed_revision)
         AND error_code ~ '^[a-z0-9_]{1,64}$')
    )
);

CREATE TRIGGER trg_operator_config_worker_ack_guard
BEFORE UPDATE OR DELETE ON v2_operator_config_worker_ack
FOR EACH ROW EXECUTE FUNCTION v2_guard_operator_config_ack();
