import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LoginPage from './login';

const source = readFileSync(`${process.cwd()}/src/pages/auth/login.tsx`, 'utf8');
const controllerSource = readFileSync(
  `${process.cwd()}/src/pages/auth/use-login-controller.ts`,
  'utf8',
);

const mocks = vi.hoisted(() => ({
  apiClient: { name: 'apiClient' },
  checkLogin: vi.fn(),
  fetchUserInfo: vi.fn(),
  getAuthData: vi.fn(),
  isPending: false,
  labels: {
    'auth.email': '邮箱',
    'auth.forget_password': '忘记密码',
    'auth.password': '密码',
    'auth.sign_up': '注册',
    'auth.submit_login': '登入',
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

vi.mock('react-router-dom', () => ({
  useNavigate: () => mocks.navigate,
  useSearchParams: () => [mocks.params],
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => mocks.queryClient,
}));

vi.mock('@v2board/api-client', () => ({
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
  getLegacyDescription: () => mocks.settings.description,
  getLegacyLogo: () => mocks.settings.logo,
  getLegacyTitle: () => mocks.settings.title,
}));

vi.mock('@/lib/queries', () => ({
  fetchUserInfo: mocks.fetchUserInfo,
  userKeys: {
    info: ['user', 'info'],
  },
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
  mocks.tokenLoginMutateAsync.mockReset();
  mocks.tokenLoginMutateAsync.mockResolvedValue({ auth_data: 'TOKEN_AUTH' });
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('LoginPage modern markup', () => {
  beforeEach(resetMocks);

  it('renders the modern login card, translated placeholders, footer links, and language trigger', () => {
    const html = renderToStaticMarkup(<LoginPage />);

    // Reskinned shell — the legacy OneUI/Bootstrap markup is retired. Tailwind
    // utilities carry the `tw:` prefix so they never collide with the globally
    // bundled vendored `.block`/`.container`/etc. legacy classes.
    expect(html).toContain('tw:rounded-card');
    // Token-driven primary + input surfaces (from @v2board/tokens) — never hardcoded blue/slate
    // or a bespoke arbitrary shadow.
    expect(html).toContain('tw:bg-primary');
    expect(html).toContain('tw:border-input');
    expect(html).toContain('tw:text-foreground');
    expect(html).not.toContain('tw:bg-blue-600');
    expect(html).not.toContain('tw:border-slate-300');
    expect(html).not.toContain('tw:shadow-[');
    // The display utility must stay prefixed (`tw:block`); a bare `block` class would
    // collide with vendored OneUI `.block` cards and inherit their shadow/background.
    expect(html).toContain('tw:block');
    // A bare `block` token (preceded by quote/space, not the `tw:` prefix, and not
    // part of `block-rounded`) must never appear — that is the OneUI card collision.
    expect(html).not.toMatch(/[\s"]block[\s"]/);
    expect(html).not.toContain('block block-rounded block-transparent');
    expect(html).not.toContain('form-control form-control-alt');
    expect(html).not.toContain('btn btn-block btn-primary');
    expect(html).not.toContain('bg-gray-lighter');

    // The heading must stay an <h2> (never <h1>/.block-title) so the
    // user-login-language-persistence interaction's titleText stays '' versus the oracle.
    expect(html).toContain('>V2Board</h2>');
    expect(html).not.toContain('<h1');
    expect(html).not.toContain('block-title');

    // Label-only fields — the placeholder no longer duplicates the visible label.
    expect(html).toContain('邮箱');
    expect(html).toContain('密码');
    expect(html).not.toContain('placeholder=');
    expect(html).toContain('登入');
    expect(html).toContain('注册');
    expect(html).toContain('忘记密码');
    expect(html).toContain('class="v2board-login-i18n-btn"');
    expect(html).toContain('简体中文');
  });

  it('renders the operator logo + description and the antd loading icon while login is pending', () => {
    mocks.isPending = true;
    mocks.settings.description = 'Legacy description';
    mocks.settings.logo = '/theme/logo.png';

    const html = renderToStaticMarkup(<LoginPage />);

    expect(html).toContain('v2board-logo');
    expect(html).toContain('src="/theme/logo.png"');
    expect(html).toContain('alt="V2Board"');
    expect(html).toContain('Legacy description');
    expect(html).toContain('disabled=""');
    // The base Button shows its own spinner alongside the still-visible label while pending.
    expect(html).toContain('tw:animate-spin');
    expect(html).toContain('登入');
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
  });

  async function renderLogin() {
    await act(async () => {
      root.render(<LoginPage />);
      await Promise.resolve();
    });
  }

  it('keeps focus order email → password → submit with no tabindex overrides (re-pins retired tab-focus parity)', async () => {
    await renderLogin();

    const controls = Array.from(container.querySelectorAll<HTMLElement>('input, button'));
    const order = controls.map((element) =>
      element.tagName === 'INPUT' ? element.getAttribute('type') : 'submit',
    );

    // The email field stays type="text" (not "email"): user-home-root-page-state captures input
    // type and compares it to the oracle, which used "text".
    expect(order).toEqual(['text', 'password', 'submit']);
    expect(controls.every((element) => !element.hasAttribute('tabindex'))).toBe(true);
  });

  it('submits the form values, stores auth data, fetches user info, and pushes the redirect', async () => {
    mocks.params = new URLSearchParams('redirect=order');
    await renderLogin();

    const [email, password] = Array.from(container.querySelectorAll('input'));
    email!.value = 'user@example.com';
    password!.value = 'secret';

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

  it('reads the submitted values from the native form, never via the retired refs', () => {
    expect(controllerSource).toContain('new FormData(event.currentTarget)');
    expect(controllerSource).toContain("form.get('email')");
    expect(controllerSource).toContain("form.get('password')");
    // The request payload shape is unchanged — still exactly { email, password }.
    expect(controllerSource).toContain('mutateAsync({ email, password })');
    expect(controllerSource).not.toContain('emailRef');
    expect(controllerSource).not.toContain('passwordRef');
  });

  it('submits via a native <form>, retiring the global Enter-key keydown listener', () => {
    // Re-pin: the old window keydown(keyCode===13) shortcut is replaced by native form
    // submission (the browser submits on Enter from any field), so neither the page nor the
    // controller registers a global key listener.
    expect(source).toContain('<form');
    expect(source).toContain('onSubmit={');
    expect(source).not.toContain('keyCode');
    expect(controllerSource).not.toContain("addEventListener('keydown'");
    expect(controllerSource).not.toContain('keyCode');
  });

  it('navigates register and forgetpassword via real HashRouter anchors (no javascript: hrefs)', async () => {
    await renderLogin();

    const links = Array.from(container.querySelectorAll('a'));
    const register = links.find((link) => link.textContent === '注册')!;
    const forget = links.find((link) => link.textContent === '忘记密码')!;

    // Real hash anchors — HashRouter navigates natively, with no JS click handler or
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

  it('keeps token2Login and checkLogin uncancelled like the old login component', () => {
    // The bootstrap effect moved intact into useLoginController; it stays uncancelled (no cleanup
    // flag) so it matches the old component's fire-and-forget behavior exactly.
    expect(controllerSource).toContain('const finishLogin = (authData: string) => {');
    expect(controllerSource).toContain('setAuthData(authData);');
    expect(controllerSource).toContain('navigate(redirect);');
    expect(controllerSource).toContain('user.checkLogin(apiClient)');
    expect(controllerSource).not.toContain('cancelled');
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
