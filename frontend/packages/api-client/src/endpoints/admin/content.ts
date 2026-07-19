import type { Coupon, Giftcard, Knowledge, Notice } from '@v2board/types';
import type { ApiClient, BinaryApiResponse } from '../../client';
import { adminListQueryParams, pageSchema, type AdminListQuery } from '../../dialect';
import { decimalToCents } from '../../money';
import {
  arraySchema,
  createdIdSchema,
  giftcardSchema,
  knowledgeSchema,
  knowledgeSummarySchema,
  noContentSchema,
  noticeSchema,
  stringArraySchema,
  userCouponSchema,
} from '../../contracts';
import type { QueryRequestConfig } from './shared';

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
    responseSchema: pageSchema(userCouponSchema),
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
