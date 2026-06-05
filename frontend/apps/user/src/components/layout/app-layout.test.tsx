import { act } from 'react';
import { readFileSync } from 'node:fs';
import { createRoot, type Root } from 'react-dom/client';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppLayout } from './app-layout';

const mocks = vi.hoisted(() => ({
  darkMode: false,
  labels: {
    'common.logout': '登出',
    'common.search': '搜索',
    'nav.buy_subscribe': '购买订阅',
    'nav.dashboard': '仪表盘',
    'nav.group_finance': '财务',
    'nav.group_subscribe': '订阅',
    'nav.group_user': '用户',
    'nav.invite': '我的邀请',
    'nav.knowledge': '使用文档',
    'nav.node': '节点状态',
    'nav.orders': '我的订单',
    'nav.profile': '个人中心',
    'nav.tickets': '我的工单',
    'nav.traffic': '流量明细',
    'order.detail': '订单详情',
    'plan.checkout_title': '确认订单',
  } as Record<string, string>,
  location: { pathname: '/dashboard' },
  logout: vi.fn(),
  navigate: vi.fn(),
  setDarkMode: vi.fn(),
  theme: { header: 'light', sidebar: 'light' },
  title: 'V2Board',
  user: { email: 'user@example.com' },
}));

vi.mock('react-router-dom', () => ({
  Outlet: () => <div data-outlet="true">Outlet content</div>,
  useLocation: () => mocks.location,
  useNavigate: () => mocks.navigate,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: 'zh-CN' },
    t: (key: string) => mocks.labels[key] ?? key,
  }),
}));

vi.mock('./language-menu', () => ({
  LanguageMenu: ({
    triggerClassName,
  }: {
    legacyIcon?: boolean;
    triggerClassName?: string;
  }) => (
    <button type="button" className={triggerClassName}>
      <i className="far fa fa-language" />
    </button>
  ),
}));

vi.mock('@/lib/queries', () => ({
  useUserInfo: () => ({
    data: mocks.user,
  }),
}));

vi.mock('@/lib/auth', () => ({
  logout: mocks.logout,
}));

vi.mock('@/lib/dark-mode', () => ({
  isDarkModeEnabled: () => mocks.darkMode,
  setDarkMode: mocks.setDarkMode,
}));

