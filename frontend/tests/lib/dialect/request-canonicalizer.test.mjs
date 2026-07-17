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
  // Exact numeric round-trip folds… (W4 lifts the legacy body-carried
  // trade_no onto the canonical path identity.)
  assert.equal(request.params.trade_no, 2026110099007);
  const padded = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/order/cancel',
    postData: 'trade_no=007T',
  });
  // …but non-round-tripping identifiers stay strings.
  assert.equal(padded.params.trade_no, '007T');
});

test('the W4 create-order bodies fold onto the §9.2 union in both worlds (§5.5)', () => {
  const oraclePlan = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/order/save',
    postData: JSON.stringify({ plan_id: 1, period: 'month_price', coupon_code: '' }),
  });
  const sourcePlan = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/user/orders',
    postData: JSON.stringify({ kind: 'plan', plan_id: 1, period: 'month_price' }),
  });
  assert.deepEqual(oraclePlan, {
    routeId: 'user.orders.create',
    params: {},
    // §5.5: the legacy empty coupon_code spelling folds to omission.
    body: { kind: 'plan', plan_id: 1, period: 'month_price' },
  });
  assert.deepEqual(sourcePlan, oraclePlan);

  const oracleDeposit = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/order/save',
    postData: JSON.stringify({ plan_id: 0, period: 'deposit', deposit_amount: 1234 }),
  });
  const sourceDeposit = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/user/orders',
    postData: JSON.stringify({ kind: 'deposit', deposit_amount: 1234 }),
  });
  assert.deepEqual(oracleDeposit, {
    routeId: 'user.orders.create',
    params: {},
    // The plan_id: 0 + period: "deposit" sentinel dies with the wave.
    body: { kind: 'deposit', deposit_amount: 1234 },
  });
  assert.deepEqual(sourceDeposit, oracleDeposit);
});

test('the W4 checkout selection folds onto path trade_no + method_id (§5.5)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/order/checkout',
    postData: JSON.stringify({ trade_no: 'VISUAL2026110001', method: '3' }),
  });
  const source = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/user/orders/VISUAL2026110001/checkout',
    postData: JSON.stringify({ method_id: 3 }),
  });
  assert.deepEqual(oracle, {
    routeId: 'user.orders.checkout',
    params: { trade_no: 'VISUAL2026110001' },
    body: { method_id: 3 },
  });
  assert.deepEqual(source, oracle);

  const oracleIntent = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/order/stripe/intent',
    postData: JSON.stringify({ trade_no: 'VISUAL2026110001', method: 2 }),
  });
  const sourceIntent = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/user/orders/VISUAL2026110001/stripe-intent',
    postData: JSON.stringify({ method_id: 2 }),
  });
  assert.deepEqual(oracleIntent, {
    routeId: 'user.orders.stripe-intent',
    params: { trade_no: 'VISUAL2026110001' },
    body: { method_id: 2 },
  });
  assert.deepEqual(sourceIntent, oracleIntent);
});

test('the W5 session revocation folds onto path session_id in both worlds (§9.4)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/removeActiveSession',
    postData: 'session_id=digest-abc',
  });
  const source = canonicalizeRequest('source', {
    method: 'DELETE',
    url: '/api/v1/user/sessions/digest-abc',
  });
  assert.deepEqual(oracle, {
    routeId: 'user.sessions.delete',
    params: { session_id: 'digest-abc' },
    body: null,
  });
  assert.deepEqual(source, oracle);
});

test('the W5 profile PATCH flags equal the legacy 0/1 form spelling (§4.1)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/user/update',
    postData: 'remind_expire=1',
  });
  const source = canonicalizeRequest('source', {
    method: 'PATCH',
    url: '/api/v1/user/profile',
    postData: JSON.stringify({ remind_expire: true }),
  });
  assert.deepEqual(oracle, {
    routeId: 'user.profile.update',
    params: {},
    body: { remind_expire: true },
  });
  assert.deepEqual(source, oracle);
});

test('unknown routes canonicalize with routeId null', () => {
  const request = canonicalizeRequest('oracle', {
    method: 'GET',
    url: '/api/v1/user/not-a-route',
  });
  assert.equal(request.routeId, null);
  assert.deepEqual(request.params, {});
});
