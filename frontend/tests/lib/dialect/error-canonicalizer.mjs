// Internal-dialect error canonicalizer (docs/api-dialect.md §13.3).
//
// Maps each world's error surface to one canonical `{status_class, code}`
// object:
//
// - source world: reads the problem+json `code` directly (§3.1);
// - oracle world: looks the legacy `message` literal up in the
//   anchor-message→code table below, generated from the §3.4 registry
//   ("Legacy anchor" column). Legacy validation errors carry an `errors`
//   bag and fold to `validation_failed` by shape.
//
// `status_class` compares on the status hundred (4xx/5xx) so the deliberate
// exact-status moves (§3.2: the 403→401 session split, legacy-500 business
// rejections reclassified to 4xx, mail 5xx refinements) never diverge
// cross-world: the oracle side derives it from the mapped code's single
// registered modern status (§3.3 rule 6), the source side from its real
// HTTP status.
//
// Anchors §3.4 cites only as a family/format! (their literal lives in a
// legacy call site, not the spec text) are matched by pattern; family waves
// extend this table when they wire their error scenarios through it.

/**
 * §3.4 — the code registry: slug → single modern HTTP status (§3.3 rule 6).
 * Mirrors `backend/rust/crates/compat/src/problem.rs` in registry order.
 */
export const PROBLEM_CODES = Object.freeze({
  // Transport / generic.
  validation_failed: 422,
  invalid_parameter: 400,
  endpoint_not_found: 404,
  rate_limited: 429,
  internal_error: 500,
  service_unavailable: 503,
  // Auth / session.
  session_expired: 401,
  permission_denied: 403,
  step_up_required: 403,
  invalid_credentials: 400,
  account_suspended: 400,
  registration_closed: 400,
  register_ip_rate_limited: 429,
  password_attempts_rate_limited: 400,
  email_already_registered: 400,
  email_not_registered: 400,
  invalid_email_code: 400,
  invalid_invite_code: 400,
  email_suffix_not_allowed: 400,
  gmail_alias_not_supported: 400,
  recaptcha_failed: 400,
  email_send_rate_limited: 429,
  invalid_token: 400,
  old_password_incorrect: 400,
  password_reset_failed: 400,
  user_not_found: 404,
  user_not_registered: 400,
  // Commerce (user).
  plan_not_found: 404,
  plan_unavailable: 400,
  plan_sold_out: 400,
  plan_period_unavailable: 400,
  plan_change_disabled: 400,
  pending_order_exists: 400,
  order_not_found: 404,
  order_not_pending: 400,
  payment_method_unavailable: 400,
  payment_config_invalid: 400,
  payment_gateway_unsupported: 400,
  payment_amount_out_of_range: 400,
  handling_fee_out_of_range: 400,
  stripe_binding_invalid: 400,
  insufficient_balance: 400,
  coupon_invalid: 400,
  coupon_unavailable: 400,
  coupon_not_started: 400,
  coupon_expired: 400,
  coupon_exhausted: 400,
  coupon_not_applicable: 400,
  gift_card_invalid: 400,
  subscription_value_out_of_range: 400,
  renewal_not_allowed: 400,
  reset_period_invalid: 400,
  // Profile / invite / ticket / content (user).
  transfer_amount_invalid: 422,
  insufficient_commission_balance: 400,
  balance_out_of_range: 400,
  telegram_not_configured: 400,
  telegram_unbind_failed: 400,
  ticket_not_found: 404,
  ticket_invalid_state: 400,
  unresolved_ticket_exists: 400,
  ticket_requires_plan: 400,
  withdraw_method_unsupported: 400,
  withdraw_below_minimum: 400,
  article_not_found: 404,
  notice_not_found: 404,
  knowledge_not_found: 404,
  // Admin.
  config_revision_conflict: 409,
  config_validation_failed: 400,
  payment_method_not_found: 404,
  payment_method_in_use: 400,
  reconciliation_not_found: 404,
  reconciliation_already_processed: 409,
  order_assign_conflict: 400,
  order_update_conflict: 409,
  order_update_failed: 400,
  plan_in_use: 400,
  coupon_not_found: 404,
  gift_card_not_found: 404,
  server_not_found: 404,
  route_not_found: 404,
  server_group_not_found: 404,
  invalid_server_type: 400,
  app_url_not_configured: 400,
  mail_sender_not_configured: 400,
  mail_invalid: 400,
  mail_send_failed: 502,
  mail_idempotency_conflict: 409,
  mail_idempotency_key_invalid: 400,
  telegram_request_failed: 502,
  telegram_token_invalid: 400,
  telegram_webhook_failed: 502,
});

