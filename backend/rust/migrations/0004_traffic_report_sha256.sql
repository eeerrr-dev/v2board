-- Traffic report idempotency initially used SHA-1-sized storage. The runtime now
-- hashes both the idempotency key and payload with SHA-256; widen the existing
-- primary/foreign-key pair without rewriting or discarding queued reports.
ALTER TABLE `v2_server_traffic_report_item`
    DROP FOREIGN KEY `fk_traffic_report_item_report`;

ALTER TABLE `v2_server_traffic_report`
    MODIFY COLUMN `report_key` char(64) NOT NULL,
    MODIFY COLUMN `payload_hash` char(64) NOT NULL;

ALTER TABLE `v2_server_traffic_report_item`
    MODIFY COLUMN `report_key` char(64) NOT NULL,
    ADD CONSTRAINT `fk_traffic_report_item_report`
        FOREIGN KEY (`report_key`) REFERENCES `v2_server_traffic_report` (`report_key`)
        ON DELETE CASCADE;
