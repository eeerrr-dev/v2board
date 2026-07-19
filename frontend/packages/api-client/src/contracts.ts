import { z } from 'zod';
import type { AdminConfig } from '@v2board/types';

const nullableNumber = z.number().nullable();
const nullableString = z.string().nullable();
const binaryFlagSchema = z.union([z.literal(0), z.literal(1)]);

export const stringSchema = z.string();
export const numberSchema = z.number();
export const stringArraySchema = z.array(stringSchema);
export const numberArraySchema = z.array(numberSchema);

export const arraySchema = <TSchema extends z.ZodType>(item: TSchema) => z.array(item);

/** Dialect 204 successes (docs/api-dialect.md §3.3): no body at all. */
export const noContentSchema = z.undefined();

// Deliberately excludes the permanent subscription credential (`users.token`):
// the Rust backend no longer returns it from login/register/token-login, and
// the subscribe URL is fetched separately through /user/subscription.
export const authDataSchema = z.looseObject({
  is_admin: z.boolean(),
  auth_data: z.string().min(1),
});

/** POST /auth/step-up: a fresh privileged step-up grant. */
export const stepUpGrantSchema = z.looseObject({
  step_up_token: z.string().min(1),
  expires_in: z.number().int().positive(),
});

/** GET account/mfa (§6.10): the caller's own two-factor state. */
export const mfaStatusSchema = z.looseObject({
  totp_enabled: z.boolean(),
  totp_enabled_at: z.string().min(1).nullable(),
});

/**
 * POST account/mfa/totp (§6.10): the one-time provisioning body — the base32
 * secret is never readable again after this response.
 */
export const totpProvisioningSchema = z.looseObject({
  secret: z.string().min(1),
  otpauth_url: z.string().min(1),
});

/** GET /auth/session: the checkLogin successor's bare probe body (§5.2). */
export const sessionStateSchema = z.looseObject({
  is_login: z.boolean(),
  is_admin: z.boolean().optional(),
});

/** POST /auth/quick-login-url: the minted `{url}` body (§5.2, §9.4). */
export const quickLoginUrlSchema = z.looseObject({
  url: z.string().min(1),
});

/**
 * GET /public/config (docs/api-dialect.md §5.1, W3): bare body, boolean
 * flags, and an always-array `email_whitelist_suffix` (the legacy `0`
 * disabled-sentinel died with the flip). Keeps its historical `guest` name so
 * call sites stay stable while the wire moved off `/guest/comm/config`.
 */
export const guestConfigSchema = z.looseObject({
  tos_url: nullableString,
  is_email_verify: z.boolean(),
  is_invite_force: z.boolean(),
  email_whitelist_suffix: stringArraySchema,
  is_recaptcha: z.boolean(),
  recaptcha_site_key: nullableString,
  app_description: nullableString,
  app_url: nullableString,
  logo: nullableString,
});

/**
 * GET /user/profile (docs/api-dialect.md §5.3, W5): bare body, boolean flags
 * (§4.1), RFC 3339 timestamps (§4.5). Money stays integer cents.
 */
export const userProfileSchema = z.looseObject({
  email: z.string(),
  transfer_enable: z.number(),
  device_limit: nullableNumber,
  last_login_at: nullableString,
  created_at: z.string(),
  banned: z.boolean(),
  auto_renewal: z.boolean(),
  remind_expire: z.boolean(),
  remind_traffic: z.boolean(),
  expired_at: nullableString,
  balance: z.number(),
  commission_balance: z.number(),
  plan_id: nullableNumber,
  discount: nullableNumber,
  commission_rate: nullableNumber,
  telegram_id: nullableNumber,
  uuid: z.string(),
  avatar_url: z.string(),
});

export const planPeriodSchema = z.enum([
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
]);

/**
 * Admin GET /{secure_path}/plans + /{secure_path}/plans/{id} (docs/api-dialect.md
 * §6.2, W11): bare rows with boolean `show`/`renew` (§4.1) and RFC 3339
 * timestamps (§4.5). Prices stay integer cents; the admin list keeps the
 * legacy sold `count`.
 */
