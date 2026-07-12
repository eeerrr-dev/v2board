-- Provider transaction identifiers are untrusted and can exceed the legacy
-- varchar(255) display column. Keep a bounded utf8mb3-safe label there while
-- retaining the complete identity in a fixed-width digest for exact replay
-- detection and Payment Element binding. Existing rows stay NULL and are
-- lazily backfilled on replay, avoiding a blocking full-table migration update.
ALTER TABLE `v2_order`
    ADD COLUMN `callback_no_hash` binary(32) DEFAULT NULL,
    ALGORITHM=INSTANT;
