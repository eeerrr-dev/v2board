SET NAMES utf8mb4;

-- One relationship-valid row in every converter-owned legacy table.  A
-- second user exercises the deferred self-reference and gift-card expansion.
INSERT INTO v2_server_group (id, name, created_at, updated_at)
VALUES (1, '默认节点组', 1700000000, 1700000001);

INSERT INTO v2_plan (
  id, group_id, transfer_enable, device_limit, name, speed_limit, `show`, sort,
  renew, content, month_price, quarter_price, half_year_price, year_price,
  two_year_price, three_year_price, onetime_price, reset_price,
  reset_traffic_method, capacity_limit, created_at, updated_at
) VALUES (
  1, 1, 1073741824, 5, '完整迁移套餐', 1000, 1, 10,
  1, '保留 Unicode 套餐内容', 999, 2699, 4999, 8999,
  16999, 23999, 29999, 199, 1, 1000, 1700000000, 1700000001
);

INSERT INTO v2_payment (
  id, uuid, payment, name, icon, config, notify_domain, handling_fee_fixed,
  handling_fee_percent, enable, sort, created_at, updated_at
) VALUES (
  1, '00000000000000000000000000000001', 'manual', '手动支付', NULL,
  '{"merchant":"legacy","enabled":true}', 'pay.example.test', 25, 1.25,
  1, 1, 1700000000, 1700000001
);

INSERT INTO v2_coupon (
  id, code, name, type, value, `show`, limit_use, limit_use_with_user,
  limit_plan_ids, limit_period, started_at, ended_at, created_at, updated_at
) VALUES (
  1, 'WELCOME-ONE', '迁移优惠券', 1, 500, 1, 100, 1,
  '["1"]', '["month_price","year_price"]', 1690000000, 1890000000,
  1700000000, 1700000001
);

INSERT INTO v2_user (
  id, invite_user_id, telegram_id, email, password, password_algo,
  password_salt, balance, discount, commission_type, commission_rate,
  commission_balance, t, u, d, transfer_enable, device_limit, banned,
  is_admin, last_login_at, is_staff, last_login_ip, uuid, group_id, plan_id,
  speed_limit, auto_renewal, remind_expire, remind_traffic, token, expired_at,
  remarks, created_at, updated_at
) VALUES
  (
    1, NULL, 9007199254740991, 'owner@example.test',
    'legacy-password-hash-owner', 'bcrypt', 'saltowner1', 12345, 90, 0, 10,
    345, 1700000100, 11111111111, 22222222222, 1099511627776, 5, 0, 1,
    1700000050, 1, 2130706433, '00000000-0000-0000-0000-000000000001',
    1, 1, 1000, 1, 1, 1, '00000000000000000000000000000001',
    1890000000, '主用户 ☃', 1700000000, 1700000001
  ),
  (
    2, 1, NULL, 'invitee@example.test', 'legacy-password-hash-invitee',
    NULL, NULL, 678, NULL, 1, 15, 90, 1700000200, 333, 444,
    1099511627776, NULL, 0, 0, NULL, 0, NULL,
    '00000000-0000-0000-0000-000000000002', 1, 1, NULL, 0, 1, 1,
    '00000000000000000000000000000002', 1890000000, '被邀请用户',
    1700000002, 1700000003
  );

INSERT INTO v2_order (
  id, invite_user_id, user_id, plan_id, coupon_id, payment_id, type, period,
  trade_no, callback_no, total_amount, handling_amount, discount_amount,
  surplus_amount, refund_amount, balance_amount, surplus_order_ids, status,
  commission_status, commission_balance, actual_commission_balance, paid_at,
  created_at, updated_at
) VALUES (
  1, 1, 2, 1, 1, 1, 1, 'month_price',
  '00000000-0000-0000-0000-000000000010', 'legacy-callback-1', 999, 37,
  500, 0, 0, 499, '["1"]', 3, 2, 50, 50, 1700000300,
  1700000200, 1700000300
);

INSERT INTO v2_commission_log (
  id, invite_user_id, user_id, trade_no, order_amount, get_amount,
  created_at, updated_at
) VALUES (
  1, 1, 2, '00000000-0000-0000-0000-000000000010', 999, 50,
  1700000300, 1700000301
);

