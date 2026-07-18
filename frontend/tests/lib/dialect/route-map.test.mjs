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

const W10_ROUTE_IDS = Object.freeze([
  'admin.notices.list',
  'admin.notices.create',
  'admin.notices.update',
  'admin.notices.toggle',
  'admin.notices.delete',
  'admin.knowledge.list',
  'admin.knowledge.get',
  'admin.knowledge-categories.list',
  'admin.knowledge.create',
  'admin.knowledge.update',
  'admin.knowledge.toggle',
  'admin.knowledge.delete',
  'admin.knowledge.sort',
  'admin.coupons.list',
  'admin.coupons.create',
  'admin.coupons.update',
  'admin.coupons.toggle',
  'admin.coupons.delete',
  'admin.gift-cards.list',
  'admin.gift-cards.create',
  'admin.gift-cards.update',
  'admin.gift-cards.delete',
]);

const W11_ROUTE_IDS = Object.freeze([
  'admin.plans.list',
  'admin.plans.create',
  'admin.plans.update',
  'admin.plans.toggle',
  'admin.plans.delete',
  'admin.plans.sort',
  'admin.payments.list',
  'admin.payment-providers.list',
  'admin.payment-providers.form',
  'admin.payments.create',
  'admin.payments.update',
  'admin.payments.toggle',
  'admin.payments.delete',
  'admin.payments.sort',
  'admin.orders.list',
  'admin.orders.get',
  'admin.payment-reconciliations.resolve',
  'admin.orders.update',
  'admin.orders.mark-paid',
  'admin.orders.cancel',
  'admin.orders.create',
  'admin.payment-reconciliations.list',
]);

const W12_ROUTE_IDS = Object.freeze([
  'admin.users.list',
  'admin.users.get',
  'admin.users.update',
  'admin.users.set-inviter',
  'admin.users.create',
  'admin.users.export',
  'admin.users.mail',
  'admin.users.ban',
  'admin.users.reset-secret',
  'admin.users.delete',
  'admin.users.bulk-delete',
]);

