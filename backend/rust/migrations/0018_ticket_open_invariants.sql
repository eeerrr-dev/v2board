ALTER TABLE `v2_ticket`
    ADD COLUMN `open_user_id` bigint
        GENERATED ALWAYS AS (CASE WHEN `status` = 0 THEN `user_id` ELSE NULL END) STORED,
    ADD UNIQUE KEY `uniq_ticket_open_user` (`open_user_id`),
    ADD KEY `idx_ticket_user_status` (`user_id`, `status`);
