import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  canonicalizeFilterClauses,
  canonicalizeQueryParams,
  canonicalizeRequest,
  decodeRequestBody,
  foldBracketEntries,
} from './request-canonicalizer.mjs';

test('form-encoded and JSON bodies decode to one canonical object (§13.2)', () => {
  // The same coupon-generate mutation captured in both worlds: the oracle
  // sends the legacy form/bracket dialect, the source world the modern JSON
  // dialect (§4.1). Both must canonicalize identically.
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/coupon/generate',
    postData:
      'name=Sale&type=1&value=100&show=1&limit_plan_ids[0]=1&limit_plan_ids[1]=3&' +
      'started_at=1750000000&ended_at=1760000000',
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/sec/coupon/generate',
    postData: JSON.stringify({
      name: 'Sale',
      type: 1,
      value: 100,
      show: true,
      limit_plan_ids: [1, 3],
      started_at: '2025-06-15T15:06:40Z',
      ended_at: '2025-10-09T08:53:20Z',
    }),
    securePath: 'sec',
  });

  assert.deepEqual(oracle, {
    routeId: 'admin.coupons.create',
    params: {},
    body: {
      name: 'Sale',
      type: 1,
      value: 100,
      show: true,
      limit_plan_ids: [1, 3],
      started_at: 1_750_000_000,
      ended_at: 1_760_000_000,
    },
  });
  assert.deepEqual(source, oracle);
});

test('pagination params fold onto page/per_page in both worlds (§8)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'GET',
    url: '/api/v1/user/invite/details?current=2&pageSize=10',
  });
  const source = canonicalizeRequest('source', {
    method: 'GET',
    url: '/api/v1/user/invite/details?page=2&per_page=10',
  });
  assert.deepEqual(oracle, {
    routeId: 'user.commissions.list',
    params: { page: 2, per_page: 10 },
    body: null,
  });
  assert.deepEqual(source, oracle);
});

test('the W3 knowledge detail path param equals the legacy ?id= spelling (§5.8)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'GET',
    url: '/api/v1/user/knowledge/fetch?id=2&language=en-US',
  });
  const source = canonicalizeRequest('source', {
    method: 'GET',
    url: '/api/v1/user/knowledge/2?language=en-US',
  });
  assert.deepEqual(oracle, {
    routeId: 'user.knowledge.get',
    params: { id: 2, language: 'en-US' },
    body: null,
  });
  assert.deepEqual(source, oracle);
});

test('legacy bracket filters and the modern filter JSON fold to one clause list (§7.1)', () => {
  const oracleUrl =
    '/api/v1/sec/user/fetch?' +
    new URLSearchParams({
      'filter[0][key]': 'email',
      'filter[0][condition]': '模糊',
      'filter[0][value]': '@gmail.com',
      'filter[1][key]': 'banned',
      'filter[1][condition]': '=',
      'filter[1][value]': '1',
      'filter[2][key]': 'plan_id',
      'filter[2][condition]': '=',
      'filter[2][value]': 'null',
    }).toString();
  const oracle = canonicalizeRequest('oracle', {
    method: 'GET',
    url: oracleUrl,
    securePath: 'sec',
  });
  const modernFilter = JSON.stringify([
    { field: 'email', op: 'like', value: '@gmail.com' },
    { field: 'banned', op: 'eq', value: true },
    { field: 'plan_id', op: 'eq', value: null },
  ]);
  const source = canonicalizeRequest('source', {
    method: 'GET',
    url: `/api/v1/sec/user/fetch?filter=${encodeURIComponent(modernFilter)}`,
    securePath: 'sec',
  });

  assert.deepEqual(oracle.routeId, 'admin.users.list');
  assert.deepEqual(oracle.params.filter, [
    { field: 'email', op: 'like', value: '@gmail.com' },
    { field: 'banned', op: 'eq', value: true },
    { field: 'plan_id', op: 'eq', value: null },
  ]);
  assert.deepEqual(source, oracle);
});

test('like clauses compare on the raw string (§7.1/§13.2)', () => {
  const clauses = canonicalizeFilterClauses([
    { key: 'email', condition: 'like', value: '100' },
  ]);
  assert.deepEqual(clauses, [{ field: 'email', op: 'like', value: '100' }]);
});

test('§4.1 boolean flags fold to booleans, enums stay numeric', () => {
  const params = canonicalizeQueryParams(
    new URLSearchParams({ is_forget: '1', status: '1', level: '2' }),
  );
  assert.equal(params.is_forget, true);
  // True enums (order status, ticket level) stay numbers (§4.1).
  assert.equal(params.status, 1);
  assert.equal(params.level, 2);
  // The legacy `isforget` spelling folds onto the modern name (§4.1).
  const renamed = canonicalizeQueryParams(new URLSearchParams({ isforget: '0' }));
  assert.deepEqual(renamed, { is_forget: false });
});

test('decodeRequestBody handles JSON, form, bracket arrays, and empty bodies', () => {
  assert.equal(decodeRequestBody(null), null);
  assert.equal(decodeRequestBody(''), null);
  assert.deepEqual(decodeRequestBody('{"trade_no":"T1"}'), { trade_no: 'T1' });
  assert.deepEqual(decodeRequestBody('trade_no=T1&method=2'), { trade_no: 'T1', method: '2' });
  assert.deepEqual(foldBracketEntries([['ids[0]', '5'], ['ids[1]', '9']]), {
    ids: ['5', '9'],
  });
});

test('identifier-like strings never fold to numbers', () => {
  const request = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/order/cancel',
    postData: 'trade_no=2026110099007',
  });
  assert.equal(request.routeId, 'user.orders.cancel');
  // Exact numeric round-trip folds…
  assert.equal(request.body.trade_no, 2026110099007);
  const padded = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/order/cancel',
    postData: 'trade_no=007T',
  });
  // …but non-round-tripping identifiers stay strings.
  assert.equal(padded.body.trade_no, '007T');
});

test('unknown routes canonicalize with routeId null', () => {
  const request = canonicalizeRequest('oracle', {
    method: 'GET',
    url: '/api/v1/user/not-a-route',
  });
  assert.equal(request.routeId, null);
  assert.deepEqual(request.params, {});
});
