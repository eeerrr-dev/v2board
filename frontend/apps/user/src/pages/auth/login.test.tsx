import { readFileSync } from 'node:fs';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LoginPage from './login';

const source = readFileSync(`${process.cwd()}/src/pages/auth/login.tsx`, 'utf8');

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

describe('LoginPage bundled-theme markup', () => {
  beforeEach(resetMocks);

  it('renders the original login card, translated placeholders, footer links, and language trigger', () => {
    const html = renderToStaticMarkup(<LoginPage />);

    expect(html).toContain(
      'block block-rounded block-transparent block-fx-pop w-100 mb-0 overflow-hidden bg-image',
    );
    expect(html).toContain('box-shadow:0 0.5rem 2rem #0000000d');
    expect(html).toContain('class="row no-gutters"');
    expect(html).toContain('class="col-md-12 order-md-1 bg-white"');
    expect(html).toContain('class="block-content block-content-full px-lg-4 py-md-4 py-lg-4"');
    expect(html).toContain('<span class="text-dark">V2Board</span>');
    expect(html).toContain('placeholder="邮箱"');
    expect(html).toContain('placeholder="密码"');
    expect(html).toContain('class="si si-login mr-1"');
    expect(html).toContain('登入');
    expect(html).toContain('class="text-left bg-gray-lighter p-3 px-4"');
    expect(html).toContain('注册');
    expect(html).toContain('class="ant-divider ant-divider-vertical"');
    expect(html).toContain('忘记密码');
    expect(html).toContain('class="v2board-login-i18n-btn"');
    expect(html).toContain('class="si si-globe pr-1"');
    expect(html).toContain('简体中文');
  });

  it('renders the legacy logo, description, and antd loading icon while login is pending', () => {
    mocks.isPending = true;
    mocks.settings.description = 'Legacy description';
    mocks.settings.logo = '/theme/logo.png';

    const html = renderToStaticMarkup(<LoginPage />);

    expect(html).toContain('class="v2board-logo mb-3"');
    expect(html).toContain('src="/theme/logo.png"');
    expect(html).toContain('class="font-size-sm text-muted mb-3"');
    expect(html).toContain('Legacy description');
    expect(html).toContain('disabled=""');
    expect(html).toContain('class="anticon anticon-loading"');
    expect(html).not.toContain('class="si si-login mr-1"');
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

  it('submits ref values, stores auth data, fetches user info, and pushes the redirect', async () => {
    mocks.params = new URLSearchParams('redirect=order');
    await renderLogin();

    const [email, password] = Array.from(container.querySelectorAll('input'));
    email!.value = 'user@example.com';
    password!.value = 'secret';

    await act(async () => {
      container.querySelector('button')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
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

  it('keeps the original login submit values as direct ref reads', () => {
    expect(source).toContain('email: emailRef.current!.value');
    expect(source).toContain('password: passwordRef.current!.value');
    expect(source).not.toContain("emailRef.current?.value ?? ''");
    expect(source).not.toContain("passwordRef.current?.value ?? ''");
  });

  it('uses the original global Enter-key login shortcut', async () => {
    await renderLogin();

    const [email, password] = Array.from(container.querySelectorAll('input'));
    email!.value = 'enter@example.com';
    password!.value = 'keyboard';

    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, keyCode: 13 }));
      await Promise.resolve();
    });
    await flushPromises();

    expect(mocks.loginMutateAsync).toHaveBeenCalledWith({
      email: 'enter@example.com',
      password: 'keyboard',
    });
  });

  it('keeps the footer links as javascript-style anchors that push register and forgetpassword', async () => {
    await renderLogin();

    const links = Array.from(container.querySelectorAll('a'));
    const register = links.find((link) => link.textContent === '注册')!;
    const forget = links.find((link) => link.textContent === '忘记密码')!;

    expect(register.getAttribute('href')).toBe('javascript:void(0);');
    expect(forget.getAttribute('href')).toBe('javascript:void(0);');

    await act(async () => {
      register.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      forget.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(mocks.navigate).toHaveBeenNthCalledWith(1, '/register');
    expect(mocks.navigate).toHaveBeenNthCalledWith(2, '/forgetpassword');
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
    const authEffectBlock = source.slice(
      source.indexOf('useEffect(() => {'),
      source.indexOf('useEffect(() => {', source.indexOf('useEffect(() => {') + 1),
    );

    expect(authEffectBlock).toContain('const finishLogin = (authData: string) => {');
    expect(authEffectBlock).toContain('setAuthData(authData);');
    expect(authEffectBlock).toContain('navigate(redirect);');
    expect(authEffectBlock).toContain('user.checkLogin(apiClient)');
    expect(authEffectBlock).not.toContain('cancelled');
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
