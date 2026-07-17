// Internal-dialect route map (docs/api-dialect.md §13.1) — the
// machine-readable form of the §5–6 route tables. Each entry names one
// canonical modern route id and carries a per-world URL matcher:
//
//   { id, legacy: { method, path, … }, modern: { method, path, … } }
//
// - `legacy` is the oracle-world shape (the frozen reference dialect). It
//   never changes.
// - `modern` is the live source-world shape. Per Appendix A §W0 the map
//   starts as identity — every `modern` row equals its `legacy` row — and
//   each family wave (W2+) replaces its rows with the §5–6 "New" column in
//   the same commit series that flips the family.
// - Paths are relative to `/api/v1` (§5 preamble). Admin entries are
//   parameterized by the dynamic `{secure_path}` prefix; `{name}` segments
//   are path parameters (optionally constrained via `params`).
// - `query` lists query parameters that must be present for the entry to
//   match (legacy `?id=`-style discriminators sharing one path); `bodyKeys`
//   does the same for body-discriminated legacy actions (the §6.4
//   `order/update` reconciliation arm). `aliases` lists additional legacy
//   paths that collapse into the same canonical route (§6.8 stat aliases,
//   legacy `save`/`update` merges).
//
// Where one legacy upsert splits into modern create/update rows (§6
// preamble), the `.create` entry is listed first and wins ambiguous
// legacy-world matches until the owning wave lands distinct modern paths.

export const API_PREFIX = '/api/v1';

export const WORLDS = Object.freeze(['oracle', 'source']);

/** §6.7 — the protocol CRUD `{type}` vocabulary. */
export const SERVER_TYPES = Object.freeze([
  'shadowsocks',
  'vmess',
  'trojan',
  'tuic',
  'hysteria',
  'vless',
  'anytls',
  'v2node',
]);

const route = (id, legacy, modern) => ({
  id,
  legacy,
  // Identity until the family's wave adds its modern row (§13.1, App A §W0).
  modern: modern ?? legacy,
});

