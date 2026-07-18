/**
 * GET /user/traffic-logs row (docs/api-dialect.md §5.4, W6): numeric
 * `server_rate`, RFC 3339 `record_at`.
 */
export interface TrafficLogEntry {
  u: number;
  d: number;
  record_at: string;
  user_id: number;
  server_rate: number;
}

/** GET /{secure_path}/stats/summary (docs/api-dialect.md §6.8, W14): integer-cent money. */
export interface AdminStatSummary {
  online_user?: number;
  month_income: number;
  month_register_total: number;
  day_register_total?: number;
  ticket_pending_total: number;
  commission_pending_total: number;
  payment_reconciliation_pending_total?: number;
  payment_reconciliation_pending_amount?: number;
  day_income: number;
  last_month_income: number;
  commission_month_payout: number;
  commission_last_month_payout: number;
}

/** GET /{secure_path}/stats/server-rank `?window=` row (§6.8, W14). */
export interface ServerRankItem {
  server_id: number;
  server_type: string;
  server_name: string | null;
  u: number;
  d: number;
  total: number;
}

/** GET /{secure_path}/stats/user-rank `?window=` row (§6.8, W14). */
export interface UserRankItem {
  user_id: number;
  email: string;
  u: number;
  d: number;
  total: number;
}

/**
 * GET /{secure_path}/stats/{orders,records} row (§6.8, W14): stable
 * snake_case `series` slugs; money series carry integer cents.
 */
export interface StatSeriesPoint {
  series: string;
  date: string;
  value: number;
}
