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
    'auth.hide_password': '隐藏密码',
    'auth.invite_code': '邀请码',
    'auth.invite_code_optional': '邀请码(选填)',
    'auth.password': '密码',
    'auth.password_mismatch': '两次密码输入不同',
    'auth.return_to_login': '返回登入',
    'auth.send_code': '发送',
    'auth.show_password': '显示密码',
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

vi.mock('@/components/layout/auth-language-menu', () => ({
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

  it('renders the reskinned register card, labels, footer link, and language trigger', () => {
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
    expect(html).toContain('tw:rounded-card');
    expect(html).toContain('>V2Board</h1>');
    expect(html).toContain('邮箱');
    expect(html).toContain('<option value="example.com" selected="">@example.com</option>');
    expect(html).toContain('邮箱验证码');
    expect(html).toContain('邀请码');
    expect(html).toContain('disabled=""');
    expect(html).toContain('value="INVITE123"');
    expect(html).toContain('href="https://terms.example"');
    expect(html).toContain('返回登入');
    expect(html).toContain('href="#/login"');
    expect(html).toContain('class="v2board-auth-language-trigger"');
    expect(html).not.toContain('block block-rounded');
    expect(html).not.toContain('form-control');
    expect(html).not.toContain('btn btn-block');
    expect(html).not.toContain('placeholder=');
    expect(source).toContain("components/layout/auth-language-menu");
    expect(source).toContain("lib/auth-toast");
    expect(source).not.toContain("components/layout/language-menu");
    expect(source).not.toContain("lib/legacy-toast");
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
      email_whitelist_suffix: ['example.com', 'mail.test'],
      is_email_verify: true,
      is_invite_force: false,
      is_recaptcha: true,
    };

    await renderRegister();

    container.querySelector<HTMLInputElement>('input[name="email"]')!.value = 'user';
    const select = container.querySelector<HTMLSelectElement>('select')!;
    select.value = 'mail.test';
    await act(async () => {
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });

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

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(sendButton.textContent).toBe('59');
    expect(sendButton.disabled).toBe(true);
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
      container.querySelector<HTMLInputElement>('input[type="checkbox"]')!.dispatchEvent(
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
      container.querySelector<HTMLInputElement>('input[type="checkbox"]')!.dispatchEvent(
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

  it('reads submitted values from FormData instead of retired field refs', () => {
    expect(source).toContain('new FormData(form)');
    expect(source).toContain("readFormValue(formRef.current, 'email')");
    expect(source).toContain("readFormValue(formRef.current, 'password')");
    expect(source).toContain("readFormValue(formRef.current, 'confirm_password')");
    expect(source).not.toContain('emailRef');
    expect(source).not.toContain('passwordRef');
    expect(source).not.toContain('confirmPasswordRef');
  });

  it('keeps the recursive countdown timer without unmount cleanup', () => {
    expect(source).toContain('const cooldownRef = useRef(60);');
    expect(source).toContain('const startSendEmailVerifyCountdown = () => {');
    expect(source).toContain('startSendEmailVerifyCountdown();');
    expect(source).not.toContain('clearTimeout');
  });
});
