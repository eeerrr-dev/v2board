-- Concurrency and replay guards for native order/worker processing.

-- MySQL unique indexes permit multiple NULLs. Project only unfinished orders to
-- the generated value so each user may have at most one status 0/1 order while
-- completed/cancelled/surplus orders remain unrestricted.
ALTER TABLE `v2_order`
    ADD COLUMN `unfinished_user_id` int(11)
        GENERATED ALWAYS AS (
            CASE WHEN `status` IN (0, 1) THEN `user_id` ELSE NULL END
        ) STORED,
    ADD UNIQUE KEY `uniq_unfinished_order_per_user` (`unfinished_user_id`),
    ADD KEY `idx_commission_claim` (`commission_status`, `id`);

-- Idempotency ledger for a Redis traffic hash after it has been atomically
-- renamed from the live accumulator to the processing slot. The ledger row and
-- user counter updates commit together, so an ambiguous Redis acknowledgement
-- can never apply the same batch twice.
CREATE TABLE `v2_traffic_batch` (
    `batch_id` char(36) NOT NULL,
    `applied_at` bigint NOT NULL,
    PRIMARY KEY (`batch_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- Durable outbox for node clients that supply an explicit report idempotency
-- key. Daily statistics are written in the same transaction as this header;
-- workers atomically claim unapplied headers and apply the charged user deltas.
CREATE TABLE `v2_server_traffic_report` (
    `report_key` char(40) NOT NULL,
    `payload_hash` char(40) NOT NULL,
    `applied_at` bigint DEFAULT NULL,
    `created_at` bigint NOT NULL,
    `updated_at` bigint NOT NULL,
    PRIMARY KEY (`report_key`),
    KEY `idx_traffic_report_claim` (`applied_at`, `created_at`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE `v2_server_traffic_report_item` (
    `report_key` char(40) NOT NULL,
    `user_id` int(11) NOT NULL,
    `u` bigint NOT NULL,
    `d` bigint NOT NULL,
    PRIMARY KEY (`report_key`, `user_id`),
    CONSTRAINT `fk_traffic_report_item_report`
        FOREIGN KEY (`report_key`) REFERENCES `v2_server_traffic_report` (`report_key`)
        ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
