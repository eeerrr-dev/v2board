ALTER TABLE `v2_giftcard`
    ADD KEY `idx_giftcard_plan_id` (`plan_id`),
    ADD CONSTRAINT `fk_giftcard_plan`
        FOREIGN KEY (`plan_id`) REFERENCES `v2_plan` (`id`) ON DELETE RESTRICT;
