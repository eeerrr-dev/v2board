-- Keep each irreversible MySQL DDL in its own SQLx migration. If this ALTER
-- fails it is atomic; if a later version fails, SQLx can resume from there.
ALTER TABLE `v2_server_traffic_report_item`
    ADD COLUMN `traffic_epoch` bigint NOT NULL DEFAULT 0 AFTER `user_id`;
