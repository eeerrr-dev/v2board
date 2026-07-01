import { readFileSync } from 'node:fs';
import { act } from 'react';
import type { ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '@v2board/api-client';
import ProfilePage from './profile';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const source = readFileSync(`${process.cwd()}/src/pages/profile.tsx`, 'utf8');
const componentSource = readFileSync(`${process.cwd()}/src/pages/profile-components.tsx`, 'utf8');

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  refetchInfo: vi.fn(),
  refetchSubscribe: vi.fn(),
  redeem: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  updateProfile: vi.fn(),
  changePassword: vi.fn(),
  resetSub: vi.fn(),
  unbindTelegram: vi.fn(),
  saveOrder: vi.fn(),
  copyText: vi.fn(),
  refetchSessions: vi.fn(),
  removeSession: vi.fn(),
  confirmDialog: vi.fn(),
  getAuthData: vi.fn(),
  sessions: {
    data: undefined as Record<
      string,
      { ip: string; login_at: number; ua: string; auth_data: string }
    > | undefined,
    isLoading: false,
    isError: false,
  },
  userInfo: {
    balance: 0,
    auto_renewal: 0,
    remind_expire: 0,
    remind_traffic: 0,
    telegram_id: null as number | null,
    email: 'user@example.test',
    uuid: 'uuid-abc-123',
    created_at: 1_700_000_000,
    last_login_at: 1_700_003_600 as number | null,
  },
  comm: {
    currency: 'USD',
    is_telegram: false,
    telegram_discuss_link: '',
  },
  subscribe: {
    subscribe_url: 'https://example.test/sub',
  } as { subscribe_url?: string } | undefined,
  botInfo: undefined as { username: string } | undefined,
}));

const labels: Record<string, string> = {
  'common.cancel': '取消',
  'common.copy': '复制',
  'common.attention': '注意',
  'common.loading': '加载中',
  'common.error_title': '加载失败',
  'common.retry': '重试',
  'dashboard.copy_success': '复制成功',
  'profile.active_sessions': '登录设备',
  'profile.active_sessions_desc': '这些设备当前已登录你的账户，如有陌生设备可将其注销。',
  'profile.session_device': '设备',
  'profile.session_ip': 'IP 地址',
  'profile.session_login_at': '登录时间',
  'profile.session_current': '当前设备',
  'profile.session_revoke': '注销',
  'profile.session_revoke_confirm': '确定要注销该设备的登录？',
  'profile.session_revoke_success': '已注销该设备',
  'profile.no_sessions': '暂无登录设备',
  'profile.account': '账户信息',
  'profile.email': '邮箱',
  'profile.uuid': 'UUID',
  'profile.last_login': '上次登录',
  'profile.created_at': '注册时间',
  'profile.wallet': '我的钱包(仅消费)',
  'profile.auto_renewal': '自动续费',
  'profile.recharge': '充值',
  'profile.redeem_giftcard': '礼品卡',
  'profile.redeem_placeholder': '请输入礼品卡',
  'profile.redeem_submit': '兑换',
  'profile.redeem_success': '兑换成功: {{detail}}',
  'profile.redeem_balance': '账户余额 {{amount}}',
  'profile.redeem_days': '订阅时长 {{days}} 天',
  'profile.redeem_traffic': '套餐流量 {{traffic}} GB',
  'profile.redeem_reset': '流量已重置',
  'profile.redeem_plan_days': '订阅套餐 {{days}} 天',
  'profile.redeem_unknown': '未知类型',
  'profile.change_password': '修改密码',
  'profile.old_password': '旧密码',
  'profile.new_password': '新密码',
  'profile.old_password_placeholder': '请输入旧密码',
  'profile.new_password_placeholder': '请输入新密码',
  'profile.save': '保存',
  'profile.notifications': '通知',
  'profile.remind_expire': '到期邮件提醒',
  'profile.remind_traffic': '流量邮件提醒',
  'profile.reset_subscribe': '重置订阅信息',
  'profile.reset_subscribe_warning':
    '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
  'profile.reset_subscribe_confirm': '确定要重置订阅信息？',
  'profile.reset_subscribe_tip':
    '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
  'profile.reset_success': '重置成功',
  'profile.reset': '重置',
  'profile.confirm': '确认',
  'profile.telegram_bind': '绑定Telegram',
  'profile.telegram_unbind': '解除绑定',
  'profile.telegram_unbind_confirm': '确定要解除绑定Telegram？',
  'profile.telegram_unbind_tip': '如果你的Telegram ID已失效可以进行此操作。重置后你需要重新进行绑定。',
  'profile.telegram_discuss': 'Telegram 讨论组',
  'profile.start_now': '立即开始',
  'profile.join_now': '立即加入',
  'profile.i_know': '我知道了',
  'profile.telegram_step1': '第一步',
  'profile.telegram_step2': '第二步',
  'profile.telegram_search': '打开Telegram搜索',
  'profile.telegram_send': '向机器人发送您的',
  'profile.password_mismatch': '两次新密码输入不同',
  'profile.change_password_success': '修改成功，请重新登陆',
  'profile.deposit_placeholder': '请输入充值金额{{currency}}',
};

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => {
      let label = labels[key] ?? key;
      Object.entries(values ?? {}).forEach(([name, value]) => {
        label = label.replaceAll(`{{${name}}}`, String(value));
      });
      return label;
    },
  }),
}));

