import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ForgetPage from './forget';

const source = readFileSync(`${process.cwd()}/src/pages/auth/forget.tsx`, 'utf8');
const controllerSource = readFileSync(
  `${process.cwd()}/src/pages/auth/use-forget-controller.ts`,
  'utf8',
);
const countdownSource = readFileSync(
  `${process.cwd()}/src/pages/auth/use-countdown.ts`,
  'utf8',
);
const flowSource = readFileSync(
  `${process.cwd()}/src/pages/auth/use-send-email-verify-flow.ts`,
  'utf8',
);

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
    'auth.forget_password': '忘记密码？',
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
  settings: {
    description: '',
    logo: '',
    title: 'V2Board',
  },
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

vi.mock('@/lib/legacy-settings', () => ({
  getLegacyTitle: () => mocks.settings.title,
}));

vi.mock('@/lib/toast', () => ({
  toast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

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
  mocks.settings.description = '';
  mocks.settings.logo = '';
  mocks.settings.title = 'V2Board';
  window.g_lang = 'zh-CN';
  window.settings = {
    i18n: ['en-US', 'zh-CN'] as string[] & Record<string, Record<string, string>>,
  };
  mocks.toastError.mockReset();
  mocks.toastSuccess.mockReset();
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

function setInputValue(input: HTMLInputElement | null, value: string) {
  if (!input) throw new Error('Expected input to exist');
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
}

describe('ForgetPage modern markup', () => {
  beforeEach(resetMocks);

  it('renders the reskinned reset card, labels, footer, and action title', () => {
    const html = renderToStaticMarkup(<ForgetPage />);

    expect(html).toContain('v2board-auth-card');
    expect(html).toContain('v2board-auth-panel');
    expect(html).toContain('rounded-xl');
    expect(html).toContain('bg-card');
    expect(html).not.toContain('v2board-auth-visual');
    expect(html).not.toContain('v2board-auth-shell-brand');
    expect(html).toContain('>重置密码</h1>');
    expect(html).toContain('验证邮箱并设置新密码');
    expect(html).not.toContain('>V2Board</h1>');
    expect(html).toContain('邮箱');
    expect(html).toContain('邮箱验证码');
    // The two password fields now carry distinct labels (password + confirm password).
    expect(html).toContain('>密码<');
    expect(html).toContain('>确认密码<');
    expect(html).toContain('重置密码');
    expect(html).toContain('返回登录');
    expect(html).toContain('href="#/login"');
    expect(html).not.toContain('v2board-auth-language-trigger');
    expect(html).not.toContain('block block-rounded');
    expect(html).not.toContain('form-control');
    expect(html).not.toContain('btn btn-block');
    expect(html).toContain('placeholder="m@example.com"');
    expect(html).not.toContain('placeholder="请输入密码"');
    expect(source).toContain("from './auth-panel'");
    // The behavior controller owns the modern toast; the view stays free of legacy toast/menu.
    expect(controllerSource).toContain("lib/toast");
    expect(controllerSource).toContain("from './auth-recaptcha'");
    expect(controllerSource).not.toContain('useLegacyRecaptcha');
    expect(source).not.toContain("components/layout/auth-language-menu");
    expect(source).not.toContain("components/layout/language-menu");
    expect(source).not.toContain("lib/legacy-toast");
    expect(controllerSource).not.toContain("lib/legacy-toast");
  });
});

describe('ForgetPage behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    resetMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    vi.useRealTimers();
    container.remove();
    document.body.innerHTML = '';
    window.settings = undefined;
    window.g_lang = undefined;
  });

  async function renderForget() {
    await act(async () => {
      root.render(<ForgetPage />);
      await Promise.resolve();
    });
  }

  it('shows the modern centered spinner after mount while guest config is fetching', async () => {
    mocks.config = undefined;
    mocks.isLoading = true;

    await renderForget();

    expect(container.querySelector('[role="status"]')).not.toBeNull();
    expect(container.querySelector('input[name="email"]')).toBeNull();
  });

  it('sends the forgot-password verification payload and starts the countdown', async () => {
    vi.useFakeTimers();
    mocks.config = { is_recaptcha: true };
    await renderForget();

    setInputValue(
      container.querySelector<HTMLInputElement>('input[name="email"]'),
      'reset@example.com',
    );
    const sendButton = Array.from(container.querySelectorAll('button')).find((button) =>
      button.textContent?.includes('发送'),
    )!;

    await act(async () => {
      sendButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.runRecaptcha).toHaveBeenCalledTimes(1);
    expect(mocks.sendCodeMutateAsync).toHaveBeenCalledWith({
      email: 'reset@example.com',
      isforget: 1,
      recaptcha_data: 'recaptcha-token',
    });
    expect(mocks.toastSuccess).toHaveBeenCalledWith('发送成功', {
      description: '如果没有收到验证码请检查垃圾箱。',
    });

    expect(sendButton.textContent).toBe('59');
    expect(sendButton.disabled).toBe(true);

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(sendButton.textContent).toBe('58');
  });

  it('shows the password mismatch toast without resetting', async () => {
    await renderForget();

    setInputValue(container.querySelector<HTMLInputElement>('input[name="password"]'), 'one');
    setInputValue(container.querySelector<HTMLInputElement>('input[name="confirm_password"]'), 'two');

    await act(async () => {
      container.querySelector('form')!.dispatchEvent(
        new Event('submit', { bubbles: true, cancelable: true }),
      );
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.toastError).toHaveBeenCalledWith('请求失败', {
      description: '两次密码输入不同',
    });
    expect(mocks.forgetMutateAsync).not.toHaveBeenCalled();
  });

  it('resets with the exact payload and returns to login after success', async () => {
    await renderForget();

    setInputValue(
      container.querySelector<HTMLInputElement>('input[name="email"]'),
      'reset@example.com',
    );
    setInputValue(container.querySelector<HTMLInputElement>('input[name="email_code"]'), '123456');
    setInputValue(container.querySelector<HTMLInputElement>('input[name="password"]'), 'secret');
    setInputValue(
      container.querySelector<HTMLInputElement>('input[name="confirm_password"]'),
      'secret',
    );

    await act(async () => {
      container.querySelector('form')!.dispatchEvent(
        new Event('submit', { bubbles: true, cancelable: true }),
      );
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.runRecaptcha).not.toHaveBeenCalled();
    expect(mocks.forgetMutateAsync).toHaveBeenCalledWith({
      email: 'reset@example.com',
      email_code: '123456',
      password: 'secret',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });

  it('uses react-hook-form and zod instead of retired refs or FormData readers', () => {
    expect(controllerSource).toContain("from 'react-hook-form'");
    expect(controllerSource).toContain("from 'zod'");
    expect(controllerSource).toContain('zodResolver(forgetSchema)');
    expect(controllerSource).toContain('form.handleSubmit(');
    expect(controllerSource).toContain("registerInput: form.register");
    expect(controllerSource).not.toContain('new FormData');
    expect(controllerSource).not.toContain('readFormValue');
    expect(controllerSource).not.toContain('formRef');
    expect(controllerSource).not.toContain('emailRef');
    expect(controllerSource).not.toContain('passwordRef');
    expect(controllerSource).not.toContain('confirmPasswordRef');
    expect(controllerSource).not.toContain('emailCodeRef');
  });

  it('delegates the send-code countdown to the shared flow hook with cleanup', () => {
    // The recaptcha-gated send + 60s cooldown is delegated to one shared hook.
    expect(controllerSource).toContain('useSendEmailVerifyFlow(');
    expect(controllerSource).not.toContain('useCountdown');
    expect(controllerSource).not.toContain('startSendEmailVerifyCountdown');
    expect(controllerSource).not.toContain('cooldownRef');
    // The shared hook owns the countdown, which owns its own timer cleanup.
    expect(flowSource).toContain('const cooldown = useCountdown(60);');
    expect(flowSource).toContain('cooldown.start()');
    expect(countdownSource).toContain('return () => window.clearTimeout(timer);');
  });
});
