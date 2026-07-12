-- Enforce business identifiers and the one-open-ticket state machine in MySQL.
--
-- The temporary-table insert deliberately collides with its seeded UNIQUE key
-- when any legacy violation exists. This keeps the migration fail-closed before
-- the first persistent DDL statement. Operators can use the GROUP BY predicates
-- below to locate and resolve every conflicting row before retrying the upgrade.

CREATE TEMPORARY TABLE `_v2_0010_business_invariant_preflight` (
    `guard` tinyint NOT NULL,
    `violation` varchar(64) NOT NULL,
    UNIQUE KEY `business_invariant_preflight_failed` (`guard`)
) ENGINE=InnoDB;

INSERT INTO `_v2_0010_business_invariant_preflight` (`guard`, `violation`)
VALUES (1, 'no legacy violations');

INSERT INTO `_v2_0010_business_invariant_preflight` (`guard`, `violation`)
SELECT 1, violations.violation
FROM (
    SELECT 'duplicate coupon code' AS violation
    FROM `v2_coupon`
    GROUP BY `code`
    HAVING COUNT(*) > 1

    UNION ALL

    SELECT 'duplicate giftcard code' AS violation
    FROM `v2_giftcard`
    GROUP BY `code`
    HAVING COUNT(*) > 1

    UNION ALL

    SELECT 'duplicate invite code' AS violation
    FROM `v2_invite_code`
    GROUP BY `code`
    HAVING COUNT(*) > 1

    UNION ALL

    SELECT 'duplicate payment webhook key' AS violation
    FROM `v2_payment`
    GROUP BY `payment`, `uuid`
    HAVING COUNT(*) > 1

    UNION ALL

    SELECT 'user has multiple open tickets' AS violation
    FROM `v2_ticket`
    WHERE `status` = 0
    GROUP BY `user_id`
    HAVING COUNT(*) > 1
) AS violations
LIMIT 1;

DROP TEMPORARY TABLE `_v2_0010_business_invariant_preflight`;
