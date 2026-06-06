import { act } from 'react';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LoginPage from './login';

const loginSource = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), 'login.tsx'),
  'utf8',
);

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  passportLogin: vi.fn(),
  passportToken2Login: vi.fn(),
  userCheckLogin: vi.fn(),
  userInfo: vi.fn(),
  searchParams: new URLSearchParams(),
}));

vi.mock('react-router-dom', () => ({
  useNavigate: () => mocks.navigate,
  useSearchParams: () => [mocks.searchParams],
}));

vi.mock('antd', () => ({
  App: {
    useApp: () => ({
      message: { error: vi.fn() },
      modal: { info: vi.fn() },
    }),
  },
}));

vi.mock('@/lib/api', () => ({
  apiClient: {},
}));

vi.mock('@v2board/api-client', () => ({
  passport: { login: mocks.passportLogin, token2Login: mocks.passportToken2Login },
  user: {
    checkLogin: mocks.userCheckLogin,
    info: mocks.userInfo,
  },
}));

(
  globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

describe('Admin LoginPage legacy behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.navigate.mockReset();
    mocks.passportLogin.mockReset();
    mocks.passportToken2Login.mockReset();
    mocks.userCheckLogin.mockReset();
    mocks.userInfo.mockReset();
    mocks.searchParams = new URLSearchParams();
    localStorage.clear();
    window.settings = {
      title: 'V2Board',
      logo: '',
      background_url: '/bg.jpg',
      secure_path: 'admin',
    };
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
    localStorage.clear();
    window.settings = undefined;
  });

  it('matches the original admin login shell and does not render secure path input', () => {
    const html = renderToStaticMarkup(<LoginPage />);

    expect(html).toContain('id="page-container"');
    expect(html).toContain('class="no-gutters v2board-auth-box"');
    expect(html).toContain('登录到管理中心');
    expect(html).toContain('placeholder="邮箱"');
    expect(html).toContain('placeholder="密码"');
    expect(html).toContain('si si-login mr-1');
    expect(html).toContain('忘记密码');
    expect(html).toContain('<a>忘记密码</a>');
    expect(html).not.toContain('secure_path');
    expect(html).not.toContain('Secure path');
  });

  it('uses the bundled admin background_url short-circuit expression', () => {
    expect(loginSource).toContain(
      'const legacyBackgroundImage = (backgroundUrl && `url(${backgroundUrl})`) as string;',
    );
    expect(loginSource).not.toContain(
      'backgroundImage: backgroundUrl ? `url(${backgroundUrl})` : undefined',
    );
  });

  it('keeps the original forgot-password modal options', () => {
    expect(loginSource).toContain("title: '忘记密码'");
    expect(loginSource).toContain("okText: '我知道了'");
    expect(loginSource).toContain('onOk() {}');
  });

  it('clears the legacy login loading state before saving auth and navigating', () => {
    const loginRequest = loginSource.indexOf('const result = await passport.login');
    const clearLoading = loginSource.indexOf('setSubmitting(false);', loginRequest);
    const saveAuth = loginSource.indexOf('setAuthData(result.auth_data);');
    const dashboardPush = loginSource.indexOf("navigate('/dashboard');");

    expect(loginRequest).toBeGreaterThan(-1);
    expect(clearLoading).toBeGreaterThan(loginRequest);
    expect(saveAuth).toBeGreaterThan(clearLoading);
    expect(dashboardPush).toBeGreaterThan(saveAuth);
    expect(loginSource).not.toContain('} finally {');
  });

  it('uses the shared old Ant Design loading icon for the submit button', () => {
    expect(loginSource).toContain(
      "import { LegacyLoadingIcon } from '@/components/legacy-ant-icon';",
    );
    expect(loginSource).toContain('<LegacyLoadingIcon />');
    expect(loginSource).not.toContain('function LegacyLoadingIcon()');
  });

  it('keeps the original admin login submit values as direct ref reads', () => {
    expect(loginSource).toContain('email: emailRef.current!.value');
    expect(loginSource).toContain('password: passwordRef.current!.value');
    expect(loginSource).not.toContain("emailRef.current?.value ?? ''");
    expect(loginSource).not.toContain("passwordRef.current?.value ?? ''");
  });

  it('pushes dashboard after admin login instead of replacing browser history', async () => {
    mocks.passportLogin.mockResolvedValue({ token: 't', is_admin: 1, auth_data: 'jwt' });
    mocks.userInfo.mockResolvedValue({});

    await act(async () => {
      root.render(<LoginPage />);
    });

    const inputs = container.querySelectorAll('input');
    inputs[0]!.value = 'admin@example.com';
    inputs[1]!.value = 'password';

    await act(async () => {
      container.querySelector('button')!.click();
    });

    expect(mocks.passportLogin).toHaveBeenCalledWith(
      {},
      {
        email: 'admin@example.com',
        password: 'password',
      },
    );
    expect(localStorage.getItem('authorization')).toBe('jwt');
    expect(mocks.navigate).toHaveBeenCalledWith('/dashboard');
    expect(mocks.navigate).not.toHaveBeenCalledWith('/dashboard', { replace: true });
  });

  it('pushes the redirect target for an existing admin session like the legacy checkLogin effect', async () => {
    localStorage.setItem('authorization', 'jwt');
    mocks.searchParams = new URLSearchParams('redirect=/order');
    mocks.userCheckLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({});

    await act(async () => {
      root.render(<LoginPage />);
    });

    expect(mocks.userCheckLogin).toHaveBeenCalledWith({});
    expect(mocks.userInfo).toHaveBeenCalledTimes(1);
    expect(mocks.userInfo.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.navigate.mock.invocationCallOrder[0]!,
    );
    expect(mocks.navigate).toHaveBeenCalledWith('/order');
    expect(mocks.navigate).not.toHaveBeenCalledWith('/order', { replace: true });
  });

  it('uses an absolute dashboard target for an existing admin session without redirect', async () => {
    localStorage.setItem('authorization', 'jwt');
    mocks.userCheckLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({});

    await act(async () => {
      root.render(<LoginPage />);
    });

    expect(mocks.userInfo).toHaveBeenCalledTimes(1);
    expect(mocks.userInfo.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.navigate.mock.invocationCallOrder[0]!,
    );
    expect(mocks.navigate).toHaveBeenCalledWith('/dashboard');
    expect(mocks.navigate).not.toHaveBeenCalledWith('dashboard');
  });

  it('normalizes bare redirect targets before navigating from login', async () => {
    localStorage.setItem('authorization', 'jwt');
    mocks.searchParams = new URLSearchParams('redirect=order');
    mocks.userCheckLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({});

    await act(async () => {
      root.render(<LoginPage />);
    });

    expect(mocks.navigate).toHaveBeenCalledWith('/order');
    expect(mocks.navigate).not.toHaveBeenCalledWith('order');
  });

  it('keeps the old checkLogin effect uncancelled after login page unmount', () => {
    const checkLoginBlock = loginSource.slice(
      loginSource.indexOf('useEffect(() => {'),
      loginSource.indexOf('useEffect(() => {', loginSource.indexOf('useEffect(() => {') + 1),
    );

    expect(checkLoginBlock).toContain('if (getAuthData()) {');
    expect(checkLoginBlock).toContain('.checkLogin(apiClient)');
    expect(checkLoginBlock).toContain('navigate(redirect);');
    expect(checkLoginBlock).not.toContain('cancelled');
  });

  it('keeps the bundled admin verify parameter inert because the old passport model has no token2Login effect', async () => {
    mocks.searchParams = new URLSearchParams('verify=abc&redirect=/ticket');
    mocks.passportToken2Login.mockResolvedValue({
      token: 't',
      is_admin: 1,
      auth_data: 'quick-jwt',
    });

    await act(async () => {
      root.render(<LoginPage />);
    });

    expect(mocks.passportToken2Login).not.toHaveBeenCalled();
    expect(mocks.userCheckLogin).not.toHaveBeenCalled();
    expect(localStorage.getItem('authorization')).toBeNull();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });
});
