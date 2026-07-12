-- Fence asynchronous traffic reports from subscription/reset boundaries.
--
-- Every traffic-resetting mutation increments the user's epoch while holding
-- the user row lock.  Node reports capture that epoch when they are accepted;
-- the worker applies an item only when it still belongs to the user's current
-- quota period.  Existing queued reports and users both start at epoch zero,
-- so an online migration preserves all pre-migration work.

ALTER TABLE `v2_user`
    ADD COLUMN `traffic_epoch` bigint NOT NULL DEFAULT 0 AFTER `session_epoch`,
    ADD COLUMN `scheduled_traffic_reset_key` char(10) DEFAULT NULL AFTER `traffic_epoch`;
