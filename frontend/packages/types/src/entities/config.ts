/**
 * GET /public/config (docs/api-dialect.md §5.1, W3): boolean flags and an
 * always-array `email_whitelist_suffix`. Keeps its historical `Guest` name so
 * call sites stay stable while the wire moved off `/guest/comm/config`.
 */
export interface GuestConfig {
  tos_url: string | null;
  is_email_verify: boolean;
  is_invite_force: boolean;
  email_whitelist_suffix: string[];
  is_recaptcha: boolean;
  recaptcha_site_key: string | null;
  app_description: string | null;
  app_url: string | null;
  logo: string | null;
}

/**
 * GET /user/config (docs/api-dialect.md §5.3, W3): boolean flags and numeric
 * commission distribution rates. Keeps its historical `Comm` name so call
 * sites stay stable while the wire moved off `/user/comm/config`.
 */
export interface UserCommConfig {
  is_telegram: boolean;
  telegram_discuss_link: string | null;
  withdraw_methods: string[];
  withdraw_close: boolean;
  currency: string;
  currency_symbol: string;
  commission_distribution_enable: boolean;
  commission_distribution_l1: number | null;
  commission_distribution_l2: number | null;
  commission_distribution_l3: number | null;
}

/**
 * GET `/{secure_path}/config` (docs/api-dialect.md §6.1, W9): §4.1 native
 * JSON types — real booleans for every config flag, real string arrays, JSON
 * numbers for the §4.1 number inventory. `commission_withdraw_limit` stays a
 * decimal string (exact PostgreSQL NUMERIC round-trip, recorded §4.1
 * exception). `legacy_hash_redirect_enable` is the §10.3 site-group toggle.
 */
export interface AdminConfigFlat {
  invite_force: boolean;
  invite_commission: number;
  invite_gen_limit: number;
  invite_never_expire: boolean;
  commission_first_time_enable: boolean;
  commission_auto_check_enable: boolean;
  commission_distribution_enable: boolean;
  commission_distribution_l1: number | null;
  commission_distribution_l2: number | null;
  commission_distribution_l3: number | null;
  withdraw_close_enable: boolean;
  commission_withdraw_limit: string;
  commission_withdraw_method: string[];
  app_name: string;
  app_description: string | null;
  app_url: string | null;
  logo: string | null;
  force_https: boolean;
  stop_register: boolean;
  tos_url: string | null;
  email_verify: boolean;
  email_whitelist_enable: boolean;
  email_whitelist_suffix: string[];
  email_gmail_limit_enable: boolean;
  recaptcha_enable: boolean;
  recaptcha_key: string | null;
  recaptcha_site_key: string | null;
  register_limit_by_ip_enable: boolean;
  register_limit_count: number;
  register_limit_expire: number;
  password_limit_enable: boolean;
  password_limit_count: number;
  password_limit_expire: number;
  try_out_plan_id: number;
  try_out_hour: number;
  plan_change_enable: boolean;
  reset_traffic_method: 0 | 1 | 2 | 3 | 4;
  surplus_enable: boolean;
  allow_new_period: boolean;
  new_order_event_id: boolean;
  renew_order_event_id: boolean;
  change_order_event_id: boolean;
  show_info_to_server_enable: boolean;
  show_subscribe_method: 0 | 1 | 2;
  show_subscribe_expire: number;
  ticket_status: 0 | 1 | 2;
  currency: string;
  currency_symbol: string;
  subscribe_url: string | null;
  subscribe_path: string | null;
  secure_path: string;
  legacy_hash_redirect_enable: boolean;
  frontend_theme_color: 'default' | 'darkblue' | 'black' | 'green';
  frontend_background_url: string | null;
  safe_mode_enable: boolean;
  admin_mfa_force: boolean;
  server_api_url: string | null;
  server_token: string | null;
  server_pull_interval: number;
  server_push_interval: number;
  server_node_report_min_traffic: number;
  server_device_online_min_traffic: number;
  device_limit_mode: boolean;
  email_template: string;
  email_host: string | null;
  email_port: number | null;
  email_username: string | null;
  email_password: string | null;
  email_encryption: string | null;
  email_from_address: string | null;
  telegram_bot_enable: boolean;
  telegram_bot_token: string | null;
  telegram_discuss_link: string | null;
  deposit_bounus: string[];
  windows_version: string | null;
  windows_download_url: string | null;
  macos_version: string | null;
  macos_download_url: string | null;
  android_version: string | null;
  android_download_url: string | null;
}

