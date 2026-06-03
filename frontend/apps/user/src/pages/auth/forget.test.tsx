import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ForgetPage from './forget';

const source = readFileSync(`${process.cwd()}/src/pages/auth/forget.tsx`, 'utf8');

const mocks = vi.hoisted(() => ({
  config: undefined as Record<string, unknown> | undefined,
  forgetMutateAsync: vi.fn(),
  isPending: false,
  isSendingCode: false,
  labels: {
    'auth.email': '邮箱',
    'auth.email_code': '邮箱验证码',
    'auth.password': '密码',
    'auth.return_to_login': '返回登入',
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

vi.mock('react-router-dom', () => ({
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
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
  useForgetMutation: () => ({
    isPending: mocks.isPending,
    mutateAsync: mocks.forgetMutateAsync,
  }),
  useGuestConfig: () => ({ data: mocks.config }),
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

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.config = {
    is_recaptcha: false,
  };
  mocks.forgetMutateAsync.mockReset();
  mocks.forgetMutateAsync.mockResolvedValue(true);
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

describe('ForgetPage bundled-theme markup', () => {
  beforeEach(resetMocks);

  it('renders the original reset form, send-code row, footer, and language trigger', () => {
    const html = renderToStaticMarkup(<ForgetPage />);

    expect(html).toContain(
      'block block-rounded block-transparent block-fx-pop w-100 mb-0 overflow-hidden bg-image',
    );
    expect(html).toContain('class="row no-gutters"');
    expect(html).toContain('class="col-md-12 order-md-1 bg-white"');
    expect(html).toContain('placeholder="邮箱"');
    expect(html).toContain('class="form-group form-row"');
    expect(html).toContain('class="col-9"');
    expect(html).toContain('placeholder="邮箱验证码"');
    expect(html).toContain('class="col-3"');
    expect(html).toContain('class="btn btn-block btn-primary">发送</button>');
    expect(html.match(/placeholder="密码"/g)).toHaveLength(2);
    expect(html).toContain('class="si si-support mr-1"');
    expect(html).toContain('重置密码');
    expect(html).toContain('class="text-left bg-gray-lighter p-3 px-4"');
    expect(html).toContain('返回登入');
    expect(html).toContain('class="v2board-login-i18n-btn"');
    expect(html).toContain('简体中文');
  });
});

describe('ForgetPage bundled-theme behavior', () => {
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

  it('sends the old forgot-password verification payload and starts the countdown', async () => {
    vi.useFakeTimers();
    mocks.config = { is_recaptcha: true };
    await renderForget();

    container.querySelector<HTMLInputElement>('input[placeholder="邮箱"]')!.value =
      'reset@example.com';
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
      email: 'reset@example.com',
      isforget: 1,
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

  it('shows the original password mismatch toast without resetting', async () => {
    await renderForget();

    const passwordInputs = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="password"]'));
    passwordInputs[0]!.value = 'one';
    passwordInputs[1]!.value = 'two';

    const submit = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '重置密码',
    )!;
    await act(async () => {
      submit.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.toastError).toHaveBeenCalledWith('请求失败', {
      description: '两次密码输入不同',
    });
    expect(mocks.forgetMutateAsync).not.toHaveBeenCalled();
  });

  it('resets with the exact old payload and returns to login after success', async () => {
    await renderForget();

    container.querySelector<HTMLInputElement>('input[placeholder="邮箱"]')!.value =
      'reset@example.com';
    container.querySelector<HTMLInputElement>('input[placeholder="邮箱验证码"]')!.value = '123456';
    const passwordInputs = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="password"]'));
    passwordInputs[0]!.value = 'secret';
    passwordInputs[1]!.value = 'secret';

    const submit = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '重置密码',
    )!;
    await act(async () => {
      submit.dispatchEvent(new MouseEvent('click', { bubbles: true }));
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

  it('keeps the original forgot-password values as direct ref reads', () => {
    expect(source).toContain('email: emailRef.current!.value');
    expect(source).toContain('const password = passwordRef.current!.value;');
    expect(source).toContain('password !== confirmPasswordRef.current!.value');
    expect(source).toContain('email_code: emailCodeRef.current!.value');
    expect(source).not.toContain("emailRef.current?.value ?? ''");
    expect(source).not.toContain("passwordRef.current?.value ?? ''");
    expect(source).not.toContain("confirmPasswordRef.current?.value ?? ''");
    expect(source).not.toContain("emailCodeRef.current?.value ?? ''");
  });

  it('keeps the old recursive countdown timer without unmount cleanup', () => {
    expect(source).toContain('const cooldownRef = useRef(60);');
    expect(source).toContain('const startSendEmailVerifyCountdown = () => {');
    expect(source).toContain('startSendEmailVerifyCountdown();');
    expect(source).not.toContain('clearTimeout');
  });

  it('keeps the footer login link as a javascript-style anchor', async () => {
    await renderForget();

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
