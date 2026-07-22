import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { cloneElement, type ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LinkProps } from 'react-router';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import RegisterPage from './register';
import { useRegisterController } from './use-register-controller';

const mocks = vi.hoisted(() => ({
  config: undefined as Record<string, unknown> | undefined,
  isLoading: false,
  isError: false,
  isSuccess: true,
  isPending: false,
  isSendingCode: false,
  labels: {
    'auth.confirm_password': '确认密码',
    'auth.email': '邮箱',
    'auth.email_invalid': '请输入有效邮箱',
    'auth.email_code': '邮箱验证码',
    'auth.email_code_invalid': '请输入 6 位数字邮箱验证码',
    'auth.email_code_sent_description': '如果没有收到验证码请检查垃圾箱。',
    'auth.email_code_sent_title': '发送成功',
    'auth.email_domain': '邮箱后缀',
    'auth.have_account': '已有账号？',
    'auth.invite_code': '邀请码',
    'auth.invite_code_optional': '邀请码(选填)',
    'auth.invite_code_required': '请输入邀请码',
    'auth.password': '密码',
    'auth.password_min': '密码至少需要 8 个字符',
    'auth.password_mismatch': '两次密码输入不同',
    'auth.register_description': '填写信息开始使用',
    'auth.register_title': '创建账户',
    'auth.send_code': '发送',
    'auth.sign_in': '登录',
    'auth.submit_register': '注册',
    'auth.tos_html': '我已阅读并同意 <terms>服务条款</terms>',
    'auth.tos_required': '请同意服务条款',
    'common.error_title': '出错了',
    'common.retry': '重试',
  } as Record<string, string>,
  navigate: vi.fn(),
  params: new URLSearchParams(),
  registerMutateAsync: vi.fn(),
  refetchConfig: vi.fn(),
  runRecaptcha: vi.fn(),
  sendCodeMutateAsync: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

vi.mock('react-router', () => ({
  Link: ({ to, children, className }: LinkProps) => (
    <a href={`#${String(to)}`} className={className}>
      {children}
    </a>
  ),
  useNavigate: () => mocks.navigate,
  useSearchParams: () => [mocks.params],
}));

vi.mock('react-i18next', () => ({
  Trans: ({ components }: { components: { terms: ReactElement } }) => (
    // eslint-disable-next-line @eslint-react/no-clone-element -- test double mirrors Trans's own cloneElement injection
    <>我已阅读并同意 {cloneElement(components.terms, {}, '服务条款')}</>
  ),
  useTranslation: () => createTestTranslation(mocks.labels),
}));

vi.mock('./auth-recaptcha', () => ({
  useAuthRecaptcha: () => ({
    recaptchaModal: <div data-testid="recaptcha-modal" />,
    run: mocks.runRecaptcha,
  }),
}));

vi.mock('@/lib/guest', () => ({
  useGuestConfig: () => ({
    data: mocks.config,
    isError: mocks.isError,
    isLoading: mocks.isLoading,
    isSuccess: mocks.isSuccess,
    refetch: mocks.refetchConfig,
  }),
  useRegisterMutation: () => ({
    isPending: mocks.isPending,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.registerMutateAsync(payload)).then(
        options?.onSuccess,
        () => undefined,
      );
    },
  }),
  useSendEmailVerifyMutation: () => ({
    isPending: mocks.isSendingCode,
    mutate: (payload: unknown, options?: { onSuccess?: (data: unknown) => void }) => {
      void Promise.resolve(mocks.sendCodeMutateAsync(payload)).then(
        options?.onSuccess,
        () => undefined,
      );
    },
  }),
}));

vi.mock('@v2board/app-shell/toast', () => ({
  toast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

vi.mock('@/lib/errors', () => ({
  i18nGet: (message: string) => message,
}));

function resetMocks() {
  mocks.config = {
    email_whitelist_suffix: undefined,
    is_email_verify: false,
    is_invite_force: false,
    is_recaptcha: false,
    tos_url: undefined,
  };
  mocks.isLoading = false;
  mocks.isError = false;
  mocks.isSuccess = true;
  mocks.isPending = false;
  mocks.isSendingCode = false;
  mocks.navigate.mockReset();
  mocks.params = new URLSearchParams();
  mocks.registerMutateAsync.mockReset();
  mocks.registerMutateAsync.mockResolvedValue({ auth_data: 'REGISTER_AUTH' });
  mocks.refetchConfig.mockReset();
  mocks.refetchConfig.mockResolvedValue(undefined);
  mocks.runRecaptcha.mockReset();
  mocks.runRecaptcha.mockImplementation((action: (token?: string) => void | Promise<void>) => {
    void action('recaptcha-token');
  });
  mocks.sendCodeMutateAsync.mockReset();
  mocks.sendCodeMutateAsync.mockResolvedValue(true);
  mocks.toastError.mockReset();
  mocks.toastSuccess.mockReset();
}

function RegisterControllerActions() {
  const { sendCode, submit } = useRegisterController();
  return (
    <form data-testid="register-controller-form" onSubmit={submit}>
      <button type="button" onClick={sendCode}>
        controller-send
      </button>
      <button type="submit">controller-submit</button>
    </form>
  );
}

beforeEach(resetMocks);

afterEach(() => {
  vi.useRealTimers();
});

describe('RegisterPage rendering', () => {
  it('renders the register card with whitelist, code, invite, TOS, and login-link contracts', () => {
    mocks.config = {
      email_whitelist_suffix: ['example.com', 'mail.test'],
      is_email_verify: true,
      is_invite_force: true,
      is_recaptcha: true,
      tos_url: 'https://terms.example',
    };
    mocks.params = new URLSearchParams('code=INVITE123');

    renderWithProviders(<RegisterPage />);

    expect(screen.getByTestId('auth-card')).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 1, name: '创建账户' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'V2Board' })).toBeNull();
    expect(screen.getByText('填写信息开始使用')).toBeInTheDocument();

    // Whitelist mode: local-part input plus the Radix suffix combobox on the default suffix.
    expect(screen.getByLabelText('邮箱')).toHaveAttribute('placeholder', 'name');
    expect(screen.getByRole('combobox', { name: '邮箱后缀' })).toHaveTextContent('@example.com');
    expect(screen.getByLabelText('邮箱验证码')).toBeInTheDocument();

    // The ?code= invite prefill locks the field to the URL value.
    const invite = screen.getByLabelText('邀请码');
    expect(invite).toBeDisabled();
    expect(invite).toHaveValue('INVITE123');

    // TOS checkbox is named by the rendered sentence (aria-labelledby wiring).
    expect(screen.getByRole('checkbox', { name: /服务条款/ })).not.toBeChecked();
    const tosLink = screen.getByRole('link', { name: '服务条款' });
    expect(tosLink).toHaveAttribute('href', 'https://terms.example');
    expect(tosLink).toHaveAttribute('target', '_blank');

    expect(screen.getByText(/已有账号？/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '登录' })).toHaveAttribute('href', '#/login');
  });

  it('renders unsafe ToS URLs as plain text instead of unsafe links', () => {
    mocks.config = {
      email_whitelist_suffix: undefined,
      is_email_verify: false,
      is_invite_force: false,
      is_recaptcha: false,
      tos_url: 'javascript:alert(1)',
    };

    renderWithProviders(<RegisterPage />);

    expect(
      screen.getByText(
        (_content, element) =>
          element?.id === 'register-tos-text' && element.textContent === '我已阅读并同意 服务条款',
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: '服务条款' })).toBeNull();
  });

  it('shows the centered loading state while guest config is fetching', () => {
    mocks.config = undefined;
    mocks.isLoading = true;
    mocks.isSuccess = false;

    renderWithProviders(<RegisterPage />);

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByLabelText('邮箱')).toBeNull();
  });

  it('fails closed on a 500 config response, offers retry, and guards controller actions', async () => {
    mocks.config = undefined;
    mocks.isError = true;
    mocks.isSuccess = false;

    const page = renderWithProviders(<RegisterPage />);

    expect(screen.getByTestId('register-config-error')).toHaveTextContent('出错了');
    expect(screen.queryByLabelText('邮箱')).not.toBeInTheDocument();
    await page.user.click(screen.getByRole('button', { name: '重试' }));
    expect(mocks.refetchConfig).toHaveBeenCalledTimes(1);

    page.unmount();
    const { user } = renderWithProviders(<RegisterControllerActions />);
    await user.click(screen.getByRole('button', { name: 'controller-send' }));
    await user.click(screen.getByRole('button', { name: 'controller-submit' }));

    expect(mocks.runRecaptcha).not.toHaveBeenCalled();
    expect(mocks.sendCodeMutateAsync).not.toHaveBeenCalled();
    expect(mocks.registerMutateAsync).not.toHaveBeenCalled();
  });
});