INSERT INTO v2_invite_code (id, user_id, code, status, pv, created_at, updated_at)
VALUES (1, 1, '00000000000000000000000000000011', 0, 7, 1700000000, 1700000001);

INSERT INTO v2_giftcard (
  id, code, name, type, value, plan_id, limit_use, used_user_ids,
  started_at, ended_at, created_at, updated_at
) VALUES (
  1, 'GIFTCARD-ONE', '迁移礼品卡', 1, 1000, 1, 3, '[1,"2",2]',
  1690000000, 1890000000, 1700000000, 1700000001
);

INSERT INTO v2_knowledge (
  id, language, category, title, body, sort, `show`, created_at, updated_at
) VALUES (
  1, 'zh-CN', '入门', '迁移知识库', '# 标题\n保留 Markdown 与 Unicode。', 1, 1,
  1700000000, 1700000001
);

INSERT INTO v2_notice (
  id, title, content, `show`, img_url, tags, created_at, updated_at
) VALUES (
  1, '迁移公告', '数据已通过真实转换器验证。', 1, NULL,
  '["弹窗","migration"]', 1700000000, 1700000001
);

INSERT INTO v2_ticket (
  id, user_id, subject, level, status, reply_status, created_at, updated_at
) VALUES (1, 2, '迁移工单', 1, 1, 1, 1700000000, 1700000001);

INSERT INTO v2_ticket_message (
  id, user_id, ticket_id, message, created_at, updated_at
) VALUES (1, 2, 1, '迁移后仍应逐字保留。', 1700000000, 1700000001);

INSERT INTO v2_log (
  id, title, level, host, uri, method, data, ip, context, created_at, updated_at
) VALUES (
  1, '迁移日志', 'info', 'panel.example.test', '/api/test', 'POST',
  '{"amount":9007199254740993}', '127.0.0.1', '上下文',
  1700000000, 1700000001
);

INSERT INTO v2_mail_log (
  id, email, subject, template_name, error, created_at, updated_at
) VALUES (
  1, 'owner@example.test', '迁移邮件', 'migration-test', NULL,
  1700000000, 1700000001
);

INSERT INTO v2_stat (
  id, record_at, record_type, order_count, order_total, commission_count,
  commission_total, paid_count, paid_total, register_count, invite_count,
  transfer_used_total, created_at, updated_at
) VALUES (
  1, 1700000000, 'd', 3, 2997, 1, 50, 2, 1998, 2, 1,
  '9007199254740993', 1700000000, 1700000001
);

INSERT INTO v2_stat_server (
  id, server_id, server_type, u, d, record_type, record_at, created_at, updated_at
) VALUES (1, 1, 'vmess', 1234567890123, 2345678901234, 'd', 1700000000, 1700000000, 1700000001);

INSERT INTO v2_stat_user (
  id, user_id, server_rate, u, d, record_type, record_at, created_at, updated_at
) VALUES (1, 2, 12345678.90, 3456789012345, 4567890123456, 'd', 1700000000, 1700000000, 1700000001);

INSERT INTO v2_server_route (
  id, remarks, `match`, action, action_value, created_at, updated_at
) VALUES (
  1, '迁移路由', '["example.com","*.internal"]', 'block', '保留的动作值',
  1700000000, 1700000001
);

INSERT INTO v2_server_shadowsocks (
  id, group_id, route_id, parent_id, tags, name, rate, host, port,
  server_port, cipher, obfs, obfs_settings, `show`, sort, created_at, updated_at
) VALUES (
  1, '["1"]', '[1]', NULL, '["edge","测试"]', 'ss-节点', '1.25',
  'ss.example.test', '443', 10001, 'aes-256-gcm', 'http',
  '{"host":"cdn.example.test"}', 1, 1, 1700000000, 1700000001
);

