// ——— W3 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.1,
// §5.3, §5.8) ——— fixture-data.mjs stays the single legacy-shaped source;
// the source world serializes these derived modern shapes (booleans, real
// arrays, RFC 3339 timestamps, numeric rates) for the flipped family.
export const rfc3339FixtureTime = (epochSeconds) =>
  new Date(epochSeconds * 1000).toISOString().replace(/\.\d{3}Z$/, 'Z');

export const modernPublicConfigFixture = (fixture) => ({
  ...fixture,
  is_email_verify: fixture.is_email_verify !== 0,
  is_invite_force: fixture.is_invite_force !== 0,
  is_recaptcha: fixture.is_recaptcha !== 0,
  // §5.1: always an array — the legacy `0` disabled-sentinel died.
  email_whitelist_suffix: Array.isArray(fixture.email_whitelist_suffix)
    ? fixture.email_whitelist_suffix
    : [],
});

export const modernUserConfigFixture = (fixture) => ({
  ...fixture,
  is_telegram: fixture.is_telegram !== 0,
  withdraw_close: fixture.withdraw_close !== 0,
  commission_distribution_enable: fixture.commission_distribution_enable !== 0,
  commission_distribution_l1: numericRate(fixture.commission_distribution_l1),
  commission_distribution_l2: numericRate(fixture.commission_distribution_l2),
  commission_distribution_l3: numericRate(fixture.commission_distribution_l3),
});

const numericRate = (value) => {
  if (value === null || value === undefined) return null;
  const rate = Number(value);
  return Number.isFinite(rate) ? rate : null;
};

export const modernNoticeFixture = (notice) => ({
  id: notice.id,
  title: notice.title,
  content: notice.content,
  show: notice.show !== 0,
  img_url: notice.img_url ?? null,
  tags: notice.tags ?? null,
  created_at: rfc3339FixtureTime(notice.created_at),
  updated_at: rfc3339FixtureTime(notice.updated_at),
});

export const modernKnowledgeSummaryFixture = (row) => ({
  id: row.id,
  category: row.category,
  title: row.title,
  sort: row.sort ?? null,
  show: row.show !== 0,
  updated_at: rfc3339FixtureTime(row.updated_at),
});

export const modernKnowledgeDetailFixture = (row) => ({
  ...modernKnowledgeSummaryFixture(row),
  body: row.body,
  language: row.language,
  created_at: rfc3339FixtureTime(row.created_at),
});

export const modernKnowledgeRecordFixture = (fixtures) =>
  Object.fromEntries(
    Object.entries(fixtures).map(([category, rows]) => [
      category,
      rows.map(modernKnowledgeSummaryFixture),
    ]),
  );

// ——— W4 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.5) ———
// booleans for show/renew, RFC 3339 timestamps, a numeric
// handling_fee_percent, and no legacy plan `count` (the modern plan body
// serializes remaining capacity in capacity_limit and drops the sold count).
export const modernPlanFixture = (plan) => {
  const { count: _count, ...rest } = plan;
  return {
    ...rest,
    show: plan.show !== 0,
    renew: plan.renew !== 0,
    created_at: rfc3339FixtureTime(plan.created_at),
    updated_at: rfc3339FixtureTime(plan.updated_at),
  };
};

export const modernOrderFixture = (order) => ({
  ...order,
  actual_commission_balance: order.actual_commission_balance ?? null,
  paid_at: order.paid_at == null ? null : rfc3339FixtureTime(order.paid_at),
  created_at: rfc3339FixtureTime(order.created_at),
  updated_at: rfc3339FixtureTime(order.updated_at),
  ...(order.plan
    ? {
        // Deposit orders carry the §5.5 `{id: 0, name: "deposit"}` synthetic
        // plan; real plans project like the plan routes.
        plan: order.plan.id === 0 ? { id: 0, name: 'deposit' } : modernPlanFixture(order.plan),
      }
    : {}),
});

export const modernPaymentMethodFixture = (paymentMethod) => ({
  ...paymentMethod,
  handling_fee_percent:
    paymentMethod.handling_fee_percent == null
      ? null
      : Number(paymentMethod.handling_fee_percent),
});

