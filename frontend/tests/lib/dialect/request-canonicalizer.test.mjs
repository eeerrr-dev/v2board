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
    // W10: the source world requests the modern §6.3 coupons row.
    url: '/api/v1/sec/coupons',
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

test('the W10 legacy edit-by-generate folds onto the modern PATCH identity (§6.3)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/coupon/generate',
    postData: 'id=5&name=Edited&type=2&value=30&started_at=1750000000&ended_at=1760000000',
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'PATCH',
    url: '/api/v1/sec/coupons/5',
    postData: JSON.stringify({
      name: 'Edited',
      type: 2,
      value: 30,
      started_at: '2025-06-15T15:06:40Z',
      ended_at: '2025-10-09T08:53:20Z',
    }),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.coupons.update',
    params: { id: 5 },
    body: {
      name: 'Edited',
      type: 2,
      value: 30,
      started_at: 1_750_000_000,
      ended_at: 1_760_000_000,
    },
  });
  assert.deepEqual(source, oracle);

  // The knowledge sort body renames `knowledge_ids` → `ids` (§6.3).
  const sort = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/knowledge/sort',
    postData: 'knowledge_ids[0]=2&knowledge_ids[1]=1',
    securePath: 'sec',
  });
  assert.deepEqual(sort.body, { ids: [2, 1] });
});

test('pagination params fold onto page/per_page in both worlds (§8)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'GET',
    url: '/api/v1/user/invite/details?current=2&pageSize=10',
  });
  const source = canonicalizeRequest('source', {
    method: 'GET',
    // W7: the source world requests the modern §5.6 commissions row.
    url: '/api/v1/user/commissions?page=2&per_page=10',
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
    url: `/api/v1/sec/users?filter=${encodeURIComponent(modernFilter)}`,
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

test('the W11 plan save folds create vs edit onto the modern identity (§6.2)', () => {
  const oracleCreate = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/plan/save',
    postData: JSON.stringify({ name: 'Pro', content: 'x', month_price: 1200 }),
    securePath: 'sec',
  });
  const sourceCreate = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/sec/plans',
    postData: JSON.stringify({ name: 'Pro', content: 'x', month_price: 1200 }),
    securePath: 'sec',
  });
  assert.deepEqual(oracleCreate, {
    routeId: 'admin.plans.create',
    params: {},
    body: { name: 'Pro', content: 'x', month_price: 1200 },
  });
  assert.deepEqual(sourceCreate, oracleCreate);

  // The legacy edit rode the same plan/save action behind a body id; the modern
  // edit is a PATCH whose path carries the identity.
  const oracleEdit = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/plan/save',
    postData: JSON.stringify({ id: 5, name: 'Pro', content: 'x', month_price: 1200 }),
    securePath: 'sec',
  });
  const sourceEdit = canonicalizeRequest('source', {
    method: 'PATCH',
    url: '/api/v1/sec/plans/5',
    postData: JSON.stringify({ name: 'Pro', content: 'x', month_price: 1200 }),
    securePath: 'sec',
  });
  assert.deepEqual(oracleEdit, {
    routeId: 'admin.plans.update',
    params: { id: 5 },
    body: { name: 'Pro', content: 'x', month_price: 1200 },
  });
  assert.deepEqual(sourceEdit, oracleEdit);
});

test('the W11 plan show/renew toggle folds id into the PATCH path (§6.2)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/plan/update',
    postData: 'id=5&show=1',
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'PATCH',
    url: '/api/v1/sec/plans/5',
    postData: JSON.stringify({ show: true }),
    securePath: 'sec',
  });
  // The legacy dedicated toggle action and the modern merged PATCH resolve to
  // different route ids (the toggle merges into the update row), but the
  // captured capture — params + the boolean flag body — is identical.
  assert.equal(oracle.routeId, 'admin.plans.toggle');
  assert.equal(source.routeId, 'admin.plans.update');
  assert.deepEqual(oracle.params, { id: 5 });
  assert.deepEqual(oracle.body, { show: true });
  assert.deepEqual(
    { params: source.params, body: source.body },
    { params: oracle.params, body: oracle.body },
  );
});

test('the W11 payment save folds the config bracket form onto nested JSON (§6.2)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/payment/save',
    postData: 'id=1&name=Alipay&payment=AlipayF2F&config[key]=sk&config[mch_id]=m1',
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'PATCH',
    url: '/api/v1/sec/payments/1',
    postData: JSON.stringify({
      name: 'Alipay',
      payment: 'AlipayF2F',
      config: { key: 'sk', mch_id: 'm1' },
    }),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.payments.update',
    params: { id: 1 },
    body: { name: 'Alipay', payment: 'AlipayF2F', config: { key: 'sk', mch_id: 'm1' } },
  });
  assert.deepEqual(source, oracle);
});

