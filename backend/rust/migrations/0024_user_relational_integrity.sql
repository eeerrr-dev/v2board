ALTER TABLE `v2_user`
    ADD KEY `idx_user_plan_id` (`plan_id`),
    ADD KEY `idx_user_group_id` (`group_id`),
    ADD KEY `idx_user_invite_user_id` (`invite_user_id`),
    ADD KEY `idx_user_renewal_candidate` (`auto_renewal`, `expired_at`, `id`),
    ADD CONSTRAINT `fk_user_plan`
        FOREIGN KEY (`plan_id`) REFERENCES `v2_plan` (`id`) ON DELETE RESTRICT,
    ADD CONSTRAINT `fk_user_group`
        FOREIGN KEY (`group_id`) REFERENCES `v2_server_group` (`id`) ON DELETE RESTRICT;
