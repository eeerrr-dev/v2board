import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  API_PREFIX,
  SERVER_TYPES,
  WORLDS,
  matchRoute,
  resolveRoutePath,
  routeEntry,
  routeMap,
  worldRoute,
} from './route-map.mjs';

test('route ids are unique and every entry carries both world shapes', () => {
  const ids = new Set();
  for (const entry of routeMap) {
    assert.ok(!ids.has(entry.id), `duplicate route id ${entry.id}`);
    ids.add(entry.id);
    for (const world of ['legacy', 'modern']) {
      const shape = entry[world];
      assert.match(shape.method, /^(GET|POST|PATCH|PUT|DELETE)$/, entry.id);
      assert.ok(shape.path.startsWith('/'), `${entry.id} ${world} path must start with /`);
    }
  }
});

const W3_ROUTE_IDS = Object.freeze([
  'public.config',
  'public.invite-views.create',
  'user.telegram-bot.get',
  'user.config.get',
  'user.knowledge.get',
  'user.knowledge.list',
  'user.knowledge-categories.list',
  'user.notices.list',
]);

const W5_ROUTE_IDS = Object.freeze([
  'user.profile.get',
  'user.profile.update',
  'user.password.update',
  'user.stats.get',
  'user.sessions.list',
  'user.sessions.delete',
  'user.gift-card-redemptions.create',
  'user.telegram-binding.delete',
  'user.subscription.get',
  'user.subscription.new-period',
  'user.subscription.reset-token',
]);

const W6_ROUTE_IDS = Object.freeze(['user.servers.list', 'user.traffic-logs.list']);

const W7_ROUTE_IDS = Object.freeze([
  'user.commission-transfers.create',
  'user.invite-codes.create',
  'user.invite.get',
  'user.commissions.list',
]);

const W8_ROUTE_IDS = Object.freeze([
  'user.tickets.get',
  'user.tickets.list',
  'user.tickets.create',
  'user.tickets.replies.create',
  'user.tickets.close',
  'user.withdrawal-tickets.create',
]);

const W9_ROUTE_IDS = Object.freeze([
  'admin.config.get',
  'admin.config.update',
  'admin.email-templates.list',
  'admin.telegram-webhook.set',
  'admin.test-mail.send',
  'admin.system.status',
  'admin.system.queue-stats',
  'admin.system.queue-workload',
  'admin.system.queue-masters',
  'admin.system.logs',
]);

const W4_ROUTE_IDS = Object.freeze([
  'user.plans.get',
  'user.plans.list',
  'user.orders.create',
  'user.orders.list',
  'user.orders.get',
  'user.orders.status',
  'user.orders.cancel',
  'user.orders.checkout',
  'user.orders.stripe-intent',
  'user.payment-methods.list',
  'user.coupons.check',
]);

test('unflipped families stay identity: modern equals legacy until each wave', () => {
  for (const entry of routeMap) {
    if (entry.id.startsWith('auth.')) continue; // §5.2 flipped in W2
    if (W3_ROUTE_IDS.includes(entry.id)) continue; // §5.1/§5.8 flipped in W3
    if (W4_ROUTE_IDS.includes(entry.id)) continue; // §5.5 flipped in W4
    if (W5_ROUTE_IDS.includes(entry.id)) continue; // §5.3/§5.4 flipped in W5
    if (W6_ROUTE_IDS.includes(entry.id)) continue; // §5.4 service usage flipped in W6
    if (W7_ROUTE_IDS.includes(entry.id)) continue; // §5.6 invite/commission flipped in W7
    if (W8_ROUTE_IDS.includes(entry.id)) continue; // §5.7 user tickets flipped in W8
    if (W9_ROUTE_IDS.includes(entry.id)) continue; // §6.1 admin config/system flipped in W9
    assert.deepEqual(entry.modern, entry.legacy, `${entry.id} must stay legacy→legacy until its wave`);
  }
});