export const modernCouponFixture = (coupon) => ({
  ...coupon,
  show: coupon.show !== 0,
  started_at: rfc3339FixtureTime(coupon.started_at),
  ended_at: rfc3339FixtureTime(coupon.ended_at),
  created_at: rfc3339FixtureTime(coupon.created_at),
  updated_at: rfc3339FixtureTime(coupon.updated_at),
});

// ——— W10 modern-wire projection (docs/api-dialect.md §6.3) ——— RFC 3339
// windows and a real `used_user_ids` array (the legacy null sentinel died).
export const modernGiftcardFixture = (giftcard) => ({
  ...giftcard,
  used_user_ids: giftcard.used_user_ids ?? [],
  started_at: rfc3339FixtureTime(giftcard.started_at),
  ended_at: rfc3339FixtureTime(giftcard.ended_at),
  created_at: rfc3339FixtureTime(giftcard.created_at),
  updated_at: rfc3339FixtureTime(giftcard.updated_at),
});

// ——— W11 modern-wire projections (docs/api-dialect.md §6.2, §6.4) ———
// boolean show/renew/enable flags, RFC 3339 timestamps, a numeric
// handling_fee_percent, and prices/fees that stay cents. The admin plan list
// keeps the sold `count` (unlike the user-side §5.5 plan body, which drops it).
export const modernAdminPlanFixture = (plan) => ({
  ...plan,
  show: plan.show !== 0,
  renew: plan.renew !== 0,
  created_at: rfc3339FixtureTime(plan.created_at),
  updated_at: rfc3339FixtureTime(plan.updated_at),
});

export const modernAdminPaymentFixture = (payment) => ({
  ...payment,
  enable: payment.enable !== 0,
  handling_fee_percent:
    payment.handling_fee_percent == null ? null : Number(payment.handling_fee_percent),
  legacy_md5_signature: payment.legacy_md5_signature ?? false,
  security_warning: payment.security_warning ?? null,
  created_at: rfc3339FixtureTime(payment.created_at),
  updated_at: rfc3339FixtureTime(payment.updated_at),
});

const modernAdminOrderFieldsFixture = (order) => {
  const {
    commission_log: _commissionLog,
    email: _email,
    payment_reconciliation_open_count: _paymentReconciliationOpenCount,
    payment_reconciliations: _paymentReconciliations,
    plan_name: _planName,
    ...fields
  } = order;
  return {
    ...fields,
    actual_commission_balance: order.actual_commission_balance ?? null,
    paid_at: order.paid_at == null ? null : rfc3339FixtureTime(order.paid_at),
    created_at: rfc3339FixtureTime(order.created_at),
    updated_at: rfc3339FixtureTime(order.updated_at),
  };
};

export const modernAdminOrderFixture = (order) => ({
  ...modernAdminOrderFieldsFixture(order),
  email: order.email ?? `user-${order.user_id}@example.test`,
  plan_name: order.plan_name ?? null,
  payment_reconciliation_open_count: order.payment_reconciliation_open_count ?? 0,
});

export const modernAdminOrderDetailFixture = (order) => ({
  ...modernAdminOrderFieldsFixture(order),
  commission_log: (order.commission_log ?? []).map((entry) => ({
    ...entry,
    created_at: rfc3339FixtureTime(entry.created_at),
    updated_at: rfc3339FixtureTime(entry.updated_at),
  })),
  payment_reconciliations: (order.payment_reconciliations ?? []).map((entry) => ({
    ...entry,
    first_seen_at: rfc3339FixtureTime(entry.first_seen_at),
    last_seen_at: rfc3339FixtureTime(entry.last_seen_at),
    resolved_at: entry.resolved_at == null ? null : rfc3339FixtureTime(entry.resolved_at),
  })),
});

