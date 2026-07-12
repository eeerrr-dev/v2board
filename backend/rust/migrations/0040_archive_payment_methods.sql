-- A payment row is an immutable verification version. Admin "drop" is a soft
-- archive so its routing UUID and verification material remain available for
-- callbacks that arrive after cancellation, user deletion, or long delay.
ALTER TABLE `v2_payment`
    ADD COLUMN `archived_at` bigint DEFAULT NULL AFTER `enable`,
    ADD KEY `idx_payment_archived` (`archived_at`, `id`);
