import type {
  AdminOrderRow,
  AdminPlanDto,
  AdminPlanModel,
  InternalApiAdminPlanItem,
  InternalApiOperationMap,
  MoneyMajor,
  MoneyMinor,
  PlanPeriod,
} from '@v2board/types';
import type { ApiClient } from '../../client';
import { adminListQueryParams, pageSchema, type FilterClause } from '../../dialect';
import { decimalToCents, moneyMinorToMajor } from '../../money';
import { internalApiOperations, internalApiPath } from '../../generated/internal-api';
import {
  adminOrderSchema,
  adminPaymentSchema,
  arraySchema,
  createdIdSchema,
  createdOrderSchema,
  noContentSchema,
  paymentFormSchema,
  stringArraySchema,
} from '../../contracts';
import {
  LEGACY_FILTER_OPS,
  type AdminFilter,
  type AdminPageQuery,
  type PageResult,
  type QueryRequestConfig,
} from './shared';

const PLAN_PRICE_KEYS = [
  'month_price',
  'quarter_price',
  'half_year_price',
  'year_price',
  'two_year_price',
  'three_year_price',
  'onetime_price',
  'reset_price',
] as const satisfies readonly PlanPeriod[];

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

type GeneratedPlanCreate = InternalApiOperationMap['adminPlanCreate']['request'];
type GeneratedPlanPatch = InternalApiOperationMap['adminPlanPatch']['request'];
type BrandedPlanPrices<T> = Omit<T, PlanPeriod> & Partial<Record<PlanPeriod, MoneyMinor | null>>;
type AdminPlanCreateRequest = BrandedPlanPrices<GeneratedPlanCreate>;
type AdminPlanPatchRequest = BrandedPlanPrices<Omit<GeneratedPlanPatch, 'show' | 'renew'>>;

/**
 * Admin plan write DTO. It is intentionally a wire type: prices are branded
 * minor units and numeric fields are JSON numbers. Form strings and the
 * major-unit {@link AdminPlanModel} must be mapped before reaching this API.
 */
export type AdminPlanSaveRequest =
  | (AdminPlanCreateRequest & { id?: undefined; force_update?: never })
  | (AdminPlanPatchRequest & { id: number });

function toAdminPlanDto(plan: InternalApiAdminPlanItem): AdminPlanDto {
  const resetTrafficMethod = plan.reset_traffic_method;
  if (resetTrafficMethod !== null && ![0, 1, 2, 3, 4].includes(resetTrafficMethod)) {
    throw new TypeError(`Unsupported reset traffic method: ${resetTrafficMethod}`);
  }
  const toMinor = (value: number | null): MoneyMinor | null => value as MoneyMinor | null;
  return {
    ...plan,
    reset_traffic_method: resetTrafficMethod as AdminPlanDto['reset_traffic_method'],
    month_price: toMinor(plan.month_price),
    quarter_price: toMinor(plan.quarter_price),
    half_year_price: toMinor(plan.half_year_price),
    year_price: toMinor(plan.year_price),
    two_year_price: toMinor(plan.two_year_price),
    three_year_price: toMinor(plan.three_year_price),
    onetime_price: toMinor(plan.onetime_price),
    reset_price: toMinor(plan.reset_price),
  };
}

function toAdminPlanModel(plan: AdminPlanDto): AdminPlanModel {
  const toMajor = (value: MoneyMinor | null): MoneyMajor | null =>
    value === null ? null : moneyMinorToMajor(value);
  return {
    ...plan,
    month_price: toMajor(plan.month_price),
    quarter_price: toMajor(plan.quarter_price),
    half_year_price: toMajor(plan.half_year_price),
    year_price: toMajor(plan.year_price),
    two_year_price: toMajor(plan.two_year_price),
    three_year_price: toMajor(plan.three_year_price),
    onetime_price: toMajor(plan.onetime_price),
    reset_price: toMajor(plan.reset_price),
  };
}

/**
 * GET /{secure_path}/plans — dialect v2 bare wire DTO array (§6.2, W11),
 * mapped once at the API boundary to the admin major-unit domain model.
 */
