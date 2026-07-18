import type {
  AdminConfig,
  AdminConfigFlat,
  AdminConfigPatchResult,
  AdminOrderRow,
  AdminUserRow,
  AdminUserUpdatePayload,
  Coupon,
  Giftcard,
  Knowledge,
  Notice,
  Plan,
  PlanPeriod,
  Ticket,
  TicketReplyPayload,
} from '@v2board/types';
import { z, type output } from 'zod';
import type { ApiClient, ApiRequestConfig, BinaryApiResponse } from '../client';
import type { serverRouteActionSchema, serverTypeNameSchema } from '../contracts';
import {
  adminListQueryParams,
  pageSchema,
  type AdminListQuery,
  type FilterClause,
  type FilterOp,
} from '../dialect';
import { decimalToCents, decimalToScaledInteger } from '../money';
import {
  adminConfigSchema,
  adminTicketDetailSchema,
  adminTicketSchema,
  configActivationPendingSchema,
  createdIdSchema,
  createdOrderSchema,
  noContentSchema,
  systemLogSchema,
  testMailResultSchema,
  adminOrderSchema,
  adminPaymentSchema,
  adminStatSummarySchema,
  adminUserDetailSchema,
  adminUserSchema,
  adminUserTrafficSchema,
  arraySchema,
  couponSchema,
  giftcardSchema,
  knowledgeSchema,
  knowledgeSummarySchema,
  noticeSchema,
  paymentFormSchema,
  planSchema,
  queueStatsSchema,
  queueWorkloadSchema,
  serverGroupSchema,
  serverNodeSchema,
  serverRankSchema,
  serverRouteSchema,
  statSeriesPointSchema,
  stringArraySchema,
  userRankSchema,
} from '../contracts';

/**
 * App-internal shared filter clause (`{key, condition, value}`), persisted in
 * cross-page sessionStorage handoffs and drawer state. It is not a wire
 * shape: each list endpoint translates it into the §7 DSL at this boundary
 * (see `userFilterClauses`).
 */
export interface AdminFilter {
  key: string;
  condition: string;
  value: string | number | null;
}

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
  current?: number;
  pageSize?: number;
}

interface PageResult<T> {
  data: T[];
  total?: number;
}

type QueryRequestConfig = Pick<ApiRequestConfig, 'signal'>;

const BYTES_PER_GIB = 1_073_741_824;

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

/** GET /{secure_path}/plans — dialect v2 bare array (§6.2, W11). Prices ride
 * as cents; `normalizePlan` divides them to yuan for the editor. */
export const fetchPlans = async (client: ApiClient, config?: QueryRequestConfig) =>
  (
    await client.request({
      url: client.resolveAdminPath('/plans'),
      method: 'GET',
      dialect: 'v2',
      responseSchema: arraySchema(planSchema),
      ...config,
    })
  ).map(normalizePlan);

/**
 * POST /{secure_path}/plans (201 `{id}`) / PATCH `plans/{id}` (204) — the
 * dialect-v2 upsert split (§6.2, W11). Prices serialize to cents; on PATCH an
 * empty price is a §4.4 clear (null). `force_update` is an edit-only body flag
 * — the create body denies it (`deny_unknown_fields`; there are no subscribers
 * to force yet).
 */
export const savePlan = (client: ApiClient, { id, force_update, ...data }: AdminPlanSavePayload) =>
  id === undefined
    ? client.request({
        url: client.resolveAdminPath('/plans'),
        method: 'POST',
        dialect: 'v2',
        data: serializePlanBody(data),
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/plans/${id}`),
        method: 'PATCH',
        dialect: 'v2',
        data: {
          ...serializePlanBody(data),
          ...(force_update === undefined ? {} : { force_update }),
        },
        responseSchema: noContentSchema,
      });

function serializePlanBody(
  data: Omit<AdminPlanSavePayload, 'id' | 'force_update'>,
): Record<string, unknown> {
  const next: Record<string, unknown> = {};
  for (const key of PLAN_SAVE_KEYS) {
    if (key === 'id' || key === 'force_update') continue;
    const value = data[key as keyof typeof data];
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

/** PATCH /{secure_path}/plans/{id} `{show|renew}` — the merged legacy toggle (§6.2). */
export const updatePlan = (client: ApiClient, id: number, key: 'show' | 'renew', value: boolean) =>
  client.request({
    url: client.resolveAdminPath(`/plans/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { [key]: value },
    responseSchema: noContentSchema,
  });

export const dropPlan = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/plans/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/plans/sort `{ids}` — `plan_ids` → `ids` (§6.2, §4.1). */
export const sortPlans = (client: ApiClient, ids: number[]) =>
  client.request({
    url: client.resolveAdminPath('/plans/sort'),
    method: 'POST',
    dialect: 'v2',
    data: { ids },
    responseSchema: noContentSchema,
  });

/**
 * §7.1 — the `GET users` filter whitelist (docs/api-dialect.md §7.1: the
 * guarded `user_column` list) plus the §7.2 sort columns (the same list plus
 * the computed `total_used`). Every column here resolves on the backend; a
 * clause on any other field is dropped client-side, matching the legacy
 * `user_column` guard that silently ignored unknown filter keys.
 */
export const USER_FILTER_FIELDS = [
  'id',
  'email',
  'telegram_id',
  'balance',
  'discount',
  'commission_type',
  'commission_rate',
  'commission_balance',
  't',
  'u',
  'd',
  'transfer_enable',
  'device_limit',
  'banned',
  'is_admin',
  'is_staff',
  'last_login_at',
  'uuid',
  'group_id',
  'plan_id',
  'speed_limit',
  'token',
  'expired_at',
  'remarks',
  'invite_user_id',
  'created_at',
  'updated_at',
] as const;
export type UserFilterField = (typeof USER_FILTER_FIELDS)[number];

