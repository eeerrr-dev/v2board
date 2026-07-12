ALTER TABLE `v2_server_v2node`
    ADD CONSTRAINT `chk_v2node_group_json`
        CHECK (JSON_VALID(`group_id`) AND JSON_TYPE(`group_id`) = 'ARRAY' AND JSON_LENGTH(`group_id`) > 0);
