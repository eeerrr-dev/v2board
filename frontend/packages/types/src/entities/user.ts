import type { Plan } from './plan';

export interface UserInfo {
  email: string;
  transfer_enable: number;
  device_limit: number | null;
  last_login_at: number | null;
  created_at: number;
  banned: 0 | 1;
  auto_renewal: 0 | 1;
  remind_expire: 0 | 1;
  remind_traffic: 0 | 1;
  expired_at: number | null;
  balance: number;
  commission_balance: number;
  plan_id: number | null;
  discount: number | null;
  commission_rate: number | null;
  telegram_id: number | null;
  uuid: string;
  avatar_url: string;
}

export interface UserStat {
  pending_orders: number;
  pending_tickets: number;
}

// One entry from the backend USER_SESSIONS cache map (AuthService::sessions).
// The requesting session is identified explicitly; `auth_data` is retained only
// as the backend's redacted response-shape field and is never used as a bearer.
export interface ActiveSession {
  ip: string;
  login_at: number;
  ua: string;
  auth_data: string;
  current: boolean;
}

export type ActiveSessionMap = Record<string, ActiveSession>;

export interface SubscribeInfo {
  plan_id: number | null;
  token: string;
  expired_at: number | null;
  u: number;
  d: number;
  transfer_enable: number;
  device_limit: number | null;
  email: string;
  uuid: string;
  plan?: Plan;
  alive_ip: number;
  subscribe_url: string;
  reset_day: number | null;
  allow_new_period: 0 | 1;
}

export interface UserUpdatePayload {
  auto_renewal?: 0 | 1;
  remind_expire?: 0 | 1;
  remind_traffic?: 0 | 1;
}

export interface AdminUserRow {
  id: number;
  email: string;
  password: string;
  // The wire values are JSON numbers (adminUserSchema); the api-client
  // normalizer reformats these as fixed-decimal display strings (GiB / major
  // currency units) before they reach the app.
  balance: string;
  commission_balance: string;
  transfer_enable: string;
  device_limit: number | null;
  u: string;
  d: string;
  total_used: number | string;
  alive_ip: number;
  ips: string;
  plan_id: number | null;
  plan_name: string | null;
  group_id: number | null;
  expired_at: number | null;
  uuid: string;
  token: string;
  subscribe_url: string;
  banned: 0 | 1;
  is_admin: 0 | 1;
  is_staff: 0 | 1;
  invite_user_id: number | null;
  invite_user_email?: string | null;
  discount: number | null;
  commission_type?: 0 | 1 | 2 | null;
  commission_rate: number | null;
  speed_limit?: number | null;
  remarks?: string | null;
  telegram_id: number | null;
  last_login_at: number | null;
  created_at: number;
  updated_at: number;
}

export interface AdminUserUpdatePayload {
  id: number;
  email: string;
  password?: string;
  plan_id?: number | null;
  expired_at?: number | null;
  transfer_enable?: number | null;
  device_limit?: number | null;
  balance?: number;
  commission_balance?: number;
  commission_type?: number | string;
  commission_rate?: number | null;
  discount?: number | null;
  speed_limit?: number | null;
  u?: number;
  d?: number;
  remarks?: string | null;
  invite_user_email?: string | null;
  is_admin?: 0 | 1;
  is_staff?: 0 | 1;
  banned?: 0 | 1;
  remind_expire?: 0 | 1;
  remind_traffic?: 0 | 1;
}
