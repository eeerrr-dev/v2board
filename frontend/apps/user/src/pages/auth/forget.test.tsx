import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import ForgetPage from './forget';

const mocks = vi.hoisted(() => ({
  config: undefined as Record<string, unknown> | undefined,
  forgetMutateAsync: vi.fn(),
  isLoading: false,
  isPending: false,
  isSendingCode: false,
  labels: {
    'auth.confirm_password': '确认密码',
    'auth.email': '邮箱',
    'auth.email_code': '邮箱验证码',
    'auth.email_code_sent_description': '如果没有收到验证码请检查垃圾箱。',
    'auth.email_code_sent_title': '发送成功',
    'auth.password': '密码',
    'auth.password_mismatch': '两次密码输入不同',
    'auth.reset_description': '验证邮箱并设置新密码',
    'auth.reset_title': '重置密码',
    'auth.return_to_login': '返回登录',
    'auth.send_code': '发送',
    'auth.submit_reset': '重置密码',
  } as Record<string, string>,
  navigate: vi.fn(),
  runRecaptcha: vi.fn(),
  sendCodeMutateAsync: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => mocks.labels[key] ?? key,
  }),
}));

vi.mock('./auth-recaptcha', () => ({
  useAuthRecaptcha: () => ({
    recaptchaModal: <div data-testid="recaptcha-modal" />,
    run: mocks.runRecaptcha,
  }),
}));

vi.mock('@/lib/guest', () => ({
  useForgetMutation: () => ({
    isPending: mocks.isPending,
    mutateAsync: mocks.forgetMutateAsync,
  }),
  useGuestConfig: () => ({ data: mocks.config, isLoading: mocks.isLoading }),
  useSendEmailVerifyMutation: () => ({
    isPending: mocks.isSendingCode,
    mutateAsync: mocks.sendCodeMutateAsync,
  }),
}));

vi.mock('@/lib/errors', () => ({
  i18nGet: (message: string) => message,
}));

