import type { ReactNode } from 'react';
import { screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import ProfilePage from './index';

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  refetchInfo: vi.fn(),
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
  refetchBotInfo: vi.fn(),
  removeSession: vi.fn(),
  confirmDialog: vi.fn(),
  logout: vi.fn(),
  sessions: {
    data: undefined as
      | { session_id: string; ip: string; ua: string; login_at: string; current: boolean }[]
      | undefined,
    isLoading: false,
    isError: false,
  },
  userInfo: {
    balance: 0,
    auto_renewal: false,
    remind_expire: false,
    remind_traffic: false,
    telegram_id: null as number | null,
    email: 'user@example.test',
    uuid: 'uuid-abc-123',
    created_at: '2023-11-14T22:13:20Z',
    last_login_at: '2023-11-14T23:13:20Z' as string | null,
  },
  comm: {
    currency: 'USD',
    is_telegram: false,
    telegram_discuss_link: '',
  },
  subscribe: undefined as { subscribe_url?: string } | undefined,
  botInfo: undefined as { username: string } | undefined,
  botInfoPending: false,
  botInfoError: false,
  botInfoSuccess: true,
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
  'profile.redeem_days': '订阅时长 {{count}} 天',
  'profile.redeem_traffic': '套餐流量 {{traffic}} GB',
  'profile.redeem_reset': '流量已重置',
  'profile.redeem_plan_days': '订阅套餐 {{count}} 天',
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
  'profile.telegram_unbind_tip':
    '如果你的Telegram ID已失效可以进行此操作。重置后你需要重新进行绑定。',
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
  'profile.deposit_invalid': '请输入有效的充值金额',
  'profile.deposit_decimals': '充值金额最多支持两位小数',
};

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => createTestTranslation(labels),
}));