vi.mock('@/components/ui/shadcn-dialog', () => ({
  Dialog: ({ children, open }: { children: ReactNode; open?: boolean }) =>
    open ? <>{children}</> : null,
  DialogContent: ({
    children,
    className,
  }: {
    children: ReactNode;
    className?: string;
  }) => <div className={className}>{children}</div>,
  DialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock('@/components/ui/alert-dialog', () => ({
  AlertDialog: ({ children, open }: { children: ReactNode; open?: boolean }) =>
    open ? <>{children}</> : null,
  AlertDialogContent: ({
    children,
    className,
    ...props
  }: {
    children: ReactNode;
    className?: string;
    [key: string]: unknown;
  }) => (
    <div className={className} {...props}>
      {children}
    </div>
  ),
  AlertDialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  AlertDialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  AlertDialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  AlertDialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock('@/lib/queries', () => ({
  useUserInfo: () => ({
    data: mocks.userInfo,
    refetch: mocks.refetchInfo,
  }),
  useCommConfig: () => ({
    data: mocks.comm,
  }),
  useSubscribe: () => ({
    data: mocks.subscribe,
    refetch: mocks.refetchSubscribe,
  }),
  useUpdateProfileMutation: () => ({
    mutateAsync: mocks.updateProfile,
  }),
  useChangePasswordMutation: () => ({
    isPending: false,
    mutateAsync: mocks.changePassword,
  }),
  useRedeemGiftCardMutation: () => ({
    isPending: false,
    mutateAsync: mocks.redeem,
  }),
  useResetSubscribeMutation: () => ({
    mutateAsync: mocks.resetSub,
  }),
  useUnbindTelegramMutation: () => ({
    mutateAsync: mocks.unbindTelegram,
  }),
  useSaveOrderMutation: () => ({
    mutateAsync: mocks.saveOrder,
  }),
  useTelegramBotInfo: () => ({
    data: mocks.botInfo,
  }),
  useActiveSessions: () => ({
    data: mocks.sessions.data,
    isLoading: mocks.sessions.isLoading,
    isError: mocks.sessions.isError,
    refetch: mocks.refetchSessions,
  }),
  useRemoveSessionMutation: () => ({
    mutateAsync: mocks.removeSession,
  }),
}));

vi.mock('@/lib/legacy-settings', () => ({
  copyText: mocks.copyText,
}));

vi.mock('@/lib/auth', () => ({
  getAuthData: mocks.getAuthData,
}));

vi.mock('@/components/ui/confirm-dialog', () => ({
  confirmDialog: mocks.confirmDialog,
}));

vi.mock('@/lib/toast', () => ({
  toast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

describe('ProfilePage shadcn account surface', () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    mocks.navigate.mockClear();
    mocks.refetchInfo.mockReset();
    mocks.refetchSubscribe.mockReset();
    mocks.redeem.mockReset();
    mocks.toastSuccess.mockClear();
    mocks.toastError.mockClear();
    mocks.updateProfile.mockReset();
    mocks.changePassword.mockReset();
    mocks.resetSub.mockReset();
    mocks.unbindTelegram.mockReset();
    mocks.saveOrder.mockReset();
    mocks.copyText.mockClear();
    mocks.copyText.mockResolvedValue(true);
    mocks.refetchSessions.mockReset();
    mocks.removeSession.mockReset();
    mocks.removeSession.mockResolvedValue(true);
    mocks.getAuthData.mockReset();
    mocks.getAuthData.mockReturnValue('token-current');
    mocks.confirmDialog.mockReset();
    // The imperative confirm dialog is exercised elsewhere; here we auto-confirm
    // so the revoke flow's onConfirm runs synchronously.
    mocks.confirmDialog.mockImplementation(
      (options: { onConfirm?: () => unknown }) => {
        void options.onConfirm?.();
        return Promise.resolve(true);
      },
    );
    mocks.sessions = {
      data: {
        'guid-other': {
          ip: '203.0.113.9',
          login_at: 1_700_003_600,
          ua: 'Firefox on Windows',
          auth_data: 'token-other',
        },
        'guid-current': {
          ip: '198.51.100.4',
          login_at: 1_700_000_000,
          ua: 'Chrome on macOS',
          auth_data: 'token-current',
        },
      },
      isLoading: false,
      isError: false,
    };
    mocks.userInfo = {
      balance: 0,
      auto_renewal: 0,
      remind_expire: 0,
      remind_traffic: 0,
      telegram_id: null,
      email: 'user@example.test',
      uuid: 'uuid-abc-123',
      created_at: 1_700_000_000,
      last_login_at: 1_700_003_600,
    };
    mocks.comm = {
      currency: 'USD',
      is_telegram: false,
      telegram_discuss_link: '',
    };
    mocks.subscribe = { subscribe_url: 'https://example.test/sub' };
    mocks.botInfo = undefined;
  });

  afterEach(() => {
    if (root) act(() => root?.unmount());
    root = null;
    container.remove();
    document.body.innerHTML = '';
  });

  it('shows the redeem success toast only after the mutation resolves, leaving the refresh to the mutation', async () => {
    let resolveRedeem!: () => void;
    mocks.redeem.mockImplementation(
      () =>
        new Promise<{ type: number; value: number }>((resolve) => {
          resolveRedeem = () => resolve({ type: 1, value: 1234 });
        }),
    );

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const giftCardInput = container.querySelector<HTMLInputElement>(
      'input[placeholder="请输入礼品卡"]',
    );
    expect(giftCardInput).toBeTruthy();

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        giftCardInput,
        'CARD-123',
      );
      giftCardInput!.dispatchEvent(new Event('input', { bubbles: true }));
      container.querySelector<HTMLButtonElement>('[data-testid="profile-redeem-button"]')!.click();
      await Promise.resolve();
    });

    expect(mocks.redeem).toHaveBeenCalledWith('CARD-123');
    // The success toast must wait for the mutation to resolve, not fire on submit.
    expect(mocks.toastSuccess).not.toHaveBeenCalled();

    await act(async () => {
      resolveRedeem();
      await Promise.resolve();
      await Promise.resolve();
    });

    // The user-record refresh is now the mutation's onSuccess job (see
    // queries.test.ts), so the component no longer refetches here directly.
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
    expect(mocks.toastSuccess).toHaveBeenCalledWith('兑换成功: 账户余额 12.34');
  });

  it('keeps the stuck loading state when gift card redeem times out', async () => {
    // A timeout / network drop reaches the page as an ApiError with status 0
    // (the api-client's transport-failure signal), not a plain Error.
    mocks.redeem.mockRejectedValue(new ApiError(0, 'timeout exceeded'));

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const giftCardInput = container.querySelector<HTMLInputElement>(
      'input[placeholder="请输入礼品卡"]',
    );
    expect(giftCardInput).toBeTruthy();

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        giftCardInput,
        'CARD-FAIL',
      );
      giftCardInput!.dispatchEvent(new Event('input', { bubbles: true }));
      container.querySelector<HTMLButtonElement>('[data-testid="profile-redeem-button"]')!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    const redeemButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-redeem-button"]',
    );
    expect(mocks.redeem).toHaveBeenCalledWith('CARD-FAIL');
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
    expect(redeemButton?.getAttribute('aria-busy')).toBe('true');
    expect(redeemButton?.disabled).toBe(true);
  });

  it('renders the shadcn profile cards and telegram binding dialog content', async () => {
    mocks.comm = {
      currency: 'USD',
      is_telegram: true,
      telegram_discuss_link: 'https://t.me/discuss',
    };
    mocks.botInfo = { username: 'legacy_bot' };

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    expect(container.querySelector('[data-testid="profile-page"]')).toBeTruthy();
    expect(container.querySelectorAll('[data-testid="profile-card-title"]').length).toBeGreaterThan(3);
    expect(container.textContent).toContain(
      '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
    );
    expect(container.textContent).toContain('绑定Telegram');
    expect(container.textContent).toContain('Telegram 讨论组');

    const startButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-telegram-start"]',
    );
    expect(startButton).toBeTruthy();

    await act(async () => {
      startButton!.click();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('第一步');
    expect(container.textContent).toContain('打开Telegram搜索');
    expect(container.querySelector('a[href="https://t.me/legacy_bot"]')?.textContent).toBe(
      '@legacy_bot',
    );
    expect(container.textContent).toContain('向机器人发送您的');
    expect(container.querySelector('[data-testid="profile-copy-code"]')?.textContent).toBe('/bind https://example.test/sub');

    await act(async () => {
      container.querySelector('[data-testid="profile-copy-code"]')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.copyText).toHaveBeenCalledWith('/bind https://example.test/sub');
  });

  it('surfaces the account identity fields and copies the uuid', async () => {
    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    // The backend already returns these on info(); the card must surface them
    // instead of leaving the email/uuid/last_login/created_at keys dead.
    expect(container.querySelector('[data-testid="profile-account-card"]')).toBeTruthy();
    expect(container.querySelector('[data-testid="profile-account-email"]')?.textContent).toBe(
      'user@example.test',
    );
    expect(container.querySelector('[data-testid="profile-account-uuid"]')?.textContent).toBe(
      'uuid-abc-123',
    );
    // Registration + last login render through the shared legacy datetime formatter.
    expect(container.querySelector('[data-testid="profile-account-created"]')?.textContent).not.toBe(
      '—',
    );
    expect(
      container.querySelector('[data-testid="profile-account-last-login"]')?.textContent,
    ).not.toBe('—');

    const copyButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-account-uuid-copy"]',
    );
    expect(copyButton).toBeTruthy();
    await act(async () => {
      copyButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    expect(mocks.copyText).toHaveBeenCalledWith('uuid-abc-123');
    expect(mocks.toastSuccess).toHaveBeenCalled();
  });

  it('falls back to an em dash when a last login timestamp is absent', async () => {
    mocks.userInfo = { ...mocks.userInfo, last_login_at: null };

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    expect(
      container.querySelector('[data-testid="profile-account-last-login"]')?.textContent,
    ).toBe('—');
  });

  it('lists active sessions, badges the current device, and blocks self-revocation', async () => {
    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    expect(container.querySelector('[data-testid="profile-sessions-card"]')).toBeTruthy();
    const rows = container.querySelectorAll('[data-testid="profile-session-row"]');
    expect(rows).toHaveLength(2);
    // Both devices surface their UA + IP so an unfamiliar device is recognizable.
    expect(container.textContent).toContain('Firefox on Windows');
    expect(container.textContent).toContain('203.0.113.9');
    expect(container.textContent).toContain('Chrome on macOS');

    // Rows sort newest-first, so the current (older) device is the second row.
    const badges = container.querySelectorAll('[data-testid="profile-session-current"]');
    expect(badges).toHaveLength(1);
    expect(rows[1]!.contains(badges[0]!)).toBe(true);

    const revokeButtons = container.querySelectorAll<HTMLButtonElement>(
      '[data-testid="profile-session-revoke"]',
    );
    expect(revokeButtons).toHaveLength(2);
    // The other device can be signed out; the current device cannot revoke itself.
    expect(revokeButtons[0]!.disabled).toBe(false);
    expect(revokeButtons[1]!.disabled).toBe(true);
  });

  it('revokes another device through the confirm dialog and toasts on success', async () => {
    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const revokeButtons = container.querySelectorAll<HTMLButtonElement>(
      '[data-testid="profile-session-revoke"]',
    );

    await act(async () => {
      revokeButtons[0]!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.confirmDialog).toHaveBeenCalledTimes(1);
    // The revoke posts the guid of the other device, not the current one.
    expect(mocks.removeSession).toHaveBeenCalledWith('guid-other');
    expect(mocks.toastSuccess).toHaveBeenCalledWith('已注销该设备');
  });

  it('shows a retryable error state when the session fetch fails', async () => {
    mocks.sessions = { data: undefined, isLoading: false, isError: true };

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    expect(container.querySelector('[data-testid="profile-sessions-error"]')).toBeTruthy();
    expect(container.querySelector('[data-testid="profile-session-row"]')).toBeNull();

    const retry = container.querySelector<HTMLButtonElement>('[data-testid="error-state-retry"]');
    expect(retry).toBeTruthy();
    await act(async () => {
      retry!.click();
      await Promise.resolve();
    });
    expect(mocks.refetchSessions).toHaveBeenCalledTimes(1);
  });

  it('shows a loading state while the session list is fetching', async () => {
    mocks.sessions = { data: undefined, isLoading: true, isError: false };

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    expect(container.querySelector('[data-testid="profile-sessions-card"]')).toBeTruthy();
    expect(container.querySelector('[data-testid="profile-session-row"]')).toBeNull();
    expect(container.querySelector('[data-testid="profile-sessions-error"]')).toBeNull();
    expect(container.textContent).toContain('加载中');
  });

  it('uses the legacy bare Telegram bind command when no subscribe url is cached', async () => {
    mocks.comm = {
      currency: 'USD',
      is_telegram: true,
      telegram_discuss_link: '',
    };
    mocks.botInfo = { username: 'legacy_bot' };
    mocks.subscribe = undefined;

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const startButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-telegram-start"]',
    );
    expect(startButton).toBeTruthy();

    await act(async () => {
      startButton!.click();
      await Promise.resolve();
    });

    expect(container.querySelector('[data-testid="profile-copy-code"]')?.textContent).toBe('/bind');
    expect(container.textContent).not.toContain('undefined');
  });

  it('keeps direct source values for profile switches and omits empty Telegram bind urls', () => {
    expect(source).toContain('checked={data?.auto_renewal}');
    expect(source).toContain('checked={data?.remind_expire}');
    expect(source).toContain('checked={data?.remind_traffic}');
    expect(componentSource).toContain(
      "const bindCommand = subscribeUrl ? `/bind ${subscribeUrl}` : '/bind';",
    );
    expect(source).not.toContain('checked={Boolean(data?.auto_renewal)}');
    expect(source).not.toContain('checked={Boolean(data?.remind_expire)}');
    expect(source).not.toContain('checked={Boolean(data?.remind_traffic)}');
    expect(componentSource).not.toContain('/bind undefined');
  });

  it('updates profile switches with the original 0/1 payload and leaves the refresh to the mutation', async () => {
    mocks.updateProfile.mockResolvedValue(true);

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const switches = container.querySelectorAll<HTMLButtonElement>(
      '[data-testid="profile-switch"]',
    );
    expect(switches).toHaveLength(3);
    expect(switches[0]!.getAttribute('aria-checked')).toBe('false');
    expect(switches[0]!.getAttribute('aria-label')).toBe('自动续费');
    expect(switches[1]!.getAttribute('aria-label')).toBe('到期邮件提醒');
    expect(switches[2]!.getAttribute('aria-label')).toBe('流量邮件提醒');

    await act(async () => {
      switches[0]!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.updateProfile).toHaveBeenCalledWith({ auto_renewal: 1 });
    expect(mocks.refetchInfo).not.toHaveBeenCalled();

    mocks.updateProfile.mockRejectedValue(new Error('failed'));

    await act(async () => {
      switches[1]!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.updateProfile).toHaveBeenCalledWith({ remind_expire: 1 });
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
  });

  it('renders profile switches with Radix boolean checked state', async () => {
    mocks.updateProfile.mockResolvedValue(true);
    mocks.userInfo = {
      ...mocks.userInfo,
      auto_renewal: 1,
      remind_expire: 0,
      remind_traffic: 1,
      telegram_id: null,
    };

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const switches = container.querySelectorAll<HTMLButtonElement>(
      '[data-testid="profile-switch"]',
    );
    expect(switches).toHaveLength(3);
    expect(switches[0]!.getAttribute('aria-checked')).toBe('true');
    expect(switches[0]!.dataset.state).toBe('checked');
    expect(switches[1]!.getAttribute('aria-checked')).toBe('false');
    expect(switches[1]!.dataset.state).toBe('unchecked');
    expect(switches[2]!.getAttribute('aria-checked')).toBe('true');
    expect(switches[2]!.dataset.state).toBe('checked');

    await act(async () => {
      switches[0]!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.updateProfile).toHaveBeenCalledWith({ auto_renewal: 0 });
  });

  it('keeps profile form values in react-hook-form schemas instead of imperative refs', () => {
    expect(source).toContain("from 'react-hook-form'");
    expect(source).toContain("from 'zod'");
    expect(source).toContain('zodResolver(passwordSchema)');
    expect(source).toContain('zodResolver(giftCardSchema)');
    expect(source).toContain("passwordForm.register('oldPassword')");
    expect(source).toContain("giftCardForm.register('code')");
    expect(source).not.toContain('setPasswordForm');
    expect(source).not.toContain('setGiftCard');
    expect(source).not.toContain('oldPasswordRef.current!.value');
    expect(source).not.toContain('newPasswordRef.current!.value');
    expect(source).not.toContain('confirmPasswordRef.current!.value');
    expect(source).not.toContain('giftCardRef.current!.value');
  });

  it('keeps the old password-change success flow without clearing local auth', async () => {
    mocks.changePassword.mockResolvedValue(true);

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const passwordInputs = container.querySelectorAll<HTMLInputElement>('input[type="password"]');
    expect(passwordInputs).toHaveLength(3);

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        passwordInputs[0],
        'old-password',
      );
      passwordInputs[0]!.dispatchEvent(new Event('input', { bubbles: true }));
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        passwordInputs[1],
        'new-password',
      );
      passwordInputs[1]!.dispatchEvent(new Event('input', { bubbles: true }));
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        passwordInputs[2],
        'new-password',
      );
      passwordInputs[2]!.dispatchEvent(new Event('input', { bubbles: true }));
      container.querySelector<HTMLButtonElement>('[data-testid="profile-password-save"]')!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.changePassword).toHaveBeenCalledWith({
      oldPassword: 'old-password',
      newPassword: 'new-password',
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith('修改成功，请重新登陆');
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
    expect(mocks.toastSuccess.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.navigate.mock.invocationCallOrder[0]!,
    );
    expect(source).not.toContain("import { logout } from '@/lib/auth';");
    expect(source).not.toContain('logout();');
  });

  it('submits the deposit order payload from the shadcn recharge dialog', async () => {
    mocks.comm = {
      currency: 'CNY',
      is_telegram: false,
      telegram_discuss_link: '',
    };
    mocks.saveOrder.mockResolvedValue('DEPOSIT123');

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const rechargeButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-recharge"]',
    );
    expect(rechargeButton).toBeTruthy();

    await act(async () => {
      rechargeButton!.click();
      await Promise.resolve();
    });

    const amountInput = container.querySelector<HTMLInputElement>(
      'input[placeholder="请输入充值金额CNY"]',
    );
    expect(amountInput).toBeTruthy();

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        amountInput,
        '12.34',
      );
      amountInput!.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    const confirmButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-deposit-confirm"]',
    );
    expect(confirmButton).toBeTruthy();

    await act(async () => {
      confirmButton!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveOrder).toHaveBeenCalledWith({
      plan_id: 0,
      period: 'deposit',
      deposit_amount: 1234,
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order/DEPOSIT123');
  });

  it('uses shadcn confirmation dialogs for reset and telegram unbind behavior', async () => {
    mocks.comm = {
      currency: 'USD',
      is_telegram: true,
      telegram_discuss_link: '',
    };
    mocks.userInfo = {
      ...mocks.userInfo,
      auto_renewal: 0,
      remind_expire: 0,
      remind_traffic: 0,
      telegram_id: 12345,
    };
    mocks.resetSub.mockResolvedValue(true);
    mocks.unbindTelegram.mockResolvedValue(true);

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const resetButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-reset-button"]',
    );
    expect(resetButton).toBeTruthy();

    await act(async () => {
      resetButton!.click();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('确定要重置订阅信息？');
    expect(container.textContent).toContain(
      '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
    );

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="profile-confirm-primary"]')!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.resetSub).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith('重置成功');
    expect(mocks.refetchInfo).not.toHaveBeenCalled();

    const unbindButton = container.querySelector<HTMLButtonElement>(
      '[data-testid="profile-telegram-unbind-button"]',
    );
    expect(unbindButton).toBeTruthy();

    await act(async () => {
      unbindButton!.click();
      await Promise.resolve();
    });

    expect(container.textContent).toContain('确定要解除绑定Telegram？');
    expect(container.textContent).toContain(
      '如果你的Telegram ID已失效可以进行此操作。重置后你需要重新进行绑定。',
    );

    await act(async () => {
      container.querySelector<HTMLButtonElement>('[data-testid="profile-confirm-primary"]')!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.unbindTelegram).toHaveBeenCalledTimes(1);
    // Unbinding's user-record refresh now lives in the mutation's onSuccess; the
    // disabled subscribe query still needs the explicit call-site refetch.
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
    expect(mocks.refetchSubscribe).toHaveBeenCalledTimes(1);
  });
});
