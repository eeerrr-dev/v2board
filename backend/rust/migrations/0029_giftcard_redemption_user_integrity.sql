ALTER TABLE `v2_giftcard_redemption`
    MODIFY COLUMN `user_id` int(11) NOT NULL,
    ADD CONSTRAINT `fk_giftcard_redemption_user`
        FOREIGN KEY (`user_id`) REFERENCES `v2_user` (`id`) ON DELETE CASCADE;