describe('RegisterPage behavior', () => {
  it('sends the email-verify payload and runs the 60-second countdown with timer cleanup', async () => {
    vi.useFakeTimers();
    mocks.config = {
      email_whitelist_suffix: ['mail.test'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
    };

    const { unmount } = renderWithProviders(<RegisterPage />);

    // fireEvent keeps this test synchronous so the faked clock stays in control
    // (userEvent's internal delays would deadlock against vi.useFakeTimers()).
    fireEvent.change(screen.getByLabelText('邮箱'), { target: { value: 'user' } });
    const sendButton = screen.getByRole('button', { name: '发送' });
    fireEvent.click(sendButton);
    await act(async () => {});

    expect(mocks.runRecaptcha).toHaveBeenCalledTimes(1);
    expect(mocks.sendCodeMutateAsync).toHaveBeenCalledWith({
      email: 'user@mail.test',
      is_forget: false,
      recaptcha_data: 'recaptcha-token',
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith('发送成功', {
      description: '如果没有收到验证码请检查垃圾箱。',
    });

    expect(sendButton).toBeDisabled();
    expect(sendButton).toHaveTextContent('59');

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(sendButton).toHaveTextContent('58');

    // The pending 1s tick is cleared when the page unmounts mid-countdown.
    const clearTimeoutSpy = vi.spyOn(window, 'clearTimeout');
    unmount();
    expect(clearTimeoutSpy).toHaveBeenCalled();
    clearTimeoutSpy.mockRestore();
  });

  it('shows the password mismatch error without registering', async () => {
    const { user } = renderWithProviders(<RegisterPage />);

    await user.type(screen.getByLabelText('邮箱'), 'user@example.com');
    await user.type(screen.getByLabelText('密码'), 'password-one');
    await user.type(screen.getByLabelText('确认密码'), 'password-two');
    await user.click(screen.getByRole('button', { name: '注册' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('两次密码输入不同');
    expect(mocks.toastError).not.toHaveBeenCalled();
    expect(mocks.registerMutateAsync).not.toHaveBeenCalled();
  });

  it('gates register behind the TOS checkbox', async () => {
    mocks.config = {
      email_whitelist_suffix: undefined,
      is_email_verify: false,
      is_invite_force: false,
      is_recaptcha: false,
      tos_url: 'https://terms.example',
    };

    const { user, container } = renderWithProviders(<RegisterPage />);

    await user.type(screen.getByLabelText('邮箱'), 'user@example.com');
    await user.type(screen.getByLabelText('密码'), 'password88');
    await user.type(screen.getByLabelText('确认密码'), 'password88');

    const submit = screen.getByRole('button', { name: '注册' });
    expect(submit).toBeDisabled();

    // Submitting the form directly (e.g. implicit Enter submission) still hits the gate.
    fireEvent.submit(container.querySelector('form')!);
    await waitFor(() =>
      expect(mocks.toastError).toHaveBeenCalledWith('请求失败', {
        description: '请同意服务条款',
      }),
    );
    expect(mocks.registerMutateAsync).not.toHaveBeenCalled();

    await user.click(screen.getByRole('checkbox'));
    expect(submit).toBeEnabled();
  });

  it('blocks submit with an inline invite error when invite is forced and empty', async () => {
    mocks.config = {
      email_whitelist_suffix: undefined,
      is_email_verify: false,
      is_invite_force: true,
      is_recaptcha: false,
      tos_url: undefined,
    };

    const { user } = renderWithProviders(<RegisterPage />);

    await user.type(screen.getByLabelText('邮箱'), 'new-user@example.com');
    await user.type(screen.getByLabelText('密码'), 'secret88');
    await user.type(screen.getByLabelText('确认密码'), 'secret88');
    await user.click(screen.getByRole('button', { name: '注册' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('请输入邀请码');
    expect(mocks.runRecaptcha).not.toHaveBeenCalled();
    expect(mocks.registerMutateAsync).not.toHaveBeenCalled();
  });

  it('validates the composed whitelist email before opening recaptcha or sending code', async () => {
    mocks.config = {
      email_whitelist_suffix: ['mail.test'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
    };
    const { user } = renderWithProviders(<RegisterPage />);

    await user.type(screen.getByLabelText('邮箱'), 'invalid@local');
    await user.click(screen.getByRole('button', { name: '发送' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('请输入有效邮箱');
    expect(screen.getByLabelText('邮箱')).toHaveAttribute('aria-invalid', 'true');
    expect(mocks.runRecaptcha).not.toHaveBeenCalled();
    expect(mocks.sendCodeMutateAsync).not.toHaveBeenCalled();
  });

  it('renders password and six-digit email-code errors without reaching recaptcha', async () => {
    mocks.config = {
      email_whitelist_suffix: undefined,
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
    };
    const { user } = renderWithProviders(<RegisterPage />);

    await user.type(screen.getByLabelText('邮箱'), 'user@example.com');
    await user.type(screen.getByLabelText('邮箱验证码'), '12x');
    await user.type(screen.getByLabelText('密码'), 'short');
    await user.type(screen.getByLabelText('确认密码'), 'short');
    await user.click(screen.getByRole('button', { name: '注册' }));

    expect(await screen.findByText('请输入 6 位数字邮箱验证码')).toBeInTheDocument();
    expect(screen.getByText('密码至少需要 8 个字符')).toBeInTheDocument();
    expect(mocks.runRecaptcha).not.toHaveBeenCalled();
    expect(mocks.registerMutateAsync).not.toHaveBeenCalled();
  });

  it('sends the validated email snapshot even if the input changes during recaptcha', async () => {
    mocks.config = {
      email_whitelist_suffix: ['mail.test'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
    };
    let finishRecaptcha: ((token?: string) => void | Promise<void>) | undefined;
    mocks.runRecaptcha.mockImplementation((action: (token?: string) => void | Promise<void>) => {
      finishRecaptcha = action;
    });
    const { user } = renderWithProviders(<RegisterPage />);
    const email = screen.getByLabelText('邮箱');

    await user.type(email, 'valid-user');
    await user.click(screen.getByRole('button', { name: '发送' }));
    await waitFor(() => expect(mocks.runRecaptcha).toHaveBeenCalledTimes(1));
    await user.clear(email);
    await user.type(email, 'invalid@local');
    await act(async () => {
      await finishRecaptcha?.('recaptcha-token');
    });

    expect(mocks.sendCodeMutateAsync).toHaveBeenCalledWith({
      email: 'valid-user@mail.test',
      is_forget: false,
      recaptcha_data: 'recaptcha-token',
    });
  });

  it('registers with the exact payload and returns to login after success', async () => {
    mocks.config = {
      email_whitelist_suffix: ['example.com'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
      tos_url: 'https://terms.example',
    };

    const { user } = renderWithProviders(<RegisterPage />);

    await user.type(screen.getByLabelText('邮箱'), 'new-user');
    await user.type(screen.getByLabelText('邮箱验证码'), '123456');
    await user.type(screen.getByLabelText('密码'), 'secret88');
    await user.type(screen.getByLabelText('确认密码'), 'secret88');
    await user.type(screen.getByLabelText('邀请码(选填)'), 'INVITE');
    await user.click(screen.getByRole('checkbox'));
    await user.click(screen.getByRole('button', { name: '注册' }));

    await waitFor(() =>
      expect(mocks.registerMutateAsync).toHaveBeenCalledWith({
        email: 'new-user@example.com',
        email_code: '123456',
        invite_code: 'INVITE',
        password: 'secret88',
        recaptcha_data: 'recaptcha-token',
      }),
    );
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/login'));
  });

  it('registers with a non-default selected email suffix', async () => {
    mocks.config = {
      email_whitelist_suffix: ['example.com', 'mail.test'],
      is_email_verify: false,
      is_invite_force: false,
      is_recaptcha: true,
      tos_url: undefined,
    };

    const { user } = renderWithProviders(<RegisterPage />);

    const trigger = screen.getByRole('combobox', { name: '邮箱后缀' });
    await user.click(trigger);
    await user.click(await screen.findByRole('option', { name: '@mail.test' }));
    expect(trigger).toHaveTextContent('@mail.test');

    await user.type(screen.getByLabelText('邮箱'), 'new-user');
    await user.type(screen.getByLabelText('密码'), 'secret88');
    await user.type(screen.getByLabelText('确认密码'), 'secret88');
    await user.click(screen.getByRole('button', { name: '注册' }));

    await waitFor(() =>
      expect(mocks.registerMutateAsync).toHaveBeenCalledWith({
        email: 'new-user@mail.test',
        email_code: '',
        invite_code: '',
        password: 'secret88',
        recaptcha_data: 'recaptcha-token',
      }),
    );
  });

  it('treats an empty email whitelist as disabled and submits the raw email', async () => {
    mocks.config = {
      email_whitelist_suffix: [],
      is_email_verify: false,
      is_invite_force: false,
      is_recaptcha: true,
      tos_url: undefined,
    };

    const { user } = renderWithProviders(<RegisterPage />);

    expect(screen.queryByRole('combobox')).toBeNull();
    await user.type(screen.getByLabelText('邮箱'), 'raw@example.com');
    await user.type(screen.getByLabelText('密码'), 'secret88');
    await user.type(screen.getByLabelText('确认密码'), 'secret88');
    await user.click(screen.getByRole('button', { name: '注册' }));

    await waitFor(() =>
      expect(mocks.registerMutateAsync).toHaveBeenCalledWith({
        email: 'raw@example.com',
        email_code: '',
        invite_code: '',
        password: 'secret88',
        recaptcha_data: 'recaptcha-token',
      }),
    );
  });
});
