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

test('unflipped families stay identity: modern equals legacy until each wave', () => {
  for (const entry of routeMap) {
    if (entry.id.startsWith('auth.')) continue; // §5.2 flipped in W2
    if (W3_ROUTE_IDS.includes(entry.id)) continue; // §5.1/§5.8 flipped in W3
    assert.deepEqual(entry.modern, entry.legacy, `${entry.id} must stay legacy→legacy until its wave`);
  }
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
  for (const world of WORLDS) {
    assert.equal(worldRoute('user.profile.get', world).path, '/user/info');
  }
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
