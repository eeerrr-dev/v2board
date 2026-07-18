import type {
  AdminConfig,
  AdminConfigFlat,
  AdminConfigPatchResult,
  AdminOrderRow,
  AdminUserRow,
  AdminUserUpdatePayload,
  AdminKnowledge,
  AdminNotice,
  Coupon,
  Giftcard,
  Plan,
  PlanPeriod,
  Ticket,
  TicketReplyPayload,
} from '@v2board/types';
import { z, type output, type ZodType } from 'zod';
import type { ApiClient, ApiRequestConfig, BinaryApiResponse } from '../client';
import type { serverRouteActionSchema, serverTypeNameSchema } from '../contracts';
import { adminListQueryParams, pageSchema, type AdminListQuery } from '../dialect';
import { decimalToCents, decimalToScaledInteger } from '../money';
import {
  adminConfigSchema,
  adminFilterSchema,
  configActivationPendingSchema,
  noContentSchema,
  systemLogSchema,
  testMailResultSchema,
  adminKnowledgeSchema,
  adminKnowledgeSummarySchema,
  adminNoticeSchema,
  adminOrderSchema,
  adminPaymentSchema,
  adminStatSummarySchema,
  adminUserDetailSchema,
  adminUserSchema,
  adminUserTrafficSchema,
  arraySchema,
  couponSchema,
  csvJsonEnvelopeSchema,
  giftcardSchema,
  orderStatSchema,
  pageEnvelopeSchema,
  paymentFormSchema,
  planSchema,
  queueStatsSchema,
  queueWorkloadSchema,
  serverGroupSchema,
  serverNodeSchema,
  serverRankSchema,
  serverRouteSchema,
  stringArraySchema,
  stringSchema,
  ticketSchema,
  trueSchema,
  userRankSchema,
} from '../contracts';

export const adminFilterArraySchema = arraySchema(adminFilterSchema);
export type AdminFilter = output<typeof adminFilterSchema>;

export interface AdminPageQuery {
  current?: number;
  pageSize?: number;
  sort?: string;
  sort_type?: 'ASC' | 'DESC';
  filter?: AdminFilter[];
}

export type AdminTestMailResult = output<typeof testMailResultSchema>;

export type AdminUserTrafficRecord = output<typeof adminUserTrafficSchema>;

export interface AdminUserTrafficQuery {
  user_id: number;
  page?: number;
  current?: number;
  pageSize?: number;
}

interface PageResult<T> {
  data: T[];
  total?: number;
}

type QueryRequestConfig = Pick<ApiRequestConfig, 'signal'>;
type MutationRequestConfig = Pick<ApiRequestConfig, 'signal' | 'headers'>;

const BYTES_PER_GIB = 1_073_741_824;

const adminGet = <TSchema extends ZodType>(
  client: ApiClient,
  path: string,
  responseSchema: TSchema,
  params?: Record<string, unknown>,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath(path),
    method: 'GET',
    params,
    responseSchema,
    ...config,
  });

const adminGetEnvelope = <TSchema extends ZodType>(
  client: ApiClient,
  path: string,
  responseSchema: TSchema,
  params?: Record<string, unknown>,
  config?: QueryRequestConfig,
) =>
  client.requestEnvelope({
    url: client.resolveAdminPath(path),
    method: 'GET',
    params,
    responseSchema,
    ...config,
  });

const adminPost = <TSchema extends ZodType>(
  client: ApiClient,
  path: string,
  responseSchema: TSchema,
  data?: unknown,
  config?: MutationRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath(path),
    method: 'POST',
    data,
    responseSchema,
    ...config,
  });

const adminPostTrue = (
  client: ApiClient,
  path: string,
  data?: unknown,
  config?: MutationRequestConfig,
) => adminPost(client, path, trueSchema, data, config);

const bulkMailIdempotencyKeys = new WeakMap<object, string>();

