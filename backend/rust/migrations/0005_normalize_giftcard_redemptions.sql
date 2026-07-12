-- Normalize the legacy JSON redemption ledger. The application migration
-- preflight validates every non-NULL legacy value as a JSON array of signed
-- integer user ids before this migration runs. JSON_TABLE also rejects invalid
-- JSON and values that cannot be represented by the normalized BIGINT column.

CREATE TABLE `v2_giftcard_redemption` (
    `giftcard_id` int(11) NOT NULL,
    `user_id` bigint(20) NOT NULL,
    `created_at` bigint NOT NULL,
    PRIMARY KEY (`giftcard_id`, `user_id`),
    KEY `idx_giftcard_redemption_user` (`user_id`),
    CONSTRAINT `fk_giftcard_redemption_giftcard`
        FOREIGN KEY (`giftcard_id`) REFERENCES `v2_giftcard` (`id`)
        ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT INTO `v2_giftcard_redemption` (`giftcard_id`, `user_id`, `created_at`)
SELECT DISTINCT
    giftcard.id,
    legacy.user_id,
    COALESCE(NULLIF(giftcard.updated_at, 0), giftcard.created_at)
FROM `v2_giftcard` AS giftcard
CROSS JOIN JSON_TABLE(
    COALESCE(giftcard.used_user_ids, '[]'),
    '$[*]' COLUMNS (
        user_id BIGINT PATH '$' ERROR ON EMPTY ERROR ON ERROR
    )
) AS legacy;

ALTER TABLE `v2_giftcard` DROP COLUMN `used_user_ids`;
