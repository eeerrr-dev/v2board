// Invite & commission family — modern dialect (docs/api-dialect.md §5.6,
// §8, §9.2, W7): RFC 3339 timestamps, the named stat object, and
// integer-cents commissions.

export interface InviteCode {
  id: number;
  code: string;
  pv: number;
  created_at: string;
  updated_at: string;
}

export interface InviteStat {
  registered_count: number;
  valid_commission: number;
  pending_commission: number;
  commission_rate: number;
  available_commission: number;
}

export interface InviteFetchResult {
  codes: InviteCode[];
  stat: InviteStat;
}

export interface CommissionDetail {
  id: number;
  trade_no: string;
  order_amount: number;
  get_amount: number;
  created_at: string;
}

export interface CommissionDetailPage {
  data: CommissionDetail[];
  total: number;
}
