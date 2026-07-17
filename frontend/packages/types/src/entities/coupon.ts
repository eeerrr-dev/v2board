export type CouponType = 1 | 2;

/**
 * POST /user/coupons/check (docs/api-dialect.md §5.5, W4): bare coupon body
 * with a boolean `show` flag and RFC 3339 windows; `type` stays the numeric
 * 1 (fixed cents) / 2 (percent) enum.
 */
export interface UserCoupon {
  id: number;
  code: string;
  name: string;
  type: CouponType;
  value: number;
  show: boolean;
  limit_use: number | null;
  limit_use_with_user: number | null;
  limit_plan_ids: number[] | null;
  limit_period: string[] | null;
  started_at: string;
  ended_at: string;
  created_at: string;
  updated_at: string;
}

/**
 * Legacy-dialect coupon row (numeric flags, epoch timestamps), still
 * delivered by the admin coupon endpoints; W10 owns their dialect flip.
 */
export interface Coupon {
  id: number;
  code: string;
  name: string;
  type: CouponType;
  value: number;
  show: 0 | 1;
  limit_use: number | null;
  limit_use_with_user: number | null;
  limit_plan_ids: number[] | null;
  limit_period: string[] | null;
  started_at: number;
  ended_at: number;
  created_at: number;
  updated_at: number;
}

export interface CouponCheckPayload {
  code: string;
  plan_id: number;
}

export interface Giftcard {
  id: number;
  name: string;
  code: string;
  type: 1 | 2 | 3 | 4 | 5;
  value: number | null;
  plan_id: number | null;
  limit_use: number | null;
  used_user_ids: string | Array<number | string> | null;
  started_at: number | null;
  ended_at: number | null;
  created_at: number;
  updated_at: number;
}
