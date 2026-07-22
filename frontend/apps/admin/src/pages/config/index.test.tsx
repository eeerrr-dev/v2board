import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { parseProblem } from '@v2board/api-client';
import type * as ApiClientModule from '@v2board/api-client';
import type * as ReactRouterModule from 'react-router';
import { setAdminRuntimeConfig } from '@/test/runtime-config';
import ConfigPage from './index';
import {
  clearPendingConfigCommit,
  readPendingConfigCommit,
  subscribePendingConfigCommit,
  writePendingConfigCommit,
} from './pending-commit';
import {
  adminSecurePathLocation,
  isBackendEnabled,
  parseBackendInteger,
  parseBackendNumber,
} from './values';

function makeConfig() {
  return {
    revision: 7,
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
    },
    server: {
      server_api_url: 'https://node.example.com',
      server_token: '********',
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
      email_password: '********',
      email_from_address: 'noreply@example.com',
    },
    telegram: {
      telegram_bot_token: '********',
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
      admin_mfa_force: false,
      secure_path: 'admin-path',
      email_whitelist_enable: true,
      email_whitelist_suffix: ['qq.com', 'gmail.com'],
      recaptcha_enable: true,
      recaptcha_key: '********',
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
const PENDING_SESSION_KEY = 'v2board.admin.pending-config.v2';
const PENDING_FALLBACK_KEY = 'v2board.admin.pending-config-fallback.v2';
const storageRestorers: (() => void)[] = [];

function trackStorageSpy<T extends { mockRestore: () => void }>(spy: T): T {
  storageRestorers.push(() => spy.mockRestore());
  return spy;
}

const mocks = vi.hoisted(() => ({
  configData: undefined as unknown,
  configError: null as Error | null,
  refetch: vi.fn(),
  plansData: [
    { id: 1, name: '基础订阅' },
    { id: 2, name: '高级订阅' },
  ] as { id: number; name: string }[] | undefined,
  plansError: null as Error | null,
  plansRefetch: vi.fn(),
  emailTemplatesData: ['default', 'notify'] as string[] | undefined,
  emailTemplatesError: null as Error | null,
  emailTemplatesRefetch: vi.fn(),
  saveMutateAsync: vi.fn(),
  webhookMutateAsync: vi.fn(),
  testMailMutateAsync: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  blockerState: 'unblocked' as 'unblocked' | 'blocked' | 'proceeding',
  blockerProceed: vi.fn(),
  blockerReset: vi.fn(),
  useBlocker: vi.fn(),
  beforeUnload: undefined as ((event: BeforeUnloadEvent) => void) | undefined,
  probeConfigAtAdminPath: vi.fn(),
}));

vi.mock('@v2board/api-client', async () => {
  const actual = await vi.importActual<typeof ApiClientModule>('@v2board/api-client');
  return {
    ...actual,
    admin: {
      ...actual.admin,
      fetchConfigAtAdminPath: mocks.probeConfigAtAdminPath,
    },
  };
});

vi.mock('react-router', async () => {
  const actual = await vi.importActual<typeof ReactRouterModule>('react-router');
  return {
    ...actual,
    useBlocker: (when: boolean) => {
      mocks.useBlocker(when);
      return {
        state: when ? mocks.blockerState : 'unblocked',
        proceed: mocks.blockerProceed,
        reset: mocks.blockerReset,
      };
    },
    useBeforeUnload: (handler: (event: BeforeUnloadEvent) => void) => {
      mocks.beforeUnload = handler;
    },
  };
});

vi.mock('@v2board/app-shell/toast', () => ({
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
    isPending: false,
    refetch: mocks.plansRefetch,
  }),
  useConfig: () => ({
    error: mocks.configError,
    isError: mocks.configError !== null,
    isPending: mocks.configData === undefined && mocks.configError === null,
    refetch: mocks.refetch,
    data: mocks.configData,
  }),
  useEmailTemplates: () => ({
    data: mocks.emailTemplatesData,
    error: mocks.emailTemplatesError,
    isError: mocks.emailTemplatesError !== null,
    isPending: false,
    refetch: mocks.emailTemplatesRefetch,
  }),
  useSaveSystemConfigMutation: () => ({ mutateAsync: mocks.saveMutateAsync, isPending: false }),
  useSetTelegramWebhookMutation: () => ({
    mutateAsync: mocks.webhookMutateAsync,
    isPending: false,
  }),
  useTestSendMailMutation: () => ({ mutateAsync: mocks.testMailMutateAsync, isPending: false }),
}));