export const fetchPlans = async (
  client: ApiClient,
  config?: QueryRequestConfig,
): Promise<AdminPlanModel[]> => {
  const operation = internalApiOperations.adminPlansList;
  const plans = await client.request({
    url: client.resolveAdminPath(internalApiPath(operation.adminPath)),
    method: operation.method,
    dialect: 'v2',
    expectedStatus: operation.successStatus,
    responseSchema: operation.responseSchema,
    ...config,
  });
  return plans.map(toAdminPlanDto).map(toAdminPlanModel);
};

/**
 * POST /{secure_path}/plans (201 `{id}`) / PATCH `plans/{id}` (204) — the
 * dialect-v2 upsert split (§6.2, W11). Prices ride as integer minor units; on
 * PATCH an empty price is a §4.4 clear (null). The caller supplies an explicit
 * wire DTO; no form-value or currency-unit coercion happens in the transport layer.
 * `force_update` is an edit-only body flag
 * — the create body denies it (`deny_unknown_fields`; there are no subscribers
 * to force yet).
 */
export const savePlan = (
  client: ApiClient,
  { id, force_update, ...data }: AdminPlanSaveRequest,
) => {
  if (id === undefined) {
    if (force_update !== undefined) {
      throw new TypeError('force_update is only valid when editing an existing plan');
    }
    const operation = internalApiOperations.adminPlanCreate;
    const request = operation.requestSchema.parse(pickPlanWriteBody(data));
    return client.request({
      url: client.resolveAdminPath(internalApiPath(operation.adminPath)),
      method: operation.method,
      dialect: 'v2',
      data: request,
      expectedStatus: operation.successStatus,
      responseSchema: operation.responseSchema,
    });
  }
  const operation = internalApiOperations.adminPlanPatch;
  const request = operation.requestSchema.parse({
    ...pickPlanWriteBody(data),
    ...(force_update === undefined ? {} : { force_update }),
  });
  return client.request({
    url: client.resolveAdminPath(internalApiPath(operation.adminPath, { id })),
    method: operation.method,
    dialect: 'v2',
    data: request,
    expectedStatus: operation.successStatus,
    responseSchema: operation.responseSchema,
  });
};

function pickPlanWriteBody(
  data: Omit<AdminPlanSaveRequest, 'id' | 'force_update'>,
): Record<string, unknown> {
  const next: Record<string, unknown> = {};
  for (const key of PLAN_SAVE_KEYS) {
    if (key === 'id' || key === 'force_update') continue;
    const value = data[key as keyof typeof data];
    if (value !== undefined) next[key] = value;
  }
  return next;
}

/** PATCH /{secure_path}/plans/{id} `{show|renew}` — the merged legacy toggle (§6.2). */
export const updatePlan = (
  client: ApiClient,
  id: number,
  key: 'show' | 'renew',
  value: boolean,
) => {
  const operation = internalApiOperations.adminPlanPatch;
  const data = { [key]: value } satisfies GeneratedPlanPatch;
  const request = operation.requestSchema.parse(data);
  return client.request({
    url: client.resolveAdminPath(internalApiPath(operation.adminPath, { id })),
    method: operation.method,
    dialect: 'v2',
    data: request,
    expectedStatus: operation.successStatus,
    responseSchema: operation.responseSchema,
  });
};

export const dropPlan = (client: ApiClient, id: number) => {
  const operation = internalApiOperations.adminPlanDelete;
  return client.request({
    url: client.resolveAdminPath(internalApiPath(operation.adminPath, { id })),
    method: operation.method,
    dialect: 'v2',
    expectedStatus: operation.successStatus,
    responseSchema: operation.responseSchema,
  });
};

/** POST /{secure_path}/plans/sort `{ids}` — `plan_ids` → `ids` (§6.2, §4.1). */
export const sortPlans = (client: ApiClient, ids: number[]) => {
  const operation = internalApiOperations.adminPlansSort;
  const data = {
    ids,
  } satisfies InternalApiOperationMap['adminPlansSort']['request'];
  const request = operation.requestSchema.parse(data);
  return client.request({
    url: client.resolveAdminPath(internalApiPath(operation.adminPath)),
    method: operation.method,
    dialect: 'v2',
    data: request,
    expectedStatus: operation.successStatus,
    responseSchema: operation.responseSchema,
  });
};

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
      total_amount: data.total_amount === '' ? undefined : decimalToCents(data.total_amount),
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
