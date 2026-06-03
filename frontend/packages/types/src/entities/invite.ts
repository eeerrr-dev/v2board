export interface InviteCode {
  id: number;
  user_id: number;
  code: string;
  status: 0 | 1;
  pv: number;
  created_at: number;
  updated_at: number;
}

export type InviteStat = [
  registered: number,
  validCommission: number,
  pendingCommission: number,
  commissionRate: number,
  availableCommission: number,
];

export interface InviteFetchResult {
  codes: InviteCode[];
  stat: InviteStat;
}

export interface CommissionDetail {
  id: number;
  trade_no: string;
  order_amount: number;
  get_amount: number;
  created_at: number;
}

export interface CommissionDetailPage {
  data: CommissionDetail[];
  total?: number;
}