test('the W11 order status update folds trade_no into the PATCH path (§6.4)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/order/update',
    postData: JSON.stringify({ trade_no: 'VISUAL2026110001', status: 3 }),
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'PATCH',
    url: '/api/v1/sec/orders/VISUAL2026110001',
    postData: JSON.stringify({ status: 3 }),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.orders.update',
    params: { trade_no: 'VISUAL2026110001' },
    body: { status: 3 },
  });
  assert.deepEqual(source, oracle);
});

test('the W11 reconciliation resolve splits out of order/update (§6.4)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/order/update',
    postData: JSON.stringify({ reconciliation_id: 7, resolution: 'confirm' }),
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/sec/payment-reconciliations/7/resolve',
    postData: JSON.stringify({ resolution: 'confirm' }),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.payment-reconciliations.resolve',
    params: { id: 7 },
    body: { resolution: 'confirm' },
  });
  assert.deepEqual(source, oracle);
});

test('the W12 user list folds pagination/sort/filter onto the §7/§8 form', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'GET',
    url:
      '/api/v1/sec/user/fetch?current=2&pageSize=10&sort=banned&sort_type=ASC' +
      '&filter[0][key]=email&filter[0][condition]=模糊&filter[0][value]=visual@example.com' +
      '&filter[1][key]=banned&filter[1][condition]==&filter[1][value]=1' +
      '&filter[2][key]=plan_id&filter[2][condition]==&filter[2][value]=null',
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'GET',
    url:
      '/api/v1/sec/users?page=2&per_page=10&sort_by=banned&sort_dir=asc&filter=' +
      encodeURIComponent(
        JSON.stringify([
          { field: 'email', op: 'like', value: 'visual@example.com' },
          { field: 'banned', op: 'eq', value: true },
          { field: 'plan_id', op: 'eq', value: null },
        ]),
      ),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.users.list',
    params: {
      page: 2,
      per_page: 10,
      sort_by: 'banned',
      sort_dir: 'asc',
      filter: [
        { field: 'email', op: 'like', value: 'visual@example.com' },
        { field: 'banned', op: 'eq', value: true },
        { field: 'plan_id', op: 'eq', value: null },
      ],
    },
    body: null,
  });
  assert.deepEqual(source, oracle);
});

test('the W12 user update folds the body id onto the PATCH path (§6.6)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/user/update',
    postData: JSON.stringify({ id: 1, email: 'a@b.test', banned: 1 }),
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'PATCH',
    url: '/api/v1/sec/users/1',
    postData: JSON.stringify({ email: 'a@b.test', banned: true }),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.users.update',
    params: { id: 1 },
    body: { email: 'a@b.test', banned: true },
  });
  assert.deepEqual(source, oracle);
});

test('the W12 user delete folds the body id onto the DELETE path (§6.6)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/user/delUser',
    postData: JSON.stringify({ id: 1 }),
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'DELETE',
    url: '/api/v1/sec/users/1',
    securePath: 'sec',
  });
  assert.deepEqual(oracle, { routeId: 'admin.users.delete', params: { id: 1 }, body: null });
  assert.deepEqual(source, oracle);
});

test('the W12 user set-inviter folds body id + clears the inviter (§6.6)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/user/setInviteUser',
    postData: JSON.stringify({ id: 2, invite_user_email: null }),
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/sec/users/2/set-inviter',
    postData: JSON.stringify({ invite_user_email: null }),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.users.set-inviter',
    params: { id: 2 },
    body: { invite_user_email: null },
  });
  assert.deepEqual(source, oracle);
});

test('the W12 user bulk actions fold the body-borne filter clauses (§7.1)', () => {
  const oracle = canonicalizeRequest('oracle', {
    method: 'POST',
    url: '/api/v1/sec/user/ban',
    postData:
      'filter[0][key]=email&filter[0][condition]=模糊&filter[0][value]=visual@example.com',
    securePath: 'sec',
  });
  const source = canonicalizeRequest('source', {
    method: 'POST',
    url: '/api/v1/sec/users/ban',
    postData: JSON.stringify({
      filter: [{ field: 'email', op: 'like', value: 'visual@example.com' }],
    }),
    securePath: 'sec',
  });
  assert.deepEqual(oracle, {
    routeId: 'admin.users.ban',
    params: {},
    body: { filter: [{ field: 'email', op: 'like', value: 'visual@example.com' }] },
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
