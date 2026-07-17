import { waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LinkProps } from 'react-router';
import type * as AuthModule from '@/lib/auth';
import { renderWithProviders } from '@/test/render';
import { createTestTranslation } from '@/test/i18next-selector';
import LoginPage from './login';
import type * as RuntimeConfigModule from '@/lib/runtime-config';

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
  fetchUserInfo: vi.fn(),
  isPending: false,
  labels: {
    'auth.email': '邮箱',
    'auth.email_invalid': '请输入有效邮箱',
    'auth.forget_password': '忘记密码？',
    'auth.login_description': '输入邮箱和密码继续',
    'auth.login_title': '欢迎回来',
    'auth.no_account': '还没有账号？',
    'auth.password': '密码',
    'auth.password_min': '密码至少需要 8 个字符',
    'auth.sign_up': '注册',
    'auth.submit_login': '登录',
  } as Record<string, string>,
  loginMutateAsync: vi.fn(),
  navigate: vi.fn(),
  params: new URLSearchParams(),
  queryClient: {
    prefetchQuery: vi.fn(),
  },
  setAuthData: vi.fn(),
  settings: {
    logo: '',
    title: 'V2Board',
  },
  tokenLoginMutateAsync: vi.fn(),
}));

vi.mock('react-router', () => ({
  Link: ({ to, children, className }: LinkProps) => (
    <a href={String(to)} className={className}>
      {children}
    </a>
  ),
  useNavigate: () => mocks.navigate,
  useSearchParams: () => [mocks.params],
}));

vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => mocks.queryClient,
}));

vi.mock('@/lib/auth', async (importOriginal) => {
  const actual = await importOriginal<typeof AuthModule>();
  return { ...actual, setAuthData: mocks.setAuthData };
});

vi.mock('@/lib/guest', () => ({
  useLoginMutation: () => ({
    isPending: mocks.isPending,
    mutateAsync: mocks.loginMutateAsync,
  }),
  useTokenLoginMutation: () => ({
    mutateAsync: mocks.tokenLoginMutateAsync,
  }),
}));

vi.mock('@/lib/runtime-config', async (importOriginal) => ({
  ...(await importOriginal<typeof RuntimeConfigModule>()),
  getBackgroundUrl: () => '',
  getLogoUrl: () => '',
  getSiteTitle: () => mocks.settings.title,
  getRuntimeConfig: () => ({ i18n: ['en-US', 'zh-CN'] }),
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
  useTranslation: () => createTestTranslation(mocks.labels),
}));