const USER_FILTER_FIELD_SET = new Set<string>(USER_FILTER_FIELDS);
/** §7.1 boolean-typed columns: their 0/1 select value becomes a JSON boolean. */
const USER_BOOLEAN_FILTER_FIELDS = new Set(['banned', 'is_admin', 'is_staff']);
/** §7.1 timestamp columns: their epoch-second filter value becomes RFC 3339. */
const USER_TIMESTAMP_FILTER_FIELDS = new Set([
  'last_login_at',
  'expired_at',
  'created_at',
  'updated_at',
]);
/**
 * §7.1 text columns whose value is a string for every op. `email`/`remarks`
 * ride `like` in the UI, but the backend coerces `value` to the column type
 * (§7.1), so an `eq`/`neq` clause on any of them must still bind a string
 * rather than a numeric coercion.
 */
const USER_STRING_FILTER_FIELDS = new Set(['uuid', 'token', 'email', 'remarks']);

const userFilterEpochToRfc3339 = (value: unknown) =>
  new Date(1000 * Number(value)).toISOString().replace(/\.\d{3}Z$/, 'Z');

/**
 * Translate the app-internal shared `{key, condition, value}` filter shape into
 * the §7 DSL clause array. Unknown fields drop (the legacy `user_column` guard
 * ignored them); `like` keeps the raw substring; the `'null'` sentinel becomes
 * JSON null; boolean columns coerce 0/1 to booleans and timestamp columns
 * coerce their epoch-second value to an RFC 3339 string (§7.1 value types).
 */
function userFilterClauses(filter?: AdminFilter[]): FilterClause<UserFilterField>[] | undefined {
  if (!filter?.length) return undefined;
  const clauses: FilterClause<UserFilterField>[] = [];
  for (const clause of filter) {
    const field = clause.key;
    if (!USER_FILTER_FIELD_SET.has(field)) continue;
    const op = LEGACY_FILTER_OPS[clause.condition] ?? 'eq';
    const raw = clause.value;
    if (op === 'like') {
      clauses.push({ field: field as UserFilterField, op, value: raw == null ? '' : String(raw) });
      continue;
    }
    if (raw === null || raw === 'null') {
      clauses.push({ field: field as UserFilterField, op, value: null });
      continue;
    }
    if (USER_BOOLEAN_FILTER_FIELDS.has(field)) {
      const value = raw === 1 || raw === '1';
      clauses.push({ field: field as UserFilterField, op, value });
      continue;
    }
    if (USER_TIMESTAMP_FILTER_FIELDS.has(field)) {
      clauses.push({ field: field as UserFilterField, op, value: userFilterEpochToRfc3339(raw) });
      continue;
    }
    if (USER_STRING_FILTER_FIELDS.has(field)) {
      clauses.push({ field: field as UserFilterField, op, value: String(raw) });
      continue;
    }
    clauses.push({
      field: field as UserFilterField,
      op,
      value: typeof raw === 'number' ? raw : Number(raw),
    });
  }
  return clauses.length ? clauses : undefined;
}

/**
 * GET /{secure_path}/users — dialect v2 `{items, total}` page (§6.6, W12): §8
 * pagination, the §7 DSL over the guarded user column whitelist, and §7.2 sort
 * (`sort_by`/`sort_dir`, incl. the computed `total_used`). The app keeps its
 * shared legacy filter representation; the DSL translation happens here.
 */
