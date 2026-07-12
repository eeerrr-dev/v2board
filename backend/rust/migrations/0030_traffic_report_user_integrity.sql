ALTER TABLE `v2_server_traffic_report_item`
    ADD KEY `idx_traffic_report_item_user` (`user_id`),
    ADD CONSTRAINT `fk_traffic_report_item_user`
        FOREIGN KEY (`user_id`) REFERENCES `v2_user` (`id`) ON DELETE CASCADE;
