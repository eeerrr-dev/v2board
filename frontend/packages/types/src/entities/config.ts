export interface GuestConfig {
  tos_url: string | null;
  is_email_verify: 0 | 1;
  is_invite_force: 0 | 1;
  email_whitelist_suffix: string[] | 0;
  is_recaptcha: 0 | 1;
  recaptcha_site_key: string | null;
  app_description: string | null;
  app_url: string | null;
  logo: string | null;
}

export interface UserCommConfig {
  is_telegram: 0 | 1;
  telegram_discuss_link: string | null;
  withdraw_methods: string[];
  withdraw_close: 0 | 1;
  currency: string;
  currency_symbol: string;
  commission_distribution_enable: 0 | 1;
  commission_distribution_l1: string | number | null;
  commission_distribution_l2: string | number | null;
  commission_distribution_l3: string | number | null;
}

export interface AdminConfigFlat {
  invite_force: 0 | 1;
  invite_commission: number;
  invite_gen_limit: number;
  invite_never_expire: 0 | 1;
  commission_first_time_enable: 0 | 1;
  commission_auto_check_enable: 0 | 1;
  commission_distribution_enable: 0 | 1;
  commission_distribution_l1: number | string | null;
  commission_distribution_l2: number | string | null;
  commission_distribution_l3: number | string | null;
  withdraw_methods?: string[];
  withdraw_close_enable: 0 | 1;
  commission_withdraw_limit: number | string | null;
  commission_withdraw_method: string[];
  app_name: string;
  app_description: string;
  app_url: string | null;
  logo: string | null;
  force_https: 0 | 1;
  stop_register: 0 | 1;
  tos_url: string | null;
  email_verify: 0 | 1;
  email_whitelist_enable: 0 | 1;
  email_whitelist_suffix: string[];
  email_gmail_limit_enable: 0 | 1;
  recaptcha_enable: 0 | 1;
  recaptcha_key: string | null;
  recaptcha_site_key: string | null;
  register_limit_by_ip_enable: 0 | 1;
  register_limit_count: number;
  register_limit_expire: number;
  password_limit_enable: 0 | 1;
  password_limit_count: number;
  password_limit_expire: number;
  try_out_plan_id: number | string | null;
  try_out_hour: number | string;
  plan_change_enable: 0 | 1;
  reset_traffic_method: 0 | 1 | 2 | 3 | 4;
  surplus_enable: 0 | 1;
  allow_new_period: 0 | 1;
  new_order_event_id: 0 | 1;
  renew_order_event_id: 0 | 1;
  change_order_event_id: 0 | 1;
  show_info_to_server_enable: 0 | 1;
  show_subscribe_method: 0 | 1 | 2;
  show_subscribe_expire: number | string | null;
  ticket_status: 0 | 1 | 2;
  currency: string;
  currency_symbol: string;
  subscribe_url: string | null;
  subscribe_path: string | null;
  secure_path: string | null;
  frontend_theme_color: 'default' | 'darkblue' | 'black' | 'green';
  frontend_background_url: string | null;
  chat_widget_provider: string | null;
  chat_widget_crisp_website_id: string | null;
  chat_widget_tawk_property_id: string | null;
  chat_widget_tawk_widget_id: string | null;
  safe_mode_enable: 0 | 1;
  server_api_url: string | null;
  server_token: string | null;
  server_pull_interval: number | string;
  server_push_interval: number | string;
  server_node_report_min_traffic: number | string;
  server_device_online_min_traffic: number | string;
  device_limit_mode: 0 | 1;
  email_template: string;
  email_host: string | null;
  email_port: string | null;
  email_username: string | null;
  email_password: string | null;
  email_encryption: string | null;
  email_from_address: string | null;
  telegram_bot_enable: 0 | 1;
  telegram_bot_token: string | null;
  telegram_discuss_link: string | null;
  stripe_pk_live: string | null;
  stripe_sk_live: string | null;
  stripe_webhook_key: string | null;
  deposit_bounus: string[];
  available_payment_methods: string[];
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
  > &
    Partial<Pick<AdminConfigFlat, 'email_whitelist_suffix'>>;
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
  frontend: Pick<
    AdminConfigFlat,
    | 'frontend_theme_color'
    | 'frontend_background_url'
    | 'chat_widget_provider'
    | 'chat_widget_crisp_website_id'
    | 'chat_widget_tawk_property_id'
    | 'chat_widget_tawk_widget_id'
  >;
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
 * `/admin/config/fetch` may return every group or only the requested group.
 * Keep that partial response shape honest instead of inventing absent groups.
 */
export type AdminConfig = Partial<AdminConfigGroups> &
  Partial<AdminConfigFlat> & {
    tabs?: keyof AdminConfigGroups;
  };

export interface QueueStats {
  failedJobs: number;
  jobsPerMinute: number;
  pausedMasters: number;
  periods: { failedJobs: number; recentJobs: number };
  processes: number;
  queueWithMaxRuntime: string | null;
  queueWithMaxThroughput: string | null;
  recentJobs: number;
  status: string | boolean | null;
  wait: Record<string, number>;
}

export interface QueueWorkloadItem {
  name: string;
  processes: number;
  length: number;
  wait: number;
}