// §3.4 "Legacy anchor" column: exact legacy `message` literal → code. Where
// one literal is shared by two codes split on path-vs-body reference (§3.3
// rule 6), the entry carries per-route overrides; without route context the
// path-identified (not-found) reading wins.
const ANCHOR_CODES = new Map([
  // Transport / generic.
  ['Invalid parameter', 'invalid_parameter'],
  ['参数有误', 'invalid_parameter'],
  ['参数错误', 'invalid_parameter'],
  ['Invalid admin request body', 'invalid_parameter'],
  ['Not Found', 'endpoint_not_found'],
  ['Admin endpoint does not exist', 'endpoint_not_found'],
  ['Staff endpoint does not exist', 'endpoint_not_found'],
  ["Uh-oh, we've had some problems, we're working on it.", 'internal_error'],
  // Auth / session.
  ['未登录或登陆已过期', 'session_expired'],
  ['Permission denied', 'permission_denied'],
  ['Recent password verification is required', 'step_up_required'],
  ['Incorrect email or password', 'invalid_credentials'],
  ['Your account has been suspended', 'account_suspended'],
  ['Registration has closed', 'registration_closed'],
  ['Email already exists', 'email_already_registered'],
  ['This email is registered', 'email_already_registered'],
  ['This email is not registered in the system', 'email_not_registered'],
  ['Incorrect email verification code', 'invalid_email_code'],
  ['Invalid invitation code', 'invalid_invite_code'],
  ['Email suffix is not in the Whitelist', 'email_suffix_not_allowed'],
  ['Gmail alias is not supported', 'gmail_alias_not_supported'],
  ['Invalid code is incorrect', 'recaptcha_failed'],
  // The sendEmailVerify limiter shares the generic 429 literal; only the
  // email-codes route reads it as the dedicated code (§3.4).
  [
    'Too many requests, please try again later.',
    { default: 'rate_limited', byRoute: { 'auth.email-codes': 'email_send_rate_limited' } },
  ],
  ['Token error', 'invalid_token'],
  ['The old password is wrong', 'old_password_incorrect'],
  ['Reset failed', 'password_reset_failed'],
  ['Reset failed, Please try again later', 'password_reset_failed'],
  [
    'The user does not exist',
    {
      default: 'user_not_found',
      byRoute: {
        'admin.users.set-inviter': 'user_not_registered',
        'admin.orders.create': 'user_not_registered',
        'admin.users.mail': 'user_not_registered',
        'staff.users.mail': 'user_not_registered',
      },
    },
  ],
  [
    '该用户不存在',
    {
      default: 'user_not_found',
      byRoute: {
        'admin.users.set-inviter': 'user_not_registered',
        'admin.orders.create': 'user_not_registered',
        'admin.users.mail': 'user_not_registered',
        'staff.users.mail': 'user_not_registered',
      },
    },
  ],
  // Commerce (user).
  [
    'Subscription plan does not exist',
    {
      default: 'plan_not_found',
      byRoute: { 'user.orders.create': 'plan_unavailable' },
    },
  ],
  [
    '该订阅(ID)不存在',
    {
      default: 'plan_not_found',
      byRoute: { 'user.orders.create': 'plan_unavailable' },
    },
  ],
  ['Current product is sold out', 'plan_sold_out'],
  ['Wrong plan period', 'plan_period_unavailable'],
  [
    'This payment period cannot be purchased, please choose another period',
    'plan_period_unavailable',
  ],
  [
    'You have an unpaid or pending order, please try again later or cancel it',
    'pending_order_exists',
  ],
  ['订单不存在', 'order_not_found'],
  ['Order does not exist or has been paid', 'order_not_found'],
  ['只能对待支付的订单进行操作', 'order_not_pending'],
  ['Payment method is not available', 'payment_method_unavailable'],
  ['Payment config is invalid', 'payment_config_invalid'],
  ['gate is not found', 'payment_gateway_unsupported'],
  ['Payment amount is outside the supported range', 'payment_amount_out_of_range'],
  ['Order amount is outside the supported range', 'payment_amount_out_of_range'],
  ['Payment handling fee is outside the supported range', 'handling_fee_out_of_range'],
  ['Stripe payment binding is invalid', 'stripe_binding_invalid'],
  ['Insufficient balance', 'insufficient_balance'],
  ['Invalid coupon', 'coupon_invalid'],
  ['Invalid coupon discount value', 'coupon_invalid'],
  ['This coupon is no longer available', 'coupon_unavailable'],
  ['This coupon has not yet started', 'coupon_not_started'],
  ['This coupon has expired', 'coupon_expired'],
  ['Coupon failed', 'coupon_exhausted'],
  ['The coupon code cannot be used for this subscription', 'coupon_not_applicable'],
  ['The coupon code cannot be used for this period', 'coupon_not_applicable'],
  [
    '礼品卡不存在',
    {
      default: 'gift_card_not_found',
      byRoute: { 'user.gift-card-redemptions.create': 'gift_card_invalid' },
    },
  ],
  ['Renewal is not allowed', 'renewal_not_allowed'],
  ['You do not allow to renew the subscription', 'renewal_not_allowed'],
  ['Invalid reset period', 'reset_period_invalid'],
  // Profile / invite / ticket / content (user).
  ['The transfer amount parameter is wrong', 'transfer_amount_invalid'],
  ['Insufficient commission balance', 'insufficient_commission_balance'],
  ['Balance exceeds the supported range', 'balance_out_of_range'],
  ['Telegram bot is not configured', 'telegram_not_configured'],
  ['telegram bot token is null', 'telegram_not_configured'],
  ['Unbind telegram failed', 'telegram_unbind_failed'],
  ['Ticket does not exist', 'ticket_not_found'],
  ['工单不存在', 'ticket_not_found'],
  ['未知的工单状态', 'ticket_invalid_state'],
  ['There are other unresolved tickets', 'unresolved_ticket_exists'],
  ['用户存在其他未解决工单，无法重新打开该工单', 'unresolved_ticket_exists'],
  ['请先购买套餐', 'ticket_requires_plan'],
  ['当前套餐不允许发起工单', 'ticket_requires_plan'],
  ['Unsupported withdrawal method', 'withdraw_method_unsupported'],
  ['Article does not exist', 'article_not_found'],
  ['Notice not found', 'notice_not_found'],
  ['公告不存在', 'notice_not_found'],
  ['知识不存在', 'knowledge_not_found'],
  // Admin.
  ['配置已被其他请求更新，请刷新后重试', 'config_revision_conflict'],
  ['支付方式不存在', 'payment_method_not_found'],
  ['付款核对记录不存在', 'reconciliation_not_found'],
  ['付款核对记录已处理', 'reconciliation_already_processed'],
  ['该用户还有待支付的订单，无法分配', 'order_assign_conflict'],
  ['订单状态正在被其他请求修改，请重试', 'order_update_conflict'],
  ['更新失败', 'order_update_failed'],
  ['该订阅下存在订单无法删除', 'plan_in_use'],
  ['该订阅下存在用户无法删除', 'plan_in_use'],
  ['该订阅仍被礼品卡使用，无法删除', 'plan_in_use'],
  ['优惠券不存在', 'coupon_not_found'],
  ['该服务器不存在', 'server_not_found'],
  ['路由不存在', 'route_not_found'],
  ['该服务器组不存在', 'server_group_not_found'],
  ['Invalid server type', 'invalid_server_type'],
  ['请在站点配置中配置站点地址', 'app_url_not_configured'],
  ['Email sender is not configured', 'mail_sender_not_configured'],
  ['Email host is not configured', 'mail_sender_not_configured'],
  ['Invalid email sender', 'mail_invalid'],
  ['Invalid recipient email', 'mail_invalid'],
  ['Build mail failed', 'mail_send_failed'],
  ['Mail idempotency key was reused with a different payload', 'mail_idempotency_conflict'],
  ['Telegram bot response is invalid', 'telegram_request_failed'],
  ['Telegram token is invalid', 'telegram_token_invalid'],
  ['Telegram bot token cannot be empty', 'telegram_token_invalid'],
  ['Telegram webhook failed', 'telegram_webhook_failed'],
]);