// ——— W12 modern-wire projections (docs/api-dialect.md §6.6) ——— the admin user
// list/detail keep 0/1 flag columns and integer bytes/cents, cross every epoch
// field as RFC 3339 UTC (nullable ones stay null), drop the `t` online marker
// and the stored password, and — on the detail only — attach the conditional
// `invite_user: {id, email}` object resolved from the inviter row.
export const modernAdminUserFixture = (user) => ({
  ...user,
  password: '',
  // §6.12: the modern projection always carries the staff grant array.
  admin_permissions: user.admin_permissions ?? [],
  auto_renewal: user.auto_renewal ?? 0,
  commission_type: user.commission_type ?? 0,
  remarks: user.remarks ?? null,
  remind_expire: user.remind_expire ?? 1,
  remind_traffic: user.remind_traffic ?? 1,
  speed_limit: user.speed_limit ?? null,
  expired_at: user.expired_at == null ? null : rfc3339FixtureTime(user.expired_at),
  last_login_at: user.last_login_at == null ? null : rfc3339FixtureTime(user.last_login_at),
  created_at: rfc3339FixtureTime(user.created_at),
  updated_at: rfc3339FixtureTime(user.updated_at),
});

export const modernAdminUserDetailFixture = (user, users) => {
  const inviter =
    user.invite_user_id == null ? null : users.find((row) => row.id === user.invite_user_id);
  return {
    ...modernAdminUserFixture(user),
    subscribe_url: user.subscribe_url ?? '',
    ...(inviter ? { invite_user: { id: inviter.id, email: inviter.email } } : {}),
  };
};

// ——— W13 modern-wire projections (docs/api-dialect.md §6.7) ——— the nodes
// list drops the legacy `is_online`, casts show to boolean and rate/id arrays
// to numbers, crosses timestamps as RFC 3339 UTC, and always carries the
// projection's api_key/last_push_at columns; groups and routes keep their
// rows with RFC 3339 timestamps.
const fixtureBoolean = (value) => value === true || value === 1 || value === '1';

const modernAdminServerProtocolFixture = (node) => {
  switch (node.type) {
    case 'shadowsocks':
      return {
        cipher: node.cipher ?? 'aes-128-gcm',
        obfs: node.obfs ?? null,
        obfs_settings: node.obfs_settings ?? null,
      };
    case 'vmess':
      return {
        dnsSettings: node.dnsSettings ?? null,
        network: node.network ?? 'tcp',
        networkSettings: node.networkSettings ?? node.network_settings ?? null,
        rules: node.rules ?? null,
        ruleSettings: node.ruleSettings ?? null,
        tls: Number(node.tls ?? 0),
        tlsSettings: node.tlsSettings ?? node.tls_settings ?? null,
      };
    case 'trojan':
      return {
        allow_insecure: fixtureBoolean(node.allow_insecure ?? false),
        network: node.network ?? null,
        network_settings: node.network_settings ?? null,
        server_name: node.server_name ?? null,
      };
    case 'tuic':
      return {
        congestion_control: node.congestion_control ?? null,
        disable_sni: fixtureBoolean(node.disable_sni ?? false),
        insecure: fixtureBoolean(node.insecure ?? false),
        server_name: node.server_name ?? null,
        udp_relay_mode: node.udp_relay_mode ?? null,
        zero_rtt_handshake: fixtureBoolean(node.zero_rtt_handshake ?? false),
      };
    case 'hysteria':
      return {
        down_mbps: Number(node.down_mbps ?? 0),
        insecure: fixtureBoolean(node.insecure ?? false),
        obfs: node.obfs ?? null,
        obfs_password: node.obfs_password ?? null,
        server_name: node.server_name ?? null,
        up_mbps: Number(node.up_mbps ?? 0),
        version: Number(node.version ?? 2),
      };
    case 'vless':
      return {
        encryption: node.encryption ?? null,
        encryption_settings: node.encryption_settings ?? null,
        flow: node.flow ?? null,
        network: node.network ?? 'tcp',
        network_settings: node.network_settings ?? null,
        tls: Number(node.tls ?? 0),
        tls_settings: node.tls_settings ?? null,
      };
    case 'anytls':
      return {
        insecure: fixtureBoolean(node.insecure ?? false),
        padding_scheme: node.padding_scheme ?? null,
        server_name: node.server_name ?? null,
      };
    case 'v2node':
      return {
        cipher: node.cipher ?? null,
        congestion_control: node.congestion_control ?? null,
        disable_sni: fixtureBoolean(node.disable_sni ?? false),
        down_mbps: Number(node.down_mbps ?? 0),
        encryption: node.encryption ?? null,
        encryption_settings: node.encryption_settings ?? null,
        flow: node.flow ?? null,
        install_command: node.install_command ?? '',
        listen_ip: node.listen_ip ?? '0.0.0.0',
        network: node.network ?? 'tcp',
        network_settings: node.network_settings ?? null,
        obfs: node.obfs ?? null,
        obfs_password: node.obfs_password ?? null,
        padding_scheme: node.padding_scheme ?? null,
        protocol: node.protocol ?? 'vless',
        tls: Number(node.tls ?? 0),
        tls_settings: node.tls_settings ?? null,
        udp_relay_mode: node.udp_relay_mode ?? null,
        up_mbps: Number(node.up_mbps ?? 0),
        zero_rtt_handshake: fixtureBoolean(node.zero_rtt_handshake ?? false),
      };
    default:
      throw new Error(`Unsupported modern admin server fixture type: ${node.type}`);
  }
};

