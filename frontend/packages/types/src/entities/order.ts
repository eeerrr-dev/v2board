import type { Plan, PlanPeriod } from './plan';

export type OrderStatus = 0 | 1 | 2 | 3 | 4;

export interface Order {
  trade_no: string;
  callback_no: string | null;
  plan_id: number;
  period: PlanPeriod | 'deposit';
  type: 1 | 2 | 3 | 4 | 9;
  total_amount: number;
  handling_amount: number | null;
  pre_handling_amount?: number;
  discount_amount: number | null;
  surplus_amount: number | null;
  refund_amount: number | null;
  balance_amount: number | null;
  surplus_order_ids: number[] | null;
  status: OrderStatus;
  commission_status: 0 | 1 | 2 | 3;
  commission_balance: number;
  payment_id: number | null;
  invite_user_id: number | null;
  actual_commission_balance?: number | null;
  coupon_id: number | null;
  paid_at: number | null;
  created_at: number;
  updated_at: number;
  plan?: Plan | { id: 0; name: 'deposit' };
  try_out_plan_id?: number;
  surplus_orders?: Order[];
  bounus?: number;
  get_amount?: number;
}

export interface OrderSavePayload {
  plan_id: number;
  period?: PlanPeriod | 'deposit';
  coupon_code?: string;
  deposit_amount?: number;
}

export interface OrderCheckoutPayload {
  trade_no: string;
  method: number;
}

export interface OrderCheckoutResult {
  type: -1 | 0 | 1 | 2;
  data: string | boolean;
}

export interface StripePaymentIntentPayload {
  trade_no: string;
  method: number;
}

export interface StripePaymentIntent {
  public_key: string;
  client_secret: string;
  amount: number;
  currency: string;
}

export interface AdminOrderRow extends Order {
  id: number;
  user_id: number;
  email?: string;
  plan_name?: string | null;
}
