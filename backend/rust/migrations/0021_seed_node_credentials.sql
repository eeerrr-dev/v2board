-- One atomic DML statement seeds every existing node after the credential
-- table exists. New nodes create their credential row in the admin write path.
INSERT INTO `v2_server_credential` (`node_type`, `node_id`, `credential_epoch`, `updated_at`)
SELECT 'shadowsocks', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_shadowsocks`
UNION ALL SELECT 'vmess', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_vmess`
UNION ALL SELECT 'trojan', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_trojan`
UNION ALL SELECT 'vless', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_vless`
UNION ALL SELECT 'tuic', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_tuic`
UNION ALL SELECT 'hysteria', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_hysteria`
UNION ALL SELECT 'anytls', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_anytls`
UNION ALL SELECT 'v2node', `id`, 0, UNIX_TIMESTAMP() FROM `v2_server_v2node`;
