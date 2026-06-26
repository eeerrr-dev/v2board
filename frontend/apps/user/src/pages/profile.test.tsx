import { readFileSync } from 'node:fs';
import { act } from 'react';
import type { ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ProfilePage from './profile';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const source = readFileSync(`${process.cwd()}/src/pages/profile.tsx`, 'utf8');

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
  userInfo: {
    balance: 0,
    auto_renewal: 0,
    remind_expire: 0,
    remind_traffic: 0,
    telegram_id: null as number | null,
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
  'profile.wallet': '我的钱包(仅消费)',
  'profile.auto_renewal': '自动续费',
  'profile.recharge': '充值',
  'profile.redeem_giftcard': '礼品卡',
  'profile.redeem_placeholder': '请输入礼品卡',
  'profile.redeem_submit': '兑换',
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
};

vi.mock('react-router-dom', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => labels[key] ?? key }),
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
  useTelegramBotInfo: () => ({
    data: mocks.botInfo,
  }),
}));

vi.mock('@/lib/legacy-settings', () => ({
  legacyCopyText: mocks.copyText,
}));

vi.mock('@/lib/legacy-toast', () => ({
  toast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

vi.mock('@/lib/api', () => ({
  apiClient: {},
}));

vi.mock('@v2board/api-client', () => ({
  user: {
    saveOrder: mocks.saveOrder,
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
    mocks.userInfo = {
      balance: 0,
      auto_renewal: 0,
      remind_expire: 0,
      remind_traffic: 0,
      telegram_id: null,
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

  it('refreshes user info after redeem success without waiting before the success toast', async () => {
    let resolveRedeem!: () => void;
    mocks.redeem.mockImplementation(
      () =>
        new Promise<{ type: number; value: number }>((resolve) => {
          resolveRedeem = () => resolve({ type: 1, value: 1234 });
        }),
    );
    mocks.refetchInfo.mockImplementation(() => new Promise(() => {}));

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const giftCardInput = container.querySelector<HTMLInputElement>(
      'input[placeholder="请输入礼品卡"]',
    );
    expect(giftCardInput).toBeTruthy();

    await act(async () => {
      giftCardInput!.value = 'CARD-123';
      giftCardInput!.dispatchEvent(new Event('input', { bubbles: true }));
      container.querySelector<HTMLButtonElement>('.v2board-profile-redeem-button')!.click();
      await Promise.resolve();
    });

    expect(mocks.redeem).toHaveBeenCalledWith('CARD-123');

    await act(async () => {
      resolveRedeem();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.refetchInfo).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith('兑换成功: 账户余额 12.34');
  });

  it('keeps the stuck loading state when gift card redeem times out', async () => {
    mocks.redeem.mockRejectedValue(new Error('timeout exceeded'));

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const giftCardInput = container.querySelector<HTMLInputElement>(
      'input[placeholder="请输入礼品卡"]',
    );
    expect(giftCardInput).toBeTruthy();

    await act(async () => {
      giftCardInput!.value = 'CARD-FAIL';
      giftCardInput!.dispatchEvent(new Event('input', { bubbles: true }));
      container.querySelector<HTMLButtonElement>('.v2board-profile-redeem-button')!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    const redeemButton = container.querySelector<HTMLButtonElement>(
      '.v2board-profile-redeem-button',
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

    expect(container.querySelector('.v2board-profile-page')).toBeTruthy();
    expect(container.querySelectorAll('.v2board-profile-card-title').length).toBeGreaterThan(3);
    expect(container.textContent).toContain(
      '当你的订阅地址或账户发生泄漏被他人滥用时，可以在此重置订阅信息。避免带来不必要的损失。',
    );
    expect(container.textContent).toContain('绑定Telegram');
    expect(container.textContent).toContain('Telegram 讨论组');

    const startButton = container.querySelector<HTMLButtonElement>(
      '.v2board-profile-telegram-start',
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
    expect(container.querySelector('code')?.textContent).toBe('/bind https://example.test/sub');

    await act(async () => {
      container.querySelector('code')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.copyText).toHaveBeenCalledWith('/bind https://example.test/sub');
  });

  it('keeps direct source values for profile switches and Telegram bind command', () => {
    expect(source).toContain('checked={data?.auto_renewal}');
    expect(source).toContain('checked={data?.remind_expire}');
    expect(source).toContain('checked={data?.remind_traffic}');
    expect(source).toContain('/bind {subscribe?.subscribe_url}');
    expect(source).not.toContain('checked={Boolean(data?.auto_renewal)}');
    expect(source).not.toContain('checked={Boolean(data?.remind_expire)}');
    expect(source).not.toContain('checked={Boolean(data?.remind_traffic)}');
    expect(source).not.toContain("/bind {subscribe?.subscribe_url ?? ''}");
  });

  it('updates profile switches with the original 0/1 payload and refetches after success', async () => {
    mocks.updateProfile.mockResolvedValue(true);

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const switches = container.querySelectorAll<HTMLButtonElement>('.v2board-profile-switch');
    expect(switches).toHaveLength(3);
    expect(switches[0]!.getAttribute('aria-checked')).toBe('false');

    await act(async () => {
      switches[0]!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.updateProfile).toHaveBeenCalledWith({ auto_renewal: 1 });
    expect(mocks.refetchInfo).toHaveBeenCalledTimes(1);

    mocks.updateProfile.mockRejectedValue(new Error('failed'));
    mocks.refetchInfo.mockClear();

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
      balance: 0,
      auto_renewal: 1,
      remind_expire: 0,
      remind_traffic: 1,
      telegram_id: null,
    };

    await act(async () => {
      root!.render(<ProfilePage />);
      await Promise.resolve();
    });

    const switches = container.querySelectorAll<HTMLButtonElement>('.v2board-profile-switch');
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

  it('keeps the original profile form values as direct ref reads', () => {
    expect(source).toContain('const oldPassword = oldPasswordRef.current!.value;');
    expect(source).toContain('const newPassword = newPasswordRef.current!.value;');
    expect(source).toContain('const confirmPassword = confirmPasswordRef.current!.value;');
    expect(source).toContain('const giftcard = giftCardRef.current!.value;');
    expect(source).not.toContain("oldPasswordRef.current?.value ?? ''");
    expect(source).not.toContain("newPasswordRef.current?.value ?? ''");
    expect(source).not.toContain("confirmPasswordRef.current?.value ?? ''");
    expect(source).not.toContain("giftCardRef.current?.value ?? ''");
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
      container.querySelector<HTMLButtonElement>('.v2board-profile-password-save')!.click();
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

  it('submits the legacy deposit order payload from the shadcn recharge dialog', async () => {
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

    const rechargeButton = container.querySelector<HTMLButtonElement>('.v2board-profile-recharge');
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
      '.v2board-profile-deposit-confirm',
    );
    expect(confirmButton).toBeTruthy();

    await act(async () => {
      confirmButton!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.saveOrder).toHaveBeenCalledWith(expect.anything(), {
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
      balance: 0,
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

    const resetButton = container.querySelector<HTMLButtonElement>('.v2board-profile-reset-button');
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
      container.querySelector<HTMLButtonElement>('.v2board-profile-confirm-primary')!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.resetSub).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith('重置成功');
    expect(mocks.refetchInfo).not.toHaveBeenCalled();

    const unbindButton = container.querySelector<HTMLButtonElement>(
      '.v2board-profile-telegram-unbind-button',
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
      container.querySelector<HTMLButtonElement>('.v2board-profile-confirm-primary')!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(mocks.unbindTelegram).toHaveBeenCalledTimes(1);
    expect(mocks.refetchInfo).toHaveBeenCalledTimes(1);
    expect(mocks.refetchSubscribe).toHaveBeenCalledTimes(1);
  });
});
