import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ConfigPage, { isLegacyChecked, parseLegacyInteger } from './config';

// The config surface is a redesigned shadcn island (PageShell + section nav +
// Card sections of labeled shadcn controls) replacing the antd tabs / OneUI
// replica. All legacy DOM and source byte-pins are retired. What stays covered
// is the Tier-1 contract: fetch populates the fields, and each control persists
// the EXACT backend key with its legacy-coerced value through /config/save
// (per-field `{ [key]: value }`), plus the theme activate / theme-settings
// save payloads.

function makeConfig() {
  return {
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
  };
}

const mocks = vi.hoisted(() => ({
  pathname: '/config/system',
  configData: undefined as unknown,
  refetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  saveThemeMutateAsync: vi.fn(),
  themeConfigMutateAsync: vi.fn(),
  webhookMutateAsync: vi.fn(),
  testMailMutateAsync: vi.fn(),
  themesRefetch: vi.fn(),
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
      classic: { name: '经典主题', description: '经典主题描述', configs: [] },
    },
  } as {
    active?: string;
    themes: Record<
      string,
      {
        name: string;
        description: string;
        configs: {
          field_name: string;
          field_type: string;
          label: string;
          placeholder?: string;
        }[];
      }
    >;
  },
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock('react-router', () => ({
  useLocation: () => ({ pathname: mocks.pathname }),
}));

vi.mock('@/lib/toast', () => ({
  toast: {
    success: mocks.toastSuccess,
    error: mocks.toastError,
    info: vi.fn(),
    loading: vi.fn(),
    dismiss: vi.fn(),
  },
}));

vi.mock('@/lib/queries', () => ({
  useAdminPlans: () => ({
    data: [
      { id: 1, name: '基础订阅' },
      { id: 2, name: '高级订阅' },
    ],
  }),
  useConfig: () => ({
    isPending: false,
    isFetching: false,
    refetch: mocks.refetch,
    data: mocks.configData,
  }),
  useEmailTemplates: () => ({ data: ['default', 'notify'] }),
  useThemeTemplates: () => ({ data: ['v2board'] }),
  useThemes: () => ({ refetch: mocks.themesRefetch, data: mocks.themesData }),
  useSaveConfigMutation: () => ({ mutateAsync: mocks.saveMutateAsync, isPending: false }),
  useThemeConfigMutation: () => ({ mutateAsync: mocks.themeConfigMutateAsync }),
  useSaveThemeConfigMutation: () => ({ mutateAsync: mocks.saveThemeMutateAsync, isPending: false }),
  useSetTelegramWebhookMutation: () => ({ mutateAsync: mocks.webhookMutateAsync, isPending: false }),
  useTestSendMailMutation: () => ({ mutateAsync: mocks.testMailMutateAsync, isPending: false }),
}));

function encodeExpectedThemeConfig(params: Record<string, unknown>) {
  const json = JSON.stringify(params);
  // eslint-disable-next-line @typescript-eslint/no-deprecated -- mirror encodeLegacyThemeConfig
  return window.btoa(unescape(encodeURIComponent(json)));
}

beforeEach(() => {
  mocks.pathname = '/config/system';
  mocks.configData = makeConfig();
  mocks.refetch.mockReset().mockResolvedValue(undefined);
  mocks.saveMutateAsync.mockReset().mockResolvedValue(undefined);
  mocks.saveThemeMutateAsync.mockReset().mockResolvedValue(undefined);
  mocks.themeConfigMutateAsync.mockReset().mockResolvedValue({ homepage: 'Hi' });
  mocks.webhookMutateAsync.mockReset().mockResolvedValue(undefined);
  mocks.testMailMutateAsync.mockReset().mockResolvedValue({ data: true, log: { email: 'a@b.c' } });
  mocks.themesRefetch.mockReset().mockResolvedValue(undefined);
  mocks.toastSuccess.mockReset();
  mocks.toastError.mockReset();
  // Radix Select / Dialog pointer + scroll shims for happy-dom.
  window.HTMLElement.prototype.scrollIntoView = vi.fn();
  window.HTMLElement.prototype.hasPointerCapture = vi.fn(() => false);
  window.HTMLElement.prototype.setPointerCapture = vi.fn();
  window.HTMLElement.prototype.releasePointerCapture = vi.fn();
});

describe('SystemConfigPage', () => {
  it('populates fields from the fetched config', () => {
    render(<ConfigPage />);
    expect(screen.getByTestId('config-app_name')).toHaveValue('V2Board');
    expect(screen.getByTestId('config-app_url')).toHaveValue('https://example.com');
    // force_https = 1 → switch is on.
    expect(screen.getByTestId('config-force_https')).toHaveAttribute('aria-checked', 'true');
    // stop_register = 0 → switch is off.
    expect(screen.getByTestId('config-stop_register')).toHaveAttribute('aria-checked', 'false');
  });

  it('saves a text field on blur with the exact key and raw string value', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    const input = screen.getByTestId('config-app_name');
    await user.clear(input);
    await user.type(input, 'My New Site');
    await user.tab();

    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ app_name: 'My New Site' }));
    expect(mocks.refetch).toHaveBeenCalled();
  });

  it('saves a switch immediately with the key coerced to 1/0', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    // stop_register starts at 0 → toggling on sends 1.
    await user.click(screen.getByTestId('config-stop_register'));
    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ stop_register: 1 }));

    // force_https starts at 1 → toggling off sends 0.
    await user.click(screen.getByTestId('config-force_https'));
    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ force_https: 0 }));
  });

  it('saves a select immediately with the chosen option value', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-subscribe'));
    await user.click(screen.getByTestId('config-reset_traffic_method'));
    await user.click(await screen.findByRole('option', { name: '按月重置' }));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ reset_traffic_method: '1' }),
    );
  });

  it('parseInt-coerces the invite number fields before saving', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-invite'));
    const input = screen.getByTestId('config-invite_commission');
    await user.clear(input);
    await user.type(input, '25');
    await user.tab();

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ invite_commission: 25 }),
    );
  });

  it('splits comma fields into an array before saving', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-safe'));
    // email_whitelist_enable = 1 → the suffix field is shown.
    const input = screen.getByTestId('config-email_whitelist_suffix');
    await user.clear(input);
    await user.type(input, 'a.com,b.com');
    await user.tab();

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        email_whitelist_suffix: ['a.com', 'b.com'],
      }),
    );
  });

  it('saves the light/dark theme switch as the exact string under the frontend key', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-frontend'));
    // frontend_theme_sidebar = 'light' → toggling off sends 'dark'.
    await user.click(screen.getByTestId('config-frontend_theme_sidebar'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ frontend_theme_sidebar: 'dark' }),
    );
  });

  it('hides the conditional child field until its toggle is on', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-safe'));
    expect(screen.getByTestId('config-recaptcha_key')).toBeInTheDocument();

    // recaptcha_enable = 1 → the keys show. Turning it off hides them.
    await user.click(screen.getByTestId('config-recaptcha_enable'));
    await waitFor(() =>
      expect(screen.queryByTestId('config-recaptcha_key')).not.toBeInTheDocument(),
    );
  });

  it('triggers the telegram webhook and test-mail side effects', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-telegram'));
    await user.click(screen.getByTestId('config-set-webhook'));
    await waitFor(() => expect(mocks.webhookMutateAsync).toHaveBeenCalled());

    await user.click(screen.getByTestId('config-tab-email'));
    await user.click(screen.getByTestId('config-test-mail'));
    await waitFor(() => expect(mocks.testMailMutateAsync).toHaveBeenCalled());
  });
});