export const routeMap = Object.freeze([
  // ——— §5.1 Public (was guest comm) — flipped to the modern rows in W3 ———
  route(
    'public.config',
    { method: 'GET', path: '/guest/comm/config' },
    { method: 'GET', path: '/public/config' },
  ),
  route(
    'public.invite-views.create',
    { method: 'POST', path: '/passport/comm/pv' },
    { method: 'POST', path: '/public/invite-views' },
  ),

  // ——— §5.2 Auth (was passport) — flipped to the modern rows in W2 ———
  route(
    'auth.register',
    { method: 'POST', path: '/passport/auth/register' },
    { method: 'POST', path: '/auth/register' },
  ),
  route(
    'auth.login',
    { method: 'POST', path: '/passport/auth/login' },
    { method: 'POST', path: '/auth/login' },
  ),
  route(
    'auth.quick-login',
    { method: 'GET', path: '/passport/auth/token2Login', query: ['token'] },
    { method: 'GET', path: '/auth/quick-login', query: ['token'] },
  ),
  route(
    'auth.token-login',
    { method: 'GET', path: '/passport/auth/token2Login', query: ['verify'] },
    // The legacy GET-with-side-effect exchange became a POST body (§5.2).
    { method: 'POST', path: '/auth/token-login' },
  ),
  route(
    'auth.password-reset',
    { method: 'POST', path: '/passport/auth/forget' },
    { method: 'POST', path: '/auth/password-reset' },
  ),
  route(
    'auth.step-up',
    { method: 'POST', path: '/passport/auth/stepUp' },
    { method: 'POST', path: '/auth/step-up' },
  ),
  route(
    'auth.quick-login-url',
    {
      method: 'POST',
      path: '/passport/auth/getQuickLoginUrl',
      // §5.2: consolidates with the duplicate user-side endpoint.
      aliases: ['/user/getQuickLoginUrl'],
    },
    { method: 'POST', path: '/auth/quick-login-url' },
  ),
  route(
    'auth.email-codes',
    { method: 'POST', path: '/passport/comm/sendEmailVerify' },
    { method: 'POST', path: '/auth/email-codes' },
  ),
  route(
    'auth.session.get',
    { method: 'GET', path: '/user/checkLogin' },
    { method: 'GET', path: '/auth/session' },
  ),
  route(
    'auth.session.delete',
    { method: 'POST', path: '/user/logout' },
    { method: 'DELETE', path: '/auth/session' },
  ),

  // ——— §5.3 User account & profile — flipped to the modern rows in W5 ———
  route(
    'user.profile.get',
    { method: 'GET', path: '/user/info' },
    { method: 'GET', path: '/user/profile' },
  ),
  route(
    'user.profile.update',
    { method: 'POST', path: '/user/update' },
    { method: 'PATCH', path: '/user/profile' },
  ),
  route(
    'user.password.update',
    { method: 'POST', path: '/user/changePassword' },
    { method: 'PUT', path: '/user/password' },
  ),
  route(
    'user.stats.get',
    { method: 'GET', path: '/user/getStat' },
    { method: 'GET', path: '/user/stats' },
  ),
  route(
    'user.sessions.list',
    { method: 'GET', path: '/user/getActiveSession' },
    { method: 'GET', path: '/user/sessions' },
  ),
  route(
    'user.sessions.delete',
    { method: 'POST', path: '/user/removeActiveSession' },
    // The legacy body-carried session_id became a path parameter (§9.4).
    { method: 'DELETE', path: '/user/sessions/{session_id}' },
  ),
  route(
    'user.commission-transfers.create',
    { method: 'POST', path: '/user/transfer' },
    // §5.3 — flipped with the W7 invite & commission family.
    { method: 'POST', path: '/user/commission-transfers' },
  ),
  route(
    'user.gift-card-redemptions.create',
    { method: 'POST', path: '/user/redeemgiftcard' },
    { method: 'POST', path: '/user/gift-card-redemptions' },
  ),
  route(
    'user.telegram-binding.delete',
    { method: 'GET', path: '/user/unbindTelegram' },
    { method: 'DELETE', path: '/user/telegram-binding' },
  ),
  route(
    'user.telegram-bot.get',
    { method: 'GET', path: '/user/telegram/getBotInfo' },
    // §5.3 — flipped with the W3 content family.
    { method: 'GET', path: '/user/telegram-bot' },
  ),
  route(
    'user.config.get',
    { method: 'GET', path: '/user/comm/config' },
    { method: 'GET', path: '/user/config' },
  ),

  // ——— §5.4 Subscription & service usage — subscription rows flipped in W5,
  // service-usage rows in W6 ———
  route(
    'user.subscription.get',
    { method: 'GET', path: '/user/getSubscribe' },
    { method: 'GET', path: '/user/subscription' },
  ),
  route(
    'user.subscription.new-period',
    { method: 'POST', path: '/user/newPeriod' },
    { method: 'POST', path: '/user/subscription/new-period' },
  ),
  route(
    'user.subscription.reset-token',
    { method: 'GET', path: '/user/resetSecurity' },
    // The legacy GET-with-side-effect rotation became a POST (§9.4).
    { method: 'POST', path: '/user/subscription/reset-token' },
  ),
  route(
    'user.servers.list',
    { method: 'GET', path: '/user/server/fetch' },
    { method: 'GET', path: '/user/servers' },
  ),
  route(
    'user.traffic-logs.list',
    { method: 'GET', path: '/user/stat/getTrafficLog' },
    { method: 'GET', path: '/user/traffic-logs' },
  ),

  // ——— §5.5 Commerce — flipped to the modern rows in W4 ———
  // The detail row is listed first so `/user/plans/{id}` outranks the
  // sibling list path (same ordering as the §5.8 knowledge rows).
  route(
    'user.plans.get',
    { method: 'GET', path: '/user/plan/fetch', query: ['id'] },
    { method: 'GET', path: '/user/plans/{id}' },
  ),
  route(
    'user.plans.list',
    { method: 'GET', path: '/user/plan/fetch' },
    { method: 'GET', path: '/user/plans' },
  ),
  route(
    'user.orders.create',
    { method: 'POST', path: '/user/order/save' },
    { method: 'POST', path: '/user/orders' },
  ),
  route(
    'user.orders.list',
    { method: 'GET', path: '/user/order/fetch' },
    { method: 'GET', path: '/user/orders' },
  ),
  route(
    'user.orders.get',
    { method: 'GET', path: '/user/order/detail' },
    { method: 'GET', path: '/user/orders/{trade_no}' },
  ),
  route(
    'user.orders.status',
    { method: 'GET', path: '/user/order/check' },
    { method: 'GET', path: '/user/orders/{trade_no}/status' },
  ),
  route(
    'user.orders.cancel',
    { method: 'POST', path: '/user/order/cancel' },
    { method: 'POST', path: '/user/orders/{trade_no}/cancel' },
  ),
  route(
    'user.orders.checkout',
    { method: 'POST', path: '/user/order/checkout' },
    { method: 'POST', path: '/user/orders/{trade_no}/checkout' },
  ),
  route(
    'user.orders.stripe-intent',
    { method: 'POST', path: '/user/order/stripe/intent' },
    { method: 'POST', path: '/user/orders/{trade_no}/stripe-intent' },
  ),
  route(
    'user.payment-methods.list',
    { method: 'GET', path: '/user/order/getPaymentMethod' },
    { method: 'GET', path: '/user/payment-methods' },
  ),
  route(
    'user.coupons.check',
    { method: 'POST', path: '/user/coupon/check' },
    { method: 'POST', path: '/user/coupons/check' },
  ),

  // ——— §5.6 Invite & commission — flipped to the modern rows in W7 ———
  route(
    'user.invite-codes.create',
    { method: 'GET', path: '/user/invite/save' },
    // The legacy GET-with-side-effect became the one deliberate 204 POST
    // create (§1/§5.6).
    { method: 'POST', path: '/user/invite-codes' },
  ),
  route(
    'user.invite.get',
    { method: 'GET', path: '/user/invite/fetch' },
    { method: 'GET', path: '/user/invite' },
  ),
  route(
    'user.commissions.list',
    { method: 'GET', path: '/user/invite/details' },
    { method: 'GET', path: '/user/commissions' },
  ),

  // ——— §5.7 Tickets — flipped to the modern rows in W8 ———
  // The detail row is listed first so `/user/tickets/{id}` outranks the
  // sibling list path on modern-world matches.
  route(
    'user.tickets.get',
    { method: 'GET', path: '/user/ticket/fetch', query: ['id'] },
    { method: 'GET', path: '/user/tickets/{id}' },
  ),
  route(
    'user.tickets.list',
    { method: 'GET', path: '/user/ticket/fetch' },
    { method: 'GET', path: '/user/tickets' },
  ),
  route(
    'user.tickets.create',
    { method: 'POST', path: '/user/ticket/save' },
    { method: 'POST', path: '/user/tickets' },
  ),
  route(
    'user.tickets.replies.create',
    { method: 'POST', path: '/user/ticket/reply' },
    // The legacy body-carried ticket id became a path parameter (§5.7).
    { method: 'POST', path: '/user/tickets/{id}/replies' },
  ),
  route(
    'user.tickets.close',
    { method: 'POST', path: '/user/ticket/close' },
    { method: 'POST', path: '/user/tickets/{id}/close' },
  ),
  route(
    'user.withdrawal-tickets.create',
    { method: 'POST', path: '/user/ticket/withdraw' },
    { method: 'POST', path: '/user/withdrawal-tickets' },
  ),

  // ——— §5.8 Knowledge & notices — flipped to the modern rows in W3 ———
  // The detail row is listed first so `/user/knowledge/{id}` outranks the
  // sibling list path on modern-world matches.
  route(
    'user.knowledge.get',
    { method: 'GET', path: '/user/knowledge/fetch', query: ['id'] },
    { method: 'GET', path: '/user/knowledge/{id}' },
  ),
  route(
    'user.knowledge.list',
    { method: 'GET', path: '/user/knowledge/fetch' },
    { method: 'GET', path: '/user/knowledge' },
  ),
  route(
    'user.knowledge-categories.list',
    { method: 'GET', path: '/user/knowledge/getCategory' },
    { method: 'GET', path: '/user/knowledge-categories' },
  ),
  // §5.8: the legacy `?id=` single-notice branch is dropped (recorded
  // decision) — the modern route is list-only.
  route(
    'user.notices.list',
    { method: 'GET', path: '/user/notice/fetch' },
    { method: 'GET', path: '/user/notices' },
  ),

  // ——— §6.1 Config & system ———
  route('admin.config.get', { method: 'GET', path: '/{secure_path}/config/fetch' }),
  route('admin.config.update', { method: 'POST', path: '/{secure_path}/config/save' }),
  route('admin.email-templates.list', {
    method: 'GET',
    path: '/{secure_path}/config/getEmailTemplate',
  }),
  route('admin.telegram-webhook.set', {
    method: 'POST',
    path: '/{secure_path}/config/setTelegramWebhook',
  }),
  route('admin.test-mail.send', { method: 'POST', path: '/{secure_path}/config/testSendMail' }),
  route('admin.system.status', { method: 'GET', path: '/{secure_path}/system/getSystemStatus' }),
  route('admin.system.queue-stats', {
    method: 'GET',
    path: '/{secure_path}/system/getQueueStats',
  }),
  route('admin.system.queue-workload', {
    method: 'GET',
    path: '/{secure_path}/system/getQueueWorkload',
  }),
  route('admin.system.queue-masters', {
    method: 'GET',
    path: '/{secure_path}/system/getQueueMasters',
  }),
  route('admin.system.logs', { method: 'GET', path: '/{secure_path}/system/getSystemLog' }),

  // ——— §6.2 Plans, payments ———
  route('admin.plans.list', { method: 'GET', path: '/{secure_path}/plan/fetch' }),
  route('admin.plans.create', { method: 'POST', path: '/{secure_path}/plan/save' }),
  route('admin.plans.update', { method: 'POST', path: '/{secure_path}/plan/save' }),
  route('admin.plans.toggle', { method: 'POST', path: '/{secure_path}/plan/update' }),
  route('admin.plans.delete', { method: 'POST', path: '/{secure_path}/plan/drop' }),
  route('admin.plans.sort', { method: 'POST', path: '/{secure_path}/plan/sort' }),
  route('admin.payments.list', { method: 'GET', path: '/{secure_path}/payment/fetch' }),
  route('admin.payment-providers.list', {
    method: 'GET',
    path: '/{secure_path}/payment/getPaymentMethods',
  }),
  route('admin.payment-providers.form', {
    method: 'POST',
    path: '/{secure_path}/payment/getPaymentForm',
  }),
  route('admin.payments.create', { method: 'POST', path: '/{secure_path}/payment/save' }),
  route('admin.payments.update', { method: 'POST', path: '/{secure_path}/payment/save' }),
  route('admin.payments.toggle', { method: 'POST', path: '/{secure_path}/payment/show' }),
  route('admin.payments.delete', { method: 'POST', path: '/{secure_path}/payment/drop' }),
  route('admin.payments.sort', { method: 'POST', path: '/{secure_path}/payment/sort' }),

  // ——— §6.3 Content: notices, knowledge, coupons, gift cards ———
  route('admin.notices.list', { method: 'GET', path: '/{secure_path}/notice/fetch' }),
  route('admin.notices.create', { method: 'POST', path: '/{secure_path}/notice/save' }),
  route('admin.notices.update', {
    method: 'POST',
    path: '/{secure_path}/notice/save',
    aliases: ['/{secure_path}/notice/update'],
  }),
  route('admin.notices.toggle', { method: 'POST', path: '/{secure_path}/notice/show' }),
  route('admin.notices.delete', { method: 'POST', path: '/{secure_path}/notice/drop' }),
  route('admin.knowledge.list', { method: 'GET', path: '/{secure_path}/knowledge/fetch' }),
  route('admin.knowledge.get', {
    method: 'GET',
    path: '/{secure_path}/knowledge/fetch',
    query: ['id'],
  }),
  route('admin.knowledge-categories.list', {
    method: 'GET',
    path: '/{secure_path}/knowledge/getCategory',
  }),
  route('admin.knowledge.create', { method: 'POST', path: '/{secure_path}/knowledge/save' }),
  route('admin.knowledge.update', { method: 'POST', path: '/{secure_path}/knowledge/save' }),
  route('admin.knowledge.toggle', { method: 'POST', path: '/{secure_path}/knowledge/show' }),
  route('admin.knowledge.delete', { method: 'POST', path: '/{secure_path}/knowledge/drop' }),
  route('admin.knowledge.sort', { method: 'POST', path: '/{secure_path}/knowledge/sort' }),
  route('admin.coupons.list', { method: 'GET', path: '/{secure_path}/coupon/fetch' }),
  route('admin.coupons.create', { method: 'POST', path: '/{secure_path}/coupon/generate' }),
  route('admin.coupons.toggle', { method: 'POST', path: '/{secure_path}/coupon/show' }),
  route('admin.coupons.delete', { method: 'POST', path: '/{secure_path}/coupon/drop' }),
  route('admin.gift-cards.list', { method: 'GET', path: '/{secure_path}/giftcard/fetch' }),
  route('admin.gift-cards.create', { method: 'POST', path: '/{secure_path}/giftcard/generate' }),
  route('admin.gift-cards.delete', { method: 'POST', path: '/{secure_path}/giftcard/drop' }),

  // ——— §6.4 Orders & reconciliation ———
  route('admin.orders.list', { method: 'GET', path: '/{secure_path}/order/fetch' }),
  route('admin.orders.get', { method: 'POST', path: '/{secure_path}/order/detail' }),
  // The reconciliation arm rides the same legacy action behind a
  // `reconciliation_id` body param (§6.4 demultiplex); list it before the
  // plain status/commission arm so the body discriminator can win.
  route('admin.payment-reconciliations.resolve', {
    method: 'POST',
    path: '/{secure_path}/order/update',
    bodyKeys: ['reconciliation_id'],
  }),
  route('admin.orders.update', { method: 'POST', path: '/{secure_path}/order/update' }),
  route('admin.orders.mark-paid', { method: 'POST', path: '/{secure_path}/order/paid' }),
  route('admin.orders.cancel', { method: 'POST', path: '/{secure_path}/order/cancel' }),
  route('admin.orders.create', { method: 'POST', path: '/{secure_path}/order/assign' }),
  route('admin.payment-reconciliations.list', {
    method: 'GET',
    path: '/{secure_path}/order/reconciliation/fetch',
  }),

  // ——— §6.5 Tickets (admin) ———
  route('admin.tickets.list', { method: 'GET', path: '/{secure_path}/ticket/fetch' }),
  route('admin.tickets.get', {
    method: 'GET',
    path: '/{secure_path}/ticket/fetch',
    query: ['id'],
  }),
  route('admin.tickets.replies.create', { method: 'POST', path: '/{secure_path}/ticket/reply' }),
  route('admin.tickets.close', { method: 'POST', path: '/{secure_path}/ticket/close' }),

  // ——— §6.6 Users ———
  route('admin.users.list', { method: 'GET', path: '/{secure_path}/user/fetch' }),
  route('admin.users.get', { method: 'GET', path: '/{secure_path}/user/getUserInfoById' }),
  route('admin.users.update', { method: 'POST', path: '/{secure_path}/user/update' }),
  route('admin.users.set-inviter', { method: 'POST', path: '/{secure_path}/user/setInviteUser' }),
  route('admin.users.create', { method: 'POST', path: '/{secure_path}/user/generate' }),
  route('admin.users.export', { method: 'POST', path: '/{secure_path}/user/dumpCSV' }),
  route('admin.users.mail', { method: 'POST', path: '/{secure_path}/user/sendMail' }),
  route('admin.users.ban', { method: 'POST', path: '/{secure_path}/user/ban' }),
  route('admin.users.reset-secret', { method: 'POST', path: '/{secure_path}/user/resetSecret' }),
  route('admin.users.delete', { method: 'POST', path: '/{secure_path}/user/delUser' }),
  route('admin.users.bulk-delete', { method: 'POST', path: '/{secure_path}/user/allDel' }),

  // ——— §6.7 Servers (nodes, groups, routes, protocol CRUD) ———
  route('admin.nodes.list', { method: 'GET', path: '/{secure_path}/server/manage/getNodes' }),
  route('admin.nodes.sort', { method: 'POST', path: '/{secure_path}/server/manage/sort' }),
  route('admin.server-groups.list', { method: 'GET', path: '/{secure_path}/server/group/fetch' }),
  route('admin.server-groups.create', {
    method: 'POST',
    path: '/{secure_path}/server/group/save',
  }),
  route('admin.server-groups.update', {
    method: 'POST',
    path: '/{secure_path}/server/group/save',
  }),
  route('admin.server-groups.delete', {
    method: 'POST',
    path: '/{secure_path}/server/group/drop',
  }),
  route('admin.server-routes.list', { method: 'GET', path: '/{secure_path}/server/route/fetch' }),
  route('admin.server-routes.create', {
    method: 'POST',
    path: '/{secure_path}/server/route/save',
  }),
  route('admin.server-routes.update', {
    method: 'POST',
    path: '/{secure_path}/server/route/save',
  }),
  route('admin.server-routes.delete', {
    method: 'POST',
    path: '/{secure_path}/server/route/drop',
  }),
  route('admin.servers.create', {
    method: 'POST',
    path: '/{secure_path}/server/{type}/save',
    params: { type: SERVER_TYPES },
  }),
  route('admin.servers.update', {
    method: 'POST',
    path: '/{secure_path}/server/{type}/save',
    params: { type: SERVER_TYPES },
  }),
  route('admin.servers.toggle', {
    method: 'POST',
    path: '/{secure_path}/server/{type}/update',
    params: { type: SERVER_TYPES },
  }),
  route('admin.servers.delete', {
    method: 'POST',
    path: '/{secure_path}/server/{type}/drop',
    params: { type: SERVER_TYPES },
  }),
  route('admin.servers.copy', {
    method: 'POST',
    path: '/{secure_path}/server/{type}/copy',
    params: { type: SERVER_TYPES },
  }),

  // ——— §6.8 Stats ———
  route('admin.stats.summary', {
    method: 'GET',
    path: '/{secure_path}/stat/getStat',
    // §6.8: three legacy aliases collapse into one modern route.
    aliases: ['/{secure_path}/stat/getOverride', '/{secure_path}/stat/getRanking'],
  }),
  route('admin.stats.server-rank', {
    method: 'GET',
    path: '/{secure_path}/stat/getServerTodayRank',
    // The today/last split becomes the modern `?window=today|previous`.
    aliases: ['/{secure_path}/stat/getServerLastRank'],
  }),
  route('admin.stats.user-rank', {
    method: 'GET',
    path: '/{secure_path}/stat/getUserTodayRank',
    aliases: ['/{secure_path}/stat/getUserLastRank'],
  }),
  route('admin.stats.orders', { method: 'GET', path: '/{secure_path}/stat/getOrder' }),
  route('admin.stats.user-traffic', { method: 'GET', path: '/{secure_path}/stat/getStatUser' }),
  route('admin.stats.records', { method: 'GET', path: '/{secure_path}/stat/getStatRecord' }),

  // ——— §6.9 Staff namespace ———
  route('staff.tickets.list', { method: 'GET', path: '/staff/ticket/fetch' }),
  route('staff.tickets.get', { method: 'GET', path: '/staff/ticket/fetch', query: ['id'] }),
  route('staff.tickets.replies.create', { method: 'POST', path: '/staff/ticket/reply' }),
  route('staff.tickets.close', { method: 'POST', path: '/staff/ticket/close' }),
  route('staff.users.get', { method: 'GET', path: '/staff/user/getUserInfoById' }),
  route('staff.users.update', { method: 'POST', path: '/staff/user/update' }),
  route('staff.users.mail', { method: 'POST', path: '/staff/user/sendMail' }),
  route('staff.users.ban', { method: 'POST', path: '/staff/user/ban' }),
  route('staff.plans.list', { method: 'GET', path: '/staff/plan/fetch' }),
  route('staff.notices.list', { method: 'GET', path: '/staff/notice/fetch' }),
  route('staff.notices.create', { method: 'POST', path: '/staff/notice/save' }),
  route('staff.notices.update', {
    method: 'POST',
    path: '/staff/notice/save',
    aliases: ['/staff/notice/update'],
  }),
  route('staff.notices.delete', { method: 'POST', path: '/staff/notice/drop' }),
]);