export const fetchUsers = async (
  client: ApiClient,
  query: AdminPageQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<AdminUserRow>> => {
  const page = await client.request({
    url: client.resolveAdminPath('/users'),
    method: 'GET',
    dialect: 'v2',
    params: adminListQueryParams<UserFilterField>({
      page: query.current,
      per_page: query.pageSize,
      sort_by: query.sort,
      sort_dir: query.sort_type ? (query.sort_type === 'ASC' ? 'asc' : 'desc') : undefined,
      filter: userFilterClauses(query.filter),
    }),
    responseSchema: pageSchema(adminUserSchema),
    ...config,
  });
  return {
    data: page.items.map((user) => normalizeAdminUser(user, { normalizeTotalUsed: true })),
    total: page.total,
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

/** PATCH `users/{id}` fields that clear on JSON `null` (§4.4 double-Option). */
const ADMIN_USER_NULLABLE_FIELDS = [
  'plan_id',
  'device_limit',
  'commission_rate',
  'discount',
  'speed_limit',
  'remarks',
] as const;

/**
 * Build the §6.6 PATCH `users/{id}` body (docs/api-dialect.md §4.4): scaled
 * fields ride as integer bytes/cents, the 0/1 flags cross as JSON booleans,
 * `expired_at` as RFC 3339, and the nullable columns clear on JSON `null`.
 * `id`/`invite_user_email` never enter the body (the id is the path; the
 * inviter is the dedicated `set-inviter` action) and the profile
 * `remind_*` flags are not part of the admin update.
 */
function serializeAdminUserPatch(data: AdminUserUpdateInput): Record<string, unknown> {
  const body: Record<string, unknown> = {};
  const set = (key: string, value: unknown) => {
    if (value !== undefined) body[key] = value;
  };
  set('email', data.email);
  if (data.password !== undefined && data.password !== '') body.password = data.password;
  set('commission_type', data.commission_type === undefined ? undefined : Number(data.commission_type));
  for (const flag of ['banned', 'is_admin', 'is_staff'] as const) {
    if (data[flag] !== undefined) body[flag] = Boolean(data[flag]);
  }
  for (const [field, scale] of Object.entries(ADMIN_USER_SCALED_FIELDS)) {
    const value = data[field as AdminUserScaledField];
    if (value === undefined || value === null || value === '') continue;
    body[field] = decimalToScaledInteger(value, scale);
  }
  for (const field of ADMIN_USER_NULLABLE_FIELDS) {
    const value = data[field];
    if (value === undefined) continue;
    body[field] = value === null || value === '' ? null : value;
  }
  if (data.expired_at !== undefined) {
    body.expired_at =
      data.expired_at === null ? null : userFilterEpochToRfc3339(data.expired_at);
  }
  return body;
}

/** POST /{secure_path}/users/{id}/set-inviter — dialect v2 (§6.6, W12): a
 * present `invite_user_email` resolves the inviter; empty/null clears it. */
export const setUserInviter = (client: ApiClient, id: number, invite_user_email?: string | null) =>
  client.request({
    url: client.resolveAdminPath(`/users/${id}/set-inviter`),
    method: 'POST',
    dialect: 'v2',
    data: { invite_user_email: invite_user_email ?? null },
    responseSchema: noContentSchema,
  });

/**
 * PATCH /{secure_path}/users/{id} — dialect v2 update (§6.6, W12). The legacy
 * `user/update` inviter arm split into the dedicated `set-inviter` action, so
 * an editor save is the field PATCH followed by the inviter action whenever the
 * form carries an `invite_user_email` value.
 */
export const updateUser = async (client: ApiClient, data: AdminUserUpdateInput) => {
  await client.request({
    url: client.resolveAdminPath(`/users/${data.id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: serializeAdminUserPatch(data),
    responseSchema: noContentSchema,
  });
  if (data.invite_user_email !== undefined) {
    await setUserInviter(client, data.id, data.invite_user_email);
  }
};

export const getUserInfoById = async (client: ApiClient, id: number, config?: QueryRequestConfig) =>
  normalizeAdminUserDetail(
    await client.request({
      url: client.resolveAdminPath(`/users/${id}`),
      method: 'GET',
      dialect: 'v2',
      responseSchema: adminUserDetailSchema,
      ...config,
    }),
  );

/**
 * POST /{secure_path}/users — dialect v2 (§6.6, W12): a single create (a real
 * `email_prefix`) returns the bare 201 `{id}`; a bulk run streams the
 * byte-frozen credential CSV attachment. `expired_at` serializes to RFC 3339.
 */
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
    url: client.resolveAdminPath('/users'),
    method: 'POST',
    dialect: 'v2',
    data: {
      ...(data.email_prefix ? { email_prefix: data.email_prefix } : {}),
      email_suffix: data.email_suffix,
      ...(data.password ? { password: data.password } : {}),
      ...(data.plan_id == null ? {} : { plan_id: Number(data.plan_id) }),
      ...(data.expired_at == null || data.expired_at === ''
        ? {}
        : { expired_at: userFilterEpochToRfc3339(data.expired_at) }),
      ...(data.generate_count ? { generate_count: Number(data.generate_count) } : {}),
    },
    jsonResponseSchema: createdIdSchema,
  });

/** POST /{secure_path}/users/mail — dialect v2, 204 (§6.6, W12): the DSL
 * `{subject, content, filter?}` body with the unchanged `Idempotency-Key`
 * replay contract. */
export const sendMailToUsers = (
  client: ApiClient,
  data: { subject: string; content: string; filter?: AdminFilter[] },
) =>
  client.request({
    url: client.resolveAdminPath('/users/mail'),
    method: 'POST',
    dialect: 'v2',
    data: {
      subject: data.subject,
      content: data.content,
      ...(userFilterClauses(data.filter) ? { filter: userFilterClauses(data.filter) } : {}),
    },
    responseSchema: noContentSchema,
    // TanStack invokes a mutation retry with the same variables object. Keeping
    // the key by object identity makes that retry replay the durable batch.
    headers: { 'Idempotency-Key': mutationIdempotencyKey(data) },
  });

/** POST /{secure_path}/users/export — dialect v2 (§6.6, W12): CSV over the DSL
 * `{filter?}` body. */
export const dumpUsersCsv = (client: ApiClient, filter?: AdminFilter[]) =>
  client.requestBinary({
    url: client.resolveAdminPath('/users/export'),
    method: 'POST',
    dialect: 'v2',
    data: { ...(userFilterClauses(filter) ? { filter: userFilterClauses(filter) } : {}) },
    jsonResponseSchema: createdIdSchema,
  });

/** POST /{secure_path}/users/ban — dialect v2, 204 (§6.6, W12): DSL `{filter?}`. */
export const banUsers = (client: ApiClient, filter?: AdminFilter[]) =>
  client.request({
    url: client.resolveAdminPath('/users/ban'),
    method: 'POST',
    dialect: 'v2',
    data: { ...(userFilterClauses(filter) ? { filter: userFilterClauses(filter) } : {}) },
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/users/{id}/reset-secret — dialect v2, 204 (§6.6, W12). */
export const resetUserSecret = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/users/${id}/reset-secret`),
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** DELETE /{secure_path}/users/{id} — dialect v2, 204 (§6.6, W12). */
export const deleteUser = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/users/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/users/bulk-delete — dialect v2, 204 (§6.6, W12): DSL
 * `{filter?}` (kept a POST action; DELETE-with-body is hostile to proxies). */
export const deleteAllUsers = (client: ApiClient, filter?: AdminFilter[]) =>
  client.request({
    url: client.resolveAdminPath('/users/bulk-delete'),
    method: 'POST',
    dialect: 'v2',
    data: { ...(userFilterClauses(filter) ? { filter: userFilterClauses(filter) } : {}) },
    responseSchema: noContentSchema,
  });

/** §7.1 order filter column kinds. Integer columns take JSON numbers, text/
 * timestamp columns strings; `like` values ride raw. */
const ORDER_FILTER_STRING_FIELDS = new Set([
  'period',
  'trade_no',
  'callback_no',
  'paid_at',
  'created_at',
  'updated_at',
]);

/** §7.1 — legacy `{key, condition, value}` conditions folded onto the op set. */
const LEGACY_FILTER_OPS: Record<string, FilterOp> = {
  '=': 'eq',
  is: 'eq',
  '!=': 'neq',
  '<>': 'neq',
  not: 'neq',
  like: 'like',
  模糊: 'like',
  '>': 'gt',
  '>=': 'gte',
  '<': 'lt',
  '<=': 'lte',
};

/**
 * Translate the app-internal legacy filter representation (still the shared
 * cross-page `{key, condition, value}` shape written by the dashboard and user
 * pages) into the §7 DSL clause array on the way to the wire. `like` keeps the
 * raw substring; integer columns coerce to JSON numbers (the DSL rejects a
 * string on a numeric column); the `'null'` sentinel becomes JSON null.
 */
function orderFilterClauses(filter?: AdminFilter[]): FilterClause[] | undefined {
  if (!filter?.length) return undefined;
  return filter.map((clause) => {
    const field = clause.key;
    const op = LEGACY_FILTER_OPS[clause.condition] ?? 'eq';
    const raw = clause.value;
    if (op === 'like') return { field, op, value: raw == null ? '' : String(raw) };
    if (raw === null || raw === 'null') return { field, op, value: null };
    if (ORDER_FILTER_STRING_FIELDS.has(field)) return { field, op, value: String(raw) };
    return { field, op, value: typeof raw === 'number' ? raw : Number(raw) };
  });
}

/**
 * GET /{secure_path}/orders — dialect v2 `{items, total}` page (§6.4, W11):
 * §8 pagination, the §7 DSL over the guarded order column list, and
 * `?commission_only=` (the legacy `?is_commission=`). The app keeps its shared
 * legacy filter representation; the DSL translation happens here.
 */
export const fetchOrders = async (
  client: ApiClient,
  query: AdminPageQuery & { commission_only?: boolean } = {},
  config?: QueryRequestConfig,
): Promise<PageResult<AdminOrderRow>> => {
  const params: Record<string, unknown> = adminListQueryParams({
    page: query.current,
    per_page: query.pageSize,
    sort_by: query.sort,
    sort_dir: query.sort_type ? (query.sort_type === 'ASC' ? 'asc' : 'desc') : undefined,
    filter: orderFilterClauses(query.filter),
  });
  if (query.commission_only !== undefined) params.commission_only = query.commission_only;
  const page = await client.request({
    url: client.resolveAdminPath('/orders'),
    method: 'GET',
    dialect: 'v2',
    params,
    responseSchema: pageSchema(adminOrderSchema),
    ...config,
  });
  return { data: page.items, total: page.total };
};

/** GET /{secure_path}/orders/{trade_no} — dialect v2 bare detail (§6.4, W11);
 * the read moved off POST and the identifier from numeric id to trade_no. */
export const orderDetail = (client: ApiClient, trade_no: string, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath(`/orders/${encodeURIComponent(trade_no)}`),
    method: 'GET',
    dialect: 'v2',
    responseSchema: adminOrderSchema,
    ...config,
  });

/** POST /{secure_path}/orders/{trade_no}/mark-paid — 204 (§6.4). */
export const paidOrder = (client: ApiClient, trade_no: string) =>
  client.request({
    url: client.resolveAdminPath(`/orders/${encodeURIComponent(trade_no)}/mark-paid`),
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/orders/{trade_no}/cancel — 204 (§6.4). */
export const cancelOrder = (client: ApiClient, trade_no: string) =>
  client.request({
    url: client.resolveAdminPath(`/orders/${encodeURIComponent(trade_no)}/cancel`),
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/**
 * PATCH /{secure_path}/orders/{trade_no} — dialect v2 (§6.4): exactly one of
 * `{status, commission_status}` per call (the backend 422s on both/neither).
 * The value is a numeric enum.
 */
export const updateOrder = (
  client: ApiClient,
  trade_no: string,
  key: 'commission_status' | 'status',
  value: string | number,
) =>
  client.request({
    url: client.resolveAdminPath(`/orders/${encodeURIComponent(trade_no)}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { [key]: Number(value) },
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/orders — dialect v2 create (§6.4, legacy `order/assign`):
 * 201 `{trade_no}`; `total_amount` serializes to cents. */
export const assignOrder = (
  client: ApiClient,
  data: {
    email: string;
    plan_id: number;
    period: PlanPeriod;
    total_amount: number | string;
  },
) =>
  client.request({
    url: client.resolveAdminPath('/orders'),
    method: 'POST',
    dialect: 'v2',
    data: {
      ...data,
      // Legacy '' meant "no charge"; omit so Rust's Option default (0) applies.
      total_amount:
        data.total_amount === '' ? undefined : decimalToCents(data.total_amount),
    },
    responseSchema: createdOrderSchema,
  });

/** GET /{secure_path}/payments — dialect v2 bare array (§6.2, W11). */
export const fetchPayments = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/payments'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(adminPaymentSchema),
    ...config,
  });

/** GET /{secure_path}/payment-providers — dialect v2 provider-code array (§6.2). */
export const paymentMethods = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/payment-providers'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: stringArraySchema,
    ...config,
  });

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

/**
 * GET /{secure_path}/payment-providers/{code}/form `?payment_id=` — dialect v2
 * bare provider form (§6.2, W11): the legacy POST `getPaymentForm` read moved
 * to GET, provider code in the path and the optional row id in the query.
 */
export const paymentForm = (
  client: ApiClient,
  payment?: string,
  id?: number,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath(`/payment-providers/${encodeURIComponent(payment ?? '')}/form`),
    method: 'GET',
    dialect: 'v2',
    params: id === undefined ? undefined : { payment_id: id },
    responseSchema: paymentFormSchema,
    ...config,
  });

const optionalPaymentFields = [
  'icon',
  'notify_domain',
  'handling_fee_fixed',
  'handling_fee_percent',
] as const;

/**
 * The dialect-v2 payment body (§6.2, §4.4, W11): whitelist the columns the
 * upsert consumes (never echo fetched uuid/enable/sort/timestamps/notify_url).
 * On create an empty optional is absent (the documented default); on PATCH an
 * empty optional is an explicit `null` clear. `handling_fee_fixed` serializes
 * to cents and `handling_fee_percent` to a JSON number (§4.1).
 */
function serializePaymentBody(data: SavePaymentPayload): Record<string, unknown> {
  const isCreate = data.id === undefined;
  const payload: Record<string, unknown> = {
    name: data.name,
    payment: data.payment,
    config: data.config,
  };
  for (const key of optionalPaymentFields) {
    const value = data[key];
    if (value === undefined) continue;
    if (value === '') {
      if (isCreate) continue;
      payload[key] = null;
      continue;
    }
    payload[key] = value;
  }
  if (payload.handling_fee_fixed != null) {
    payload.handling_fee_fixed = decimalToCents(payload.handling_fee_fixed as string | number);
  }
  if (payload.handling_fee_percent != null) {
    payload.handling_fee_percent = Number(payload.handling_fee_percent);
  }
  return payload;
}

/**
 * POST /{secure_path}/payments (201 `{id}`) / PATCH `payments/{id}` (204) — the
 * dialect-v2 upsert split (§6.2, W11); §4.4 replaces the legacy
 * present-but-empty=clear convention.
 */
export const savePayment = (client: ApiClient, data: SavePaymentPayload) =>
  data.id === undefined
    ? client.request({
        url: client.resolveAdminPath('/payments'),
        method: 'POST',
        dialect: 'v2',
        data: serializePaymentBody(data),
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/payments/${data.id}`),
        method: 'PATCH',
        dialect: 'v2',
        data: serializePaymentBody(data),
        responseSchema: noContentSchema,
      });

/** PATCH /{secure_path}/payments/{id} `{enable}` — the merged legacy toggle (§6.2). */
export const showPayment = (client: ApiClient, id: number, enable: boolean) =>
  client.request({
    url: client.resolveAdminPath(`/payments/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { enable },
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/payments/sort `{ids}` — 204 (§6.2). */
export const sortPayments = (client: ApiClient, ids: number[]) =>
  client.request({
    url: client.resolveAdminPath('/payments/sort'),
    method: 'POST',
    dialect: 'v2',
    data: { ids },
    responseSchema: noContentSchema,
  });

export const dropPayment = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/payments/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/**
 * GET /{secure_path}/notices — dialect v2 bare array (§6.3, W10). The list
 * stays deliberately unpaginated: the legacy route returned every row and no
 * pagination was invented.
 */
export const fetchNotices = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/notices'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(noticeSchema),
    ...config,
  });

export type SaveNoticePayload = Pick<Notice, 'content' | 'img_url' | 'tags' | 'title'> & {
  id?: number;
};

/**
 * POST /{secure_path}/notices (201 `{id}`) / PATCH `notices/{id}` (204) —
 * the dialect-v2 upsert split (§6.3, W10). The full-form editor sends every
 * field; explicit `img_url`/`tags` nulls are §4.4 clears.
 */
export const saveNotice = (client: ApiClient, { id, ...data }: SaveNoticePayload) =>
  id === undefined
    ? client.request({
        url: client.resolveAdminPath('/notices'),
        method: 'POST',
        dialect: 'v2',
        data,
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/notices/${id}`),
        method: 'PATCH',
        dialect: 'v2',
        data,
        responseSchema: noContentSchema,
      });

export const dropNotice = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/notices/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** PATCH /{secure_path}/notices/{id} `{show}` — the merged legacy toggle (§6.3). */
export const showNotice = (client: ApiClient, id: number, show: boolean) =>
  client.request({
    url: client.resolveAdminPath(`/notices/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { show },
    responseSchema: noContentSchema,
  });

/**
 * §6.5 (W14) list query: pages keep their local `{current, pageSize}` state;
 * the §8 `page`/`per_page` wire query is minted here. `status`, `email`, and
 * the repeatable `reply_status` keys are the admin ticket list's only
 * filters (no §7 DSL — the spec invents none for this family).
 */
export interface AdminTicketListQuery {
  current?: number;
  pageSize?: number;
  status?: number;
  email?: string;
  reply_status?: number[] | null;
}

/**
 * GET /{secure_path}/tickets — dialect v2 `{items, total}` page (§6.5, W14).
 * `reply_status` rides as a repeated real-array query key (the legacy
 * JSON-stringified array param died); an empty `email` means "no filter",
 * matching the legacy falsy guard, so it is omitted from the wire.
 */
export const fetchTickets = async (
  client: ApiClient,
  query: AdminTicketListQuery = {},
  config?: QueryRequestConfig,
): Promise<PageResult<Ticket>> => {
  const page = await client.request({
    url: client.resolveAdminPath('/tickets'),
    method: 'GET',
    dialect: 'v2',
    params: {
      page: query.current,
      per_page: query.pageSize,
      status: query.status,
      email: query.email || undefined,
      reply_status: query.reply_status?.length ? query.reply_status : undefined,
    },
    responseSchema: pageSchema(adminTicketSchema),
    ...config,
  });
  return { data: page.items, total: page.total };
};

/** GET /{secure_path}/tickets/{id} — bare detail with the `message[]` thread (§6.5, W14). */
export const ticketDetail = (client: ApiClient, id: number | string, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath(`/tickets/${encodeURIComponent(id)}`),
    method: 'GET',
    dialect: 'v2',
    responseSchema: adminTicketDetailSchema,
    ...config,
  });

/** POST /{secure_path}/tickets/{id}/replies `{message}` — 204; the `id` moves to the path (§6.5). */
export const replyTicket = (client: ApiClient, payload: TicketReplyPayload) => {
  const { id, ...data } = payload;
  return client.request({
    url: client.resolveAdminPath(`/tickets/${encodeURIComponent(id)}/replies`),
    method: 'POST',
    dialect: 'v2',
    data,
    responseSchema: noContentSchema,
  });
};

/** POST /{secure_path}/tickets/{id}/close — 204, no body (§6.5, W14). */
export const closeTicket = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/tickets/${encodeURIComponent(id)}/close`),
    method: 'POST',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/**
 * §6.3 (W10): the coupon/gift-card lists take §8 pagination plus the §7.2
 * `created_at` sort only — the legacy lists had no filter support and the
 * spec invents none.
 */
export type ContentListQuery = Pick<AdminListQuery, 'page' | 'per_page' | 'sort_by' | 'sort_dir'>;

/** GET /{secure_path}/coupons — dialect v2 `{items, total}` page (§6.3, W10). */
export const fetchCoupons = async (
  client: ApiClient,
  query: ContentListQuery = {},
  config?: QueryRequestConfig,
) => {
  const page = await client.request({
    url: client.resolveAdminPath('/coupons'),
    method: 'GET',
    dialect: 'v2',
    params: adminListQueryParams(query),
    responseSchema: pageSchema(couponSchema),
    ...config,
  });
  // Client-side money rule (§6.3): amount coupons stay integer cents on the
  // wire; the admin table displays decimal yuan.
  page.items.forEach((coupon) => {
    if (coupon.type === 1) coupon.value = coupon.value / 100;
  });
  return page;
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

export type GenerateCsvResponse = BinaryApiResponse<typeof createdIdSchema>;

// §4.5: the editor form keeps its epoch-second window state; the wire takes
// RFC 3339 UTC. Conversion lives here at the API boundary, like the money
// rules below. Empty inputs stay absent (the backend rejects a missing
// required window as a 422 problem).
const epochInputToRfc3339 = (value: number | string | null | undefined) =>
  value == null || value === ''
    ? undefined
    : new Date(1000 * Number(value)).toISOString().replace(/\.\d{3}Z$/, 'Z');

const optionalCount = (value: number | string | null | undefined) =>
  value == null || value === '' ? null : Number(value);

/**
 * The shared §6.3 coupon body: JSON with real arrays (`limit_plan_ids` drops
 * the form-encoded brackets), RFC 3339 windows, and the unchanged client-side
 * money rule (`type === 1 → value*100`). Explicit nulls on the limit fields
 * are §4.4 clears — matching the legacy full-form editor semantics.
 */
function serializeCouponBody(data: GenerateCouponPayload) {
  return {
    name: data.name,
    type: data.type,
    value:
      data.value == null || data.value === ''
        ? null
        : data.type === 1
          ? decimalToCents(data.value)
          : Number(data.value),
    started_at: epochInputToRfc3339(data.started_at),
    ended_at: epochInputToRfc3339(data.ended_at),
    limit_use: optionalCount(data.limit_use),
    limit_use_with_user: optionalCount(data.limit_use_with_user),
    limit_plan_ids: data.limit_plan_ids?.length ? data.limit_plan_ids.map(Number) : null,
    limit_period: data.limit_period?.length ? data.limit_period : null,
    ...(data.code ? { code: data.code } : {}),
  };
}

/**
 * POST /{secure_path}/coupons — dialect v2 create (§6.3, W10): a single
 * create is the bare 201 `{id}`; `generate_count` streams the byte-frozen
 * CSV bulk attachment.
 */
export const generateCoupon = (client: ApiClient, data: GenerateCouponPayload) =>
  client.requestBinary({
    url: client.resolveAdminPath('/coupons'),
    method: 'POST',
    dialect: 'v2',
    data: {
      ...serializeCouponBody(data),
      ...(data.generate_count ? { generate_count: Number(data.generate_count) } : {}),
    },
    jsonResponseSchema: createdIdSchema,
  });

/** PATCH /{secure_path}/coupons/{id} — dialect v2 update (§6.3, W10). */
export const updateCoupon = (client: ApiClient, id: number, data: GenerateCouponPayload) =>
  client.request({
    url: client.resolveAdminPath(`/coupons/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: serializeCouponBody(data),
    responseSchema: noContentSchema,
  });

export const dropCoupon = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/coupons/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** PATCH /{secure_path}/coupons/{id} `{show}` — the merged legacy toggle (§6.3). */
export const showCoupon = (client: ApiClient, id: number, show: boolean) =>
  client.request({
    url: client.resolveAdminPath(`/coupons/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { show },
    responseSchema: noContentSchema,
  });

/** GET /{secure_path}/gift-cards — dialect v2 `{items, total}` page (§6.3, W10). */
export const fetchGiftcards = async (
  client: ApiClient,
  query: ContentListQuery = {},
  config?: QueryRequestConfig,
) => {
  const page = await client.request({
    url: client.resolveAdminPath('/gift-cards'),
    method: 'GET',
    dialect: 'v2',
    params: adminListQueryParams(query),
    responseSchema: pageSchema(giftcardSchema),
    ...config,
  });
  // Client-side money rule (§6.3): amount cards stay integer cents on the wire.
  page.items.forEach((giftcard) => {
    if (giftcard.type === 1 && giftcard.value !== null) giftcard.value /= 100;
  });
  return page;
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

/**
 * The shared §6.3 gift-card body: same conventions as coupons (JSON, RFC 3339
 * windows, gift-card cents for `type` 1). Type 4 (reset) carries no value.
 */
function serializeGiftcardBody(data: GenerateGiftcardPayload) {
  return {
    name: data.name,
    type: data.type,
    value:
      data.type === 4 || data.value == null || data.value === ''
        ? null
        : data.type === 1
          ? decimalToCents(data.value)
          : Number(data.value),
    plan_id: data.plan_id == null || data.plan_id === '' ? null : Number(data.plan_id),
    started_at: epochInputToRfc3339(data.started_at),
    ended_at: epochInputToRfc3339(data.ended_at),
    limit_use: optionalCount(data.limit_use),
    ...(data.code ? { code: data.code } : {}),
  };
}

/**
 * POST /{secure_path}/gift-cards — dialect v2 create (§6.3, W10): bare 201
 * `{id}` single create or the byte-frozen CSV bulk attachment.
 */
export const generateGiftcard = (client: ApiClient, data: GenerateGiftcardPayload) =>
  client.requestBinary({
    url: client.resolveAdminPath('/gift-cards'),
    method: 'POST',
    dialect: 'v2',
    data: {
      ...serializeGiftcardBody(data),
      ...(data.generate_count ? { generate_count: Number(data.generate_count) } : {}),
    },
    jsonResponseSchema: createdIdSchema,
  });

/** PATCH /{secure_path}/gift-cards/{id} — dialect v2 update (§6.3, W10). */
export const updateGiftcard = (client: ApiClient, id: number, data: GenerateGiftcardPayload) =>
  client.request({
    url: client.resolveAdminPath(`/gift-cards/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: serializeGiftcardBody(data),
    responseSchema: noContentSchema,
  });

export const dropGiftcard = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/gift-cards/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** GET /{secure_path}/knowledge — dialect v2 bare summary array (§6.3, W10). */
export const fetchKnowledge = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/knowledge'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(knowledgeSummarySchema),
    ...config,
  });

/** GET /{secure_path}/knowledge/{id} — dialect v2 bare detail, raw stored body (§6.3). */
export const knowledgeDetail = (client: ApiClient, id: number, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath(`/knowledge/${id}`),
    method: 'GET',
    dialect: 'v2',
    responseSchema: knowledgeSchema,
    ...config,
  });

/** GET /{secure_path}/knowledge-categories — dialect v2 bare name array (§6.3). */
export const knowledgeCategories = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/knowledge-categories'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: stringArraySchema,
    ...config,
  });

export type SaveKnowledgePayload = Pick<Knowledge, 'body' | 'category' | 'language' | 'title'> & {
  id?: number;
};

/**
 * POST /{secure_path}/knowledge (201 `{id}`) / PATCH `knowledge/{id}` (204)
 * — the dialect-v2 upsert split (§6.3, W10).
 */
export const saveKnowledge = (client: ApiClient, { id, ...data }: SaveKnowledgePayload) =>
  id === undefined
    ? client.request({
        url: client.resolveAdminPath('/knowledge'),
        method: 'POST',
        dialect: 'v2',
        data,
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/knowledge/${id}`),
        method: 'PATCH',
        dialect: 'v2',
        data,
        responseSchema: noContentSchema,
      });

/** PATCH /{secure_path}/knowledge/{id} `{show}` — the merged legacy toggle (§6.3). */
export const showKnowledge = (client: ApiClient, id: number, show: boolean) =>
  client.request({
    url: client.resolveAdminPath(`/knowledge/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { show },
    responseSchema: noContentSchema,
  });

export const dropKnowledge = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/knowledge/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/knowledge/sort `{ids}` — dialect v2 full resequencing (§6.3). */
export const sortKnowledge = (client: ApiClient, ids: number[]) =>
  client.request({
    url: client.resolveAdminPath('/knowledge/sort'),
    method: 'POST',
    dialect: 'v2',
    data: { ids },
    responseSchema: noContentSchema,
  });

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

/** §6.8 (W14): the `stats/server-rank` + `stats/user-rank` window selector. */
export type StatsRankWindow = 'today' | 'previous';

/** GET /{secure_path}/stats/summary — dialect v2 bare object (§6.8, W14):
 * the three legacy aliases collapsed into one route; money in integer cents. */
export const statSummary = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/stats/summary'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: adminStatSummarySchema,
    ...config,
  });

/** GET /{secure_path}/stats/server-rank `?window=today|previous` — bare array (§6.8, W14). */
export const statServerRank = (
  client: ApiClient,
  window: StatsRankWindow,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath('/stats/server-rank'),
    method: 'GET',
    dialect: 'v2',
    params: { window },
    responseSchema: arraySchema(serverRankSchema),
    ...config,
  });

/** GET /{secure_path}/stats/user-rank `?window=today|previous` — bare array (§6.8, W14). */
export const statUserRank = (
  client: ApiClient,
  window: StatsRankWindow,
  config?: QueryRequestConfig,
) =>
  client.request({
    url: client.resolveAdminPath('/stats/user-rank'),
    method: 'GET',
    dialect: 'v2',
    params: { window },
    responseSchema: arraySchema(userRankSchema),
    ...config,
  });

/** GET /{secure_path}/stats/orders — bare `{series, date, value}` array (§6.8, W14):
 * snake_case series slugs, integer-cent money. */
export const statOrder = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/stats/orders'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(statSeriesPointSchema),
    ...config,
  });

/**
 * GET /{secure_path}/stats/user-traffic `?user_id=&page=&per_page=` — dialect
 * v2 `{items, total}` page (§6.8, W14): RFC 3339 `record_at`, numeric
 * `server_rate`. The modal keeps its local `{current, pageSize}` state; the
 * §8 wire query is minted here.
 */
export const statUser = async (
  client: ApiClient,
  query: AdminUserTrafficQuery,
  config?: QueryRequestConfig,
): Promise<PageResult<AdminUserTrafficRecord>> => {
  const page = await client.request({
    url: client.resolveAdminPath('/stats/user-traffic'),
    method: 'GET',
    dialect: 'v2',
    params: { user_id: query.user_id, page: query.current, per_page: query.pageSize },
    responseSchema: pageSchema(adminUserTrafficSchema),
    ...config,
  });
  return { data: page.items, total: page.total };
};

export type ServerNode = output<typeof serverNodeSchema>;

/** GET /{secure_path}/nodes — dialect v2 bare array (§6.7, W13). The rows
 * carry live node credentials, so the read is step-up gated in the backend
 * (the client attaches `x-v2board-step-up` globally when a grant is held). */
export const fetchServerNodes = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/nodes'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(serverNodeSchema),
    ...config,
  });

/** POST /{secure_path}/nodes/sort `{<type>: {<id>: sort}}` — 204 (§6.7); the
 * legacy JSON body shape is kept as-is. */
export const sortServerNodes = (
  client: ApiClient,
  payload: Record<string, Record<string | number, number>>,
) =>
  client.request({
    url: client.resolveAdminPath('/nodes/sort'),
    method: 'POST',
    dialect: 'v2',
    data: payload,
    responseSchema: noContentSchema,
  });

export type ServerGroup = output<typeof serverGroupSchema>;

export interface SaveServerGroupPayload {
  id?: number;
  name: string;
}

/** GET /{secure_path}/server-groups — dialect v2 bare array (§6.7, W13). */
export const fetchServerGroups = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/server-groups'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(serverGroupSchema),
    ...config,
  });

/**
 * POST /{secure_path}/server-groups (201 `{id}`) / PATCH `server-groups/{id}`
 * (204) — the dialect-v2 upsert split (§6.7, W13); the one-field `{name}` body
 * is required in both verbs.
 */
export const saveServerGroup = (client: ApiClient, { id, name }: SaveServerGroupPayload) =>
  id === undefined
    ? client.request({
        url: client.resolveAdminPath('/server-groups'),
        method: 'POST',
        dialect: 'v2',
        data: { name },
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/server-groups/${id}`),
        method: 'PATCH',
        dialect: 'v2',
        data: { name },
        responseSchema: noContentSchema,
      });

/** DELETE /{secure_path}/server-groups/{id} — 204; a still-referenced group is
 * the 400 `server_group_in_use` problem (§6.7). */
export const dropServerGroup = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/server-groups/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

export type ServerRouteAction = output<typeof serverRouteActionSchema>;
export type ServerRoute = output<typeof serverRouteSchema>;

export interface SaveServerRoutePayload {
  id?: number;
  remarks: string;
  match: string[];
  action: ServerRouteAction;
  action_value: string | null;
}

/** GET /{secure_path}/server-routes — dialect v2 bare array; `match` is
 * always an array (§6.7, W13). */
export const fetchServerRoutes = (client: ApiClient, config?: QueryRequestConfig) =>
  client.request({
    url: client.resolveAdminPath('/server-routes'),
    method: 'GET',
    dialect: 'v2',
    responseSchema: arraySchema(serverRouteSchema),
    ...config,
  });

/**
 * POST /{secure_path}/server-routes (201 `{id}`) / PATCH `server-routes/{id}`
 * (204) — the dialect-v2 upsert split (§6.7, W13). The `ROUTE_ACTIONS`
 * vocabulary is unchanged; `action_value` is the one §4.4 nullable field.
 */
export const saveServerRoute = (client: ApiClient, { id, ...data }: SaveServerRoutePayload) =>
  id === undefined
    ? client.request({
        url: client.resolveAdminPath('/server-routes'),
        method: 'POST',
        dialect: 'v2',
        data,
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/server-routes/${id}`),
        method: 'PATCH',
        dialect: 'v2',
        data,
        responseSchema: noContentSchema,
      });

/** DELETE /{secure_path}/server-routes/{id} — 204 (§6.7). */
export const dropServerRoute = (client: ApiClient, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/server-routes/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

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

/** §6.7 plain integer/number wire fields (JSON numbers on the modern wire). */
const SERVER_NUMBER_FIELDS = new Set(['port', 'server_port', 'tls', 'version', 'rate']);
/** §6.7 nullable integers under §4.4 (empty input is an explicit clear). */
const SERVER_NULLABLE_NUMBER_FIELDS = new Set(['parent_id', 'sort', 'up_mbps', 'down_mbps']);
/** §4.1 boolean flags the legacy form spelled 0/1. */
const SERVER_FLAG_FIELDS = new Set([
  'show',
  'allow_insecure',
  'insecure',
  'disable_sni',
  'zero_rtt_handshake',
]);

/**
 * Serialize a tolerant form payload to the §6.7 dialect-v2 protocol body:
 * ports/rate/tls/version as JSON numbers, 0/1 flags as booleans, id arrays as
 * integer arrays, `padding_scheme` as its decoded JSON container, and the R22
 * camelCase vmess settings keys passed through exactly as spelled. Field
 * presence is preserved so the legacy `param_present` gates map 1:1 onto the
 * §4.4 tri-state (absent retains, null clears, value sets).
 */
function serializeServerBody(data: SaveServerPayload): Record<string, unknown> {
  const body: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(data)) {
    if (key === 'id' || value === undefined) continue;
    if (key === 'group_id') {
      body[key] = (value as ServerPayloadScalar[]).map(Number);
    } else if (key === 'route_id') {
      body[key] = value === null ? null : (value as ServerPayloadScalar[]).map(Number);
    } else if (SERVER_NULLABLE_NUMBER_FIELDS.has(key)) {
      body[key] = value === null || value === '' ? null : Number(value);
    } else if (SERVER_NUMBER_FIELDS.has(key)) {
      body[key] = Number(value);
    } else if (SERVER_FLAG_FIELDS.has(key)) {
      body[key] = value === 1 || value === '1';
    } else if (key === 'padding_scheme') {
      body[key] = parsePaddingScheme(value as string | null);
    } else {
      body[key] = value;
    }
  }
  return body;
}

function parsePaddingScheme(value: string | null): unknown {
  if (value === null || value.trim() === '') return null;
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

/**
 * POST /{secure_path}/servers/{type} (201 `{id}`) / PATCH `servers/{type}/{id}`
 * (204) — the dialect-v2 upsert split for the eight protocol matrices
 * (§6.7, W13).
 */
export const saveServer = (client: ApiClient, type: ServerTypeName, data: SaveServerPayload) =>
  data.id === undefined
    ? client.request({
        url: client.resolveAdminPath(`/servers/${type}`),
        method: 'POST',
        dialect: 'v2',
        data: serializeServerBody(data),
        responseSchema: createdIdSchema,
      })
    : client.request({
        url: client.resolveAdminPath(`/servers/${type}/${data.id}`),
        method: 'PATCH',
        dialect: 'v2',
        data: serializeServerBody(data),
        responseSchema: noContentSchema,
      });

/** DELETE /{secure_path}/servers/{type}/{id} — 204 (§6.7). */
export const dropServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/servers/${type}/${id}`),
    method: 'DELETE',
    dialect: 'v2',
    responseSchema: noContentSchema,
  });

/** PATCH /{secure_path}/servers/{type}/{id} `{show}` — the merged legacy
 * show toggle (§6.7). */
export const showServer = (client: ApiClient, type: ServerTypeName, id: number, show: boolean) =>
  client.request({
    url: client.resolveAdminPath(`/servers/${type}/${id}`),
    method: 'PATCH',
    dialect: 'v2',
    data: { show },
    responseSchema: noContentSchema,
  });

/** POST /{secure_path}/servers/{type}/{id}/copy — 201 bare `{id}` of the new
 * copy (§6.7). */
export const copyServer = (client: ApiClient, type: ServerTypeName, id: number) =>
  client.request({
    url: client.resolveAdminPath(`/servers/${type}/${id}/copy`),
    method: 'POST',
    dialect: 'v2',
    responseSchema: createdIdSchema,
  });
