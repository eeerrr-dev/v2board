ALTER TABLE `v2_ticket`
    ADD KEY `idx_ticket_auto_close` (`status`, `reply_status`, `updated_at`, `id`),
    ADD CONSTRAINT `fk_ticket_user`
        FOREIGN KEY (`user_id`) REFERENCES `v2_user` (`id`) ON DELETE RESTRICT;