INSERT INTO v2_server_vmess (
  id, group_id, route_id, name, parent_id, host, port, server_port, tls, tags,
  rate, network, rules, networkSettings, tlsSettings, ruleSettings, dnsSettings,
  `show`, sort, created_at, updated_at
) VALUES (
  1, '[1]', '["1"]', 'vmess-节点', NULL, 'vmess.example.test', '443',
  10002, 1, '["edge"]', '1.50', 'ws', '[{"type":"field"}]',
  '{"path":"/ws"}', '{"serverName":"vmess.example.test"}', '{}',
  '{"servers":["1.1.1.1"]}', 1, 2, 1700000000, 1700000001
);

INSERT INTO v2_server_trojan (
  id, group_id, route_id, parent_id, tags, name, rate, host, port,
  server_port, network, network_settings, allow_insecure, server_name,
  `show`, sort, created_at, updated_at
) VALUES (
  1, '[1]', '[1]', NULL, '["edge"]', 'trojan-节点', '1.75',
  'trojan.example.test', '443', 10003, 'tcp', '{"header":{"type":"none"}}',
  0, 'trojan.example.test', 1, 3, 1700000000, 1700000001
);

INSERT INTO v2_server_tuic (
  id, group_id, route_id, name, parent_id, host, port, server_port, tags,
  rate, `show`, sort, server_name, insecure, disable_sni, udp_relay_mode,
  zero_rtt_handshake, congestion_control, created_at, updated_at
) VALUES (
  1, '[1]', '[1]', 'tuic-节点', NULL, 'tuic.example.test', '443', 10004,
  '["edge"]', '2.00', 1, 4, 'tuic.example.test', 0, 0, 'native', 1,
  'bbr', 1700000000, 1700000001
);

INSERT INTO v2_server_hysteria (
  id, version, group_id, route_id, name, parent_id, host, port, server_port,
  tags, rate, `show`, sort, up_mbps, down_mbps, obfs, obfs_password,
  server_name, insecure, created_at, updated_at
) VALUES (
  1, 2, '[1]', '[1]', 'hysteria-节点', NULL, 'hy.example.test', '443',
  10005, '["edge"]', '2.25', 1, 5, 100, 200, 'salamander', 'obfs-secret',
  'hy.example.test', 0, 1700000000, 1700000001
);

INSERT INTO v2_server_vless (
  id, group_id, route_id, name, parent_id, host, port, server_port, tls,
  tls_settings, flow, network, network_settings, encryption,
  encryption_settings, tags, rate, `show`, sort, created_at, updated_at
) VALUES (
  1, '[1]', '[1]', 'vless-节点', NULL, 'vless.example.test', 443, 10006, 1,
  '{"serverName":"vless.example.test"}', 'xtls-rprx-vision', 'tcp',
  '{"header":{"type":"none"}}', 'none', '{}', '["edge"]', '2.50', 1, 6,
  1700000000, 1700000001
);

INSERT INTO v2_server_anytls (
  id, group_id, route_id, name, parent_id, host, port, server_port, tags,
  rate, `show`, sort, server_name, insecure, padding_scheme,
  created_at, updated_at
) VALUES (
  1, '[1]', '[1]', 'anytls-节点', NULL, 'anytls.example.test', '443', 10007,
  '["edge"]', '2.75', 1, 7, 'anytls.example.test', 0,
  '["stop=8","0=30-30"]', 1700000000, 1700000001
);

INSERT INTO v2_server_v2node (
  id, group_id, route_id, name, parent_id, host, listen_ip, port, server_port,
  tags, rate, `show`, sort, protocol, tls, tls_settings, flow, network,
  network_settings, encryption, encryption_settings, disable_sni,
  udp_relay_mode, zero_rtt_handshake, congestion_control, cipher, up_mbps,
  down_mbps, obfs, obfs_password, padding_scheme, created_at, updated_at
) VALUES (
  1, '[1]', '[1]', 'v2node-节点', NULL, 'v2node.example.test', '0.0.0.0',
  '443', 10008, '["edge"]', '3.00', 1, 8, 'vless', 1,
  '{"serverName":"v2node.example.test"}', 'xtls-rprx-vision', 'tcp',
  '{"header":{"type":"none"}}', 'none', '{}', 0, 'native', 1, 'bbr',
  'aes-256-gcm', 300, 600, 'salamander', 'v2node-obfs',
  '["stop=8","0=30-30"]', 1700000000, 1700000001
);