const W13_ROUTE_IDS = Object.freeze([
  'admin.nodes.list',
  'admin.nodes.sort',
  'admin.server-groups.list',
  'admin.server-groups.create',
  'admin.server-groups.update',
  'admin.server-groups.delete',
  'admin.server-routes.list',
  'admin.server-routes.create',
  'admin.server-routes.update',
  'admin.server-routes.delete',
  'admin.servers.create',
  'admin.servers.update',
  'admin.servers.toggle',
  'admin.servers.delete',
  'admin.servers.copy',
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
    if (W10_ROUTE_IDS.includes(entry.id)) continue; // §6.3 admin content flipped in W10
    if (W11_ROUTE_IDS.includes(entry.id)) continue; // §6.2/§6.4 admin commerce flipped in W11
    if (W12_ROUTE_IDS.includes(entry.id)) continue; // §6.6 admin users flipped in W12
    if (W13_ROUTE_IDS.includes(entry.id)) continue; // §6.7 admin servers flipped in W13
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

test('W10: the admin content family carries the modern rows', () => {
  const modern = Object.fromEntries(W10_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    // §6.3: deliberately unpaginated bare array.
    'admin.notices.list': { method: 'GET', path: '/{secure_path}/notices' },
    'admin.notices.create': { method: 'POST', path: '/{secure_path}/notices' },
    'admin.notices.update': { method: 'PATCH', path: '/{secure_path}/notices/{id}' },
    // The legacy server-side flip became the explicit `{show}` PATCH body.
    'admin.notices.toggle': {
      method: 'PATCH',
      path: '/{secure_path}/notices/{id}',
      bodyKeys: ['show'],
    },
    'admin.notices.delete': { method: 'DELETE', path: '/{secure_path}/notices/{id}' },
    'admin.knowledge.list': { method: 'GET', path: '/{secure_path}/knowledge' },
    // The legacy `?id=` discriminator became a real path segment (§6.3).
    'admin.knowledge.get': { method: 'GET', path: '/{secure_path}/knowledge/{id}' },
    'admin.knowledge-categories.list': {
      method: 'GET',
      path: '/{secure_path}/knowledge-categories',
    },
    'admin.knowledge.create': { method: 'POST', path: '/{secure_path}/knowledge' },
    'admin.knowledge.update': { method: 'PATCH', path: '/{secure_path}/knowledge/{id}' },
    'admin.knowledge.toggle': {
      method: 'PATCH',
      path: '/{secure_path}/knowledge/{id}',
      bodyKeys: ['show'],
    },
    'admin.knowledge.delete': { method: 'DELETE', path: '/{secure_path}/knowledge/{id}' },
    'admin.knowledge.sort': { method: 'POST', path: '/{secure_path}/knowledge/sort' },
    'admin.coupons.list': { method: 'GET', path: '/{secure_path}/coupons' },
    'admin.coupons.create': { method: 'POST', path: '/{secure_path}/coupons' },
    'admin.coupons.update': { method: 'PATCH', path: '/{secure_path}/coupons/{id}' },
    'admin.coupons.toggle': {
      method: 'PATCH',
      path: '/{secure_path}/coupons/{id}',
      bodyKeys: ['show'],
    },
    'admin.coupons.delete': { method: 'DELETE', path: '/{secure_path}/coupons/{id}' },
    'admin.gift-cards.list': { method: 'GET', path: '/{secure_path}/gift-cards' },
    'admin.gift-cards.create': { method: 'POST', path: '/{secure_path}/gift-cards' },
    'admin.gift-cards.update': { method: 'PATCH', path: '/{secure_path}/gift-cards/{id}' },
    'admin.gift-cards.delete': { method: 'DELETE', path: '/{secure_path}/gift-cards/{id}' },
  });
  // The oracle keeps requesting the legacy rows, including the
  // body-discriminated generate/save upserts.
  assert.equal(worldRoute('admin.notices.list', 'oracle').path, '/{secure_path}/notice/fetch');
  assert.deepEqual(worldRoute('admin.notices.update', 'oracle').bodyKeys, ['id']);
  assert.deepEqual(worldRoute('admin.coupons.update', 'oracle').path, '/{secure_path}/coupon/generate');
  assert.equal(
    worldRoute('admin.knowledge-categories.list', 'oracle').path,
    '/{secure_path}/knowledge/getCategory',
  );
  // Oracle-world upsert bodies discriminate create vs update on the row id.
  assert.equal(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/coupon/generate`,
      securePath: 'sec',
      body: { name: 'New', type: 1 },
    })?.id,
    'admin.coupons.create',
  );
  assert.equal(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/coupon/generate`,
      securePath: 'sec',
      body: { id: 5, name: 'Edited' },
    })?.id,
    'admin.coupons.update',
  );
  // Source-world PATCHes discriminate the `{show}` toggle on the body key.
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/notices/3`,
      securePath: 'sec',
      body: { show: true },
    }),
    { id: 'admin.notices.toggle', params: { id: '3' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/notices/3`,
      securePath: 'sec',
      body: { title: 'Edited', content: 'Body' },
    }),
    { id: 'admin.notices.update', params: { id: '3' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'DELETE',
      pathname: `${API_PREFIX}/sec/gift-cards/9`,
      securePath: 'sec',
    }),
    { id: 'admin.gift-cards.delete', params: { id: '9' } },
  );
  // The modern knowledge detail path outranks nothing else: list and
  // categories stay distinct rows.
  assert.deepEqual(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/knowledge/7`,
      securePath: 'sec',
    }),
    { id: 'admin.knowledge.get', params: { id: '7' } },
  );
  assert.equal(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/knowledge`,
      securePath: 'sec',
    })?.id,
    'admin.knowledge.list',
  );
  assert.equal(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/knowledge-categories`,
      securePath: 'sec',
    })?.id,
    'admin.knowledge-categories.list',
  );
});

test('W11: the admin commerce family carries the modern rows', () => {
  const modern = Object.fromEntries(W11_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    // §6.2: bare unpaginated array, prices stay cents.
    'admin.plans.list': { method: 'GET', path: '/{secure_path}/plans' },
    'admin.plans.create': { method: 'POST', path: '/{secure_path}/plans' },
    'admin.plans.update': { method: 'PATCH', path: '/{secure_path}/plans/{id}' },
    // §6.2: the show/renew toggle merges into PATCH (no fixed body key — the
    // flag varies per toggle, so the row stays undiscriminated).
    'admin.plans.toggle': { method: 'PATCH', path: '/{secure_path}/plans/{id}' },
    'admin.plans.delete': { method: 'DELETE', path: '/{secure_path}/plans/{id}' },
    'admin.plans.sort': { method: 'POST', path: '/{secure_path}/plans/sort' },
    'admin.payments.list': { method: 'GET', path: '/{secure_path}/payments' },
    'admin.payment-providers.list': { method: 'GET', path: '/{secure_path}/payment-providers' },
    // §6.2: the provider-form read moved to GET, provider code in the path.
    'admin.payment-providers.form': {
      method: 'GET',
      path: '/{secure_path}/payment-providers/{code}/form',
    },
    'admin.payments.create': { method: 'POST', path: '/{secure_path}/payments' },
    'admin.payments.update': { method: 'PATCH', path: '/{secure_path}/payments/{id}' },
    'admin.payments.toggle': {
      method: 'PATCH',
      path: '/{secure_path}/payments/{id}',
      bodyKeys: ['enable'],
    },
    'admin.payments.delete': { method: 'DELETE', path: '/{secure_path}/payments/{id}' },
    'admin.payments.sort': { method: 'POST', path: '/{secure_path}/payments/sort' },
    // §8 pagination + the §7 DSL; `?is_commission=` → `?commission_only=`.
    'admin.orders.list': { method: 'GET', path: '/{secure_path}/orders' },
    // §6.4: the identifier standardizes on trade_no and the read moves to GET.
    'admin.orders.get': { method: 'GET', path: '/{secure_path}/orders/{trade_no}' },
    // §6.4: the reconciliation arm splits out onto its own resolve route.
    'admin.payment-reconciliations.resolve': {
      method: 'POST',
      path: '/{secure_path}/payment-reconciliations/{id}/resolve',
    },
    // §6.4: exactly one of {status, commission_status}; trade_no in the path.
    'admin.orders.update': { method: 'PATCH', path: '/{secure_path}/orders/{trade_no}' },
    'admin.orders.mark-paid': { method: 'POST', path: '/{secure_path}/orders/{trade_no}/mark-paid' },
    'admin.orders.cancel': { method: 'POST', path: '/{secure_path}/orders/{trade_no}/cancel' },
    // §6.4: creates an assigned order returning 201 `{trade_no}`.
    'admin.orders.create': { method: 'POST', path: '/{secure_path}/orders' },
    'admin.payment-reconciliations.list': {
      method: 'GET',
      path: '/{secure_path}/payment-reconciliations',
    },
  });
  // The oracle keeps requesting the legacy rows, including the body-carried
  // id upserts and the reconciliation demultiplex on order/update.
  assert.equal(worldRoute('admin.plans.list', 'oracle').path, '/{secure_path}/plan/fetch');
  assert.deepEqual(worldRoute('admin.plans.update', 'oracle').bodyKeys, ['id']);
  assert.equal(worldRoute('admin.plans.toggle', 'oracle').path, '/{secure_path}/plan/update');
  assert.equal(worldRoute('admin.orders.get', 'oracle').path, '/{secure_path}/order/detail');
  assert.equal(
    worldRoute('admin.payment-providers.form', 'oracle').path,
    '/{secure_path}/payment/getPaymentForm',
  );
  assert.deepEqual(worldRoute('admin.payment-reconciliations.resolve', 'oracle').bodyKeys, [
    'reconciliation_id',
  ]);
  // Oracle-world upsert bodies discriminate create vs update on the row id.
  assert.equal(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/plan/save`,
      securePath: 'sec',
      body: { name: 'New', content: 'x' },
    })?.id,
    'admin.plans.create',
  );
  assert.equal(
    matchRoute('oracle', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/plan/save`,
      securePath: 'sec',
      body: { id: 5, name: 'Edited' },
    })?.id,
    'admin.plans.update',
  );
  // Source-world matches extract the path id / trade_no.
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/plans/5`,
      securePath: 'sec',
      body: { name: 'Edited' },
    }),
    { id: 'admin.plans.update', params: { id: '5' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/payments/2`,
      securePath: 'sec',
      body: { enable: false },
    }),
    { id: 'admin.payments.toggle', params: { id: '2' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/orders/2026TRADE1`,
      securePath: 'sec',
    }),
    { id: 'admin.orders.get', params: { trade_no: '2026TRADE1' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/orders/2026TRADE1`,
      securePath: 'sec',
      body: { status: 3 },
    }),
    { id: 'admin.orders.update', params: { trade_no: '2026TRADE1' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/payment-reconciliations/7/resolve`,
      securePath: 'sec',
      body: { resolution: 'confirm' },
    }),
    { id: 'admin.payment-reconciliations.resolve', params: { id: '7' } },
  );
  assert.equal(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/payment-providers`,
      securePath: 'sec',
    })?.id,
    'admin.payment-providers.list',
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/payment-providers/StripeCheckout/form`,
      securePath: 'sec',
    }),
    { id: 'admin.payment-providers.form', params: { code: 'StripeCheckout' } },
  );
  assert.equal(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/orders`,
      securePath: 'sec',
    })?.id,
    'admin.orders.create',
  );
});

test('W12: the admin users family carries the modern rows', () => {
  const modern = Object.fromEntries(W12_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    // §8 pagination + the §7 DSL over the guarded user column whitelist.
    'admin.users.list': { method: 'GET', path: '/{secure_path}/users' },
    // §6.6: the identifier moves from the `?id=` query to the path.
    'admin.users.get': { method: 'GET', path: '/{secure_path}/users/{id}' },
    'admin.users.update': { method: 'PATCH', path: '/{secure_path}/users/{id}' },
    'admin.users.set-inviter': { method: 'POST', path: '/{secure_path}/users/{id}/set-inviter' },
    'admin.users.create': { method: 'POST', path: '/{secure_path}/users' },
    'admin.users.export': { method: 'POST', path: '/{secure_path}/users/export' },
    'admin.users.mail': { method: 'POST', path: '/{secure_path}/users/mail' },
    'admin.users.ban': { method: 'POST', path: '/{secure_path}/users/ban' },
    'admin.users.reset-secret': { method: 'POST', path: '/{secure_path}/users/{id}/reset-secret' },
    'admin.users.delete': { method: 'DELETE', path: '/{secure_path}/users/{id}' },
    'admin.users.bulk-delete': { method: 'POST', path: '/{secure_path}/users/bulk-delete' },
  });
  // The oracle keeps requesting the legacy user rows.
  assert.equal(worldRoute('admin.users.list', 'oracle').path, '/{secure_path}/user/fetch');
  assert.equal(worldRoute('admin.users.get', 'oracle').path, '/{secure_path}/user/getUserInfoById');
  assert.equal(worldRoute('admin.users.update', 'oracle').method, 'POST');
  assert.equal(worldRoute('admin.users.delete', 'oracle').path, '/{secure_path}/user/delUser');
  assert.equal(worldRoute('admin.users.bulk-delete', 'oracle').path, '/{secure_path}/user/allDel');
  // Source-world matches extract the path id.
  assert.deepEqual(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/users/5`,
      securePath: 'sec',
    }),
    { id: 'admin.users.get', params: { id: '5' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/users/5`,
      securePath: 'sec',
      body: { email: 'x@y.test' },
    }),
    { id: 'admin.users.update', params: { id: '5' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'DELETE',
      pathname: `${API_PREFIX}/sec/users/5`,
      securePath: 'sec',
    }),
    { id: 'admin.users.delete', params: { id: '5' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/users/5/set-inviter`,
      securePath: 'sec',
    }),
    { id: 'admin.users.set-inviter', params: { id: '5' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/users/5/reset-secret`,
      securePath: 'sec',
    }),
    { id: 'admin.users.reset-secret', params: { id: '5' } },
  );
  assert.equal(
    matchRoute('source', {
      method: 'GET',
      pathname: `${API_PREFIX}/sec/users`,
      securePath: 'sec',
    })?.id,
    'admin.users.list',
  );
  assert.equal(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/users`,
      securePath: 'sec',
    })?.id,
    'admin.users.create',
  );
  assert.equal(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/users/export`,
      securePath: 'sec',
    })?.id,
    'admin.users.export',
  );
});

test('W13: the admin servers family carries the modern rows', () => {
  const modern = Object.fromEntries(W13_ROUTE_IDS.map((id) => [id, routeEntry(id).modern]));
  assert.deepEqual(modern, {
    'admin.nodes.list': { method: 'GET', path: '/{secure_path}/nodes' },
    // §6.7: the grouped `{type: {id: sort}}` JSON body is kept verbatim.
    'admin.nodes.sort': { method: 'POST', path: '/{secure_path}/nodes/sort' },
    'admin.server-groups.list': { method: 'GET', path: '/{secure_path}/server-groups' },
    'admin.server-groups.create': { method: 'POST', path: '/{secure_path}/server-groups' },
    'admin.server-groups.update': { method: 'PATCH', path: '/{secure_path}/server-groups/{id}' },
    'admin.server-groups.delete': { method: 'DELETE', path: '/{secure_path}/server-groups/{id}' },
    'admin.server-routes.list': { method: 'GET', path: '/{secure_path}/server-routes' },
    'admin.server-routes.create': { method: 'POST', path: '/{secure_path}/server-routes' },
    'admin.server-routes.update': { method: 'PATCH', path: '/{secure_path}/server-routes/{id}' },
    'admin.server-routes.delete': { method: 'DELETE', path: '/{secure_path}/server-routes/{id}' },
    'admin.servers.create': {
      method: 'POST',
      path: '/{secure_path}/servers/{type}',
      params: { type: SERVER_TYPES },
    },
    // §6.7: the full edit-save PATCH is discriminated from the single-key
    // {show} toggle PATCH by the always-present `name` body key.
    'admin.servers.update': {
      method: 'PATCH',
      path: '/{secure_path}/servers/{type}/{id}',
      params: { type: SERVER_TYPES },
      bodyKeys: ['name'],
    },
    'admin.servers.toggle': {
      method: 'PATCH',
      path: '/{secure_path}/servers/{type}/{id}',
      params: { type: SERVER_TYPES },
    },
    'admin.servers.delete': {
      method: 'DELETE',
      path: '/{secure_path}/servers/{type}/{id}',
      params: { type: SERVER_TYPES },
    },
    'admin.servers.copy': {
      method: 'POST',
      path: '/{secure_path}/servers/{type}/{id}/copy',
      params: { type: SERVER_TYPES },
    },
  });
  // The oracle keeps requesting the legacy server rows.
  assert.equal(worldRoute('admin.nodes.list', 'oracle').path, '/{secure_path}/server/manage/getNodes');
  assert.equal(worldRoute('admin.nodes.sort', 'oracle').path, '/{secure_path}/server/manage/sort');
  assert.equal(worldRoute('admin.server-groups.update', 'oracle').method, 'POST');
  assert.equal(worldRoute('admin.servers.delete', 'oracle').path, '/{secure_path}/server/{type}/drop');
  // Source-world matches extract the protocol and path id.
  assert.deepEqual(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/servers/vless`,
      securePath: 'sec',
      body: { name: 'Node', group_id: [1] },
    }),
    { id: 'admin.servers.create', params: { type: 'vless' } },
  );
  // A full edit body (with `name`) is the update; the bare {show} body is the
  // merged toggle.
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/servers/vless/5`,
      securePath: 'sec',
      body: { name: 'Node', group_id: [1], show: true },
    }),
    { id: 'admin.servers.update', params: { type: 'vless', id: '5' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/servers/vless/5`,
      securePath: 'sec',
      body: { show: false },
    }),
    { id: 'admin.servers.toggle', params: { type: 'vless', id: '5' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/servers/tuic/7/copy`,
      securePath: 'sec',
    }),
    { id: 'admin.servers.copy', params: { type: 'tuic', id: '7' } },
  );
  // The protocol vocabulary still guards the modern rows: /servers/groups is
  // not a protocol CRUD path.
  assert.equal(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/servers/group`,
      securePath: 'sec',
    }),
    null,
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'PATCH',
      pathname: `${API_PREFIX}/sec/server-groups/3`,
      securePath: 'sec',
      body: { name: 'Edited' },
    }),
    { id: 'admin.server-groups.update', params: { id: '3' } },
  );
  assert.deepEqual(
    matchRoute('source', {
      method: 'DELETE',
      pathname: `${API_PREFIX}/sec/server-routes/2`,
      securePath: 'sec',
    }),
    { id: 'admin.server-routes.delete', params: { id: '2' } },
  );
  assert.equal(
    matchRoute('source', {
      method: 'POST',
      pathname: `${API_PREFIX}/sec/nodes/sort`,
      securePath: 'sec',
    })?.id,
    'admin.nodes.sort',
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
    resolveRoutePath('admin.servers.toggle', 'oracle', {
      securePath: 'admin',
      params: { type: 'vmess' },
    }),
    `${API_PREFIX}/admin/server/vmess/update`,
  );
  // §6.7 (W13): the source world resolves the merged PATCH row.
  assert.equal(
    resolveRoutePath('admin.servers.toggle', 'source', {
      securePath: 'admin',
      params: { type: 'vmess', id: 8 },
    }),
    `${API_PREFIX}/admin/servers/vmess/8`,
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
