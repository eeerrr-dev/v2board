SET NAMES utf8mb4;

INSERT INTO v2_server_group (id, name, created_at, updated_at)
VALUES (1, 'default', 1700000000, 1700000000);

INSERT INTO v2_plan (
    id, group_id, transfer_enable, device_limit, name, speed_limit, `show`, sort,
    renew, content, month_price, quarter_price, half_year_price, year_price,
    two_year_price, three_year_price, onetime_price, reset_price,
    reset_traffic_method, capacity_limit, created_at, updated_at
) VALUES (
    1, 1, 1073741824, 5, 'Standard', 100, 1, 1,
    1, 'standard plan', 1000, 2700, 5000, 9000,
    16000, 21000, 30000, 500, 1, 100,
    1700000000, 1700000000
);

INSERT INTO v2_payment (
    id, uuid, payment, name, icon, config, notify_domain,
    handling_fee_fixed, handling_fee_percent, enable, sort, created_at, updated_at
) VALUES
    (1, '11111111111111111111111111111111', 'Manual', 'Manual payment', NULL,
     '{"account":"migration-test","exact":9007199254740993.25,"scientific":1.2300e3}', NULL,
     10, 1.25, 1, 1, 1700000000, 1700000000),
    (2, '22222222222222222222222222222222', 'StripeCheckout', 'Discarded Stripe', NULL,
     '{"secret_key":"must-not-migrate"}', NULL, 0, 0.00, 1, 2, 1700000000, 1700000000);

INSERT INTO v2_coupon (
    id, code, name, type, value, `show`, limit_use, limit_use_with_user,
    limit_plan_ids, limit_period, started_at, ended_at, created_at, updated_at
) VALUES (
    1, 'SAVE10', 'Ten percent', 2, 10, 1, 100, 1,
    '[1]', '["month_price"]', 1690000000, 1890000000, 1700000000, 1700000000
);

INSERT INTO v2_user (
    id, invite_user_id, telegram_id, email, password, password_algo, password_salt,
    balance, discount, commission_type, commission_rate, commission_balance,
    t, u, d, transfer_enable, device_limit, banned, is_admin, last_login_at,
    is_staff, last_login_ip, uuid, group_id, plan_id, speed_limit, auto_renewal,
    remind_expire, remind_traffic, token, expired_at, remarks, created_at, updated_at
) VALUES
    (1, NULL, 10001, 'owner@example.test', 'legacy-hash-1', NULL, NULL,
     1000, NULL, 0, NULL, 0, 1700000000, 100, 200, 107374182400, 5,
     0, 1, 1700000100, 0, NULL, '00000000-0000-0000-0000-000000000001',
     1, 1, 100, 1, 1, 1, '11111111111111111111111111111111',
     1800000000, 'owner', 1700000000, 1700000000),
    (2, 1, NULL, 'member@example.test', 'legacy-hash-2', NULL, NULL,
     0, NULL, 0, NULL, 0, 1700000000, 0, 0, 107374182400, 5,
     0, 0, NULL, 0, NULL, '00000000-0000-0000-0000-000000000002',
     1, 1, NULL, 0, 1, 1, '22222222222222222222222222222222',
     1800000000, '', 1700000000, 1700000000);

INSERT INTO v2_order (
    id, invite_user_id, user_id, plan_id, coupon_id, payment_id, type, period,
    trade_no, callback_no, total_amount, handling_amount, discount_amount,
    surplus_amount, refund_amount, balance_amount, surplus_order_ids, status,
    commission_status, commission_balance, actual_commission_balance, paid_at,
    created_at, updated_at
) VALUES
    (1, NULL, 1, 1, 1, 1, 1, 'month_price',
     'trade-manual-complete', 'manual-callback', 1000, 10, 100,
     0, 0, 0, NULL, 3, 2, 100, 100, 1700000200, 1700000000, 1700000200),
    (2, NULL, 2, 1, NULL, 2, 1, 'month_price',
     'trade-stripe-pending', 'pi_pending', 1000, 0, 0,
     0, 0, 0, NULL, 0, 0, 0, NULL, NULL, 1700000000, 1700000000),
    (3, NULL, 2, 1, NULL, 2, 1, 'month_price',
     'trade-stripe-complete', 'pi_complete', 1000, 0, 0,
     0, 0, 0, '[1]', 3, 0, 0, NULL, 1700000200, 1700000000, 1700000200);

INSERT INTO v2_commission_log (
    id, invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at
) VALUES (1, 1, 2, 'trade-manual-complete', 1000, 100, 1700000200, 1700000200);

INSERT INTO v2_invite_code (id, user_id, code, status, pv, created_at, updated_at)
VALUES (1, 1, 'INVITECODE0000000000000000000001', 0, 2, 1700000000, 1700000000);

INSERT INTO v2_giftcard (
    id, code, name, type, value, plan_id, limit_use, used_user_ids,
    started_at, ended_at, created_at, updated_at
) VALUES (
    1, 'GIFT100', 'Gift 100', 1, 100, NULL, 10, '[1,2]',
    1690000000, 1890000000, 1700000000, 1700000000
);

INSERT INTO v2_knowledge (
    id, language, category, title, body, sort, `show`, created_at, updated_at
) VALUES (1, 'zh-CN', 'guide', 'Getting started', 'Body', 1, 1, 1700000000, 1700000000);

INSERT INTO v2_notice (id, title, content, `show`, img_url, tags, created_at, updated_at)
VALUES (
    1, 'Notice',
    CONCAT(CHAR(92), 'N, "quoted"', CHAR(13), CHAR(10), 'line', CHAR(92), 'tail'),
    1, NULL, '["弹窗"]', 1700000000, 1700000000
);

INSERT INTO v2_ticket (
    id, user_id, subject, level, status, reply_status, created_at, updated_at
) VALUES (1, 1, 'Migration test', 1, 0, 0, 1700000000, 1700000000);

INSERT INTO v2_ticket_message (id, user_id, ticket_id, message, created_at, updated_at)
VALUES (1, 1, 1, 'Test message', 1700000000, 1700000000);

INSERT INTO v2_stat (
    id, record_at, record_type, order_count, order_total, commission_count,
    commission_total, paid_count, paid_total, register_count, invite_count,
    transfer_used_total, created_at, updated_at
) VALUES (
    1, 1700000000, 'd', 3, 3000, 1, 100, 2, 2000, 2, 1,
    '300', 1700000000, 1700000000
);

INSERT INTO v2_log (
    id, title, level, host, uri, method, data, ip, context, created_at, updated_at
) VALUES (
    1, 'discarded log', 'info', 'old-host', '/', 'GET', NULL, '127.0.0.1', NULL,
    1700000000, 1700000000
);

INSERT INTO failed_jobs (
    id, connection, queue, payload, exception, failed_at
) VALUES (1, 'redis', 'default', '{}', 'discarded failure', CURRENT_TIMESTAMP);

-- Some upgraded legacy installations retain this unused, empty table even
-- though current installs and the application no longer use it. The importer
-- allowlists its presence and discards it without inspecting its schema/rows.
CREATE TABLE v2_tutorial (
    id int NOT NULL AUTO_INCREMENT,
    category_id int NOT NULL,
    title varchar(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci NOT NULL,
    steps text,
    `show` tinyint(1) NOT NULL DEFAULT '0',
    sort int DEFAULT NULL,
    created_at int NOT NULL,
    updated_at int NOT NULL,
    PRIMARY KEY (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;
