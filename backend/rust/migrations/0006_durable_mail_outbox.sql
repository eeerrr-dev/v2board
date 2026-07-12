-- Durable, idempotent application mail outbox. The batch row makes one mail
-- mutation an atomic unit: admin/staff bulk sends, transactional notifications,
-- and user/kind/business-day reminder occurrences are committed with their
-- application decision. Stable reminder batches replace pre-send Redis cooldowns,
-- so scheduler reruns and restarts do not duplicate delivery while SMTP failures
-- remain retryable. SMTP delivery remains at-least-once because a worker
-- can lose its lease after the relay accepts a message but before its durable
-- acknowledgement commits; the stable message_id lets relays/clients deduplicate
-- that narrow uncertainty window. The message envelope lives once on the batch;
-- successful items are deleted, terminal failures retain their diagnostic state,
-- and the worker clears the batch envelope after no pending items remain.

CREATE TABLE `v2_mail_outbox_batch` (
    `batch_key` char(64) NOT NULL,
    `payload_hash` char(64) NOT NULL,
    `actor` varchar(512) NOT NULL,
    `sender` varchar(512) DEFAULT NULL,
    `template_name` varchar(255) DEFAULT NULL,
    `subject` mediumtext,
    `body` mediumtext,
    `created_at` bigint NOT NULL,
    `updated_at` bigint NOT NULL,
    PRIMARY KEY (`batch_key`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE `v2_mail_outbox` (
    `id` bigint unsigned NOT NULL AUTO_INCREMENT,
    `batch_key` char(64) NOT NULL,
    `recipient` varchar(512) NOT NULL,
    `message_id` varchar(255) NOT NULL,
    `attempt_count` int unsigned NOT NULL DEFAULT 0,
    `available_at` bigint NOT NULL,
    `lease_token` char(36) DEFAULT NULL,
    `lease_expires_at` bigint DEFAULT NULL,
    `failed_at` bigint DEFAULT NULL,
    `last_error` text DEFAULT NULL,
    `created_at` bigint NOT NULL,
    `updated_at` bigint NOT NULL,
    PRIMARY KEY (`id`),
    UNIQUE KEY `uniq_mail_outbox_batch_recipient` (`batch_key`, `recipient`),
    UNIQUE KEY `uniq_mail_outbox_message_id` (`message_id`),
    KEY `idx_mail_outbox_claim` (`failed_at`, `available_at`, `id`),
    KEY `idx_mail_outbox_lease` (`lease_expires_at`),
    CONSTRAINT `fk_mail_outbox_batch`
        FOREIGN KEY (`batch_key`) REFERENCES `v2_mail_outbox_batch` (`batch_key`)
        ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
