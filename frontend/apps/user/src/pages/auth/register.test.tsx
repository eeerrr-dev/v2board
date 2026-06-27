import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import RegisterPage from './register';

const source = readFileSync(`${process.cwd()}/src/pages/auth/register.tsx`, 'utf8');
const tosSource = readFileSync(`${process.cwd()}/src/pages/auth/auth-tos-field.tsx`, 'utf8');
const controllerSource = readFileSync(
  `${process.cwd()}/src/pages/auth/use-register-controller.ts`,
  'utf8',
);

const mocks = vi.hoisted(() => ({
  config: undefined as Record<string, unknown> | undefined,
  isFetching: false,
  isPending: false,
  isSendingCode: false,
  labels: {
    'auth.confirm_password': '确认密码',
    'auth.email': '邮箱',
    'auth.email_code': '邮箱验证码',
    'auth.email_code_sent_description': '如果没有收到验证码请检查垃圾箱。',
    'auth.email_code_sent_title': '发送成功',
    'auth.hide_password': '隐藏密码',
    'auth.invite_code': '邀请码',
    'auth.invite_code_optional': '邀请码(选填)',
    'auth.password': '密码',
    'auth.password_mismatch': '两次密码输入不同',
    'auth.have_account': '已有账号？',
    'auth.register_description': '填写信息开始使用',
    'auth.register_title': '创建账户',
    'auth.return_to_login': '返回登录',
    'auth.send_code': '发送',
    'auth.show_password': '显示密码',
    'auth.sign_in': '登录',
    'auth.submit_register': '注册',
    'auth.tos_html': '我已阅读并同意 <a target="_blank" href="{url}">服务条款</a>',
    'auth.tos_required': '请同意服务条款',
  } as Record<string, string>,
  navigate: vi.fn(),
  params: new URLSearchParams(),
  registerMutateAsync: vi.fn(),
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
  useSearchParams: () => [mocks.params],
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
  useGuestConfig: () => ({ data: mocks.config, isFetching: mocks.isFetching }),
  useRegisterMutation: () => ({
    isPending: mocks.isPending,
    mutateAsync: mocks.registerMutateAsync,
  }),
  useSendEmailVerifyMutation: () => ({
    isPending: mocks.isSendingCode,
    mutateAsync: mocks.sendCodeMutateAsync,
  }),
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

vi.mock('@/lib/errors', () => ({
  i18nGet: (message: string) => message,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.config = {
    email_whitelist_suffix: undefined,
    is_email_verify: false,
    is_invite_force: false,
    is_recaptcha: false,
    tos_url: undefined,
  };
  mocks.isFetching = false;
  mocks.isPending = false;
  mocks.isSendingCode = false;
  mocks.navigate.mockReset();
  mocks.params = new URLSearchParams();
  mocks.registerMutateAsync.mockReset();
  mocks.registerMutateAsync.mockResolvedValue({ auth_data: 'REGISTER_AUTH' });
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

describe('RegisterPage modern markup', () => {
  beforeEach(resetMocks);

  it('renders the reskinned register card, labels, footer link, and action title', () => {
    mocks.config = {
      email_whitelist_suffix: ['example.com', 'mail.test'],
      is_email_verify: true,
      is_invite_force: true,
      is_recaptcha: true,
      tos_url: 'https://terms.example',
    };
    mocks.params = new URLSearchParams('code=INVITE123');

    const html = renderToStaticMarkup(<RegisterPage />);

    expect(html).toContain('v2board-auth-card');
    expect(html).toContain('max-w-md');
    expect(html).toContain('rounded-xl');
    expect(html).toContain('bg-card');
    expect(html).not.toContain('v2board-auth-visual');
    expect(html).not.toContain('md:grid-cols-2');
    expect(html).not.toContain('v2board-auth-shell-brand');
    expect(html).toContain('>创建账户</h1>');
    expect(html).toContain('填写信息开始使用');
    expect(html).not.toContain('>V2Board</h1>');
    expect(html).toContain('邮箱');
    expect(html).toContain('placeholder="name"');
    expect(html).toContain('role="combobox"');
    expect(html).toContain('@example.com');
    expect(html).not.toContain('<option');
    expect(html).toContain('邮箱验证码');
    expect(html).toContain('邀请码');
    expect(html).toContain('disabled=""');
    expect(html).toContain('value="INVITE123"');
    expect(html).toContain('id="register-tos"');
    expect(html).toContain('role="checkbox"');
    expect(html).toContain('aria-labelledby="register-tos-text"');
    expect(html).toContain('href="https://terms.example"');
    expect(html).toContain('已有账号？');
    expect(html).toContain('>登录</a>');
    expect(html).toContain('href="#/login"');
    expect(html).not.toContain('v2board-auth-language-trigger');
    expect(html).not.toContain('block block-rounded');
    expect(html).not.toContain('form-control');
    expect(html).not.toContain('btn btn-block');
    expect(html).not.toContain('placeholder="请输入密码"');
    expect(source).toContain("from './auth-panel'");
    expect(source).toContain("from './auth-tos-field'");
    // The behavior controller owns the modern auth-toast; the view stays free of legacy toast/menu.
    expect(controllerSource).toContain("lib/auth-toast");
    expect(controllerSource).toContain("from './auth-recaptcha'");
    expect(controllerSource).not.toContain('useLegacyRecaptcha');
    expect(source).not.toContain("components/layout/auth-language-menu");
    expect(source).not.toContain("components/layout/language-menu");
    expect(source).not.toContain("lib/legacy-toast");
    expect(controllerSource).not.toContain("lib/legacy-toast");
  });

  it('renders unsafe ToS URLs as plain text instead of unsafe links', () => {
    mocks.config = {
      email_whitelist_suffix: undefined,
      is_email_verify: false,
      is_invite_force: false,
      is_recaptcha: false,
      tos_url: 'javascript:alert(1)',
    };

    const html = renderToStaticMarkup(<RegisterPage />);

    expect(html).toContain('服务条款');
    expect(html).not.toContain('href="javascript:alert(1)"');
    expect(html).not.toContain('target="_blank"');
    expect(tosSource).toContain('function getSafeTosHref');
    expect(tosSource).toContain("parsed.protocol === 'http:' || parsed.protocol === 'https:'");
  });
});

describe('RegisterPage behavior', () => {
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

  async function renderRegister() {
    await act(async () => {
      root.render(<RegisterPage />);
      await Promise.resolve();
    });
  }

  it('switches to the modern centered spinner after mount while guest config is fetching', async () => {
    mocks.config = undefined;
    mocks.isFetching = true;

    await renderRegister();

    expect(container.querySelector('[role="status"]')).not.toBeNull();
    expect(container.querySelector('input[name="email"]')).toBeNull();
  });

  it('sends the email-verify payload and starts the 60-second countdown after success', async () => {
    vi.useFakeTimers();
    mocks.config = {
      email_whitelist_suffix: ['mail.test'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
    };

    await renderRegister();

    container.querySelector<HTMLInputElement>('input[name="email"]')!.value = 'user';

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
      email: 'user@mail.test',
      isforget: 0,
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

  it('shows the password mismatch toast without registering', async () => {
    await renderRegister();

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
    expect(mocks.registerMutateAsync).not.toHaveBeenCalled();
  });

  it('keeps the TOS-disabled register button until the checkbox is clicked', async () => {
    mocks.config = {
      email_whitelist_suffix: undefined,
      is_email_verify: false,
      is_invite_force: false,
      is_recaptcha: false,
      tos_url: 'https://terms.example',
    };

    await renderRegister();

    const submit = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '注册',
    )!;
    expect(submit.disabled).toBe(true);

    await act(async () => {
      container.querySelector<HTMLElement>('[role="checkbox"]')!.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
      await Promise.resolve();
    });

    expect(submit.disabled).toBe(false);
  });

  it('registers with the exact payload and returns to login after success', async () => {
    mocks.config = {
      email_whitelist_suffix: ['example.com'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
      tos_url: 'https://terms.example',
    };

    await renderRegister();

    container.querySelector<HTMLInputElement>('input[name="email"]')!.value = 'new-user';
    container.querySelector<HTMLInputElement>('input[name="email_code"]')!.value = '123456';
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = 'secret';
    container.querySelector<HTMLInputElement>('input[name="confirm_password"]')!.value = 'secret';
    container.querySelector<HTMLInputElement>('input[name="invite_code"]')!.value = 'INVITE';

    await act(async () => {
      container.querySelector<HTMLElement>('[role="checkbox"]')!.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
      await Promise.resolve();
    });

    await act(async () => {
      container.querySelector('form')!.dispatchEvent(
        new Event('submit', { bubbles: true, cancelable: true }),
      );
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.registerMutateAsync).toHaveBeenCalledWith({
      email: 'new-user@example.com',
      email_code: '123456',
      invite_code: 'INVITE',
      password: 'secret',
      recaptcha_data: 'recaptcha-token',
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });

  it('treats an empty email whitelist as disabled and submits the raw email', async () => {
    mocks.config = {
      email_whitelist_suffix: [],
      is_email_verify: false,
      is_invite_force: false,
      is_recaptcha: true,
      tos_url: undefined,
    };

    await renderRegister();

    expect(container.querySelector('select')).toBeNull();
    container.querySelector<HTMLInputElement>('input[name="email"]')!.value = 'raw@example.com';
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = 'secret';
    container.querySelector<HTMLInputElement>('input[name="confirm_password"]')!.value = 'secret';

    await act(async () => {
      container.querySelector('form')!.dispatchEvent(
        new Event('submit', { bubbles: true, cancelable: true }),
      );
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.registerMutateAsync).toHaveBeenCalledWith({
      email: 'raw@example.com',
      email_code: '',
      invite_code: '',
      password: 'secret',
      recaptcha_data: 'recaptcha-token',
    });
  });

  it('reads submitted values from FormData instead of retired field refs', () => {
    // Behavior moved into the controller (mirroring login); the FormData-not-refs contract holds there.
    expect(controllerSource).toContain('new FormData(form)');
    expect(controllerSource).toContain("readFormValue(formRef.current, 'email')");
    expect(controllerSource).toContain("readFormValue(formRef.current, 'password')");
    expect(controllerSource).toContain("readFormValue(formRef.current, 'confirm_password')");
    expect(controllerSource).not.toContain('emailRef');
    expect(controllerSource).not.toContain('passwordRef');
    expect(controllerSource).not.toContain('confirmPasswordRef');
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
