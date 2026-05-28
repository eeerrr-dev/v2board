export interface TrafficLogEntry {
  u: number;
  d: number;
  record_at: number;
  user_id: number;
  server_rate: string;
}

export interface AdminStatSummary {
  month_income: number;
  month_register_total: number;
  ticket_pending_total: number;
  commission_pending_total: number;
  day_income: number;
  last_month_income: number;
  commission_month_payout: number;
  commission_last_month_payout: number;
}

export interface ServerRankItem {
  server_id: number;
  server_name: string;
  total: number;
}

export interface UserRankItem {
  user_id: number;
  email: string;
  total: number;
}

export interface OrderStatPoint {
  type: string;
  date: string;
  value: number;
}