test('W5: the profile/subscription family carries the modern rows', () => {
  const modern = Object.fromEntries(
    W5_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]),
  );
  assert.deepEqual(modern, {
    'user.profile.get': { method: 'GET', path: '/user/profile' },
    'user.profile.update': { method: 'PATCH', path: '/user/profile' },
    'user.password.update': { method: 'PUT', path: '/user/password' },
    'user.stats.get': { method: 'GET', path: '/user/stats' },
    'user.sessions.list': { method: 'GET', path: '/user/sessions' },
    // The legacy body-carried session_id became path identity (§9.4).
    'user.sessions.delete': { method: 'DELETE', path: '/user/sessions/{session_id}' },
    'user.gift-card-redemptions.create': {
      method: 'POST',
      path: '/user/gift-card-redemptions',
    },
    'user.telegram-binding.delete': { method: 'DELETE', path: '/user/telegram-binding' },
    'user.subscription.get': { method: 'GET', path: '/user/subscription' },
    'user.subscription.new-period': { method: 'POST', path: '/user/subscription/new-period' },
    // GET-with-side-effect became a POST rotation (§9.4).
    'user.subscription.reset-token': { method: 'POST', path: '/user/subscription/reset-token' },
  });
  // The oracle keeps requesting the legacy rows.
  assert.equal(worldRoute('user.profile.get', 'oracle').path, '/user/info');
  assert.equal(worldRoute('user.sessions.delete', 'oracle').method, 'POST');
  assert.equal(worldRoute('user.subscription.reset-token', 'oracle').path, '/user/resetSecurity');
  assert.deepEqual(
    matchRoute('source', {
      method: 'DELETE',
      pathname: `${API_PREFIX}/user/sessions/digest-abc`,
    }),
    { id: 'user.sessions.delete', params: { session_id: 'digest-abc' } },
  );
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/sessions` })?.id,
    'user.sessions.list',
  );
  assert.equal(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/user/subscription/new-period`,
    })?.id,
    'user.subscription.new-period',
  );
});

test('W6: the user service-usage family carries the modern rows', () => {
  const modern = Object.fromEntries(W6_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    'user.servers.list': { method: 'GET', path: '/user/servers' },
    'user.traffic-logs.list': { method: 'GET', path: '/user/traffic-logs' },
  });
  // The oracle keeps requesting the legacy rows.
  assert.equal(worldRoute('user.servers.list', 'oracle').path, '/user/server/fetch');
  assert.equal(worldRoute('user.traffic-logs.list', 'oracle').path, '/user/stat/getTrafficLog');
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/servers` })?.id,
    'user.servers.list',
  );
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/traffic-logs` })?.id,
    'user.traffic-logs.list',
  );
});

test('W7: the invite & commission family carries the modern rows', () => {
  const modern = Object.fromEntries(W7_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    'user.commission-transfers.create': { method: 'POST', path: '/user/commission-transfers' },
    // The legacy GET-with-side-effect became the one deliberate 204 POST
    // create (§1/§5.6).
    'user.invite-codes.create': { method: 'POST', path: '/user/invite-codes' },
    'user.invite.get': { method: 'GET', path: '/user/invite' },
    'user.commissions.list': { method: 'GET', path: '/user/commissions' },
  });
  // The oracle keeps requesting the legacy rows.
  assert.equal(worldRoute('user.commission-transfers.create', 'oracle').path, '/user/transfer');
  assert.equal(worldRoute('user.invite-codes.create', 'oracle').method, 'GET');
  assert.equal(worldRoute('user.invite-codes.create', 'oracle').path, '/user/invite/save');
  assert.equal(worldRoute('user.invite.get', 'oracle').path, '/user/invite/fetch');
  assert.equal(worldRoute('user.commissions.list', 'oracle').path, '/user/invite/details');
  assert.equal(
    matchRoute('source', { method: 'POST', pathname: `${API_PREFIX}/user/invite-codes` })?.id,
    'user.invite-codes.create',
  );
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/invite` })?.id,
    'user.invite.get',
  );
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/commissions` })?.id,
    'user.commissions.list',
  );
  assert.equal(
    matchRoute('source', { method: 'POST', pathname: `${API_PREFIX}/user/commission-transfers` })
      ?.id,
    'user.commission-transfers.create',
  );
});