describe('ThemeConfigPage', () => {
  beforeEach(() => {
    mocks.pathname = '/config/theme';
  });

  it('renders the theme cards with activate state', () => {
    render(<ConfigPage />);
    expect(screen.getByText('默认主题')).toBeInTheDocument();
    expect(screen.getByText('经典主题')).toBeInTheDocument();
    expect(screen.getByText('当前主题')).toBeInTheDocument();
    expect(screen.getByText('激活主题')).toBeInTheDocument();
  });

  it('activates a theme through the frontend_theme config key', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('theme-activate-classic'));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ frontend_theme: 'classic' }),
    );
    expect(mocks.themesRefetch).toHaveBeenCalled();
  });

  it('saves theme settings as base64-encoded JSON under {name, config}', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('theme-settings-default'));
    const dialog = await screen.findByTestId('theme-settings-dialog');
    await waitFor(() => expect(within(dialog).getByLabelText('首页标题')).toHaveValue('Hi'));

    await user.click(screen.getByTestId('theme-settings-save'));

    await waitFor(() =>
      expect(mocks.saveThemeMutateAsync).toHaveBeenCalledWith({
        name: 'default',
        config: encodeExpectedThemeConfig({ homepage: 'Hi' }),
      }),
    );
  });

  it('shows the loading spinner while themes are empty', () => {
    mocks.themesData = { active: undefined, themes: {} };
    render(<ConfigPage />);
    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByText('主题设置')).not.toBeInTheDocument();
  });
});

describe('legacy coercion helpers', () => {
  it('reads truthy legacy booleans through parseInt', () => {
    expect(isLegacyChecked(1)).toBe(true);
    expect(isLegacyChecked('2')).toBe(true);
    expect(isLegacyChecked(0)).toBe(false);
    expect(isLegacyChecked('0')).toBe(false);
    expect(isLegacyChecked('')).toBe(false);
  });

  it('parses integer fields with parseInt semantics', () => {
    expect(parseLegacyInteger('12px')).toBe(12);
    expect(Number.isNaN(parseLegacyInteger(''))).toBe(true);
  });
});
