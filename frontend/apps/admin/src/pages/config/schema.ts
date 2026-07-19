import { z } from 'zod';
import type { Control } from 'react-hook-form';
import type { AdminConfigGroups } from '@v2board/types';

export type ConfigGroupKey = keyof AdminConfigGroups;

export type ConfigFieldValue = string | number | boolean | string[] | null | undefined;
export type ConfigSectionValues = Record<string, ConfigFieldValue>;
export type SerializedConfigSave = <T>(operation: () => Promise<T>) => Promise<T>;

export interface FormCtx {
  control: Control<ConfigSectionValues>;
  get: (group: ConfigGroupKey, field: string) => ConfigFieldValue;
  isSaving: (field: string) => boolean;
  save: (group: ConfigGroupKey, field: string, value: ConfigFieldValue) => Promise<void>;
}

export const SECTION_FIELDS = {
  site: [
    'app_name',
    'app_description',
    'app_url',
    'force_https',
    'logo',
    'subscribe_url',
    'subscribe_path',
    'tos_url',
    'stop_register',
    'try_out_plan_id',
    'try_out_hour',
    'currency',
    'currency_symbol',
    'legacy_hash_redirect_enable',
  ],
  safe: [
    'email_verify',
    'email_gmail_limit_enable',
    'safe_mode_enable',
    'admin_mfa_force',
    'secure_path',
    'email_whitelist_enable',
    'email_whitelist_suffix',
    'recaptcha_enable',
    'recaptcha_key',
    'recaptcha_site_key',
    'register_limit_by_ip_enable',
    'register_limit_count',
    'register_limit_expire',
    'password_limit_enable',
    'password_limit_count',
    'password_limit_expire',
  ],
  subscribe: [
    'plan_change_enable',
    'reset_traffic_method',
    'surplus_enable',
    'allow_new_period',
    'new_order_event_id',
    'renew_order_event_id',
    'change_order_event_id',
    'show_info_to_server_enable',
    'show_subscribe_method',
    'show_subscribe_expire',
  ],
  deposit: ['deposit_bounus'],
  ticket: ['ticket_status'],
  invite: [
    'invite_force',
    'invite_commission',
    'invite_gen_limit',
    'invite_never_expire',
    'commission_first_time_enable',
    'commission_auto_check_enable',
    'commission_withdraw_limit',
    'commission_withdraw_method',
    'withdraw_close_enable',
    'commission_distribution_enable',
    'commission_distribution_l1',
    'commission_distribution_l2',
    'commission_distribution_l3',
  ],
  frontend: [
    'frontend_theme_color',
    'frontend_background_url',
    'chat_widget_provider',
    'chat_widget_crisp_website_id',
    'chat_widget_tawk_property_id',
    'chat_widget_tawk_widget_id',
  ],
  server: [
    'server_api_url',
    'server_token',
    'server_pull_interval',
    'server_push_interval',
    'server_node_report_min_traffic',
    'server_device_online_min_traffic',
    'device_limit_mode',
  ],
  email: [
    'email_host',
    'email_port',
    'email_encryption',
    'email_username',
    'email_password',
    'email_from_address',
    'email_template',
  ],
  telegram: ['telegram_bot_token', 'telegram_bot_enable', 'telegram_discuss_link'],
  app: [
    'windows_version',
    'windows_download_url',
    'macos_version',
    'macos_download_url',
    'android_version',
    'android_download_url',
  ],
} as const satisfies Record<ConfigGroupKey, readonly string[]>;

export const configFieldValueSchema = z.union([
  z.string(),
  z.number(),
  z.boolean(),
  z.array(z.string()),
  z.null(),
  z.undefined(),
]);

function createSectionSchema(
  fields: readonly string[],
): z.ZodType<ConfigSectionValues, ConfigSectionValues> {
  const allowed = new Set(fields);
  return z.record(z.string(), configFieldValueSchema).superRefine((values, ctx) => {
    for (const field of Object.keys(values)) {
      if (allowed.has(field)) continue;
      ctx.addIssue({ code: 'custom', path: [field], message: '配置字段不属于当前分组' });
    }
  });
}

export const SECTION_SCHEMAS: Record<
  ConfigGroupKey,
  z.ZodType<ConfigSectionValues, ConfigSectionValues>
> = {
  site: createSectionSchema(SECTION_FIELDS.site),
  safe: createSectionSchema(SECTION_FIELDS.safe),
  subscribe: createSectionSchema(SECTION_FIELDS.subscribe),
  deposit: createSectionSchema(SECTION_FIELDS.deposit),
  ticket: createSectionSchema(SECTION_FIELDS.ticket),
  invite: createSectionSchema(SECTION_FIELDS.invite),
  frontend: createSectionSchema(SECTION_FIELDS.frontend),
  server: createSectionSchema(SECTION_FIELDS.server),
  email: createSectionSchema(SECTION_FIELDS.email),
  telegram: createSectionSchema(SECTION_FIELDS.telegram),
  app: createSectionSchema(SECTION_FIELDS.app),
};

export const SECTIONS: { key: ConfigGroupKey; title: string }[] = [
  { key: 'site', title: '站点' },
  { key: 'safe', title: '安全' },
  { key: 'subscribe', title: '订阅' },
  { key: 'deposit', title: '充值' },
  { key: 'ticket', title: '工单' },
  { key: 'invite', title: '邀请&佣金' },
  { key: 'frontend', title: '个性化' },
  { key: 'server', title: '节点' },
  { key: 'email', title: '邮件' },
  { key: 'telegram', title: 'Telegram' },
  { key: 'app', title: 'APP' },
];
