import type {
  AdminOrderRow,
  AdminPlanDto,
  AdminPlanModel,
  InternalApiAdminPlanItem,
  InternalApiOperationMap,
  MoneyMajor,
  MoneyMinor,
  PaymentFormDefinition,
  PlanPeriod,
} from '@v2board/types';
import type { ApiClient } from '../../client';
import { adminListQueryParams, type FilterClause } from '../../dialect';
import {
  internalApiAdminPaymentCreateRequestSchema,
  internalApiPlanCreateSchema,
  internalApiPlanPatchSchema,
  internalApiPaymentProviderCodeSchema,
} from '../../generated/internal-api';
import { requestInternal } from '../../internal-operation';
import { decimalToCents, moneyMinorToMajor } from '../../money';
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

type GeneratedPlanCreate = InternalApiOperationMap['adminPlanCreate']['request'];
type GeneratedPlanPatch = InternalApiOperationMap['adminPlanPatch']['request'];
type GeneratedAdminOrder = InternalApiOperationMap['adminOrdersList']['response']['items'][number];
type BrandedPlanPrices<T> = Omit<T, PlanPeriod> & Partial<Record<PlanPeriod, MoneyMinor | null>>;
type AdminPlanCreateRequest = BrandedPlanPrices<GeneratedPlanCreate>;
type AdminPlanPatchRequest = BrandedPlanPrices<Omit<GeneratedPlanPatch, 'show' | 'renew'>>;

const ADMIN_ORDER_PERIODS = new Set<string>(['deposit', ...PLAN_PRICE_KEYS]);
const ADMIN_ORDER_TYPES = new Set([1, 2, 3, 4, 9]);
const ADMIN_ORDER_STATUSES = new Set([0, 1, 2, 3, 4]);
const ADMIN_ORDER_COMMISSION_STATUSES = new Set([0, 1, 2, 3]);

function toAdminOrderRow(order: GeneratedAdminOrder): AdminOrderRow {
  if (!ADMIN_ORDER_PERIODS.has(order.period)) {
    throw new TypeError(`Unsupported admin order period: ${order.period}`);
  }
  if (!ADMIN_ORDER_TYPES.has(order.type)) {
    throw new TypeError(`Unsupported admin order type: ${order.type}`);
  }
  if (!ADMIN_ORDER_STATUSES.has(order.status)) {
    throw new TypeError(`Unsupported admin order status: ${order.status}`);
  }
  if (!ADMIN_ORDER_COMMISSION_STATUSES.has(order.commission_status)) {
    throw new TypeError(`Unsupported admin order commission status: ${order.commission_status}`);
  }
  return {
    ...order,
    period: order.period as AdminOrderRow['period'],
    type: order.type as AdminOrderRow['type'],
    status: order.status as AdminOrderRow['status'],
    commission_status: order.commission_status as AdminOrderRow['commission_status'],
  };
}

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
  const plans = await requestInternal(client, 'adminPlansList', {
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
    return requestInternal(client, 'adminPlanCreate', {
      data: internalApiPlanCreateSchema.parse(pickPlanWriteBody(data)),
    });
  }
  return requestInternal(client, 'adminPlanPatch', {
    path: { id },
    data: internalApiPlanPatchSchema.parse({
      ...pickPlanWriteBody(data),
      ...(force_update === undefined ? {} : { force_update }),
    }),
  });
};

function pickPlanWriteBody(
  data: Omit<AdminPlanSaveRequest, 'id' | 'force_update'>,
): GeneratedPlanPatch {
  return {
    ...(data.name === undefined ? {} : { name: data.name }),
    ...(data.content === undefined ? {} : { content: data.content }),
    ...(data.group_id === undefined ? {} : { group_id: data.group_id }),
    ...(data.transfer_enable === undefined ? {} : { transfer_enable: data.transfer_enable }),
    ...(data.device_limit === undefined ? {} : { device_limit: data.device_limit }),
    ...(data.month_price === undefined ? {} : { month_price: data.month_price }),
    ...(data.quarter_price === undefined ? {} : { quarter_price: data.quarter_price }),
    ...(data.half_year_price === undefined ? {} : { half_year_price: data.half_year_price }),
    ...(data.year_price === undefined ? {} : { year_price: data.year_price }),
    ...(data.two_year_price === undefined ? {} : { two_year_price: data.two_year_price }),
    ...(data.three_year_price === undefined ? {} : { three_year_price: data.three_year_price }),
    ...(data.onetime_price === undefined ? {} : { onetime_price: data.onetime_price }),
    ...(data.reset_price === undefined ? {} : { reset_price: data.reset_price }),
    ...(data.reset_traffic_method === undefined
      ? {}
      : { reset_traffic_method: data.reset_traffic_method }),
    ...(data.capacity_limit === undefined ? {} : { capacity_limit: data.capacity_limit }),
    ...(data.speed_limit === undefined ? {} : { speed_limit: data.speed_limit }),
  };
}

