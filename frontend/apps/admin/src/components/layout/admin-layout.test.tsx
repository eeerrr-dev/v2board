import { act } from 'react';
import { readFileSync } from 'node:fs';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { user } from '@v2board/api-client';
import { AdminLayout } from './admin-layout';

const mocks = vi.hoisted(() => ({
  location: { pathname: '/dashboard' },
  navigate: vi.fn(),
}));

vi.mock('react-router-dom', () => ({
  Outlet: () => <div data-outlet="true" />,
  useLocation: () => mocks.location,
  useNavigate: () => mocks.navigate,
}));

vi.mock('@v2board/api-client', () => ({
  user: { info: vi.fn() },
}));

vi.mock('@/lib/api', () => ({
  apiClient: {},
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetAdminLayoutMocks() {
  mocks.location = { pathname: '/dashboard' };
  mocks.navigate.mockReset();
  vi.mocked(user.info).mockReset();
  vi.mocked(user.info).mockResolvedValue({ email: 'admin@example.com' } as Awaited<
    ReturnType<typeof user.info>
  >);
  window.localStorage.clear();
  document.cookie = 'dark_mode=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
  document.documentElement.className = '';
  window.webpackJsonp = undefined;
  const scrollTo = vi.fn();
  Object.defineProperty(window, 'scrollTo', {
    configurable: true,
    value: scrollTo,
  });
  window.settings = {
    title: 'V2Board',
    theme: { sidebar: 'dark', header: 'dark', color: 'default' },
    secure_path: 'admin',
  };
  return scrollTo;
}

describe('AdminLayout legacy shell', () => {
  beforeEach(() => {
    resetAdminLayoutMocks();
  });

  it('renders the original OneUI admin layout shell', () => {
    const html = renderToStaticMarkup(<AdminLayout />);

    expect(html).toContain('id="page-container"');
    expect(html).toContain('sidebar-o sidebar-dark page-header-dark');
    expect(html).toContain('class="v2board-nav-mask"');
    expect(html).toContain('id="sidebar"');
    expect(html).toContain('class="content-side content-side-full"');
    expect(html).toContain('id="page-header"');
    expect(html).toContain('id="main-container"');
    expect(html).toContain('class="p-0 p-lg-4"');
    expect(html).toContain('V2Board v1.7.5');
  });

  it('renders the legacy admin navigation labels and icons', () => {
    const html = renderToStaticMarkup(<AdminLayout />);

    expect(html).toContain('系统配置');
    expect(html).toContain('支付配置');
    expect(html).toContain('节点管理');
    expect(html).toContain('礼品卡管理');
    expect(html).toContain('队列监控');
    expect(html).toContain('nav-main-link-icon si si-speedometer');
    expect(html).toContain('nav-main-link active');
    expect(html).toContain('href="/#/dashboard"');
    expect(html).toContain('href="/#/server/manage"');
  });

  it('keeps bundled-theme random keys for admin sidebar menu headings and items', () => {
    const source = readFileSync(`${process.cwd()}/src/components/layout/admin-layout.tsx`, 'utf8');

    expect(source).toContain('<li key={Math.random()} className="nav-main-heading">');
    expect(source).toContain('<li key={Math.random()} className="nav-main-item">');
    expect(source).toContain(
      "className={`dropdown-menu dropdown-menu-right dropdown-menu-lg p-0 ${showAvatarMenu && 'show'}`}",
    );
    expect(source).not.toContain('key={`${item.title}-${index}`}');
    expect(source).not.toContain('key={item.href}');
  });
});

describe('AdminLayout legacy dark mode behavior', () => {
  let container: HTMLDivElement;
  let root: Root;
  let scrollTo: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    scrollTo = resetAdminLayoutMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  async function renderLayout() {
    await act(async () => {
      root.render(<AdminLayout />);
      await Promise.resolve();
    });
  }

  it('scrolls to top on mount and toggles the mobile sidebar mask like the old layout', async () => {
    await renderLayout();

    expect(scrollTo).toHaveBeenCalledWith(0, 0);
    const page = container.querySelector('#page-container')!;
    const toggle = container.querySelector<HTMLButtonElement>('.sidebar-toggle button')!;

    await act(async () => {
      toggle.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(page.className).toContain('sidebar-o-xs');
    expect((container.querySelector('.v2board-nav-mask') as HTMLElement).style.display).toBe(
      'block',
    );

    await act(async () => {
      container
        .querySelector('.v2board-nav-mask')!
        .dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(page.className).not.toContain('sidebar-o-xs');
    expect((container.querySelector('.v2board-nav-mask') as HTMLElement).style.display).toBe(
      'none',
    );
  });

  it('scrolls back to top when the route changes', async () => {
    await renderLayout();
    scrollTo.mockClear();

    mocks.location = { pathname: '/order' };
    await act(async () => {
      root.render(<AdminLayout />);
      await Promise.resolve();
    });

    expect(scrollTo).toHaveBeenCalledTimes(1);
    expect(scrollTo).toHaveBeenCalledWith(0, 0);
  });

  it('requests user info on mount even without a local authorization token', async () => {
    expect(window.localStorage.getItem('authorization')).toBeNull();

    await renderLayout();

    expect(user.info).toHaveBeenCalledTimes(1);
    expect(container.querySelector('#page-header-user-dropdown')!.textContent).toContain(
      'admin@example.com',
    );
  });

  it('navigates with sidebar links and closes the mobile sidebar after navigation', async () => {
    await renderLayout();

    const toggle = container.querySelector<HTMLButtonElement>('.sidebar-toggle button')!;
    await act(async () => {
      toggle.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const nodeManage = Array.from(container.querySelectorAll('.nav-main-link')).find(
      (link) => link.textContent === '节点管理',
    )!;
    const click = new MouseEvent('click', { bubbles: true, cancelable: true });
    await act(async () => {
      nodeManage.dispatchEvent(click);
      await Promise.resolve();
    });

    expect((nodeManage as HTMLAnchorElement).getAttribute('href')).toBe('/#/server/manage');
    expect(click.defaultPrevented).toBe(true);
    expect(mocks.navigate).toHaveBeenCalledWith('/server/manage');
    expect(container.querySelector('#page-container')!.className).not.toContain('sidebar-o-xs');
  });

  it('only titles legacy route paths and does not support removed alias paths', async () => {
    mocks.location = { pathname: '/user' };
    await renderLayout();

    expect(container.querySelector('.v2board-container-title')!.textContent).toBe('用户管理');

    mocks.location = { pathname: '/users' };
    await act(async () => {
      root.render(<AdminLayout />);
      await Promise.resolve();
    });

    expect(container.querySelector('.v2board-container-title')!.textContent).toBe('');
  });

  it('closes the avatar menu on the next document click like the old layout', async () => {
    await renderLayout();

    const userButton =
      container.querySelector<HTMLButtonElement>('#page-header-user-dropdown')!;

    await act(async () => {
      userButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.querySelector('.dropdown-menu')!.className).toContain('show');

    await act(async () => {
      document.body.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.querySelector('.dropdown-menu')!.className).not.toContain('show');
  });

  it('toggles the original dark_mode cookie and header icon from the old admin button', async () => {
    await renderLayout();

    const darkButton = container.querySelector<HTMLButtonElement>('#page-header .dropdown button')!;
    expect(darkButton.innerHTML).toContain('fa-sun');

    await act(async () => {
      darkButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(document.cookie).toContain('dark_mode=1');
    expect(window.localStorage.getItem('dark_mode')).toBeNull();
    expect(document.documentElement.classList.contains('v2board-dark-mode')).toBe(true);
    expect(darkButton.innerHTML).toContain('fa-moon');
  });
});