export const planSchema = z.looseObject({
  id: z.number(),
  group_id: z.number(),
  transfer_enable: z.number(),
  device_limit: nullableNumber,
  speed_limit: nullableNumber,
  reset_traffic_method: z
    .union([z.literal(0), z.literal(1), z.literal(2), z.literal(3), z.literal(4)])
    .nullable(),
  name: z.string(),
  show: z.boolean(),
  sort: nullableNumber,
  renew: z.boolean(),
  content: nullableString,
  month_price: nullableNumber,
  quarter_price: nullableNumber,
  half_year_price: nullableNumber,
  year_price: nullableNumber,
  two_year_price: nullableNumber,
  three_year_price: nullableNumber,
  onetime_price: nullableNumber,
  reset_price: nullableNumber,
  capacity_limit: nullableNumber,
  count: z.number().optional(),
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * GET /user/plans and /user/plans/{id} (docs/api-dialect.md §5.5, W4): bare
 * bodies, boolean `show`/`renew` (§4.1), RFC 3339 timestamps (§4.5).
 * `capacity_limit` keeps the legacy remaining-capacity rewrite.
 */
export const userPlanSchema = z.looseObject({
  id: z.number(),
  group_id: z.number(),
  transfer_enable: z.number(),
  device_limit: nullableNumber,
  speed_limit: nullableNumber,
  reset_traffic_method: z
    .union([z.literal(0), z.literal(1), z.literal(2), z.literal(3), z.literal(4)])
    .nullable(),
  name: z.string(),
  show: z.boolean(),
  sort: nullableNumber,
  renew: z.boolean(),
  content: nullableString,
  month_price: nullableNumber,
  quarter_price: nullableNumber,
  half_year_price: nullableNumber,
  year_price: nullableNumber,
  two_year_price: nullableNumber,
  three_year_price: nullableNumber,
  onetime_price: nullableNumber,
  reset_price: nullableNumber,
  capacity_limit: nullableNumber,
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * GET /user/subscription (docs/api-dialect.md §5.4, W5): bare body, boolean
 * `allow_new_period` (§4.1), RFC 3339 `expired_at` (§4.5), explicit-null
 * `plan` on the modern §5.5 shape. The `subscribe_url`/token scheme inside
 * stays frozen (§2).
 */
export const subscriptionSchema = z.looseObject({
  plan_id: nullableNumber,
  token: z.string(),
  expired_at: nullableString,
  u: z.number(),
  d: z.number(),
  transfer_enable: z.number(),
  device_limit: nullableNumber,
  email: z.string(),
  uuid: z.string(),
  plan: userPlanSchema.nullable(),
  alive_ip: z.number(),
  subscribe_url: z.string(),
  reset_day: nullableNumber,
  allow_new_period: z.boolean(),
});

/**
 * POST /user/subscription/reset-token (docs/api-dialect.md §9.4, W5): the
 * legacy bare URL string became a named object.
 */
export const resetSubscribeTokenSchema = z.looseObject({ subscribe_url: z.string() });

/**
 * One GET /user/sessions entry (docs/api-dialect.md §5.3/§9.4, W5): the
 * legacy digest-keyed map became an array carrying `session_id`; `login_at`
 * is RFC 3339 and the redacted `auth_data` filler died.
 */
export const activeSessionSchema = z.looseObject({
  session_id: z.string(),
  ip: z.string(),
  ua: z.string(),
  login_at: z.string(),
  current: z.boolean(),
});

/**
 * User order rows from GET /user/orders[/{trade_no}] (docs/api-dialect.md
 * §5.5, W4): RFC 3339 timestamps, nullable RFC 3339 `paid_at`;
 * `status`/`type`/`commission_status` stay numeric enums (§4.1).
 */
export const userOrderSchema = z.looseObject({
  trade_no: z.string(),
  callback_no: nullableString,
  plan_id: z.number(),
  period: z.union([planPeriodSchema, z.literal('deposit')]),
  type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(9)]),
  total_amount: z.number(),
  handling_amount: nullableNumber,
  discount_amount: nullableNumber,
  surplus_amount: nullableNumber,
  refund_amount: nullableNumber,
  balance_amount: nullableNumber,
  surplus_order_ids: z.array(z.number()).nullable(),
  status: z.union([z.literal(0), z.literal(1), z.literal(2), z.literal(3), z.literal(4)]),
  commission_status: z.union([z.literal(0), z.literal(1), z.literal(2), z.literal(3)]),
  commission_balance: z.number(),
  payment_id: nullableNumber,
  invite_user_id: nullableNumber,
  actual_commission_balance: nullableNumber.optional(),
  coupon_id: nullableNumber,
  paid_at: nullableString,
  created_at: z.string(),
  updated_at: z.string(),
  plan: z
    .union([userPlanSchema, z.looseObject({ id: z.literal(0), name: z.literal('deposit') })])
    .optional(),
  try_out_plan_id: z.number().optional(),
  bounus: z.number().optional(),
  get_amount: z.number().optional(),
});

export const userOrdersSchema = z.array(userOrderSchema);

/** POST /user/orders (§9.4): 201 with the created identity. */
export const createdOrderSchema = z.looseObject({
  trade_no: z.string().min(1),
});

/** GET /user/orders/{trade_no}/status (§9.4): the bare `{status}` body. */
export const orderStatusSchema = z.looseObject({
  status: z.number().int(),
});

/**
 * One payment-reconciliation row (docs/api-dialect.md §6.4, W11). Served both
 * as the `GET /{secure_path}/payment-reconciliations` page items and embedded
 * in the admin order detail's `payment_reconciliations[]`. `trade_no`/
 * `callback_no` ride raw alongside their server-side identity hashes;
 * `payment_name`/`payment_archived_at` appear only on the standalone list.
 */
export const reconciliationSchema = z.looseObject({
  id: z.number(),
  payment_id: z.number(),
  provider: z.string(),
  reason: z.string(),
  order_status: z.number(),
  expected_amount: z.number(),
  settled_amount: z.number(),
  occurrence_count: z.number(),
  trade_no: nullableString,
  trade_no_hash: z.string(),
  callback_no: nullableString,
  callback_no_hash: z.string(),
  resolution: nullableString,
  resolved_at: nullableString,
  first_seen_at: z.string(),
  last_seen_at: z.string(),
  payment_name: z.string().optional(),
  payment_archived_at: nullableString.optional(),
});

/** One commission-log entry embedded in the admin order detail (§6.4, W11). */
export const adminCommissionLogSchema = z.looseObject({
  id: z.number(),
  user_id: z.number(),
  invite_user_id: nullableNumber,
  trade_no: z.string(),
  order_amount: z.number(),
  get_amount: z.number(),
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * Admin order rows (docs/api-dialect.md §6.4, W11): RFC 3339 timestamps
 * (§4.5), numeric status/type enums (§4.1). The list row carries `email`,
 * `plan_name`, and `payment_reconciliation_open_count`; the detail carries
 * `commission_log[]` and `payment_reconciliations[]`.
 */
export const adminOrderSchema = z.looseObject({
  id: z.number(),
  user_id: z.number(),
  email: z.string().optional(),
  plan_name: nullableString.optional(),
  trade_no: z.string(),
  callback_no: nullableString,
  plan_id: z.number(),
  period: z.union([planPeriodSchema, z.literal('deposit')]),
  type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(9)]),
  total_amount: z.number(),
  handling_amount: nullableNumber,
  discount_amount: nullableNumber,
  surplus_amount: nullableNumber,
  refund_amount: nullableNumber,
  balance_amount: nullableNumber,
  surplus_order_ids: z.array(z.number()).nullable(),
  status: z.union([z.literal(0), z.literal(1), z.literal(2), z.literal(3), z.literal(4)]),
  commission_status: z.union([z.literal(0), z.literal(1), z.literal(2), z.literal(3)]),
  commission_balance: z.number(),
  payment_id: nullableNumber,
  invite_user_id: nullableNumber,
  actual_commission_balance: nullableNumber.optional(),
  coupon_id: nullableNumber,
  paid_at: nullableString,
  created_at: z.string(),
  updated_at: z.string(),
  payment_reconciliation_open_count: z.number().optional(),
  commission_log: z.array(adminCommissionLogSchema).optional(),
  payment_reconciliations: z.array(reconciliationSchema).optional(),
});

/** POST /user/orders/{trade_no}/checkout (§9.3, W4): the checkout result union. */
export const checkoutResultSchema = z.discriminatedUnion('kind', [
  z.looseObject({ kind: z.literal('qr_code'), payload: z.string() }),
  z.looseObject({ kind: z.literal('redirect'), url: z.string() }),
  z.looseObject({ kind: z.literal('settled') }),
]);

export const stripePaymentIntentSchema = z.looseObject({
  public_key: z.string().min(1),
  client_secret: z.string().min(1),
  amount: z.number().int().positive(),
  currency: z.string().regex(/^[a-z]{3}$/),
});

/**
 * GET /user/payment-methods (docs/api-dialect.md §5.5, W4):
 * `handling_fee_percent` is a JSON number (§4.1; the legacy route emitted
 * Eloquent's decimal string).
 */
export const paymentMethodSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  payment: z.string(),
  icon: nullableString,
  handling_fee_fixed: nullableNumber,
  handling_fee_percent: nullableNumber,
});

