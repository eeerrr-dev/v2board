import { z } from 'zod';
import type { AdminConfig } from '@v2board/types';

const nullableNumber = z.number().nullable();
const nullableString = z.string().nullable();
const binaryFlagSchema = z.union([z.literal(0), z.literal(1)]);
const numericStringSchema = z
  .string()
  .trim()
  .regex(/^-?(?:\d+\.?\d*|\.\d+)$/)
  .transform(Number);
const numberFromApiSchema = z.union([z.number(), numericStringSchema]);

export const trueSchema = z.literal(true);
export const booleanSchema = z.boolean();
export const stringSchema = z.string();
export const numberSchema = z.number();
export const stringArraySchema = z.array(stringSchema);
export const numberArraySchema = z.array(numberSchema);

export const adminFilterSchema = z.object({
  key: z.string(),
  condition: z.string(),
  value: z.union([z.string(), z.number(), z.null()]),
});

/** Dynamic JSON is allowed at a leaf, never as an endpoint-level bypass. */
export const jsonValueSchema = z.json();
export const jsonObjectSchema = z.record(z.string(), jsonValueSchema);

export const arraySchema = <TSchema extends z.ZodType>(item: TSchema) => z.array(item);

/** The client adds `code` before this schema runs when the backend JSON omitted it. */
export const envelopeSchema = <TDataSchema extends z.ZodType>(data: TDataSchema) =>
  z.looseObject({
    code: z.number(),
    data,
    total: z.number().optional(),
    type: z.number().optional(),
    message: z.string().optional(),
  });

export const pageEnvelopeSchema = <TItemSchema extends z.ZodType>(item: TItemSchema) =>
  envelopeSchema(z.array(item)).extend({ total: z.number().optional() });

export const authDataSchema = z.looseObject({
  token: z.string().min(1),
  is_admin: binaryFlagSchema,
  auth_data: z.string().min(1),
});

export const nullableAuthDataSchema = authDataSchema.nullable();

export const checkLoginSchema = z.looseObject({
  is_login: z.boolean(),
  is_admin: z.boolean().optional(),
});

export const guestConfigSchema = z.looseObject({
  tos_url: nullableString,
  is_email_verify: binaryFlagSchema,
  is_invite_force: binaryFlagSchema,
  email_whitelist_suffix: z.union([stringArraySchema, z.literal(0)]),
  is_recaptcha: binaryFlagSchema,
  recaptcha_site_key: nullableString,
  app_description: nullableString,
  app_url: nullableString,
  logo: nullableString,
});

export const userInfoSchema = z.looseObject({
  email: z.string(),
  transfer_enable: z.number(),
  device_limit: nullableNumber,
  last_login_at: nullableNumber,
  created_at: z.number(),
  banned: binaryFlagSchema,
  auto_renewal: binaryFlagSchema,
  remind_expire: binaryFlagSchema,
  remind_traffic: binaryFlagSchema,
  expired_at: nullableNumber,
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
  show: binaryFlagSchema,
  sort: nullableNumber,
  renew: binaryFlagSchema,
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
  created_at: z.number(),
  updated_at: z.number(),
});

export const subscribeInfoSchema = z.looseObject({
  plan_id: nullableNumber,
  token: z.string(),
  expired_at: nullableNumber,
  u: z.number(),
  d: z.number(),
  transfer_enable: z.number(),
  device_limit: nullableNumber,
  email: z.string(),
  uuid: z.string(),
  plan: planSchema.optional(),
  alive_ip: z.number(),
  subscribe_url: z.string(),
  reset_day: nullableNumber,
  allow_new_period: binaryFlagSchema,
});

export const activeSessionSchema = z.looseObject({
  ip: z.string(),
  login_at: z.number(),
  ua: z.string(),
  auth_data: z.string(),
  current: z.boolean(),
});
export const activeSessionMapSchema = z.record(z.string(), activeSessionSchema);