test('W8: the user ticket family carries the modern rows', () => {
  const modern = Object.fromEntries(W8_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    'user.tickets.get': { method: 'GET', path: '/user/tickets/{id}' },
    'user.tickets.list': { method: 'GET', path: '/user/tickets' },
    'user.tickets.create': { method: 'POST', path: '/user/tickets' },
    // The legacy body-carried ticket id became path identity (§5.7).
    'user.tickets.replies.create': { method: 'POST', path: '/user/tickets/{id}/replies' },
    'user.tickets.close': { method: 'POST', path: '/user/tickets/{id}/close' },
    'user.withdrawal-tickets.create': { method: 'POST', path: '/user/withdrawal-tickets' },
  });
  // The oracle keeps requesting the legacy rows.
  assert.equal(worldRoute('user.tickets.get', 'oracle').path, '/user/ticket/fetch');
  assert.deepEqual(worldRoute('user.tickets.get', 'oracle').query, ['id']);
  assert.equal(worldRoute('user.tickets.list', 'oracle').path, '/user/ticket/fetch');
  assert.equal(worldRoute('user.tickets.create', 'oracle').path, '/user/ticket/save');
  assert.equal(worldRoute('user.tickets.replies.create', 'oracle').path, '/user/ticket/reply');
  assert.equal(worldRoute('user.tickets.close', 'oracle').path, '/user/ticket/close');
  assert.equal(worldRoute('user.withdrawal-tickets.create', 'oracle').path, '/user/ticket/withdraw');
  // Source-world matches extract the path id; the list row stays distinct.
  assert.deepEqual(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/tickets/7` }),
    { id: 'user.tickets.get', params: { id: '7' } },
  );
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/tickets` })?.id,
    'user.tickets.list',
  );
  assert.equal(
    matchRoute('source', { method: 'POST', pathname: `${API_PREFIX}/user/tickets` })?.id,
    'user.tickets.create',
  );
  assert.deepEqual(
    matchRoute('source', { method: 'POST', pathname: `${API_PREFIX}/user/tickets/7/replies` }),
    { id: 'user.tickets.replies.create', params: { id: '7' } },
  );
  assert.deepEqual(
    matchRoute('source', { method: 'POST', pathname: `${API_PREFIX}/user/tickets/7/close` }),
    { id: 'user.tickets.close', params: { id: '7' } },
  );
  assert.equal(
    matchRoute('source', { method: 'POST', pathname: `${API_PREFIX}/user/withdrawal-tickets` })
      ?.id,
    'user.withdrawal-tickets.create',
  );
});

test('W9: the admin config & system family carries the modern rows', () => {
  const modern = Object.fromEntries(W9_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    'admin.config.get': { method: 'GET', path: '/{secure_path}/config' },
    // §6.1: partial-update semantics become a real PATCH (202 pending /
    // 409 config_revision_conflict).
    'admin.config.update': { method: 'PATCH', path: '/{secure_path}/config' },
    'admin.email-templates.list': { method: 'GET', path: '/{secure_path}/email-templates' },
    'admin.telegram-webhook.set': { method: 'POST', path: '/{secure_path}/telegram-webhook' },
    'admin.test-mail.send': { method: 'POST', path: '/{secure_path}/test-mail' },
    'admin.system.status': { method: 'GET', path: '/{secure_path}/system/status' },
    'admin.system.queue-stats': { method: 'GET', path: '/{secure_path}/system/queue-stats' },
    'admin.system.queue-workload': {
      method: 'GET',
      path: '/{secure_path}/system/queue-workload',
    },
    'admin.system.queue-masters': { method: 'GET', path: '/{secure_path}/system/queue-masters' },
    // §7 (W9): the modern list rides the single JSON `filter` query param.
    'admin.system.logs': { method: 'GET', path: '/{secure_path}/system/logs' },
  });
  // The oracle keeps requesting the legacy rows.
  assert.equal(worldRoute('admin.config.get', 'oracle').path, '/{secure_path}/config/fetch');
  assert.equal(worldRoute('admin.config.update', 'oracle').method, 'POST');
  assert.equal(worldRoute('admin.config.update', 'oracle').path, '/{secure_path}/config/save');
  assert.equal(
    worldRoute('admin.email-templates.list', 'oracle').path,
    '/{secure_path}/config/getEmailTemplate',
  );
  assert.equal(
    worldRoute('admin.system.queue-stats', 'oracle').path,
    '/{secure_path}/system/getQueueStats',
  );
  assert.equal(
    worldRoute('admin.system.logs', 'oracle').path,
    '/{secure_path}/system/getSystemLog',
  );
  // Source-world matches resolve under the dynamic secure_path prefix, and
  // GET vs PATCH split the two /config rows.
  assert.deepEqual(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/config`,
      securePath: 'sec',
    }),
    { id: 'admin.config.get', params: {} },
  );
  assert.equal(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/config`,
      securePath: 'sec',
    })?.id,
    'admin.config.update',
  );
  assert.equal(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/system/logs`,
      securePath: 'sec',
    })?.id,
    'admin.system.logs',
  );
  assert.equal(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/test-mail`,
      securePath: 'sec',
    })?.id,
    'admin.test-mail.send',
  );
});

