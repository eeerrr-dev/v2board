import { screen, waitFor } from '@testing-library/react';
import { ApiProblemError } from '@v2board/api-client';
import type * as ApiClientModule from '@v2board/api-client';
import type * as ReactRouterModule from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { adminSessionKeys } from '@/lib/session-queries';
import { renderWithProviders } from '@/test/render';
import { setAdminRuntimeConfig } from '@/test/runtime-config';
import LoginPage from './login';

// The admin login is a redesigned shadcn island: the legacy Bootstrap/OneUI DOM
// byte-pins (#page-container, si si-login, legacyInfo modal, ref-read source
// strings) are retired. What remains covered is the Tier-1 auth contract — the
// passport.login payload, auth_data persistence, the is_admin gate, and the
// post-login redirect navigation, all pinned as behavior. Existing-session
// probing belongs to the data-router loader and is covered in App.test.tsx.

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  passportLogin: vi.fn(),
  passportTokenLogin: vi.fn(),
  userCheckLogin: vi.fn(),
  userInfo: vi.fn(),
  toastError: vi.fn(),
  redirectTarget: '/dashboard',
}));

vi.mock('react-router', async (importOriginal) => {
  const actual = await importOriginal<typeof ReactRouterModule>();
  return {
    ...actual,
    useNavigate: () => mocks.navigate,
    useLoaderData: () => ({ redirectTarget: mocks.redirectTarget }),
  };
});

vi.mock('@/lib/api', () => ({
  apiClient: {},
}));

vi.mock('@/lib/toast', () => ({
  toast: { error: mocks.toastError },
}));

vi.mock('@v2board/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof ApiClientModule>();
  return {
    ...actual,
    passport: {
      ...actual.passport,
      login: mocks.passportLogin,
      tokenLogin: mocks.passportTokenLogin,
    },
    user: {
      ...actual.user,
      checkLogin: mocks.userCheckLogin,
      info: mocks.userInfo,
    },
  };
});

function renderLogin() {
  return renderWithProviders(<LoginPage />, { queryClient: true });
}