export const orderSchema = z.looseObject({
  trade_no: z.string(),
  callback_no: nullableString,
  plan_id: z.number(),
  period: z.union([planPeriodSchema, z.literal('deposit')]),
  type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(9)]),
  total_amount: z.number(),
  handling_amount: nullableNumber,
  pre_handling_amount: z.number().optional(),
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
  paid_at: nullableNumber,
  created_at: z.number(),
  updated_at: z.number(),
  plan: z
    .union([planSchema, z.looseObject({ id: z.literal(0), name: z.literal('deposit') })])
    .optional(),
  try_out_plan_id: z.number().optional(),
  bounus: z.number().optional(),
  get_amount: z.number().optional(),
});

export const ordersSchema = z.array(orderSchema);
export const adminOrderSchema = orderSchema.extend({
  id: z.number(),
  user_id: z.number(),
  email: z.string().optional(),
  plan_name: nullableString.optional(),
});

export const orderCheckoutDataSchema = z.union([z.string(), z.boolean()]);
// `type` is a payment-gateway discriminant, not a closed set: the checkout
// controller interprets 0/1/-1 and treats anything else as an unsupported
// gateway response instead of failing envelope validation.
export const checkoutEnvelopeSchema = envelopeSchema(orderCheckoutDataSchema).extend({
  type: z.number().int(),
});

export const stripePaymentIntentSchema = z.looseObject({
  public_key: z.string().min(1),
  client_secret: z.string().min(1),
  amount: z.number().int().positive(),
  currency: z.string().regex(/^[a-z]{3}$/),
});

export const paymentMethodSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  payment: z.string(),
  icon: nullableString,
  handling_fee_fixed: nullableNumber,
  // /user/order/getPaymentMethod serializes the NUMERIC column as its exact
  // decimal string (db/src/payment.rs `handling_fee_percent::text`).
  handling_fee_percent: numericStringSchema.nullable(),
});

export const adminPaymentSchema = paymentMethodSchema.extend({
  // /payment/fetch emits this field as a JSON number instead
  // (admin commerce CAST(handling_fee_percent AS DOUBLE PRECISION)).
  handling_fee_percent: nullableNumber,
  uuid: z.string(),
  config: z.record(z.string(), z.string()),
  notify_domain: nullableString,
  notify_url: z.string(),
  enable: binaryFlagSchema,
  sort: nullableNumber,
  created_at: z.number(),
  updated_at: z.number(),
});

export const paymentFormFieldSchema = z.looseObject({
  label: z.string(),
  description: z.string().optional(),
  type: z.string().optional(),
  value: z.string().optional(),
});
export const paymentFormSchema = z.record(z.string(), paymentFormFieldSchema);

export const noticeSchema = z.looseObject({
  id: z.number(),
  title: z.string(),
  content: z.string(),
  img_url: nullableString,
  tags: stringArraySchema.nullable(),
  show: binaryFlagSchema,
  created_at: z.number(),
  updated_at: z.number(),
});

export const ticketMessageSchema = z.looseObject({
  id: z.number(),
  user_id: z.number(),
  ticket_id: z.number(),
  message: z.string(),
  is_me: z.boolean(),
  created_at: z.number(),
  updated_at: z.number(),
});

export const ticketSchema = z.looseObject({
  id: z.number(),
  user_id: z.number(),
  subject: z.string(),
  level: z.union([z.literal(0), z.literal(1), z.literal(2)]),
  status: binaryFlagSchema,
  reply_status: binaryFlagSchema,
  last_reply_user_id: nullableNumber.optional(),
  created_at: z.number(),
  updated_at: z.number(),
  message: z.array(ticketMessageSchema).optional(),
});