test('W4: the user commerce family carries the modern rows', () => {
  const modern = Object.fromEntries(
    W4_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]),
  );
  assert.deepEqual(modern, {
    // The legacy `?id=` discriminator became a real path segment (§5.5).
    'user.plans.get': { method: 'GET', path: '/user/plans/{id}' },
    'user.plans.list': { method: 'GET', path: '/user/plans' },
    'user.orders.create': { method: 'POST', path: '/user/orders' },
    'user.orders.list': { method: 'GET', path: '/user/orders' },
    // The legacy `?trade_no=` discriminators became path identity (§5.5).
    'user.orders.get': { method: 'GET', path: '/user/orders/{trade_no}' },
    'user.orders.status': { method: 'GET', path: '/user/orders/{trade_no}/status' },
    'user.orders.cancel': { method: 'POST', path: '/user/orders/{trade_no}/cancel' },
    'user.orders.checkout': { method: 'POST', path: '/user/orders/{trade_no}/checkout' },
    'user.orders.stripe-intent': {
      method: 'POST',
      path: '/user/orders/{trade_no}/stripe-intent',
    },
    'user.payment-methods.list': { method: 'GET', path: '/user/payment-methods' },
    'user.coupons.check': { method: 'POST', path: '/user/coupons/check' },
  });
  // The oracle keeps requesting the legacy rows.
  assert.equal(worldRoute('user.plans.list', 'oracle').path, '/user/plan/fetch');
  assert.equal(worldRoute('user.orders.create', 'oracle').path, '/user/order/save');
  assert.equal(worldRoute('user.payment-methods.list', 'oracle').path, '/user/order/getPaymentMethod');
  assert.deepEqual(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/plans/3` }),
    { id: 'user.plans.get', params: { id: '3' } },
  );
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/plans` })?.id,
    'user.plans.list',
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/user/orders/2026TRADE1/checkout`,
    }),
    { id: 'user.orders.checkout', params: { trade_no: '2026TRADE1' } },
  );
  assert.deepEqual(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/orders/2026TRADE1/status` }),
    { id: 'user.orders.status', params: { trade_no: '2026TRADE1' } },
  );
});

