import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  FIXTURE_WORLDS,
  emitFixtureResponse,
  emitLegacyFixtureResponse,
  emitModernFixtureResponse,
} from './fixture-emitters.mjs';

test('legacy fixtures emit the identical legacy wire dialect in both worlds (§13.5)', () => {
  assert.deepEqual(FIXTURE_WORLDS, ['oracle', 'source']);
  const fixture = { code: 200, data: { id: 1 }, total: 1 };
  const oracle = emitFixtureResponse('oracle', fixture);
  const source = emitFixtureResponse('source', fixture);
  assert.deepEqual(oracle, {
    status: 200,
    contentType: 'application/json',
    body: '{"code":200,"data":{"id":1},"total":1}',
  });
  assert.deepEqual(source, oracle);
  assert.throws(() => emitFixtureResponse('staging', fixture), /Unknown fixture world/);
});

test('the legacy HTTP-200 {code:400} error emulation is preserved', () => {
  const wire = emitLegacyFixtureResponse({ code: 400, data: null, message: '优惠券无效' });
  assert.equal(wire.status, 200);
  assert.deepEqual(JSON.parse(wire.body), { code: 400, data: null, message: '优惠券无效' });
});

test('httpStatus and rawBody transport overrides pass through', () => {
  const httpError = emitLegacyFixtureResponse({
    code: 500,
    data: null,
    httpStatus: 500,
    message: 'Server Error',
  });
  assert.equal(httpError.status, 500);
  assert.deepEqual(JSON.parse(httpError.body), { code: 500, data: null, message: 'Server Error' });

  const csv = emitLegacyFixtureResponse({
    contentType: 'text/csv',
    httpStatus: 200,
    rawBody: 'id,email\n1,a@example.com\n',
  });
  assert.deepEqual(csv, {
    status: 200,
    contentType: 'text/csv',
    body: 'id,email\n1,a@example.com\n',
  });
});

test('W2: v2 dialect fixtures emit bare bodies with real HTTP statuses in the source world', () => {
  const bare = emitFixtureResponse('source', {
    dialect: 'v2',
    data: { auth_data: 'TOKEN', is_admin: false },
  });
  assert.deepEqual(bare, {
    status: 200,
    contentType: 'application/json',
    body: '{"auth_data":"TOKEN","is_admin":false}',
  });

  const created = emitModernFixtureResponse({ dialect: 'v2', data: { id: 1 }, httpStatus: 201 });
  assert.equal(created.status, 201);
  assert.deepEqual(JSON.parse(created.body), { id: 1 });

  const empty = emitModernFixtureResponse({ dialect: 'v2', data: null, httpStatus: 204 });
  assert.deepEqual(empty, { status: 204, contentType: 'application/json', body: '' });
});

test('W2: v2 problem fixtures emit RFC 9457 problem+json with the problem status', () => {
  const problem = {
    type: 'about:blank',
    title: 'Unauthorized',
    status: 401,
    code: 'session_expired',
    detail: '未登录或登陆已过期',
  };
  const wire = emitFixtureResponse('source', { dialect: 'v2', httpStatus: 401, problem });
  assert.deepEqual(wire, {
    status: 401,
    contentType: 'application/problem+json',
    body: JSON.stringify(problem),
  });
});

test('W2: the oracle world rejects v2 dialect fixtures (it speaks legacy forever)', () => {
  assert.throws(
    () => emitFixtureResponse('oracle', { dialect: 'v2', data: { is_login: true } }),
    /source-world only/,
  );
});
