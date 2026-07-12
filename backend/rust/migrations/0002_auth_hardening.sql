ALTER TABLE `v2_user`
    MODIFY COLUMN `password` varchar(255) NOT NULL,
    ADD COLUMN `session_epoch` bigint NOT NULL DEFAULT 0 AFTER `password_salt`;
