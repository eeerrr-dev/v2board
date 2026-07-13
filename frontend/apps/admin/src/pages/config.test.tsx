import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ConfigPage, {
  adminSecurePathLocation,
  isBackendEnabled,
  parseBackendInteger,
} from './config';

// The config surface is a redesigned shadcn island (PageShell + section nav +
// Card sections of labeled shadcn controls) replacing the antd tabs / OneUI
// replica. All legacy DOM and source byte-pins are retired. What stays covered
// is the Tier-1 contract: fetch populates the fields, and each control persists
// the EXACT backend key with its legacy-coerced value through /config/save
// (per-field `{ [key]: value }`), plus the theme activate / theme-settings
// save payloads. The RHF/Zod section forms also keep rejected drafts inline,
// replace successful drafts with the refetched server value, and never treat a
// failed plans/template dependency as an empty successful result.

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
      frontend_theme_color: 'default',
      frontend_background_url: 'https://example.com/bg.png',
      frontend_custom_html: '<script src="https://example.com/widget.js"></script>',
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
  configData: undefined as unknown,
  refetch: vi.fn(),
  plansData: [
    { id: 1, name: '基础订阅' },
    { id: 2, name: '高级订阅' },
  ] as { id: number; name: string }[] | undefined,
  plansError: null as Error | null,
  plansPending: false,
  plansRefetch: vi.fn(),
  emailTemplatesData: ['default', 'notify'] as string[] | undefined,
  emailTemplatesError: null as Error | null,
  emailTemplatesPending: false,
  emailTemplatesRefetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  webhookMutateAsync: vi.fn(),
  testMailMutateAsync: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
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
    data: mocks.plansData,
    error: mocks.plansError,
    isError: mocks.plansError !== null,
    isPending: mocks.plansPending,
    refetch: mocks.plansRefetch,
  }),
  useConfig: () => ({
    error: null,
    isError: false,
    isPending: false,
    isFetching: false,
    refetch: mocks.refetch,
    data: mocks.configData,
  }),
  useEmailTemplates: () => ({
    data: mocks.emailTemplatesData,
    error: mocks.emailTemplatesError,
    isError: mocks.emailTemplatesError !== null,
    isPending: mocks.emailTemplatesPending,
    refetch: mocks.emailTemplatesRefetch,
  }),
  useSaveSystemConfigMutation: () => ({
    mutateAsync: mocks.saveMutateAsync,
    isPending: false,
  }),
  useSetTelegramWebhookMutation: () => ({
    mutateAsync: mocks.webhookMutateAsync,
    isPending: false,
  }),
  useTestSendMailMutation: () => ({
    mutateAsync: mocks.testMailMutateAsync,
    isPending: false,
  }),
}));

