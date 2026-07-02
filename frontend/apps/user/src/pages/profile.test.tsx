import type { ReactNode } from 'react';
import { screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '@v2board/api-client';
import { renderWithProviders } from '@/test/render';
import ProfilePage from './profile';

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
  logout: vi.fn(),
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
  'profile.deposit_invalid': '请输入有效的充值金额',
  'profile.deposit_decimals': '充值金额最多支持两位小数',
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
  logout: mocks.logout,
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
  beforeEach(() => {
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
    mocks.logout.mockClear();
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

  it('keeps the stuck loading state when gift card redeem times out', async () => {
    // A timeout / network drop reaches the page as an ApiError with status 0
    // (the api-client's transport-failure signal), not a plain Error.
    mocks.redeem.mockRejectedValue(new ApiError(0, 'timeout exceeded'));

    const { user } = renderWithProviders(<ProfilePage />);

    await user.type(screen.getByLabelText('礼品卡'), 'CARD-FAIL');
    await user.click(screen.getByTestId('profile-redeem-button'));

    await waitFor(() => expect(mocks.redeem).toHaveBeenCalledWith('CARD-FAIL'));
    const redeemButton = screen.getByTestId('profile-redeem-button');
    await waitFor(() => expect(redeemButton).toBeDisabled());
    expect(redeemButton).toHaveAttribute('aria-busy', 'true');
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
  });

  it('rejects an empty gift card inline without calling the redeem API', async () => {
    const { user } = renderWithProviders(<ProfilePage />);

    await user.click(screen.getByTestId('profile-redeem-button'));

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledWith('请输入礼品卡'));
    expect(mocks.redeem).not.toHaveBeenCalled();
    expect(screen.getByLabelText('礼品卡')).toHaveAttribute('aria-invalid', 'true');
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
    expect(screen.getByTestId('profile-account-email')).toHaveTextContent(
      /^user@example\.test$/,
    );
    expect(screen.getByTestId('profile-account-uuid')).toHaveTextContent(/^uuid-abc-123$/);
    // Registration + last login render through the shared legacy datetime formatter.
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

    // Rows sort newest-first, so the current (older) device is the second row.
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

  it('updates profile switches with the original 0/1 payload and leaves the refresh to the mutation', async () => {
    mocks.updateProfile.mockResolvedValue(true);

    const { user } = renderWithProviders(<ProfilePage />);

    expect(screen.getAllByTestId('profile-switch')).toHaveLength(3);
    const autoRenewal = screen.getByRole('switch', { name: '自动续费' });
    expect(autoRenewal).toHaveAttribute('aria-checked', 'false');
    const remindExpire = screen.getByRole('switch', { name: '到期邮件提醒' });
    expect(screen.getByRole('switch', { name: '流量邮件提醒' })).toBeInTheDocument();

    await user.click(autoRenewal);

    await waitFor(() => expect(mocks.updateProfile).toHaveBeenCalledWith({ auto_renewal: 1 }));
    expect(mocks.refetchInfo).not.toHaveBeenCalled();

    mocks.updateProfile.mockRejectedValue(new Error('failed'));

    await user.click(remindExpire);

    await waitFor(() => expect(mocks.updateProfile).toHaveBeenCalledWith({ remind_expire: 1 }));
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

    const { user } = renderWithProviders(<ProfilePage />);

    // The backend sends 0/1 numbers; the switches must normalize them into a
    // real Radix boolean checked state instead of leaking raw values.
    const switches = screen.getAllByTestId('profile-switch');
    expect(switches).toHaveLength(3);
    expect(switches[0]).toHaveAttribute('aria-checked', 'true');
    expect(switches[0]).toHaveAttribute('data-state', 'checked');
    expect(switches[1]).toHaveAttribute('aria-checked', 'false');
    expect(switches[1]).toHaveAttribute('data-state', 'unchecked');
    expect(switches[2]).toHaveAttribute('aria-checked', 'true');
    expect(switches[2]).toHaveAttribute('data-state', 'checked');

    await user.click(switches[0]!);

    await waitFor(() => expect(mocks.updateProfile).toHaveBeenCalledWith({ auto_renewal: 0 }));
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
        plan_id: 0,
        period: 'deposit',
        deposit_amount: 1234,
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
    await user.type(screen.getByTestId('profile-deposit-input'), '19.999');
    await user.click(screen.getByTestId('profile-deposit-confirm'));

    // The dialog stays open with an inline error rather than silently closing.
    expect(await screen.findByTestId('profile-deposit-error')).toHaveTextContent(
      '充值金额最多支持两位小数',
    );
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
      auto_renewal: 0,
      remind_expire: 0,
      remind_traffic: 0,
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
    // Unbinding's user-record refresh now lives in the mutation's onSuccess; the
    // disabled subscribe query still needs the explicit call-site refetch.
    expect(mocks.refetchInfo).not.toHaveBeenCalled();
    await waitFor(() => expect(mocks.refetchSubscribe).toHaveBeenCalledTimes(1));
  });
});
