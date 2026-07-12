-- Selective indexes for permanent worker/admin access paths. Deposit orders
-- intentionally use plan_id = 0, projected to NULL for the conditional FK.
ALTER TABLE `v2_order`
    ADD COLUMN `referenced_plan_id` int(11)
        GENERATED ALWAYS AS (CASE WHEN `plan_id` = 0 THEN NULL ELSE `plan_id` END) STORED,
    ADD KEY `idx_order_status_id` (`status`, `id`),
    ADD KEY `idx_order_referenced_plan` (`referenced_plan_id`),
    ADD KEY `idx_order_coupon_user_status` (`coupon_id`, `user_id`, `status`),
    ADD CONSTRAINT `fk_order_user`
        FOREIGN KEY (`user_id`) REFERENCES `v2_user` (`id`) ON DELETE RESTRICT,
    ADD CONSTRAINT `fk_order_plan_non_deposit`
        FOREIGN KEY (`referenced_plan_id`) REFERENCES `v2_plan` (`id`) ON DELETE RESTRICT;