function mutationIdempotencyKey(payload: object): string {
  const existing = bulkMailIdempotencyKeys.get(payload);
  if (existing) return existing;
  const generated =
    globalThis.crypto?.randomUUID?.() ??
    `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  bulkMailIdempotencyKeys.set(payload, generated);
  return generated;
}

function normalizeAdminConfig(config: output<typeof adminConfigSchema>): AdminConfig {
  return {
    ...(config.ticket ?? {}),
    ...(config.deposit ?? {}),
    ...(config.invite ?? {}),
    ...(config.site ?? {}),
    ...(config.subscribe ?? {}),
    ...(config.frontend ?? {}),
    ...(config.server ?? {}),
    ...(config.email ?? {}),
    ...(config.telegram ?? {}),
    ...(config.app ?? {}),
    ...(config.safe ?? {}),
    ...config,
  };
}

/** GET /{secure_path}/config `?group=` — dialect v2 bare grouped object (§6.1, W9). */
export const fetchConfig = async (client: ApiClient, group?: string, config?: QueryRequestConfig) =>
  normalizeAdminConfig(
    await client.request({
      url: client.resolveAdminPath('/config'),
      method: 'GET',
      dialect: 'v2',
      params: group ? { group } : undefined,
      responseSchema: adminConfigSchema,
      ...config,
    }),
  );

/**
 * PATCH /{secure_path}/config — dialect v2 partial JSON body in §4.1 native
 * types (real booleans/arrays; the legacy `'[]'`-string empty-array hack is
 * dead) with §4.4 null-clear semantics. 204 means the write fully activated;
 * 202 `{activation: "pending"}` means it is durable but not yet active — the
 * caller must refetch, never resubmit (a resubmit would 409
 * `config_revision_conflict` on the now-stale revision).
 */
export const saveConfig = (
  client: ApiClient,
  data: Partial<AdminConfigFlat>,
): Promise<AdminConfigPatchResult> =>
  client
    .request({
      url: client.resolveAdminPath('/config'),
      method: 'PATCH',
      dialect: 'v2',
      data,
      responseSchema: z.union([noContentSchema, configActivationPendingSchema]),
    })
    .then((body) => ({ activation: body?.activation ?? 'applied' }));

/** GET /{secure_path}/email-templates — dialect v2 bare array (§6.1, W9). */
export const getEmailTemplate = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/email-templates'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: stringArraySchema,
    ...config,
  });

/** POST /{secure_path}/telegram-webhook — dialect v2, 204 (§6.1, W9). */
export const setTelegramWebhook = (client: ApiClient, telegram_bot_token?: string) =>
  client.request({
    url: client.resolveAdminPath('/telegram-webhook'),
    method: 'POST',
    dialect: 'v2',
    data: telegram_bot_token === undefined ? {} : { telegram_bot_token },
    responseSchema: noContentSchema,
  });

/**
 * POST /{secure_path}/test-mail — dialect v2 bare `{sent, log}` (§6.1, W9):
 * the legacy `{data: true, log}` envelope became a named object; failures are
 * problems (400 mail_sender_not_configured/mail_invalid, 502 mail_send_failed).
 */
export const testSendMail = (client: ApiClient) =>
  client.request({
    url: client.resolveAdminPath('/test-mail'),
    method: 'POST',
    dialect: 'v2',
    responseSchema: testMailResultSchema,
  });

const PLAN_PRICE_KEYS = [
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
] as const satisfies readonly (keyof Plan)[];

const PLAN_SAVE_KEYS = [
  'id',
  'name',
  'content',
  'group_id',
  'transfer_enable',
  'device_limit',
  ...PLAN_PRICE_KEYS,
  'reset_traffic_method',
  'capacity_limit',
  'speed_limit',
  'force_update',
] as const;

type AdminPlanField = Exclude<(typeof PLAN_SAVE_KEYS)[number], 'force_update'>;

export type AdminPlanSavePayload = {
  [K in AdminPlanField]?: Plan[K] | string | null;
} & {
  force_update?: boolean;
};

function normalizePlan(plan: Plan): Plan {
  const next = { ...plan };
  for (const key of PLAN_PRICE_KEYS) {
    const value = next[key];
    next[key] = value !== null ? ((Number(value) / 100) as Plan[typeof key]) : null;
  }
  return next;
}

function formatScaledBackendValue(value: unknown, divisor: number) {
  return (Number(value) / divisor).toFixed(2);
}

function normalizeAdminUser(
  user: output<typeof adminUserSchema>,
  { normalizeTotalUsed = false }: { normalizeTotalUsed?: boolean } = {},
): AdminUserRow {
  return {
    ...user,
    password: '',
    transfer_enable: formatScaledBackendValue(user.transfer_enable, BYTES_PER_GIB),
    u: formatScaledBackendValue(user.u, BYTES_PER_GIB),
    d: formatScaledBackendValue(user.d, BYTES_PER_GIB),
    plan_name: user.plan_name ?? null,
    total_used: normalizeTotalUsed
      ? formatScaledBackendValue(user.total_used, BYTES_PER_GIB)
      : user.total_used,
    commission_balance: formatScaledBackendValue(user.commission_balance, 100),
    balance: formatScaledBackendValue(user.balance, 100),
  };
}

function normalizeAdminUserDetail(user: output<typeof adminUserDetailSchema>) {
  return {
    ...user,
    password: '',
    transfer_enable: formatScaledBackendValue(user.transfer_enable, BYTES_PER_GIB),
    u: formatScaledBackendValue(user.u, BYTES_PER_GIB),
    d: formatScaledBackendValue(user.d, BYTES_PER_GIB),
    commission_balance: formatScaledBackendValue(user.commission_balance, 100),
    balance: formatScaledBackendValue(user.balance, 100),
    ...(user.invite_user?.email !== undefined ? { invite_user_email: user.invite_user.email } : {}),
  };
}

export const fetchPlans = async (client: ApiClient, config?: QueryRequestConfig) =>
  (await adminGet(client, '/plan/fetch', arraySchema(planSchema), undefined, config)).map(
    normalizePlan,
  );

export const savePlan = (client: ApiClient, data: AdminPlanSavePayload) =>
  adminPostTrue(client, '/plan/save', serializePlanForSave(data));

function serializePlanForSave(data: AdminPlanSavePayload): Record<string, unknown> {
  const next: Record<string, unknown> = {};
  for (const key of PLAN_SAVE_KEYS) {
    const value = data[key];
    if (value !== undefined) next[key] = value;
  }
  for (const key of PLAN_PRICE_KEYS) {
    const value = next[key];
    if (value === undefined) continue;
    if (value === null || value === '') {
      next[key] = null;
      continue;
    }
    next[key] = decimalToCents(value as string | number);
  }
  return next;
}

export const updatePlan = (client: ApiClient, id: number, key: 'show' | 'renew', value: 0 | 1) =>
  adminPostTrue(client, '/plan/update', { id, [key]: value });

export const dropPlan = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/plan/drop', { id });

export const sortPlans = (client: ApiClient, plan_ids: number[]) =>
  adminPostTrue(client, '/plan/sort', { plan_ids });

export const fetchUsers = async (
  client: ApiClient,
  query: AdminPageQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<AdminUserRow>> => {
  const env = await adminGetEnvelope(
    client,
    '/user/fetch',
    pageEnvelopeSchema(adminUserSchema),
    { ...query },
    config,
  );
  return {
    data: env.data.map((user) => normalizeAdminUser(user, { normalizeTotalUsed: true })),
    total: env.total,
  };
};

const ADMIN_USER_SCALED_FIELDS = {
  transfer_enable: BYTES_PER_GIB,
  u: BYTES_PER_GIB,
  d: BYTES_PER_GIB,
  balance: 100,
  commission_balance: 100,
} as const;

type AdminUserScaledField = keyof typeof ADMIN_USER_SCALED_FIELDS;

/** Display-unit input accepted by the Admin editor. Unit conversion belongs at
 * this API boundary so no page can accidentally send fractional bytes/cents. */
export type AdminUserUpdateInput = Omit<AdminUserUpdatePayload, AdminUserScaledField> & {
  [K in AdminUserScaledField]?: string | number | null;
};

function serializeAdminUserUpdate(data: AdminUserUpdateInput): AdminUserUpdatePayload {
  const payload = { ...data } as Record<string, unknown>;
  for (const [field, scale] of Object.entries(ADMIN_USER_SCALED_FIELDS)) {
    const value = data[field as AdminUserScaledField];
    if (value === undefined || value === null || value === '') {
      payload[field] = value;
    } else {
      payload[field] = decimalToScaledInteger(value, scale);
    }
  }
  return payload as unknown as AdminUserUpdatePayload;
}

export const updateUser = async (client: ApiClient, data: AdminUserUpdateInput) =>
  adminPostTrue(client, '/user/update', serializeAdminUserUpdate(data));

export const getUserInfoById = async (client: ApiClient, id: number, config?: QueryRequestConfig) =>
  normalizeAdminUserDetail(
    await adminGet(client, '/user/getUserInfoById', adminUserDetailSchema, { id }, config),
  );

export const generateUser = (
  client: ApiClient,
  data: {
    email_prefix?: string;
    email_suffix: string;
    password?: string;
    plan_id?: number | null;
    expired_at?: number | string | null;
    generate_count?: number | string;
  },
) =>
  client.requestBinary({
    url: client.resolveAdminPath('/user/generate'),
    method: 'POST',
    data,
    jsonResponseSchema: csvJsonEnvelopeSchema,
  });

export const sendMailToUsers = (
  client: ApiClient,
  data: { subject: string; content: string; filter?: AdminFilter[] },
) =>
  adminPostTrue(client, '/user/sendMail', data, {
    // TanStack invokes a mutation retry with the same variables object. Keeping
    // the key by object identity makes that retry replay the durable batch while
    // leaving the legacy form body byte-for-byte unchanged.
    headers: { 'Idempotency-Key': mutationIdempotencyKey(data) },
  });

export const dumpUsersCsv = (client: ApiClient, filter?: AdminFilter[]) =>
  client.requestBinary({
    url: client.resolveAdminPath('/user/dumpCSV'),
    method: 'POST',
    data: { filter },
    jsonResponseSchema: csvJsonEnvelopeSchema,
  });

export const banUsers = (client: ApiClient, filter?: AdminFilter[]) =>
  adminPostTrue(client, '/user/ban', { filter });

export const resetUserSecret = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/user/resetSecret', { id });

export const deleteUser = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/user/delUser', { id });

export const deleteAllUsers = (client: ApiClient, filter?: AdminFilter[]) =>
  adminPostTrue(client, '/user/allDel', { filter });

export const fetchOrders = async (
  client: ApiClient,
  query: AdminPageQuery & { is_commission?: 0 | 1 } = {},
  config?: QueryRequestConfig,
): Promise<PageResult<AdminOrderRow>> => {
  const env = await adminGetEnvelope(
    client,
    '/order/fetch',
    pageEnvelopeSchema(adminOrderSchema),
    { ...query },
    config,
  );
  return { data: env.data, total: env.total };
};

export const orderDetail = (client: ApiClient, id: number, config?: QueryRequestConfig) =>
  adminPost(client, '/order/detail', adminOrderSchema, { id }, config);

export const paidOrder = (client: ApiClient, trade_no: string) =>
  adminPostTrue(client, '/order/paid', { trade_no });

export const cancelOrder = (client: ApiClient, trade_no: string) =>
  adminPostTrue(client, '/order/cancel', { trade_no });

export const updateOrder = (
  client: ApiClient,
  trade_no: string,
  key: 'commission_status' | 'status',
  value: string | number,
) => adminPostTrue(client, '/order/update', { trade_no, [key]: value });

export const assignOrder = (
  client: ApiClient,
  data: {
    email: string;
    plan_id: number;
    period: PlanPeriod;
    total_amount: number | string;
  },
) =>
  adminPost(client, '/order/assign', stringSchema, {
    ...data,
    total_amount: data.total_amount === '' ? data.total_amount : decimalToCents(data.total_amount),
  });

export const fetchPayments = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/payment/fetch', arraySchema(adminPaymentSchema), undefined, config);

export const paymentMethods = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/payment/getPaymentMethods', stringArraySchema, undefined, config);

export interface SavePaymentPayload {
  id?: number;
  name: string;
  icon?: string | null;
  payment: string;
  config: Record<string, unknown>;
  notify_domain?: string | null;
  handling_fee_fixed?: string | number | null;
  handling_fee_percent?: string | number | null;
}

export const paymentForm = (
  client: ApiClient,
  payment?: string,
  id?: number,
  config?: QueryRequestConfig,
) => adminPost(client, '/payment/getPaymentForm', paymentFormSchema, { payment, id }, config);

const optionalPaymentFields = [
  'icon',
  'notify_domain',
  'handling_fee_fixed',
  'handling_fee_percent',
] as const;

function serializePaymentForSave(data: SavePaymentPayload): Record<string, unknown> {
  // Whitelist the fields PaymentController::save actually consumes. In
  // particular, never echo fetched uuid/enable/sort/timestamps/notify_url back
  // into an update just because a caller started from an AdminPayment object.
  const payload: Record<string, unknown> = {
    ...(data.id === undefined ? {} : { id: data.id }),
    name: data.name,
    payment: data.payment,
    config: data.config,
  };
  for (const key of optionalPaymentFields) {
    const value = data[key];
    if (value === undefined) continue;
    if (value === '') {
      // Missing optional fields are cleanest on create. On edit an explicit
      // null (form-encoded as an empty value, see serializeForm's 'empty'
      // mode) keeps the payload uniform with the coupon/giftcard editors,
      // where present-but-empty means "clear" and absent means "retain"
      // (values.rs coupon_field_values/giftcard_field_values gate columns on
      // contains_key). payment_save itself binds every optional column
      // unconditionally (commerce.rs UPDATE payment_method), so there absent
      // and cleared coincide and the explicit null is convention, not
      // load-bearing.
      if (data.id === undefined) continue;
      payload[key] = null;
      continue;
    }
    payload[key] = value;
  }
  if (payload.handling_fee_fixed != null) {
    payload.handling_fee_fixed = decimalToCents(payload.handling_fee_fixed as string | number);
  }
  return payload;
}

export const savePayment = (client: ApiClient, data: SavePaymentPayload) =>
  adminPostTrue(client, '/payment/save', serializePaymentForSave(data));

export const showPayment = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/payment/show', { id });

export const sortPayments = (client: ApiClient, payment_ids: number[]) =>
  adminPostTrue(client, '/payment/sort', { ids: payment_ids });

export const dropPayment = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/payment/drop', { id });

export const fetchNotices = async (
  client: ApiClient,
  _query: AdminPageQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<AdminNotice>> => {
  const env = await adminGetEnvelope(
    client,
    '/notice/fetch',
    pageEnvelopeSchema(adminNoticeSchema),
    undefined,
    config,
  );
  return { data: env.data, total: env.total };
};

export type SaveNoticePayload = Pick<AdminNotice, 'content' | 'img_url' | 'tags' | 'title'> & {
  id?: number;
};

export const saveNotice = (client: ApiClient, data: SaveNoticePayload) =>
  adminPostTrue(client, '/notice/save', data);

export const dropNotice = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/notice/drop', { id });

export const showNotice = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/notice/show', { id });

export const fetchTickets = async (
  client: ApiClient,
  query: AdminPageQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<Ticket>> => {
  const env = await adminGetEnvelope(
    client,
    '/ticket/fetch',
    pageEnvelopeSchema(ticketSchema),
    { ...query },
    config,
  );
  return { data: env.data, total: env.total };
};

export const ticketDetail = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  adminGet(client, '/ticket/fetch', ticketSchema, { id }, config);

export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) =>
  adminPostTrue(client, '/ticket/reply', payload);

export const closeTicket = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/ticket/close', { id });

export const fetchCoupons = async (
  client: ApiClient,
  query: AdminPageQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<Coupon>> => {
  const env = await adminGetEnvelope(
    client,
    '/coupon/fetch',
    pageEnvelopeSchema(couponSchema),
    { ...query },
    config,
  );
  env.data.forEach((coupon) => {
    if (coupon.type === 1) coupon.value = coupon.value / 100;
  });
  return { data: env.data, total: env.total };
};

export type GenerateCouponPayload = Omit<
  Partial<Coupon>,
  'value' | 'limit_use' | 'limit_use_with_user' | 'limit_plan_ids' | 'started_at' | 'ended_at'
> & {
  value?: number | string;
  limit_use?: number | string | null;
  limit_use_with_user?: number | string | null;
  limit_plan_ids?: Array<number | string> | null;
  started_at?: number | string | null;
  ended_at?: number | string | null;
  generate_count?: number | string;
};

export type GenerateCsvResponse = BinaryApiResponse<typeof csvJsonEnvelopeSchema>;

export const generateCoupon = (client: ApiClient, data: GenerateCouponPayload) =>
  client.requestBinary({
    url: client.resolveAdminPath('/coupon/generate'),
    method: 'POST',
    data: {
      ...data,
      value:
        data.type === 1 && data.value != null && data.value !== ''
          ? decimalToCents(data.value)
          : data.value,
    },
    jsonResponseSchema: csvJsonEnvelopeSchema,
  });

export const dropCoupon = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/coupon/drop', { id });

export const showCoupon = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/coupon/show', { id });

export const fetchGiftcards = async (
  client: ApiClient,
  query: AdminPageQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<Giftcard>> => {
  const env = await adminGetEnvelope(
    client,
    '/giftcard/fetch',
    pageEnvelopeSchema(giftcardSchema),
    { ...query },
    config,
  );
  env.data.forEach((giftcard) => {
    if (giftcard.type === 1 && giftcard.value !== null) giftcard.value /= 100;
  });
  return { data: env.data, total: env.total };
};

export type GenerateGiftcardPayload = Omit<
  Partial<Giftcard>,
  'value' | 'plan_id' | 'limit_use' | 'started_at' | 'ended_at'
> & {
  value?: number | string;
  plan_id?: number | string | null;
  limit_use?: number | string | null;
  started_at?: number | string | null;
  ended_at?: number | string | null;
  generate_count?: number | string;
};

export const generateGiftcard = (client: ApiClient, data: GenerateGiftcardPayload) =>
  client.requestBinary({
    url: client.resolveAdminPath('/giftcard/generate'),
    method: 'POST',
    data: {
      ...data,
      value:
        data.type === 1 && data.value != null && data.value !== ''
          ? decimalToCents(data.value)
          : data.value,
    },
    jsonResponseSchema: csvJsonEnvelopeSchema,
  });

export const dropGiftcard = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/giftcard/drop', { id });

export const fetchKnowledge = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/knowledge/fetch', arraySchema(adminKnowledgeSummarySchema), undefined, config);

export const knowledgeDetail = (client: ApiClient, id: number, config?: QueryRequestConfig) =>
  adminGet(client, '/knowledge/fetch', adminKnowledgeSchema, { id }, config);

export const knowledgeCategories = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/knowledge/getCategory', stringArraySchema, undefined, config);

export type SaveKnowledgePayload = Pick<AdminKnowledge, 'body' | 'category' | 'language' | 'title'> & {
  id?: number;
};

export const saveKnowledge = (client: ApiClient, data: SaveKnowledgePayload) =>
  adminPostTrue(client, '/knowledge/save', data);

export const showKnowledge = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/knowledge/show', { id });

export const dropKnowledge = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/knowledge/drop', { id });

export const sortKnowledge = (client: ApiClient, knowledge_ids: number[]) =>
  adminPostTrue(client, '/knowledge/sort', { knowledge_ids });

/** GET /{secure_path}/system/queue-stats — dialect v2 bare object (§6.1, W9). */
export const queueStats = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/system/queue-stats'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: queueStatsSchema,
    ...config,
  });

/** GET /{secure_path}/system/queue-workload — dialect v2 bare array (§6.1, W9). */
export const queueWorkload = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/system/queue-workload'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(queueWorkloadSchema),
    ...config,
  });

/** §7.1 — the GET system/logs filter whitelist (`level` only) and §7.2 sort columns. */
export const SYSTEM_LOG_FILTER_FIELDS = ['level'] as const;
export const SYSTEM_LOG_SORT_FIELDS = ['created_at', 'level'] as const;
export type SystemLogFilterField = (typeof SYSTEM_LOG_FILTER_FIELDS)[number];
export type AdminSystemLogRecord = output<typeof systemLogSchema>;

/**
 * GET /{secure_path}/system/logs — dialect v2 `{items, total}` page (§6.1,
 * W9) and the §7 filter/sort DSL's first consumer: clauses ride the single
 * JSON `filter` query param, sorting rides enum-validated
 * `sort_by`/`sort_dir`. No modern route parses legacy `filter[i][key]`
 * brackets.
 */
export const fetchSystemLogs = (
  client: ApiClient,
  query: AdminListQuery<SystemLogFilterField> = {},
  config?: QueryRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath('/system/logs'),
    method: 'GET',
    dialect: 'v2',
    params: adminListQueryParams(query),
    responseSchema: pageSchema(systemLogSchema),
    ...config,
  });

export const statSummary = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/stat/getOverride', adminStatSummarySchema, undefined, config);

export const statServerLastRank = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/stat/getServerLastRank', arraySchema(serverRankSchema), undefined, config);

export const statServerTodayRank = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/stat/getServerTodayRank', arraySchema(serverRankSchema), undefined, config);

export const statUserLastRank = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/stat/getUserLastRank', arraySchema(userRankSchema), undefined, config);

export const statUserTodayRank = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/stat/getUserTodayRank', arraySchema(userRankSchema), undefined, config);

export const statOrder = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/stat/getOrder', arraySchema(orderStatSchema), undefined, config);

export const statUser = async (
  client: ApiClient,
  query: AdminUserTrafficQuery,
  config?: QueryRequestConfig,
): Promise<PageResult<AdminUserTrafficRecord>> => {
  const env = await adminGetEnvelope(
    client,
    '/stat/getStatUser',
    pageEnvelopeSchema(adminUserTrafficSchema),
    { ...query },
    config,
  );
  return { data: env.data, total: env.total };
};

export type ServerNode = output<typeof serverNodeSchema>;

export const fetchServerNodes = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/server/manage/getNodes', arraySchema(serverNodeSchema), undefined, config);

export const sortServerNodes = (
  client: ApiClient,
  payload: Record<string, Record<string | number, number>>,
) =>
  client.request({
    url: client.resolveAdminPath('/server/manage/sort'),
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    data: JSON.stringify(payload),
    responseSchema: trueSchema,
  });

export type ServerGroup = output<typeof serverGroupSchema>;

export interface SaveServerGroupPayload {
  id?: number;
  name: string;
}

export const fetchServerGroups = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/server/group/fetch', arraySchema(serverGroupSchema), undefined, config);

export const saveServerGroup = (client: ApiClient, data: SaveServerGroupPayload) =>
  adminPostTrue(client, '/server/group/save', data);

export const dropServerGroup = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/server/group/drop', { id });

export type ServerRouteAction = output<typeof serverRouteActionSchema>;
export type ServerRoute = output<typeof serverRouteSchema>;

export interface SaveServerRoutePayload {
  id?: number;
  remarks: string;
  match: string[];
  action: ServerRouteAction;
  action_value: string | null;
}

export const fetchServerRoutes = (client: ApiClient, config?: QueryRequestConfig) =>
  adminGet(client, '/server/route/fetch', arraySchema(serverRouteSchema), undefined, config);

export const saveServerRoute = (client: ApiClient, data: SaveServerRoutePayload) =>
  adminPostTrue(client, '/server/route/save', data);

export const dropServerRoute = (client: ApiClient, id: number) =>
  adminPostTrue(client, '/server/route/drop', { id });

export type ServerTypeName = output<typeof serverTypeNameSchema>;

type ServerPayloadScalar = string | number;
type ServerPayloadBinary = 0 | 1 | '0' | '1';
type ServerPayloadSecurity = ServerPayloadBinary | 2 | '2';
type ServerJsonContainer = Record<string, unknown> | unknown[] | null;

/**
 * Exact public request surface shared by the eight server save endpoints.
 * Protocol-specific keys are optional because the endpoint path supplies the
 * discriminator, but arbitrary keys and response-only fields are rejected.
 */
export interface SaveServerPayload {
  id?: number;
  name: string;
  group_id: ServerPayloadScalar[];
  route_id?: ServerPayloadScalar[] | null;
  parent_id?: ServerPayloadScalar | null;
  host: string;
  port: ServerPayloadScalar;
  server_port: ServerPayloadScalar;
  tags?: string[] | null;
  rate: ServerPayloadScalar;
  show?: ServerPayloadBinary;
  sort?: ServerPayloadScalar | null;
  listen_ip?: string | null;
  protocol?: 'shadowsocks' | 'vmess' | 'vless' | 'trojan' | 'tuic' | 'hysteria2' | 'anytls';
  tls?: ServerPayloadSecurity;
  tls_settings?: ServerJsonContainer;
  tlsSettings?: ServerJsonContainer;
  network?: string;
  network_settings?: ServerJsonContainer;
  networkSettings?: ServerJsonContainer;
  ruleSettings?: ServerJsonContainer;
  dnsSettings?: ServerJsonContainer;
  flow?: 'xtls-rprx-vision' | null;
  encryption?: string | null;
  encryption_settings?: ServerJsonContainer;
  cipher?: string | null;
  obfs?: string | null;
  obfs_settings?: {
    path?: ServerPayloadScalar | null;
    host?: ServerPayloadScalar | null;
  } | null;
  obfs_password?: string | null;
  server_name?: string | null;
  allow_insecure?: ServerPayloadBinary;
  insecure?: ServerPayloadBinary;
  version?: 1 | 2 | '1' | '2';
  up_mbps?: ServerPayloadScalar | null;
  down_mbps?: ServerPayloadScalar | null;
  disable_sni?: ServerPayloadBinary;
  udp_relay_mode?: string | null;
  zero_rtt_handshake?: ServerPayloadBinary;
  congestion_control?: string | null;
  padding_scheme?: string | null;
}

export interface SaveServerRequest {
  type: ServerTypeName;
  data: SaveServerPayload;
}

export const saveServer = (client: ApiClient, type: ServerTypeName, data: SaveServerPayload) =>
  adminPostTrue(client, `/server/${type}/save`, data);

export const dropServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  adminPostTrue(client, `/server/${type}/drop`, { id });

export const updateServer = (
  client: ApiClient,
  type: ServerTypeName,
  id: number,
  key: 'show',
  value: 0 | 1,
) => adminPostTrue(client, `/server/${type}/update`, { id, [key]: value });

export const copyServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  adminPostTrue(client, `/server/${type}/copy`, { id });
