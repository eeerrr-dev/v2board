import type { ComponentProps } from 'react';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { user } from '@v2board/api-client';
import type * as ApiClientModule from '@v2board/api-client';
import { setAdminRuntimeConfig } from '@/test/runtime-config';
import { adminSessionKeys } from '@/lib/session-queries';
import { AdminLayout } from './admin-layout';

// The admin shell is a redesigned shadcn island (SidebarProvider + token
// dark-mode) replacing the OneUI/Bootstrap replica. The legacy byte-pins
// (#sidebar OneUI classes, darkreader, the header search overlay, avatar
// document-click, dead loading/search/title props) are retired. What stays
// covered is behavior: navigation targets, the user.info/email fetch, active +
// title routing, logout, router scroll management, and the dark-mode toggle outcome.

const mocks = vi.hoisted(() => ({
  location: { pathname: '/dashboard' } as { pathname: string },
  navigate: vi.fn(),
  signOut: vi.fn(),
}));

vi.mock('react-router', () => ({
  // Scroll management is delegated to the router; the shell only mounts it.
  ScrollRestoration: () => <div data-testid="scroll-restoration" />,
  Link: ({
    to,
    onClick,
    children,
    ...rest
  }: { to: string } & Omit<ComponentProps<'a'>, 'href'>) => (
    <a
      href={to}
      onClick={(event) => {
        onClick?.(event);
        if (!event.defaultPrevented) {
          event.preventDefault();
          mocks.navigate(to);
        }
      }}
      {...rest}
    >
      {children}
    </a>
  ),
  Outlet: () => <div data-outlet="true" />,
  useLocation: () => mocks.location,
  useNavigate: () => mocks.navigate,
}));

vi.mock('@v2board/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof ApiClientModule>()),
  user: { info: vi.fn() },
}));
// The account menu's explicit sign-out (revocation + local teardown) lives in
// lib/api as signOut; the shell only wires the menu item to it.
vi.mock('@/lib/api', () => ({ apiClient: {}, signOut: mocks.signOut }));

function renderShell({ preloadUserInfo = true }: { preloadUserInfo?: boolean } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  if (preloadUserInfo) {
    client.setQueryData(adminSessionKeys.userInfo, { email: 'admin@example.com' });
  }
  return render(
    <QueryClientProvider client={client}>
      <AdminLayout />
    </QueryClientProvider>,
  );
}

describe('AdminLayout', () => {
  beforeEach(() => {
    mocks.location = { pathname: '/dashboard' };
    mocks.navigate.mockReset();
    mocks.signOut.mockReset();
    vi.mocked(user.info).mockReset();
    vi.mocked(user.info).mockResolvedValue({ email: 'admin@example.com' } as Awaited<
      ReturnType<typeof user.info>
    >);
    localStorage.clear();
    document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.cookie = 'sidebar_state=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.documentElement.className = '';
    setAdminRuntimeConfig({ title: 'V2Board', secure_path: 'admin' });
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 1024,
    });
  });

  afterEach(() => {
    setAdminRuntimeConfig();
    document.documentElement.className = '';
  });

  it('renders the grouped admin navigation without any legacy OneUI shell markup', () => {
    renderShell();

    for (const label of [
      '系统配置',
      '支付配置',
      '节点管理',
      '礼品卡管理',
      '队列监控',
      '审计日志',
      '知识库管理',
    ]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
    expect(document.querySelector('.nav-main-link')).toBeNull();
    expect(document.querySelector('.content-side')).toBeNull();
    expect(document.querySelector('i.si')).toBeNull();
    expect(document.body.innerHTML).not.toContain('v1.7.5');
  });

  it('marks the active route and titles the header from the route', () => {
    mocks.location = { pathname: '/user' };
    renderShell();

    const active = screen.getByRole('link', { name: '用户管理' });
    expect(active).toHaveAttribute('data-active', 'true');
    expect(active).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('用户管理');
  });

  it('leaves the header title empty for a non-legacy route path', () => {
    mocks.location = { pathname: '/users' };
    renderShell();

    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('');
  });

  it('renders real links for the brand and sidebar routes', async () => {
    const u = userEvent.setup();
    renderShell();

    expect(screen.getByRole('link', { name: 'V2Board' })).toHaveAttribute('href', '/dashboard');
    const node = screen.getByRole('link', { name: '节点管理' });
    expect(node).toHaveAttribute('href', '/server/manage');
    await u.click(node);
    expect(mocks.navigate).toHaveBeenCalledWith('/server/manage');
  });

  it('closes the mobile navigation sheet after following a link', async () => {
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 500,
    });
    const u = userEvent.setup();
    const { container } = renderShell();
    const sheet = () =>
      document.body.querySelector<HTMLElement>('[data-slot="sidebar"][data-mobile="true"]');

    expect(sheet()).toBeNull();
    await u.click(
      container.querySelector<HTMLButtonElement>('#page-header [data-sidebar="trigger"]')!,
    );
    expect(sheet()).not.toBeNull();

    const node = within(sheet()!).getByRole('link', { name: '节点管理' });
    expect(node).toHaveAttribute('href', '/server/manage');
    await u.click(node);

    expect(mocks.navigate).toHaveBeenCalledWith('/server/manage');
    expect(sheet()).toBeNull();
  });

  it('reads the loader-owned identity without issuing a duplicate shell request', () => {
    renderShell();

    expect(screen.getByTestId('admin-avatar-trigger')).toHaveTextContent('admin@example.com');
    expect(user.info).not.toHaveBeenCalled();
  });

  it('shows the suspense fallback while a defensive identity read is pending', () => {
    vi.mocked(user.info).mockReturnValue(
      new Promise<Awaited<ReturnType<typeof user.info>>>(() => {}),
    );

    renderShell({ preloadUserInfo: false });

    expect(screen.getByRole('status')).toHaveTextContent('正在加载');
    expect(screen.queryByRole('navigation', { name: '主导航' })).not.toBeInTheDocument();
  });

  it('mounts router scroll management', () => {
    renderShell();
    expect(screen.getByTestId('scroll-restoration')).toBeInTheDocument();
  });

  it('toggles the desktop sidebar through the global keyboard shortcut', () => {
    renderShell();
    const rail = document.querySelector('[data-slot="sidebar"]');
    expect(rail).toHaveAttribute('data-state', 'expanded');

    fireEvent.keyDown(window, { key: 'b', ctrlKey: true });

    expect(rail).toHaveAttribute('data-state', 'collapsed');
    expect(document.cookie).toContain('sidebar_state=false');
  });

  it('logs out and returns to the login route from the account menu', async () => {
    const u = userEvent.setup();
    renderShell();

    await u.click(screen.getByTestId('admin-avatar-trigger'));
    await u.click(await screen.findByTestId('admin-logout'));

    expect(mocks.signOut).toHaveBeenCalledTimes(1);
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });

  it('applies dark mode through the token theme menu instead of darkreader', async () => {
    const u = userEvent.setup();
    renderShell();

    await u.click(screen.getByRole('button', { name: '切换主题' }));
    await u.click(await screen.findByText('深色'));

    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(document.cookie).toContain('dark_mode=1');
  });
});