// §3.4 anchors whose legacy literal is a format!/family (dynamic
// interpolations stay in the localized `detail`, §3.1).
const ANCHOR_PATTERNS = [
  [/^There are too many password errors, please try again after \d+ minutes?\.$/, 'password_attempts_rate_limited'],
  [/^Register frequently, please try again after \d+ minute/, 'register_ip_rate_limited'],
  [/^The current required minimum withdrawal commission is /, 'withdraw_below_minimum'],
  [/^Subscription .*exceeds the supported range$/, 'subscription_value_out_of_range'],
  [/^配置校验失败: /, 'config_validation_failed'],
  [/^配置安全校验失败: /, 'config_validation_failed'],
  [/^Email (sender|recipient|content) is invalid$/, 'mail_invalid'],
  [/^(Send mail|Email send) (failed|timed out)/, 'mail_send_failed'],
  [/^Mail idempotency key is (invalid|too long)/, 'mail_idempotency_key_invalid'],
  [/^Telegram request failed/, 'telegram_request_failed'],
];

/** Resolve a legacy `message` literal to its §3.4 code (or null). */
export function anchorCodeFor(message, { routeId = null } = {}) {
  if (typeof message !== 'string' || message === '') return null;
  const entry = ANCHOR_CODES.get(message);
  if (entry) {
    if (typeof entry === 'string') return entry;
    return (routeId && entry.byRoute[routeId]) || entry.default;
  }
  for (const [pattern, code] of ANCHOR_PATTERNS) {
    if (pattern.test(message)) return code;
  }
  return null;
}

