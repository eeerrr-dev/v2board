import type { PlanPeriod, UserPlan } from './plan';

export type OrderStatus = 0 | 1 | 2 | 3 | 4;

/**
 * User order rows from GET /user/orders[/{trade_no}] (docs/api-dialect.md
 * §5.5, W4): RFC 3339 timestamps and a nullable RFC 3339 `paid_at`;
 * `status`/`type`/`commission_status` stay numeric enums (§4.1).
 */
export interface Order {
  trade_no: string;
  callback_no: string | null;
  plan_id: number;
  period: PlanPeriod | 'deposit';
  type: 1 | 2 | 3 | 4 | 9;
  total_amount: number;
  handling_amount: number | null;
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
  paid_at: string | null;
  created_at: string;
  updated_at: string;
  plan?: UserPlan | { id: 0; name: 'deposit' };
  try_out_plan_id?: number;
  surplus_orders?: Order[];
  bounus?: number;
  get_amount?: number;
}

/**
 * POST /user/orders (§5.5, §9.4): the discriminated create-order union. The
 * deposit arm replaced the legacy `plan_id: 0` + `period: "deposit"`
 * sentinel; `deposit_amount` is integer cents. When no coupon is applied the
 * `coupon_code` field is omitted entirely — never sent as `""` (§5.5).
 */
export type OrderCreatePayload =
  | { kind: 'plan'; plan_id: number; period: PlanPeriod; coupon_code?: string }
  | { kind: 'deposit'; deposit_amount: number };

/** POST /user/orders/{trade_no}/checkout — trade_no rides the path (§5.5). */
export interface OrderCheckoutPayload {
  trade_no: string;
  method_id: number;
}

/** The §9.3 checkout result union. */
export type OrderCheckoutResult =
  | { kind: 'qr_code'; payload: string }
  | { kind: 'redirect'; url: string }
  | { kind: 'settled' };

/** POST /user/orders/{trade_no}/stripe-intent — trade_no rides the path (§5.5). */
export interface StripePaymentIntentPayload {
  trade_no: string;
  method_id: number;
}

export interface StripePaymentIntent {
  public_key: string;
  client_secret: string;
  amount: number;
  currency: string;
}

/**
 * Legacy-dialect admin order rows (numeric flags, epoch timestamps) from the
 * admin order endpoints; W11 owns their dialect flip.
 */
/** One payment-reconciliation row (docs/api-dialect.md §6.4, W11). */
export interface PaymentReconciliation {
  id: number;
  payment_id: number;
  provider: string;
  reason: string;
  order_status: number;
  expected_amount: number;
  settled_amount: number;
  occurrence_count: number;
  trade_no: string | null;
  trade_no_hash: string;
  callback_no: string | null;
  callback_no_hash: string;
  resolution: string | null;
  resolved_at: string | null;
  first_seen_at: string;
  last_seen_at: string;
  payment_name?: string;
  payment_archived_at?: string | null;
}

/** One commission-log entry in the admin order detail (docs/api-dialect.md §6.4, W11). */
export interface AdminCommissionLog {
  id: number;
  user_id: number;
  invite_user_id: number | null;
  trade_no: string;
  order_amount: number;
  get_amount: number;
  created_at: string;
  updated_at: string;
}

/**
 * Admin order rows (docs/api-dialect.md §6.4, W11): RFC 3339 timestamps and a
 * nullable RFC 3339 `paid_at`; `status`/`type`/`commission_status` stay
 * numeric enums (§4.1). The list row carries `email`, `plan_name`, and
 * `payment_reconciliation_open_count`; the detail carries `commission_log[]`
 * and `payment_reconciliations[]`.
 */
export interface AdminOrderRow {
  id: number;
  user_id: number;
  email?: string;
  plan_name?: string | null;
  trade_no: string;
  callback_no: string | null;
  plan_id: number;
  period: PlanPeriod | 'deposit';
  type: 1 | 2 | 3 | 4 | 9;
  total_amount: number;
  handling_amount: number | null;
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
  paid_at: string | null;
  created_at: string;
  updated_at: string;
  payment_reconciliation_open_count?: number;
  commission_log?: AdminCommissionLog[];
  payment_reconciliations?: PaymentReconciliation[];
}