function resetMocks() {
  mocks.fetchUserInfo.mockReset();
  mocks.isPending = false;
  mocks.loginMutateAsync.mockReset();
  mocks.loginMutateAsync.mockResolvedValue({ auth_data: 'LOGIN_AUTH' });
  mocks.navigate.mockReset();
  mocks.params = new URLSearchParams();
  mocks.queryClient.prefetchQuery.mockReset();
  mocks.queryClient.prefetchQuery.mockResolvedValue(undefined);
  mocks.setAuthData.mockReset();
  mocks.settings.logo = '';
  mocks.settings.title = 'V2Board';
  window.g_lang = 'zh-CN';
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
    window.g_lang = undefined;
  });

  it('renders the login card with translated labels, shadcn examples, and footer copy', () => {
    const view = renderWithProviders(<LoginPage />);

    expect(view.getByTestId('auth-card')).toBeInTheDocument();
    const heading = view.getByRole('heading', { level: 1, name: '欢迎回来' });
    expect(heading).toHaveAttribute('data-slot', 'auth-title');
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
    mocks.settings.logo = '/theme/logo.png';

    const view = renderWithProviders(<LoginPage />);

    // The base Button shows its own spinner alongside the still-visible label while pending.
    const submit = view.getByRole('button', { name: '登录' });
    expect(submit).toBeDisabled();
    expect(submit.querySelector('svg')).not.toBeNull();

    // Operator logo stays out of the action card.
    expect(view.queryByRole('img')).toBeNull();
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

  it('renders email and minimum-password validation inline without calling login', async () => {
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'not-an-email');
    await view.user.type(view.getByLabelText('密码'), 'short');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    expect(await view.findByText('请输入有效邮箱')).toBeInTheDocument();
    expect(view.getByText('密码至少需要 8 个字符')).toBeInTheDocument();
    expect(view.getByLabelText('邮箱')).toHaveAttribute('aria-describedby', 'login-email-error');
    expect(view.getByLabelText('密码')).toHaveAttribute('aria-describedby', 'login-password-error');
    expect(mocks.loginMutateAsync).not.toHaveBeenCalled();
    expect(mocks.setAuthData).not.toHaveBeenCalled();
  });

  it('submits the form values, stores auth data, fetches user info, and pushes the redirect', async () => {
    mocks.params = new URLSearchParams('redirect=order');
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), '  user@example.com  ');
    await view.user.type(view.getByLabelText('密码'), 'secret88');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/order'));
    expect(mocks.loginMutateAsync).toHaveBeenCalledWith({
      email: 'user@example.com',
      password: 'secret88',
    });
    expect(mocks.setAuthData).toHaveBeenCalledWith('LOGIN_AUTH');
    expect(mocks.queryClient.prefetchQuery).toHaveBeenCalledWith({
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
    await view.user.type(view.getByLabelText('密码'), 'secret88{Enter}');

    await waitFor(() =>
      expect(mocks.loginMutateAsync).toHaveBeenCalledWith({
        email: 'user@example.com',
        password: 'secret88',
      }),
    );
    await waitFor(() => expect(mocks.navigate).toHaveBeenCalledWith('/dashboard'));
  });

  it('falls back to dashboard for network-path redirect values', async () => {
    mocks.params = new URLSearchParams('redirect=//evil.example/path');
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'secret88');
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
      await view.user.type(view.getByLabelText('密码'), 'secret88');
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
    await view.user.type(view.getByLabelText('密码'), 'wrongpass');
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

  it('shows an inline error for transport failures', async () => {
    mocks.loginMutateAsync.mockRejectedValue(new mocks.ApiError(0, 'Network Error'));
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'secret88');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    await waitFor(() => expect(mocks.loginMutateAsync).toHaveBeenCalled());
    await flushMicrotasks();

    expect(view.getByRole('alert')).toHaveTextContent('Network Error');
    expect(mocks.setAuthData).not.toHaveBeenCalled();
    expect(mocks.navigate).not.toHaveBeenCalled();
  });

  it('does not render stale inline feedback for a 403 owned by API redirect teardown', async () => {
    mocks.loginMutateAsync.mockRejectedValue(new mocks.ApiError(403, '登录已过期'));
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'secret88');
    await view.user.click(view.getByRole('button', { name: '登录' }));

    await waitFor(() => expect(mocks.loginMutateAsync).toHaveBeenCalledOnce());
    await flushMicrotasks();
    expect(view.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('clears the inline error once the user edits a field', async () => {
    mocks.loginMutateAsync.mockRejectedValue(new mocks.ApiError(422, '邮箱或密码错误'));
    const view = renderWithProviders(<LoginPage />);

    await view.user.type(view.getByLabelText('邮箱'), 'user@example.com');
    await view.user.type(view.getByLabelText('密码'), 'wrongpass');
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

  it('keeps the register and forgetpassword route contracts through React Router links', () => {
    const view = renderWithProviders(<LoginPage />);

    // Router-native Links carrying the history route paths (docs/api-dialect.md
    // §10.1) — no JS click handler, no hardcoded anchors.
    expect(view.getByRole('link', { name: '注册' })).toHaveAttribute('href', '/register');
    expect(view.getByRole('link', { name: '忘记密码？' })).toHaveAttribute(
      'href',
      '/forgetpassword',
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
    expect(mocks.queryClient.prefetchQuery).not.toHaveBeenCalled();
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
});