export const modernAdminServerNodeFixture = (node) => {
  return {
    ...modernAdminServerProtocolFixture(node),
    api_key: node.api_key ?? null,
    available_status: node.available_status ?? 0,
    created_at: rfc3339FixtureTime(node.created_at ?? 1_700_000_000),
    group_id: node.group_id.map(Number),
    host: node.host,
    id: node.id,
    last_check_at: node.last_check_at == null ? null : rfc3339FixtureTime(node.last_check_at),
    last_push_at:
      node.last_push_at == null ? null : rfc3339FixtureTime(node.last_push_at),
    name: node.name,
    online: node.online ?? null,
    parent_id: node.parent_id ?? null,
    port: Number(node.port),
    rate: Number(node.rate),
    route_id: node.route_id == null ? null : node.route_id.map(Number),
    server_port: Number(node.server_port ?? node.port),
    show: node.show !== 0,
    sort: node.sort ?? node.id,
    tags: node.tags ?? null,
    type: node.type,
    updated_at: rfc3339FixtureTime(node.updated_at ?? 1_700_000_000),
  };
};

export const modernAdminServerGroupFixture = (group) => ({
  ...group,
  created_at: rfc3339FixtureTime(group.created_at),
  updated_at: rfc3339FixtureTime(group.updated_at),
});

export const modernAdminServerRouteFixture = (route) => ({
  ...route,
  created_at: rfc3339FixtureTime(route.created_at),
  updated_at: rfc3339FixtureTime(route.updated_at),
});

// ——— W5 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.3,
// §5.4) ——— boolean profile/subscription flags, RFC 3339 timestamps, and the
// subscription's explicit-null modern plan.
export const modernUserProfileFixture = (fixture) => ({
  ...fixture,
  banned: fixture.banned !== 0,
  auto_renewal: fixture.auto_renewal !== 0,
  remind_expire: fixture.remind_expire !== 0,
  remind_traffic: fixture.remind_traffic !== 0,
  created_at: rfc3339FixtureTime(fixture.created_at),
  last_login_at:
    fixture.last_login_at == null ? null : rfc3339FixtureTime(fixture.last_login_at),
  expired_at: fixture.expired_at == null ? null : rfc3339FixtureTime(fixture.expired_at),
});

export const modernSubscribeFixture = (fixture) => ({
  ...fixture,
  allow_new_period: fixture.allow_new_period !== 0,
  expired_at: fixture.expired_at == null ? null : rfc3339FixtureTime(fixture.expired_at),
  plan: fixture.plan ? modernPlanFixture(fixture.plan) : null,
});

// ——— W6 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.4) ———
// boolean is_online, numeric rate/port/server_rate, RFC 3339
// last_check_at/record_at.
export const modernServerFixture = (server) => ({
  ...server,
  rate: Number(server.rate),
  port: Number(server.port),
  is_online: server.is_online !== 0,
  sort: server.sort ?? server.id,
  last_check_at: server.last_check_at == null ? null : rfc3339FixtureTime(server.last_check_at),
});