vi.mock('@v2board/ui/dialog', () => ({
  Dialog: ({ children, open }: { children: ReactNode; open?: boolean }) =>
    open ? <>{children}</> : null,
  DialogContent: ({ children, className }: { children: ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
  DialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock('@v2board/ui/alert-dialog', () => ({
  AlertDialogAction: ({ children }: { children: ReactNode }) => <>{children}</>,
  AlertDialogCancel: ({ children }: { children: ReactNode }) => <>{children}</>,
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
  useSubscribe: () => ({ data: mocks.subscribe }),
  useUpdateProfileMutation: () => ({
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.updateProfile, payload, options),
  }),
  useChangePasswordMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.changePassword, payload, options),
  }),
  useRedeemGiftCardMutation: () => ({
    isPending: false,
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.redeem, payload, options),
  }),
  useResetSubscribeMutation: () => ({
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.resetSub, payload, options),
  }),
  useUnbindTelegramMutation: () => ({
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.unbindTelegram, payload, options),
  }),
  useSaveOrderMutation: () => ({
    mutate: (payload: unknown, options?: MutationCallbacks) =>
      runMockMutation(mocks.saveOrder, payload, options),
  }),
  useTelegramBotInfo: () => ({
    data: mocks.botInfo,
    isError: mocks.botInfoError,
    isFetching: mocks.botInfoPending,
    isPending: mocks.botInfoPending,
    isSuccess: mocks.botInfoSuccess,
    refetch: mocks.refetchBotInfo,
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

interface MutationCallbacks {
  onError?: (error: unknown) => void;
  onSettled?: () => void;
  onSuccess?: (data: unknown) => void;
}

function runMockMutation(
  mutation: (...args: unknown[]) => unknown,
  payload: unknown,
  options?: MutationCallbacks,
) {
  void Promise.resolve(mutation(payload)).then(
    (data) => {
      options?.onSuccess?.(data);
      options?.onSettled?.();
    },
    (error: unknown) => {
      options?.onError?.(error);
      options?.onSettled?.();
    },
  );
}

vi.mock('@v2board/config/clipboard', () => ({
  copyText: mocks.copyText,
}));

vi.mock('@/lib/auth', () => ({
  logout: mocks.logout,
}));

vi.mock('@v2board/ui/confirm-dialog', () => ({
  confirmDialog: mocks.confirmDialog,
}));

vi.mock('@v2board/app-shell/toast', () => ({
  toast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

describe('ProfilePage shadcn account surface', () => {
  beforeEach(() => {
    mocks.navigate.mockClear();
    mocks.refetchInfo.mockReset();
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
    mocks.refetchBotInfo.mockReset();
    mocks.removeSession.mockReset();
    mocks.removeSession.mockResolvedValue(true);
    mocks.logout.mockClear();
    mocks.confirmDialog.mockReset();
    // The imperative confirm dialog is exercised elsewhere; here we auto-confirm
    // so the revoke flow's onConfirm runs synchronously.
    mocks.confirmDialog.mockImplementation((options: { onConfirm?: () => unknown }) => {
      void options.onConfirm?.();
      return Promise.resolve(true);
    });
    // GET /user/sessions delivers the array newest-first (W5).
    mocks.sessions = {
      data: [
        {
          session_id: 'guid-other',
          ip: '203.0.113.9',
          ua: 'Firefox on Windows',
          login_at: '2023-11-14T23:13:20Z',
          current: false,
        },
        {
          session_id: 'guid-current',
          ip: '198.51.100.4',
          ua: 'Chrome on macOS',
          login_at: '2023-11-14T22:13:20Z',
          current: true,
        },
      ],
      isLoading: false,
      isError: false,
    };
    mocks.userInfo = {
      balance: 0,
      auto_renewal: false,
      remind_expire: false,
      remind_traffic: false,
      telegram_id: null,
      email: 'user@example.test',
      uuid: 'uuid-abc-123',
      created_at: '2023-11-14T22:13:20Z',
      last_login_at: '2023-11-14T23:13:20Z',
    };
    mocks.comm = {
      currency: 'USD',
      is_telegram: false,
      telegram_discuss_link: '',
    };
    mocks.subscribe = { subscribe_url: 'https://example.test/sub' };
    mocks.botInfo = undefined;
    mocks.botInfoPending = false;
    mocks.botInfoError = false;
    mocks.botInfoSuccess = true;
  });

  it('shows the redeem success toast only after the mutation resolves, leaving the refresh to the mutation', async () => {
    let resolveRedeem!: () => void;
    mocks.redeem.mockImplementation(
      () =>
        new Promise<{ type: number; value: number }>((resolve) => {
          resolveRedeem = () => resolve({ type: 1, value: 1234 });
        }),
    );

    const { user } = renderWithProviders(<ProfilePage />);

    await user.type(screen.getByLabelText('礼品卡'), 'CARD-123');
    await user.click(screen.getByTestId('profile-redeem-button'));

    await waitFor(() => expect(mocks.redeem).toHaveBeenCalledWith('CARD-123'));
    // The success toast must wait for the mutation to resolve, not fire on submit.
    expect(mocks.toastSuccess).not.toHaveBeenCalled();

    resolveRedeem();

    await waitFor(() =>
      expect(mocks.toastSuccess).toHaveBeenCalledWith('兑换成功: 账户余额 12.34'),
    );
    // The user-record refresh is now the mutation's onSuccess job (see
    // queries.test.ts), so the component no longer refetches here directly.
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
  });

  it('re-enables gift card redemption after a transport failure', async () => {
    mocks.redeem.mockRejectedValue(new Error('timeout exceeded'));

    const { user } = renderWithProviders(<ProfilePage />);

    await user.type(screen.getByLabelText('礼品卡'), 'CARD-FAIL');
    await user.click(screen.getByTestId('profile-redeem-button'));

    await waitFor(() => expect(mocks.redeem).toHaveBeenCalledWith('CARD-FAIL'));
    const redeemButton = screen.getByTestId('profile-redeem-button');
    await waitFor(() => expect(redeemButton).toBeEnabled());
    expect(redeemButton).not.toHaveAttribute('aria-busy', 'true');
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
  });

  it('rejects an empty gift card inline without calling the redeem API', async () => {
    const { user } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByTestId('profile-redeem-button'));

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledWith('请输入礼品卡'));
    expect(mocks.redeem).not.toHaveBeenCalled();
    const giftCard = screen.getByLabelText('礼品卡');
    expect(screen.getByText('请输入礼品卡')).toBeInTheDocument();
    expect(giftCard).toHaveAttribute('aria-invalid', 'true');
    expect(giftCard).toHaveAccessibleDescription('请输入礼品卡');
  });

  it('renders the shadcn profile cards and telegram binding dialog content', async () => {
    mocks.comm = {
      currency: 'USD',
      is_telegram: true,
      telegram_discuss_link: 'https://t.me/discuss',
    };
    mocks.botInfo = { username: 'legacy_bot' };

    const { user } = renderWithProviders(<ProfilePage />);

    expect(screen.getByTestId('profile-page')).toBeInTheDocument();
    expect(screen.getAllByTestId('profile-card-title').length).toBeGreaterThan(3);
    expect(
      screen.getByText(
        '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText('绑定Telegram')).toBeInTheDocument();
    expect(screen.getByText('Telegram 讨论组')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '立即加入' })).toHaveAttribute(
      'href',
      'https://t.me/discuss',
    );

    await user.click(screen.getByRole('button', { name: '立即开始' }));

    expect(screen.getByText('第一步')).toBeInTheDocument();
    expect(screen.getByText(/打开Telegram搜索/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '@legacy_bot' })).toHaveAttribute(
      'href',
      'https://t.me/legacy_bot',
    );
    expect(screen.getByText('向机器人发送您的')).toBeInTheDocument();
    const copyCode = screen.getByTestId('profile-copy-code');
    expect(copyCode).toHaveTextContent('/bind https://example.test/sub');

    await user.click(copyCode);

    expect(mocks.copyText).toHaveBeenCalledWith('/bind https://example.test/sub');
  });

  it('surfaces the account identity fields and copies the uuid', async () => {
    const { user } = renderWithProviders(<ProfilePage />);

    // The backend already returns these on info(); the card must surface them
    // instead of leaving the email/uuid/last_login/created_at keys dead.
    expect(screen.getByTestId('profile-account-card')).toBeInTheDocument();
    expect(screen.getByTestId('profile-account-email')).toHaveTextContent(/^user@example\.test$/);
    expect(screen.getByTestId('profile-account-uuid')).toHaveTextContent(/^uuid-abc-123$/);
    // Registration + last login render through the shared backend datetime formatter.
    expect(screen.getByTestId('profile-account-created')).not.toHaveTextContent('—');
    expect(screen.getByTestId('profile-account-last-login')).not.toHaveTextContent('—');

    await user.click(screen.getByRole('button', { name: '复制' }));

    expect(mocks.copyText).toHaveBeenCalledWith('uuid-abc-123');
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalled());
  });

  it('falls back to an em dash when a last login timestamp is absent', () => {
    mocks.userInfo = { ...mocks.userInfo, last_login_at: null };

    renderWithProviders(<ProfilePage />);

    expect(screen.getByTestId('profile-account-last-login')).toHaveTextContent(/^—$/);
  });

  it('lists active sessions, badges the current device, and blocks self-revocation', () => {
    renderWithProviders(<ProfilePage />);

    expect(screen.getByTestId('profile-sessions-card')).toBeInTheDocument();
    const rows = screen.getAllByTestId('profile-session-row');
    expect(rows).toHaveLength(2);
    // Both devices surface their UA + IP so an unfamiliar device is recognizable.
    expect(screen.getByText('Firefox on Windows')).toBeInTheDocument();
    expect(screen.getByText('203.0.113.9')).toBeInTheDocument();
    expect(screen.getByText('Chrome on macOS')).toBeInTheDocument();

    // Rows arrive newest-first from the API, so the current (older) device is
    // the second row.
    const badge = screen.getByTestId('profile-session-current');
    expect(rows[1]).toContainElement(badge);

    const revokeButtons = screen.getAllByRole('button', { name: '注销' });
    expect(revokeButtons).toHaveLength(2);
    // The other device can be signed out; the current device cannot revoke itself.
    expect(revokeButtons[0]).toBeEnabled();
    expect(revokeButtons[1]).toBeDisabled();
  });

  it('revokes another device through the confirm dialog and toasts on success', async () => {
    const { user } = renderWithProviders(<ProfilePage />);

    const revokeButtons = screen.getAllByRole('button', { name: '注销' });
    await user.click(revokeButtons[0]!);

    expect(mocks.confirmDialog).toHaveBeenCalledTimes(1);
    // The revoke posts the guid of the other device, not the current one.
    await waitFor(() => expect(mocks.removeSession).toHaveBeenCalledWith('guid-other'));
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('已注销该设备'));
  });

  it('shows a retryable error state when the session fetch fails', async () => {
    mocks.sessions = { data: undefined, isLoading: false, isError: true };

    const { user } = renderWithProviders(<ProfilePage />);

    expect(screen.getByTestId('profile-sessions-error')).toBeInTheDocument();
    expect(screen.queryByTestId('profile-session-row')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '重试' }));

    expect(mocks.refetchSessions).toHaveBeenCalledTimes(1);
  });

  it('shows a loading state while the session list is fetching', () => {
    mocks.sessions = { data: undefined, isLoading: true, isError: false };

    renderWithProviders(<ProfilePage />);

    expect(screen.getByTestId('profile-sessions-card')).toBeInTheDocument();
    expect(screen.queryByTestId('profile-session-row')).not.toBeInTheDocument();
    expect(screen.queryByTestId('profile-sessions-error')).not.toBeInTheDocument();
    expect(screen.getByText('加载中')).toBeInTheDocument();
  });

  it('uses the legacy bare Telegram bind command when no subscribe url is cached', async () => {
    mocks.comm = {
      currency: 'USD',
      is_telegram: true,
      telegram_discuss_link: '',
    };
    mocks.botInfo = { username: 'legacy_bot' };
    mocks.subscribe = undefined;

    const { user } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByRole('button', { name: '立即开始' }));

    // The command the user pastes to the bot must be exactly `/bind` — never
    // `/bind undefined` — both as rendered and as copied to the clipboard.
    const copyCode = screen.getByTestId('profile-copy-code');
    expect(copyCode).toHaveTextContent(/^\/bind$/);

    await user.click(copyCode);

    expect(mocks.copyText).toHaveBeenCalledWith('/bind');
  });

  it('shows Telegram bot loading separately from a retryable fetch failure', async () => {
    mocks.comm = {
      currency: 'USD',
      is_telegram: true,
      telegram_discuss_link: '',
    };
    mocks.botInfo = undefined;
    mocks.botInfoPending = true;
    mocks.botInfoSuccess = false;

    const { user, rerender } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByRole('button', { name: '立即开始' }));
    expect(screen.getByRole('status')).toHaveTextContent('加载中');
    expect(screen.queryByTestId('profile-telegram-bot-error')).not.toBeInTheDocument();

    mocks.botInfoPending = false;
    mocks.botInfoError = true;
    // The mocked bot-info hook has no store subscription (in production the
    // TanStack query re-renders the card itself), and the compiled card bails
    // out of parent-cascade re-renders — so remount and reopen to observe the
    // error state, exactly as a fresh visit would.
    rerender(<ProfilePage key="botinfo-error" />);
    await user.click(screen.getByRole('button', { name: '立即开始' }));

    expect(screen.getByTestId('profile-telegram-bot-error')).toHaveTextContent('加载失败');
    expect(screen.queryByRole('status')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '重试' }));
    expect(mocks.refetchBotInfo).toHaveBeenCalledTimes(1);
  });

  it('updates profile switches with the single changed boolean flag and leaves the refresh to the mutation', async () => {
    mocks.updateProfile.mockResolvedValue(true);

    const { user } = renderWithProviders(<ProfilePage />);

    expect(screen.getAllByTestId('profile-switch')).toHaveLength(3);
    const autoRenewal = screen.getByRole('switch', { name: '自动续费' });
    expect(autoRenewal).toHaveAttribute('aria-checked', 'false');
    const remindExpire = screen.getByRole('switch', { name: '到期邮件提醒' });
    expect(screen.getByRole('switch', { name: '流量邮件提醒' })).toBeInTheDocument();

    await user.click(autoRenewal);

    await waitFor(() => expect(mocks.updateProfile).toHaveBeenCalledWith({ auto_renewal: true }));
    expect(mocks.refetchInfo).not.toHaveBeenCalled();

    mocks.updateProfile.mockRejectedValue(new Error('failed'));

    await user.click(remindExpire);

    await waitFor(() => expect(mocks.updateProfile).toHaveBeenCalledWith({ remind_expire: true }));
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
  });

  it('renders profile switches with Radix boolean checked state', async () => {
    mocks.updateProfile.mockResolvedValue(true);
    mocks.userInfo = {
      ...mocks.userInfo,
      auto_renewal: true,
      remind_expire: false,
      remind_traffic: true,
      telegram_id: null,
    };

    const { user } = renderWithProviders(<ProfilePage />);

    // The backend sends booleans (§4.1); the switches surface them as the
    // Radix checked state.
    const switches = screen.getAllByTestId('profile-switch');
    expect(switches).toHaveLength(3);
    expect(switches[0]).toHaveAttribute('aria-checked', 'true');
    expect(switches[0]).toHaveAttribute('data-state', 'checked');
    expect(switches[1]).toHaveAttribute('aria-checked', 'false');
    expect(switches[1]).toHaveAttribute('data-state', 'unchecked');
    expect(switches[2]).toHaveAttribute('aria-checked', 'true');
    expect(switches[2]).toHaveAttribute('data-state', 'checked');

    await user.click(switches[0]!);

    await waitFor(() => expect(mocks.updateProfile).toHaveBeenCalledWith({ auto_renewal: false }));
  });

  it('blocks a mismatched confirm password with an inline error instead of calling the API', async () => {
    const { user } = renderWithProviders(<ProfilePage />);

    await user.type(screen.getByLabelText('旧密码'), 'old-password');
    const newPasswordInputs = screen.getAllByLabelText('新密码');
    expect(newPasswordInputs).toHaveLength(2);
    await user.type(newPasswordInputs[0]!, 'new-password');
    await user.type(newPasswordInputs[1]!, 'different-password');
    await user.click(screen.getByTestId('profile-password-save'));

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledWith('两次新密码输入不同'));
    // The mismatch also surfaces inline at the confirm field (toast is mocked,
    // so this text can only come from the form error).
    expect(screen.getByText('两次新密码输入不同')).toBeInTheDocument();
    expect(newPasswordInputs[1]).toHaveAttribute('aria-invalid', 'true');
    expect(newPasswordInputs[1]).toHaveAccessibleDescription('两次新密码输入不同');
    expect(mocks.changePassword).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('keeps the old password-change success flow without clearing local auth', async () => {
    mocks.changePassword.mockResolvedValue(true);

    const { user } = renderWithProviders(<ProfilePage />);

    await user.type(screen.getByLabelText('旧密码'), 'old-password');
    const newPasswordInputs = screen.getAllByLabelText('新密码');
    await user.type(newPasswordInputs[0]!, 'new-password');
    await user.type(newPasswordInputs[1]!, 'new-password');
    await user.click(screen.getByTestId('profile-password-save'));

    await waitFor(() =>
      expect(mocks.changePassword).toHaveBeenCalledWith({
        oldPassword: 'old-password',
        newPassword: 'new-password',
      }),
    );
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/login'));
    expect(mocks.toastSuccess).toHaveBeenCalledWith('修改成功，请重新登陆');
    expect(mocks.toastSuccess.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.navigate.mock.invocationCallOrder[0]!,
    );
    // Legacy behavior: the redirect happens with the token intact — the page
    // must not clear local auth on its way to /login.
    expect(mocks.logout).not.toHaveBeenCalled();
  });

  it('submits the deposit order payload from the shadcn recharge dialog', async () => {
    mocks.comm = {
      currency: 'CNY',
      is_telegram: false,
      telegram_discuss_link: '',
    };
    mocks.saveOrder.mockResolvedValue('DEPOSIT123');

    const { user } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByTestId('profile-recharge'));

    const amountInput = screen.getByTestId('profile-deposit-input');
    expect(amountInput).toHaveAttribute('placeholder', '请输入充值金额CNY');
    await user.type(amountInput, '12.34');
    await user.click(screen.getByTestId('profile-deposit-confirm'));

    await waitFor(() =>
      expect(mocks.saveOrder).toHaveBeenCalledWith({
        kind: 'deposit',
        deposit_amount: '12.34',
      }),
    );
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order/DEPOSIT123'));
  });

  it('rejects a deposit amount with more than two decimals inline instead of closing', async () => {
    mocks.comm = {
      currency: 'CNY',
      is_telegram: false,
      telegram_discuss_link: '',
    };

    const { user } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByTestId('profile-recharge'));

    // 19.999 is finite and positive but cannot be represented in cents; the old
    // path silently rounded it to 2000 cents and closed the dialog.
    const amountInput = screen.getByTestId('profile-deposit-input');
    await user.type(amountInput, '19.999');
    await user.click(screen.getByTestId('profile-deposit-confirm'));

    // The dialog stays open with an inline error rather than silently closing.
    expect(await screen.findByTestId('profile-deposit-error')).toHaveTextContent(
      '充值金额最多支持两位小数',
    );
    expect(amountInput).toHaveAttribute('aria-invalid', 'true');
    expect(amountInput).toHaveAccessibleDescription('充值金额最多支持两位小数');
    expect(mocks.saveOrder).not.toHaveBeenCalled();
    expect(screen.getByTestId('profile-deposit-confirm')).toBeInTheDocument();
  });

  it('rejects a deposit that cannot be represented as a safe integer number of cents', async () => {
    const { user } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByTestId('profile-recharge'));
    const amountInput = screen.getByTestId('profile-deposit-input');
    await user.type(amountInput, '900719925474099.99');
    await user.click(screen.getByTestId('profile-deposit-confirm'));

    expect(await screen.findByTestId('profile-deposit-error')).toHaveTextContent(
      '请输入有效的充值金额',
    );
    expect(amountInput).toHaveAttribute('aria-invalid', 'true');
    expect(mocks.saveOrder).not.toHaveBeenCalled();
    expect(screen.getByTestId('profile-deposit-confirm')).toBeInTheDocument();
  });

  it('uses shadcn confirmation dialogs for reset and telegram unbind behavior', async () => {
    mocks.comm = {
      currency: 'USD',
      is_telegram: true,
      telegram_discuss_link: '',
    };
    mocks.userInfo = {
      ...mocks.userInfo,
      auto_renewal: false,
      remind_expire: false,
      remind_traffic: false,
      telegram_id: 12345,
    };
    mocks.resetSub.mockResolvedValue(true);
    mocks.unbindTelegram.mockResolvedValue(true);

    const { user } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByTestId('profile-reset-button'));

    expect(screen.getByText('确定要重置订阅信息？')).toBeInTheDocument();
    expect(
      screen.getByText(
        '如果您的订阅地址或信息发生泄露可以执行此操作。重置后您的 UUID 及订阅将会变更，需要重新导入订阅。',
      ),
    ).toBeInTheDocument();

    await user.click(screen.getByTestId('profile-confirm-primary'));

    await waitFor(() => expect(mocks.resetSub).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalledWith('重置成功'));
    expect(mocks.refetchInfo).not.toHaveBeenCalled();

    await user.click(screen.getByTestId('profile-telegram-unbind-button'));

    expect(screen.getByText('确定要解除绑定Telegram？')).toBeInTheDocument();
    expect(
      screen.getByText('如果你的Telegram ID已失效可以进行此操作。重置后你需要重新进行绑定。'),
    ).toBeInTheDocument();

    await user.click(screen.getByTestId('profile-confirm-primary'));

    await waitFor(() => expect(mocks.unbindTelegram).toHaveBeenCalledTimes(1));
    // Unbinding's user-record refresh lives in the mutation's onSuccess.
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
  });
});