const routesById = new Map(routeMap.map((entry) => [entry.id, entry]));
if (routesById.size !== routeMap.length) {
  throw new Error('route-map ids must be unique');
}

export function routeEntry(id) {
  const entry = routesById.get(id);
  if (!entry) throw new Error(`Unknown canonical route id "${id}"`);
  return entry;
}

/** The per-world URL shape for a canonical route id (§13.1). */
export function worldRoute(id, world) {
  assertWorld(world);
  const entry = routeEntry(id);
  return world === 'oracle' ? entry.legacy : entry.modern;
}

/**
 * Resolve a canonical route id to a concrete per-world request path
 * (including the `/api/v1` prefix), substituting `{secure_path}` and any
 * `{name}` path parameters.
 */
export function resolveRoutePath(id, world, { securePath, params = {}, query } = {}) {
  const shape = worldRoute(id, world);
  const path = substitutePath(shape.path, { securePath, params });
  if (!query) return `${API_PREFIX}${path}`;
  const search = new URLSearchParams(query).toString();
  return search ? `${API_PREFIX}${path}?${search}` : `${API_PREFIX}${path}`;
}

/**
 * Resolve a captured per-world request to its canonical route id. Entries
 * whose `query`/`bodyKeys` discriminators are satisfied outrank plain
 * entries; remaining ties resolve to the first entry in map order.
 */