describe('Admin LoginPage', () => {
  beforeEach(() => {
    mocks.navigate.mockReset();
    mocks.passportLogin.mockReset();
    mocks.passportTokenLogin.mockReset();
    mocks.userCheckLogin.mockReset();
    mocks.userInfo.mockReset();
    mocks.toastError.mockReset();
    mocks.redirectTarget = '/dashboard';
    localStorage.clear();
    setAdminRuntimeConfig({
      title: 'V2Board',
      logo: '',
      background_url: '/bg.jpg',
      secure_path: 'admin',
    });
  });

  afterEach(() => {
    localStorage.clear();
    setAdminRuntimeConfig();
  });

  it('renders the login surface through an owned test id', () => {
    const { container } = renderLogin();
    expect(screen.getByTestId('admin-login-surface')).toBeInTheDocument();
    const background = container.querySelector('img[src="/bg.jpg"]');
    expect(background).toHaveAttribute('aria-hidden', 'true');
    expect(background).toHaveAttribute('decoding', 'async');
    expect(background).toHaveAttribute('fetchpriority', 'high');
  });

  it('renders an operator logo with asynchronous decoding', () => {
    setAdminRuntimeConfig({
      title: 'V2Board',
      logo: '/logo.svg',
      background_url: '',
      secure_path: 'admin',
    });

    const { container } = renderLogin();

    expect(container.querySelector('img[src="/logo.svg"]')).toHaveAttribute('decoding', 'async');
  });

  it('renders the admin sign-in copy and no secure-path field', () => {
    renderLogin();

    expect(screen.getByText('登录到管理中心')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('邮箱')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('密码')).toBeInTheDocument();
    expect(screen.queryByText(/secure_path/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Secure path/i)).not.toBeInTheDocument();
  });

  it('associates resolver errors with the labelled login controls', async () => {
    const { user } = renderLogin();

    await user.click(screen.getByTestId('admin-login-submit'));

    const email = screen.getByRole('textbox', { name: '邮箱' });
    const password = screen.getByLabelText('密码');
    expect(await screen.findByText('请输入邮箱')).toBeInTheDocument();
    expect(screen.getByText('密码至少需要 8 个字符')).toBeInTheDocument();
    expect(email).toHaveAttribute('aria-invalid', 'true');
    expect(email).toHaveAccessibleDescription('请输入邮箱');
    expect(password).toHaveAttribute('aria-invalid', 'true');
    expect(password).toHaveAccessibleDescription('密码至少需要 8 个字符');
    expect(mocks.passportLogin).not.toHaveBeenCalled();
  });

  it.each([
    ['not-an-email', 'password', '请输入有效邮箱'],
    ['admin@example.com', 'short', '密码至少需要 8 个字符'],
    ['admin@example.com', '😀😀😀😀', '密码至少需要 8 个字符'],
  ])('rejects invalid AuthLogin input without a request', async (email, password, message) => {
    const { user } = renderLogin();

    await user.type(screen.getByPlaceholderText('邮箱'), email);
    await user.type(screen.getByPlaceholderText('密码'), password);
    await user.click(screen.getByTestId('admin-login-submit'));

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(mocks.passportLogin).not.toHaveBeenCalled();
  });

  it('logs an admin in with the passport payload, persists auth_data, and enters the console', async () => {
    mocks.redirectTarget = '/order';
    mocks.passportLogin.mockResolvedValue({ is_admin: true, auth_data: 'jwt' });
    mocks.userInfo.mockResolvedValue({ email: 'admin@example.com' });

    const { user, queryClient } = renderLogin();
    await user.type(screen.getByPlaceholderText('邮箱'), '  admin@example.com  ');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    await waitFor(() =>
      expect(mocks.passportLogin).toHaveBeenCalledWith(
        {},
        { email: 'admin@example.com', password: 'password' },
      ),
    );
    expect(localStorage.getItem('authorization')).toBe('jwt');
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order', { replace: true }));
    await waitFor(() => expect(mocks.userInfo).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(queryClient?.getQueryData(adminSessionKeys.userInfo)).toEqual({
        email: 'admin@example.com',
      }),
    );
  });

  it('rejects a non-admin session without persisting its credential', async () => {
    // §6.12: a non-admin login answers with a session probe; an ordinary user
    // (no staff pair) is denied and the transient credential is destroyed.
    mocks.passportLogin.mockResolvedValue({ is_admin: false, auth_data: 'jwt' });
    mocks.userCheckLogin.mockResolvedValue({ is_login: true });

    const { user } = renderLogin();
    await user.type(screen.getByPlaceholderText('邮箱'), 'user@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledTimes(1));
    expect(localStorage.getItem('authorization')).toBeNull();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('logs a staff account in through the session probe and lands on a granted route', async () => {
    // §6.12: the login body only carries `is_admin`, so a staff account proves
    // its grants via the session probe. The /dashboard target maps to the
    // ungranted stats family — the landing falls to the first granted route.
    mocks.passportLogin.mockResolvedValue({ is_admin: false, auth_data: 'staff-jwt' });
    mocks.userCheckLogin.mockResolvedValue({
      is_login: true,
      is_staff: true,
      admin_permissions: ['tickets:write', 'users:read'],
    });
    mocks.userInfo.mockResolvedValue({ email: 'staff@example.com' });

    const { user } = renderLogin();
    await user.type(screen.getByPlaceholderText('邮箱'), 'staff@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/user', { replace: true }));
    expect(localStorage.getItem('authorization')).toBe('staff-jwt');
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it('delegates a login mutation failure to the shared MutationCache presenter', async () => {
    mocks.passportLogin.mockRejectedValue({ status: 500, message: '登录失败' });
    const { user } = renderLogin();

    await user.type(screen.getByPlaceholderText('邮箱'), 'admin@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    await waitFor(() => expect(mocks.passportLogin).toHaveBeenCalledOnce());
    expect(mocks.toastError).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('does not add a local toast for a 403 authorization verdict (MutationCache owns it)', async () => {
    mocks.passportLogin.mockRejectedValue({ status: 403, message: '登录已过期' });
    const { user } = renderLogin();

    await user.type(screen.getByPlaceholderText('邮箱'), 'admin@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    await waitFor(() => expect(mocks.passportLogin).toHaveBeenCalledOnce());
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it('does not probe an existing session from a component effect', async () => {
    localStorage.setItem('authorization', 'jwt');
    renderLogin();

    await Promise.resolve();
    expect(mocks.userCheckLogin).not.toHaveBeenCalled();
    expect(mocks.userInfo).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('leaves a ?verify= parameter inert (the admin passport model has no tokenLogin effect)', async () => {
    mocks.redirectTarget = '/ticket';
    mocks.passportTokenLogin.mockResolvedValue({
      is_admin: true,
      auth_data: 'quick-jwt',
    });

    renderLogin();

    await Promise.resolve();
    expect(mocks.passportTokenLogin).not.toHaveBeenCalled();
    expect(mocks.userCheckLogin).not.toHaveBeenCalled();
    expect(localStorage.getItem('authorization')).toBeNull();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('runs the §6.10 two-phase TOTP login: reveal the code field, then resubmit with totp_code', async () => {
    const mfaRequired = new ApiProblemError(401, {
      type: 'about:blank',
      title: 'Unauthorized',
      status: 401,
      code: 'mfa_code_required',
      detail: '需要两步验证码',
    });
    mocks.passportLogin
      .mockRejectedValueOnce(mfaRequired)
      .mockResolvedValueOnce({ is_admin: true, auth_data: 'jwt' });
    mocks.userInfo.mockResolvedValue({ email: 'admin@example.com' });

    const { user } = renderLogin();
    await user.type(screen.getByPlaceholderText('邮箱'), 'admin@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    // Phase 1: the plain payload carries no totp_code and reveals the field.
    await waitFor(() =>
      expect(mocks.passportLogin).toHaveBeenCalledWith(
        {},
        { email: 'admin@example.com', password: 'password' },
      ),
    );
    const totp = await screen.findByTestId('admin-login-totp');
    expect(localStorage.getItem('authorization')).toBeNull();

    // Phase 2: the retry includes the entered code and completes the login.
    await user.type(totp, '123456');
    await user.click(screen.getByTestId('admin-login-submit'));
    await waitFor(() =>
      expect(mocks.passportLogin).toHaveBeenLastCalledWith(
        {},
        { email: 'admin@example.com', password: 'password', totp_code: '123456' },
      ),
    );
    expect(localStorage.getItem('authorization')).toBe('jwt');
    await waitFor(() =>
      expect(mocks.navigate).toHaveBeenCalledWith('/dashboard', { replace: true }),
    );
  });

  it('surfaces a wrong TOTP code inline on the code field without tearing the form down', async () => {
    const invalid = new ApiProblemError(401, {
      type: 'about:blank',
      title: 'Unauthorized',
      status: 401,
      code: 'mfa_code_invalid',
      detail: '验证码错误',
    });
    mocks.passportLogin.mockRejectedValue(invalid);

    const { user } = renderLogin();
    await user.type(screen.getByPlaceholderText('邮箱'), 'admin@example.com');
    await user.type(screen.getByPlaceholderText('密码'), 'password');
    await user.click(screen.getByTestId('admin-login-submit'));

    const totp = await screen.findByTestId('admin-login-totp');
    expect(await screen.findByText('验证码错误或已被使用')).toBeInTheDocument();

    await user.type(totp, '000000');
    expect(screen.getByPlaceholderText('邮箱')).toHaveValue('admin@example.com');
    expect(localStorage.getItem('authorization')).toBeNull();
  });

  it('shows the native reset command in the forgot-password dialog', async () => {
    const { user } = renderLogin();

    const forgotPassword = screen.getByTestId('admin-forgot-password');
    expect(forgotPassword).toHaveAttribute('type', 'button');
    await user.click(forgotPassword);

    expect(await screen.findByText('在站点目录下执行命令找回密码')).toBeInTheDocument();
    expect(
      screen.getByText("V2BOARD_NEW_PASSWORD='新密码' v2board-api reset-admin-password 管理员邮箱"),
    ).toBeInTheDocument();
  });
});
