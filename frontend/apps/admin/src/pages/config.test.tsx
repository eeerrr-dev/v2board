import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { parseProblem } from '@v2board/api-client';
import { setAdminRuntimeConfig } from '@/test/runtime-config';
import ConfigPage, {
  adminSecurePathLocation,
  isBackendEnabled,
  parseBackendInteger,
  parseBackendNumber,
} from './config';

// The config surface is a redesigned shadcn island (PageShell + section nav +
// Card sections of labeled shadcn controls) replacing the antd tabs / OneUI
// replica. All legacy DOM and source byte-pins are retired. What stays covered
// is the Tier-1 contract: fetch populates the fields, and each control persists
// the EXACT backend key with its §4.1-typed value through PATCH config
// (per-field `{ [key]: value }`): real booleans for flags, JSON numbers for
// integers/rates/enums, real string arrays for lists, and the decimal-string
// `commission_withdraw_limit` exception. The RHF/Zod section forms also keep
// rejected drafts inline, replace successful drafts with the refetched server
// value, honour the §6.1 202 refetch-never-resubmit and 409 conflict flows,
// and never treat a failed plans/template dependency as an empty result.

function makeConfig() {
  return {
    ticket: { ticket_status: 0 },
    deposit: { deposit_bounus: ['50:18', '100:38'] },
    invite: {
      invite_force: true,
      invite_commission: 10,
      invite_gen_limit: 5,
      invite_never_expire: false,
      commission_first_time_enable: true,
      commission_auto_check_enable: true,
      commission_withdraw_limit: '100',
      commission_withdraw_method: ['支付宝', 'USDT'],
      withdraw_close_enable: false,
      commission_distribution_enable: true,
      commission_distribution_l1: 50,
      commission_distribution_l2: 30,
      commission_distribution_l3: 20,
    },
    site: {
      app_name: 'V2Board',
      app_description: 'V2Board is best!',
      app_url: 'https://example.com',
      force_https: true,
      logo: 'https://example.com/logo.png',
      subscribe_url: 'https://sub.example.com',
      subscribe_path: '/api/v1/client/subscribe',
      tos_url: 'https://example.com/tos',
      stop_register: false,
      try_out_plan_id: 1,
      try_out_hour: 24,
      currency: 'CNY',
      currency_symbol: '¥',
      legacy_hash_redirect_enable: true,
    },
    subscribe: {
      plan_change_enable: true,
      reset_traffic_method: 0,
      surplus_enable: true,
      allow_new_period: false,
      new_order_event_id: true,
      renew_order_event_id: false,
      change_order_event_id: true,
      show_info_to_server_enable: true,
      show_subscribe_method: 2,
      show_subscribe_expire: 30,
    },
    frontend: {
      frontend_theme_color: 'default',
      frontend_background_url: 'https://example.com/bg.png',
      chat_widget_provider: null,
      chat_widget_crisp_website_id: null,
      chat_widget_tawk_property_id: null,
      chat_widget_tawk_widget_id: null,
    },
    server: {
      server_api_url: 'https://node.example.com',
      server_token: '1234567890123456',
      server_pull_interval: 60,
      server_push_interval: 60,
      server_node_report_min_traffic: 0,
      server_device_online_min_traffic: 0,
      device_limit_mode: false,
    },
    email: {
      email_template: 'default',
      email_host: 'smtp.example.com',
      email_port: 465,
      email_encryption: 'ssl',
      email_username: 'mailer',
      email_password: 'password',
      email_from_address: 'noreply@example.com',
    },
    telegram: {
      telegram_bot_token: '0000000000:token',
      telegram_bot_enable: true,
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
      email_verify: true,
      email_gmail_limit_enable: true,
      safe_mode_enable: true,
      secure_path: 'admin-path',
      email_whitelist_enable: true,
      email_whitelist_suffix: ['qq.com', 'gmail.com'],
      recaptcha_enable: true,
      recaptcha_key: 'secret',
      recaptcha_site_key: 'site',
      register_limit_by_ip_enable: true,
      register_limit_count: 3,
      register_limit_expire: 60,
      password_limit_enable: true,
      password_limit_count: 5,
      password_limit_expire: 60,
    },
  };
}