export const modernTrafficLogFixture = (entry) => ({
  ...entry,
  record_at: rfc3339FixtureTime(entry.record_at),
  server_rate: Number(entry.server_rate),
});

// ——— W7 modern-wire projections (docs/api-dialect.md §4.5, §5.6, §8,
// §9.2) ——— the named invite stat object (was the legacy 5-tuple), RFC 3339
// timestamps, and the constant-status/caller-echo code columns dropped from
// the wire. Commission amounts stay integer cents.
export const modernInviteFixture = (fixture) => ({
  codes: fixture.codes.map((code) => ({
    id: code.id,
    code: code.code,
    pv: code.pv,
    created_at: rfc3339FixtureTime(code.created_at),
    updated_at: rfc3339FixtureTime(code.updated_at),
  })),
  stat: {
    registered_count: fixture.stat[0],
    valid_commission: fixture.stat[1],
    pending_commission: fixture.stat[2],
    commission_rate: fixture.stat[3],
    available_commission: fixture.stat[4],
  },
});

export const modernCommissionFixture = (entry) => ({
  id: entry.id,
  trade_no: entry.trade_no,
  order_amount: entry.order_amount,
  get_amount: entry.get_amount,
  created_at: rfc3339FixtureTime(entry.created_at),
});

// ——— W8 modern-wire projections (docs/api-dialect.md §4.1, §4.5, §5.7) ———
// RFC 3339 timestamps, numeric level/status/reply_status enums, an
// always-present nullable last_reply_user_id, and no `message` stub on list
// rows (the thread ships only on the detail body).
export const modernTicketFixture = (ticket) => ({
  id: ticket.id,
  user_id: ticket.user_id,
  subject: ticket.subject,
  level: ticket.level,
  status: ticket.status,
  reply_status: ticket.reply_status,
  last_reply_user_id: ticket.last_reply_user_id ?? null,
  created_at: rfc3339FixtureTime(ticket.created_at),
  updated_at: rfc3339FixtureTime(ticket.updated_at),
});

const modernTicketMessageFixture = (entry) => ({
  id: entry.id,
  user_id: entry.user_id,
  ticket_id: entry.ticket_id,
  message: entry.message,
  is_me: entry.is_me,
  created_at: rfc3339FixtureTime(entry.created_at),
  updated_at: rfc3339FixtureTime(entry.updated_at),
});

export const modernTicketDetailFixture = (ticket) => ({
  ...modernTicketFixture(ticket),
  message: (ticket.message ?? []).map(modernTicketMessageFixture),
});

// ——— W14 modern-wire projections (docs/api-dialect.md §4.5, §6.8) ———
// stats/user-traffic rows carry RFC 3339 record_at; server_rate is already a
// JSON number on the admin fixture row.
export const modernAdminStatFixture = (stat) => ({
  ...stat,
  payment_reconciliation_pending_amount: stat.payment_reconciliation_pending_amount ?? 0,
  payment_reconciliation_pending_total: stat.payment_reconciliation_pending_total ?? 0,
});

export const modernAdminUserTrafficFixture = (entry) => ({
  d: entry.d,
  record_at: rfc3339FixtureTime(entry.record_at),
  server_rate: Number(entry.server_rate),
  u: entry.u,
});

// ——— W9 modern-wire projections (docs/api-dialect.md §4.1, §6.1) ———
// the grouped config body flips every flag to a real boolean, keeps
// enums/counters as JSON numbers, converts email_port to a number, adds the
// §10.3 legacy_hash_redirect_enable site toggle, and pins
// commission_withdraw_limit to its decimal-string exception; queue
// stats/workload turn bare snake_case with a boolean status and RFC 3339
// last-run maps.
const modernConfigFlag = (value) => value !== 0 && value !== '0' && value !== false;

