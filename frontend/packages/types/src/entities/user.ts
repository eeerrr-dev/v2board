import type { UserPlan } from './plan';

/**
 * GET /user/profile (docs/api-dialect.md §5.3, W5): boolean flags (§4.1) and
 * RFC 3339 timestamps (§4.5). Money stays integer cents.
 */
export interface UserInfo {
  email: string;
  transfer_enable: number;
  device_limit: number | null;
  last_login_at: string | null;
  created_at: string;
  banned: boolean;
  auto_renewal: boolean;
  remind_expire: boolean;
  remind_traffic: boolean;
  expired_at: string | null;
  balance: number;
  commission_balance: number;
  plan_id: number | null;
  discount: number | null;
  commission_rate: number | null;
  telegram_id: number | null;
  uuid: string;
  avatar_url: string;
}

/** GET /user/stats (docs/api-dialect.md §9.1, W5): the named-count object. */
export interface UserStat {
  pending_order_count: number;
  pending_ticket_count: number;
  invited_user_count: number;
}

/**
 * One entry from GET /user/sessions (docs/api-dialect.md §5.3/§9.4, W5): the
 * legacy map key is `session_id`, `login_at` is RFC 3339, and the redacted
 * `auth_data` filler died with the map shape.
 */
export interface ActiveSession {
  session_id: string;
  ip: string;
  ua: string;
  login_at: string;
  current: boolean;
}

/**
 * GET /user/subscription (docs/api-dialect.md §5.4, W5): boolean
 * `allow_new_period`, RFC 3339 `expired_at`, explicit-null `plan` on the
 * modern §5.5 shape. The `subscribe_url`/token scheme inside stays frozen.
 */
export interface SubscribeInfo {
  plan_id: number | null;
  token: string;
  expired_at: string | null;
  u: number;
  d: number;
  transfer_enable: number;
  device_limit: number | null;
  email: string;
  uuid: string;
  plan: UserPlan | null;
  alive_ip: number;
  subscribe_url: string;
  reset_day: number | null;
  allow_new_period: boolean;
}

/**
 * PATCH /user/profile (docs/api-dialect.md §5.3, W5): boolean preference
 * flags; an absent field retains the stored value (§4.4).
 */
export interface UserUpdatePayload {
  auto_renewal?: boolean;
  remind_expire?: boolean;
  remind_traffic?: boolean;
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
  // §6.6 (W12): epoch fields cross as RFC 3339 UTC strings (nullable ones stay
  // null when unset); render through the `formatBackend*` date helpers.
  expired_at: string | null;
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
  last_login_at: string | null;
  created_at: string;
  updated_at: string;
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