/**
 * Admin GET /{secure_path}/payments rows (docs/api-dialect.md §6.2, W11):
 * boolean `enable` (§4.1), RFC 3339 timestamps (§4.5), a numeric
 * `handling_fee_percent` (via `paymentMethodSchema`), and the server-redacted
 * `config` map. `legacy_md5_signature`/`security_warning` flag MD5 providers.
 */
export const adminPaymentSchema = paymentMethodSchema.extend({
  uuid: z.string(),
  config: z.record(z.string(), z.string()),
  notify_domain: nullableString,
  notify_url: z.string(),
  enable: z.boolean(),
  sort: nullableNumber,
  created_at: z.string(),
  updated_at: z.string(),
  legacy_md5_signature: z.boolean().optional(),
  security_warning: nullableString.optional(),
});

export const paymentFormFieldSchema = z.looseObject({
  label: z.string(),
  description: z.string().optional(),
  type: z.string().optional(),
  value: z.string().optional(),
});
export const paymentFormSchema = z.record(z.string(), paymentFormFieldSchema);

/**
 * GET /user/notices items (docs/api-dialect.md §5.8, W3) and, since W10, the
 * admin `GET /{secure_path}/notices` rows (§6.3 — the same field set on the
 * same modern value types): boolean `show`, RFC 3339 timestamps. `tags` keeps
 * carrying the backend `弹窗` auto-popup marker (Tier-1).
 */
