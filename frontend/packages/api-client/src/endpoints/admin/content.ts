import type { Coupon, Giftcard, InternalApiOperationMap, Knowledge, Notice } from '@v2board/types';
import type { ApiClient, BinaryApiResponse } from '../../client';
import { adminListQueryParams, type AdminListQuery } from '../../dialect';
import { internalApiCreatedInt32IdSchema } from '../../generated/internal-api';
import { requestInternal, requestInternalBinary } from '../../internal-operation';
import { decimalToCents } from '../../money';
import type { QueryRequestConfig } from './shared';

/**
 * GET /{secure_path}/notices — dialect v2 bare array (§6.3, W10). The list
 * stays deliberately unpaginated: the legacy route returned every row and no
 * pagination was invented.
 */
export const fetchNotices = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminNoticesList', {
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
    ? requestInternal(client, 'adminNoticesCreate', {
        data,
      })
    : requestInternal(client, 'adminNoticesUpdate', {
        path: { id },
        data,
      });

export const dropNotice = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminNoticesDelete', { path: { id } });

/** PATCH /{secure_path}/notices/{id} `{show}` — the merged legacy toggle (§6.3). */
export const showNotice = (client: ApiClient, id: number, show: boolean) =>
  requestInternal(client, 'adminNoticesUpdate', {
    path: { id },
    data: { show },
  });

/**
 * §6.3 (W10): the coupon/gift-card lists take §8 pagination plus the §7.2
 * `created_at` sort only — the legacy lists had no filter support and the
 * spec invents none.
 */
export type ContentListQuery = Pick<AdminListQuery, 'page' | 'per_page' | 'sort_by' | 'sort_dir'>;

type GeneratedCoupon = InternalApiOperationMap['adminCouponsList']['response']['items'][number];
type GeneratedGiftcard = InternalApiOperationMap['adminGiftCardsList']['response']['items'][number];

function toCoupon(coupon: GeneratedCoupon): Coupon {
  if (coupon.type !== 1 && coupon.type !== 2) {
    throw new TypeError(`Unsupported coupon type: ${coupon.type}`);
  }
  return {
    ...coupon,
    type: coupon.type,
    // Client-side money rule (§6.3): amount coupons stay integer cents on the
    // wire; the admin table displays decimal yuan.
    value: coupon.type === 1 ? coupon.value / 100 : coupon.value,
  };
}

function toGiftcard(giftcard: GeneratedGiftcard): Giftcard {
  if (![1, 2, 3, 4, 5].includes(giftcard.type)) {
    throw new TypeError(`Unsupported gift-card type: ${giftcard.type}`);
  }
  return {
    ...giftcard,
    type: giftcard.type as Giftcard['type'],
    // Client-side money rule (§6.3): amount cards stay integer cents on the wire.
    value: giftcard.type === 1 && giftcard.value !== null ? giftcard.value / 100 : giftcard.value,
  };
}

