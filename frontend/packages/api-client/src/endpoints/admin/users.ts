import type { AdminUserRow, AdminUserUpdatePayload } from '@v2board/types';
import type { output } from 'zod';
import type { ApiClient } from '../../client';
import { adminListQueryParams, pageSchema, type FilterClause } from '../../dialect';
import { decimalToScaledInteger } from '../../money';
import {
  adminUserDetailSchema,
  adminUserSchema,
  createdIdSchema,
  noContentSchema,
} from '../../contracts';
import {
  LEGACY_FILTER_OPS,
  type AdminFilter,
  type AdminPageQuery,
  type PageResult,
  type QueryRequestConfig,
} from './shared';

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
  set(
    'commission_type',
    data.commission_type === undefined ? undefined : Number(data.commission_type),
  );
  for (const flag of ['banned', 'is_admin', 'is_staff'] as const) {
    if (data[flag] !== undefined) body[flag] = Boolean(data[flag]);
  }
  // §6.12: the staff grant array crosses verbatim (full replacement).
  set('admin_permissions', data.admin_permissions);
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
    body.expired_at = data.expired_at === null ? null : userFilterEpochToRfc3339(data.expired_at);
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
