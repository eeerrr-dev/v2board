-- Relational integrity and capacity indexes for the native backend.
--
-- Legacy schemas intentionally omitted most foreign keys.  Refuse to install
-- the first persistent DDL statement when an existing row would make one of
-- the new constraints lossy or ambiguous.  The seeded UNIQUE guard turns the
-- first violation into a deterministic migration failure; operators must
-- repair or explicitly remove the orphan before retrying.

CREATE TEMPORARY TABLE `_v2_0012_relational_integrity_preflight` (
    `guard` tinyint NOT NULL,
    `violation` varchar(96) NOT NULL,
    UNIQUE KEY `relational_integrity_preflight_failed` (`guard`)
) ENGINE=InnoDB;

INSERT INTO `_v2_0012_relational_integrity_preflight` (`guard`, `violation`)
VALUES (1, 'no legacy violations');

INSERT INTO `_v2_0012_relational_integrity_preflight` (`guard`, `violation`)
SELECT 1, violations.violation
FROM (
    SELECT 'plan references a missing server group' AS violation
    WHERE EXISTS (
        SELECT 1
        FROM `v2_plan` AS child
        LEFT JOIN `v2_server_group` AS parent ON parent.`id` = child.`group_id`
        WHERE parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'user references a missing plan'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_user` AS child
        LEFT JOIN `v2_plan` AS parent ON parent.`id` = child.`plan_id`
        WHERE child.`plan_id` IS NOT NULL AND parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'user references a missing server group'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_user` AS child
        LEFT JOIN `v2_server_group` AS parent ON parent.`id` = child.`group_id`
        WHERE child.`group_id` IS NOT NULL AND parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'order references a missing user'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_order` AS child
        LEFT JOIN `v2_user` AS parent ON parent.`id` = child.`user_id`
        WHERE parent.`id` IS NULL
    )

    UNION ALL

    -- Deposit orders deliberately use plan_id = 0.  Every other value is a
    -- real relationship and is projected to a nullable generated FK below.
    SELECT 'non-deposit order references a missing plan'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_order` AS child
        LEFT JOIN `v2_plan` AS parent ON parent.`id` = child.`plan_id`
        WHERE child.`plan_id` <> 0 AND parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'gift card references a missing plan'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_giftcard` AS child
        LEFT JOIN `v2_plan` AS parent ON parent.`id` = child.`plan_id`
        WHERE child.`plan_id` IS NOT NULL AND parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'invite code references a missing user'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_invite_code` AS child
        LEFT JOIN `v2_user` AS parent ON parent.`id` = child.`user_id`
        WHERE parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'ticket references a missing user'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_ticket` AS child
        LEFT JOIN `v2_user` AS parent ON parent.`id` = child.`user_id`
        WHERE parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'ticket message references a missing ticket'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_ticket_message` AS child
        LEFT JOIN `v2_ticket` AS parent ON parent.`id` = child.`ticket_id`
        WHERE parent.`id` IS NULL
    )

    UNION ALL

    -- ticket_message.user_id may be zero for an administrator reply, so it is
    -- intentionally not a user FK.
    SELECT 'gift card redemption references a missing user'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_giftcard_redemption` AS child
        LEFT JOIN `v2_user` AS parent ON parent.`id` = child.`user_id`
        WHERE parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'queued traffic item references a missing user'
    WHERE EXISTS (
        SELECT 1
        FROM `v2_server_traffic_report_item` AS child
        LEFT JOIN `v2_user` AS parent ON parent.`id` = child.`user_id`
        WHERE parent.`id` IS NULL
    )

    UNION ALL

    SELECT 'server node has malformed group JSON'
    WHERE EXISTS (
        SELECT 1 FROM `v2_server_shadowsocks`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
        UNION ALL SELECT 1 FROM `v2_server_vmess`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
        UNION ALL SELECT 1 FROM `v2_server_trojan`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
        UNION ALL SELECT 1 FROM `v2_server_tuic`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
        UNION ALL SELECT 1 FROM `v2_server_hysteria`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
        UNION ALL SELECT 1 FROM `v2_server_vless`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
        UNION ALL SELECT 1 FROM `v2_server_anytls`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
        UNION ALL SELECT 1 FROM `v2_server_v2node`
        WHERE IF(JSON_VALID(`group_id`), JSON_TYPE(`group_id`) <> 'ARRAY' OR JSON_LENGTH(`group_id`) = 0, TRUE)
    )

    UNION ALL

    SELECT 'server node references an invalid or missing group'
    WHERE EXISTS (
        SELECT 1
        FROM (
            SELECT membership.`member`
            FROM `v2_server_shadowsocks` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
            UNION ALL
            SELECT membership.`member`
            FROM `v2_server_vmess` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
            UNION ALL
            SELECT membership.`member`
            FROM `v2_server_trojan` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
            UNION ALL
            SELECT membership.`member`
            FROM `v2_server_tuic` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
            UNION ALL
            SELECT membership.`member`
            FROM `v2_server_hysteria` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
            UNION ALL
            SELECT membership.`member`
            FROM `v2_server_vless` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
            UNION ALL
            SELECT membership.`member`
            FROM `v2_server_anytls` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
            UNION ALL
            SELECT membership.`member`
            FROM `v2_server_v2node` AS node
            JOIN JSON_TABLE(
                IF(JSON_VALID(node.`group_id`), node.`group_id`, JSON_ARRAY()),
                '$[*]' COLUMNS (`member` JSON PATH '$' NULL ON EMPTY NULL ON ERROR)
            ) AS membership ON TRUE
        ) AS membership
        LEFT JOIN `v2_server_group` AS parent
          ON parent.`id` = CAST(JSON_UNQUOTE(membership.`member`) AS UNSIGNED)
        WHERE membership.`member` IS NULL
           OR JSON_TYPE(membership.`member`) NOT IN ('INTEGER', 'STRING')
           OR JSON_UNQUOTE(membership.`member`) NOT REGEXP '^[1-9][0-9]*$'
           OR parent.`id` IS NULL
    )
) AS violations
LIMIT 1;

DROP TEMPORARY TABLE `_v2_0012_relational_integrity_preflight`;
