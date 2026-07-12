-- Node tokens are derived from the deployment's server-token master key plus
-- this per-node epoch. A disclosed node credential cannot authorize another
-- node, and incrementing one row revokes only that node.
CREATE TABLE `v2_server_credential` (
    `node_type` varchar(32) NOT NULL,
    `node_id` int(11) NOT NULL,
    `credential_epoch` bigint NOT NULL DEFAULT 0,
    `updated_at` bigint NOT NULL,
    PRIMARY KEY (`node_type`, `node_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