test('W3: the public/content family carries the modern rows', () => {
  const modern = Object.fromEntries(
    W3_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]),
  );
  assert.deepEqual(modern, {
    'public.config': { method: 'GET', path: '/public/config' },
    'public.invite-views.create': { method: 'POST', path: '/public/invite-views' },
    'user.telegram-bot.get': { method: 'GET', path: '/user/telegram-bot' },
    'user.config.get': { method: 'GET', path: '/user/config' },
    // The legacy `?id=` discriminator became a real path segment (§5.8).
    'user.knowledge.get': { method: 'GET', path: '/user/knowledge/{id}' },
    'user.knowledge.list': { method: 'GET', path: '/user/knowledge' },
    'user.knowledge-categories.list': { method: 'GET', path: '/user/knowledge-categories' },
    // The legacy single-notice `?id=` branch is dropped (§5.8 recorded
    // decision): the modern route is list-only.
    'user.notices.list': { method: 'GET', path: '/user/notices' },
  });
  // The oracle keeps requesting the legacy rows.
  assert.equal(worldRoute('public.config', 'oracle').path, '/guest/comm/config');
  assert.equal(worldRoute('user.notices.list', 'oracle').path, '/user/notice/fetch');
  assert.deepEqual(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/knowledge/7` }),
    { id: 'user.knowledge.get', params: { id: '7' } },
  );
  assert.equal(
    matchRoute('source', { method: 'GET', pathname: `${API_PREFIX}/user/knowledge` })?.id,
    'user.knowledge.list',
  );
});

test('W2: the §5.2 auth family carries the modern /auth/* rows', () => {
  const modern = Object.fromEntries(
    routeMap
      .filter((entry) => entry.id.startsWith('auth.'))
      .map((entry) => [entry.id, entry.modern]),
  );
  assert.deepEqual(modern, {
    'auth.register': { method: 'POST', path: '/auth/register' },
    'auth.login': { method: 'POST', path: '/auth/login' },
    'auth.quick-login': { method: 'GET', path: '/auth/quick-login', query: ['token'] },
    // GET-with-side-effect became the POST body exchange (§5.2).
    'auth.token-login': { method: 'POST', path: '/auth/token-login' },
    'auth.password-reset': { method: 'POST', path: '/auth/password-reset' },
    'auth.step-up': { method: 'POST', path: '/auth/step-up' },
    'auth.quick-login-url': { method: 'POST', path: '/auth/quick-login-url' },
    'auth.email-codes': { method: 'POST', path: '/auth/email-codes' },
    'auth.session.get': { method: 'GET', path: '/auth/session' },
    'auth.session.delete': { method: 'DELETE', path: '/auth/session' },
  });
  // The oracle keeps requesting the legacy passport/user rows.
  assert.equal(worldRoute('auth.login', 'oracle').path, '/passport/auth/login');
  assert.equal(worldRoute('auth.login', 'source').path, '/auth/login');
  assert.equal(worldRoute('auth.session.get', 'oracle').path, '/user/checkLogin');
  assert.equal(worldRoute('auth.session.delete', 'source').method, 'DELETE');
});

test('worldRoute resolves the per-world shape for both worlds', () => {
  assert.deepEqual(WORLDS, ['oracle', 'source']);
  assert.equal(worldRoute('user.profile.get', 'oracle').path, '/user/info');
  assert.equal(worldRoute('user.profile.get', 'source').path, '/user/profile');
  assert.throws(() => worldRoute('user.profile.get', 'staging'), /Unknown parity world/);
  assert.throws(() => routeEntry('user.unknown'), /Unknown canonical route id/);
});

test('resolveRoutePath substitutes secure_path and path parameters', () => {
  assert.equal(
    resolveRoutePath('admin.config.get', 'oracle', { securePath: 'sec-path' }),
    `${API_PREFIX}/sec-path/config/fetch`,
  );
  assert.equal(
    resolveRoutePath('admin.servers.toggle', 'source', {
      securePath: 'admin',
      params: { type: 'vmess' },
    }),
    `${API_PREFIX}/admin/server/vmess/update`,
  );
  assert.equal(
    resolveRoutePath('user.plans.get', 'oracle', { query: { id: 2 } }),
    `${API_PREFIX}/user/plan/fetch?id=2`,
  );
  assert.throws(
    () => resolveRoutePath('admin.config.get', 'oracle', {}),
    /requires a securePath/,
  );
  assert.throws(
    () => resolveRoutePath('admin.servers.delete', 'oracle', { securePath: 'admin' }),
    /requires the "type" parameter/,
  );
});

test('matchRoute resolves plain, aliased, and parameterized legacy paths', () => {
  assert.deepEqual(
    matchRoute('oracle', { method: 'GET', pathname: `${API_PREFIX}/user/info` }),
    { id: 'user.profile.get', params: {} },
  );
  // §6.8: legacy stat aliases collapse into one canonical id.
  for (const action of ['getStat', 'getOverride', 'getRanking']) {
    assert.equal(
      matchRoute('oracle', {
        method: 'GET',
        pathname: `${API_PREFIX}/sec/stat/${action}`,
        securePath: 'sec',
      })?.id,
      'admin.stats.summary',
      action,
    );
  }
  // §6.7: {type} is constrained to the protocol vocabulary, so group/route
  // saves never fall into the protocol CRUD entries.
  assert.deepEqual(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/server/vless/update`,
      securePath: 'sec',
    }),
    { id: 'admin.servers.toggle', params: { type: 'vless' } },
  );
  assert.equal(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/server/group/save`,
      securePath: 'sec',
    })?.id,
    'admin.server-groups.create',
  );
  assert.ok(!SERVER_TYPES.includes('group'));
  // Admin paths never match without the live secure path.
  assert.equal(
    matchRoute('oracle', {
      method: 'GET',
      pathname: `${API_PREFIX}/other/config/fetch`,
      securePath: 'sec',
    }),
    null,
  );
  // Non-API paths never match.
  assert.equal(matchRoute('oracle', { method: 'GET', pathname: '/assets/user/app.js' }), null);
});

test('matchRoute prefers query- and body-discriminated entries', () => {
  const url = new URL(`http://fixture${API_PREFIX}/user/plan/fetch?id=2`);
  assert.equal(
    matchRoute('oracle', {
      method: 'GET',
      pathname: url.pathname,
      searchParams: url.searchParams,
    })?.id,
    'user.plans.get',
  );
  assert.equal(
    matchRoute('oracle', { method: 'GET', pathname: `${API_PREFIX}/user/plan/fetch` })?.id,
    'user.plans.list',
  );
  // §6.4: the reconciliation arm rides order/update behind a body param.
  assert.equal(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/order/update`,
      securePath: 'sec',
      body: { reconciliation_id: 7, resolution: 'confirm' },
    })?.id,
    'admin.payment-reconciliations.resolve',
  );
  assert.equal(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/order/update`,
      securePath: 'sec',
      body: { trade_no: 'T1', status: 1 },
    })?.id,
    'admin.orders.update',
  );
});
