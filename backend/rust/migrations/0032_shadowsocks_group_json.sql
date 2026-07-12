-- JSON arrays cannot use a scalar FK without changing the node payload. The
-- admin write path locks and verifies members; MySQL enforces storage shape.
ALTER TABLE `v2_server_shadowsocks`
    ADD CONSTRAINT `chk_shadowsocks_group_json`
        CHECK (JSON_VALID(`group_id`) AND JSON_TYPE(`group_id`) = 'ARRAY' AND JSON_LENGTH(`group_id`) > 0);