export interface AdminConfigGroups {
  ticket: Pick<AdminConfigFlat, 'ticket_status'>;
  deposit: Pick<AdminConfigFlat, 'deposit_bounus'>;
  invite: Pick<
    AdminConfigFlat,
    | 'invite_force'
    | 'invite_commission'
    | 'invite_gen_limit'
    | 'invite_never_expire'
    | 'commission_first_time_enable'
    | 'commission_auto_check_enable'
    | 'commission_withdraw_limit'
    | 'commission_withdraw_method'
    | 'withdraw_close_enable'
    | 'commission_distribution_enable'
    | 'commission_distribution_l1'
    | 'commission_distribution_l2'
    | 'commission_distribution_l3'
  >;
  site: Pick<
    AdminConfigFlat,
    | 'logo'
    | 'force_https'
    | 'stop_register'
    | 'app_name'
    | 'app_description'
    | 'app_url'
    | 'subscribe_url'
    | 'subscribe_path'
    | 'try_out_plan_id'
    | 'try_out_hour'
    | 'tos_url'
    | 'currency'
    | 'currency_symbol'
    | 'legacy_hash_redirect_enable'
  >;
  subscribe: Pick<
    AdminConfigFlat,
    | 'plan_change_enable'
    | 'reset_traffic_method'
    | 'surplus_enable'
    | 'allow_new_period'
    | 'new_order_event_id'
    | 'renew_order_event_id'
    | 'change_order_event_id'
    | 'show_info_to_server_enable'
    | 'show_subscribe_method'
    | 'show_subscribe_expire'
  >;
  frontend: Pick<AdminConfigFlat, 'frontend_theme_color' | 'frontend_background_url'>;
  server: Pick<
    AdminConfigFlat,
    | 'server_api_url'
    | 'server_token'
    | 'server_pull_interval'
    | 'server_push_interval'
    | 'server_node_report_min_traffic'
    | 'server_device_online_min_traffic'
    | 'device_limit_mode'
  >;
  email: Pick<
    AdminConfigFlat,
    | 'email_template'
    | 'email_host'
    | 'email_port'
    | 'email_username'
    | 'email_password'
    | 'email_encryption'
    | 'email_from_address'
  >;
  telegram: Pick<
    AdminConfigFlat,
    'telegram_bot_enable' | 'telegram_bot_token' | 'telegram_discuss_link'
  >;
  app: Pick<
    AdminConfigFlat,
    | 'windows_version'
    | 'windows_download_url'
    | 'macos_version'
    | 'macos_download_url'
    | 'android_version'
    | 'android_download_url'
  >;
  safe: Pick<
    AdminConfigFlat,
    | 'email_verify'
    | 'safe_mode_enable'
    | 'admin_mfa_force'
    | 'secure_path'
    | 'email_whitelist_enable'
    | 'email_whitelist_suffix'
    | 'email_gmail_limit_enable'
    | 'recaptcha_enable'
    | 'recaptcha_key'
    | 'recaptcha_site_key'
    | 'register_limit_by_ip_enable'
    | 'register_limit_count'
    | 'register_limit_expire'
    | 'password_limit_enable'
    | 'password_limit_count'
    | 'password_limit_expire'
  >;
}

/**
 * GET `/{secure_path}/config` may return every group or only the `?group=`
 * requested one. Keep that partial response shape honest instead of
 * inventing absent groups.
 */
export type AdminConfig = Partial<AdminConfigGroups> &
  Partial<AdminConfigFlat> & {
    /** Active operator-config revision represented by this projection. */
    revision: number;
    tabs?: keyof AdminConfigGroups;
  };

/**
 * PATCH `/{secure_path}/config` (docs/api-dialect.md §6.1): 204 means the
 * write fully activated; 202 returns the durable revision that the admin UI
 * must observe on GET before it enables another config transaction.
 */
export type AdminConfigPatchResult =
  { activation: 'applied' } | { activation: 'pending'; revision: number };

/**
 * PATCH `/{secure_path}/config` changes. Every field is optional (absent
 * retains the current value) and every resettable field may be `null` (clear
 * to the backend-owned default), even when the corresponding GET projection
 * is non-null after default resolution. `secure_path` is the deliberate
 * exception: the backend requires an explicit non-empty replacement.
 */
export type AdminConfigChanges = {
  [Field in Exclude<keyof AdminConfigFlat, 'secure_path'>]?: AdminConfigFlat[Field] | null;
} & { secure_path?: NonNullable<AdminConfigFlat['secure_path']> };

/**
 * PATCH body. `expected_revision` is required and must be the positive token
 * from the GET projection on which the local draft was based.
 */
export type AdminConfigPatch = AdminConfigChanges & { expected_revision: number };

/** GET `/{secure_path}/system/queue-stats` (§6.1, W9): bare snake_case. */
export interface QueueStats {
  failed_jobs: number;
  jobs_per_minute: number;
  paused_masters: number;
  periods: { failed_jobs: number; recent_jobs: number };
  processes: number;
  queue_with_max_runtime: string | null;
  queue_with_max_throughput: string | null;
  recent_jobs: number;
  status: boolean;
  wait: Record<string, number>;
  last_run_at: Record<string, string>;
  last_success_at: Record<string, string>;
  last_failure_at: Record<string, string>;
}

/** GET `/{secure_path}/system/queue-workload` row (§6.1, W9). */
export interface QueueWorkloadItem {
  name: string;
  processes: number;
  length: number;
  wait: number;
  recent_jobs: number;
  failed_jobs: number;
  last_run_at: string | null;
  last_success_at: string | null;
  last_failure_at: string | null;
}
