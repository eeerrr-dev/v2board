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
 * Admin `GET /{secure_path}/coupons` row (docs/api-dialect.md §6.3, W10):
 * the same modern coupon body the user check route returns.
 */
export type Coupon = UserCoupon;

export interface CouponCheckPayload {
  code: string;
  plan_id: number;
}

/**
 * Admin `GET /{secure_path}/gift-cards` row (docs/api-dialect.md §6.3, W10):
 * RFC 3339 windows and a real `used_user_ids` array of redeemer ids.
 */
export interface Giftcard {
  id: number;
  name: string;
  code: string;
  type: 1 | 2 | 3 | 4 | 5;
  value: number | null;
  plan_id: number | null;
  limit_use: number | null;
  used_user_ids: number[];
  started_at: string;
  ended_at: string;
  created_at: string;
  updated_at: string;
}