/** The 4xx/5xx status class one canonical error compares on (§13.3). */
export function statusClassOf(status) {
  const numeric = Number(status);
  if (!Number.isFinite(numeric) || numeric < 100) return 'unknown';
  return `${Math.floor(numeric / 100)}xx`;
}

/**
 * Canonicalize one error surface to `{status_class, code}` (§13.3).
 *
 * - source world: `body` is the problem+json document; `code` is read
 *   directly and `status_class` derives from the real HTTP status.
 * - oracle world: `message` (or `body.message`) is looked up in the anchor
 *   table; a legacy `errors` bag folds to `validation_failed` by shape, and
 *   `status_class` derives from the mapped code's registered modern status.
 *   `status` is the legacy error status — the in-body `code` for the
 *   HTTP-200 `{code: 400}` fixture emulation, else the HTTP status.
 *   Unmapped messages keep `code: null` and the raw status class so a
 *   missing anchor surfaces as a cross-world mismatch instead of a silent
 *   pass.
 */
export function canonicalizeError(world, { status, body, message, routeId = null }) {
  if (world === 'source') {
    const code = typeof body?.code === 'string' ? body.code : null;
    return { status_class: statusClassOf(status), code };
  }
  if (world !== 'oracle') {
    throw new Error(`Unknown parity world "${world}" (expected oracle | source)`);
  }
  const legacyMessage = typeof message === 'string' ? message : body?.message;
  const hasValidationBag =
    body?.errors !== null && typeof body?.errors === 'object' && !Array.isArray(body?.errors);
  const code = hasValidationBag ? 'validation_failed' : anchorCodeFor(legacyMessage, { routeId });
  if (code) return { status_class: statusClassOf(PROBLEM_CODES[code]), code };
  return { status_class: statusClassOf(status), code: fallbackCodeFor(status) };
}

// §3.4: generic 429/500/503 fall back to their status-wide codes.
function fallbackCodeFor(status) {
  switch (Number(status)) {
    case 429:
      return 'rate_limited';
    case 500:
      return 'internal_error';
    case 503:
      return 'service_unavailable';
    default:
      return null;
  }
}

/**
 * §13.3/§6.1 — the pinned non-error equivalence: the oracle's 503
 * `配置已提交…` config-activation message and the source world's
 * 202 `{"activation": "pending"}` map to one canonical outcome.
 */
export const CONFIG_ACTIVATION_PENDING = Object.freeze({ kind: 'config_activation_pending' });

export function canonicalizeConfigActivation(world, { status, body, message }) {
  if (world === 'source') {
    return Number(status) === 202 && body?.activation === 'pending'
      ? CONFIG_ACTIVATION_PENDING
      : null;
  }
  if (world !== 'oracle') {
    throw new Error(`Unknown parity world "${world}" (expected oracle | source)`);
  }
  const legacyMessage = typeof message === 'string' ? message : body?.message;
  return Number(status) === 503 && typeof legacyMessage === 'string' && legacyMessage.startsWith('配置已提交')
    ? CONFIG_ACTIVATION_PENDING
    : null;
}