beforeEach(() => {
  window.sessionStorage.clear();
  window.localStorage.clear();
  window.localStorage.setItem('v2board.admin_auth_data', 'admin-test-token');
  // Reconcile the module-level fail-closed snapshot with the newly emptied
  // test stores before a case makes either Storage implementation throw.
  readPendingConfigCommit();
  mocks.configData = makeConfig();
  mocks.configError = null;
  mocks.plansData = [
    { id: 1, name: '基础订阅' },
    { id: 2, name: '高级订阅' },
  ];
  mocks.plansError = null;
  mocks.emailTemplatesData = ['default', 'notify'];
  mocks.emailTemplatesError = null;
  mocks.plansRefetch.mockReset().mockResolvedValue(undefined);
  mocks.emailTemplatesRefetch.mockReset().mockResolvedValue(undefined);
  mocks.refetch.mockReset().mockImplementation(async () => ({
    data: mocks.configData,
    error: null,
    isError: false,
  }));
  mocks.saveMutateAsync.mockReset().mockResolvedValue(APPLIED);
  mocks.webhookMutateAsync.mockReset().mockResolvedValue(undefined);
  mocks.testMailMutateAsync.mockReset().mockResolvedValue({ sent: true, log: null });
  mocks.toastSuccess.mockReset();
  mocks.toastError.mockReset();
  mocks.blockerState = 'unblocked';
  mocks.blockerProceed.mockReset();
  mocks.blockerReset.mockReset();
  mocks.useBlocker.mockReset();
  mocks.beforeUnload = undefined;
  mocks.probeConfigAtAdminPath.mockReset().mockImplementation(() => new Promise(() => {}));
  setAdminRuntimeConfig({ secure_path: 'admin-path' });
  window.history.replaceState({}, '', '/admin-path/config/system');
  window.HTMLElement.prototype.scrollIntoView = vi.fn();
  window.HTMLElement.prototype.hasPointerCapture = vi.fn(() => false);
  window.HTMLElement.prototype.setPointerCapture = vi.fn();
  window.HTMLElement.prototype.releasePointerCapture = vi.fn();
});

afterEach(() => {
  for (const restore of storageRestorers.splice(0).reverse()) restore();
  vi.restoreAllMocks();
});