/** PATCH /{secure_path}/plans/{id} `{show|renew}` — the merged legacy toggle (§6.2). */
export const updatePlan = (
  client: ApiClient,
  id: number,
  key: 'show' | 'renew',
  value: boolean,
) => {
  const data = { [key]: value } satisfies GeneratedPlanPatch;
  return requestInternal(client, 'adminPlanPatch', {
    path: { id },
    data,
  });
};

export const dropPlan = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminPlanDelete', { path: { id } });

/** POST /{secure_path}/plans/sort `{ids}` — `plan_ids` → `ids` (§6.2, §4.1). */
export const sortPlans = (client: ApiClient, ids: number[]) => {
  const data = {
    ids,
  } satisfies InternalApiOperationMap['adminPlansSort']['request'];
  return requestInternal(client, 'adminPlansSort', {
    data,
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
  const params = {
    ...adminListQueryParams({
      page: query.current,
      per_page: query.pageSize,
      sort_by: query.sort,
      sort_dir: query.sort_type ? (query.sort_type === 'ASC' ? 'asc' : 'desc') : undefined,
      filter: orderFilterClauses(query.filter),
    }),
    ...(query.commission_only === undefined ? {} : { commission_only: query.commission_only }),
  };
  const page = await requestInternal(client, 'adminOrdersList', {
    query: params as InternalApiOperationMap['adminOrdersList']['parameters']['query'],
    ...config,
  });
  return { data: page.items.map(toAdminOrderRow), total: page.total };
};

/** GET /{secure_path}/orders/{trade_no} — dialect v2 bare detail (§6.4, W11);
 * the read moved off POST and the identifier from numeric id to trade_no. */
export const orderDetail = (client: ApiClient, trade_no: string, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminOrdersGet', {
    path: { trade_no },
    ...config,
  });

/** POST /{secure_path}/orders/{trade_no}/mark-paid — 204 (§6.4). */
export const paidOrder = (client: ApiClient, trade_no: string) =>
  requestInternal(client, 'adminOrdersMarkPaid', { path: { trade_no } });

/** POST /{secure_path}/orders/{trade_no}/cancel — 204 (§6.4). */
export const cancelOrder = (client: ApiClient, trade_no: string) =>
  requestInternal(client, 'adminOrdersCancel', { path: { trade_no } });

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
  requestInternal(client, 'adminOrdersUpdate', {
    path: { trade_no },
    data: key === 'status' ? { status: Number(value) } : { commission_status: Number(value) },
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
  requestInternal(client, 'adminOrdersCreate', {
    data: {
      ...data,
      // Legacy '' meant "no charge"; omit so Rust's Option default (0) applies.
      total_amount: data.total_amount === '' ? undefined : decimalToCents(data.total_amount),
    },
  });

/** GET /{secure_path}/payments — dialect v2 bare array (§6.2, W11). */
export const fetchPayments = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminPaymentsList', {
    ...config,
  });

/** GET /{secure_path}/payment-providers — dialect v2 provider-code array (§6.2). */
export const paymentMethods = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminPaymentProvidersList', {
    ...config,
  });

type GeneratedPaymentCreate = InternalApiOperationMap['adminPaymentsCreate']['request'];
type GeneratedPaymentPatch = InternalApiOperationMap['adminPaymentsUpdate']['request'];
export type PaymentProviderCode =
  InternalApiOperationMap['adminPaymentProvidersList']['response'][number];
export const paymentProviderCodeSchema = internalApiPaymentProviderCodeSchema;