export function matchRoute(world, { method, pathname, searchParams, body, securePath }) {
  assertWorld(world);
  const requestMethod = String(method ?? 'GET').toUpperCase();
  const path = stripApiPrefix(pathname);
  if (path === null) return null;
  const presentQueryKeys = new Set(searchParams ? [...searchParams.keys()] : []);
  const bodyFieldNames = new Set(
    body && typeof body === 'object' && !Array.isArray(body) ? Object.keys(body) : [],
  );

  let best = null;
  for (const entry of routeMap) {
    const shape = world === 'oracle' ? entry.legacy : entry.modern;
    if (shape.method !== requestMethod) continue;
    const params = matchPath(shape, path, securePath);
    if (!params) continue;
    const requiredQuery = shape.query ?? [];
    if (!requiredQuery.every((key) => presentQueryKeys.has(key))) continue;
    const requiredBodyKeys = shape.bodyKeys ?? [];
    if (!requiredBodyKeys.every((key) => bodyFieldNames.has(key))) continue;
    const specificity = requiredQuery.length + requiredBodyKeys.length;
    if (!best || specificity > best.specificity) {
      best = { id: entry.id, params, specificity };
    }
  }

  return best ? { id: best.id, params: best.params } : null;
}

function assertWorld(world) {
  if (!WORLDS.includes(world)) {
    throw new Error(`Unknown parity world "${world}" (expected ${WORLDS.join(' | ')})`);
  }
}