describe('SystemConfigPage section drafts', () => {
  it('populates fetched values and starts with clean actions disabled', () => {
    render(<ConfigPage />);
    expect(screen.getByTestId('config-app_name')).toHaveValue('V2Board');
    expect(screen.getByTestId('config-force_https')).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId('config-save')).toBeDisabled();
    expect(screen.getByTestId('config-discard')).toBeDisabled();
  });

  it('stages edits locally and sends the complete dirty change-set in one PATCH', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    const name = screen.getByTestId('config-app_name');
    await user.clear(name);
    await user.type(name, 'My New Site');
    await user.tab();
    await user.click(screen.getByTestId('config-force_https'));

    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByTestId('config-tab-invite')).toBeDisabled();
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        app_name: 'My New Site',
        force_https: false,
      }),
    );
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
    expect(mocks.refetch).toHaveBeenCalledTimes(1);
  });

  it('keeps the GET revision that the dirty draft was based on', async () => {
    const user = userEvent.setup();
    const view = render(<ConfigPage />);

    const name = screen.getByTestId('config-app_name');
    await user.clear(name);
    await user.type(name, 'Draft from revision 7');

    // A background observation must not silently rebase an already-dirty
    // form onto a winning revision that the operator has never reviewed.
    mocks.configData = {
      ...makeConfig(),
      revision: 8,
      site: { ...makeConfig().site, app_name: 'Winner at revision 8' },
    };
    view.rerender(<ConfigPage />);
    expect(name).toHaveValue('Draft from revision 7');

    await user.click(screen.getByTestId('config-save'));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        app_name: 'Draft from revision 7',
      }),
    );
  });

  it('discards a section draft without a request and unlocks section navigation', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    const name = screen.getByTestId('config-app_name');
    await user.clear(name);
    await user.type(name, 'Discard me');
    expect(screen.getByTestId('config-tab-invite')).toBeDisabled();
    await user.click(screen.getByTestId('config-discard'));

    expect(name).toHaveValue('V2Board');
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByTestId('config-tab-invite')).toBeEnabled();
  });

  it('guards SPA and hard navigation while a section transaction is dirty', async () => {
    mocks.blockerState = 'blocked';
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.type(screen.getByTestId('config-app_name'), ' changed');
    expect(mocks.useBlocker).toHaveBeenLastCalledWith(true);
    expect(await screen.findByTestId('config-leave-dialog')).toHaveTextContent(
      '本组配置还没有保存，是否离开',
    );

    const hardLeave = new Event('beforeunload', { cancelable: true }) as BeforeUnloadEvent;
    mocks.beforeUnload?.(hardLeave);
    expect(hardLeave.defaultPrevented).toBe(true);

    await user.click(screen.getByTestId('config-stay'));
    expect(mocks.blockerReset).toHaveBeenCalledOnce();
    expect(mocks.blockerProceed).not.toHaveBeenCalled();
  });

  it('keeps JSON number/array coercions inside the draft until Save', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-invite'));

    const limit = screen.getByTestId('config-invite_gen_limit');
    const methods = screen.getByTestId('config-commission_withdraw_method');
    await user.clear(limit);
    await user.type(limit, '9');
    await user.tab();
    await user.clear(methods);
    await user.type(methods, ' a.com, b.com, ,');
    await user.tab();
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();

    await user.click(screen.getByTestId('config-save'));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        invite_gen_limit: 9,
        commission_withdraw_method: ['a.com', 'b.com'],
      }),
    );
  });

  it('canonicalizes a focused integer field when Enter submits without blur', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-invite'));

    const limit = screen.getByTestId('config-invite_gen_limit');
    await user.clear(limit);
    await user.type(limit, '9');
    expect(limit).toHaveFocus();
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        invite_gen_limit: 9,
      }),
    );
  });

  it('rejects an integer prefix instead of truncating and sending it', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-invite'));

    const limit = screen.getByTestId('config-invite_gen_limit');
    await user.clear(limit);
    await user.type(limit, '12.9');
    await user.keyboard('{Enter}');

    const field = limit.closest('[data-slot="field"]');
    expect(field).not.toBeNull();
    expect(await within(field as HTMLElement).findByRole('alert')).toHaveTextContent(
      '请输入完整且有效的整数',
    );
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
  });

  it('rejects a fractional invite commission before PATCH', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-invite'));

    const commission = screen.getByTestId('config-invite_commission');
    await user.clear(commission);
    await user.type(commission, '12.5');
    await user.keyboard('{Enter}');

    const field = commission.closest('[data-slot="field"]');
    expect(field).not.toBeNull();
    expect(await within(field as HTMLElement).findByRole('alert')).toHaveTextContent(
      '请输入完整且有效的整数',
    );
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
  });

  it('maps an emptied nullable text field to the explicit null-clear wire value', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-email'));

    await user.clear(screen.getByTestId('config-email_host'));
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        email_host: null,
      }),
    );
  });

  it('offers an explicit null-clear for non-null projected fields', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);

    await user.click(screen.getByTestId('config-app_name-reset-default'));
    expect(screen.getByText('保存后将恢复系统默认值')).toBeInTheDocument();
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        app_name: null,
      }),
    );
    expect(screen.queryByTestId('config-secure_path-reset-default')).not.toBeInTheDocument();
  });

  it('locks the whole section while a PATCH is in flight', async () => {
    let finishSave: ((value: typeof APPLIED) => void) | undefined;
    mocks.saveMutateAsync.mockImplementationOnce(
      () => new Promise<typeof APPLIED>((resolve) => (finishSave = resolve)),
    );
    const user = userEvent.setup();
    render(<ConfigPage />);

    const name = screen.getByTestId('config-app_name');
    await user.clear(name);
    await user.type(name, 'Locked Site');
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() => expect(name).toBeDisabled());
    expect(screen.getByTestId('config-force_https')).toBeDisabled();
    await user.type(name, ' overwritten');
    expect(name).toHaveValue('Locked Site');
    finishSave?.(APPLIED);
    await waitFor(() => expect(name).toBeEnabled());
  });

  it('refetches but never resubmits a durable 202 change-set', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    const user = userEvent.setup();
    render(<ConfigPage />);

    const name = screen.getByTestId('config-app_name');
    await user.clear(name);
    await user.type(name, 'Pending Site');
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    expect(name).toHaveValue('Pending Site');
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('config-save')).toBeDisabled();
    expect(screen.getByTestId('config-pending-activation')).toHaveTextContent('revision 8');
    await user.click(screen.getByTestId('config-tab-invite'));
    expect(screen.getByTestId('config-invite_gen_limit')).toBeDisabled();
    await user.click(screen.getByTestId('config-tab-site'));
    expect(screen.getByTestId('config-save')).toBeDisabled();
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith('保存成功', {
      description: '配置已保存，正在等待所有进程生效。',
    });
  });

  it('restores pending metadata after remount and unlocks only at the observed revision', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    const user = userEvent.setup();
    const view = render(<ConfigPage />);

    await user.clear(screen.getByTestId('config-app_name'));
    await user.type(screen.getByTestId('config-app_name'), 'Pending Site');
    await user.click(screen.getByTestId('config-save'));
    await screen.findByTestId('config-pending-activation');

    view.unmount();
    const restored = render(<ConfigPage />);
    expect(await screen.findByTestId('config-pending-activation')).toHaveTextContent('revision 8');
    expect(screen.getByTestId('config-app_name')).toBeDisabled();

    mocks.configData = {
      ...makeConfig(),
      revision: 8,
      site: { ...makeConfig().site, app_name: 'Pending Site' },
    };
    restored.unmount();
    render(<ConfigPage />);
    await waitFor(() =>
      expect(screen.queryByTestId('config-pending-activation')).not.toBeInTheDocument(),
    );
    expect(screen.getByTestId('config-app_name')).toHaveValue('Pending Site');
    expect(screen.getByTestId('config-app_name')).toBeEnabled();
    expect(window.sessionStorage.length).toBe(0);
    expect(window.localStorage.getItem(PENDING_FALLBACK_KEY)).toBeNull();
  });

  it('recovers a pending commit across remounts through the local fallback when session storage fails', async () => {
    trackStorageSpy(
      vi.spyOn(window.sessionStorage, 'getItem').mockImplementation(() => {
        throw new Error('session get denied');
      }),
    );
    trackStorageSpy(
      vi.spyOn(window.sessionStorage, 'setItem').mockImplementation(() => {
        throw new Error('session set denied');
      }),
    );
    trackStorageSpy(
      vi.spyOn(window.sessionStorage, 'removeItem').mockImplementation(() => {
        throw new Error('session remove denied');
      }),
    );
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    const user = userEvent.setup();
    const first = render(<ConfigPage />);

    await user.clear(screen.getByTestId('config-app_name'));
    await user.type(screen.getByTestId('config-app_name'), 'Fallback Site');
    await user.click(screen.getByTestId('config-save'));
    await screen.findByTestId('config-pending-activation');

    const fallback = window.localStorage.getItem(PENDING_FALLBACK_KEY);
    expect(fallback).toContain('"version":2');
    expect(fallback).not.toContain('admin-test-token');
    expect(fallback).not.toContain('Fallback Site');
    first.unmount();

    mocks.blockerState = 'blocked';
    const restored = render(<ConfigPage />);
    expect(await screen.findByTestId('config-pending-activation')).toHaveTextContent('revision 8');
    const dialog = await screen.findByTestId('config-leave-dialog');
    expect(dialog).toHaveTextContent('已有一笔配置提交正在等待 revision 8 生效');
    expect(within(dialog).queryByTestId('config-leave')).not.toBeInTheDocument();

    mocks.configData = {
      ...makeConfig(),
      revision: 8,
      site: { ...makeConfig().site, app_name: 'Fallback Site' },
    };
    restored.unmount();
    render(<ConfigPage />);
    await waitFor(() =>
      expect(screen.queryByTestId('config-pending-activation')).not.toBeInTheDocument(),
    );
    expect(window.localStorage.getItem(PENDING_FALLBACK_KEY)).toBeNull();
  });

  it('keeps the current mount fail-closed when both browser stores become unavailable', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    const user = userEvent.setup();
    render(<ConfigPage />);

    trackStorageSpy(
      vi.spyOn(window.sessionStorage, 'getItem').mockImplementation(() => {
        throw new Error('session get denied');
      }),
    );
    trackStorageSpy(
      vi.spyOn(window.sessionStorage, 'setItem').mockImplementation(() => {
        throw new Error('session set denied');
      }),
    );
    trackStorageSpy(
      vi.spyOn(window.sessionStorage, 'removeItem').mockImplementation(() => {
        throw new Error('session remove denied');
      }),
    );
    trackStorageSpy(
      vi.spyOn(window.localStorage, 'getItem').mockImplementation(() => {
        throw new Error('local get denied');
      }),
    );
    trackStorageSpy(
      vi.spyOn(window.localStorage, 'setItem').mockImplementation(() => {
        throw new Error('local set denied');
      }),
    );
    trackStorageSpy(
      vi.spyOn(window.localStorage, 'removeItem').mockImplementation(() => {
        throw new Error('local remove denied');
      }),
    );

    await user.clear(screen.getByTestId('config-app_name'));
    await user.type(screen.getByTestId('config-app_name'), 'Memory Site');
    await user.click(screen.getByTestId('config-save'));

    expect(await screen.findByTestId('config-pending-activation')).toHaveTextContent('revision 8');
    expect(mocks.useBlocker).toHaveBeenCalledWith(true);
  });

  it('does not let an old activation observer clear a newer pending commit', () => {
    const oldCommit = { group: 'site' as const, revision: 8 };
    const newCommit = { group: 'invite' as const, revision: 9 };
    writePendingConfigCommit(oldCommit);
    writePendingConfigCommit(newCommit);

    expect(clearPendingConfigCommit(oldCommit)).toBe(false);
    expect(readPendingConfigCommit()).toEqual(newCommit);
  });

  it('does not resurrect a consumed commit when session remove alone fails', () => {
    const commit = { group: 'site' as const, revision: 8 };
    writePendingConfigCommit(commit);
    trackStorageSpy(
      vi.spyOn(window.sessionStorage, 'removeItem').mockImplementation(() => {
        throw new Error('session remove denied');
      }),
    );

    expect(clearPendingConfigCommit(commit)).toBe(true);
    // The physical stale record demonstrates that this is the in-memory
    // consumed/tombstone path, not a successful remove disguised as success.
    expect(window.sessionStorage.getItem(PENDING_SESSION_KEY)).not.toBeNull();
    expect(window.localStorage.getItem(PENDING_FALLBACK_KEY)).toBeNull();
    expect(readPendingConfigCommit()).toBeNull();
  });

  it('reconciles a newer cross-tab fallback over an older session snapshot', () => {
    writePendingConfigCommit({ group: 'site', revision: 8 });
    const olderSession = window.sessionStorage.getItem(PENDING_SESSION_KEY);
    writePendingConfigCommit({ group: 'invite', revision: 9 });
    const newerFallback = window.localStorage.getItem(PENDING_FALLBACK_KEY);
    expect(olderSession).not.toBeNull();
    expect(newerFallback).not.toBeNull();
    window.sessionStorage.setItem(PENDING_SESSION_KEY, olderSession!);

    expect(readPendingConfigCommit()).toEqual({ group: 'invite', revision: 9 });
    expect(window.sessionStorage.getItem(PENDING_SESSION_KEY)).toBe(newerFallback);
  });

  it('keeps equal-revision storage conflicts locked and does not overwrite either source', () => {
    writePendingConfigCommit({ group: 'site', revision: 8 });
    const sessionCommit = window.sessionStorage.getItem(PENDING_SESSION_KEY);
    writePendingConfigCommit({ group: 'invite', revision: 8 });
    const fallbackCommit = window.localStorage.getItem(PENDING_FALLBACK_KEY);
    expect(sessionCommit).not.toBeNull();
    expect(fallbackCommit).not.toBeNull();
    window.sessionStorage.setItem(PENDING_SESSION_KEY, sessionCommit!);

    const first = readPendingConfigCommit();
    const second = readPendingConfigCommit();
    expect(first).not.toBeNull();
    expect(second).toEqual(first);
    expect(window.sessionStorage.getItem(PENDING_SESSION_KEY)).toBe(sessionCommit);
    expect(window.localStorage.getItem(PENDING_FALLBACK_KEY)).toBe(fallbackCommit);
  });

  it('notifies subscribers when another tab changes the local fallback', () => {
    const listener = vi.fn();
    const unsubscribe = subscribePendingConfigCommit(listener);

    window.dispatchEvent(
      new StorageEvent('storage', {
        key: PENDING_FALLBACK_KEY,
        storageArea: window.localStorage,
      }),
    );
    window.dispatchEvent(
      new StorageEvent('storage', {
        key: 'unrelated',
        storageArea: window.localStorage,
      }),
    );

    expect(listener).toHaveBeenCalledOnce();
    unsubscribe();
  });

  it('rejects and clears persisted pending metadata after the auth identity changes', () => {
    writePendingConfigCommit({ group: 'site', revision: 8 });
    expect(window.localStorage.getItem(PENDING_FALLBACK_KEY)).not.toContain('admin-test-token');

    window.localStorage.setItem('v2board.admin_auth_data', 'different-admin-token');

    expect(readPendingConfigCommit()).toBeNull();
    expect(window.sessionStorage.getItem(PENDING_SESSION_KEY)).toBeNull();
    expect(window.localStorage.getItem(PENDING_FALLBACK_KEY)).toBeNull();
  });

  it('keeps the losing draft and refetches once on revision conflict', async () => {
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

    const name = screen.getByTestId('config-app_name');
    await user.clear(name);
    await user.type(name, 'Losing draft');
    await user.click(screen.getByTestId('config-save'));

    expect(await screen.findByTestId('config-save-error')).toHaveTextContent(
      '配置已被其他请求更新，请刷新后重试',
    );
    expect(name).toHaveValue('Losing draft');
    expect(mocks.refetch).toHaveBeenCalledTimes(1);
    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
      expected_revision: 7,
      app_name: 'Losing draft',
    });
  });

  it('redirects only after an applied secure_path change', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-safe'));

    const securePath = screen.getByTestId('config-secure_path');
    await user.clear(securePath);
    await user.type(securePath, 'next-admin');
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() => expect(window.location.pathname).toBe('/next-admin/config/system'));
    expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
      expected_revision: 7,
      secure_path: 'next-admin',
    });
    expect(mocks.refetch).not.toHaveBeenCalled();
    const systemRedirect = new Event('beforeunload', { cancelable: true }) as BeforeUnloadEvent;
    mocks.beforeUnload?.(systemRedirect);
    expect(systemRedirect.defaultPrevented).toBe(false);
  });

  it('keeps the current path when a secure_path change is pending activation', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-safe'));

    const securePath = screen.getByTestId('config-secure_path');
    await user.clear(securePath);
    await user.type(securePath, 'next-admin');
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() => expect(mocks.probeConfigAtAdminPath).toHaveBeenCalledTimes(1));
    expect(mocks.refetch).not.toHaveBeenCalled();
    expect(window.location.pathname).toBe('/admin-path/config/system');
  });

  it('redirects a pending secure_path only after the new prefix serves its revision', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    mocks.probeConfigAtAdminPath.mockResolvedValueOnce({
      ...makeConfig(),
      revision: 8,
      safe: { ...makeConfig().safe, secure_path: 'next-admin' },
    });
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-safe'));

    const securePath = screen.getByTestId('config-secure_path');
    await user.clear(securePath);
    await user.type(securePath, 'next-admin');
    await user.click(screen.getByTestId('config-save'));

    await waitFor(() => expect(window.location.pathname).toBe('/next-admin/config/system'));
    expect(mocks.probeConfigAtAdminPath).toHaveBeenCalledWith(
      expect.anything(),
      'next-admin',
      'safe',
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it('blocks SPA navigation while the secure-path activation probe owns the redirect', async () => {
    writePendingConfigCommit({ group: 'safe', revision: 8, securePath: 'next-admin' });
    mocks.blockerState = 'blocked';
    render(<ConfigPage />);

    const dialog = await screen.findByTestId('config-leave-dialog');
    expect(dialog).toHaveTextContent('后台路径正在切换');
    expect(dialog).toHaveTextContent('确认 revision 后会自动跳转');
    expect(within(dialog).queryByTestId('config-leave')).not.toBeInTheDocument();
    expect(mocks.probeConfigAtAdminPath).toHaveBeenCalledTimes(1);
  });

  it('never treats the old-prefix revision as proof that a pending secure path is active', async () => {
    writePendingConfigCommit({ group: 'safe', revision: 8, securePath: 'next-admin' });
    mocks.configData = { ...makeConfig(), revision: 99 };
    render(<ConfigPage />);

    await waitFor(() => expect(mocks.probeConfigAtAdminPath).toHaveBeenCalledTimes(1));
    expect(window.location.pathname).toBe('/admin-path/config/system');
    expect(window.sessionStorage.getItem(PENDING_SESSION_KEY)).not.toBeNull();
  });

  it('keeps the secure-path navigation lock mounted after the old prefix starts returning errors', async () => {
    writePendingConfigCommit({ group: 'safe', revision: 8, securePath: 'next-admin' });
    mocks.configError = new Error('old prefix is inactive');
    mocks.blockerState = 'blocked';
    render(<ConfigPage />);

    expect(await screen.findByText(/等待 revision 8 生效/)).toBeInTheDocument();
    const dialog = await screen.findByTestId('config-leave-dialog');
    expect(dialog).toHaveTextContent('后台路径正在切换');
    expect(within(dialog).queryByTestId('config-leave')).not.toBeInTheDocument();
    expect(mocks.probeConfigAtAdminPath).toHaveBeenCalledTimes(1);
  });

  it('rejects an empty secure_path without sending or moving the admin route', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-safe'));

    const securePath = screen.getByTestId('config-secure_path');
    await user.clear(securePath);
    await user.click(screen.getByTestId('config-save'));

    const field = securePath.closest('[data-slot="field"]');
    expect(field).not.toBeNull();
    expect(await within(field as HTMLElement).findByRole('alert')).toHaveTextContent(
      '后台路径不能为空',
    );
    expect(mocks.saveMutateAsync).not.toHaveBeenCalled();
    expect(window.location.pathname).toBe('/admin-path/config/system');
  });

  it('persists the email draft before testing with the new settings', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-email'));

    const host = screen.getByTestId('config-email_host');
    await user.clear(host);
    await user.type(host, 'smtp.changed.example');
    await user.click(screen.getByTestId('config-test-mail'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        email_host: 'smtp.changed.example',
      }),
    );
    await waitFor(() => expect(mocks.testMailMutateAsync).toHaveBeenCalledTimes(1));
    expect(mocks.saveMutateAsync.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.testMailMutateAsync.mock.invocationCallOrder[0]!,
    );
  });

  it('does not test mail against the old active snapshot after a pending save', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-email'));

    const host = screen.getByTestId('config-email_host');
    await user.clear(host);
    await user.type(host, 'smtp.pending.example');
    await user.click(screen.getByTestId('config-test-mail'));

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    expect(mocks.testMailMutateAsync).not.toHaveBeenCalled();
  });

  it('persists a staged Telegram token before setting the webhook', async () => {
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-telegram'));

    const token = screen.getByTestId('config-telegram_bot_token');
    await user.clear(token);
    await user.type(token, '1111111111:new-token');
    await user.click(screen.getByTestId('config-set-webhook'));

    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({
        expected_revision: 7,
        telegram_bot_token: '1111111111:new-token',
      }),
    );
    await waitFor(() =>
      expect(mocks.webhookMutateAsync).toHaveBeenCalledWith('1111111111:new-token'),
    );
  });

  it('does not set a webhook against the old active snapshot after a pending save', async () => {
    mocks.saveMutateAsync.mockResolvedValueOnce({ activation: 'pending', revision: 8 });
    const user = userEvent.setup();
    render(<ConfigPage />);
    await user.click(screen.getByTestId('config-tab-telegram'));

    const token = screen.getByTestId('config-telegram_bot_token');
    await user.clear(token);
    await user.type(token, '1111111111:pending-token');
    await user.click(screen.getByTestId('config-set-webhook'));

    await waitFor(() => expect(mocks.refetch).toHaveBeenCalledTimes(1));
    expect(token).toHaveValue('********');
    expect(window.sessionStorage.getItem(PENDING_SESSION_KEY)).not.toContain(
      '1111111111:pending-token',
    );
    expect(window.localStorage.getItem(PENDING_FALLBACK_KEY)).not.toContain(
      '1111111111:pending-token',
    );
    expect(screen.getByTestId('config-set-webhook')).toBeDisabled();
    await user.click(screen.getByTestId('config-set-webhook'));
    expect(mocks.webhookMutateAsync).not.toHaveBeenCalled();
    expect(mocks.saveMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('gates a section when its dependency fails and exposes retry', async () => {
    mocks.plansData = undefined;
    mocks.plansError = new Error('plans failed');
    const user = userEvent.setup();
    render(<ConfigPage />);

    const error = screen.getByTestId('config-plans-error');
    expect(screen.queryByTestId('config-app_name')).not.toBeInTheDocument();
    await user.click(within(error).getByTestId('error-state-retry'));
    expect(mocks.plansRefetch).toHaveBeenCalledTimes(1);
  });
});

describe('config value helpers', () => {
  it('coerces backend values and preserves history-routing paths', () => {
    expect(parseBackendInteger('12')).toBe(12);
    expect(() => parseBackendInteger('12.9')).toThrow('admin.config.integer_invalid');
    expect(() => parseBackendInteger('12oops')).toThrow('admin.config.integer_invalid');
    expect(parseBackendInteger('')).toBeNull();
    expect(parseBackendNumber('12.5')).toBe(12.5);
    expect(() => parseBackendNumber('12oops')).toThrow('admin.config.number_invalid');
    expect(isBackendEnabled(true)).toBe(true);
    expect(isBackendEnabled('0')).toBe(false);
    expect(adminSecurePathLocation('/next-admin/', '/config/system?tab=safe')).toBe(
      '/next-admin/config/system?tab=safe',
    );
  });
});