interface PaymentFormMetadata {
  name: string;
  icon?: string | null;
  notify_domain?: string | null;
  handling_fee_fixed?: string | number | null;
  handling_fee_percent?: string | number | null;
}

export interface SavePaymentCreatePayload extends PaymentFormMetadata {
  id?: undefined;
  payment: PaymentProviderCode;
  config: Record<string, string>;
}

export interface SavePaymentPatchPayload extends PaymentFormMetadata {
  id: number;
  payment?: never;
  config?: never;
}

export type SavePaymentPayload = SavePaymentCreatePayload | SavePaymentPatchPayload;

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
  requestInternal(client, 'adminPaymentProvidersForm', {
    path: { code: payment ?? '' },
    query: id === undefined ? undefined : { payment_id: id },
    ...config,
  }).then((definition): PaymentFormDefinition =>
    Object.fromEntries(
      Object.entries(definition).map(([key, field]) => [
        key,
        {
          label: field.label,
          description: field.description,
          type: field.type,
          ...(field.value == null ? {} : { value: field.value }),
        },
      ]),
    ),
  );

/**
 * The dialect-v2 payment body (§6.2, §4.4, W11): whitelist the columns the
 * upsert consumes (never echo fetched uuid/enable/sort/timestamps/notify_url).
 * On create an empty optional is absent (the documented default); on PATCH an
 * empty optional is an explicit `null` clear. `handling_fee_fixed` serializes
 * to cents and `handling_fee_percent` to a JSON number (§4.1).
 */
function paymentOptionalFields(
  data: PaymentFormMetadata,
  emptyMeansClear: boolean,
): Pick<
  GeneratedPaymentPatch,
  'icon' | 'notify_domain' | 'handling_fee_fixed' | 'handling_fee_percent'
> {
  const text = (value: string | null | undefined): string | null | undefined => {
    if (value === undefined || (value === '' && !emptyMeansClear)) return undefined;
    return value === '' ? null : value;
  };
  const number = (
    value: string | number | null | undefined,
    convert: (present: string | number) => number,
  ): number | null | undefined => {
    if (value === undefined || (value === '' && !emptyMeansClear)) return undefined;
    return value === '' || value === null ? null : convert(value);
  };
  return {
    icon: text(data.icon),
    notify_domain: text(data.notify_domain),
    handling_fee_fixed: number(data.handling_fee_fixed, decimalToCents),
    handling_fee_percent: number(data.handling_fee_percent, Number),
  };
}

function serializePaymentCreate(data: SavePaymentCreatePayload): GeneratedPaymentCreate {
  // The form definition is keyed dynamically for rendering, but the transport
  // boundary selects the generated provider-discriminated DTO and rejects a
  // missing, surplus, or cross-provider configuration field.
  return internalApiAdminPaymentCreateRequestSchema.parse({
    name: data.name,
    payment: data.payment,
    config: data.config,
    ...paymentOptionalFields(data, false),
  });
}

function serializePaymentPatch(data: SavePaymentPatchPayload): GeneratedPaymentPatch {
  return {
    name: data.name,
    ...paymentOptionalFields(data, true),
  };
}

/**
 * POST /{secure_path}/payments (201 `{id}`) / PATCH `payments/{id}` (204) — the
 * dialect-v2 upsert split (§6.2, W11); §4.4 replaces the legacy
 * present-but-empty=clear convention.
 */
export const savePayment = (client: ApiClient, data: SavePaymentPayload) =>
  data.id === undefined
    ? requestInternal(client, 'adminPaymentsCreate', {
        data: serializePaymentCreate(data),
      })
    : requestInternal(client, 'adminPaymentsUpdate', {
        path: { id: data.id },
        data: serializePaymentPatch(data),
      });

/** PATCH /{secure_path}/payments/{id} `{enable}` — the merged legacy toggle (§6.2). */
export const showPayment = (client: ApiClient, id: number, enable: boolean) =>
  requestInternal(client, 'adminPaymentsUpdate', {
    path: { id },
    data: { enable },
  });

/** POST /{secure_path}/payments/sort `{ids}` — 204 (§6.2). */
export const sortPayments = (client: ApiClient, ids: number[]) =>
  requestInternal(client, 'adminPaymentsSort', {
    data: { ids },
  });

export const dropPayment = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminPaymentsDelete', { path: { id } });
