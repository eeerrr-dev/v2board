-- Admin/staff TOTP two-factor state: at most one row per privileged account.
-- The RFC 6238 shared secret is sealed with AES-256-GCM under a key derived
-- from the installation app_key and is never stored or logged in plaintext.
-- `enabled_at IS NULL` marks a pending (unconfirmed) enrollment; `last_step`
-- is the highest accepted TOTP time-step, making every accepted code
-- one-time-use.
CREATE TABLE admin_mfa (
    user_id BIGINT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    secret_nonce BYTEA NOT NULL CHECK (octet_length(secret_nonce) = 12),
    secret_ciphertext BYTEA NOT NULL,
    secret_tag BYTEA NOT NULL CHECK (octet_length(secret_tag) = 16),
    enabled_at BIGINT,
    last_step BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    CHECK (last_step >= 0 AND created_at >= 0 AND updated_at >= 0)
);
