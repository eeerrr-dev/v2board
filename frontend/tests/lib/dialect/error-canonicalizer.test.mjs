import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  CONFIG_ACTIVATION_PENDING,
  PROBLEM_CODES,
  anchorCodeFor,
  canonicalizeConfigActivation,
  canonicalizeError,
  statusClassOf,
} from './error-canonicalizer.mjs';

test('legacy {message} and problem+json reduce to one canonical error (§13.3)', () => {
  const oracle = canonicalizeError('oracle', {
    status: 400,
    body: { code: 400, data: null, message: 'Current product is sold out' },
  });
  const source = canonicalizeError('source', {
    status: 400,
    body: {
      type: 'about:blank',
      title: 'Bad Request',
      status: 400,
      code: 'plan_sold_out',
      detail: '当前产品已售罄',
    },
  });
  assert.deepEqual(oracle, { status_class: '4xx', code: 'plan_sold_out' });
  assert.deepEqual(source, oracle);
});

test('the 403-vs-401 session split normalizes via session_expired (§13.3)', () => {
  // Legacy: 403 「未登录或登陆已过期」. Modern: 401 problem+json.
  const oracle = canonicalizeError('oracle', {
    status: 403,
    body: { code: 403, data: null, message: '未登录或登陆已过期' },
  });
  const source = canonicalizeError('source', {
    status: 401,
    body: { type: 'about:blank', title: 'Unauthorized', status: 401, code: 'session_expired', detail: 'x' },
  });
  assert.deepEqual(oracle, { status_class: '4xx', code: 'session_expired' });
  assert.deepEqual(source, oracle);
});

test('legacy validation bags fold to validation_failed by shape (§3.4)', () => {
  const oracle = canonicalizeError('oracle', {
    status: 422,
    body: { message: '邮箱格式不正确', errors: { email: ['邮箱格式不正确'] } },
  });
  assert.deepEqual(oracle, { status_class: '4xx', code: 'validation_failed' });
});

test('anchor lookups honor the path-vs-body route splits (§3.3 rule 6)', () => {
  assert.equal(anchorCodeFor('The user does not exist'), 'user_not_found');
  assert.equal(
    anchorCodeFor('The user does not exist', { routeId: 'admin.users.set-inviter' }),
    'user_not_registered',
  );
  assert.equal(
    anchorCodeFor('礼品卡不存在', { routeId: 'user.gift-card-redemptions.create' }),
    'gift_card_invalid',
  );
  assert.equal(anchorCodeFor('礼品卡不存在', { routeId: 'admin.gift-cards.delete' }), 'gift_card_not_found');
  assert.equal(
    anchorCodeFor('Too many requests, please try again later.', { routeId: 'auth.email-codes' }),
    'email_send_rate_limited',
  );
  assert.equal(anchorCodeFor('Too many requests, please try again later.'), 'rate_limited');
});

test('format!-family anchors match by pattern (§3.4)', () => {
  assert.equal(
    anchorCodeFor('There are too many password errors, please try again after 15 minutes.'),
    'password_attempts_rate_limited',
  );
  assert.equal(
    anchorCodeFor('Register frequently, please try again after 3 minute'),
    'register_ip_rate_limited',
  );
  assert.equal(
    anchorCodeFor('The current required minimum withdrawal commission is 100'),
    'withdraw_below_minimum',
  );
  assert.equal(
    anchorCodeFor('配置校验失败: app_url 无效'),
    'config_validation_failed',
  );
  assert.equal(anchorCodeFor('Send mail timed out'), 'mail_send_failed');
  assert.equal(anchorCodeFor('unmapped message'), null);
});

test('reclassified legacy statuses compare equal through the code registry', () => {
  // `gate is not found` was a legacy 500; the modern code is a 400 (§3.2).
  const oracle = canonicalizeError('oracle', {
    status: 500,
    body: { code: 500, data: null, message: 'gate is not found' },
  });
  const source = canonicalizeError('source', {
    status: 400,
    body: { type: 'about:blank', title: 'Bad Request', status: 400, code: 'payment_gateway_unsupported', detail: 'x' },
  });
  assert.deepEqual(oracle, { status_class: '4xx', code: 'payment_gateway_unsupported' });
  assert.deepEqual(source, oracle);
});

test('unmapped oracle errors keep the raw status class and a null/fallback code', () => {
  assert.deepEqual(canonicalizeError('oracle', { status: 400, body: { message: '优惠券无效' } }), {
    status_class: '4xx',
    code: null,
  });
  assert.deepEqual(canonicalizeError('oracle', { status: 500, body: { message: 'boom' } }), {
    status_class: '5xx',
    code: 'internal_error',
  });
  assert.deepEqual(canonicalizeError('oracle', { status: 429, body: {} }), {
    status_class: '4xx',
    code: 'rate_limited',
  });
});

test('every anchor and fallback resolves to a registered §3.4 code', () => {
  assert.equal(statusClassOf(PROBLEM_CODES.session_expired), '4xx');
  assert.equal(statusClassOf(PROBLEM_CODES.mail_send_failed), '5xx');
  const anchors = [
    'Permission denied',
    'Recent password verification is required',
    'Invalid coupon',
    '该服务器不存在',
    '配置已被其他请求更新，请刷新后重试',
  ];
  for (const anchor of anchors) {
    const code = anchorCodeFor(anchor);
    assert.ok(code, anchor);
    assert.ok(PROBLEM_CODES[code], `${anchor} → ${code} must be registered`);
  }
});

test('the config-activation 503/202 equivalence is pinned (§13.3/§6.1)', () => {
  const oracle = canonicalizeConfigActivation('oracle', {
    status: 503,
    body: {
      code: 503,
      message: '配置已提交，但当前 API 未能激活新配置；服务将自动重试，请稍后刷新',
    },
  });
  const source = canonicalizeConfigActivation('source', {
    status: 202,
    body: { activation: 'pending', revision: 8 },
  });
  assert.equal(oracle, CONFIG_ACTIVATION_PENDING);
  assert.equal(source, CONFIG_ACTIVATION_PENDING);
  assert.equal(
    canonicalizeConfigActivation('source', { status: 204, body: null }),
    null,
  );
  assert.equal(
    canonicalizeConfigActivation('source', {
      status: 202,
      body: { activation: 'pending' },
    }),
    null,
  );
  assert.equal(
    canonicalizeConfigActivation('oracle', { status: 503, body: { message: 'other outage' } }),
    null,
  );
});