export const modernAdminConfigFixture = (config) => ({
  ...config,
  revision: 7,
  invite: {
    ...config.invite,
    invite_force: modernConfigFlag(config.invite.invite_force),
    invite_never_expire: modernConfigFlag(config.invite.invite_never_expire),
    commission_first_time_enable: modernConfigFlag(config.invite.commission_first_time_enable),
    commission_auto_check_enable: modernConfigFlag(config.invite.commission_auto_check_enable),
    withdraw_close_enable: modernConfigFlag(config.invite.withdraw_close_enable),
    commission_distribution_enable: modernConfigFlag(
      config.invite.commission_distribution_enable,
    ),
    commission_withdraw_limit: String(config.invite.commission_withdraw_limit),
  },
  site: {
    ...config.site,
    force_https: modernConfigFlag(config.site.force_https),
    stop_register: modernConfigFlag(config.site.stop_register),
    legacy_hash_redirect_enable: false,
  },
  subscribe: {
    ...config.subscribe,
    plan_change_enable: modernConfigFlag(config.subscribe.plan_change_enable),
    surplus_enable: modernConfigFlag(config.subscribe.surplus_enable),
    allow_new_period: modernConfigFlag(config.subscribe.allow_new_period),
    new_order_event_id: modernConfigFlag(config.subscribe.new_order_event_id),
    renew_order_event_id: modernConfigFlag(config.subscribe.renew_order_event_id),
    change_order_event_id: modernConfigFlag(config.subscribe.change_order_event_id),
    show_info_to_server_enable: modernConfigFlag(config.subscribe.show_info_to_server_enable),
  },
  server: {
    ...config.server,
    server_token: config.server.server_token ? '********' : null,
    device_limit_mode: modernConfigFlag(config.server.device_limit_mode),
  },
  email: {
    ...config.email,
    email_password: config.email.email_password ? '********' : null,
    email_port: config.email.email_port == null ? null : Number(config.email.email_port),
  },
  telegram: {
    ...config.telegram,
    telegram_bot_token: config.telegram.telegram_bot_token ? '********' : null,
    telegram_bot_enable: modernConfigFlag(config.telegram.telegram_bot_enable),
  },
  safe: {
    ...config.safe,
    email_verify: modernConfigFlag(config.safe.email_verify),
    safe_mode_enable: modernConfigFlag(config.safe.safe_mode_enable),
    admin_mfa_force: modernConfigFlag(config.safe.admin_mfa_force),
    email_whitelist_enable: modernConfigFlag(config.safe.email_whitelist_enable),
    email_gmail_limit_enable: modernConfigFlag(config.safe.email_gmail_limit_enable),
    recaptcha_enable: modernConfigFlag(config.safe.recaptcha_enable),
    recaptcha_key: config.safe.recaptcha_key ? '********' : null,
    register_limit_by_ip_enable: modernConfigFlag(config.safe.register_limit_by_ip_enable),
    password_limit_enable: modernConfigFlag(config.safe.password_limit_enable),
  },
});

const modernQueueLastRunMap = (stats) =>
  Object.fromEntries(Object.keys(stats.wait).map((name) => [name, rfc3339FixtureTime(1700000000)]));

export const modernQueueStatsFixture = (stats) => ({
  failed_jobs: stats.failedJobs,
  jobs_per_minute: stats.jobsPerMinute,
  last_failure_at: {},
  last_run_at: modernQueueLastRunMap(stats),
  last_success_at: modernQueueLastRunMap(stats),
  paused_masters: stats.pausedMasters,
  periods: { failed_jobs: stats.periods.failedJobs, recent_jobs: stats.periods.recentJobs },
  processes: stats.processes,
  queue_with_max_runtime: stats.queueWithMaxRuntime,
  queue_with_max_throughput: stats.queueWithMaxThroughput,
  recent_jobs: stats.recentJobs,
  status: stats.status,
  wait: stats.wait,
});

export const modernQueueWorkloadFixture = (row) => ({
  failed_jobs: 0,
  last_failure_at: null,
  last_run_at: rfc3339FixtureTime(1700000000),
  last_success_at: rfc3339FixtureTime(1700000000),
  length: row.length,
  name: row.name,
  processes: row.processes,
  recent_jobs: row.length,
  wait: row.wait,
});