const APPLIED = { activation: 'applied' as const };

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
  mocks.saveMutateAsync.mockReset().mockResolvedValue(APPLIED);
  mocks.webhookMutateAsync.mockReset().mockResolvedValue(undefined);
  // §6.1: POST test-mail returns bare `{sent, log}` with a nullable log line.
  mocks.testMailMutateAsync.mockReset().mockResolvedValue({ sent: true, log: null });
  mocks.toastSuccess.mockReset();
  mocks.toastError.mockReset();
  // History routing (docs/api-dialect.md §10.1): the admin app lives under
  // its injected basename, and the page URL is a path, not a hash.
  setAdminRuntimeConfig({ secure_path: 'admin-path' });
  window.history.replaceState({}, '', '/admin-path/config/system');
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
    // force_https = true → switch is on.
    expect(screen.getByTestId('config-force_https')).toHaveAttribute('aria-checked', 'true');
    // stop_register = false → switch is off.
    expect(screen.getByTestId('config-stop_register')).toHaveAttribute('aria-checked', 'false');
    // §10.3: the site group carries the legacy-hash redirect toggle.
    expect(screen.getByTestId('config-legacy_hash_redirect_enable')).toHaveAttribute(
      'aria-checked',
      'true',
    );
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
          new Promise<typeof APPLIED>((resolve) => {
            resolveFirstSave = () => resolve(APPLIED);
          }),
      )
      .mockResolvedValueOnce(APPLIED);
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

  it('saves a switch immediately with a real JSON boolean (§4.1)', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    // stop_register starts at false → toggling on sends true.
    await user.click(screen.getByTestId('config-stop_register'));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ stop_register: true }),
    );

    // force_https starts at true → toggling off sends false.
    await user.click(screen.getByTestId('config-force_https'));
    await waitFor(() => expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ force_https: false }));
  });

  it('saves enum selects as JSON integers and event selects as booleans', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-subscribe'));
    await user.click(screen.getByTestId('config-reset_traffic_method'));
    await user.click(await screen.findByRole('option', { name: '按月重置' }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ reset_traffic_method: 1 }),
    );

    // Order-event toggles keep their legacy '0'/'1' option ids but travel as
    // §4.1 booleans (renew_order_event_id starts false and displays 不执行).
    await user.click(screen.getByTestId('config-renew_order_event_id'));
    await user.click(await screen.findByRole('option', { name: '重置用户流量' }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ renew_order_event_id: true }),
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
      return APPLIED;
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
        new Promise<typeof APPLIED>((resolve) => {
          resolveSave = () => {
            mocks.configData = canonicalConfig;
            resolve(APPLIED);
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
    firstCanonical.site.force_https = false;
    const finalCanonical = makeConfig();
    finalCanonical.site.force_https = true;
    let resolveFirstRefresh!: (result: {
      data: ReturnType<typeof makeConfig>;
      error: null;
      isError: false;
    }) => void;
    let resolveSecondSave!: () => void;
    mocks.saveMutateAsync.mockResolvedValueOnce(APPLIED).mockImplementationOnce(
      () =>
        new Promise<typeof APPLIED>((resolve) => {
          resolveSecondSave = () => resolve(APPLIED);
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
    expect(mocks.saveMutateAsync.mock.calls).toEqual([
      [{ force_https: false }],
      [{ force_https: true }],
    ]);
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

  it('refetches without resubmitting when a save hits a stale-revision 409 (§6.1)', async () => {
    mocks.saveMutateAsync.mockRejectedValueOnce(
      parseProblem(
        {
          type: 'about:blank',
          title: 'Conflict',
          status: 409,
          code: 'config_revision_conflict',
          detail: '配置已被其他请求更新，请刷新后重试',
        },
        409,
      ),
    );
    const user = userEvent.setup();
    render(<ConfigPage />);

    const input = screen.getByTestId('config-app_name');
    await user.clear(input);
    await user.type(input, '落败的草稿');
    await user.tab();

    const field = input.closest('[data-slot="field"]');
    expect(field).not.toBeNull();
    expect(await within(field as HTMLElement).findByRole('alert')).toHaveTextContent(
      '配置已被其他请求更新，请刷新后重试',
    );
    // The winning config is refetched; the losing value is never resubmitted.
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
    expect(input).toHaveValue('落败的草稿');
    expect(mocks.toastSuccess).not.toHaveBeenCalled();
  });

  it('refetches and keeps the durable draft on 202 activation-pending (§6.1)', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending' });
    const user = userEvent.setup();
    render(<ConfigPage />);

    const input = screen.getByTestId('config-app_name');
    await user.clear(input);
    await user.type(input, '待激活名称');
    await user.tab();

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    // Durable but not yet active: keep the submitted draft, never resubmit.
    expect(input).toHaveValue('待激活名称');
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(mocks.toastSuccess).toHaveBeenCalledWith('保存成功', {
        description: '配置已保存，正在等待所有进程生效。',
      }),
    );

    // The durable value is the new baseline — an unchanged blur stays silent.
    await user.click(input);
    await user.tab();
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('keeps the admin base while a secure_path save is activation-pending (§6.1)', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending' });
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
    // The new path is not active yet, so the base must not move.
    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    expect(window.location.pathname).toBe('/admin-path/config/system');
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
    // The app-relative route survives the base move (history routing).
    await waitFor(() => expect(window.location.pathname).toBe('/next-admin/config/system'));
    expect(window.location.hash).toBe('');
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
    expect(window.location.pathname).toBe('/admin-path/config/system');
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

  it('renders the native frontend color and background controls without custom HTML', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-frontend'));

    expect(screen.getByTestId('config-frontend_theme_color')).toBeInTheDocument();
    expect(screen.getByTestId('config-frontend_background_url')).toBeInTheDocument();
    // docs/api-dialect.md §10.5: the custom HTML injection control is removed;
    // the typed chat-widget editor (§10.6) replaces it.
    expect(screen.queryByTestId('config-frontend_custom_html')).not.toBeInTheDocument();
    expect(screen.getByTestId('config-chat_widget_provider')).toBeInTheDocument();
  });

  it('drives the typed chat-widget editor with per-provider identifier fields', async () => {
    // The post-save refetch is authoritative, so each step models the
    // backend-applied value before saving (same pattern as the recaptcha test).
    const applied = (values: Record<string, unknown>) => {
      const config = makeConfig();
      Object.assign(config.frontend, values);
      mocks.configData = config;
    };
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-frontend'));

    // Provider off (null fixture): no identifier fields are shown.
    expect(screen.queryByTestId('config-chat_widget_crisp_website_id')).not.toBeInTheDocument();
    expect(screen.queryByTestId('config-chat_widget_tawk_property_id')).not.toBeInTheDocument();
    expect(screen.queryByTestId('config-chat_widget_tawk_widget_id')).not.toBeInTheDocument();

    // Selecting Crisp saves the exact backend key/value and reveals only the
    // Crisp identifier field.
    applied({ chat_widget_provider: 'crisp' });
    await user.click(screen.getByTestId('config-chat_widget_provider'));
    await user.click(await screen.findByRole('option', { name: 'Crisp' }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ chat_widget_provider: 'crisp' }),
    );
    const crispInput = await screen.findByTestId('config-chat_widget_crisp_website_id');
    expect(screen.queryByTestId('config-chat_widget_tawk_property_id')).not.toBeInTheDocument();
    applied({
      chat_widget_provider: 'crisp',
      chat_widget_crisp_website_id: '01234567-89ab-cdef-0123-456789abcdef',
    });
    await user.type(crispInput, '01234567-89ab-cdef-0123-456789abcdef');
    await user.tab();
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        chat_widget_crisp_website_id: '01234567-89ab-cdef-0123-456789abcdef',
      }),
    );

    // Switching to Tawk swaps the identifier fields.
    applied({ chat_widget_provider: 'tawk' });
    await user.click(screen.getByTestId('config-chat_widget_provider'));
    await user.click(await screen.findByRole('option', { name: 'Tawk.to' }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ chat_widget_provider: 'tawk' }),
    );
    expect(screen.queryByTestId('config-chat_widget_crisp_website_id')).not.toBeInTheDocument();
    expect(await screen.findByTestId('config-chat_widget_tawk_property_id')).toBeInTheDocument();
    expect(screen.getByTestId('config-chat_widget_tawk_widget_id')).toBeInTheDocument();

    // 关闭 serializes to the backend's clear value (empty string), never the
    // 'off' UI sentinel, and hides the identifier fields again.
    applied({ chat_widget_provider: '' });
    await user.click(screen.getByTestId('config-chat_widget_provider'));
    await user.click(await screen.findByRole('option', { name: '关闭' }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ chat_widget_provider: '' }),
    );
    await waitFor(() =>
      expect(screen.queryByTestId('config-chat_widget_tawk_property_id')).not.toBeInTheDocument(),
    );
  });

  it('hides the conditional child field until its toggle is on', async () => {
    const canonicalConfig = makeConfig();
    canonicalConfig.safe.recaptcha_enable = false;
    let resolveSave!: () => void;
    mocks.saveMutateAsync.mockImplementationOnce(
      () =>
        new Promise<typeof APPLIED>((resolve) => {
          resolveSave = () => resolve(APPLIED);
        }),
    );
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-tab-safe'));
    expect(screen.getByTestId('config-recaptcha_key')).toBeInTheDocument();

    // recaptcha_enable = true → the keys show. The local form value must hide
    // them immediately, without waiting for the queued save or refetch.
    await user.click(screen.getByTestId('config-recaptcha_enable'));
    expect(screen.queryByTestId('config-recaptcha_key')).not.toBeInTheDocument();
    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ recaptcha_enable: false });
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
        new Promise<typeof APPLIED>((resolve) => {
          resolveSave = () => resolve(APPLIED);
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
        new Promise<typeof APPLIED>((resolve) => {
          resolveSave = () => resolve(APPLIED);
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
  it('reads §4.1 wire booleans first, then legacy numeric spellings', () => {
    expect(isBackendEnabled(true)).toBe(true);
    expect(isBackendEnabled(false)).toBe(false);
    expect(isBackendEnabled(1)).toBe(true);
    expect(isBackendEnabled('2')).toBe(true);
    expect(isBackendEnabled(0)).toBe(false);
    expect(isBackendEnabled('0')).toBe(false);
    expect(isBackendEnabled('')).toBe(false);
  });

  it('coerces numeric fields to JSON numbers, clearing to null when empty (§4.4)', () => {
    expect(parseBackendInteger('12px')).toBe(12);
    expect(parseBackendInteger('')).toBeNull();
    expect(parseBackendNumber('12.5')).toBe(12.5);
    expect(parseBackendNumber('')).toBeNull();
  });

  it('re-roots the current route path under the new secure-path base', () => {
    // History routing (docs/api-dialect.md §10.1): a saved secure_path moves
    // the admin basename, keeping the app-relative route (and query) intact.
    expect(adminSecurePathLocation('/new-admin/', '/config/system')).toBe(
      '/new-admin/config/system',
    );
    expect(adminSecurePathLocation('new-admin', '/user?page=2')).toBe('/new-admin/user?page=2');
    // No usable current route falls back to the config page that saved it.
    expect(adminSecurePathLocation('new-admin', '')).toBe('/new-admin/config/system');
    expect(adminSecurePathLocation('new-admin', '/')).toBe('/new-admin/config/system');
  });
});
