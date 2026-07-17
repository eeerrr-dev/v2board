import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  ROUTING_DIALECTS,
  canonicalizeLocation,
  canonicalizeLocationForDialect,
  entryUrlFor,
  entryUrlForDialect,
  routingDialectFor,
  worldRoutingDialect,
} from './page-location-canonicalizer.mjs';

test('W0: both worlds are still hash-routed (W1 flips source to path)', () => {
  assert.deepEqual(ROUTING_DIALECTS, ['hash', 'path']);
  assert.equal(worldRoutingDialect.oracle, 'hash');
  assert.equal(worldRoutingDialect.source, 'hash');
  assert.throws(() => routingDialectFor('staging'), /Unknown parity world/);
});

test('entryUrlFor maps a canonical route path to the per-world entry URL (§13.4a)', () => {
  // Hash-routed worlds (the oracle forever; the source world until W1).
  assert.equal(entryUrlFor('/order/T1?cashier=1', 'oracle'), '/#/order/T1?cashier=1');
  assert.equal(entryUrlFor('/order/T1?cashier=1', 'source'), '/#/order/T1?cashier=1');
  assert.equal(entryUrlFor('/', 'oracle'), '/#/');
  assert.equal(entryUrlFor('login', 'oracle'), '/#/login');
  // The path dialect the source world adopts at W1 (§10.1).
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
  // …and in a history-routed (W1 source) world reduce to one canonical
  // route object.
  const pathRead = canonicalizeLocationForDialect('path', {
    pathname: '/order/T1',
    search: '?cashier=1&cashier=2',
    hash: '',
  });
  assert.deepEqual(hashRead, {
    path: '/order/T1',
    query: { cashier: ['1', '2'] },
  });
  assert.deepEqual(pathRead, hashRead);
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
  assert.deepEqual(canonicalizeLocation('source', 'http://app.local/#/login?verify=abc'), {
    path: '/login',
    query: { verify: 'abc' },
  });
});
