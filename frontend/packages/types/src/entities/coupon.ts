export type CouponType = 1 | 2;

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
