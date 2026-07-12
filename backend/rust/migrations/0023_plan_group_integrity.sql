ALTER TABLE `v2_plan`
    ADD KEY `idx_plan_group_id` (`group_id`),
    ADD CONSTRAINT `fk_plan_group`
        FOREIGN KEY (`group_id`) REFERENCES `v2_server_group` (`id`) ON DELETE RESTRICT;
