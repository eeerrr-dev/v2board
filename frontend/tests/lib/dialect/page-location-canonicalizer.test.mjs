import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  ROUTING_DIALECTS,
  canonicalizeLocation,
  canonicalizeLocationForDialect,
  entryUrlFor,
  entryUrlForDialect,
  routingDialectFor,
  spaLocationHelpersBootstrap,
  worldRoutingDialect,
} from './page-location-canonicalizer.mjs';

test('W1: the source world is history-routed while the oracle stays hash-routed', () => {
  assert.deepEqual(ROUTING_DIALECTS, ['hash', 'path']);
  assert.equal(worldRoutingDialect.oracle, 'hash');
  assert.equal(worldRoutingDialect.source, 'path');
  assert.throws(() => routingDialectFor('staging'), /Unknown parity world/);
});

test('entryUrlFor maps a canonical route path to the per-world entry URL (§13.4a)', () => {
  // The oracle stays hash-routed forever.
  assert.equal(entryUrlFor('/order/T1?cashier=1', 'oracle'), '/#/order/T1?cashier=1');
  assert.equal(entryUrlFor('/', 'oracle'), '/#/');
  assert.equal(entryUrlFor('login', 'oracle'), '/#/login');
  // The source world is path-routed since W1 (§10.1).
  assert.equal(entryUrlFor('/order/T1?cashier=1', 'source'), '/order/T1?cashier=1');
  assert.equal(entryUrlFor('/', 'source'), '/');
  // Admin worlds mount under their /{admin_path} base.
  assert.equal(entryUrlFor('/plan', 'oracle', '/admin-path'), '/admin-path#/plan');
  assert.equal(entryUrlFor('/plan', 'source', '/admin-path'), '/admin-path/plan');
  assert.equal(entryUrlForDialect('/order/T1?cashier=1', 'path'), '/order/T1?cashier=1');
  assert.throws(() => entryUrlForDialect('/login', 'query'), /Unknown routing dialect/);
});

test('hash and path location reads canonicalize to one route object (§13.4b)', () => {
  // The same SPA location observed in the hash-routed oracle…
  const hashRead = canonicalizeLocation('oracle', {
    pathname: '/',
    search: '',
    hash: '#/order/T1?cashier=1&cashier=2',
  });
  // …and in the history-routed source world reduce to one canonical
  // route object.
  const pathRead = canonicalizeLocation('source', {
    pathname: '/order/T1',
    search: '?cashier=1&cashier=2',
    hash: '',
  });
  assert.deepEqual(hashRead, {
    path: '/order/T1',
    query: { cashier: ['1', '2'] },
  });
  assert.deepEqual(pathRead, hashRead);
  // Admin path reads strip the /{admin_path} mount.
  assert.deepEqual(
    canonicalizeLocation('source', { pathname: '/admin-path/user', search: '', hash: '' }, '/admin-path'),
    { path: '/user', query: {} },
  );
});

test('canonicalizeLocation accepts href strings and empty hashes', () => {
  assert.deepEqual(
    canonicalizeLocation('oracle', 'http://app.local/#/dashboard?tab=usage'),
    { path: '/dashboard', query: { tab: 'usage' } },
  );
  assert.deepEqual(canonicalizeLocation('oracle', { pathname: '/', search: '', hash: '' }), {
    path: '/',
    query: {},
  });
  assert.deepEqual(canonicalizeLocationForDialect('hash', 'http://app.local/#/login?verify=abc'), {
    path: '/login',
    query: { verify: 'abc' },
  });
});

test('the browser-side helpers read and navigate world-agnostically', () => {
  const events = [];
  const makeWindow = (location) => ({
    location,
    history: {
      pushState: (_state, _title, url) => {
        const parsed = new URL(url, 'http://app.local');
        location.pathname = parsed.pathname;
        location.search = parsed.search;
      },
    },
    dispatchEvent: (event) => events.push(event.type),
  });

  // Hash dialect (oracle).
  const hashWindow = makeWindow({ pathname: '/', search: '', hash: '#/order?page=2' });
  globalThis.window = hashWindow;
  globalThis.PopStateEvent = class PopStateEvent {
    constructor(type) {
      this.type = type;
    }
  };
  try {
    spaLocationHelpersBootstrap({ dialect: 'hash', basePath: '' });
    assert.equal(hashWindow.__parityReadSpaRoute(), '/order?page=2');
    hashWindow.__paritySpaNavigate('/ticket');
    assert.equal(hashWindow.location.hash, '#/ticket');

    // Path dialect (source) with an admin base.
    const pathWindow = makeWindow({ pathname: '/admin-path/user', search: '?page=2', hash: '' });
    globalThis.window = pathWindow;
    spaLocationHelpersBootstrap({ dialect: 'path', basePath: '/admin-path' });
    assert.equal(pathWindow.__parityReadSpaRoute(), '/user?page=2');
    pathWindow.__paritySpaNavigate('/notice');
    assert.equal(pathWindow.location.pathname, '/admin-path/notice');
    assert.deepEqual(events, ['popstate']);
  } finally {
    delete globalThis.window;
    delete globalThis.PopStateEvent;
  }
});
