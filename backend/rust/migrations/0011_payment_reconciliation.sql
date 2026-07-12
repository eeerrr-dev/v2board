-- Durable reconciliation for every authenticated payment that cannot safely
-- make the pending -> paid transition: late/cancelled orders, second provider
-- transactions, binding mismatches, and settled-amount mismatches.
CREATE TABLE `v2_payment_reconciliation` (
    `id` bigint unsigned NOT NULL AUTO_INCREMENT,
    `payment_id` int(11) NOT NULL,
    `provider` varchar(32) NOT NULL,
    `trade_no` varchar(255) NOT NULL,
    `trade_no_hash` binary(32) NOT NULL,
    `callback_no` varchar(255) NOT NULL,
    `callback_no_hash` binary(32) NOT NULL,
    `reason` varchar(64) NOT NULL,
    `order_status` tinyint(1) NOT NULL,
    `expected_amount` bigint NOT NULL,
    `settled_amount` bigint DEFAULT NULL,
    `occurrence_count` int unsigned NOT NULL DEFAULT 1,
    `first_seen_at` bigint NOT NULL,
    `last_seen_at` bigint NOT NULL,
    `resolved_at` bigint DEFAULT NULL,
    `resolution` varchar(255) DEFAULT NULL,
    PRIMARY KEY (`id`),
    UNIQUE KEY `uniq_payment_reconciliation_callback` (`payment_id`, `callback_no_hash`),
    KEY `idx_payment_reconciliation_open` (`resolved_at`, `first_seen_at`),
    KEY `idx_payment_reconciliation_trade` (`trade_no_hash`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
