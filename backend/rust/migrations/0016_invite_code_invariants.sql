ALTER TABLE `v2_invite_code`
    ADD UNIQUE KEY `uniq_invite_code` (`code`),
    ADD KEY `idx_invite_user_status` (`user_id`, `status`);
