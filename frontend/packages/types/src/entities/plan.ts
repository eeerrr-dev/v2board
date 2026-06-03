export type PlanPeriod =
  | 'month_price'
  | 'quarter_price'
  | 'half_year_price'
  | 'year_price'
  | 'two_year_price'
  | 'three_year_price'
  | 'onetime_price'
  | 'reset_price';

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
