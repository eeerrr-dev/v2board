import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LoginPage from './login';

const source = readFileSync(`${process.cwd()}/src/pages/auth/login.tsx`, 'utf8');
const panelSource = readFileSync(`${process.cwd()}/src/pages/auth/auth-panel.tsx`, 'utf8');
const controllerSource = readFileSync(
  `${process.cwd()}/src/pages/auth/use-login-controller.ts`,
  'utf8',
);

const mocks = vi.hoisted(() => ({
  ApiError: class ApiError extends Error {
    status: number;
    data?: unknown;
    constructor(status: number, message: string, data?: unknown) {
      super(message);
      this.name = 'ApiError';
      this.status = status;
      this.data = data;
    }
  },
  apiClient: { name: 'apiClient' },
  checkLogin: vi.fn(),
  fetchUserInfo: vi.fn(),
  getAuthData: vi.fn(),
  isPending: false,
  labels: {
    'auth.email': '邮箱',
    'auth.forget_password': '忘记密码？',
    'auth.hide_password': '隐藏密码',
    'auth.login_description': '输入邮箱和密码继续',
    'auth.login_title': '欢迎回来',
    'auth.no_account': '还没有账号？',
    'auth.password': '密码',
    'auth.show_password': '显示密码',
    'auth.sign_up': '注册',
    'auth.submit_login': '登录',
  } as Record<string, string>,
  loginMutateAsync: vi.fn(),
  navigate: vi.fn(),
  params: new URLSearchParams(),
  queryClient: {
    fetchQuery: vi.fn(),
  },
  setAuthData: vi.fn(),
  settings: {
    description: '',
    logo: '',
    title: 'V2Board',
  },
  tokenLoginMutateAsync: vi.fn(),
}));

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
  useSearchParams: () => [mocks.params],
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => mocks.queryClient,
}));

vi.mock('@v2board/api-client', () => ({
  ApiError: mocks.ApiError,
  user: {
    checkLogin: mocks.checkLogin,
  },
}));

vi.mock('@/lib/api', () => ({
  apiClient: mocks.apiClient,
}));

vi.mock('@/lib/auth', () => ({
  getAuthData: mocks.getAuthData,
  setAuthData: mocks.setAuthData,
}));

vi.mock('@/lib/guest', () => ({
  useLoginMutation: () => ({
    isPending: mocks.isPending,
    mutateAsync: mocks.loginMutateAsync,
  }),
  useTokenLoginMutation: () => ({
    mutateAsync: mocks.tokenLoginMutateAsync,
  }),
}));

vi.mock('@/lib/legacy-settings', () => ({
  getLegacyTitle: () => mocks.settings.title,
}));