/** GET /{secure_path}/coupons — dialect v2 `{items, total}` page (§6.3, W10). */
export const fetchCoupons = async (
  client: ApiClient,
  query: ContentListQuery = {},
  config?: QueryRequestConfig,
) => {
  const page = await requestInternal(client, 'adminCouponsList', {
    query: adminListQueryParams(
      query,
    ) as InternalApiOperationMap['adminCouponsList']['parameters']['query'],
    ...config,
  });
  return { items: page.items.map(toCoupon), total: page.total };
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

export type GenerateCsvResponse = BinaryApiResponse<typeof internalApiCreatedInt32IdSchema>;

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
  requestInternalBinary(client, 'adminCouponsCreate', {
    data: {
      ...serializeCouponBody(data),
      ...(data.generate_count ? { generate_count: Number(data.generate_count) } : {}),
    } as InternalApiOperationMap['adminCouponsCreate']['request'],
  });

/** PATCH /{secure_path}/coupons/{id} — dialect v2 update (§6.3, W10). */
export const updateCoupon = (client: ApiClient, id: number, data: GenerateCouponPayload) =>
  requestInternal(client, 'adminCouponsUpdate', {
    path: { id },
    data: serializeCouponBody(data) as InternalApiOperationMap['adminCouponsUpdate']['request'],
  });

export const dropCoupon = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminCouponsDelete', { path: { id } });

/** PATCH /{secure_path}/coupons/{id} `{show}` — the merged legacy toggle (§6.3). */
export const showCoupon = (client: ApiClient, id: number, show: boolean) =>
  requestInternal(client, 'adminCouponsUpdate', {
    path: { id },
    data: { show },
  });

/** GET /{secure_path}/gift-cards — dialect v2 `{items, total}` page (§6.3, W10). */
export const fetchGiftcards = async (
  client: ApiClient,
  query: ContentListQuery = {},
  config?: QueryRequestConfig,
) => {
  const page = await requestInternal(client, 'adminGiftCardsList', {
    query: adminListQueryParams(
      query,
    ) as InternalApiOperationMap['adminGiftCardsList']['parameters']['query'],
    ...config,
  });
  return { items: page.items.map(toGiftcard), total: page.total };
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
  requestInternalBinary(client, 'adminGiftCardsCreate', {
    data: {
      ...serializeGiftcardBody(data),
      ...(data.generate_count ? { generate_count: Number(data.generate_count) } : {}),
    } as InternalApiOperationMap['adminGiftCardsCreate']['request'],
  });

/** PATCH /{secure_path}/gift-cards/{id} — dialect v2 update (§6.3, W10). */
export const updateGiftcard = (client: ApiClient, id: number, data: GenerateGiftcardPayload) =>
  requestInternal(client, 'adminGiftCardsUpdate', {
    path: { id },
    data: serializeGiftcardBody(data) as InternalApiOperationMap['adminGiftCardsUpdate']['request'],
  });

export const dropGiftcard = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminGiftCardsDelete', { path: { id } });

/** GET /{secure_path}/knowledge — dialect v2 bare summary array (§6.3, W10). */
export const fetchKnowledge = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminKnowledgeList', {
    ...config,
  });

/** GET /{secure_path}/knowledge/{id} — dialect v2 bare detail, raw stored body (§6.3). */
export const knowledgeDetail = (client: ApiClient, id: number, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminKnowledgeGet', {
    path: { id },
    ...config,
  });

/** GET /{secure_path}/knowledge-categories — dialect v2 bare name array (§6.3). */
export const knowledgeCategories = (client: ApiClient, config?: QueryRequestConfig) =>
  requestInternal(client, 'adminKnowledgeCategoriesList', { ...config });

export type SaveKnowledgePayload = Pick<Knowledge, 'body' | 'category' | 'language' | 'title'> & {
  id?: number;
};

/**
 * POST /{secure_path}/knowledge (201 `{id}`) / PATCH `knowledge/{id}` (204)
 * — the dialect-v2 upsert split (§6.3, W10).
 */
export const saveKnowledge = (client: ApiClient, { id, ...data }: SaveKnowledgePayload) =>
  id === undefined
    ? requestInternal(client, 'adminKnowledgeCreate', {
        data,
      })
    : requestInternal(client, 'adminKnowledgeUpdate', {
        path: { id },
        data,
      });

/** PATCH /{secure_path}/knowledge/{id} `{show}` — the merged legacy toggle (§6.3). */
export const showKnowledge = (client: ApiClient, id: number, show: boolean) =>
  requestInternal(client, 'adminKnowledgeUpdate', {
    path: { id },
    data: { show },
  });

export const dropKnowledge = (client: ApiClient, id: number) =>
  requestInternal(client, 'adminKnowledgeDelete', { path: { id } });

/** POST /{secure_path}/knowledge/sort `{ids}` — dialect v2 full resequencing (§6.3). */
export const sortKnowledge = (client: ApiClient, ids: number[]) =>
  requestInternal(client, 'adminKnowledgeSort', {
    data: { ids },
  });