export const noticeSchema = z.looseObject({
  id: z.number(),
  title: z.string(),
  content: z.string(),
  show: z.boolean(),
  img_url: nullableString,
  tags: stringArraySchema.nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * User ticket thread messages — dialect v2 (docs/api-dialect.md §5.7, W8):
 * RFC 3339 timestamps (§4.5), boolean `is_me`.
 */
export const userTicketMessageSchema = z.looseObject({
  id: z.number(),
  user_id: z.number(),
  ticket_id: z.number(),
  message: z.string(),
  is_me: z.boolean(),
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * User ticket rows — dialect v2 (§5.7, W8): `level`/`status`/`reply_status`
 * stay numeric enums (§4.1); `last_reply_user_id` is an always-present
 * nullable. The admin-prefix ticket family (§6.5, W14) shares this row shape;
 * only the list transport differs (`{items, total}` page instead of the
 * user-side bare array).
 */
export const userTicketSchema = z.looseObject({
  id: z.number(),
  user_id: z.number(),
  subject: z.string(),
  level: z.union([z.literal(0), z.literal(1), z.literal(2)]),
  status: binaryFlagSchema,
  reply_status: binaryFlagSchema,
  last_reply_user_id: nullableNumber,
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * GET /user/tickets/{id} and admin GET /{secure_path}/tickets/{id} (§5.7,
 * §6.5): the ticket row plus its `message[]` thread.
 */
export const userTicketDetailSchema = userTicketSchema.extend({
  message: z.array(userTicketMessageSchema),
});

/** POST /user/tickets + /user/withdrawal-tickets (§5.7): 201 with `{id}`. */
export const createdTicketSchema = z.looseObject({
  id: z.number(),
});

/**
 * GET /user/servers (docs/api-dialect.md §5.4, W6): bare array rows with
 * boolean `is_online`, numeric `rate`/`port` (§4.1), and RFC 3339
 * `last_check_at` (§4.5).
 */
export const availableServerSchema = z.looseObject({
  id: z.number(),
  parent_id: nullableNumber,
  group_id: numberArraySchema,
  route_id: numberArraySchema.nullable(),
  name: z.string(),
  rate: z.number(),
  type: z.enum(['shadowsocks', 'vmess', 'trojan', 'tuic', 'vless', 'hysteria', 'anytls', 'v2node']),
  host: z.string(),
  port: z.number(),
  cache_key: z.string(),
  last_check_at: nullableString,
  is_online: z.boolean(),
  tags: stringArraySchema.nullable().optional(),
});

/**
 * POST /user/coupons/check (docs/api-dialect.md §5.5, W4): bare coupon body,
 * boolean `show` (§4.1), RFC 3339 windows (§4.5); `type` stays the numeric
 * 1/2 enum. Admin `GET /{secure_path}/coupons` items (§6.3, W10) return the
 * same modern body, paged as `{items, total}`.
 */
export const userCouponSchema = z.looseObject({
  id: z.number(),
  code: z.string(),
  name: z.string(),
  type: z.union([z.literal(1), z.literal(2)]),
  value: z.number(),
  show: z.boolean(),
  limit_use: nullableNumber,
  limit_use_with_user: nullableNumber,
  limit_plan_ids: numberArraySchema.nullable(),
  limit_period: stringArraySchema.nullable(),
  started_at: z.string(),
  ended_at: z.string(),
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * Admin `GET /{secure_path}/gift-cards` items (docs/api-dialect.md §6.3,
 * W10): RFC 3339 windows and a real `used_user_ids` array of redeemer ids.
 */
export const giftcardSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  code: z.string(),
  type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(5)]),
  value: nullableNumber,
  plan_id: nullableNumber,
  limit_use: nullableNumber,
  used_user_ids: numberArraySchema,
  started_at: z.string(),
  ended_at: z.string(),
  created_at: z.string(),
  updated_at: z.string(),
});

/**
 * Invite & commission family (docs/api-dialect.md §5.6, §9.2, W7): bare
 * `{codes, stat}` with the named stat object (was the legacy 5-tuple),
 * RFC 3339 timestamps, and integer-cents commissions (the `amount/100`
 * display math keeps reading cents).
 */
export const inviteCodeSchema = z.looseObject({
  id: z.number(),
  code: z.string(),
  pv: z.number(),
  created_at: z.string(),
  updated_at: z.string(),
});
export const inviteStatSchema = z.looseObject({
  registered_count: z.number(),
  valid_commission: z.number(),
  pending_commission: z.number(),
  commission_rate: z.number(),
  available_commission: z.number(),
});
export const inviteFetchSchema = z.looseObject({
  codes: z.array(inviteCodeSchema),
  stat: inviteStatSchema,
});
export const commissionDetailSchema = z.looseObject({
  id: z.number(),
  trade_no: z.string(),
  order_amount: z.number(),
  get_amount: z.number(),
  created_at: z.string(),
});

/**
 * User knowledge rows (docs/api-dialect.md §5.8, W3): boolean `show` and an
 * RFC 3339 `updated_at`. The detail `body` stays non-idempotent —
 * re-substituted per request (Tier-1 refetch behavior). Since W10 the admin
 * `GET /{secure_path}/knowledge` rows (§6.3) reuse the same shapes — the
 * admin detail differs only in serving the raw stored body.
 */
export const knowledgeSummarySchema = z.looseObject({
  id: z.number(),
  category: z.string(),
  title: z.string(),
  sort: nullableNumber,
  show: z.boolean(),
  updated_at: z.string(),
});
export const knowledgeSchema = knowledgeSummarySchema.extend({
  body: z.string(),
  language: z.string(),
  created_at: z.string(),
});
export const knowledgeCategorySchema = z.record(z.string(), z.array(knowledgeSummarySchema));

/**
 * GET /user/traffic-logs (docs/api-dialect.md §5.4, W6): numeric
 * `server_rate` (§4.1) and RFC 3339 `record_at` (§4.5 — still the
 * period-start marker).
 */
export const trafficLogSchema = z.looseObject({
  u: z.number(),
  d: z.number(),
  record_at: z.string(),
  user_id: z.number(),
  server_rate: z.number(),
});

/**
 * GET /user/config (docs/api-dialect.md §5.3, W3): bare body, boolean flags,
 * an always-array `withdraw_methods`, and numeric commission distribution
 * rates (the legacy string-vs-number split died with the flip). Keeps its
 * historical `comm` name so call sites stay stable.
 */
export const userCommConfigSchema = z.looseObject({
  is_telegram: z.boolean(),
  telegram_discuss_link: nullableString,
  withdraw_methods: stringArraySchema,
  withdraw_close: z.boolean(),
  currency: z.string(),
  currency_symbol: z.string(),
  commission_distribution_enable: z.boolean(),
  commission_distribution_l1: nullableNumber,
  commission_distribution_l2: nullableNumber,
  commission_distribution_l3: nullableNumber,
});

/** GET /user/stats (docs/api-dialect.md §9.1, W5): the named-count object. */
export const userStatsSchema = z.looseObject({
  pending_order_count: z.number(),
  pending_ticket_count: z.number(),
  invited_user_count: z.number(),
});
export const telegramBotInfoSchema = z.looseObject({ username: z.string() });

export const adminUserBaseSchema = z.looseObject({
  id: z.number(),
  email: z.string(),
  password: z.string(),
  balance: z.number(),
  commission_balance: z.number(),
  transfer_enable: z.number(),
  device_limit: nullableNumber,
  u: z.number(),
  d: z.number(),
  plan_id: nullableNumber,
  group_id: nullableNumber,
  // §6.6 (W12): the modern admin user projection emits RFC 3339 UTC strings
  // for every epoch field (`created_at`/`updated_at` always present; the
  // nullable `expired_at`/`last_login_at` stay null when unset) and drops the
  // `t`/`password_algo`/`password_salt`/`last_login_ip` columns.
  expired_at: nullableString,
  uuid: z.string(),
  token: z.string(),
  banned: binaryFlagSchema,
  is_admin: binaryFlagSchema,
  is_staff: binaryFlagSchema,
  invite_user_id: nullableNumber,
  invite_user_email: nullableString.optional(),
  discount: nullableNumber,
  commission_type: z.union([z.literal(0), z.literal(1), z.literal(2), z.null()]).optional(),
  commission_rate: nullableNumber,
  speed_limit: nullableNumber.optional(),
  remarks: nullableString.optional(),
  telegram_id: nullableNumber,
  last_login_at: nullableString,
  created_at: z.string(),
  updated_at: z.string(),
});
export const adminUserSchema = adminUserBaseSchema.extend({
  total_used: z.number(),
  alive_ip: z.number(),
  ips: z.string(),
  plan_name: nullableString.optional(),
  subscribe_url: z.string(),
});
export const adminUserDetailSchema = adminUserBaseSchema.extend({
  invite_user: z.looseObject({ email: z.string().optional() }).nullable().optional(),
});

/**
 * GET /{secure_path}/stats/user-traffic rows (docs/api-dialect.md §6.8, W14):
 * RFC 3339 `record_at` (§4.5) and a real JSON-number `server_rate`.
 */
export const adminUserTrafficSchema = z.looseObject({
  record_at: z.string(),
  u: z.number(),
  d: z.number(),
  server_rate: z.number(),
});

/** GET /{secure_path}/stats/summary (§6.8, W14): bare object, integer-cent money. */
export const adminStatSummarySchema = z.looseObject({
  online_user: z.number().optional(),
  month_income: z.number(),
  month_register_total: z.number(),
  day_register_total: z.number().optional(),
  ticket_pending_total: z.number(),
  commission_pending_total: z.number(),
  payment_reconciliation_pending_total: z.number().optional(),
  payment_reconciliation_pending_amount: z.number().optional(),
  day_income: z.number(),
  last_month_income: z.number(),
  commission_month_payout: z.number(),
  commission_last_month_payout: z.number(),
});
/** GET /{secure_path}/stats/server-rank `?window=` rows (§6.8, W14). */
export const serverRankSchema = z.looseObject({
  server_id: z.number(),
  server_type: z.string(),
  server_name: nullableString,
  u: z.number(),
  d: z.number(),
  total: z.number(),
});
/** GET /{secure_path}/stats/user-rank `?window=` rows (§6.8, W14). */
export const userRankSchema = z.looseObject({
  user_id: z.number(),
  email: z.string(),
  u: z.number(),
  d: z.number(),
  total: z.number(),
});
/**
 * GET /{secure_path}/stats/{orders,records} rows (§6.8, W14): stable
 * snake_case `series` slugs and integer-cent money values — the legacy
 * Chinese `type` literals and yuan floats died with the wave.
 */
export const statSeriesPointSchema = z.looseObject({
  series: z.string(),
  date: z.string(),
  value: z.number(),
});

/**
 * GET /{secure_path}/system/queue-stats (docs/api-dialect.md §6.1, W9): bare
 * snake_case worker counters with RFC 3339 timestamp maps.
 */
export const queueStatsSchema = z.looseObject({
  failed_jobs: z.number(),
  jobs_per_minute: z.number(),
  paused_masters: z.number(),
  periods: z.looseObject({ failed_jobs: z.number(), recent_jobs: z.number() }),
  processes: z.number(),
  queue_with_max_runtime: nullableString,
  queue_with_max_throughput: nullableString,
  recent_jobs: z.number(),
  status: z.boolean(),
  wait: z.record(z.string(), z.number()),
  last_run_at: z.record(z.string(), z.string()),
  last_success_at: z.record(z.string(), z.string()),
  last_failure_at: z.record(z.string(), z.string()),
});
/** GET /{secure_path}/system/queue-workload row (§6.1, W9): bare snake_case. */
export const queueWorkloadSchema = z.looseObject({
  name: z.string(),
  processes: z.number(),
  length: z.number(),
  wait: z.number(),
  recent_jobs: z.number(),
  failed_jobs: z.number(),
  last_run_at: nullableString,
  last_success_at: nullableString,
  last_failure_at: nullableString,
});
/**
 * GET /{secure_path}/system/logs item (docs/api-dialect.md §6.1, W9): the
 * legacy key set with §4.5 RFC 3339 `created_at`/`updated_at`. The route is
 * the §7 filter/sort DSL's first consumer (whitelist: `level` only).
 */
export const systemLogSchema = z.looseObject({
  id: z.number(),
  title: z.string(),
  level: nullableString,
  host: nullableString,
  uri: z.string(),
  method: z.string(),
  data: nullableString,
  ip: nullableString,
  context: nullableString,
  created_at: z.string(),
  updated_at: z.string(),
});
export const serverTypeNameSchema = z.enum([
  'shadowsocks',
  'vmess',
  'trojan',
  'tuic',
  'vless',
  'hysteria',
  'anytls',
  'v2node',
]);

/**
 * GET /{secure_path}/nodes row (docs/api-dialect.md §6.7, W13): the dialect-v2
 * projection — boolean `show`, numeric `rate`/`port`, integer id arrays,
 * RFC 3339 timestamps, and the health/credential fields the step-up-gated read
 * always attaches. Protocol-specific columns (cipher, tls, the R22 camelCase
 * vmess settings keys, …) ride through the loose object as stored.
 */
export const serverNodeSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  group_id: numberArraySchema,
  route_id: numberArraySchema.nullable(),
  type: serverTypeNameSchema,
  host: z.string(),
  port: z.number(),
  server_port: nullableNumber,
  show: z.boolean(),
  rate: z.number(),
  parent_id: nullableNumber,
  online: z
    .number()
    .nullable()
    .transform((value) => value ?? 0),
  last_check_at: nullableString,
  last_push_at: nullableString,
  available_status: z.union([z.literal(0), z.literal(1), z.literal(2)]),
  api_key: nullableString,
  install_command: z.string().optional(),
});
/** GET /{secure_path}/server-groups row (§6.7, W13): always enriched with
 * `user_count`/`server_count`; §4.5 RFC 3339 timestamps. */
export const serverGroupSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  user_count: z.number(),
  server_count: z.number(),
  created_at: z.string(),
  updated_at: z.string(),
});
export const serverRouteActionSchema = z.enum([
  'block',
  'block_ip',
  'block_port',
  'protocol',
  'dns',
  'route',
  'route_ip',
  'default_out',
]);
/** GET /{secure_path}/server-routes row (§6.7, W13): `match` is always a real
 * array on the dialect-v2 wire; §4.5 RFC 3339 timestamps. */