export const availableServerSchema = z.looseObject({
  id: z.number(),
  parent_id: nullableNumber,
  group_id: numberArraySchema,
  route_id: numberArraySchema.nullable(),
  name: z.string(),
  rate: z.string(),
  type: z.enum(['shadowsocks', 'vmess', 'trojan', 'tuic', 'vless', 'hysteria', 'anytls', 'v2node']),
  host: z.string(),
  port: z.union([z.string(), z.number()]),
  cache_key: z.string(),
  last_check_at: nullableNumber,
  is_online: binaryFlagSchema,
  tags: stringArraySchema.nullable().optional(),
});

export const couponSchema = z.looseObject({
  id: z.number(),
  code: z.string(),
  name: z.string(),
  type: z.union([z.literal(1), z.literal(2)]),
  value: z.number(),
  show: binaryFlagSchema,
  limit_use: nullableNumber,
  limit_use_with_user: nullableNumber,
  limit_plan_ids: numberArraySchema.nullable(),
  limit_period: stringArraySchema.nullable(),
  started_at: z.number(),
  ended_at: z.number(),
  created_at: z.number(),
  updated_at: z.number(),
});

export const giftcardSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  code: z.string(),
  type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(5)]),
  value: nullableNumber,
  plan_id: nullableNumber,
  limit_use: nullableNumber,
  used_user_ids: z.union([nullableString, z.array(z.union([z.number(), z.string()]))]),
  started_at: nullableNumber,
  ended_at: nullableNumber,
  created_at: z.number(),
  updated_at: z.number(),
});

export const inviteCodeSchema = z.looseObject({
  id: z.number(),
  user_id: z.number(),
  code: z.string(),
  status: binaryFlagSchema,
  pv: z.number(),
  created_at: z.number(),
  updated_at: z.number(),
});
export const inviteStatSchema = z.tuple([
  z.number(),
  z.number(),
  z.number(),
  z.number(),
  z.number(),
]);
export const inviteFetchSchema = z.looseObject({
  codes: z.array(inviteCodeSchema),
  stat: inviteStatSchema,
});
export const commissionDetailSchema = z.looseObject({
  id: z.number(),
  trade_no: z.string(),
  order_amount: z.number(),
  get_amount: z.number(),
  created_at: z.number(),
});

export const knowledgeSummarySchema = z.looseObject({
  id: z.number(),
  category: z.string(),
  title: z.string(),
  updated_at: z.number(),
});
export const knowledgeSchema = knowledgeSummarySchema.extend({
  sort: nullableNumber,
  show: binaryFlagSchema,
  body: z.string(),
  language: z.string(),
  created_at: z.number(),
});
export const adminKnowledgeSummarySchema = knowledgeSummarySchema.extend({
  show: binaryFlagSchema,
});
export const knowledgeCategorySchema = z.record(z.string(), z.array(knowledgeSummarySchema));

export const trafficLogSchema = z.looseObject({
  u: z.number(),
  d: z.number(),
  record_at: z.number(),
  user_id: z.number(),
  server_rate: z.string(),
});

export const userCommConfigSchema = z.looseObject({
  is_telegram: binaryFlagSchema,
  telegram_discuss_link: nullableString,
  withdraw_methods: z.union([
    stringArraySchema,
    z.string().transform((value) => (value === '' ? [] : value.split(','))),
  ]),
  withdraw_close: binaryFlagSchema,
  currency: z.string(),
  currency_symbol: z.string(),
  commission_distribution_enable: binaryFlagSchema,
  commission_distribution_l1: z.union([z.string(), z.number(), z.null()]),
  commission_distribution_l2: z.union([z.string(), z.number(), z.null()]),
  commission_distribution_l3: z.union([z.string(), z.number(), z.null()]),
});

export const userStatTupleSchema = z.tuple([z.number(), z.number(), z.number()]);
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
  expired_at: nullableNumber,
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
  last_login_at: nullableNumber,
  created_at: z.number(),
  updated_at: z.number(),
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

export const adminUserTrafficSchema = z.looseObject({
  record_at: z.number(),
  u: z.number(),
  d: z.number(),
  server_rate: z.number(),
});

