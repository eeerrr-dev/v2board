import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  FIXTURE_WORLDS,
  emitFixtureResponse,
  emitLegacyFixtureResponse,
} from './fixture-emitters.mjs';

test('W0: both worlds emit the identical legacy wire dialect (§13.5)', () => {
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