export const serverRouteSchema = z.looseObject({
  id: z.number(),
  remarks: z.string(),
  match: stringArraySchema,
  action: serverRouteActionSchema,
  action_value: nullableString,
  created_at: z.string(),
  updated_at: z.string(),
});

// GET `/{secure_path}/config` (docs/api-dialect.md §6.1, W9): §4.1 native
// JSON types — real booleans for every config flag, real string arrays, JSON
// numbers. The legacy 0/1-flag, comma-list-string, and number-as-string
// tolerances died with the W9 flip.
const configFlagSchema = z.boolean();
const configTicketStatusSchema = z.union([z.literal(0), z.literal(1), z.literal(2)]);
const configResetTrafficSchema = z.union([
  z.literal(0),
  z.literal(1),
  z.literal(2),
  z.literal(3),
  z.literal(4),
]);
// PostgreSQL authority emits exact decimals as strings (§4.1 recorded
// exception, `commission_withdraw_limit` only). Keep their lexical value
// instead of round-tripping through IEEE-754 in the admin form.
const configDecimalStringSchema = z
  .string()
  .trim()
  .regex(/^-?(?:\d+\.?\d*|\.\d+)$/);
const configNullableStringSchema = z.union([z.string(), z.null()]);

const ticketConfigSchema = z.looseObject({
  ticket_status: configTicketStatusSchema,
});
const depositConfigSchema = z.looseObject({
  deposit_bounus: stringArraySchema,
});
const inviteConfigSchema = z.looseObject({
  invite_force: configFlagSchema,
  invite_commission: z.number(),
  invite_gen_limit: z.number(),
  invite_never_expire: configFlagSchema,
  commission_first_time_enable: configFlagSchema,
  commission_auto_check_enable: configFlagSchema,
  commission_withdraw_limit: configDecimalStringSchema,
  commission_withdraw_method: stringArraySchema,
  withdraw_close_enable: configFlagSchema,
  commission_distribution_enable: configFlagSchema,
  commission_distribution_l1: nullableNumber,
  commission_distribution_l2: nullableNumber,
  commission_distribution_l3: nullableNumber,
});
const siteConfigSchema = z.looseObject({
  logo: configNullableStringSchema,
  force_https: configFlagSchema,
  stop_register: configFlagSchema,
  app_name: z.string(),
  app_description: configNullableStringSchema,
  app_url: configNullableStringSchema,
  subscribe_url: configNullableStringSchema,
  subscribe_path: configNullableStringSchema,
  try_out_plan_id: z.number(),
  try_out_hour: z.number(),
  tos_url: configNullableStringSchema,
  currency: z.string(),
  currency_symbol: z.string(),
  // docs/api-dialect.md §10.3: the legacy `#/…` hash → history-URL toggle.
  legacy_hash_redirect_enable: configFlagSchema,
});
const subscribeConfigSchema = z.looseObject({
  plan_change_enable: configFlagSchema,
  reset_traffic_method: configResetTrafficSchema,
  surplus_enable: configFlagSchema,
  allow_new_period: configFlagSchema,
  new_order_event_id: configFlagSchema,
  renew_order_event_id: configFlagSchema,
  change_order_event_id: configFlagSchema,
  show_info_to_server_enable: configFlagSchema,
  show_subscribe_method: configTicketStatusSchema,
  show_subscribe_expire: z.number(),
});
const frontendConfigSchema = z.looseObject({
  frontend_theme_color: z.enum(['default', 'darkblue', 'black', 'green']),
  frontend_background_url: configNullableStringSchema,
  // docs/api-dialect.md §10.6: typed chat-widget integration (custom_html is
  // removed).
  chat_widget_provider: configNullableStringSchema,
  chat_widget_crisp_website_id: configNullableStringSchema,
  chat_widget_tawk_property_id: configNullableStringSchema,
  chat_widget_tawk_widget_id: configNullableStringSchema,
});
const serverConfigSchema = z.looseObject({
  server_api_url: configNullableStringSchema,
  server_token: configNullableStringSchema,
  server_pull_interval: z.number(),
  server_push_interval: z.number(),
  server_node_report_min_traffic: z.number(),
  server_device_online_min_traffic: z.number(),
  device_limit_mode: configFlagSchema,
});
const emailConfigSchema = z.looseObject({
  email_template: z
    .string()
    .nullable()
    .transform((value) => value ?? 'default'),
  email_host: configNullableStringSchema,
  email_port: nullableNumber,
  email_username: configNullableStringSchema,
  email_password: configNullableStringSchema,
  email_encryption: configNullableStringSchema,
  email_from_address: configNullableStringSchema,
});
const telegramConfigSchema = z.looseObject({
  telegram_bot_enable: configFlagSchema,
  telegram_bot_token: configNullableStringSchema,
  telegram_discuss_link: configNullableStringSchema,
});
const appConfigSchema = z.looseObject({
  windows_version: configNullableStringSchema,
  windows_download_url: configNullableStringSchema,
  macos_version: configNullableStringSchema,
  macos_download_url: configNullableStringSchema,
  android_version: configNullableStringSchema,
  android_download_url: configNullableStringSchema,
});
const safeConfigSchema = z.looseObject({
  email_verify: configFlagSchema,
  safe_mode_enable: configFlagSchema,
  secure_path: configNullableStringSchema,
  email_whitelist_enable: configFlagSchema,
  email_whitelist_suffix: stringArraySchema,
  email_gmail_limit_enable: configFlagSchema,
  recaptcha_enable: configFlagSchema,
  recaptcha_key: configNullableStringSchema,
  recaptcha_site_key: configNullableStringSchema,
  register_limit_by_ip_enable: configFlagSchema,
  register_limit_count: z.number(),
  register_limit_expire: z.number(),
  password_limit_enable: configFlagSchema,
  password_limit_count: z.number(),
  password_limit_expire: z.number(),
});

