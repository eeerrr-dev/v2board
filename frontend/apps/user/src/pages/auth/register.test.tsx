import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import RegisterPage from './register';

const source = readFileSync(`${process.cwd()}/src/pages/auth/register.tsx`, 'utf8');

const mocks = vi.hoisted(() => ({
  config: undefined as Record<string, unknown> | undefined,
  isFetching: false,
  isPending: false,
  isSendingCode: false,
  labels: {
    'auth.email': '邮箱',
    'auth.email_code': '邮箱验证码',
    'auth.invite_code': '邀请码',
    'auth.invite_code_optional': '邀请码(选填)',
    'auth.password': '密码',
    'auth.password_mismatch': '两次密码输入不同',
    'auth.return_to_login': '返回登入',
    'auth.send_code': '发送',
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

vi.mock('react-router-dom', () => ({
  useNavigate: () => mocks.navigate,
  useSearchParams: () => [mocks.params],
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => mocks.labels[key] ?? key,
  }),
}));

vi.mock('@/components/layout/language-menu', () => ({
  LanguageMenu: () => (
    <span className="v2board-login-i18n-btn">
      <i className="si si-globe pr-1" />
      <span className="font-size-sm text-muted" style={{ verticalAlign: 'text-bottom' }}>
        简体中文
      </span>
    </span>
  ),
}));

vi.mock('@/components/legacy-recaptcha', () => ({
  useLegacyRecaptcha: () => ({
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

vi.mock('@/lib/legacy-toast', () => ({
  toast: {
    error: mocks.toastError,
    success: mocks.toastSuccess,
  },
}));

vi.mock('@/lib/errors', () => ({
  i18nGet: (message: string) => message,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const originalConsoleError = console.error.bind(console);

function suppressLegacyCheckboxWarning() {
  return vi.spyOn(console, 'error').mockImplementation((...args: unknown[]) => {
    if (String(args[0]).includes('checked` prop to a form field')) return;
    originalConsoleError(...args);
  });
}

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
  mocks.toastError.mockReset();
  mocks.toastSuccess.mockReset();
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('RegisterPage bundled-theme config loading', () => {
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
    container.remove();
    document.body.innerHTML = '';
  });

  it('renders the form while the guest config dispatch equivalent is loading', () => {
    mocks.config = undefined;
    mocks.isFetching = true;

    const html = renderToStaticMarkup(<RegisterPage />);

    expect(html).toContain('placeholder="邮箱"');
    expect(html).toContain('placeholder="密码"');
    expect(html).toContain('placeholder="邀请码(选填)"');
    expect(html).toContain('class="si si-emoticon-smile mr-1"');
    expect(html).toContain('注册');
    expect(html).not.toContain('spinner-grow');
    expect(html).not.toContain('Loading...');
  });

  it('switches to the original centered spinner after mount while guest config is fetching', async () => {
    mocks.config = undefined;
    mocks.isFetching = true;

    await act(async () => {
      root.render(<RegisterPage />);
      await Promise.resolve();
    });

    expect(container.innerHTML).toContain('content content-full text-center');
    expect(container.innerHTML).toContain('spinner-grow text-primary');
    expect(container.innerHTML).toContain('Loading...');
    expect(container.innerHTML).not.toContain('placeholder="邮箱"');
    expect(container.innerHTML).not.toContain('placeholder="密码"');
    expect(container.innerHTML).not.toContain('placeholder="邀请码(选填)"');
  });
});

describe('RegisterPage bundled-theme form and behavior', () => {
  let container: HTMLDivElement;
  let consoleErrorSpy: ReturnType<typeof suppressLegacyCheckboxWarning>;
  let root: Root;

  beforeEach(() => {
    resetMocks();
    consoleErrorSpy = suppressLegacyCheckboxWarning();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    consoleErrorSpy.mockRestore();
    vi.useRealTimers();
    container.remove();
    document.body.innerHTML = '';
  });

  async function renderRegister() {
    await act(async () => {
      root.render(<RegisterPage />);
      await Promise.resolve();
    });
  }

  it('renders the old whitelist, email-code, invite-code, TOS, footer, and i18n markup', () => {
    mocks.config = {
      email_whitelist_suffix: ['example.com', 'mail.test'],
      is_email_verify: true,
      is_invite_force: true,
      is_recaptcha: true,
      tos_url: 'https://terms.example',
    };
    mocks.params = new URLSearchParams('code=INVITE123');

    const html = renderToStaticMarkup(<RegisterPage />);

    expect(html).toContain(
      'block block-rounded block-transparent block-fx-pop w-100 mb-0 overflow-hidden bg-image',
    );
    expect(html).toContain('form-group v2board-email-whitelist-enable');
    expect(html).toContain('<option value="example.com" selected="">@example.com</option>');
    expect(html).toContain('placeholder="邮箱验证码"');
    expect(html).toContain('class="btn btn-block btn-primary font-w400"');
    expect(html).toContain('placeholder="邀请码"');
    expect(html).toContain('disabled=""');
    expect(html).toContain('value="INVITE123"');
    expect(html).toContain('custom-control custom-checkbox custom-control-primary');
    expect(html).toContain('href="https://terms.example"');
    expect(html).toContain('class="si si-emoticon-smile mr-1"');
    expect(html).toContain('返回登入');
    expect(html).toContain('class="v2board-login-i18n-btn"');
    expect(html).toContain('简体中文');
  });

  it('sends the old email-verify payload and starts the 60-second countdown after success', async () => {
    vi.useFakeTimers();
    mocks.config = {
      email_whitelist_suffix: ['example.com', 'mail.test'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
    };

    await renderRegister();

    const email = container.querySelector<HTMLInputElement>('input[placeholder="邮箱"]')!;
    email.value = 'user';
    const select = container.querySelector<HTMLSelectElement>('select')!;
    select.value = 'mail.test';
    await act(async () => {
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });

    const sendButton = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '发送',
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

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(sendButton.textContent).toBe('59');
    expect(sendButton.disabled).toBe(true);
  });

  it('shows the old password mismatch toast without registering', async () => {
    await renderRegister();

    const passwordInputs = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="password"]'));
    passwordInputs[0]!.value = 'one';
    passwordInputs[1]!.value = 'two';

    await act(async () => {
      container.querySelector('button.btn-primary')!.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.toastError).toHaveBeenCalledWith('请求失败', {
      description: '两次密码输入不同',
    });
    expect(mocks.registerMutateAsync).not.toHaveBeenCalled();
  });

  it('keeps the original TOS-disabled register button until the checkbox is clicked', async () => {
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
      container.querySelector<HTMLInputElement>('input[type="checkbox"]')!.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
      await Promise.resolve();
    });

    expect(submit.disabled).toBe(false);
  });

  it('registers with the exact old payload and returns to login after success', async () => {
    mocks.config = {
      email_whitelist_suffix: ['example.com'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
      tos_url: 'https://terms.example',
    };

    await renderRegister();

    container.querySelector<HTMLInputElement>('input[placeholder="邮箱"]')!.value = 'new-user';
    container.querySelector<HTMLInputElement>('input[placeholder="邮箱验证码"]')!.value = '123456';
    const passwordInputs = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="password"]'));
    passwordInputs[0]!.value = 'secret';
    passwordInputs[1]!.value = 'secret';
    container.querySelector<HTMLInputElement>('input[placeholder="邀请码(选填)"]')!.value = 'INVITE';

    await act(async () => {
      container.querySelector<HTMLInputElement>('input[type="checkbox"]')!.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
      await Promise.resolve();
    });

    const submit = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '注册',
    )!;
    await act(async () => {
      submit.dispatchEvent(new MouseEvent('click', { bubbles: true }));
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

  it('keeps the original register values as direct ref reads', () => {
    expect(source).toContain('const email = emailRef.current!.value;');
    expect(source).toContain('const password = passwordRef.current!.value;');
    expect(source).toContain('password !== confirmPasswordRef.current!.value');
    expect(source).toContain('invite_code: inviteCodeRef.current!.value');
    expect(source).toContain("email_code: config?.is_email_verify ? emailCodeRef.current!.value : ''");
    expect(source).not.toContain("emailRef.current?.value ?? ''");
    expect(source).not.toContain("passwordRef.current?.value ?? ''");
    expect(source).not.toContain("confirmPasswordRef.current?.value ?? ''");
    expect(source).not.toContain("inviteCodeRef.current?.value ?? ''");
    expect(source).not.toContain("emailCodeRef.current?.value ?? ''");
  });

  it('keeps the old recursive countdown timer without unmount cleanup', () => {
    expect(source).toContain('const cooldownRef = useRef(60);');
    expect(source).toContain('const startSendEmailVerifyCountdown = () => {');
    expect(source).toContain('startSendEmailVerifyCountdown();');
    expect(source).not.toContain('clearTimeout');
  });

  it('keeps the footer login link as a javascript-style anchor', async () => {
    await renderRegister();

    const link = Array.from(container.querySelectorAll('a')).find(
      (anchor) => anchor.textContent === '返回登入',
    )!;

    expect(link.getAttribute('href')).toBe('javascript:void(0);');

    await act(async () => {
      link.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });
});
