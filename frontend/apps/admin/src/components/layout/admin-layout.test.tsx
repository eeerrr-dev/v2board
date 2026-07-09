import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { user } from '@v2board/api-client';
import { AdminLayout } from './admin-layout';

// The admin shell is a redesigned shadcn island (SidebarProvider + token
// dark-mode) replacing the OneUI/Bootstrap replica. The legacy byte-pins
// (#sidebar OneUI classes, darkreader, the header search overlay, avatar
// document-click, dead loading/search/title props) are retired. What stays
// covered is behavior: navigation targets, the user.info/email fetch, active +
// title routing, logout, scroll-to-top, and the dark-mode toggle outcome.

const mocks = vi.hoisted(() => ({
  location: { pathname: '/dashboard' } as { pathname: string },
  navigate: vi.fn(),
  logout: vi.fn(),
}));

vi.mock('react-router', () => ({
  Outlet: () => <div data-outlet="true" />,
  useLocation: () => mocks.location,
  useNavigate: () => mocks.navigate,
}));

vi.mock('@v2board/api-client', () => ({ user: { info: vi.fn() } }));
vi.mock('@/lib/api', () => ({ apiClient: {} }));
vi.mock('@/lib/auth', () => ({ logout: mocks.logout }));

function renderShell() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
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
    mocks.logout.mockReset();
    vi.mocked(user.info).mockReset();
    vi.mocked(user.info).mockResolvedValue({ email: 'admin@example.com' } as Awaited<
      ReturnType<typeof user.info>
    >);
    localStorage.clear();
    document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
    document.documentElement.className = '';
    window.settings = { title: 'V2Board', secure_path: 'admin' };
    Object.defineProperty(window, 'scrollTo', { configurable: true, value: vi.fn() });
  });

  afterEach(() => {
    window.settings = undefined;
    document.documentElement.className = '';
  });

  it('renders the grouped admin navigation without any legacy OneUI shell markup', () => {
    renderShell();

    for (const label of ['系统配置', '支付配置', '节点管理', '礼品卡管理', '队列监控', '知识库管理']) {
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

    expect(screen.getByRole('button', { name: '用户管理' })).toHaveAttribute('data-active', 'true');
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('用户管理');
  });

  it('leaves the header title empty for a non-legacy route path', () => {
    mocks.location = { pathname: '/users' };
    renderShell();

    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('');
  });

  it('navigates to the target route from a sidebar item', async () => {
    const u = userEvent.setup();
    renderShell();

    await u.click(screen.getByRole('button', { name: '节点管理' }));
    expect(mocks.navigate).toHaveBeenCalledWith('/server/manage');
  });

  it('requests user info on mount and shows the email in the account footer', async () => {
    renderShell();

    await waitFor(() => expect(user.info).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(screen.getByTestId('admin-avatar-trigger').textContent).toContain(
        'admin@example.com',
      ),
    );
  });

  it('scrolls to the top on mount', () => {
    renderShell();
    expect(window.scrollTo).toHaveBeenCalledWith(0, 0);
  });

  it('logs out and returns to the login route from the account menu', async () => {
    const u = userEvent.setup();
    renderShell();

    await u.click(screen.getByTestId('admin-avatar-trigger'));
    await u.click(await screen.findByTestId('admin-logout'));

    expect(mocks.logout).toHaveBeenCalledTimes(1);
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
