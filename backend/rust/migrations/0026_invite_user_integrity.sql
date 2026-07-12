ALTER TABLE `v2_invite_code`
    ADD CONSTRAINT `fk_invite_code_user`
        FOREIGN KEY (`user_id`) REFERENCES `v2_user` (`id`) ON DELETE CASCADE;