vi.mock('@/lib/legacy-settings', () => ({
  getLegacyTheme: () => mocks.theme,
  getLegacyTitle: () => mocks.title,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function resetMocks() {
  mocks.darkMode = false;
  mocks.location = { pathname: '/dashboard' };
  mocks.logout.mockReset();
  mocks.navigate.mockReset();
  mocks.setDarkMode.mockReset();
  mocks.theme = { header: 'light', sidebar: 'light' };
  mocks.title = 'V2Board';
  mocks.user = { email: 'user@example.com' };
  window.localStorage.setItem('umi_locale', 'zh-CN');
}

describe('AppLayout bundled-theme markup', () => {
  beforeEach(resetMocks);

  it('renders the old page-container classes, sidebar menu, header title, and copyright', () => {
    mocks.theme = { header: 'dark', sidebar: 'dark' };
    mocks.darkMode = true;

    const html = renderToStaticMarkup(<AppLayout />);

    expect(html).toContain(
      'id="page-container" class="zh-CN sidebar-o sidebar-dark page-header-dark side-scroll page-header-fixed main-content-boxed side-trans-enabled false"',
    );
    expect(html).toContain('class="v2board-nav-mask" style="display:none"');
    expect(html).toContain('id="sidebar"');
    expect(html).toContain('content-header justify-content-lg-center bg-white-10');
    expect(html).toContain('class="font-size-lg text-white" href="/"');
    expect(html).toContain('<span class="text-white-75">V2Board</span>');
    expect(html).toContain('class="nav-main"');
    expect(html).not.toContain('href="/#/dashboard"');
    expect(html).not.toContain('href="/#/knowledge"');
    expect(html).toContain('class="nav-main-link active"');
    expect(html).toContain('class="nav-main-link false"');
    expect(html).toContain('class="nav-main-heading">订阅</li>');
    expect(html).toContain('nav-main-link-icon si si-speedometer');
    expect(html).toContain('nav-main-link-icon si si-book-open');
    expect(html).toContain('nav-main-link-icon si si-bag');
    expect(html).toContain('nav-main-link-icon si si-check');
    expect(html).toContain('nav-main-link-icon si si-list');
    expect(html).toContain('nav-main-link-icon si si-users');
    expect(html).toContain('nav-main-link-icon si si-user');
    expect(html).toContain('nav-main-link-icon si si-support');
    expect(html).toContain('nav-main-link-icon si si-bar-chart');
    expect(html).toContain('class="v2board-copyright">V2Board');
    expect(html).toContain(' v1.7.4</div>');
    expect(html).toContain('id="page-header"');
    expect(html).toContain('v2board-container-title text-white');
    expect(html).toContain('仪表盘');
    expect(html).toContain('class="btn btn-primary mr-1"');
    expect(html).toContain('class="far fa fa-moon"');
    expect(html).toContain('user@example.com');
    expect(html).toContain('id="main-container"');
    expect(html).toContain('class="content content-full"');
    expect(html).toContain('Outlet content');
  });

  it('renders the bundled loading main container when loading is passed', () => {
    const html = renderToStaticMarkup(<AppLayout loading />);

    expect(html).toContain('id="main-container"');
    expect(html).toContain('class="content content-full font-size-h1"');
    expect(html).toContain('class="p-md-0 p-3"');
    expect(html).toContain('class="anticon anticon-loading"');
    expect(html).toContain('data-icon="loading"');
    expect(html).not.toContain('data-outlet="true"');
  });

  it('uses detail route titles without marking sidebar items active', () => {
    mocks.location = { pathname: '/order/TRADE123' };

    const html = renderToStaticMarkup(<AppLayout />);

    expect(html).toContain('订单详情');
    expect(html).not.toContain('class="nav-main-link active"');
  });

  it('keeps bundled-theme random keys for sidebar menu headings and items', () => {
    const source = readFileSync(`${process.cwd()}/src/components/layout/app-layout.tsx`, 'utf8');

    expect(source).toContain('<li key={Math.random()} className="nav-main-heading">');
    expect(source).toContain('<li className="nav-main-item" key={Math.random()}>');
    expect(source).not.toContain('<li className="nav-main-item" key={item.to}>');
  });
});

describe('AppLayout bundled-theme behavior', () => {
  let container: HTMLDivElement;
  let root: Root;
  let scrollTo: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    resetMocks();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    scrollTo = vi.fn();
    Object.defineProperty(window, 'scrollTo', {
      configurable: true,
      value: scrollTo,
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.innerHTML = '';
  });

  async function renderLayout(props?: Parameters<typeof AppLayout>[0]) {
    await act(async () => {
      root.render(<AppLayout {...props} />);
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

  it('does not scroll again when the route changes without remounting', async () => {
    await renderLayout();
    scrollTo.mockClear();

    mocks.location = { pathname: '/order' };
    await act(async () => {
      root.render(<AppLayout />);
      await Promise.resolve();
    });

    expect(scrollTo).not.toHaveBeenCalled();
  });

  it('navigates with sidebar links and closes the sidebar after navigation', async () => {
    await renderLayout();

    const toggle = container.querySelector<HTMLButtonElement>('.sidebar-toggle button')!;
    await act(async () => {
      toggle.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    const knowledge = Array.from(container.querySelectorAll('.nav-main-link')).find(
      (link) => link.textContent === '使用文档',
    )!;
    const click = new MouseEvent('click', { bubbles: true, cancelable: true });
    await act(async () => {
      knowledge.dispatchEvent(click);
      await Promise.resolve();
    });

    expect((knowledge as HTMLAnchorElement).getAttribute('href')).toBeNull();
    expect(click.defaultPrevented).toBe(false);
    expect(mocks.navigate).toHaveBeenCalledWith('/knowledge');
    expect(container.querySelector('#page-container')!.className).not.toContain('sidebar-o-xs');
  });

  it('keeps the legacy brand href while routing the click inside the hash app', async () => {
    await renderLayout();

    const brand = container.querySelector<HTMLAnchorElement>('#sidebar .content-header > a')!;
    const click = new MouseEvent('click', { bubbles: true, cancelable: true });

    await act(async () => {
      brand.dispatchEvent(click);
      await Promise.resolve();
    });

    expect(brand.getAttribute('href')).toBe('/');
    expect(click.defaultPrevented).toBe(true);
    expect(mocks.navigate).toHaveBeenCalledWith('/dashboard');
  });

  it('toggles dark mode through the old header button', async () => {
    await renderLayout();

    const darkButton = container.querySelector<HTMLButtonElement>('#page-header .dropdown button')!;
    expect(darkButton.innerHTML).toContain('fa-sun');

    await act(async () => {
      darkButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.setDarkMode).toHaveBeenCalledWith(true);
    expect(darkButton.innerHTML).toContain('fa-moon');
  });

  it('renders and controls the bundled header search overlay when search props are passed', async () => {
    const onChange = vi.fn();
    await renderLayout({
      search: {
        placeholder: '搜索文档',
        defaultValue: 'node',
        onChange,
      },
    });

    const sidebarToggle = container.querySelector<HTMLElement>('.sidebar-toggle')!;
    expect(sidebarToggle.style.display).toBe('block');
    expect(container.innerHTML).toContain('overlay-header bg-dark ');

    const searchButton = Array.from(
      sidebarToggle.querySelectorAll<HTMLButtonElement>('button'),
    ).find((button) => button.textContent?.includes('搜索'))!;

    await act(async () => {
      searchButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.querySelector('.overlay-header')!.className).toContain('show');
    const input = container.querySelector<HTMLInputElement>('.overlay-header input')!;
    expect(input.defaultValue).toBe('node');
    expect(input.placeholder).toBe('搜索文档');

    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set?.call(
        input,
        'trojan',
      );
      input.dispatchEvent(new Event('input', { bubbles: true }));
      await Promise.resolve();
    });

    expect(onChange).toHaveBeenCalledWith('trojan');

    const closeButton = container.querySelector<HTMLButtonElement>('.overlay-header .btn-dark')!;
    await act(async () => {
      closeButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.querySelector('.overlay-header')!.className).not.toContain('show');
  });

  it('prefers the bundled layout title prop over the route title', async () => {
    await renderLayout({ title: '自定义标题' });

    expect(container.querySelector('.v2board-container-title')!.textContent).toBe('自定义标题');
    expect(container.querySelector('.v2board-copyright')!.textContent).toBe('V2Board v1.7.4');
  });

  it('opens the avatar menu, keeps javascript logout href, and logs out to login', async () => {
    await renderLayout();

    const userButton = Array.from(
      container.querySelectorAll<HTMLButtonElement>('#page-header .dropdown button'),
    ).find((button) => button.textContent?.includes('user@example.com'))!;

    await act(async () => {
      userButton.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(container.querySelector('.dropdown-menu')!.className).toContain('show');
    const profile = Array.from(container.querySelectorAll('.dropdown-item')).find(
      (item) => item.textContent?.includes('个人中心'),
    ) as HTMLAnchorElement;
    const logoutLink = Array.from(container.querySelectorAll('.dropdown-item')).find(
      (item) => item.textContent?.includes('登出'),
    ) as HTMLAnchorElement;

    expect(profile.getAttribute('href')).toBe('/#/profile');
    expect(logoutLink.getAttribute('href')).toBe('javascript:void(0);');

    await act(async () => {
      logoutLink.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.logout).toHaveBeenCalledTimes(1);
    expect(mocks.navigate).toHaveBeenCalledWith('/login');
  });
});
