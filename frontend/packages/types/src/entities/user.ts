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
  balance: number | string;
  commission_balance: number | string;
  transfer_enable: number | string;
  device_limit: number | null;
  u: number | string;
  d: number | string;
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
  discount: number | null;
  commission_rate: number | null;
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
  commission_rate?: number | null;
  discount?: number | null;
  speed_limit?: number | null;
  invite_user_email?: string | null;
  is_admin?: 0 | 1;
  is_staff?: 0 | 1;
  banned?: 0 | 1;
  remind_expire?: 0 | 1;
  remind_traffic?: 0 | 1;
}
