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
  stripe_pk: string | null;
  withdraw_methods: string[];
  withdraw_close: 0 | 1;
  currency: string;
  currency_symbol: string;
  commission_distribution_enable: 0 | 1;
  commission_distribution_l1: string | null;
  commission_distribution_l2: string | null;
  commission_distribution_l3: string | null;
}

export interface AdminConfig {
  invite_force: 0 | 1;
  invite_commission: number;
  invite_gen_limit: number;
  invite_never_expire: 0 | 1;
  commission_distribution_enable: 0 | 1;
  commission_distribution_l1: number;
  commission_distribution_l2: number;
  commission_distribution_l3: number;
  withdraw_methods: string[];
  withdraw_close_enable: 0 | 1;
  commission_withdraw_limit: number;
  commission_withdraw_method: string[];
  app_name: string;
  app_description: string;
  app_url: string | null;
  logo: string;
  force_https: 0 | 1;
  stop_register: 0 | 1;
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
  try_out_plan_id: number | null;
  try_out_hour: number;
  allow_new_period: 0 | 1;
  ticket_status: 0 | 1 | 2;
  currency: string;
  currency_symbol: string;
  subscribe_url: string;
  subscribe_path: string;
  secure_path: string | null;
  frontend_theme: string;
  frontend_theme_sidebar: 'light' | 'dark';
  frontend_theme_header: 'light' | 'dark';
  frontend_theme_color: 'default' | 'darkblue' | 'black' | 'green';
  frontend_background_url: string | null;
  safe_mode_enable: 0 | 1;
  telegram_bot_enable: 0 | 1;
  telegram_bot_token: string | null;
  telegram_discuss_link: string | null;
  stripe_pk_live: string | null;
  stripe_sk_live: string | null;
  stripe_webhook_key: string | null;
  deposit_bounus: string[];
  available_payment_methods: string[];
}

export interface SystemStatus {
  schedule: boolean;
  horizon: boolean;
  logChannel: string;
  logLevel: string;
  cacheDriver: string;
  backendVersion: string;
  frontendVersion: string;
}

export interface QueueStats {
  failedJobs: number;
  jobsPerMinute: number;
  pausedMasters: number;
  periods: { failedJobs: number; recentJobs: number };
  processes: number;
  queueWithMaxRuntime: string | null;
  queueWithMaxThroughput: string | null;
  recentJobs: number;
  status: string;
  wait: Record<string, number>;
}
