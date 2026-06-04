import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import ConfigPage, { isLegacyChecked, parseLegacyInteger } from './config';

const configSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'), 'utf8');
const queriesSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), '../lib/queries.ts'),
  'utf8',
);

type MockThemeData = {
  active?: string;
  themes: Record<
    string,
    {
      name: string;
      description: string;
      configs: Array<{
        field_name: string;
        field_type: string;
        label: string;
        placeholder?: string;
      }>;
    }
  >;
};

const mocks = vi.hoisted(() => ({
  pathname: '/config/theme',
  themesData: {
    active: 'default',
    themes: {
      default: {
        name: '默认主题',
        description: '默认主题描述',
        configs: [
          {
            field_name: 'homepage',
            field_type: 'input',
            label: '首页标题',
            placeholder: '请输入首页标题',
          },
        ],
      },
      classic: {
        name: '经典主题',
        description: '经典主题描述',
        configs: [],
      },
    },
  } as MockThemeData,
}));

vi.mock('react-router-dom', () => ({
  useLocation: () => ({ pathname: mocks.pathname }),
}));

vi.mock('@/lib/queries', () => ({
  useAdminPlans: () => ({
    data: [
      { id: 1, name: '基础订阅' },
      { id: 2, name: '高级订阅' },
    ],
  }),
  useConfig: () => ({
    isFetching: false,
    refetch: vi.fn(),
    data: {
      ticket: { ticket_status: 0 },
      deposit: { deposit_bounus: ['50:18', '100:38'] },
      invite: {
        invite_force: 1,
        invite_commission: 10,
        invite_gen_limit: 5,
        invite_never_expire: 0,
        commission_first_time_enable: 1,
        commission_auto_check_enable: 1,
        commission_withdraw_limit: 100,
        commission_withdraw_method: ['支付宝', 'USDT'],
        withdraw_close_enable: 0,
        commission_distribution_enable: 1,
        commission_distribution_l1: 50,
        commission_distribution_l2: 30,
        commission_distribution_l3: 20,
      },
      site: {
        app_name: 'V2Board',
        app_description: 'V2Board is best!',
        app_url: 'https://example.com',
        force_https: 1,
        logo: 'https://example.com/logo.png',
        subscribe_url: 'https://sub.example.com',
        subscribe_path: '/api/v1/client/subscribe',
        tos_url: 'https://example.com/tos',
        stop_register: 0,
        try_out_plan_id: 1,
        try_out_hour: 24,
        currency: 'CNY',
        currency_symbol: '¥',
      },
      subscribe: {
        plan_change_enable: 1,
        reset_traffic_method: 0,
        surplus_enable: 1,
        allow_new_period: 0,
        new_order_event_id: 1,
        renew_order_event_id: 0,
        change_order_event_id: 1,
        show_info_to_server_enable: 1,
        show_subscribe_method: 2,
        show_subscribe_expire: 30,
      },
      frontend: {
        frontend_theme: 'v2board',
        frontend_theme_sidebar: 'light',
        frontend_theme_header: 'dark',
        frontend_theme_color: 'default',
        frontend_background_url: 'https://example.com/bg.png',
      },
      server: {
        server_api_url: 'https://node.example.com',
        server_token: '1234567890123456',
        server_pull_interval: 60,
        server_push_interval: 60,
        server_node_report_min_traffic: 0,
        server_device_online_min_traffic: 0,
        device_limit_mode: 0,
      },
      email: {
        email_template: 'default',
        email_host: 'smtp.example.com',
        email_port: '465',
        email_encryption: 'ssl',
        email_username: 'mailer',
        email_password: 'password',
        email_from_address: 'noreply@example.com',
      },
      telegram: {
        telegram_bot_token: '0000000000:token',
        telegram_bot_enable: 1,
        telegram_discuss_link: 'https://t.me/example',
      },
      app: {
        windows_version: '1.0.0',
        windows_download_url: 'https://example.com/app.exe',
        macos_version: '1.0.0',
        macos_download_url: 'https://example.com/app.dmg',
        android_version: '1.0.0',
        android_download_url: 'https://example.com/app.apk',
      },
      safe: {
        email_verify: 1,
        email_gmail_limit_enable: 1,
        safe_mode_enable: 1,
        secure_path: 'admin-path',
        email_whitelist_enable: 1,
        email_whitelist_suffix: ['qq.com', 'gmail.com'],
        recaptcha_enable: 1,
        recaptcha_key: 'secret',
        recaptcha_site_key: 'site',
        register_limit_by_ip_enable: 1,
        register_limit_count: 3,
        register_limit_expire: 60,
        password_limit_enable: 1,
        password_limit_count: 5,
        password_limit_expire: 60,
      },
    },
  }),
  useEmailTemplates: () => ({ data: ['default', 'notify'] }),
  useThemeTemplates: () => ({ data: ['v2board'] }),
  useThemes: () => ({
    refetch: vi.fn(),
    data: mocks.themesData,
  }),
  useSaveConfigMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
  useThemeConfigMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useSaveThemeConfigMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
  useSetTelegramWebhookMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
  useTestSendMailMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
}));

