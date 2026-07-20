import dayjs from 'dayjs';
import type { SelectorParam } from 'i18next';
import type { admin } from '@v2board/api-client';
import type { AdminPlanModel, Coupon, Giftcard } from '@v2board/types';
import { copyText } from '@v2board/config/clipboard';
import { toast } from '@/lib/toast';
import { Badge } from '@v2board/ui/badge';

export type CouponSubmit = admin.GenerateCouponPayload;
export type GiftcardSubmit = admin.GenerateGiftcardPayload;
export type GenerateResponse = admin.GenerateCsvResponse;
export type CouponRow = Coupon;
export type GiftcardRow = Giftcard;

export type Translate = (selector: SelectorParam) => string;

export const PAGE_SIZE_OPTIONS = [10, 50, 100, 150];

export function paginationLabels(translate: Translate) {
  return {
    itemsPerPage: translate(($) => $.common.items_per_page),
    nextPage: translate(($) => $.common.next_page),
    nextWindow: translate(($) => $.common.next_5),
    previousPage: translate(($) => $.common.prev_page),
    previousWindow: translate(($) => $.common.prev_5),
  };
}

export interface QueryState {
  current: number;
  pageSize: number;
}

export function planOptions(plans: AdminPlanModel[] | undefined) {
  return (plans ?? []).map((plan) => ({ value: `${plan.id}`, label: plan.name }));
}

// The validity window persists as a decimal unix-seconds string. Use Day.js's
// core unix() API here: the `X` format token requires AdvancedFormat and would
// otherwise be emitted literally as "X".
export function toDateTimeLocal(seconds?: number | string | null) {
  return seconds ? dayjs(1000 * Number(seconds)).format('YYYY-MM-DDTHH:mm') : '';
}

export function fromDateTimeLocal(value: string) {
  return value ? String(dayjs(value).unix()) : null;
}

// §4.5 (W10): fetched rows carry RFC 3339 windows; the editor form state keeps
// its unix-seconds strings, so record seeding converts at the edge.
export function rfc3339ToUnixInput(value?: string | null) {
  return value ? String(dayjs(value).unix()) : null;
}

export function dateRange(startedAt?: string | null, endedAt?: string | null) {
  return `${dayjs(startedAt).format('YYYY/MM/DD HH:mm')} ~ ${dayjs(endedAt).format(
    'YYYY/MM/DD HH:mm',
  )}`;
}

export function normalizeGenerationPayload<T extends object>(value: T) {
  return Object.fromEntries(
    Object.entries(value).filter(
      ([key, fieldValue]) =>
        fieldValue !== undefined && !(key === 'generate_count' && fieldValue === ''),
    ),
  ) as T;
}

// Preserve the CSV download contract for batch generation: the §6.3 create
// endpoint returns an arraybuffer of codes only when generate_count is set.
export function downloadGeneratedCsv(prefix: 'COUPON' | 'GIFTCARD', buffer: unknown) {
  const blob = new Blob([buffer as BlobPart], { type: 'text/plain,charset=UTF-8' });
  const url = window.URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.style.display = 'none';
  anchor.download = `${prefix} ${dayjs().format('YYYY-MM-DD HH:mm:ss')}.csv`;
  anchor.click();
  window.URL.revokeObjectURL(url);
}

export async function copyWithToast(text: string, translate: Translate) {
  if (await copyText(text)) toast.success(translate(($) => $.admin.coupons.copy_success));
  else toast.error(translate(($) => $.admin.coupons.copy_fail));
}

export function CopyableCode({
  value,
  onCopy,
}: {
  value: string;
  onCopy: (value: string) => Promise<void>;
}) {
  return (
    <button type="button" onClick={() => void onCopy(value)} className="inline-flex">
      <Badge variant="secondary" className="cursor-pointer font-mono">
        {value}
      </Badge>
    </button>
  );
}