vi.mock('@/lib/queries', () => ({
  userQueryOptions: {
    info: () => ({
      queryFn: mocks.fetchUserInfo,
      queryKey: ['user', 'info'],
    }),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string) => mocks.labels[key] ?? key,
  }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.checkLogin.mockReset();
  mocks.fetchUserInfo.mockReset();
  mocks.getAuthData.mockReset();
  mocks.getAuthData.mockReturnValue(null);
  mocks.isPending = false;
  mocks.loginMutateAsync.mockReset();
  mocks.loginMutateAsync.mockResolvedValue({ auth_data: 'LOGIN_AUTH' });
  mocks.navigate.mockReset();
  mocks.params = new URLSearchParams();
  mocks.queryClient.fetchQuery.mockReset();
  mocks.queryClient.fetchQuery.mockResolvedValue(undefined);
  mocks.setAuthData.mockReset();
  mocks.settings.description = '';
  mocks.settings.logo = '';
  mocks.settings.title = 'V2Board';
  window.g_lang = 'zh-CN';
  window.settings = {
    i18n: ['en-US', 'zh-CN'] as string[] & Record<string, Record<string, string>>,
  };
  mocks.tokenLoginMutateAsync.mockReset();
  mocks.tokenLoginMutateAsync.mockResolvedValue({ auth_data: 'TOKEN_AUTH' });
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

function setInputValue(input: HTMLInputElement | undefined, value: string) {
  if (!input) throw new Error('Expected input to exist');
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(input, value);
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
}

describe('LoginPage modern markup', () => {
  beforeEach(resetMocks);

  it('renders the modern login card, translated labels, footer links, and action title', () => {
    const html = renderToStaticMarkup(<LoginPage />);

    // Pure shadcn island — auth now uses unprefixed registry-style utilities, while
    // legacy OneUI/Bootstrap form chrome stays retired.
    expect(html).toContain('v2board-auth-panel');
    expect(html).toContain('rounded-xl');
    expect(html).toContain('bg-card');
    expect(html).toContain('border-input');
    expect(html).toContain('text-card-foreground');
    expect(html).not.toContain('v2board-auth-visual');
    expect(html).not.toContain('md:grid-cols-2');
    expect(html).not.toContain('tw:rounded-card');
    expect(html).not.toContain('block block-rounded block-transparent');
    expect(html).not.toContain('form-control form-control-alt');
    expect(html).not.toContain('btn btn-block btn-primary');
    expect(html).not.toContain('bg-gray-lighter');

    // New York-style block rhythm: top chrome belongs to AuthLayout; pages own only the card.
    expect(html).toContain('max-w-md');
    expect(html).toContain('class="v2board-auth-title');
    expect(html).toContain('>欢迎回来</h1>');
    expect(html).toContain('输入邮箱和密码继续');
    expect(html).not.toContain('>V2Board</h1>');
    expect(html).not.toContain('v2board-auth-shell-brand');
    expect(html).not.toContain('v2board-logo');
    expect(html).not.toContain('size-12');
    expect(html).not.toContain('block-title');

    // Shadcn-style examples: identifier fields may show examples, passwords stay label-only.
    expect(html).toContain('邮箱');
    expect(html).toContain('密码');
    expect(html).toContain('placeholder="m@example.com"');
    expect(html).not.toContain('placeholder="请输入密码"');

    // Password fields stay as plain masked inputs; the auth island avoids decorative icons.
    expect(html).toContain('type="password"');
    expect(html).not.toContain('aria-pressed');
    expect(html).toContain('登录');
    expect(html).toContain('注册');
    expect(html).toContain('忘记密码？');
    expect(html).toContain('还没有账号？');
    expect(html).not.toContain('v2board-auth-language-trigger');
    expect(source).toContain("from './auth-panel'");
    expect(panelSource).not.toContain("from './auth-language-menu'");
    expect(source).not.toContain("components/layout/auth-language-menu");
    expect(source).not.toContain("components/layout/language-menu");
  });

  it('ignores operator logo and description inside the action card while login is pending', () => {
    mocks.isPending = true;
    mocks.settings.description = 'Legacy description';
    mocks.settings.logo = '/theme/logo.png';

    const html = renderToStaticMarkup(<LoginPage />);

    expect(html).not.toContain('v2board-logo');
    expect(html).not.toContain('src="/theme/logo.png"');
    expect(html).toContain('>欢迎回来</h1>');
    expect(html).not.toContain('Legacy description');
    expect(html).toContain('disabled=""');
    // The base Button shows its own spinner alongside the still-visible label while pending.
    expect(html).toContain('animate-spin');
    expect(html).toContain('登录');
  });
});

describe('LoginPage bundled-theme behavior', () => {
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
    window.settings = undefined;
    window.g_lang = undefined;
  });

  async function renderLogin() {
    await act(async () => {
      root.render(<LoginPage />);
      await Promise.resolve();
    });
  }

  it('keeps the card focus order email → password → submit with no tabindex overrides', async () => {
    await renderLogin();

    const controls = Array.from(container.querySelectorAll<HTMLElement>('input, button'));
    const order = controls.map((element) => {
      if (element.tagName === 'INPUT') return element.getAttribute('type');
      return 'submit';
    });

    // The email field is a proper type="email"; the redesign-aware gate normalizes the identifier
    // input's type (email -> text) so this modernization is released, not pinned.
    expect(order).toEqual(['email', 'password', 'submit']);
    expect(controls.every((element) => !element.hasAttribute('tabindex'))).toBe(true);
  });

  it('submits the form values, stores auth data, fetches user info, and pushes the redirect', async () => {
    mocks.params = new URLSearchParams('redirect=order');
    await renderLogin();

    const [email, password] = Array.from(container.querySelectorAll('input'));
    setInputValue(email, 'user@example.com');
    setInputValue(password, 'secret');

    await act(async () => {
      container
        .querySelector('form')!
        .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.loginMutateAsync).toHaveBeenCalledWith({
      email: 'user@example.com',
      password: 'secret',
    });
    expect(mocks.setAuthData).toHaveBeenCalledWith('LOGIN_AUTH');
    expect(mocks.queryClient.fetchQuery).toHaveBeenCalledWith({
      queryFn: mocks.fetchUserInfo,
      queryKey: ['user', 'info'],
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/order');
    expect(mocks.navigate).not.toHaveBeenCalledWith('order');
  });

  it('falls back to dashboard for network-path redirect values', async () => {
    mocks.params = new URLSearchParams('redirect=//evil.example/path');
    await renderLogin();

    const [email, password] = Array.from(container.querySelectorAll('input'));
    setInputValue(email, 'user@example.com');
    setInputValue(password, 'secret');

    await act(async () => {
      container
        .querySelector('form')!
        .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.navigate).toHaveBeenCalledWith('/dashboard');
    expect(mocks.navigate).not.toHaveBeenCalledWith('//evil.example/path');
  });

  it('marks both fields invalid and ties them to the single alert when login fails', async () => {
    mocks.loginMutateAsync.mockRejectedValue(new mocks.ApiError(422, '邮箱或密码错误'));
    await renderLogin();

    const [email, password] = Array.from(container.querySelectorAll('input'));
    setInputValue(email, 'user@example.com');
    setInputValue(password, 'wrong');
    await act(async () => {
      container
        .querySelector('form')!
        .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });
    await flushPromises();

    const alert = container.querySelector('[role="alert"]')!;
    expect(alert.id).toBe('login-error');
    // The auth island keeps error feedback textual; no decorative alert icon is required.
    expect(alert.querySelector('svg')).toBeNull();
    expect(alert.textContent).toContain('邮箱或密码错误');
    // Both fields are programmatically invalid and described by the one alert box.
    for (const input of Array.from(container.querySelectorAll('input'))) {
      expect(input.getAttribute('aria-invalid')).toBe('true');
      expect(input.getAttribute('aria-describedby')).toBe('login-error');
    }
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('keeps the password field masked without rendering an icon reveal button', async () => {
    await renderLogin();

    const password = container.querySelector('input[name="password"]') as HTMLInputElement;

    expect(password.getAttribute('type')).toBe('password');
    expect(container.querySelector('button[aria-pressed]')).toBeNull();
    expect(container.querySelectorAll('button')).toHaveLength(1);
    expect(container.querySelector('.btn')).toBeNull();
    expect(mocks.loginMutateAsync).not.toHaveBeenCalled();
  });

  it('submits values through react-hook-form and zod, never via retired refs', () => {
    expect(controllerSource).toContain("from 'react-hook-form'");
    expect(controllerSource).toContain("from 'zod'");
    expect(controllerSource).toContain('zodResolver(loginSchema)');
    expect(controllerSource).toContain('form.handleSubmit(login)');
    // The request payload shape is unchanged — still exactly { email, password }.
    expect(controllerSource).toContain('mutateAsync({ email, password })');
    expect(controllerSource).not.toContain('new FormData');
    expect(controllerSource).not.toContain('emailRef');
    expect(controllerSource).not.toContain('passwordRef');
  });

  it('submits via a native <form>, retiring the global Enter-key keydown listener', () => {
    // Re-pin: the old window keydown(keyCode===13) shortcut is replaced by native form
    // submission (the browser submits on Enter from any field), so neither the page nor the
    // controller registers a global key listener.
    expect(panelSource).toContain('<form');
    expect(source).toContain('onSubmit={');
    expect(source).not.toContain('keyCode');
    expect(controllerSource).not.toContain("addEventListener('keydown'");
    expect(controllerSource).not.toContain('keyCode');
  });

  it('navigates register and forgetpassword via real hash anchors (no javascript: hrefs)', async () => {
    await renderLogin();

    const links = Array.from(container.querySelectorAll('a'));
    const register = links.find((link) => link.textContent === '注册')!;
    const forget = links.find((link) => link.textContent === '忘记密码？')!;

    // Real hash anchors — the data router navigates natively, with no JS click handler or
    // `javascript:` href. The behavior contract (lands on register/forgetpassword) is preserved.
    expect(register.getAttribute('href')).toBe('#/register');
    expect(forget.getAttribute('href')).toBe('#/forgetpassword');
    expect(register.getAttribute('href')).not.toContain('javascript:');
    expect(forget.getAttribute('href')).not.toContain('javascript:');
  });

  it('runs token2Login with the raw query redirect and pushes after setting auth data', async () => {
    mocks.params = new URLSearchParams('verify=verify-token&redirect=order');
    await renderLogin();
    await flushPromises();

    expect(mocks.tokenLoginMutateAsync).toHaveBeenCalledWith({
      redirect: 'order',
      verify: 'verify-token',
    });
    expect(mocks.setAuthData).toHaveBeenCalledWith('TOKEN_AUTH');
    expect(mocks.queryClient.fetchQuery).not.toHaveBeenCalled();
    expect(mocks.navigate).toHaveBeenCalledWith('/order');
    expect(mocks.navigate).not.toHaveBeenCalledWith('order');
  });

  it('ignores stale token2Login completions after the bootstrap effect is cleaned up', async () => {
    let resolveTokenLogin: ((value: { auth_data: string }) => void) | undefined;
    mocks.params = new URLSearchParams('verify=verify-token&redirect=order');
    mocks.tokenLoginMutateAsync.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTokenLogin = resolve;
        }),
    );

    await renderLogin();
    expect(mocks.tokenLoginMutateAsync).toHaveBeenCalledWith({
      redirect: 'order',
      verify: 'verify-token',
    });

    mocks.params = new URLSearchParams();
    await act(async () => {
      root.render(<LoginPage />);
      await Promise.resolve();
    });

    await act(async () => {
      resolveTokenLogin?.({ auth_data: 'STALE_AUTH' });
      await Promise.resolve();
    });

    expect(mocks.setAuthData).not.toHaveBeenCalledWith('STALE_AUTH');
    expect(mocks.navigate).not.toHaveBeenCalledWith('/order');
  });

  it('guards token2Login and checkLogin completions after bootstrap cleanup', () => {
    expect(controllerSource).toContain('const finishLogin = (authData: string) => {');
    expect(controllerSource).toContain('let active = true;');
    expect(controllerSource).toContain('if (!active) return;');
    expect(controllerSource).toContain('setAuthData(authData);');
    expect(controllerSource).toContain('navigate(redirect);');
    expect(controllerSource).toContain('user.checkLogin(apiClient)');
    expect(controllerSource).toContain('if (active && result.is_login)');
    expect(controllerSource).toContain('active = false;');
  });

  it('keeps the original checkLogin effect auth-data guard before requesting /user/checkLogin', async () => {
    await renderLogin();
    await flushPromises();

    expect(mocks.getAuthData).toHaveBeenCalled();
    expect(mocks.checkLogin).not.toHaveBeenCalled();
    expect(mocks.queryClient.fetchQuery).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('checks an existing auth session on mount, fetches user info, and pushes dashboard', async () => {
    mocks.getAuthData.mockReturnValue('EXISTING_AUTH');
    mocks.checkLogin.mockResolvedValue({ is_login: true });

    await renderLogin();
    await flushPromises();

    expect(mocks.checkLogin).toHaveBeenCalledWith(mocks.apiClient);
    expect(mocks.queryClient.fetchQuery).toHaveBeenCalledWith({
      queryFn: mocks.fetchUserInfo,
      queryKey: ['user', 'info'],
    });
    expect(mocks.navigate).toHaveBeenCalledWith('/dashboard');
    expect(mocks.navigate).not.toHaveBeenCalledWith('dashboard');
  });
});
