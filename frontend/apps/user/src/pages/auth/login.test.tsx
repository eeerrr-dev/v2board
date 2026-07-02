import { waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithProviders } from '@/test/render';
import LoginPage from './login';

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
    'auth.login_description': '输入邮箱和密码继续',
    'auth.login_title': '欢迎回来',
    'auth.no_account': '还没有账号？',
    'auth.password': '密码',
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

/** Settle the controller's promise chains (mutation -> auth write -> navigate). */
async function flushMicrotasks() {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe('LoginPage', () => {
  beforeEach(resetMocks);

  afterEach(() => {
    window.settings = undefined;
    window.g_lang = undefined;
  });

  it('renders the login card with translated labels, shadcn examples, and footer copy', () => {
    const view = renderWithProviders(<LoginPage />);

    // Interaction-parity harness hooks (visual-parity.mjs selects
    // `.v2board-auth-card` and `.v2board-auth-title` in interactions mode).
    expect(view.container.querySelector('.v2board-auth-card')).not.toBeNull();
    const heading = view.getByRole('heading', { level: 1, name: '欢迎回来' });
    expect(view.container.querySelector('.v2board-auth-title')).toBe(heading);
    expect(view.getByText('输入邮箱和密码继续')).toBeInTheDocument();

    // Shadcn-style examples: the identifier field may show an example, passwords stay label-only.
    const email = view.getByLabelText('邮箱');
    expect(email).toHaveAttribute('type', 'email');
    expect(email).toHaveAttribute('placeholder', 'm@example.com');
    const password = view.getByLabelText('密码');
    expect(password).toHaveAttribute('type', 'password');
    expect(password).not.toHaveAttribute('placeholder');

    expect(view.getByRole('button', { name: '登录' })).toHaveAttribute('type', 'submit');
    expect(view.getByText('还没有账号？')).toBeInTheDocument();
    expect(view.getByRole('link', { name: '注册' })).toBeInTheDocument();
    expect(view.getByRole('link', { name: '忘记密码？' })).toBeInTheDocument();
  });

  it('shows a busy, still-labeled submit and no operator branding while login is pending', () => {
    mocks.isPending = true;
    mocks.settings.description = 'Legacy description';
    mocks.settings.logo = '/theme/logo.png';

    const view = renderWithProviders(<LoginPage />);

    // The base Button shows its own spinner alongside the still-visible label while pending.
    const submit = view.getByRole('button', { name: '登录' });
    expect(submit).toBeDisabled();
    expect(submit.querySelector('svg')).not.toBeNull();

    // Operator logo and description stay out of the action card.
    expect(view.queryByRole('img')).toBeNull();
    expect(view.queryByText('Legacy description')).toBeNull();
    expect(view.getByRole('heading', { level: 1, name: '欢迎回来' })).toBeInTheDocument();
  });

  it('keeps the card focus order email → password → submit with no tabindex overrides', () => {
    const { container } = renderWithProviders(<LoginPage />);

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
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'secret');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order'));
    expect(mocks.loginMutateAsync).toHaveBeenCalledWith({
      email: 'user@example.com',
      password: 'secret',
    });
    expect(mocks.setAuthData).toHaveBeenCalledWith('LOGIN_AUTH');
    expect(mocks.queryClient.fetchQuery).toHaveBeenCalledWith({
      queryFn: mocks.fetchUserInfo,
      queryKey: ['user', 'info'],
    });
    expect(mocks.navigate).not.toHaveBeenCalledWith('order');
  });

  it('submits natively from the password field via Enter with no global key listener', async () => {
    // Re-pin of the retired window keydown(keyCode===13) shortcut: the browser's implicit
    // form submission (Enter from any field reaches the type="submit" button) does the work.
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'secret{Enter}');

    await waitFor(() =>
      expect(mocks.loginMutateAsync).toHaveBeenCalledWith({
        email: 'user@example.com',
        password: 'secret',
      }),
    );
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/dashboard'));
  });

  it('falls back to dashboard for network-path redirect values', async () => {
    mocks.params = new URLSearchParams('redirect=//evil.example/path');
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'secret');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/dashboard'));
    expect(mocks.navigate).not.toHaveBeenCalledWith('//evil.example/path');
  });

  it('falls back to dashboard for backslash and tab protocol-relative bypasses', async () => {
    // Browsers normalize these to "//evil.example", so a literal "//" guard alone
    // would let them through to navigate() and resolve cross-origin.
    for (const evil of ['/\\evil.example/path', '/\u0009/evil.example']) {
      mocks.navigate.mockClear();
      mocks.params = new URLSearchParams();
      mocks.params.set('redirect', evil);

      const view = renderWithProviders(<LoginPage />);
      await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
      await view.user.type(view.getByLabelText('密码'), 'secret');
      await view.user.click(view.getByRole('button', { name: '登录' }));

      await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/dashboard'));
      expect(mocks.navigate).not.toHaveBeenCalledWith(evil);
      view.unmount();
    }
  });

  it('marks both fields invalid and ties them to the single alert when login fails', async () => {
    mocks.loginMutateAsync.mockRejectedValue(new mocks.ApiError(422, '邮箱或密码错误'));
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'wrong');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    const alert = await view.findByRole('alert');
    expect(alert).toHaveAttribute('id', 'login-error');
    // The auth island keeps error feedback textual; no decorative alert icon is required.
    expect(alert.querySelector('svg')).toBeNull();
    expect(alert).toHaveTextContent('邮箱或密码错误');
    // Both fields are programmatically invalid and described by the one alert box.
    for (const input of [view.getByLabelText('邮箱'), view.getByLabelText('密码')]) {
      expect(input).toHaveAttribute('aria-invalid', 'true');
      expect(input).toHaveAttribute('aria-describedby', 'login-error');
    }
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('suppresses the inline error for transport failures (status 0)', async () => {
    mocks.loginMutateAsync.mockRejectedValue(new mocks.ApiError(0, 'Network Error'));
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'secret');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    await waitFor(() => expect(mocks.loginMutateAsync).toHaveBeenCalled());
    await flushMicrotasks();

    // Transport failures surfaced nothing in the oracle: no inline alert, no auth, no navigation.
    expect(view.queryByRole('alert')).toBeNull();
    expect(mocks.setAuthData).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('clears the inline error once the user edits a field', async () => {
    mocks.loginMutateAsync.mockRejectedValue(new mocks.ApiError(422, '邮箱或密码错误'));
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'wrong');
    await view.user.click(view.getByRole('button', { name: '登录' }));
    await view.findByRole('alert');

    // The form-level onInput is wired to clearError, so editing any field dismisses the alert.
    await view.user.type(view.getByLabelText('密码'), 'x');

    await waitFor(() => expect(view.queryByRole('alert')).toBeNull());
    for (const input of [view.getByLabelText('邮箱'), view.getByLabelText('密码')]) {
      expect(input).not.toHaveAttribute('aria-invalid');
    }
  });

  it('keeps the password field masked without rendering an icon reveal button', () => {
    const view = renderWithProviders(<LoginPage />);

    expect(view.getByLabelText('密码')).toHaveAttribute('type', 'password');
    expect(view.container.querySelector('button[aria-pressed]')).toBeNull();
    // The submit button is the only button on the surface.
    expect(view.getAllByRole('button')).toHaveLength(1);
    expect(mocks.loginMutateAsync).not.toHaveBeenCalled();
  });

  it('navigates register and forgetpassword via real hash anchors (no javascript: hrefs)', () => {
    const view = renderWithProviders(<LoginPage />);

    // Real hash anchors — the data router navigates natively, with no JS click handler or
    // `javascript:` href. The behavior contract (lands on register/forgetpassword) is preserved.
    expect(view.getByRole('link', { name: '注册' })).toHaveAttribute('href', '#/register');
    expect(view.getByRole('link', { name: '忘记密码？' })).toHaveAttribute(
      'href',
      '#/forgetpassword',
    );
  });

  it('runs token2Login with the raw query redirect and pushes after setting auth data', async () => {
    mocks.params = new URLSearchParams('verify=verify-token&redirect=order');
    renderWithProviders(<LoginPage />);

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order'));
    expect(mocks.tokenLoginMutateAsync).toHaveBeenCalledWith({
      redirect: 'order',
      verify: 'verify-token',
    });
    expect(mocks.setAuthData).toHaveBeenCalledWith('TOKEN_AUTH');
    expect(mocks.queryClient.fetchQuery).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalledWith('order');
  });

  it('redeems the one-time verify token only once across a doubled bootstrap (StrictMode-safe)', async () => {
    // React 19 StrictMode double-invokes the mount effect in dev (mount → cleanup →
    // mount). A doubled bootstrap for the same verify token must POST token2Login only
    // once — the one-time token would fail the second redemption — while the surviving
    // mount still finishes the login. The controller guards this with a module-level
    // in-flight map keyed by verify value, exercised here by two live mounts.
    let resolveTokenLogin: ((value: { auth_data: string }) => void) | undefined;
    mocks.params = new URLSearchParams('verify=verify-once&redirect=order');
    mocks.tokenLoginMutateAsync.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTokenLogin = resolve;
        }),
    );

    // First mount arms the in-flight redemption for verify-once.
    const first = renderWithProviders(<LoginPage />);
    await waitFor(() => expect(mocks.tokenLoginMutateAsync).toHaveBeenCalledTimes(1));

    // A second bootstrap for the same token re-attaches to the shared redemption
    // instead of POSTing token2Login again.
    const second = renderWithProviders(<LoginPage />);
    await flushMicrotasks();
    expect(mocks.tokenLoginMutateAsync).toHaveBeenCalledTimes(1);
    expect(mocks.tokenLoginMutateAsync).toHaveBeenCalledWith({
      redirect: 'order',
      verify: 'verify-once',
    });

    // Completing the shared redemption still finishes the login on the live surface.
    resolveTokenLogin?.({ auth_data: 'TOKEN_AUTH' });
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order'));
    expect(mocks.setAuthData).toHaveBeenCalledWith('TOKEN_AUTH');

    first.unmount();
    second.unmount();
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

    const view = renderWithProviders(<LoginPage />);
    await waitFor(() =>
      expect(mocks.tokenLoginMutateAsync).toHaveBeenCalledWith({
        redirect: 'order',
        verify: 'verify-token',
      }),
    );

    // Dropping the verify param re-runs the bootstrap effect, cleaning up the first run.
    mocks.params = new URLSearchParams();
    view.rerender(<LoginPage />);

    resolveTokenLogin?.({ auth_data: 'STALE_AUTH' });
    await flushMicrotasks();

    expect(mocks.setAuthData).not.toHaveBeenCalledWith('STALE_AUTH');
    expect(mocks.navigate).not.toHaveBeenCalledWith('/order');
  });

  it('ignores stale checkLogin completions after the bootstrap effect is cleaned up', async () => {
    let resolveCheckLogin: ((value: { is_login: boolean }) => void) | undefined;
    mocks.getAuthData.mockReturnValue('EXISTING_AUTH');
    mocks.checkLogin.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveCheckLogin = resolve;
        }),
    );

    // A late is_login:true after unmount must not fetch user info or navigate.
    const first = renderWithProviders(<LoginPage />);
    await waitFor(() => expect(mocks.checkLogin).toHaveBeenCalledWith(mocks.apiClient));
    first.unmount();
    resolveCheckLogin?.({ is_login: true });
    await flushMicrotasks();
    expect(mocks.queryClient.fetchQuery).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();

    // A late is_login:false after unmount must not wipe the stored auth data.
    const second = renderWithProviders(<LoginPage />);
    await waitFor(() => expect(mocks.checkLogin).toHaveBeenCalledTimes(2));
    second.unmount();
    resolveCheckLogin?.({ is_login: false });
    await flushMicrotasks();
    expect(mocks.setAuthData).not.toHaveBeenCalled();
  });

  it('keeps the original checkLogin effect auth-data guard before requesting /user/checkLogin', async () => {
    renderWithProviders(<LoginPage />);
    await flushMicrotasks();

    expect(mocks.getAuthData).toHaveBeenCalled();
    expect(mocks.checkLogin).not.toHaveBeenCalled();
    expect(mocks.queryClient.fetchQuery).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('checks an existing auth session on mount, fetches user info, and pushes dashboard', async () => {
    mocks.getAuthData.mockReturnValue('EXISTING_AUTH');
    mocks.checkLogin.mockResolvedValue({ is_login: true });

    renderWithProviders(<LoginPage />);

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/dashboard'));
    expect(mocks.checkLogin).toHaveBeenCalledWith(mocks.apiClient);
    expect(mocks.queryClient.fetchQuery).toHaveBeenCalledWith({
      queryFn: mocks.fetchUserInfo,
      queryKey: ['user', 'info'],
    });
    expect(mocks.navigate).not.toHaveBeenCalledWith('dashboard');
  });

  it('clears existing auth when checkLogin says the session is not logged in', async () => {
    mocks.getAuthData.mockReturnValue('STALE_AUTH');
    mocks.checkLogin.mockResolvedValue({ is_login: false });

    renderWithProviders(<LoginPage />);

    await waitFor(() => expect(mocks.setAuthData).toHaveBeenCalledWith(null));
    expect(mocks.checkLogin).toHaveBeenCalledWith(mocks.apiClient);
    expect(mocks.navigate).not.toHaveBeenCalledWith('/dashboard');
  });

  it('does not run the stale-session checkLogin while a verify token is redeemed', async () => {
    // An already-authed user opening a verify handoff link: token2Login mints a
    // fresh session, so the stale-token checkLogin must not race it — a late
    // is_login:false would otherwise wipe the freshly-minted token.
    mocks.params = new URLSearchParams('verify=verify-token&redirect=order');
    mocks.getAuthData.mockReturnValue('EXISTING_AUTH');
    mocks.checkLogin.mockResolvedValue({ is_login: false });

    renderWithProviders(<LoginPage />);

    await waitFor(() => expect(mocks.setAuthData).toHaveBeenCalledWith('TOKEN_AUTH'));
    expect(mocks.checkLogin).not.toHaveBeenCalled();
    expect(mocks.setAuthData).not.toHaveBeenCalledWith(null);
  });
});
