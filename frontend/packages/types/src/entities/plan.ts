export type PlanPeriod =
  | 'month_price'
  | 'quarter_price'
  | 'half_year_price'
  | 'year_price'
  | 'two_year_price'
  | 'three_year_price'
  | 'onetime_price'
  | 'reset_price';

/**
 * Legacy-dialect plan row (numeric flags, epoch timestamps). Still delivered
 * by /user/getSubscribe (W5) and the admin plan endpoints (W11); the user
 * commerce routes moved to {@link UserPlan} with W4.
 */
export interface Plan {
  id: number;
  group_id: number;
  transfer_enable: number;
  device_limit: number | null;
  speed_limit: number | null;
  reset_traffic_method: 0 | 1 | 2 | 3 | 4 | null;
  name: string;
  show: 0 | 1;
  sort: number | null;
  renew: 0 | 1;
  content: string | null;
  month_price: number | null;
  quarter_price: number | null;
  half_year_price: number | null;
  year_price: number | null;
  two_year_price: number | null;
  three_year_price: number | null;
  onetime_price: number | null;
  reset_price: number | null;
  capacity_limit: number | null;
  count?: number;
  created_at: number;
  updated_at: number;
}

/**
 * GET /user/plans and /user/plans/{id} (docs/api-dialect.md §5.5, W4):
 * boolean `show`/`renew` flags and RFC 3339 timestamps. `capacity_limit`
 * keeps the legacy remaining-capacity rewrite (sold out ⇒ ≤ 0).
 */
export interface UserPlan {
  id: number;
  group_id: number;
  transfer_enable: number;
  device_limit: number | null;
  speed_limit: number | null;
  reset_traffic_method: 0 | 1 | 2 | 3 | 4 | null;
  name: string;
  show: boolean;
  sort: number | null;
  renew: boolean;
  content: string | null;
  month_price: number | null;
  quarter_price: number | null;
  half_year_price: number | null;
  year_price: number | null;
  two_year_price: number | null;
  three_year_price: number | null;
  onetime_price: number | null;
  reset_price: number | null;
  capacity_limit: number | null;
  created_at: string;
  updated_at: string;
}