export const adminStatSummarySchema = z.looseObject({
  online_user: z.number().optional(),
  month_income: z.number(),
  month_register_total: z.number(),
  day_register_total: z.number().optional(),
  ticket_pending_total: z.number(),
  commission_pending_total: z.number(),
  day_income: z.number(),
  last_month_income: z.number(),
  commission_month_payout: z.number(),
  commission_last_month_payout: z.number(),
});
export const serverRankSchema = z.looseObject({
  server_id: z.number(),
  server_name: z.string(),
  total: z.number(),
});
export const userRankSchema = z.looseObject({
  user_id: z.number(),
  email: z.string(),
  total: z.number(),
});
export const orderStatSchema = z.looseObject({
  type: z.string(),
  date: z.string(),
  value: z.number(),
});

export const queueStatsSchema = z.looseObject({
  failedJobs: z.number(),
  jobsPerMinute: z.number(),
  pausedMasters: z.number(),
  periods: z.looseObject({ failedJobs: z.number(), recentJobs: z.number() }),
  processes: z.number(),
  queueWithMaxRuntime: nullableString,
  queueWithMaxThroughput: nullableString,
  recentJobs: z.number(),
  status: z.union([z.string(), z.boolean(), z.null()]),
  wait: z.record(z.string(), z.number()),
});
export const queueWorkloadSchema = z.looseObject({
  name: z.string(),
  processes: z.number(),
  length: z.number(),
  wait: z.number(),
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

export const serverNodeSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  group_id: z.union([numberArraySchema, stringArraySchema]),
  route_id: numberArraySchema.nullable(),
  type: serverTypeNameSchema,
  host: z.string(),
  port: z.union([z.number(), z.string()]),
  server_port: nullableNumber,
  show: z.union([binaryFlagSchema, z.string()]),
  rate: z.string(),
  parent_id: nullableNumber,
  online: z
    .number()
    .nullable()
    .transform((value) => value ?? 0),
  last_check_at: nullableNumber,
  is_online: binaryFlagSchema.optional(),
  available_status: z.union([z.literal(0), z.literal(1), z.literal(2)]).optional(),
});
export const serverGroupSchema = z.looseObject({
  id: z.number(),
  name: z.string(),
  user_count: z.number().optional(),
  server_count: z.number().optional(),
  created_at: z.number(),
  updated_at: z.number(),
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
export const serverRouteSchema = z.looseObject({
  id: z.number(),
  remarks: z.string(),
  match: z.union([stringArraySchema, z.string()]),
  action: serverRouteActionSchema,
  action_value: nullableString,
  created_at: z.number(),
  updated_at: z.number(),
});

const configNumberSchema = numberFromApiSchema;
const configFlagSchema = binaryFlagSchema;
const configTicketStatusSchema = z.union([z.literal(0), z.literal(1), z.literal(2)]);
const configResetTrafficSchema = z.union([
  z.literal(0),
  z.literal(1),
  z.literal(2),
  z.literal(3),
  z.literal(4),
]);
const configNullableNumberSchema = z.union([z.null(), configNumberSchema]);
// PostgreSQL authority emits exact decimals as strings. Keep their lexical
// value instead of round-tripping through IEEE-754 in the admin form.
const configDecimalStringSchema = z.union([
  z
    .string()
    .trim()
    .regex(/^-?(?:\d+\.?\d*|\.\d+)$/),
  z.number().transform(String),
]);
const configNullableDecimalStringSchema = configDecimalStringSchema.nullable();
const configNullableStringSchema = z.union([z.string(), z.null()]);
const configStringSchema = z.union([z.string(), z.number().transform(String)]);
const commaSeparatedStringArraySchema = z.union([
  stringArraySchema,
  z.string().transform((value) => (value === '' ? [] : value.split(','))),
]);

const ticketConfigSchema = z.looseObject({
  ticket_status: configTicketStatusSchema,
});
const depositConfigSchema = z.looseObject({
  deposit_bounus: commaSeparatedStringArraySchema,
});
const inviteConfigSchema = z.looseObject({
  invite_force: configFlagSchema,
  invite_commission: configNumberSchema,
  invite_gen_limit: configNumberSchema,
  invite_never_expire: configFlagSchema,
  commission_first_time_enable: configFlagSchema,
  commission_auto_check_enable: configFlagSchema,
  commission_withdraw_limit: configNullableDecimalStringSchema,
  commission_withdraw_method: commaSeparatedStringArraySchema,
  withdraw_close_enable: configFlagSchema,
  commission_distribution_enable: configFlagSchema,
  commission_distribution_l1: z.union([z.string(), z.number(), z.null()]),
  commission_distribution_l2: z.union([z.string(), z.number(), z.null()]),
  commission_distribution_l3: z.union([z.string(), z.number(), z.null()]),
});
const siteConfigSchema = z.looseObject({
  logo: configNullableStringSchema,
  force_https: configFlagSchema,
  stop_register: configFlagSchema,
  app_name: z.string(),
  app_description: z.string(),
  app_url: configNullableStringSchema,
  subscribe_url: configNullableStringSchema,
  subscribe_path: configNullableStringSchema,
  try_out_plan_id: configNullableNumberSchema,
  try_out_hour: configDecimalStringSchema,
  tos_url: configNullableStringSchema,
  currency: z.string(),
  currency_symbol: z.string(),
  // Older installations returned this safe-setting under `site`.
  email_whitelist_suffix: commaSeparatedStringArraySchema.optional(),
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
  show_subscribe_expire: configNullableNumberSchema,
});
const frontendConfigSchema = z.looseObject({
  frontend_theme_color: z.enum(['default', 'darkblue', 'black', 'green']),
  frontend_background_url: configNullableStringSchema,
  frontend_custom_html: configNullableStringSchema,
});
const serverConfigSchema = z.looseObject({
  server_api_url: configNullableStringSchema,
  server_token: configNullableStringSchema,
  server_pull_interval: configStringSchema,
  server_push_interval: configStringSchema,
  server_node_report_min_traffic: configStringSchema,
  server_device_online_min_traffic: configStringSchema,
  device_limit_mode: configFlagSchema,
});
const emailConfigSchema = z.looseObject({
  email_template: z
    .string()
    .nullable()
    .transform((value) => value ?? 'default'),
  email_host: configNullableStringSchema,
  email_port: z.union([z.string(), z.number().transform(String), z.null()]),
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
  email_whitelist_suffix: commaSeparatedStringArraySchema,
  email_gmail_limit_enable: configFlagSchema,
  recaptcha_enable: configFlagSchema,
  recaptcha_key: configNullableStringSchema,
  recaptcha_site_key: configNullableStringSchema,
  register_limit_by_ip_enable: configFlagSchema,
  register_limit_count: configNumberSchema,
  register_limit_expire: configNumberSchema,
  password_limit_enable: configFlagSchema,
  password_limit_count: configNumberSchema,
  password_limit_expire: configNumberSchema,
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

export const testMailLogSchema = z.looseObject({
  error: nullableString.optional(),
  email: z.string().optional(),
  subject: z.string().optional(),
  template_name: z.string().optional(),
  config: jsonObjectSchema.optional(),
});
export const testMailEnvelopeSchema = envelopeSchema(trueSchema).extend({
  log: testMailLogSchema.optional(),
});

export const redeemGiftCardEnvelopeSchema = envelopeSchema(trueSchema).extend({
  type: z.union([z.literal(1), z.literal(2), z.literal(3), z.literal(4), z.literal(5)]),
  value: nullableNumber,
});

export const csvJsonEnvelopeSchema = envelopeSchema(trueSchema).extend({
  /** Keeps the JSON side of `BinaryApiResponse` structurally distinct. */
  buffer: z.never().optional(),
});
