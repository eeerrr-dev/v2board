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

const mocks = vi.hoisted(() => ({
  config: undefined as Record<string, unknown> | undefined,
  forgetMutateAsync: vi.fn(),
  isFetching: false,
  isPending: false,
  isSendingCode: false,
  labels: {
    'auth.confirm_password': '确认密码',
    'auth.email': '邮箱',
    'auth.email_code': '邮箱验证码',
    'auth.hide_password': '隐藏密码',
    'auth.password': '密码',
    'auth.password_mismatch': '两次密码输入不同',
    'auth.return_to_login': '返回登入',
    'auth.send_code': '发送',
    'auth.show_password': '显示密码',
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

vi.mock('react-router-dom', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => mocks.labels[key] ?? key,
  }),
}));

vi.mock('./auth-language-menu', () => ({
  AuthLanguageMenu: () => (
    <button type="button" className="v2board-auth-language-trigger">
      简体中文
    </button>
  ),
}));

vi.mock('@/components/legacy-recaptcha', () => ({
  useLegacyRecaptcha: () => ({
    recaptchaModal: <div data-testid="recaptcha-modal" />,
    run: mocks.runRecaptcha,
  }),
}));

vi.mock('@/lib/guest', () => ({
  useForgetMutation: () => ({
    isPending: mocks.isPending,
    mutateAsync: mocks.forgetMutateAsync,
  }),
  useGuestConfig: () => ({ data: mocks.config, isFetching: mocks.isFetching }),
  useSendEmailVerifyMutation: () => ({
    isPending: mocks.isSendingCode,
    mutateAsync: mocks.sendCodeMutateAsync,
  }),
}));

vi.mock('@/lib/errors', () => ({
  i18nGet: (message: string) => message,
}));

vi.mock('@/lib/legacy-settings', () => ({
  getLegacyDescription: () => mocks.settings.description,
  getLegacyLogo: () => mocks.settings.logo,
  getLegacyTitle: () => mocks.settings.title,
}));

vi.mock('@/lib/auth-toast', () => ({
  authToast: {
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
  mocks.isFetching = false;
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
  mocks.toastError.mockReset();
  mocks.toastSuccess.mockReset();
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('ForgetPage modern markup', () => {
  beforeEach(resetMocks);

  it('renders the reskinned reset card, labels, footer, and language trigger', () => {
    const html = renderToStaticMarkup(<ForgetPage />);

    expect(html).toContain('v2board-auth-card');
    expect(html).toContain('tw:rounded-card');
    expect(html).toContain('>V2Board</h1>');
    expect(html).toContain('邮箱');
    expect(html).toContain('邮箱验证码');
    // The two password fields now carry distinct labels (password + confirm password).
    expect(html).toContain('>密码<');
    expect(html).toContain('>确认密码<');
    expect(html).toContain('重置密码');
    expect(html).toContain('返回登入');
    expect(html).toContain('href="#/login"');
    expect(html).toContain('class="v2board-auth-language-trigger"');
    expect(html).not.toContain('block block-rounded');
    expect(html).not.toContain('form-control');
    expect(html).not.toContain('btn btn-block');
    expect(html).not.toContain('placeholder=');
    expect(source).toContain("from './auth-panel'");
    // The behavior controller owns the modern auth-toast; the view stays free of legacy toast/menu.
    expect(controllerSource).toContain("lib/auth-toast");
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
  });

  async function renderForget() {
    await act(async () => {
      root.render(<ForgetPage />);
      await Promise.resolve();
    });
  }

  it('shows the modern centered spinner after mount while guest config is fetching', async () => {
    mocks.config = undefined;
    mocks.isFetching = true;

    await renderForget();

    expect(container.querySelector('[role="status"]')).not.toBeNull();
    expect(container.querySelector('input[name="email"]')).toBeNull();
  });

  it('sends the forgot-password verification payload and starts the countdown', async () => {
    vi.useFakeTimers();
    mocks.config = { is_recaptcha: true };
    await renderForget();

    container.querySelector<HTMLInputElement>('input[name="email"]')!.value = 'reset@example.com';
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

    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = 'one';
    container.querySelector<HTMLInputElement>('input[name="confirm_password"]')!.value = 'two';

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

    container.querySelector<HTMLInputElement>('input[name="email"]')!.value = 'reset@example.com';
    container.querySelector<HTMLInputElement>('input[name="email_code"]')!.value = '123456';
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = 'secret';
    container.querySelector<HTMLInputElement>('input[name="confirm_password"]')!.value = 'secret';

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

  it('reads submitted values from FormData instead of retired field refs', () => {
    // Behavior moved into the controller (mirroring login); the FormData-not-refs contract holds there.
    expect(controllerSource).toContain('new FormData(form)');
    expect(controllerSource).toContain("readFormValue(formRef.current, 'email')");
    expect(controllerSource).toContain("readFormValue(formRef.current, 'password')");
    expect(controllerSource).toContain("readFormValue(formRef.current, 'confirm_password')");
    expect(controllerSource).toContain("readFormValue(formRef.current, 'email_code')");
    expect(controllerSource).not.toContain('emailRef');
    expect(controllerSource).not.toContain('passwordRef');
    expect(controllerSource).not.toContain('confirmPasswordRef');
    expect(controllerSource).not.toContain('emailCodeRef');
  });

  it('runs the countdown as a cleanup-aware React effect', () => {
    expect(controllerSource).toContain('const mountedRef = useRef(true);');
    expect(controllerSource).toContain('if (!mountedRef.current) return;');
    expect(controllerSource).toContain("if (mountedRef.current) navigate('/login');");
    expect(controllerSource).toContain('useEffect(() => {');
    expect(controllerSource).toContain('return () => window.clearTimeout(timer);');
    expect(controllerSource).toContain('const startSendEmailVerifyCountdown = useCallback(() => {');
    expect(controllerSource).not.toContain('cooldownRef');
  });
});
