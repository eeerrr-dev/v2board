import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LoginPage from './login';

// The admin login is a redesigned shadcn island: the legacy Bootstrap/OneUI DOM
// byte-pins (#page-container, si si-login, legacyInfo modal, ref-read source
// strings) are retired. What remains covered is the Tier-1 auth contract — the
// passport.login payload, auth_data persistence, the is_admin gate, and the
// existing-session redirect navigation, all pinned as behavior.

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  passportLogin: vi.fn(),
  passportToken2Login: vi.fn(),
  userCheckLogin: vi.fn(),
  userInfo: vi.fn(),
  searchParams: new URLSearchParams(),
}));

vi.mock('react-router', () => ({
  useNavigate: () => mocks.navigate,
  useSearchParams: () => [mocks.searchParams],
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

describe('Admin LoginPage', () => {
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
  });

  afterEach(() => {
    localStorage.clear();
    window.settings = undefined;
  });

  it('renders the admin sign-in copy and no secure-path field', () => {
    render(<LoginPage />);

    expect(screen.getByText('登录到管理中心')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('邮箱')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('密码')).toBeInTheDocument();
    expect(screen.queryByText(/secure_path/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Secure path/i)).not.toBeInTheDocument();
  });

  it('logs an admin in with the passport payload, persists auth_data, and enters the console', async () => {
    const user = userEvent.setup();
    mocks.passportLogin.mockResolvedValue({ token: 't', is_admin: 1, auth_data: 'jwt' });
    mocks.userInfo.mockResolvedValue({});

    render(<LoginPage />);
    await user.type(screen.getByPlaceholderText('邮箱'), 'admin@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    await waitFor(() =>
      expect(mocks.passportLogin).toHaveBeenCalledWith(
        {},
        { email: 'admin@example.com', password: 'password' },
      ),
    );
    expect(localStorage.getItem('authorization')).toBe('jwt');
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/dashboard'));
    expect(mocks.navigate).not.toHaveBeenCalledWith('/dashboard', { replace: true });
    expect(mocks.userInfo).toHaveBeenCalledTimes(1);
  });

  it('keeps a non-admin on the login screen while still saving the returned session', async () => {
    const user = userEvent.setup();
    mocks.passportLogin.mockResolvedValue({ token: 't', is_admin: 0, auth_data: 'jwt' });

    render(<LoginPage />);
    await user.type(screen.getByPlaceholderText('邮箱'), 'user@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    await waitFor(() => expect(mocks.passportLogin).toHaveBeenCalledTimes(1));
    expect(localStorage.getItem('authorization')).toBe('jwt');
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('resumes an existing admin session at the redirect target after warming user.info', async () => {
    localStorage.setItem('authorization', 'jwt');
    mocks.searchParams = new URLSearchParams('redirect=/order');
    mocks.userCheckLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({});

    render(<LoginPage />);

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order'));
    expect(mocks.userCheckLogin).toHaveBeenCalledWith({});
    expect(mocks.userInfo).toHaveBeenCalledTimes(1);
    expect(mocks.userInfo.mock.invocationCallOrder[0]!).toBeLessThan(
      mocks.navigate.mock.invocationCallOrder[0]!,
    );
    expect(mocks.navigate).not.toHaveBeenCalledWith('/order', { replace: true });
  });

  it('defaults an existing admin session to the bare dashboard target with no redirect', async () => {
    localStorage.setItem('authorization', 'jwt');
    mocks.userCheckLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({});

    render(<LoginPage />);

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('dashboard'));
    expect(mocks.navigate).not.toHaveBeenCalledWith('/dashboard');
  });

  it('passes a bare redirect target through unchanged', async () => {
    localStorage.setItem('authorization', 'jwt');
    mocks.searchParams = new URLSearchParams('redirect=order');
    mocks.userCheckLogin.mockResolvedValue({ is_login: true, is_admin: true });
    mocks.userInfo.mockResolvedValue({});

    render(<LoginPage />);

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('order'));
    expect(mocks.navigate).not.toHaveBeenCalledWith('/order');
  });

  it('leaves a ?verify= parameter inert (the admin passport model has no token2Login effect)', async () => {
    mocks.searchParams = new URLSearchParams('verify=abc&redirect=/ticket');
    mocks.passportToken2Login.mockResolvedValue({ token: 't', is_admin: 1, auth_data: 'quick-jwt' });

    render(<LoginPage />);

    await Promise.resolve();
    expect(mocks.passportToken2Login).not.toHaveBeenCalled();
    expect(mocks.userCheckLogin).not.toHaveBeenCalled();
    expect(localStorage.getItem('authorization')).toBeNull();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('shows the artisan reset command in the forgot-password dialog', async () => {
    const user = userEvent.setup();
    render(<LoginPage />);

    await user.click(screen.getByRole('button', { name: '忘记密码' }));

    expect(await screen.findByText('在站点目录下执行命令找回密码')).toBeInTheDocument();
    expect(screen.getByText('php artisan reset:password 管理员邮箱')).toBeInTheDocument();
  });
});