vi.mock('@/lib/toast', () => ({
  toast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

function resetMocks() {
  mocks.config = {
    is_recaptcha: false,
  };
  mocks.forgetMutateAsync.mockReset();
  mocks.forgetMutateAsync.mockResolvedValue(true);
  mocks.isLoading = false;
  mocks.isPending = false;
  mocks.isSendingCode = false;
  mocks.navigate.mockReset();
  mocks.runRecaptcha.mockReset();
  mocks.runRecaptcha.mockImplementation((action: (token?: string) => void | Promise<void>) => {
    void action('recaptcha-token');
  });
  mocks.sendCodeMutateAsync.mockReset();
  mocks.sendCodeMutateAsync.mockResolvedValue(true);
  mocks.toastError.mockReset();
  mocks.toastSuccess.mockReset();
}

/**
 * userEvent session whose internal delays advance vitest fake timers.
 *
 * RTL 16's asyncWrapper drains the microtask queue with a `setTimeout(0)` and
 * only advances fake timers when a `jest` global exists, so every user-event
 * call would deadlock under vitest fake timers without this minimal shim.
 */
function setupFakeTimerUser() {
  vi.useFakeTimers();
  vi.stubGlobal('jest', { advanceTimersByTime: (ms: number) => vi.advanceTimersByTime(ms) });
  return userEvent.setup({ advanceTimers: (ms) => vi.advanceTimersByTime(ms) });
}

describe('ForgetPage', () => {
  beforeEach(resetMocks);

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('renders the reset card with labeled fields, actions, and the return-to-login link', () => {
    const { container } = renderWithProviders(<ForgetPage />);

    // Parity-harness hook: visual/interaction parity selects the auth surface
    // via `.v2board-auth-card` (the interaction-parity harness).
    expect(container.querySelector('.v2board-auth-card')).not.toBeNull();

    // The card headline is the reset flow title, not the site title.
    expect(screen.getAllByRole('heading')).toHaveLength(1);
    expect(screen.getByRole('heading', { level: 1, name: '重置密码' })).toBeInTheDocument();
    expect(screen.getByText('验证邮箱并设置新密码')).toBeInTheDocument();

    const email = screen.getByLabelText('邮箱');
    expect(email).toHaveAttribute('type', 'email');
    expect(email).toHaveAttribute('placeholder', 'm@example.com');
    expect(screen.getByLabelText('邮箱验证码')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '发送' })).toBeEnabled();

    // The two password fields carry distinct labels (password + confirm
    // password) and no legacy placeholder text.
    const password = screen.getByLabelText('密码');
    expect(password).toHaveAttribute('type', 'password');
    expect(password).not.toHaveAttribute('placeholder');
    expect(screen.getByLabelText('确认密码')).toHaveAttribute('type', 'password');

    expect(screen.getByRole('button', { name: '重置密码' })).toHaveAttribute('type', 'submit');
    // Tier-1 hash route: the footer link returns to #/login.
    expect(screen.getByRole('link', { name: '返回登录' })).toHaveAttribute('href', '#/login');
  });

  it('shows the loading status instead of the form while guest config is fetching', () => {
    mocks.config = undefined;
    mocks.isLoading = true;

    renderWithProviders(<ForgetPage />);

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByLabelText('邮箱')).not.toBeInTheDocument();
  });

  it('sends the forgot-password verification payload and starts the countdown', async () => {
    const user = setupFakeTimerUser();
    mocks.config = { is_recaptcha: true };
    renderWithProviders(<ForgetPage />);

    await user.type(screen.getByLabelText('邮箱'), 'reset@example.com');
    const sendButton = screen.getByRole('button', { name: '发送' });
    await user.click(sendButton);
    // Flush the send-code promise chain (mutation -> toast -> cooldown.start).
    await act(async () => {});

    expect(mocks.runRecaptcha).toHaveBeenCalledTimes(1);
    expect(mocks.sendCodeMutateAsync).toHaveBeenCalledWith({
      email: 'reset@example.com',
      isforget: 1,
      recaptcha_data: 'recaptcha-token',
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith('发送成功', {
      description: '如果没有收到验证码请检查垃圾箱。',
    });

    // The 60-second cooldown starts: the button drops to 59 and disables...
    expect(sendButton).toBeDisabled();
    expect(sendButton).toHaveTextContent('59');

    // ...and ticks down once per second.
    await act(async () => {
      vi.advanceTimersByTime(1000);
    });
    expect(sendButton).toHaveTextContent('58');
  });

  it('clears the pending countdown timer on unmount', async () => {
    const user = setupFakeTimerUser();
    const { unmount } = renderWithProviders(<ForgetPage />);

    await user.type(screen.getByLabelText('邮箱'), 'reset@example.com');
    const sendButton = screen.getByRole('button', { name: '发送' });
    await user.click(sendButton);
    // Flush the send-code promise chain so the countdown timer is scheduled.
    await act(async () => {});
    expect(sendButton).toHaveTextContent('59');
    expect(vi.getTimerCount()).toBeGreaterThan(0);

    unmount();

    expect(vi.getTimerCount()).toBe(0);
  });

  it('blocks the reset and surfaces the mismatch error when passwords differ', async () => {
    const { user } = renderWithProviders(<ForgetPage />);

    await user.type(screen.getByLabelText('密码'), 'one');
    await user.type(screen.getByLabelText('确认密码'), 'two');
    await user.click(screen.getByRole('button', { name: '重置密码' }));

    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith('请求失败', {
        description: '两次密码输入不同',
      }),
    );
    // The mismatch also renders as an inline error on the confirm field.
    expect(screen.getByRole('alert')).toHaveTextContent('两次密码输入不同');
    expect(mocks.forgetMutateAsync).not.toHaveBeenCalled();
  });

  it('resets with the exact payload and returns to login after success', async () => {
    const { user } = renderWithProviders(<ForgetPage />);

    await user.type(screen.getByLabelText('邮箱'), 'reset@example.com');
    await user.type(screen.getByLabelText('邮箱验证码'), '123456');
    await user.type(screen.getByLabelText('密码'), 'secret');
    await user.type(screen.getByLabelText('确认密码'), 'secret');
    await user.click(screen.getByRole('button', { name: '重置密码' }));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/login'));
    // Only send-code is recaptcha-gated; the reset submit itself is not.
    expect(mocks.runRecaptcha).not.toHaveBeenCalled();
    expect(mocks.forgetMutateAsync).toHaveBeenCalledWith({
      email: 'reset@example.com',
      email_code: '123456',
      password: 'secret',
    });
  });
});