/**
 * Config fetch is intentionally partial: `?key=` returns one complete group.
 * Every stable field is still parsed whenever its group is present.
 */
export const adminConfigSchema = z.looseObject({
  ticket: ticketConfigSchema.optional(),
  deposit: depositConfigSchema.optional(),
  invite: inviteConfigSchema.optional(),
  site: siteConfigSchema.optional(),
  subscribe: subscribeConfigSchema.optional(),
  frontend: frontendConfigSchema.optional(),
  server: serverConfigSchema.optional(),
  email: emailConfigSchema.optional(),
  telegram: telegramConfigSchema.optional(),
  app: appConfigSchema.optional(),
  safe: safeConfigSchema.optional(),
}) satisfies z.ZodType<AdminConfig>;

/**
 * POST /{secure_path}/test-mail (docs/api-dialect.md §6.1, W9): the legacy
 * `{data: true, log}` envelope became this bare named object. The native
 * probe is synchronous; failures surface as problems, so `log` is null on
 * success.
 */
export const testMailResultSchema = z.looseObject({
  sent: z.boolean(),
  log: nullableString,
});

/**
 * PATCH /{secure_path}/config (docs/api-dialect.md §6.1, W9): the only 202 in
 * the dialect — a durable-but-not-yet-active write. Full activation is a
 * bodiless 204.
 */
export const configActivationPendingSchema = z.looseObject({
  activation: z.literal('pending'),
});

/**
 * POST /user/gift-card-redemptions (docs/api-dialect.md §9.4, W5): the legacy
 * `{data: true, type, value}` envelope extras became this bare named object.
 */
export const giftCardRedemptionSchema = z.looseObject({
  type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(5)]),
  value: nullableNumber,
});

/**
 * The §1 bare 201 `{id}` create result (dialect v2 upsert splits, W10). On
 * the CSV-capable §6.3 bulk-generate routes it is the JSON arm — a bulk run
 * streams the byte-frozen CSV attachment instead.
 */
export const createdIdSchema = z.looseObject({
  id: z.number(),
  /** Keeps the JSON side of `BinaryApiResponse` structurally distinct. */
  buffer: z.never().optional(),
});