beforeEach(() => {
  mocks.configData = makeConfig();
  mocks.plansData = [
    { id: 1, name: '基础订阅' },
    { id: 2, name: '高级订阅' },
  ];
  mocks.plansError = null;
  mocks.plansPending = false;
  mocks.plansRefetch.mockReset().mockResolvedValue(undefined);
  mocks.emailTemplatesData = ['default', 'notify'];
  mocks.emailTemplatesError = null;
  mocks.emailTemplatesPending = false;
  mocks.emailTemplatesRefetch.mockReset().mockResolvedValue(undefined);
  mocks.refetch.mockReset().mockImplementation(async () => ({
    data: mocks.configData,
    error: null,
    isError: false,
  }));
  mocks.saveMutateAsync.mockReset().mockResolvedValue(undefined);
  mocks.webhookMutateAsync.mockReset().mockResolvedValue(undefined);
  mocks.testMailMutateAsync.mockReset().mockResolvedValue({ data: true, log: { email: 'a@b.c' } });
  mocks.toastSuccess.mockReset();
  mocks.toastError.mockReset();
  window.history.replaceState({}, '', '/admin-path#/config/system');
  // Radix Select pointer + scroll shims for happy-dom.
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

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ app_name: 'My New Site' }),
    );
  });

  it('does not save or refetch when an unchanged text field blurs', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-app_name'));
    await user.tab();

    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(mocks.refetch).not.toHaveBeenCalled();
  });

  it('serializes saves across configuration sections', async () => {
    let resolveFirstSave!: () => void;
    mocks.saveMutateAsync
      .mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            resolveFirstSave = resolve;
          }),
      )
      .mockResolvedValueOnce(undefined);
    const user = userEvent.setup();
    render(<ConfigPage />);

    const appName = screen.getByTestId('config-app_name');
    await user.clear(appName);
    await user.type(appName, '串行保存一');
    await user.tab();
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ app_name: '串行保存一' }),
    );

    await user.click(screen.getByTestId('config-tab-invite'));
    const inviteLimit = screen.getByTestId('config-invite_gen_limit');
    await user.clear(inviteLimit);
    await user.type(inviteLimit, '9');
    await user.tab();
    await waitFor(() =>
      expect(inviteLimit.closest('[data-slot="field"]')).toHaveAttribute('aria-busy', 'true'),
    );
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);

    act(() => resolveFirstSave());
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenNthCalledWith(2, { invite_gen_limit: 9 }),
    );
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

  it('trims comma fields and removes empty entries before saving', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-safe'));
    // email_whitelist_enable = 1 → the suffix field is shown.
    const input = screen.getByTestId('config-email_whitelist_suffix');
    await user.clear(input);
    await user.type(input, ' a.com, b.com, ,');
    await user.tab();

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        email_whitelist_suffix: ['a.com', 'b.com'],
      }),
    );
  });

  it('saves a cleared comma field as a real empty array', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-invite'));
    const input = screen.getByTestId('config-commission_withdraw_method');
    await user.clear(input);
    await user.tab();

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ commission_withdraw_method: [] }),
    );
  });

  it('replaces a successful local draft with the authoritative refetched value', async () => {
    const canonicalConfig = makeConfig();
    canonicalConfig.site.app_name = '服务端规范化名称';
    mocks.saveMutateAsync.mockImplementationOnce(async () => {
      mocks.configData = canonicalConfig;
    });
    const user = userEvent.setup();
    render(<ConfigPage />);

    const input = screen.getByTestId('config-app_name');
    await user.clear(input);
    await user.type(input, '本地草稿');
    await user.tab();

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(input).toHaveValue('服务端规范化名称'));
    expect(mocks.toastSuccess).toHaveBeenCalledWith('保存成功');
  });

  it('does not overwrite text entered while an earlier save is in flight', async () => {
    const canonicalConfig = makeConfig();
    canonicalConfig.site.app_name = '已保存的第一版';
    let resolveSave!: () => void;
    mocks.saveMutateAsync.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = () => {
            mocks.configData = canonicalConfig;
            resolve();
          };
        }),
    );
    const user = userEvent.setup();
    render(<ConfigPage />);

    const input = screen.getByTestId('config-app_name');
    await user.clear(input);
    await user.type(input, '已保存的第一版');
    await user.tab();
    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1));

    await user.click(input);
    await user.clear(input);
    await user.type(input, '尚未失焦的第二版');
    act(() => resolveSave());

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    expect(input).toHaveValue('尚未失焦的第二版');
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('queues the latest same-field value and never lets an older refresh reset it', async () => {
    const firstCanonical = makeConfig();
    firstCanonical.site.force_https = 0;
    const finalCanonical = makeConfig();
    finalCanonical.site.force_https = 1;
    let resolveFirstRefresh!: (result: {
      data: ReturnType<typeof makeConfig>;
      error: null;
      isError: false;
    }) => void;
    let resolveSecondSave!: () => void;
    mocks.saveMutateAsync.mockResolvedValueOnce(undefined).mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveSecondSave = resolve;
        }),
    );
    mocks.refetch
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveFirstRefresh = resolve;
          }),
      )
      .mockResolvedValueOnce({ data: finalCanonical, error: null, isError: false });
    const user = userEvent.setup();
    const view = render(<ConfigPage />);

    const toggle = screen.getByTestId('config-force_https');
    await user.click(toggle);
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    await user.click(toggle);

    // React Query publishes refetched data before the refetch promise settles.
    // Re-render with that stale snapshot and prove the in-flight queue protects
    // the newer local value even though it equals the original default.
    mocks.configData = firstCanonical;
    view.rerender(<ConfigPage />);
    expect(toggle).toHaveAttribute('aria-checked', 'true');

    act(() => {
      resolveFirstRefresh({ data: firstCanonical, error: null, isError: false });
    });
    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(2));
    expect(toggle).toHaveAttribute('aria-checked', 'true');

    act(() => resolveSecondSave());
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'));
    expect(mocks.saveMutateAsync.mock.calls).toEqual([[{ force_https: 0 }], [{ force_https: 1 }]]);
  });

  it('keeps a rejected draft and renders the mutation error inline', async () => {
    mocks.saveMutateAsync.mockRejectedValueOnce(new Error('保存被拒绝'));
    const user = userEvent.setup();
    render(<ConfigPage />);

    const input = screen.getByTestId('config-app_name');
    await user.clear(input);
    await user.type(input, '需要保留的草稿');
    await user.tab();

    const field = input.closest('[data-slot="field"]');
    expect(field).not.toBeNull();
    expect(await within(field as HTMLElement).findByRole('alert')).toHaveTextContent('保存被拒绝');
    expect(input).toHaveValue('需要保留的草稿');
    expect(mocks.refetch).not.toHaveBeenCalled();
    expect(mocks.toastSuccess).not.toHaveBeenCalled();
  });

  it('replaces the outer admin path after secure_path saves without refetching the old path', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-safe'));
    const input = screen.getByTestId('config-secure_path');
    await user.clear(input);
    await user.type(input, 'next-admin');
    await user.tab();

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ secure_path: 'next-admin' }),
    );
    await waitFor(() => expect(window.location.pathname).toBe('/next-admin'));
    expect(window.location.hash).toBe('#/config/system');
    expect(mocks.refetch).not.toHaveBeenCalled();
  });

  it('rejects an empty secure_path without invalidating the current admin route', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-safe'));
    const input = screen.getByTestId('config-secure_path');
    await user.clear(input);
    await user.tab();

    const field = input.closest('[data-slot="field"]');
    expect(field).not.toBeNull();
    expect(await within(field as HTMLElement).findByRole('alert')).toHaveTextContent(
      '后台路径不能为空',
    );
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(window.location.pathname).toBe('/admin-path');
  });

  it('gates sections when their dependent query fails and exposes a scoped retry', async () => {
    mocks.plansData = undefined;
    mocks.plansError = new Error('plans failed');
    mocks.emailTemplatesData = undefined;
    mocks.emailTemplatesError = new Error('templates failed');
    const user = userEvent.setup();
    render(<ConfigPage />);

    const plansError = screen.getByTestId('config-plans-error');
    expect(screen.queryByTestId('config-app_name')).not.toBeInTheDocument();
    await user.click(within(plansError).getByTestId('error-state-retry'));
    expect(mocks.plansRefetch).toHaveBeenCalledTimes(1);

    await user.click(screen.getByTestId('config-tab-email'));
    const templatesError = screen.getByTestId('config-email-templates-error');
    expect(screen.queryByTestId('config-email_host')).not.toBeInTheDocument();
    await user.click(within(templatesError).getByTestId('error-state-retry'));
    expect(mocks.emailTemplatesRefetch).toHaveBeenCalledTimes(1);
  });

  it('renders the native frontend color, background, and custom HTML controls', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-frontend'));

    expect(screen.getByTestId('config-frontend_theme_color')).toBeInTheDocument();
    expect(screen.getByTestId('config-frontend_background_url')).toBeInTheDocument();
    const customHtml = screen.getByTestId('config-frontend_custom_html');
    expect(customHtml).toHaveValue('<script src="https://example.com/widget.js"></script>');
    expect(screen.getByText(/仅供可信运维人员/)).toBeInTheDocument();

    await user.clear(customHtml);
    await user.type(customHtml, '<div data-widget="trusted" />');
    await user.tab();
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        frontend_custom_html: '<div data-widget="trusted" />',
      }),
    );
  });

  it('hides the conditional child field until its toggle is on', async () => {
    const canonicalConfig = makeConfig();
    canonicalConfig.safe.recaptcha_enable = 0;
    let resolveSave!: () => void;
    mocks.saveMutateAsync.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-safe'));
    expect(screen.getByTestId('config-recaptcha_key')).toBeInTheDocument();

    // recaptcha_enable = 1 → the keys show. The local form value must hide
    // them immediately, without waiting for the queued save or refetch.
    await user.click(screen.getByTestId('config-recaptcha_enable'));
    expect(screen.queryByTestId('config-recaptcha_key')).not.toBeInTheDocument();
    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ recaptcha_enable: 0 });
    expect(mocks.refetch).not.toHaveBeenCalled();

    // The successful refresh is authoritative. Model the backend-applied value
    // instead of returning the stale pre-save fixture, and ensure it cannot
    // re-open the conditional children.
    mocks.configData = canonicalConfig;
    act(() => resolveSave());
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId('config-recaptcha_key')).not.toBeInTheDocument();
  });

  it('passes the current Telegram token to the webhook action', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-telegram'));
    await user.click(screen.getByTestId('config-set-webhook'));
    await waitFor(() => expect(mocks.webhookMutateAsync).toHaveBeenCalledWith('0000000000:token'));
  });

  it('waits for the Telegram token save and authoritative refresh before setting webhook', async () => {
    let resolveSave!: () => void;
    mocks.saveMutateAsync.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-telegram'));
    const token = screen.getByTestId('config-telegram_bot_token');
    await user.clear(token);
    await user.type(token, '1111111111:current-token');
    await user.click(screen.getByTestId('config-set-webhook'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        telegram_bot_token: '1111111111:current-token',
      }),
    );
    expect(mocks.webhookMutateAsync).not.toHaveBeenCalled();

    const canonicalConfig = makeConfig();
    canonicalConfig.telegram.telegram_bot_token = '1111111111:current-token';
    mocks.configData = canonicalConfig;
    act(() => resolveSave());

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(mocks.webhookMutateAsync).toHaveBeenCalledWith('1111111111:current-token'),
    );
  });

  it('waits for the email section save and activation before testing mail', async () => {
    let resolveSave!: () => void;
    mocks.saveMutateAsync.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-email'));
    const fromAddress = screen.getByTestId('config-email_from_address');
    await user.clear(fromAddress);
    await user.type(fromAddress, 'fresh@example.com');
    await user.click(screen.getByTestId('config-test-mail'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        email_from_address: 'fresh@example.com',
      }),
    );
    expect(mocks.testMailMutateAsync).not.toHaveBeenCalled();

    const canonicalConfig = makeConfig();
    canonicalConfig.email.email_from_address = 'fresh@example.com';
    mocks.configData = canonicalConfig;
    act(() => resolveSave());

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.testMailMutateAsync).toHaveBeenCalledWith());
  });
});

describe('backend coercion helpers', () => {
  it('reads backend boolean-like values through parseInt', () => {
    expect(isBackendEnabled(1)).toBe(true);
    expect(isBackendEnabled('2')).toBe(true);
    expect(isBackendEnabled(0)).toBe(false);
    expect(isBackendEnabled('0')).toBe(false);
    expect(isBackendEnabled('')).toBe(false);
  });

  it('parses integer fields with parseInt semantics', () => {
    expect(parseBackendInteger('12px')).toBe(12);
    expect(Number.isNaN(parseBackendInteger(''))).toBe(true);
  });

  it('builds the canonical same-origin secure-path location without a trailing slash', () => {
    expect(adminSecurePathLocation('/new-admin/', '#/config/system')).toBe(
      '/new-admin#/config/system',
    );
    expect(adminSecurePathLocation('new-admin', '')).toBe('/new-admin#/config/system');
  });
});