describe('ConfigPage legacy theme config', () => {
  it('renders the original full page spinner while themes are empty', () => {
    mocks.themesData = { active: undefined, themes: {} };
    const html = renderToStaticMarkup(<ConfigPage />);

    expect(html).toContain('content content-full text-center pt-5');
    expect(html).toContain('spinner-grow text-primary');
    expect(html).toContain('Loading...');
    expect(html).not.toContain('主题配置将不会生效');
    expect(html).not.toContain('block block-transparent bg-image mb-0 mb-md-3 bg-primary');
  });

  it('renders /config/theme as the original theme manager cards', () => {
    mocks.themesData = {
      active: 'default',
      themes: {
        default: {
          name: '默认主题',
          description: '默认主题描述',
          configs: [
            {
              field_name: 'homepage',
              field_type: 'input',
              label: '首页标题',
              placeholder: '请输入首页标题',
            },
          ],
        },
        classic: {
          name: '经典主题',
          description: '经典主题描述',
          configs: [],
        },
      },
    };
    const html = renderToStaticMarkup(<ConfigPage />);

    expect(html).toContain('alert alert-warning mb-0 mb-md-4');
    expect(html).toContain('主题配置将不会生效');
    expect(html).toContain('前后分离');
    expect(html).toContain('block block-transparent bg-image mb-0 mb-md-3 bg-primary');
    expect(html).toContain('block-content block-content-full bg-gd-white-op-l');
    expect(html).toContain('d-md-flex justify-content-md-between align-items-md-center');
    expect(html).toContain('默认主题');
    expect(html).toContain('默认主题描述');
    expect(html).toContain('经典主题');
    expect(html).toContain('经典主题描述');
    expect(html).toContain('当前主题');
    expect(html).toContain('激活主题');
    expect(html).toContain('主题设置');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-typography');
  });

  it('keeps the original theme activation without a success toast', () => {
    const start = configSource.indexOf('const activateTheme = (name: string) => {');
    const end = configSource.indexOf('return (', start);

    expect(start).toBeGreaterThan(-1);
    expect(end).toBeGreaterThan(start);

    const activateThemeBlock = configSource.slice(start, end);
    expect(activateThemeBlock).toContain('mutateAsync({ frontend_theme: name })');
    expect(activateThemeBlock).toContain('void themes.refetch();');
    expect(activateThemeBlock).not.toContain("message.success('保存成功')");
    expect(configSource).toContain(
      'onSaved={() => {\n                      void themes.refetch();\n                    }}',
    );
  });

  it('keeps the original direct theme config data assignment', () => {
    expect(configSource).toContain('.then((data) => setParams(data))');
    expect(configSource).not.toContain('setParams(data ?? {})');
  });

  it('keeps theme and system config saves fetching from the page after success', () => {
    const themeSaveStart = configSource.indexOf(
      '.mutateAsync({ name: themeKey, config: encodeLegacyThemeConfig(params) })',
    );
    const themeRefetch = configSource.indexOf('onSaved();', themeSaveStart);
    const themeSuccess = configSource.indexOf("message.success('保存成功');", themeRefetch);
    const systemSaveStart = configSource.indexOf(
      'save\n        .mutateAsync(nextGroup as Partial<AdminConfigFlat>)',
    );
    const systemSuccess = configSource.indexOf("message.success('保存成功');", systemSaveStart);
    const systemRefetch = configSource.indexOf('void config.refetch();', systemSuccess);
    const themeHook = queriesSource.slice(
      queriesSource.indexOf('export function useSaveThemeConfigMutation()'),
      queriesSource.indexOf('export function useSetTelegramWebhookMutation()'),
    );
    const configHook = queriesSource.slice(
      queriesSource.indexOf('export function useSaveConfigMutation()'),
      queriesSource.indexOf('export function useSavePaymentMutation()'),
    );

    expect(themeSaveStart).toBeGreaterThan(-1);
    expect(themeRefetch).toBeGreaterThan(themeSaveStart);
    expect(themeSuccess).toBeGreaterThan(themeRefetch);
    expect(configSource).not.toContain('await onSaved();');
    expect(configSource).toContain('onSaved: () => void;');
    expect(systemSaveStart).toBeGreaterThan(-1);
    expect(systemSuccess).toBeGreaterThan(systemSaveStart);
    expect(systemRefetch).toBeGreaterThan(systemSuccess);
    expect(themeHook).not.toContain('onSuccess');
    expect(themeHook).not.toContain('adminKeys.themes');
    expect(configHook).not.toContain('onSuccess');
    expect(configHook).not.toContain('adminKeys.config');
  });

  it('keeps the original dynamic theme field structure and option rendering', () => {
    expect(configSource).toContain('{Object.entries(themeItems).map(([key, theme]) => {');
    const themeCardBlock = configSource.slice(
      configSource.indexOf('{Object.entries(themeItems).map(([key, theme]) => {'),
      configSource.indexOf('function ThemeSettingsButton('),
    );

    expect(themeCardBlock).not.toContain('key={key}');
    expect(configSource).toContain('{(theme.configs ?? []).map((field) => (');
    expect(configSource).toContain('<div className="form-group">');
    expect(configSource).toContain('const options = field.select_options as Record<string, string>;');
    expect(configSource).toContain('{Object.keys(options).map((key) => (');
    expect(configSource).toContain('<Select.Option value={key}>{options[key]}</Select.Option>');
    expect(configSource).not.toContain('<div className="form-group" key={field.field_name}>');
    expect(configSource).not.toContain('options={Object.entries(field.select_options ?? {}).map');
    expect(configSource).not.toContain('Object.keys(field.select_options ?? {})');
    expect(configSource).not.toContain('field.select_options?.[key]');
  });

  it('renders /config/system with the original tabbed auto-save blocks', () => {
    mocks.pathname = '/config/system';
    const html = renderToStaticMarkup(<ConfigPage />);

    expect(html).toContain('mb-0 block border-bottom');
    expect(html).toContain('站点');
    expect(html).toContain('安全');
    expect(html).toContain('订阅');
    expect(html).toContain('充值');
    expect(html).toContain('工单');
    expect(html).toContain('邀请&amp;佣金');
    expect(html).toContain('个性化');
    expect(html).toContain('节点');
    expect(html).toContain('邮件');
    expect(html).toContain('Telegram');
    expect(html).toContain('APP');
    expect(html).toContain('站点名称');
    expect(html).toContain('用于显示需要站点名称的地方。');
    expect(html).toContain('class="form-control"');
    expect(html).toContain('v2board-config-children');
    expect(html).not.toContain('ant-card');
    expect(html).not.toContain('ant-typography');
  });

  it('keeps the original split read and save keys for system config fields', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    expect(source).toMatch(
      /value=\{value\('subscribe', 'show_subscribe_expire'\)\}\s+onChange=\{\(next\) => setConfigValue\('safe', 'show_subscribe_expire', next\)\}/,
    );
    expect(source).toMatch(
      /checked=\{value\('frontend', 'frontend_theme_sidebar'\) === 'light'\}\s+onChange=\{\(checked\) =>\s+setConfigValue\('site', 'frontend_theme_sidebar', checked \? 'light' : 'dark'\)\s+\}/,
    );
    expect(source).toMatch(
      /checked=\{value\('frontend', 'frontend_theme_header'\) === 'light'\}\s+onChange=\{\(checked\) =>\s+setConfigValue\('site', 'frontend_theme_header', checked \? 'light' : 'dark'\)\s+\}/,
    );
    expect(source).toContain(
      "setConfigValue('frontend', 'frontend_theme_color', event.target.value)",
    );
    expect(source).toContain("setConfigValue('frontend', 'frontend_background_url', next)");
  });

  it('keeps the original child-field visibility coercion', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    expect(source).toContain("{value('site', 'try_out_plan_id') === 0 ? null : (");
    expect(source).toContain("{value('safe', 'email_whitelist_enable') ? (");
    expect(source).toContain("{value('safe', 'recaptcha_enable') ? (");
    expect(source).toContain("{value('safe', 'register_limit_by_ip_enable') ? (");
    expect(source).toContain("{value('safe', 'password_limit_enable') ? (");
    expect(source).toContain("{value('subscribe', 'show_subscribe_method') == 2 ? (");
    expect(source).not.toContain("Number(value('site', 'try_out_plan_id') ?? 0) === 0");
    expect(source).not.toContain("Number(value('subscribe', 'show_subscribe_method') ?? 0) === 2");
    expect(source).not.toContain("{isLegacyChecked(value('safe', 'email_whitelist_enable')) ? (");
    expect(source).not.toContain("{isLegacyChecked(value('safe', 'recaptcha_enable')) ? (");
    expect(source).not.toContain("{isLegacyChecked(value('safe', 'register_limit_by_ip_enable')) ? (");
    expect(source).not.toContain("{isLegacyChecked(value('safe', 'password_limit_enable')) ? (");
  });

  it('keeps original select value fallback rules', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    expect(source).toContain("value={legacySelectValue(value('site', 'try_out_plan_id'))}");
    expect(source).toContain("value={legacySelectValue(value('subscribe', 'reset_traffic_method'))}");
    expect(source).toContain("value={legacySelectValue(value('subscribe', 'show_subscribe_method'))}");
    expect(source).toContain("value={legacySelectValue(value('ticket', 'ticket_status') || 0)}");
    expect(source).toContain("value={legacySelectValue(value('email', 'email_template'))}");
    expect(source).toContain('value={legacySelectValue(value)}');
    expect(source).not.toContain("value={toText(value('site', 'try_out_plan_id') ?? 0)}");
    expect(source).not.toContain("value={toText(value('subscribe', 'reset_traffic_method') ?? 0)}");
    expect(source).not.toContain("value={toText(value('subscribe', 'show_subscribe_method') ?? 0)}");
    expect(source).not.toContain("value={toText(value('ticket', 'ticket_status') ?? 0)}");
    expect(source).not.toContain("value={toText(value('email', 'email_template'))}");
    expect(source).not.toContain('value={toText(value ?? 0)}');
  });

  it('keeps the original random option keys for trial plans and email templates', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    expect(source).toContain('<option key={Math.random()} value={plan.id}>');
    expect(source).toContain('<option key={Math.random()} value={template}>');
    expect(source).not.toContain('<option key={plan.id} value={plan.id}>');
    expect(source).not.toContain('<option key={template} value={template}>');
  });

  it('keeps the original telegram webhook action without passing the saved token', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    const setWebhookBlock = source.slice(
      source.indexOf('const setWebhook = () => {'),
      source.indexOf('  return (', source.indexOf('const setWebhook = () => {')),
    );
    expect(source).toContain('const setWebhook = () => {');
    expect(setWebhookBlock).toContain('webhook\n      .mutateAsync()');
    expect(setWebhookBlock).not.toContain("const token = toText(value('telegram', 'telegram_bot_token'))");
    expect(setWebhookBlock).not.toContain('mutateAsync(token)');
  });

  it('keeps the original test mail notification payload shape and debug log', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    expect(source).toContain("const title = failed ? '发送失败' : '发送成功';");
    expect(source).toContain('const content = (');
    expect(source).toContain('title,');
    expect(source).toContain('content,');
    expect(source).toContain('message: title,');
    expect(source).toContain('description: content,');
    expect(source).toContain('console.log(result);');
    expect(source).toContain('log?.config!.host');
    expect(source).toContain('log?.config!.port');
    expect(source).toContain('log?.config!.encryption');
    expect(source).toContain('log?.config!.username');
    expect(source).not.toContain("message: failed ? '发送失败' : '发送成功'");
    expect(source).not.toContain('log?.config?.host');
    expect(source).not.toContain('log?.config?.port');
    expect(source).not.toContain('log?.config?.encryption');
    expect(source).not.toContain('log?.config?.username');
  });

  it('keeps the original system config debounce as a single global timer', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    expect(source).toContain('const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)');
    expect(source).toContain('if (saveTimer.current) clearTimeout(saveTimer.current);');
    expect(source).toContain('saveTimer.current = null;');
    expect(source).not.toContain('saveTimers.current[parentKey]');
    expect(source).not.toContain('Partial<Record<ConfigGroupKey, ReturnType<typeof setTimeout>>>');
  });

  it('keeps the original grouped-only config state update shape', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );
    const updateStart = source.indexOf('const setConfigValue = (parentKey: ConfigGroupKey');
    const updateEnd = source.indexOf('  const sendTestMail = () => {', updateStart);
    const updateBlock = source.slice(updateStart, updateEnd);
    const returnBlock = updateBlock.slice(updateBlock.indexOf('      return {'));

    expect(updateBlock).toContain('[field]: nextValue,');
    expect(updateBlock).toContain('[parentKey]: nextGroup,');
    expect(returnBlock).toContain('[parentKey]: nextGroup,');
    expect(returnBlock).not.toContain('[field]: nextValue,');
  });

  it('keeps the original uncontrolled value fields for legacy inputs', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'config.tsx'),
      'utf8',
    );

    expect(source).toContain('defaultValue={toText(value)}');
    expect(source).not.toContain('value={toText(value)}');
    expect(source).toContain(
      "defaultValue={legacySelectValue(value('frontend', 'frontend_theme_color'))}",
    );
    expect(source).not.toContain(
      "defaultValue={toText(value('frontend', 'frontend_theme_color') ?? 'default')}",
    );
    expect(source).toContain("defaultValue={toText(value('server', 'server_pull_interval'))}");
    expect(source).not.toContain("value={toText(value('server', 'server_pull_interval'))}");
  });

  it('keeps the original parseInt coercion for switches and invite number fields', () => {
    expect(isLegacyChecked(1)).toBe(true);
    expect(isLegacyChecked('2')).toBe(true);
    expect(isLegacyChecked(0)).toBe(false);
    expect(isLegacyChecked('0')).toBe(false);
    expect(isLegacyChecked('')).toBe(false);
    expect(parseLegacyInteger('12px')).toBe(12);
    expect(Number.isNaN(parseLegacyInteger(''))).toBe(true);
  });
});