function stripApiPrefix(pathname) {
  const path = String(pathname ?? '');
  if (!path.startsWith(`${API_PREFIX}/`)) return null;
  return path.slice(API_PREFIX.length);
}

function substitutePath(pattern, { securePath, params }) {
  return pattern.replace(/\{([a-z_]+)\}/g, (token, name) => {
    if (name === 'secure_path') {
      if (!securePath) throw new Error(`Route pattern ${pattern} requires a securePath`);
      return securePath;
    }
    const value = params[name];
    if (value === undefined || value === null) {
      throw new Error(`Route pattern ${pattern} requires the "${name}" parameter`);
    }
    return String(value);
  });
}

function matchPath(shape, path, securePath) {
  const patterns = [shape.path, ...(shape.aliases ?? [])];
  for (const pattern of patterns) {
    const params = matchPattern(pattern, path, securePath, shape.params);
    if (params) return params;
  }
  return null;
}

function matchPattern(pattern, path, securePath, constraints = {}) {
  const patternSegments = pattern.split('/');
  const pathSegments = path.split('/');
  if (patternSegments.length !== pathSegments.length) return null;
  const params = {};
  for (let index = 0; index < patternSegments.length; index += 1) {
    const patternSegment = patternSegments[index];
    const pathSegment = pathSegments[index];
    const placeholder = /^\{([a-z_]+)\}$/.exec(patternSegment);
    if (!placeholder) {
      if (patternSegment !== pathSegment) return null;
      continue;
    }
    if (!pathSegment) return null;
    const name = placeholder[1];
    if (name === 'secure_path') {
      if (!securePath || pathSegment !== securePath) return null;
      continue;
    }
    const allowed = constraints[name];
    if (allowed && !allowed.includes(pathSegment)) return null;
    params[name] = pathSegment;
  }
  return params;
}
